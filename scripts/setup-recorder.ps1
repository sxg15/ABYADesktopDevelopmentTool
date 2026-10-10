$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$toolRoot = Join-Path (Split-Path $PSScriptRoot -Parent) 'tools/ffmpeg'
$manifest = Get-Content -LiteralPath (Join-Path $toolRoot 'source.json') -Raw | ConvertFrom-Json
$runtime = Join-Path $toolRoot 'runtime'
if (Test-Path -LiteralPath (Join-Path $runtime 'bin/ffmpeg.exe')) {
    Write-Output 'Recorder runtime is already installed.'
    exit 0
}
$staging = Join-Path $toolRoot ('staging-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging | Out-Null
$archive = Join-Path $staging 'ffmpeg.zip'
$cached = Get-ChildItem -LiteralPath $toolRoot -Directory -Filter 'staging-*' | ForEach-Object { Join-Path $_.FullName 'ffmpeg.zip' } | Where-Object { (Test-Path -LiteralPath $_) -and $_ -ne $archive -and (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant() -eq $manifest.sha256 } | Select-Object -First 1
if ($cached) { Copy-Item -LiteralPath $cached -Destination $archive } else { Invoke-WebRequest -Uri $manifest.url -OutFile $archive -TimeoutSec 180 }
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($hash -ne $manifest.sha256) { throw 'Recorder archive hash mismatch; existing runtime was not changed.' }
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    foreach ($entry in $zip.Entries) {
        if ($entry.FullName -match '(^/|^[A-Za-z]:|(^|[/\\])\.\.([/\\]|$))') { throw 'Unsafe archive entry.' }
    }
} finally { $zip.Dispose() }
$extracted = Join-Path $staging 'extracted'
Expand-Archive -LiteralPath $archive -DestinationPath $extracted
$package = @(Get-ChildItem -LiteralPath $extracted -Directory)
if ($package.Count -ne 1 -or -not (Test-Path -LiteralPath (Join-Path $package[0].FullName 'bin/ffmpeg.exe'))) { throw 'Unexpected recorder package.' }
if (Test-Path -LiteralPath $runtime) { throw 'Runtime directory already exists; preserved for inspection.' }
$source = [IO.Path]::GetFullPath($package[0].FullName)
$destination = [IO.Path]::GetFullPath($runtime)
$expectedRoot = [IO.Path]::GetFullPath($toolRoot) + [IO.Path]::DirectorySeparatorChar
if (-not $source.StartsWith($expectedRoot, [StringComparison]::OrdinalIgnoreCase) -or -not $destination.StartsWith($expectedRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Recorder paths left the tool directory.' }
Move-Item -LiteralPath $source -Destination $destination
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runtime 'abya-source.json') -Encoding utf8
Write-Output "Installed recorder $($manifest.version), preserving bundled licenses and documentation."
