import assert from "node:assert/strict";
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");

const requiredPaths = [
  "AGENTS.md",
  "sdkwork.workflow.json",
  "sdkwork.app.config.json",
  ".github/workflows/package.yml",
  "deployments/README.md",
  "etc/README.md",
  "scripts/README.md",
  "jobs/README.md",
  "tools/README.md",
  "plugins/README.md",
  "examples/README.md",
  "packages/common/browser/README.md",
  "specs/component.spec.json",
  "specs/README.md",
  "tests/README.md",
  "tests/integration/README.md",
  "tests/e2e/README.md",
  "tests/fixtures/README.md",
  "docs/adr/ADR-002-browser-engine-standard.md",
  "docs/adr/ADR-003-browser-rpc-discovery-deferred.md",
  "docs/adr/ADR-004-browser-crate-naming-alignment.md",
  "docs/adr/ADR-005-pc-package-family-alignment.md",
  "etc/profiles/browser.default.yaml",
  "apis/app-api/platform/openapi.yaml",
  "apis/app-api/platform/routes/README.md",
  "apis/backend-api/platform/openapi.yaml",
  "apis/backend-api/platform/routes/README.md",
  "tests/fixtures/database/sqlite/ddl/baseline/0001_browser_baseline.sql",
  "database/ddl/baseline/postgres/0001_browser_baseline.sql",
  "crates/sdkwork-api-browser-standalone-gateway/Cargo.toml",
  "crates/sdkwork-platform-browser-repository-sqlx/Cargo.toml",
  "crates/sdkwork-browser-database-host/Cargo.toml",
  "crates/sdkwork-browser-shared-service/Cargo.toml",
  "crates/sdkwork-routes-browser-app-api/Cargo.toml",
  "crates/sdkwork-routes-browser-app-api/Cargo.toml",
  "crates/sdkwork-routes-browser-backend-api/Cargo.toml",
  "sdks/_route-manifests/app-api/sdkwork-routes-browser-app-api.route-manifest.json",
  "sdks/_route-manifests/backend-api/sdkwork-routes-browser-backend-api.route-manifest.json",
  "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-core/package.json",
  "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-commons/package.json",
  "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-shell/package.json",
  "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-browser/package.json",
  "apps/sdkwork-browser-pc/sdkwork.app.config.json",
  "apps/sdkwork-browser-h5/package.json",
  "apps/sdkwork-browser-h5/config/browser/runtime-env.development.example.json",
  "apps/sdkwork-browser-h5/config/host/capacitor.development.example.json",
  "apps/sdkwork-browser-flutter-mobile/config/app/runtime-env.development.example.json",
  "apps/sdkwork-browser-h5/specs/README.md",
  "apps/sdkwork-browser-flutter-mobile/specs/README.md",
];

for (const rel of requiredPaths) {
  assert.ok(existsSync(join(root, rel)), `missing required path: ${rel}`);
}

const cargoToml = readFileSync(join(root, "Cargo.toml"), "utf8");
for (const dep of [
  "sdkwork-database-config",
  "sdkwork-web-core",
  "sdkwork-utils-rust",
  "sdkwork-api-browser-standalone-gateway",
  "sdkwork-platform-browser-repository-sqlx",
]) {
  assert.match(cargoToml, new RegExp(dep), `Cargo.toml must declare ${dep}`);
}
for (const retired of [
  "sdkwork-browser-app-server",
  "sdkwork-browser-storage-sqlx-rust",
  "sdkwork-discovery",
  "sdkwork-browser-pc-react",
]) {
  assert.doesNotMatch(cargoToml, new RegExp(retired), `${retired} must not appear in workspace Cargo.toml`);
}

