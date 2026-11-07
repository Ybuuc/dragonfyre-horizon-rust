# Publish a minimal, sanitized AI status to GitHub without copying local logs or binaries.
# Compatible with Windows PowerShell 5.1. Only reports/AI_STATE.json is modified.
[CmdletBinding()]
param(
    [string]$Project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [string]$RunId = '',
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath $Project).Path
$gitRoot = (& git -C $projectRoot rev-parse --show-toplevel)
if ($LASTEXITCODE -ne 0 -or -not $gitRoot) { throw "Project must be a Git repository." }
$statePath = Join-Path $projectRoot 'reports\AI_STATE.json'
if (-not (Test-Path -LiteralPath $statePath -PathType Leaf)) {
    throw "Initial AI state is missing: $statePath. Pull main from GitHub first."
}

$state = Get-Content -LiteralPath $statePath -Raw -Encoding UTF8 | ConvertFrom-Json
if (-not $state.native_integration -or -not $state.quest_engine -or -not $state.stage10) {
    throw 'AI_STATE.json has an unexpected schema; refusing to overwrite.'
}

$runParent = Join-Path $projectRoot 'outputs\native-reproduction'
if (-not (Test-Path -LiteralPath $runParent -PathType Container)) {
    throw "No native reproduction runs: $runParent"
}
if ($RunId) {
    if ($RunId -notmatch '^dragonfyre-native-[a-zA-Z0-9_-]+$') {
        throw 'Invalid RunId; expected dragonfyre-native-*'
    }
    $runFolder = Join-Path $runParent $RunId
    if (-not (Test-Path -LiteralPath $runFolder -PathType Container)) {
        throw "Selected run directory is missing: $runFolder"
    }
    $run = Get-Item -LiteralPath $runFolder
} else {
    $all = @(Get-ChildItem -LiteralPath $runParent -Directory -Filter 'dragonfyre-native-*' |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'build-debug.json') -PathType Leaf } |
        Sort-Object -Property @{Expression={
            (Get-Item -LiteralPath (Join-Path $_.FullName 'build-debug.json')).LastWriteTimeUtc
        }} -Descending)
    if ($all.Count -eq 0) { throw 'No run with build-debug.json found.' }
    $run = $all[0]
}

$buildPath = Join-Path $run.FullName 'build-debug.json'
$applyPath = Join-Path $run.FullName 'apply-report.json'
if (-not (Test-Path -LiteralPath $buildPath -PathType Leaf)) { throw "Missing receipt: $buildPath" }
if (-not (Test-Path -LiteralPath $applyPath -PathType Leaf)) { throw "Missing receipt: $applyPath" }

$build = Get-Content -LiteralPath $buildPath -Raw -Encoding UTF8 | ConvertFrom-Json
$apply = Get-Content -LiteralPath $applyPath -Raw -Encoding UTF8 | ConvertFrom-Json

# Copy only allowlisted fields. Do not copy command lines, usernames, absolute paths,
# device IDs, client logs, world data, environment variables or authentication tokens.
$state.native_integration.run_id = $run.Name
$state.native_integration.mixins = [pscustomobject]@{
    status = [string]$apply.status
    injections = [int]$apply.receipt.injections
    modules = [int]$apply.receipt.modules
    engine_files_matched = [int]$apply.verified_engine_files
}
$binary = Join-Path $run.FullName 'horizon-client-debug.exe'
$hashMatch = $false
$binarySize = $null
if (Test-Path -LiteralPath $binary -PathType Leaf) {
    $realHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = [string]$build.sha256
    $hashMatch = ($expected.Length -eq 64 -and $realHash -eq $expected.ToLowerInvariant())
    $binarySize = [math]::Round((Get-Item -LiteralPath $binary).Length / 1MB, 2)
}
$buildStatus = [string]$build.status
$buildExit = if ($null -eq $build.exit_code) { $null } else { [int]$build.exit_code }
$state.native_integration.debug_build = [pscustomobject]@{
    status = $buildStatus
    exit_code = $buildExit
    profile = [string]$build.profile
    binary_sha256 = [string]$build.sha256
    hash_verified = [bool]$hashMatch
    size_mib_approx = $binarySize
    evidence = 'Local build-debug.json and SHA-256 checked by scripts/sync-ai-state.ps1'
}
$state.synced_report_source = 'Allowlisted fields from local apply-report.json/build-debug.json; raw logs and binary excluded'
$json = ($state | ConvertTo-Json -Depth 12) + "`n"
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($statePath, $json, $utf8)

Write-Host "AI state updated locally: reports/AI_STATE.json" -ForegroundColor Green
Write-Host "Run: $($run.Name)"
Write-Host "Mixin status: $($apply.status), injections=$($apply.receipt.injections), modules=$($apply.receipt.modules)"
Write-Host "Native debug: $buildStatus, exit=$buildExit, sha256 match=$hashMatch"
Write-Host 'The status contains no absolute paths, local logs, game saves or .exe file.'
if (-not $Push) {
    Write-Host 'GitHub not updated yet. Run this script with -Push after inspecting reports/AI_STATE.json.'
    exit 0
}

$branch = (& git -C $projectRoot branch --show-current).Trim()
if ($LASTEXITCODE -ne 0 -or $branch -ne 'main') { throw "Push only from main. Current branch: $branch" }
$origin = (& git -C $projectRoot remote get-url origin).Trim()
if ($LASTEXITCODE -ne 0 -or $origin -notmatch 'github\.com[:/]Ybuuc/dragonfyre-horizon-rust(\.git)?$') {
    throw "Unexpected Git remote; refusing to publish to: $origin"
}
& git -C $projectRoot add -- reports/AI_STATE.json
if ($LASTEXITCODE -ne 0) { throw 'git add failed' }
& git -C $projectRoot diff --cached --quiet -- reports/AI_STATE.json
if ($LASTEXITCODE -eq 0) {
    Write-Host 'Already synchronized: no AI_STATE.json changes to publish.'
    exit 0
}
if ($LASTEXITCODE -ne 1) { throw 'git diff failed' }
# Commit ONLY this one file. No other staged changes, binaries, logs or secrets.
& git -C $projectRoot commit --only -m 'reports: sync sanitized Dragonfyre AI state' -- reports/AI_STATE.json
if ($LASTEXITCODE -ne 0) { throw 'git commit failed' }
& git -C $projectRoot push origin main
if ($LASTEXITCODE -ne 0) {
    throw 'GitHub push failed. Local report commit is kept; do not use force-push. Pull/reconcile first.'
}
Write-Host 'PUBLISHED: reports/AI_STATE.json can now be read by a GitHub-connected AI agent.' -ForegroundColor Green
