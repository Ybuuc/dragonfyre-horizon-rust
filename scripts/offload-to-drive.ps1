# Offload selected large Dragonfyre folders to the user's private rclone Google Drive.
# Windows PowerShell 5.1. Never deletes on upload; deletion requires a second explicit step.
[CmdletBinding()]
param(
    [ValidateSet('List','Archive','Prune')][string]$Mode = 'List',
    [string]$RelativePath = '',
    [string]$RemoteRoot = 'gdrive:Dragonfyre-Archive/dragonfyre',
    [string]$RcloneExe = 'rclone',
    [string]$ConfirmDeletion = ''
)
$ErrorActionPreference = 'Stop'
$project = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\')
$receiptFolder = Join-Path $project 'outputs\offload-receipts'
$validRemote = $RemoteRoot -match '^gdrive:[^\r\n]*$'
if (-not $validRemote) {
    throw "This workflow only supports the configured private gdrive: remote."
}
$remote = $RemoteRoot.TrimEnd('/')

function Get-FolderStats([string]$folder) {
    $files = @(Get-ChildItem -LiteralPath $folder -Recurse -File -Force -ErrorAction Stop)
    $bytes = [long]0
    foreach ($item in $files) { $bytes += $item.Length }
    return [pscustomobject]@{
        Files = $files.Count
        GiB = [math]::Round(($bytes / 1GB), 3)
        Bytes = $bytes
    }
}

if ($Mode -eq 'List') {
    Write-Host "=== Large local Dragonfyre directories (no changes) ===" -ForegroundColor Cyan
    $entries = @()
    foreach ($name in @('work','outputs')) {
        $root = Join-Path $project $name
        if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
        foreach ($dir in (Get-ChildItem -LiteralPath $root -Directory -Force)) {
            $stats = Get-FolderStats $dir.FullName
            $entries += [pscustomobject]@{
                Folder = "$name/$($dir.Name)"
                GiB = $stats.GiB
                Files = $stats.Files
            }
        }
    }
    $entries | Sort-Object GiB -Descending | Format-Table -AutoSize
    Write-Host 'Listing only. No data has been uploaded or removed.'
    return
}

