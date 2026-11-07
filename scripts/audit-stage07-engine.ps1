# Read-only follow-up audit: compare Stage 07 embedded quest engine with GitHub-tracked source.
# Also summarize Horizon integration differences without modifying local work.
[CmdletBinding()]
param([string]$Project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path)
$ErrorActionPreference = 'Stop'
$repoEngine = Join-Path $Project 'native\dragonfyre-quests\engine'
$stageEngine = Join-Path $Project 'work\stage07-build-4337axm1\applied\native-crates\dragonfyre_quests\engine'
$oldEngine = Join-Path $Project 'work\horizon-66f3981\crates\dragonfyre-quests-engine'
$horizonBase = Join-Path $Project 'work\horizon-66f3981'
$stageApplied = Join-Path $Project 'work\stage07-build-4337axm1\applied'

foreach ($p in @($repoEngine,$stageEngine,$oldEngine,$horizonBase,$stageApplied)) {
    if (-not (Test-Path -LiteralPath $p -PathType Container)) { throw "Required directory missing: $p" }
}
Write-Host '=== Quest engine versions ===' -ForegroundColor Cyan
foreach ($p in @($repoEngine,$stageEngine,$oldEngine)) {
    Write-Host "Source: $p"
    $m = Join-Path $p 'Cargo.toml'
    if (Test-Path $m) {
        Select-String -LiteralPath $m -Pattern '^name\s*=','^version\s*=' |
            ForEach-Object { Write-Host ("  " + $_.Line.Trim()) }
    }
}

# Source file hashes and presence only. File names are compared relative to respective engine roots.
$roots = [ordered]@{ 'PUBLISHED'=$repoEngine; 'STAGE07'=$stageEngine; 'HORIZON_COPY'=$oldEngine }
$records = @{}
foreach ($key in $roots.Keys) {
    $p = $roots[$key]
    $index = @{}
    foreach ($f in (Get-ChildItem -LiteralPath $p -Recurse -File)) {
        if ($f.Extension -notin @('.rs','.toml','.lock')) { continue }
        $rel = $f.FullName.Substring($p.Length).TrimStart([char]'\')
        $index[$rel] = (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash
    }
    $records[$key] = $index
}
$allPaths = @($records.Values | ForEach-Object { $_.Keys } | Sort-Object -Unique)
Write-Host "`n=== Three-way engine comparison ===" -ForegroundColor Cyan
foreach ($rel in $allPaths) {
    $a = $records['PUBLISHED'][$rel]
    $b = $records['STAGE07'][$rel]
    $c = $records['HORIZON_COPY'][$rel]
    $status = if ($null -eq $b) { 'NOT_IN_STAGE07' }
        elseif ($null -eq $a) { 'NOT_IN_GITHUB' }
        elseif ($a -eq $b) { 'MATCH_GITHUB_STAGE07' }
        else { 'DIFFERENT_GITHUB_STAGE07' }
    [pscustomobject]@{ File=$rel; Status=$status; InOldHorizon=($null -ne $c); Stage07MatchesOld=($null -ne $b -and $b -eq $c) }
} | Out-Host

Write-Host "`n=== Horizon integration source diffs (Stage 07 vs Horizon copy) ===" -ForegroundColor Cyan
$paths = @(
    'Cargo.toml',
    'Cargo.lock',
    'crates\horizon-client\Cargo.toml',
    'crates\horizon-client\src\game.rs',
    'crates\horizon-client\src\game\command_harness.rs',
    'crates\horizon-client\src\game\lan.rs',
    'crates\horizon-client\src\game\native_hooks.rs',
    'native-data\dragonfyre_quests\messages.ru.json'
)
foreach ($rel in $paths) {
    $left = Join-Path $horizonBase $rel
    $right = Join-Path $stageApplied $rel
    Write-Host "--- $rel"
    if ((Test-Path -LiteralPath $left -PathType Leaf) -and (Test-Path -LiteralPath $right -PathType Leaf)) {
        & git -c core.autocrlf=false --no-pager diff --no-index --numstat -- $left $right
        if ($LASTEXITCODE -notin @(0,1)) { Write-Warning "Comparison failed for: $rel" }
    } else {
        Write-Host 'One side missing.'
    }
}
Write-Host "`n=== Tracked mixin manifest ===" -ForegroundColor Cyan
$mixin = Join-Path $Project 'native\dragonfyre-quests\horizon-mixin.toml'
if (Test-Path $mixin) {
    Select-String -LiteralPath $mixin -Pattern '^\[\[injection\]\]','^\[\[module\]\]','^id\s*=','^source\s*=' |
        Select-Object -Last 50 |
        ForEach-Object { Write-Host $_.Line }
}
Write-Host "`nRead-only audit finished. Differences do not prove which build is active."
