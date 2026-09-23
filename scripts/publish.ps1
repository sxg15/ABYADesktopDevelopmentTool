$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$publish = Join-Path $root "Publish"
$llmDirectories = @(".codex", ".grok")
$managedSkillRelativePath = "skills\abya-game-development-task\SKILL.md"
$managedSkillRequiredFiles = @(
    $managedSkillRelativePath,
    "skills\abya-game-development-task\references\gameplay-architecture.md",
    "skills\abya-game-development-task\assets\gameplay-architecture.v1.template.json",
    "skills\abya-import-task-template\SKILL.md",
    "skills\abya-import-task-template\scripts\import-template.mjs"
)
$target = Join-Path $root "src-tauri\target\release\abya-desktop-development-tool.exe"

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

npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Test-Path -LiteralPath $target)) {
    throw "Release executable was not produced at $target"
}

$destination = Join-Path $publish "ABYA Desktop Development Tool.exe"
Copy-Item -LiteralPath $target -Destination $destination -Force

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
$manifest = [ordered]@{
    product = "ABYA Desktop Development Tool"
    version = "0.1.0"
    builtAtUtc = [DateTime]::UtcNow.ToString("O")
    executable = [IO.Path]::GetFileName($destination)
    sha256 = $hash
}
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $publish "build-manifest.json") -Encoding utf8

Get-ChildItem -LiteralPath $publish | Select-Object Name,Length,LastWriteTime
