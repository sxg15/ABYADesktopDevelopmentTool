param([switch]$VerifyOnly)
$ErrorActionPreference = 'Stop'
[AppContext]::SetSwitch('Switch.System.IO.UseLegacyPathHandling', $false)
[AppContext]::SetSwitch('Switch.System.IO.BlockLongPaths', $false)
function Get-McpHash([string]$file) {
    $stream = [IO.File]::OpenRead($file)
    $hash = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant() }
    finally { $hash.Dispose(); $stream.Dispose() }
}
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$package = Join-Path $root 'Publish-MCP'
$longRoot = if ($package.StartsWith('\\')) { '\\?\UNC\' + $package.Substring(2) } else { '\\?\' + $package }
$linkFlag = [IO.FileAttributes]::ReparsePoint
if ([IO.File]::GetAttributes($longRoot) -band $linkFlag) { throw 'Package must not be a link' }
function Read-PackageJson([string]$name) {
    $file = [IO.Path]::Combine($longRoot, $name)
    if ([IO.File]::GetAttributes($file) -band $linkFlag) { throw 'Package links are forbidden' }
    return [IO.File]::ReadAllText($file,[Text.Encoding]::UTF8) | ConvertFrom-Json
}
$manifest = Read-PackageJson 'build-manifest.json'
if ($manifest.edition -ne 'mcp' -or $manifest.toolTransport -ne 'mcp' -or
    $manifest.executable -cne 'ABYA Desktop Development Tool MCP.exe') { throw 'Not an MCP release' }
$inventory = Read-PackageJson 'package-files.json'
$pending = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($entry in $inventory.files) {
    if (!$entry.path -or $entry.path -match '\\|:|(^|/)\.{1,2}(/|$)|^/|//') { throw 'Invalid inventory path' }
    if ($entry.path -notin @($manifest.executable,'build-manifest.json','.codex/README.md') -and
        $entry.path -notmatch '^(\.codex/skills/|tools/ffmpeg/)') { throw 'Unexpected runtime file' }
    $pending.Add($entry.path,$entry.sha256)
}
foreach ($required in @($manifest.executable,'build-manifest.json','.codex/skills/abya-game-development-task/SKILL.md','tools/ffmpeg/runtime/bin/ffmpeg.exe','tools/ffmpeg/runtime/bin/ffprobe.exe','tools/ffmpeg/runtime/LICENSE.txt')) {
    if (!$pending.ContainsKey($required)) { throw "Missing runtime file: $required" }
}
if ($pending[$manifest.executable] -ne $manifest.sha256) { throw 'Manifest executable mismatch' }
function Test-PackageDirectory([string]$directory) {
    foreach ($file in [IO.Directory]::EnumerateFileSystemEntries($directory)) {
        $attributes = [IO.File]::GetAttributes($file)
        if ($attributes -band $linkFlag) { throw 'Package links are forbidden' }
        $relative = $file.Substring($longRoot.Length+1).Replace('\','/')
        if ($attributes -band [IO.FileAttributes]::Directory) {
            if ($relative -ne 'ABYA Desktop Development Tool MCP.exe.WebView2') { Test-PackageDirectory $file }
        } elseif ($relative -ne 'package-files.json') {
            if (!$pending.ContainsKey($relative)) { throw "Unlisted runtime file: $relative" }
            if ((Get-McpHash $file) -ne $pending[$relative]) { throw "Package mismatch: $relative" }
            [void]$pending.Remove($relative)
        }
    }
}
Test-PackageDirectory $longRoot
if ($pending.Count) { throw 'One or more listed runtime files are missing' }
if ($VerifyOnly) { Write-Output 'MCP package verified'; exit 0 }
$env:ABYA_TEST_MODE = $null
Start-Process -FilePath (Join-Path $package $manifest.executable) -WorkingDirectory $package -WindowStyle Hidden
