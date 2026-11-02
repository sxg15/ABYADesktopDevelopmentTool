param([switch]$VerifyOnly)
$ErrorActionPreference='Stop'
function Get-McpHash([string]$file){
 $stream=[IO.File]::OpenRead($file);$hash=[Security.Cryptography.SHA256]::Create()
 try {return ([BitConverter]::ToString($hash.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}
 finally {$hash.Dispose();$stream.Dispose()}
}
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$package=Join-Path $root 'Publish-MCP'
if((Get-Item -LiteralPath $package).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package must not be a link'}
$manifest=Get-Content -LiteralPath (Join-Path $package 'build-manifest.json') -Raw | ConvertFrom-Json
if($manifest.edition -ne 'mcp' -or $manifest.toolTransport -ne 'mcp'){throw 'Not an MCP release'}
$inventory=Get-Content -LiteralPath (Join-Path $package 'package-files.json') -Raw | ConvertFrom-Json
$listed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach($entry in $inventory.files){
 if(!$entry.path -or $entry.path -match '\\|:|(^|/)\.{1,2}(/|$)|^/|//'){throw 'Invalid inventory path'}
 if(!$listed.Add($entry.path)){throw 'Duplicate inventory path'}
 $file=[IO.Path]::GetFullPath((Join-Path $package $entry.path))
 if(!$file.StartsWith("$package\",[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid inventory path'}
 if($entry.path -notin @('ABYA Desktop Development Tool MCP.exe','build-manifest.json','.codex/README.md') -and $entry.path -notmatch '^(\.codex/skills/|tools/ffmpeg/)'){throw 'Unexpected runtime file'}
 $ancestor=$file
 while($ancestor -ne $package){
  if((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package links are forbidden'}
  $ancestor=[IO.Path]::GetDirectoryName($ancestor)
 }
 if((Get-McpHash $file) -ne $entry.sha256){throw "Package mismatch: $($entry.path)"}
}
foreach($required in @('ABYA Desktop Development Tool MCP.exe','build-manifest.json','.codex/skills/abya-game-development-task/SKILL.md','tools/ffmpeg/runtime/bin/ffmpeg.exe','tools/ffmpeg/runtime/bin/ffprobe.exe','tools/ffmpeg/runtime/LICENSE.txt')){
 if(!$listed.Contains($required)){throw "Missing runtime file: $required"}
}
function Test-PackageDirectory([string]$directory){
 foreach($item in Get-ChildItem -LiteralPath $directory -Force){
  if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Package links are forbidden'}
  if($directory -eq $package -and $item.PSIsContainer -and $item.Name -eq 'ABYA Desktop Development Tool MCP.exe.WebView2'){continue}
  if($item.PSIsContainer){Test-PackageDirectory $item.FullName}
  else {
   $relative=$item.FullName.Substring($package.Length+1).Replace('\','/')
   if($relative -ne 'package-files.json' -and !$listed.Contains($relative)){throw "Unlisted runtime file: $relative"}
  }
 }
}
Test-PackageDirectory $package
if($manifest.executable -ne 'ABYA Desktop Development Tool MCP.exe' -or (Get-McpHash (Join-Path $package $manifest.executable)) -ne $manifest.sha256){throw 'Manifest executable mismatch'}
if($VerifyOnly){Write-Output 'MCP package verified';exit 0}
$env:ABYA_TEST_MODE=$null
Start-Process -FilePath (Join-Path $package 'ABYA Desktop Development Tool MCP.exe') -WorkingDirectory $package -WindowStyle Hidden
