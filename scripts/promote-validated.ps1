param()
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$source = Join-Path $root 'Publish-Staging-Validation'
$destination = Join-Path $root 'Publish'
$manifest = Get-Content -LiteralPath (Join-Path $source 'build-manifest.json') -Raw | ConvertFrom-Json
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
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $root 'ABYA 开发工具.lnk'))
$shortcut.TargetPath = Join-Path $env:WINDIR 'explorer.exe'
$shortcut.Arguments = '"' + (Join-Path $destination $manifest.executable) + '"'
$shortcut.WorkingDirectory = $destination
$shortcut.IconLocation = Join-Path $destination $manifest.executable
$shortcut.Description = 'ABYA 正式入口；沿用当前 Windows 用户的正式任务数据'
$shortcut.Save()
[pscustomobject]@{ Release=$manifest.releaseLabel; Version=$manifest.version; Entry=(Join-Path $root 'ABYA 开发工具.lnk'); PreviousPackage=$backup } | ConvertTo-Json
