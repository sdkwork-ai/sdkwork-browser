# Bootstrap SDKWork Browser workspace structure
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot

function Ensure-Dir($Path) {
    if (-not (Test-Path $Path)) { New-Item -ItemType Directory -Path $Path -Force | Out-Null }
}

$dirs = @(
    "apis/open-api/browser", "apis/app-api/browser", "apis/backend-api/browser",
    "apis/rpc", "apis/async", "apis/internal", "apis/examples", "apis/changelogs", "apis/tests",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-browser/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-browser/tests",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-browser/specs",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-core/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-commons/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-shell/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-core/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-desktop/src-tauri/src",
    "apps/sdkwork-browser-pc/packages/sdkwork-browser-pc-desktop/src-tauri/capabilities",
    "apps/sdkwork-browser-pc/src/bootstrap",
    "apps/sdkwork-browser-pc/config/browser",
    "apps/sdkwork-browser-pc/config/desktop",
    "apps/sdkwork-browser-pc/config/server",
    "apps/sdkwork-browser-pc/config/container",
    "apps/sdkwork-browser-pc/config/tauri",
    "apps/sdkwork-browser-pc/specs",
    "apps/sdkwork-browser-pc/.sdkwork/skills",
    "apps/sdkwork-browser-pc/.sdkwork/plugins",
    "apps/sdkwork-browser-h5/packages/sdkwork-browser-h5-react/src",
    "apps/sdkwork-browser-h5/packages/sdkwork-browser-h5-react/tests",
    "apps/sdkwork-browser-h5/packages/sdkwork-browser-h5-react/specs",
    "apps/sdkwork-browser-h5/src/bootstrap",
    "apps/sdkwork-browser-h5/config/browser",
    "apps/sdkwork-browser-h5/specs",
    "apps/sdkwork-browser-h5/.sdkwork/skills",
    "apps/sdkwork-browser-h5/.sdkwork/plugins",
    "apps/sdkwork-browser-flutter-mobile/lib/bootstrap",
    "apps/sdkwork-browser-flutter-mobile/specs",
    "apps/sdkwork-browser-flutter-mobile/.sdkwork/skills",
    "apps/sdkwork-browser-flutter-mobile/.sdkwork/plugins",
    "crates/sdkwork-platform-browser-repository-sqlx/src",
    "crates/sdkwork-platform-browser-repository-sqlx/migrations",
    "crates/sdkwork-platform-browser-repository-sqlx/specs",
    "crates/sdkwork-routes-browser-app-api/src",
    "crates/sdkwork-routes-browser-app-api/tests",
    "crates/sdkwork-routes-browser-app-api/specs",
    "crates/sdkwork-routes-browser-backend-api/src",
    "crates/sdkwork-routes-browser-backend-api/tests",
    "crates/sdkwork-routes-browser-backend-api/specs",
    "crates/sdkwork-browser-tauri-host/src",
    "crates/sdkwork-browser-tauri-host/specs",
    "packages/common/browser/sdkwork-browser-contracts/src",
    "packages/common/browser/sdkwork-browser-contracts/tests",
    "packages/common/browser/sdkwork-browser-service/src",
    "sdks/sdkwork-browser-app-sdk",
    "sdks/sdkwork-browser-backend-sdk",
    "etc/schemas", "etc/profiles", "etc/examples", "etc/defaults",
    "deployments/docker", "deployments/k8s", "deployments/systemd", "deployments/nginx", "deployments/runbooks",
    "docs/adr", "docs/runbooks", "docs/changelogs",
    "examples", "jobs/schedules", "jobs/queues", "jobs/batches", "jobs/runbooks", "jobs/packages",
    "plugins", "tools/validators", "tools/generators", "tools/migrations", "tools/operators",
    "tests/contract", "tests/integration", "tests/e2e", "tests/fixtures", "tests/static",
    ".sdkwork/skills", ".sdkwork/plugins"
)

foreach ($d in $dirs) {
    Ensure-Dir (Join-Path $Root $d)
}

Write-Host "Created directory structure under $Root"
