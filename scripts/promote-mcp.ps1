param([Parameter(Mandatory=$true)][string]$PilotReport)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $root
try {
 if((git branch --show-current).Trim() -ne 'MCP'){throw 'Promotion is allowed only on MCP'}
 $source=Join-Path $root 'Publish-Staging-MCP'
 $destination=Join-Path $root 'Publish-MCP'
 node scripts/portable-package.mjs --package $source
 if($LASTEXITCODE -ne 0){throw 'Candidate package failed verification'}
 $manifest=Get-Content -LiteralPath (Join-Path $source 'build-manifest.json') -Raw | ConvertFrom-Json
 if($manifest.sourceFingerprint -ne (node scripts/source-fingerprint.mjs).Trim()){throw 'Source changed after candidate build'}
 node scripts/verify-workflow-pilot.mjs $PilotReport (Join-Path $source $manifest.executable)
 if($LASTEXITCODE -ne 0){throw 'Native gameplay qualification has not passed'}
 if(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith("$destination\",[StringComparison]::OrdinalIgnoreCase)}){throw 'MCP release is running'}
 $backupRoot=Join-Path $root 'ReleaseBackups-MCP'
 $backup=Join-Path $backupRoot ('Publish-MCP-'+[DateTime]::Now.ToString('yyyyMMdd-HHmmss'))
 $incoming=Join-Path $root ('Publish-Incoming-MCP-'+[Guid]::NewGuid().ToString('N'))
 foreach($target in @($source,$destination,$backupRoot,$incoming)){
  $full=[IO.Path]::GetFullPath($target)
  if(!$full.StartsWith("$root\",[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid MCP release boundary'}
  if((Test-Path -LiteralPath $target) -and ((Get-Item -LiteralPath $target).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Release paths must not be links'}
 }
 Copy-Item -LiteralPath $source -Destination $incoming -Recurse
 node scripts/portable-package.mjs --package $incoming
 if($LASTEXITCODE -ne 0){throw 'Incoming package failed verification; existing release retained'}
 New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
 if(Test-Path -LiteralPath $destination){Move-Item -LiteralPath $destination -Destination $backup}
 try {Move-Item -LiteralPath $incoming -Destination $destination}
 catch {
  if(!(Test-Path -LiteralPath $destination) -and (Test-Path -LiteralPath $backup)){Move-Item -LiteralPath $backup -Destination $destination}
  throw
 }
 Write-Output "MCP release: $destination; rollback: $backup"
} finally {Pop-Location}
