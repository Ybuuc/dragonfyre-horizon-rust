# Test the tracked quest engine from an isolated GitHub clone.
# No Horizon launch, no player saves. Only a uniquely named temp directory is used.
[CmdletBinding()]
param(
    [string]$Repository = 'https://github.com/Ybuuc/dragonfyre-horizon-rust.git',
    [string]$RustBin = '',
    [string]$MinGwBin = '',
    [switch]$KeepCheckout
)
$ErrorActionPreference = 'Stop'
$oldPath = $env:Path
$oldCargoHome = $env:CARGO_HOME
$oldCargoTarget = $env:CARGO_TARGET_DIR
$oldCargoOffline = $env:CARGO_NET_OFFLINE
$taskDir = Join-Path ([IO.Path]::GetTempPath()) ('dragonfyre-clean-checkout-' + [guid]::NewGuid().ToString('N'))
$clone = Join-Path $taskDir 'repo'
$success = $false

try {
    if ($RustBin) {
        if (-not (Test-Path -LiteralPath $RustBin -PathType Container)) { throw "Missing RustBin: $RustBin" }
        $env:Path = "$RustBin;$env:Path"
    }
    if ($MinGwBin) {
        if (-not (Test-Path -LiteralPath $MinGwBin -PathType Container)) { throw "Missing MinGwBin: $MinGwBin" }
        $env:Path = "$MinGwBin;$env:Path"
    }
    $cargo = (Get-Command cargo.exe -ErrorAction Stop).Source
    $rustc = (Get-Command rustc.exe -ErrorAction Stop).Source
    $git = (Get-Command git.exe -ErrorAction Stop).Source
    $null = New-Item -ItemType Directory -Path $taskDir -Force
    Write-Host "Cloning into clean temporary directory: $clone" -ForegroundColor Cyan
    & $git clone --depth 1 --single-branch --branch main $Repository $clone
    if ($LASTEXITCODE -ne 0) { throw "git clone exited with code $LASTEXITCODE" }

    $manifest = Join-Path $clone 'native\dragonfyre-quests\engine\Cargo.toml'
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { throw "Missing Cargo.toml in GitHub checkout" }
    $env:CARGO_HOME = Join-Path $taskDir 'isolated-cargo-home'
    $env:CARGO_TARGET_DIR = Join-Path $taskDir 'isolated-target'
    $env:CARGO_NET_OFFLINE = 'true'
    Write-Host "Git revision:" -ForegroundColor Cyan
    & $git -C $clone rev-parse HEAD
    Write-Host "Rust compiler: $rustc" -ForegroundColor Cyan
    & $rustc --version
    Write-Host "Cargo executable: $cargo" -ForegroundColor Cyan
    & $cargo --version
    Write-Host 'Running standalone Rust tests from isolated clone (offline, locked)...' -ForegroundColor Cyan
    & $cargo test --manifest-path $manifest --locked --offline
    if ($LASTEXITCODE -ne 0) { throw "clean-clone engine tests failed with exit code $LASTEXITCODE" }
    Write-Host 'PASS: clean GitHub clone independently compiles and tests quest engine.' -ForegroundColor Green
    Write-Host 'Note: Horizon build, external resources, Stage10 and gameplay remain unverified.'
    $success = $true
}
finally {
    $env:Path = $oldPath
    $env:CARGO_HOME = $oldCargoHome
    $env:CARGO_TARGET_DIR = $oldCargoTarget
    $env:CARGO_NET_OFFLINE = $oldCargoOffline
    if ($success -and -not $KeepCheckout) {
        # This directory was generated with a fresh GUID inside TEMP, and nothing else is removed.
        if ($taskDir.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase) -and
            (Split-Path -Leaf $taskDir) -like 'dragonfyre-clean-checkout-*') {
            Remove-Item -LiteralPath $taskDir -Force -Recurse
            Write-Host 'Removed only the temporary checkout created by this test.'
        } else {
            Write-Warning "Unexpected temp path, leaving untouched: $taskDir"
        }
    } elseif (Test-Path -LiteralPath $taskDir) {
        Write-Host "Temporary checkout retained for inspection: $taskDir"
    }
}
