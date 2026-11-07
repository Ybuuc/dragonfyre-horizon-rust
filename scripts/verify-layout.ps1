# Read-only inspection script. Does not build, write, delete, or upload files.
param(
    [string]$Project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)
$ErrorActionPreference = 'Stop'
Write-Host "=== DRAGONFYRE project audit ===" -ForegroundColor Cyan
Write-Host "Project: $Project"
if (-not (Test-Path -LiteralPath $Project -PathType Container)) {
    throw "Project directory not found"
}
$files = @(
    "DRAGONFYRE.md",
    "STAGE07.md",
    "STAGE08-PERFORMANCE.md",
    "STAGE09-JOURNAL.md",
    "STAGE10-SURVIVAL.md",
    "work/build_dragonfyre_survival_pack.py",
    "work/check_dragonfyre_survival_pack.rs",
    "work/prepare_dragonfyre_survival_test.py",
    "work/play_dragonfyre_survival_test.py",
    "outputs/native-stage-10/pack-probe.log",
    "outputs/native-stage-10/survival-first.json",
    "outputs/native-stage-10/survival-second.json"
)
foreach ($relative in $files) {
    $f = Join-Path $Project $relative
    [pscustomobject]@{
        File = $relative
        Exists = Test-Path -LiteralPath $f -PathType Leaf
        Bytes = if (Test-Path -LiteralPath $f -PathType Leaf) { (Get-Item -LiteralPath $f).Length } else { $null }
        LastWriteTime = if (Test-Path -LiteralPath $f -PathType Leaf) { (Get-Item -LiteralPath $f).LastWriteTime } else { $null }
    }
}
Write-Host "`n=== Stage folders ===" -ForegroundColor Cyan
Get-ChildItem -LiteralPath (Join-Path $Project 'outputs') -Directory -ErrorAction SilentlyContinue |
    Select-Object Name,LastWriteTime
Write-Host "`n=== Git ===" -ForegroundColor Cyan
if (Test-Path -LiteralPath (Join-Path $Project '.git')) {
    & git -C $Project status --short
    & git -C $Project log -5 --oneline
} else {
    Write-Host "No .git folder in project root."
}
Write-Host "`nThis is a presence check, NOT proof of successful gameplay."
