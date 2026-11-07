# Standalone smoke test of the tracked quest engine. No Horizon installation needed.
# Does not touch launcher profiles, game saves, or existing Horizon trees.
[CmdletBinding()]
param(
    [string]$Cargo = "",
    [string]$RustBin = "",
    [string]$MinGwBin = "",
    [switch]$NoOffline
)
$ErrorActionPreference = "Stop"
$project = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifest = Join-Path $project "native\dragonfyre-quests\engine\Cargo.toml"
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { throw "Missing quest engine manifest: $manifest" }

$oldPath = $env:Path
$oldTarget = $env:CARGO_TARGET_DIR
try {
    if ($RustBin) {
        if (-not (Test-Path -LiteralPath $RustBin -PathType Container)) { throw "RustBin directory missing: $RustBin" }
        $env:Path = "$RustBin;$env:Path"
    }
    if ($MinGwBin) {
        if (-not (Test-Path -LiteralPath $MinGwBin -PathType Container)) { throw "MinGwBin directory missing: $MinGwBin" }
        $env:Path = "$MinGwBin;$env:Path"
    }

    if ($Cargo) {
        if (-not (Test-Path -LiteralPath $Cargo -PathType Leaf)) { throw "Cargo executable missing: $Cargo" }
        $cargoExe = (Resolve-Path -LiteralPath $Cargo).Path
    } else {
        $command = Get-Command cargo.exe -ErrorAction SilentlyContinue
        if (-not $command) { throw "cargo.exe not on PATH. Pass -Cargo or -RustBin." }
        $cargoExe = $command.Source
    }
    $rustc = Get-Command rustc.exe -ErrorAction Stop
    $targetRoot = Join-Path ([System.IO.Path]::GetTempPath()) "dragonfyre-quest-engine-target"
    $env:CARGO_TARGET_DIR = $targetRoot

    Write-Host "=== Quest engine standalone test ===" -ForegroundColor Cyan
    Write-Host "Manifest: $manifest"
    Write-Host "Cargo: $cargoExe"
    Write-Host "Rustc: $($rustc.Source)"
    Write-Host "Target dir: $targetRoot"
    & $cargoExe --version
    if ($LASTEXITCODE -ne 0) { throw "cargo --version failed ($LASTEXITCODE)" }
    & $rustc.Source -vV
    if ($LASTEXITCODE -ne 0) { throw "rustc -vV failed ($LASTEXITCODE)" }

    $args = @('test','--manifest-path',$manifest,'--locked')
    if (-not $NoOffline) { $args += '--offline' }
    Write-Host ("Command: cargo " + ($args -join ' ')) -ForegroundColor Cyan
    & $cargoExe @args
    if ($LASTEXITCODE -ne 0) { throw "Quest engine tests failed with exit code $LASTEXITCODE" }
    Write-Host "PASS: standalone quest engine tests passed. Horizon client and gameplay are NOT tested." -ForegroundColor Green
}
finally {
    $env:Path = $oldPath
    $env:CARGO_TARGET_DIR = $oldTarget
}
