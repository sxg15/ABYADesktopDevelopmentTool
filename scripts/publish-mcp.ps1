param([switch]$SkipChecks)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $root
try {
 if((git branch --show-current).Trim() -ne 'MCP'){throw 'Build this edition only on MCP'}
 $publish=Join-Path $root 'Publish-Staging-MCP'
 if(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith("$publish\",[StringComparison]::OrdinalIgnoreCase)}){throw 'Candidate is running'}
 $env:CARGO_TARGET_DIR=Join-Path $root 'target-mcp'
 $env:ABYA_RELEASE_ID='mcp-native-20261031'
 $env:ABYA_BUILD_UTC=[DateTime]::UtcNow.ToString('O')
 if(!$SkipChecks){
  npm run check
  if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
  cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
  if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
  cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
  if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
  cargo test --manifest-path src-tauri/Cargo.toml
  if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
 }
 npm run tauri build -- --no-bundle
 if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
 if(Test-Path -LiteralPath $publish){
  $resolved=(Resolve-Path -LiteralPath $publish).Path
  if($resolved -ne (Join-Path $root 'Publish-Staging-MCP') -or ((Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Unsafe staging path'}
  Remove-Item -LiteralPath $resolved -Recurse -Force
 }
 New-Item -ItemType Directory -Path $publish | Out-Null
 $executable=Join-Path $publish 'ABYA Desktop Development Tool MCP.exe'
 Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/abya-desktop-development-tool.exe') -Destination $executable
 Copy-Item -LiteralPath (Join-Path $root '.codex') -Destination (Join-Path $publish '.codex') -Recurse
 New-Item -ItemType Directory -Path (Join-Path $publish 'tools/ffmpeg') -Force | Out-Null
 Copy-Item -LiteralPath (Join-Path $root 'tools/ffmpeg/source.json') -Destination (Join-Path $publish 'tools/ffmpeg/source.json')
 Copy-Item -LiteralPath (Join-Path $root 'tools/ffmpeg/runtime') -Destination (Join-Path $publish 'tools/ffmpeg/runtime') -Recurse
 $package=Get-Content package.json -Raw | ConvertFrom-Json
 $policy=Get-Content .codex/skills/abya-game-development-task/assets/production/workflow-policy.json -Raw | ConvertFrom-Json
 @{edition='mcp';toolTransport='mcp';product='ABYA Desktop Development Tool MCP';version=$package.version;workflowVersion=$policy.workflowVersion;releaseLabel=$env:ABYA_RELEASE_ID;sourceBuiltAtUtc=$env:ABYA_BUILD_UTC;builtAtUtc=[DateTime]::UtcNow.ToString('O');executable=[IO.Path]::GetFileName($executable);sha256=(Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant();sourceFingerprint=(node scripts/source-fingerprint.mjs).Trim();mcpProtocolVersion='2025-11-25'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $publish 'build-manifest.json') -Encoding UTF8
 node scripts/portable-package.mjs --package $publish --write
 if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
 Write-Output 'Built MCP candidate only. Native qualification is required before promotion.'
} finally {Pop-Location}
