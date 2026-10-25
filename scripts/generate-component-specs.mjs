import { mkdirSync, writeFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative } from "node:path";

const root = join(import.meta.dirname, "..");
const specsRoot = join(root, "..", "sdkwork-specs");

const canonicalSpecs = [
  ["COMPONENT_SPEC.md", "Component-local contract and discovery rules."],
  ["CODE_STYLE_SPEC.md", "Authored source structure and generated code boundaries."],
  ["NAMING_SPEC.md", "Canonical SDKWork naming rules."],
  ["MODULE_SPEC.md", "Reusable module and package boundary rules."],
  ["TEST_SPEC.md", "Verification and contract testing expectations."],
  ["SECURITY_SPEC.md", "Protected API and security rules."],
];

function relativeSpecPath(fromDir) {
  const rel = relative(fromDir, specsRoot).replaceAll("\\", "/");
  return `${rel}/COMPONENT_SPEC.md`.replace("/COMPONENT_SPEC.md", "");
}

function writeComponentSpec(targetDir, component) {
  const specDir = join(targetDir, "specs");
  mkdirSync(specDir, { recursive: true });
  const specPath = join(specDir, "component.spec.json");
  const prefix = relativeSpecPath(targetDir);

  const payload = {
    schemaVersion: 1,
    kind: "sdkwork.component.spec",
    component: {
      name: component.name,
      displayName: component.displayName,
      version: "0.1.0",
      type: component.type,
      root: component.root,
      domain: "platform",
      capability: "browser",
      languages: component.languages,
      generated: false,
      manifests: component.manifests,
    },
    canonicalSpecs: canonicalSpecs.map(([file, purpose]) => ({
      file,
      path: `${prefix}/${file}`,
      purpose,
    })),
    contracts: {
      publicExports: ["."],
      runtimeEntrypoints: component.runtimeEntrypoints ?? [],
      routeManifest: component.routeManifest ?? undefined,
      sdkClients: component.sdkClients ?? [],
      events: [],
      configKeys: [],
    },
    verification: {
      commands: component.verification ?? ["cargo test"],
    },
  };

  writeFileSync(specPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
}

function walkCrates() {
  const cratesDir = join(root, "crates");
  for (const entry of readdirSync(cratesDir)) {
    const crateDir = join(cratesDir, entry);
    if (!statSync(crateDir).isDirectory()) continue;
    if (!existsSync(join(crateDir, "Cargo.toml"))) continue;

    const type = entry.includes("routes")
      ? "rust-route-crate"
      : entry.includes("storage") || entry.includes("database-host") || entry.includes("repository-sqlx")
        ? "rust-storage-crate"
        : "rust-service";

    writeComponentSpec(crateDir, {
      name: entry,
      displayName: entry
        .split("-")
        .map((part) => part[0].toUpperCase() + part.slice(1))
        .join(" "),
      type,
      root: `sdkwork-browser/crates/${entry}`,
      languages: ["rust"],
      manifests: ["Cargo.toml"],
      routeManifest: type === "rust-route-crate"
        ? "sdks/_route-manifests/<surface>/<packageName>.route-manifest.json"
        : undefined,
      verification: ["cargo test -p " + entry.replaceAll("-", "_")],
    });
  }
}

function writeRootSpec() {
  writeComponentSpec(root, {
    name: "@sdkwork/browser-workspace",
    displayName: "SDKWork Browser Workspace",
    type: "workspace",
    root: "sdkwork-browser",
    languages: ["typescript", "rust"],
    manifests: ["package.json", "Cargo.toml", "sdkwork.workflow.json"],
    runtimeEntrypoints: ["apps/sdkwork-browser-pc"],
    verification: ["pnpm verify"],
  });
}

function writeAppsSpecs() {
  for (const app of ["sdkwork-browser-pc", "sdkwork-browser-h5", "sdkwork-browser-flutter-mobile"]) {
    const appDir = join(root, "apps", app);
    if (!existsSync(appDir)) continue;
    writeComponentSpec(appDir, {
      name: app,
      displayName: app,
      type: "application",
      root: `sdkwork-browser/apps/${app}`,
      languages: app.includes("flutter") ? ["dart"] : ["typescript"],
      manifests: ["agent.app.config.json", "package.json"],
      runtimeEntrypoints: ["."],
      verification: ["pnpm verify"],
    });
  }
}

writeRootSpec();
walkCrates();
writeAppsSpecs();
console.log("component specs generated");
