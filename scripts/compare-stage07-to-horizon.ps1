# Read-only comparison of Horizon source trees. Does not move or delete files.
# Stage07 source lives under stage07-build-*/applied, not at its parent root.
[CmdletBinding()]
param(
    [string]$Project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [string]$Baseline = 'work\horizon-66f3981',
    [string]$Candidate = 'work\stage07-build-4337axm1\applied',
    [int]$Show = 100
)
$ErrorActionPreference = 'Stop'
$baseRoot = (Join-Path $Project $Baseline).TrimEnd('\')
$candidateRoot = (Join-Path $Project $Candidate).TrimEnd('\')
if (-not (Test-Path -LiteralPath $baseRoot -PathType Container)) { throw "Missing baseline: $baseRoot" }
if (-not (Test-Path -LiteralPath $candidateRoot -PathType Container)) { throw "Missing candidate: $candidateRoot" }

# First-pass source/configuration audit. Binary assets need a separate manifest.
$extensions = @('.rs','.toml','.py','.ps1','.patch','.diff','.json','.yaml','.yml','.ron','.md','.lock','.mcmeta')
$ignoredParts = @('target', '.git', 'node_modules', 'build', 'dist', '__pycache__', '.gradle')

function Get-SourceMap([string]$root) {
    $map = @{}
    foreach ($file in (Get-ChildItem -LiteralPath $root -Recurse -File -Force)) {
        if ($extensions -notcontains $file.Extension.ToLowerInvariant()) { continue }
        $relative = $file.FullName.Substring($root.Length).TrimStart([char]'\')
        $parts = $relative -split '[\\/]'
        $skip = $false
        foreach ($part in $parts) {
            if ($ignoredParts -contains $part) { $skip = $true; break }
        }
        if ($skip) { continue }
        $map[$relative] = $file.FullName
    }
    return ,$map
}

Write-Host "Baseline:  $baseRoot"
Write-Host "Candidate: $candidateRoot"
$baseFiles = Get-SourceMap $baseRoot
$candidateFiles = Get-SourceMap $candidateRoot
$results = [System.Collections.Generic.List[object]]::new()

foreach ($relative in $candidateFiles.Keys) {
    if (-not $baseFiles.ContainsKey($relative)) {
        $results.Add([pscustomobject]@{ Status='ONLY_IN_STAGE07'; Path=$relative })
        continue
    }
    $a = (Get-FileHash -LiteralPath $baseFiles[$relative] -Algorithm SHA256).Hash
    $b = (Get-FileHash -LiteralPath $candidateFiles[$relative] -Algorithm SHA256).Hash
    if ($a -ne $b) { $results.Add([pscustomobject]@{ Status='MODIFIED_IN_STAGE07'; Path=$relative }) }
}
foreach ($relative in $baseFiles.Keys) {
    if (-not $candidateFiles.ContainsKey($relative)) {
        $results.Add([pscustomobject]@{ Status='MISSING_FROM_STAGE07'; Path=$relative })
    }
}

Write-Host "`nBaseline source/config files: $($baseFiles.Count)"
Write-Host "Candidate source/config files: $($candidateFiles.Count)"
Write-Host "`n=== Counts ===" -ForegroundColor Cyan
if ($results.Count -eq 0) {
    Write-Host 'No differences within the selected source/config extensions.'
} else {
    $results | Group-Object Status | Sort-Object Name | Select-Object Count, Name | Format-Table -AutoSize
    Write-Host "`n=== First $Show changed paths ===" -ForegroundColor Cyan
    $results | Sort-Object Status, Path | Select-Object -First $Show | Format-Table -AutoSize
}
Write-Host "`nRead-only comparison complete. This is NOT a build, functional, or gameplay verification."
