import { resolveBrowserDistOutDir } from '../../../../sdkwork-specs/tools/browser-dist-layout.mjs';

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Connect, PluginOption } from "vite";
import { defineConfig, loadEnv } from 'vite';

const rootDir = path.dirname(fileURLToPath(import.meta.url));
const pcPackages = path.join(rootDir, "packages");

const devHost = "127.0.0.1";
const devPort = Number(process.env.SDKWORK_BROWSER_PC_DEV_PORT ?? 1620);

/**
 * Dev-only proxy that strips X-Frame-Options and CSP frame-ancestors headers
 * so the browser's iframe can embed any website. This mimics what a real
 * browser engine (Tauri WebView / CEF / Servo) does natively — the iframe
 * sandbox in web preview mode is the only way to render pages, and most
 * major sites block embedding via these headers.
 *
 * The proxy also injects a <base> tag into HTML responses so relative URLs
 * (CSS, JS, images, links) resolve against the original origin.
 */
function browserProxyPlugin(): PluginOption {
  const PROXY_PATH = "/__browser_proxy__";
  const MAX_REDIRECTS = 8;

  // Headers that prevent iframe embedding or cause decoding issues — stripped.
  // content-encoding/content-length must be removed because fetch() auto-
  // decompresses the body, so forwarding them makes the browser try to
  // decompress already-decompressed data (ERR_CONTENT_DECODING_FAILED).
  const STRIP_HEADERS = new Set([
    "x-frame-options",
    "content-security-policy",
    "content-security-policy-report-only",
    "cross-origin-opener-policy",
    "cross-origin-embedder-policy",
    "cross-origin-resource-policy",
    "permissions-policy",
    "x-content-type-options",
    "content-encoding",
    "content-length",
    "transfer-encoding",
  ]);

  return {
    name: "sdkwork-browser-proxy",
    configureServer(server) {
      server.middlewares.use(
        PROXY_PATH,
        (async (req: Connect.IncomingMessage, res: any) => {
          try {
            const requestUrl = new URL(req.url ?? "", `http://${req.headers.host}`);
            const target = requestUrl.searchParams.get("url");

            if (!target) {
              res.statusCode = 400;
              res.end(JSON.stringify({ error: "Missing 'url' query parameter" }));
              return;
            }

            let currentUrl = target;
            let redirectCount = 0;
            let response: Response | null = null;
            let lastError: Error | null = null;

            // Follow redirects manually so we can rewrite the final URL into
            // the <base> tag and know the true origin for relative paths.
            // Retry on network errors (DNS, TLS, timeout) — some sites are
            // flaky on first connect but succeed on retry.
            const MAX_FETCH_RETRIES = 2;
            for (let attempt = 0; attempt <= MAX_FETCH_RETRIES; attempt++) {
              try {
                while (redirectCount < MAX_REDIRECTS) {
                  const fetchResp = await fetch(currentUrl, {
                    headers: {
                      "User-Agent":
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
                      Accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
                      "Accept-Language": "en-US,en;q=0.9",
                    },
                    redirect: "manual",
                    // Give slow sites time to respond
                    signal: AbortSignal.timeout(15000),
                  });

                  // 3xx redirect — follow Location header
                  if ([301, 302, 303, 307, 308].includes(fetchResp.status)) {
                    const location = fetchResp.headers.get("location");
                    if (!location) break;
                    currentUrl = new URL(location, currentUrl).href;
                    redirectCount++;
                    continue;
                  }

                  response = fetchResp;
                  break;
                }
                break; // Success — exit retry loop
              } catch (fetchError) {
                lastError = fetchError instanceof Error ? fetchError : new Error(String(fetchError));
                // Only retry on network errors, not on successful responses
                if (attempt < MAX_FETCH_RETRIES) {
                  await new Promise((r) => setTimeout(r, 500 * (attempt + 1)));
                  continue;
                }
              }
            }

            if (!response) {
              res.statusCode = 502;
              res.setHeader("Content-Type", "application/json");
              res.end(JSON.stringify({
                error: `Proxy error: ${lastError?.message ?? "unknown"}`,
                url: currentUrl,
              }));
              return;
            }

            // Build clean response headers — strip all embedding-blocking headers
            const cleanHeaders = new Headers();
            response.headers.forEach((value, key) => {
              const lower = key.toLowerCase();
              if (STRIP_HEADERS.has(lower)) return;
              cleanHeaders.set(key, value);
            });

            // Set permissive CSP that allows embedding and resource loading
            cleanHeaders.set(
              "Content-Security-Policy",
              "default-src * 'unsafe-inline' 'unsafe-eval' data: blob:; frame-ancestors *;",
            );

            // Forward status and headers
            res.statusCode = response.status;
            cleanHeaders.forEach((value, key) => {
              res.setHeader(key, value);
            });

            const contentType = response.headers.get("content-type") ?? "";
            const isHtml = contentType.includes("text/html") || contentType.includes("application/xhtml");

            if (isHtml) {
              // Read HTML, inject <base> tag for relative path resolution,
              // and add a navigation interception script that routes link
              // clicks and form submissions through the proxy. Without this,
              // clicking a link inside the iframe navigates directly to the
              // target URL, which fails due to X-Frame-Options/CSP headers.
              let html = await response.text();
              const baseUrl = currentUrl;

              const baseTag = `<base href="${baseUrl}">`;
              // Inject right after <head> or at the very start of the document
              if (/<head[^>]*>/i.test(html)) {
                html = html.replace(/<head[^>]*>/i, (match) => `${match}${baseTag}`);
              } else if (/<html[^>]*>/i.test(html)) {
                html = html.replace(/<html[^>]*>/i, (match) => `${match}<head>${baseTag}</head>`);
              } else {
                html = `${baseTag}${html}`;
              }

              // Navigation interception script — routes all cross-origin
              // link clicks and form submissions through the proxy so the
              // iframe can embed the result. Same-origin navigations (e.g.,
              // hash changes, JavaScript routes) are left alone.
              //
              // Also notifies the parent window before navigation so the
              // browser shell can show a loading indicator (matching
              // Chrome/Edge behavior where link clicks show a spinner).
              const navScript = `<script>(function(){
var PROXY='/__browser_proxy__';
function proxy(u){return PROXY+'?url='+encodeURIComponent(u);}
function notifyParent(u){
  try{window.parent.postMessage({__browserNavigate:true,url:u},'*');}catch(e){}
}
document.addEventListener('click',function(e){
  var a=e.target.closest&&e.target.closest('a');if(!a)return;
  var href=a.getAttribute('href');if(!href)return;
  if(href[0]==='#'||href.indexOf('javascript:')===0)return;
  try{var r=new URL(href,document.baseURI);
    if(r.origin!==location.origin){
      e.preventDefault();e.stopPropagation();
      notifyParent(r.href);
      if(a.target==='_blank'||a.target==='_top'){window.open(proxy(r.href),a.target);}
      else{location.href=proxy(r.href);}
    }
  }catch(err){}
},true);
document.addEventListener('submit',function(e){
  var f=e.target;if(!f||f.tagName!=='FORM')return;
  var action=f.getAttribute('action')||'';
  try{var r=new URL(action,document.baseURI);
    if(r.origin!==location.origin){f.action=proxy(r.href);notifyParent(r.href);}
  }catch(err){}
},true);
})();</script>`;

              // Inject script before </body> or at end of document
              if (/<\/body>/i.test(html)) {
                html = html.replace(/<\/body>/i, (match) => `${navScript}${match}`);
              } else {
                html = `${html}${navScript}`;
              }

              res.setHeader("Content-Type", "text/html; charset=utf-8");
              res.setHeader("Content-Length", Buffer.byteLength(html, "utf-8"));
              res.end(html);
            } else {
              // Non-HTML: pipe through as-is (images, CSS, JS, etc.)
              const buffer = Buffer.from(await response.arrayBuffer());
              res.setHeader("Content-Length", buffer.length);
              res.end(buffer);
            }
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            res.statusCode = 502;
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify({ error: `Proxy error: ${message}` }));
          }
        }) as any,
      );
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', '');
  return {
    define: {
      'process.env.SDKWORK_ACCESS_TOKEN': JSON.stringify(env.SDKWORK_ACCESS_TOKEN ?? ''),
    },
    plugins: [react(), tailwindcss(), browserProxyPlugin()],
    resolve: {
      alias: {
      },
    },
    server: {
      host: devHost,
      port: devPort,
      strictPort: true,
    },
    build: {
      outDir: resolveBrowserDistOutDir(resolveViteEnvironment(mode, env)),
      emptyOutDir: true,
    },
  };
});