if ([string]::IsNullOrWhiteSpace($RelativePath)) { throw 'Specify -RelativePath, e.g. work/stage07-build-4337axm1' }
$rel = $RelativePath.Trim() -replace '\\','/'
if ($rel -notmatch '^(work|outputs)/[^/].+$' -or
    $rel -match '(^|/)\.\.(/|$)' -or $rel -match '[\r\n:]' ) {
    throw 'Only a specific subdirectory of work/ or outputs/ is allowed; never the project root.'
}
$source = Join-Path $project ($rel -replace '/', '\')
if (-not (Test-Path -LiteralPath $source -PathType Container)) { throw "Missing local source folder: $source" }
$source = (Resolve-Path -LiteralPath $source).Path.TrimEnd('\')
if (-not $source.StartsWith(($project+'\'),[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Folder resolves outside the project root.'
}
# Prevent archiving/removing real worlds, accounts, credentials, and the source Git repository.
$blockedSegments = @('saves','playerdata','instances','accounts','credentials','.git','offload-receipts','config')
$parts = @($rel.ToLowerInvariant() -split '/')
foreach ($part in $parts) {
    if ($blockedSegments -contains $part) { throw "Protected path segment: $part" }
}
$unsafe = @(Get-ChildItem -LiteralPath $source -Recurse -Force -ErrorAction Stop |
    Where-Object {
        $_.Attributes -band [IO.FileAttributes]::ReparsePoint -or
        $_.Name -match '(?i)^(saves|playerdata|accounts\.json|credentials\.json|tokens\.json|service\.json|\.env|\.git)$'
    } | Select-Object -First 5)
if ($unsafe.Count -gt 0) {
    Write-Host 'Refusing to offload a folder containing protected data or reparse points:'
    $unsafe | ForEach-Object { Write-Host $_.FullName }
    throw 'Choose a narrower directory; do not delete game saves or secrets.'
}

$executable = (Get-Command $RcloneExe -ErrorAction SilentlyContinue)
if (-not $executable) { throw 'rclone CLI is not on PATH. Specify -RcloneExe with full path to rclone.exe.' }
& $RcloneExe listremotes | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Cannot read rclone config.' }

$destination = "$remote/$rel"
$receiptFile = Join-Path $receiptFolder (($rel -replace '[^a-zA-Z0-9._-]', '_') + '.json')
$statsBefore = Get-FolderStats $source
Write-Host "Source:      $source"
Write-Host "Google Drive:$destination"
Write-Host "Data:        $($statsBefore.GiB) GiB / $($statsBefore.Files) files" -ForegroundColor Cyan
Write-Host 'Drive transfer is private to the configured account; no public GitHub files are created.'

if ($Mode -eq 'Archive') {
    Write-Host 'Copying to Google Drive; source files remain on disk...' -ForegroundColor Cyan
    & $RcloneExe copy $source $destination --transfers 2 --checkers 2 --progress
    if ($LASTEXITCODE -ne 0) { throw "rclone copy failed: $LASTEXITCODE. Local files unchanged." }
} elseif ($Mode -eq 'Prune') {
    if (-not (Test-Path -LiteralPath $receiptFile -PathType Leaf)) {
        throw "No previous successful Archive receipt: $receiptFile"
    }
    $prior = Get-Content -LiteralPath $receiptFile -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($prior.verified -ne $true -or $prior.remote -ne $destination -or $prior.relative_path -ne $rel) {
        throw 'Receipt does not match the selected folder and destination.'
    }
    if ($ConfirmDeletion -cne 'DELETE_VERIFIED_LOCAL_COPY') {
        throw "To remove after re-check, pass -ConfirmDeletion DELETE_VERIFIED_LOCAL_COPY"
    }
}

Write-Host 'Verifying EVERY remote file by downloading and comparing bytes (no extra local copy)...' -ForegroundColor Cyan
& $RcloneExe check $source $destination --one-way --download --checkers 2
if ($LASTEXITCODE -ne 0) {
    throw "Verification failed with exit code $LASTEXITCODE. NOTHING was deleted."
}
Write-Host 'Verified: every local file matches the uploaded Google Drive copy.' -ForegroundColor Green
if ($Mode -eq 'Archive') {
    New-Item -ItemType Directory -Path $receiptFolder -Force | Out-Null
    $receipt = [ordered]@{
        verified = $true
        relative_path = $rel
        remote = $destination
        file_count = $statsBefore.Files
        bytes = $statsBefore.Bytes
        verified_utc = [DateTime]::UtcNow.ToString('o')
        check = 'rclone check --one-way --download'
        local_copy_removed = $false
    }
    $encoded = ($receipt | ConvertTo-Json -Depth 5) + "`n"
    [IO.File]::WriteAllText($receiptFile, $encoded, [Text.UTF8Encoding]::new($false))
    Write-Host "Archive complete; local files kept. Receipt: $receiptFile" -ForegroundColor Green
    Write-Host "To free space, run -Mode Prune -RelativePath '$rel' -ConfirmDeletion DELETE_VERIFIED_LOCAL_COPY"
    return
}

# Prune only after separate archive run + fresh byte-level verification.
$statsNow = Get-FolderStats $source
if ($statsNow.Files -ne [int]$prior.file_count -or $statsNow.Bytes -ne [long]$prior.bytes) {
    throw 'Local file count or size changed since Archive; refusing to delete. Re-archive first.'
}
Write-Host "Deleting ONLY verified local source: $source" -ForegroundColor Yellow
Remove-Item -LiteralPath $source -Recurse -Force -ErrorAction Stop
$prior.local_copy_removed = $true
$prior.local_removed_utc = [DateTime]::UtcNow.ToString('o')
[IO.File]::WriteAllText($receiptFile, (($prior | ConvertTo-Json -Depth 5) + "`n"), [Text.UTF8Encoding]::new($false))
Write-Host "Local copy removed; Drive copy verified. Reclaimed about $($statsNow.GiB) GiB." -ForegroundColor Green
Write-Host "Restore if needed: rclone copy '$destination' '$source'"
