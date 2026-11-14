param()
$ErrorActionPreference='Stop'
$repo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$scratch=Join-Path $repo ('logs/check-temp-'+[Guid]::NewGuid().ToString('N'))
$previousTemp=$env:TEMP
$previousTmp=$env:TMP
New-Item -ItemType Directory -Path $scratch -Force|Out-Null
try {
    $env:TEMP=$scratch
    $env:TMP=$scratch
    npm run check
    if($LASTEXITCODE -ne 0){throw 'Frontend or contract checks failed.'}
    cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
    if($LASTEXITCODE -ne 0){throw 'Rust formatting failed.'}
    cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
    if($LASTEXITCODE -ne 0){throw 'Rust Clippy failed.'}
    cargo test --manifest-path src-tauri/Cargo.toml
    if($LASTEXITCODE -ne 0){throw 'Rust tests failed.'}
} finally {
    $env:TEMP=$previousTemp
    $env:TMP=$previousTmp
    $resolved=(Resolve-Path -LiteralPath $scratch).Path
    if($resolved -ne $scratch -or (Split-Path -Parent $resolved) -ne "$repo\logs"){throw 'Unexpected test cleanup path.'}
    node (Join-Path $PSScriptRoot 'cleanup-test-temp.mjs') $resolved
    if($LASTEXITCODE -ne 0){throw "Test directory cleanup failed: $resolved"}
}
