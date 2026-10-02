param([switch]$Staging, [switch]$WorkflowStaging)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ($Staging -and $WorkflowStaging) { throw 'Choose only one staging destination.' }
$publishName = if ($WorkflowStaging) { "Publish-Staging-Workflow" } elseif ($Staging) { "Publish-Staging-Bot" } else { "Publish" }
$publish = Join-Path $root $publishName
$llmDirectories = @(".codex", ".grok")
$managedSkillRelativePath = "skills\abya-game-development-task\SKILL.md"
$managedSkillRequiredFiles = @(
    $managedSkillRelativePath,
    "skills\abya-game-development-task\references\gameplay-architecture.md",
    "skills\abya-game-development-task\assets\gameplay-architecture.v1.template.json",
    "skills\abya-game-development-task\references\bot-development.md",
    "skills\abya-game-development-task\references\bot-artifacts.md",
    "skills\abya-game-development-task\assets\bot-integration\plan.schema.json",
    "skills\abya-game-development-task\assets\bot-integration\plan.template.json",
    "skills\abya-game-development-task\assets\bot-integration\validation.schema.json",
    "skills\abya-game-development-task\assets\bot-integration\validation.template.json",
    "skills\abya-game-development-task\scripts\validate-bot-artifacts.mjs",
    "skills\abya-game-development-task\scripts\bot-artifact-core.mjs",
    "skills\abya-game-development-task\scripts\bot-definition-checks.mjs",
    "skills\abya-game-development-task\scripts\bot-evidence-checks.mjs",
    "skills\abya-import-task-template\SKILL.md",
    "skills\abya-import-task-template\scripts\import-template.mjs"
)
$target = Join-Path $root "src-tauri\target\release\abya-desktop-development-tool.exe"
$recorder = Join-Path $root 'tools/ffmpeg'
if (-not (Test-Path -LiteralPath (Join-Path $recorder 'runtime/bin/ffmpeg.exe'))) { throw 'Run scripts/setup-recorder.ps1 before publishing.' }

if (-not (Test-Path -LiteralPath $publish)) {
    New-Item -ItemType Directory -Path $publish | Out-Null
}

foreach ($directoryName in $llmDirectories) {
    $llmSource = Join-Path $root $directoryName
    if (-not (Test-Path -LiteralPath $llmSource -PathType Container)) {
        throw "Project LLM content directory was not found at $llmSource"
    }
    foreach ($requiredFile in $managedSkillRequiredFiles) {
        $managedSkillFile = Join-Path $llmSource $requiredFile
        if (-not (Test-Path -LiteralPath $managedSkillFile -PathType Leaf)) {
            throw "Bundled task Skill resource was not found at $managedSkillFile"
        }
    }
}

npm run check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --manifest-path src-tauri/Cargo.toml --release --bin abya-desktop
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Test-Path -LiteralPath $target)) {
    throw "Release executable was not produced at $target"
}

$destination = Join-Path $publish "ABYA Desktop Development Tool.exe"
Copy-Item -LiteralPath $target -Destination $destination -Force
Copy-Item -LiteralPath (Join-Path $root "src-tauri/target/release/abya-desktop.exe") -Destination $publish -Force
$runtime = Join-Path $publish "runtime"
New-Item -ItemType Directory -Path $runtime -Force | Out-Null
$nodePath = (Get-Command node.exe -ErrorAction Stop).Source
$nodeVersion = & $nodePath --version
if ([int]($nodeVersion.TrimStart('v').Split('.')[0]) -lt 20) { throw "Node 20+ is required" }
Copy-Item -LiteralPath $nodePath -Destination (Join-Path $runtime "node.exe") -Force
$toolsPath = Join-Path $publish "tools"
New-Item -ItemType Directory -Path $toolsPath -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $root "tools/abya") -Destination $toolsPath -Recurse -Force
$recorderDestination = Join-Path $toolsPath 'ffmpeg'
New-Item -ItemType Directory -Path $recorderDestination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $recorder 'source.json') -Destination $recorderDestination -Force
Copy-Item -LiteralPath (Join-Path $recorder 'runtime') -Destination $recorderDestination -Recurse -Force

foreach ($directoryName in $llmDirectories) {
    $llmSource = Join-Path $root $directoryName
    $llmDestination = Join-Path $publish $directoryName
    if (Test-Path -LiteralPath $llmDestination) {
        $resolvedPublish = (Resolve-Path -LiteralPath $publish).Path
        $resolvedLlmDestination = (Resolve-Path -LiteralPath $llmDestination).Path
        if (-not $resolvedLlmDestination.StartsWith("$resolvedPublish\", [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove LLM content outside the publish directory: $resolvedLlmDestination"
        }
        Remove-Item -LiteralPath $llmDestination -Recurse -Force
    }
    New-Item -ItemType Directory -Path $llmDestination | Out-Null
    Get-ChildItem -LiteralPath $llmSource -Force | Copy-Item -Destination $llmDestination -Recurse -Force
}

$stream = [IO.File]::OpenRead($destination)
try {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace("-", "")
    }
    finally {
        $algorithm.Dispose()
    }
}
finally {
    $stream.Dispose()
}
$cliStream = [IO.File]::OpenRead((Join-Path $publish "abya-desktop.exe"))
try {
    $cliAlgorithm = [Security.Cryptography.SHA256]::Create()
    try { $cliHash = [BitConverter]::ToString($cliAlgorithm.ComputeHash($cliStream)).Replace("-", "") }
    finally { $cliAlgorithm.Dispose() }
} finally { $cliStream.Dispose() }
$manifest = [ordered]@{
    product = "ABYA Desktop Development Tool"
    version = "0.1.0"
    builtAtUtc = [DateTime]::UtcNow.ToString("O")
    executable = [IO.Path]::GetFileName($destination)
    sha256 = $hash
    cliExecutable = "abya-desktop.exe"
    cliSha256 = $cliHash
    runtimeCliVersion = "0.2.0"
    cliProtocolVersion = 1
    nodeVersion = $nodeVersion
}
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $publish "build-manifest.json") -Encoding utf8

Get-ChildItem -LiteralPath $publish | Select-Object Name,Length,LastWriteTime
