# Read-only diagnostics for reproducible Stage 07 builds.
# Does not build, modify, copy, or delete any project data.
[CmdletBinding()]
param([string]$Project = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath $Project).Path
$dayRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $projectRoot))
$buildTools = Join-Path $dayRoot 'referenced-chatgpt-conversation-this-is-an\work\build-tools'

$requirements = @(
    [pscustomobject]@{ Role='Dragonfyre mod manifest'; Path=(Join-Path $projectRoot 'native\dragonfyre-quests\horizon-mixin.toml'); Required=$true },
    [pscustomobject]@{ Role='Tracked quest engine'; Path=(Join-Path $projectRoot 'native\dragonfyre-quests\engine\Cargo.toml'); Required=$true },
    [pscustomobject]@{ Role='Stage07 build script'; Path=(Join-Path $projectRoot 'work\build_native_stage07.py'); Required=$true },
    [pscustomobject]@{ Role='Pristine Horizon workspace'; Path=(Join-Path $projectRoot 'work\native-package-pristine\Cargo.toml'); Required=$true },
    [pscustomobject]@{ Role='Mixin executable'; Path=(Join-Path $projectRoot 'work\mixins-target\debug\horizon-mixins.exe'); Required=$true },
    [pscustomobject]@{ Role='Stage06 diagnostic command harness'; Path=(Join-Path $projectRoot 'work\native-package-applied-stage06-final\crates\horizon-client\src\game\command_harness.rs'); Required=$false },
    [pscustomobject]@{ Role='Rust cargo binary'; Path=(Join-Path $buildTools 'rustup\toolchains\stable-x86_64-pc-windows-gnu\bin\cargo.exe'); Required=$true },
    [pscustomobject]@{ Role='MinGW gcc executable'; Path=(Join-Path $buildTools 'mingw64\bin\gcc.exe'); Required=$false },
    [pscustomobject]@{ Role='MinGW strip executable'; Path=(Join-Path $buildTools 'mingw64\bin\strip.exe'); Required=$false },
    [pscustomobject]@{ Role='Existing Stage07 applied workspace'; Path=(Join-Path $projectRoot 'work\stage07-build-4337axm1\applied\Cargo.toml'); Required=$false }
)

Write-Host '=== Local build dependencies / present vs absent ===' -ForegroundColor Cyan
$requirements | ForEach-Object {
    [pscustomobject]@{
        Requirement = $_.Role
        Required = $_.Required
        Exists = Test-Path -LiteralPath $_.Path -PathType Leaf
        Path = $_.Path
    }
} | Format-Table -AutoSize -Wrap

Write-Host "`n=== Actual paths referred to by Stage07 script ===" -ForegroundColor Cyan
Write-Host "Project:  $projectRoot"
Write-Host "BuildTools (outside repo): $buildTools"
Write-Host "`n=== Files known to be tracked (local git) ===" -ForegroundColor Cyan
& git -C $projectRoot ls-files -- native/dragonfyre-quests/horizon-mixin.toml native/dragonfyre-quests/engine/Cargo.toml work/build_native_stage07.py
Write-Host "`nNote: existence is NOT reproducibility. Must rebuild from clean checkout and pinned source."
