param([Parameter(Mandatory=$true)][string]$Root)
$ErrorActionPreference='Stop'
$pilotRoot=(Resolve-Path -LiteralPath $Root).Path
$marker=Get-Content -LiteralPath (Join-Path $pilotRoot 'pilot-root.json') -Raw | ConvertFrom-Json
if($marker.schema -ne 'abya.workflow-pilot-root/v1'){throw 'Not an owned pilot root'}
$repository=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$package=Join-Path $repository 'Publish-Staging-Workflow'
$executable=Join-Path $package 'ABYA Desktop Development Tool.exe'
& (Join-Path $package 'runtime/node.exe') (Join-Path $repository 'scripts/portable-package.mjs') --package $package
if($LASTEXITCODE -ne 0){throw 'Candidate inventory failed'}
$manifest=Get-Content -LiteralPath (Join-Path $package 'build-manifest.json') -Raw | ConvertFrom-Json
if($manifest.workflowVersion -ne '2.11.0' -or $manifest.releaseLabel -ne 'delivery-first-20261030'){throw 'Wrong candidate'}
if(Get-NetTCPConnection -State Listen -LocalPort $marker.debugPort -ErrorAction SilentlyContinue){throw 'Pilot debug port is already in use'}
$env:ABYA_TEST_MODE='1'
$env:LOCALAPPDATA=Join-Path $pilotRoot 'Local'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $pilotRoot 'WebView'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$($marker.debugPort)"
$process=Start-Process -FilePath $executable -WorkingDirectory $package -WindowStyle Hidden -PassThru
@{processId=$process.Id;root=$pilotRoot;appSha256=(Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant();startedAt=[DateTime]::UtcNow.ToString('O')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $pilotRoot 'launch.json') -Encoding utf8
Write-Output "Isolated candidate started: PID $($process.Id)"
