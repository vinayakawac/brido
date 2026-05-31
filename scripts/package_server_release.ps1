param(
    [string]$Tag = "dev",
    [string]$OutputDir = "release_assets/server",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$serverDir = Join-Path $repoRoot "brido_server"
$resolvedOutputDir = Join-Path $repoRoot $OutputDir
$exePath = Join-Path $serverDir "target/release/brido.exe"
$templatePath = Join-Path $serverDir ".env.local.template"
$readmePath = Join-Path $serverDir "README.md"

if (-not $SkipBuild) {
    Push-Location $serverDir
    try {
        cargo build --release
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $exePath)) {
    throw "Server executable not found at $exePath"
}

if (-not (Test-Path $templatePath)) {
    throw "Template file not found at $templatePath"
}

if (-not (Test-Path $readmePath)) {
    throw "README file not found at $readmePath"
}

New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null

$versionedExeName = "brido-$Tag.exe"
$bundleName = "brido-$Tag-bundle.zip"
$checksumName = "brido-$Tag.sha256"

$versionedExePath = Join-Path $resolvedOutputDir $versionedExeName
$bundlePath = Join-Path $resolvedOutputDir $bundleName
$checksumPath = Join-Path $resolvedOutputDir $checksumName

Copy-Item -Path $exePath -Destination $versionedExePath -Force
Copy-Item -Path $templatePath -Destination (Join-Path $resolvedOutputDir ".env.local.template") -Force
Copy-Item -Path $readmePath -Destination (Join-Path $resolvedOutputDir "README-server.md") -Force

if (Test-Path $bundlePath) {
    Remove-Item -Path $bundlePath -Force
}

Push-Location $resolvedOutputDir
try {
    Compress-Archive -Path @(
        $versionedExeName,
        ".env.local.template",
        "README-server.md"
    ) -DestinationPath $bundleName -Force
} finally {
    Pop-Location
}

$exeHash = (Get-FileHash -Algorithm SHA256 -Path $versionedExePath).Hash.ToLowerInvariant()
$bundleHash = (Get-FileHash -Algorithm SHA256 -Path $bundlePath).Hash.ToLowerInvariant()

Set-Content -Path $checksumPath -Value "$exeHash  $versionedExeName"
Add-Content -Path $checksumPath -Value "$bundleHash  $bundleName"

Write-Host "Server artifacts created at $resolvedOutputDir"
