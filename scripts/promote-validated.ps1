param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$source = Join-Path $root 'Publish-Staging-Validation'
$destination = Join-Path $root 'Publish'
$manifest = Get-Content -LiteralPath (Join-Path $source 'build-manifest.json') -Raw | ConvertFrom-Json
& (Join-Path $source 'runtime/node.exe') (Join-Path $root 'scripts/portable-package.mjs') --package $source
if ($LASTEXITCODE -ne 0) { throw 'Candidate package inventory verification failed; Publish was not changed.' }
foreach ($pair in @(@($manifest.executable,$manifest.sha256),@($manifest.cliExecutable,$manifest.cliSha256))) {
    if ((Get-FileHash -LiteralPath (Join-Path $source $pair[0]) -Algorithm SHA256).Hash -ne $pair[1]) { throw 'Package hash mismatch' }
}
$running = Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith("$destination\", [StringComparison]::OrdinalIgnoreCase) }
if ($running) { throw 'Exit the current Publish program before promotion.' }
$backupRoot = Join-Path $root 'ReleaseBackups'
$backup = [IO.Path]::GetFullPath((Join-Path $backupRoot ('Publish-' + [DateTime]::Now.ToString('yyyyMMdd-HHmmss'))))
if ([IO.Path]::GetFullPath($destination) -ne "$root\Publish" -or -not $backup.StartsWith("$root\ReleaseBackups\",[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid deployment boundary' }
New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
$incoming = [IO.Path]::GetFullPath((Join-Path $root ('Publish-Incoming-' + [Guid]::NewGuid().ToString('N'))))
if (-not $incoming.StartsWith("$root\Publish-Incoming-",[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid incoming boundary' }
Copy-Item -LiteralPath $source -Destination $incoming -Recurse
foreach ($pair in @(@($manifest.executable,$manifest.sha256),@($manifest.cliExecutable,$manifest.cliSha256))) {
    if ((Get-FileHash -LiteralPath (Join-Path $incoming $pair[0]) -Algorithm SHA256).Hash -ne $pair[1]) { throw 'Incoming package hash mismatch; Publish was not changed.' }
}
if (Test-Path -LiteralPath $destination) {
    if ((Get-Item -LiteralPath $destination).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Publish must not be a link.' }
    Move-Item -LiteralPath $destination -Destination $backup
}
try { Move-Item -LiteralPath $incoming -Destination $destination }
catch {
    if (-not (Test-Path -LiteralPath $destination) -and (Test-Path -LiteralPath $backup)) { Move-Item -LiteralPath $backup -Destination $destination }
    throw "Promotion failed; previous package preserved. $($_.Exception.Message)"
}
& (Join-Path $destination 'runtime/node.exe') (Join-Path $root 'scripts/portable-package.mjs') --package $destination
if ($LASTEXITCODE -ne 0) { throw 'Promoted package inventory verification failed.' }
foreach ($packageCopy in @($backup,$source)) {
    if (!(Test-Path -LiteralPath $packageCopy)) { continue }
    $resolved = (Resolve-Path -LiteralPath $packageCopy).Path
    if ($resolved -ne $packageCopy -or ($resolved -ne "$root\Publish-Staging-Validation" -and (Split-Path -Parent $resolved) -ne "$root\ReleaseBackups")) { throw 'Unexpected cleanup path' }
    if (((Get-Item -Force -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint) -or (Get-ChildItem -LiteralPath $resolved -Force -Recurse -Attributes ReparsePoint | Select-Object -First 1)) { throw 'Unexpected link in package' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
[pscustomobject]@{ Release=$manifest.releaseLabel; Version=$manifest.version; Package=$destination } | ConvertTo-Json
