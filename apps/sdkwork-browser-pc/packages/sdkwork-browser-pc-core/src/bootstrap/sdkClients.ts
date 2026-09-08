import { createClient as createAppClient, type SdkworkAppClient } from "@sdkwork/browser-app-sdk";
import {
  createClient as createBackendClient,
  type SdkworkBackendClient,
} from "@sdkwork/browser-backend-sdk";
import { resolveBaseUrl } from "@sdkwork/sdk-common";

import { getRuntimeEnvironment } from "./environment.ts";

const DEV_AUTH_TOKEN =
  "tenant_id=sdkwork;user_id=browser;session_id=local;app_id=sdkwork-browser;auth_level=password";

const DEV_ACCESS_TOKEN =
  "tenant_id=sdkwork;user_id=browser;session_id=local;app_id=sdkwork-browser;environment=dev;deployment_mode=local";

function isDevAuthAllowed(): boolean {
  const mode = import.meta.env.MODE;
  if (mode === "production") {
    return false;
  }
  const explicit = import.meta.env.VITE_BROWSER_ALLOW_DEV_AUTH;
  if (explicit === "false" || explicit === "0") {
    return false;
  }
  return mode === "development" || explicit === "true" || explicit === "1";
}

function resolveAuthToken(): string {
  if (!isDevAuthAllowed()) {
    throw new Error(
      "Browser auth is not configured. Complete IAM login or enable dev auth in development.",
    );
  }
  return DEV_AUTH_TOKEN;
}

function resolveAccessToken(): string {
  const configured = String(import.meta.env.SDKWORK_ACCESS_TOKEN ?? "").trim();
  if (configured.length > 0) {
    return configured;
  }
  if (!isDevAuthAllowed()) {
    throw new Error(
      "Browser access token is not configured. Set SDKWORK_ACCESS_TOKEN for dev bootstrap or enable dev auth in development.",
    );
  }
  return DEV_ACCESS_TOKEN;
}

export function gatewayAuthHeaders(): Record<string, string> {
  return {
    Authorization: `Bearer ${resolveAuthToken()}`,
    "Access-Token": resolveAccessToken(),
    "X-SDKWork-Runtime-Environment": getRuntimeEnvironment(),
  };
}

export interface BrowserSdkClients {
  gatewayBaseUrl: string;
  appApiBaseUrl: string;
  backendApiBaseUrl: string;
  app: SdkworkAppClient;
  backend: SdkworkBackendClient;
}

export function resolveBrowserGatewayBaseUrl(): string {
  const configured =
    import.meta.env.VITE_BROWSER_GATEWAY_BASE_URL ??
    import.meta.env.VITE_BROWSER_APP_API_BASE_URL;
  if (typeof configured === "string" && configured.trim().length > 0) {
    return configured.replace(/\/$/, "");
  }
  // Resolve the shared SDKWORK_API_BASE_URL through @sdkwork/sdk-common (env +
  // brand + protocol aware), eliminating the hardcoded localhost default.
  return resolveBaseUrl().url;
}

export function resolveBrowserAppApiBaseUrl(): string {
  return resolveBrowserGatewayBaseUrl();
}

export function resolveBrowserBackendApiBaseUrl(): string {
  return resolveBrowserGatewayBaseUrl();
}

function createSdkClientConfig(baseUrl: string) {
  return {
    baseUrl,
    platform: "DESKTOP" as const,
    authToken: resolveAuthToken(),
    accessToken: resolveAccessToken(),
    headers: {
      "X-SDKWork-Runtime-Environment": getRuntimeEnvironment(),
    },
  };
}

export function createBrowserSdkClients(): BrowserSdkClients {
  const gatewayBaseUrl = resolveBrowserGatewayBaseUrl();
  const clientConfig = createSdkClientConfig(gatewayBaseUrl);
  return {
    gatewayBaseUrl,
    appApiBaseUrl: gatewayBaseUrl,
    backendApiBaseUrl: gatewayBaseUrl,
    app: createAppClient(clientConfig),
    backend: createBackendClient(clientConfig),
  };
}