const pcPackageJson = readFileSync(
  join(root, "apps/sdkwork-browser-pc/package.json"),
  "utf8",
);
for (const pkg of [
  "@sdkwork/browser-pc-core",
  "@sdkwork/browser-pc-commons",
  "@sdkwork/browser-pc-shell",
  "@sdkwork/browser-pc-browser",
]) {
  assert.match(pcPackageJson, new RegExp(pkg), `PC app must depend on ${pkg}`);
}
assert.doesNotMatch(pcPackageJson, /sdkwork-browser-pc-react/, "retired pc-react package name");

const repositoryToml = readFileSync(
  join(root, "crates/sdkwork-platform-browser-repository-sqlx/Cargo.toml"),
  "utf8",
);
assert.match(repositoryToml, /sdkwork-database-sqlx/);

const apiServerToml = readFileSync(
  join(root, "crates/sdkwork-api-browser-standalone-gateway/Cargo.toml"),
  "utf8",
);
assert.match(apiServerToml, /sdkwork-web-bootstrap/);

const appRouteToml = readFileSync(
  join(root, "crates/sdkwork-routes-browser-app-api/Cargo.toml"),
  "utf8",
);
assert.match(appRouteToml, /sdkwork-web-core/);

const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
assert.ok(packageJson.scripts["dev:server"], "package.json must expose dev:server");
assert.ok(packageJson.scripts["gateway:run"], "package.json must expose gateway:run");

const workflow = JSON.parse(readFileSync(join(root, "sdkwork.workflow.json"), "utf8"));
const workflowDeps = workflow.dependencies.map((entry) => entry.id);
for (const dep of ["sdkwork-database", "sdkwork-web-framework", "sdkwork-utils"]) {
  assert.ok(workflowDeps.includes(dep), `sdkwork.workflow.json must declare ${dep}`);
}
assert.ok(!workflowDeps.includes("sdkwork-discovery"), "discovery must not be in workflow yet");

const dbManifest = JSON.parse(
  readFileSync(join(root, "database/database.manifest.json"), "utf8"),
);
assert.ok(dbManifest.engines.includes("postgres"), "database manifest must list postgres engine");

const rootManifest = JSON.parse(readFileSync(join(root, "sdkwork.app.config.json"), "utf8"));
assert.equal(rootManifest.schemaVersion, 3);
assert.equal(rootManifest.app.key, "sdkwork-browser");

const pcManifest = JSON.parse(
  readFileSync(join(root, "apps/sdkwork-browser-pc/sdkwork.app.config.json"), "utf8"),
);
assert.equal(pcManifest.schemaVersion, 3);
assert.equal(pcManifest.app.key, "sdkwork-browser-pc");

for (const app of ["sdkwork-browser-pc", "sdkwork-browser-h5", "sdkwork-browser-flutter-mobile"]) {
  assert.ok(existsSync(join(root, "apps", app, ".sdkwork/README.md")), `${app} missing .sdkwork/README.md`);
  assert.ok(
    existsSync(join(root, "apps", app, ".sdkwork/plugins/README.md")),
    `${app} missing .sdkwork/plugins/README.md`,
  );
}

const sharedService = readFileSync(
  join(root, "crates/sdkwork-browser-shared-service/src/lib.rs"),
  "utf8",
);
for (const helper of ["new_request_id", "normalize_text", "is_valid_url", "sdkwork_utils_rust::uuid"]) {
  assert.match(sharedService, new RegExp(helper), `shared-service must expose ${helper}`);
}

const handlers = readFileSync(
  join(root, "crates/sdkwork-routes-browser-app-api/src/handlers.rs"),
  "utf8",
);
assert.doesNotMatch(handlers, /Uuid::new_v4/, "handlers must use shared-service request ids");

const cratesDir = join(root, "crates");
for (const entry of readdirSync(cratesDir)) {
  const crateDir = join(cratesDir, entry);
  if (!statSync(crateDir).isDirectory()) continue;
  assert.ok(
    existsSync(join(crateDir, "specs/component.spec.json")),
    `missing component spec for crate: ${entry}`,
  );
}

console.log("browser governance checks passed");
