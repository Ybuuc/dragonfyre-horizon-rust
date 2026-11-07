param(
    [Parameter(Mandatory=$true)][string]$ResourcesPath,
    [string]$GameDirectory = (Join-Path $PSScriptRoot 'game'),
    [string]$PlayerName = 'Dragonfyre',
    [string]$World = 'Dragonfyre Stage 07'
)
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'horizon-client.exe'
$cisco = Join-Path $PSScriptRoot 'Dragonfyre-cisco_mod.hmod'
$food = Join-Path $PSScriptRoot 'Dragonfyre-farmersdelight.hmod'
$resources = (Resolve-Path -LiteralPath $ResourcesPath).Path
foreach ($file in @($exe, $cisco, $food, (Join-Path $resources 'items.json'))) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing required file: $file" }
}
if (-not (Test-Path -LiteralPath $GameDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $GameDirectory | Out-Null
}
$game = (Resolve-Path -LiteralPath $GameDirectory).Path
$env:HORIZON_HOME = Join-Path $game 'horizon-home'
& $exe --resources $resources --game-dir $game --dev-mod $cisco --dev-mod $food --world $World --name $PlayerName --creative
exit $LASTEXITCODE
