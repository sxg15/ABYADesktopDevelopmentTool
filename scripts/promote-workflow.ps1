param([Parameter(Mandatory=$true)][string]$PilotReport)
$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$candidate=[IO.Path]::GetFullPath((Join-Path $root 'Publish-Staging-Workflow'))
$destination=[IO.Path]::GetFullPath((Join-Path $root 'Publish'))
$executable=Join-Path $candidate 'ABYA Desktop Development Tool.exe'
$node=Join-Path $candidate 'runtime/node.exe'
Push-Location $root
try {
 & $node (Join-Path $root 'scripts/portable-package.mjs') --package $candidate
 if($LASTEXITCODE -ne 0){throw 'Candidate package verification failed.'}
 & $node (Join-Path $root 'scripts/verify-workflow-pilot.mjs') $PilotReport $executable
 if($LASTEXITCODE -ne 0){throw 'Native gameplay pilots have not qualified this exact package.'}
 $manifest=Get-Content -LiteralPath (Join-Path $candidate 'build-manifest.json') -Raw | ConvertFrom-Json
 $source=(& $node (Join-Path $root 'scripts/source-fingerprint.mjs')).Trim()
 if($source -ne $manifest.sourceFingerprint){throw 'Runtime sources changed after candidate build; rebuild and retest the affected candidate.'}
 $running=Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith("$destination\",[StringComparison]::OrdinalIgnoreCase)}
 if($running){throw 'The formal APP is running. Preserve its work and close it before promotion.'}
 $stamp=[Guid]::NewGuid().ToString('N')
 $next=[IO.Path]::GetFullPath((Join-Path $root ".publish-next-$stamp"))
 $backup=[IO.Path]::GetFullPath((Join-Path $root "logs/publish-backups/$stamp/Publish"))
 foreach($target in @($candidate,$destination,$next,$backup)){
  if(!$target.StartsWith("$root\",[StringComparison]::OrdinalIgnoreCase)){throw "Promotion path escapes workspace: $target"}
 }
 if((Test-Path -LiteralPath $next) -or (Test-Path -LiteralPath $backup)){throw 'Fresh promotion paths already exist.'}
 New-Item -ItemType Directory -Path $next | Out-Null
 $inventory=Get-Content -LiteralPath (Join-Path $candidate 'package-files.json') -Raw | ConvertFrom-Json
 foreach($entry in $inventory.files){
  $target=Join-Path $next $entry.path
  New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $candidate $entry.path) -Destination $target
 }
 Copy-Item -LiteralPath (Join-Path $candidate 'package-files.json') -Destination $next
 & $node (Join-Path $root 'scripts/portable-package.mjs') --package $next
 if($LASTEXITCODE -ne 0){throw 'Prepared publication did not match the validated package.'}
 $running=Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith("$destination\",[StringComparison]::OrdinalIgnoreCase)}
 if($running){throw 'The formal APP started during preparation; publication was not replaced.'}
 $moved=$false
 try {
  if(Test-Path -LiteralPath $destination){
   New-Item -ItemType Directory -Path (Split-Path -Parent $backup) -Force | Out-Null
   Move-Item -LiteralPath $destination -Destination $backup
   $moved=$true
  }
  Move-Item -LiteralPath $next -Destination $destination
 } catch {
  if($moved -and !(Test-Path -LiteralPath $destination)){Move-Item -LiteralPath $backup -Destination $destination}
  throw
 }
 Write-Output "Promoted the exact validated package. Previous package retained at $backup"
} finally { Pop-Location }
