param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Apply')][string]$Phase,
    [Parameter(Mandatory)][string]$Root,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$Commit,
    [Parameter(Mandatory)][long]$Number,
    [Parameter(Mandatory)][int]$AppPid
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$env:GIT_TERMINAL_PROMPT = '0'
$env:GCM_INTERACTIVE = 'never'
$Root = (Resolve-Path -LiteralPath $Root).Path
$package = Join-Path $Root 'Publish'
$app = Join-Path $package 'ABYA Desktop Development Tool.exe'
$stateFile = Join-Path $PSScriptRoot 'prepared.json'
$gitOptions = @('-C',$Root,'-c',('core.hooksPath='+(Join-Path $PSScriptRoot 'no-hooks')))
function Find-Git {
    $command = Get-Command git.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) { return $command.Source }
    foreach ($base in @($env:ProgramFiles,$env:LOCALAPPDATA)) {
        $file = Join-Path $base 'Git/cmd/git.exe'
        if (Test-Path -LiteralPath $file -PathType Leaf) { return $file }
    }
    $desktop = Join-Path $env:LOCALAPPDATA 'GitHubDesktop'
    if (Test-Path -LiteralPath $desktop) {
        foreach ($dir in (Get-ChildItem -LiteralPath $desktop -Directory -Filter 'app-*' | Sort-Object LastWriteTime -Descending)) {
            $file = Join-Path $dir.FullName 'resources/app/git/cmd/git.exe'
            if (Test-Path -LiteralPath $file -PathType Leaf) { return $file }
        }
    }
    throw '未找到 Git，请安装 Git for Windows 或 GitHub Desktop 后重试。'
}
function Read-Git([string[]]$Arguments) {
    $ErrorActionPreference='Continue'
    $result = & $git @gitOptions @Arguments 2>$null
    if ($LASTEXITCODE -ne 0) { throw ('Git 操作未完成：'+$Arguments[0]+'。请在 GitHub Desktop 中核对仓库状态。') }
    return ($result -join "`n").Trim()
}
function Check-Checkout {
    if ((Read-Git @('rev-parse','--show-toplevel')).Replace('/','\') -ne $Root) { throw '请从 Git 克隆目录内的 Publish 启动 App。' }
    $origin = Read-Git @('remote','get-url','origin')
    if ($origin -notmatch '^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)sxg15/ABYADesktopDevelopmentTool(?:\.git)?/?$') {throw '当前仓库的 origin 指向其他项目。'}
    $branch = Read-Git @('symbolic-ref','--short','HEAD')
    if ($branch -cnotin @('dev','CLI')) {throw '请在 GitHub Desktop 切换到发布分支 dev 后重试。'}
    if (Read-Git @('status','--porcelain','--untracked-files=no')) {throw '仓库有未提交修改，请先在 GitHub Desktop 中提交或暂存到 stash。'}
    foreach ($name in @('MERGE_HEAD','CHERRY_PICK_HEAD','REVERT_HEAD','rebase-apply','rebase-merge','index.lock','BISECT_START')) {
        $file=Read-Git @('rev-parse','--path-format=absolute','--git-path',$name)
        if(Test-Path -LiteralPath $file){throw '另一个 Git 操作正在进行，请完成后再更新。'}
    }
    return $branch
}
function Verify-Package([string]$Path) {
    & (Join-Path $package 'runtime/node.exe') (Join-Path $Root 'scripts/portable-package.mjs') --package $Path *> $null
    if ($LASTEXITCODE -ne 0) {throw '完整发布包校验失败。'}
}
function Start-App { Start-Process -FilePath $app -WorkingDirectory $package }
$merged = $false
try {
    $git=Find-Git
    if ($Phase -eq 'Prepare') {
        $branch=Check-Checkout
        $old=Read-Git @('rev-parse','HEAD')
        $local=Get-Content -Raw -LiteralPath (Join-Path $package 'build-manifest.json') -Encoding UTF8 | ConvertFrom-Json
        if($Number -le [long]$local.releaseNumber){throw '请重新检查更新。'}
        $url='https://github.com/sxg15/ABYADesktopDevelopmentTool.git'
        $fetchOptions=@('-c','http.lowSpeedLimit=128','-c','http.lowSpeedTime=45')
        $configuredProxy=& $git @gitOptions config --get-urlmatch http.proxy $url 2>$null
        if ($LASTEXITCODE -ne 0 -and !$env:HTTPS_PROXY -and !$env:ALL_PROXY -and !$env:NO_PROXY) {
            $route=[Net.WebRequest]::GetSystemWebProxy().GetProxy([uri]$url)
            if ($route -and $route.AbsoluteUri -ne $url) {$fetchOptions+=@('-c',('http.proxy='+$route.AbsoluteUri))}
        }
        $fetchArgs=$fetchOptions+@('fetch','--quiet','--no-tags','--no-recurse-submodules',$url,'refs/heads/dev:refs/remotes/origin/dev')
        try {Read-Git $fetchArgs | Out-Null} catch {
            try {Read-Git $fetchArgs | Out-Null} catch {throw '无法从 GitHub 拉取新版，请检查网络或代理后重试。'}
        }
        if ((Read-Git @('rev-parse','refs/remotes/origin/dev')) -cne $Commit) {throw '发布分支已更新，请重新检查更新。'}
        & $git @gitOptions merge-base --is-ancestor $old $Commit
        if($LASTEXITCODE -ne 0){throw '本地分支有独立提交，无法快进。请在 GitHub Desktop 中处理。'}
        $stage=New-Item -ItemType Directory -Path (Join-Path $PSScriptRoot 'staged')
        $alternateIndex=$env:GIT_INDEX_FILE
        try {
            $env:GIT_INDEX_FILE=Join-Path $PSScriptRoot 'package.index'
            Read-Git @('read-tree',$Commit) | Out-Null
            Read-Git @('-c','core.longpaths=true',('--work-tree='+$stage.FullName),'restore',('--source='+$Commit),'--worktree','--',':(top)Publish') | Out-Null
        } finally { $env:GIT_INDEX_FILE=$alternateIndex }
        $incoming=Join-Path $stage.FullName 'Publish'
        $manifest=Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $incoming 'build-manifest.json') | ConvertFrom-Json
        if($manifest.product -ne $local.product -or [long]$manifest.releaseNumber -ne $Number){throw '提交中的发布编号不符，请重新检查更新。'}
        Verify-Package $incoming
        @{old=$old;branch=$branch} | ConvertTo-Json | Set-Content -Encoding UTF8 -LiteralPath $stateFile
        exit 0
    }
    $state=Get-Content -Raw -Encoding UTF8 -LiteralPath $stateFile | ConvertFrom-Json
    $parent=Get-Process -Id $AppPid -ErrorAction SilentlyContinue
    if (!$parent -or [IO.Path]::GetFullPath($parent.Path) -ne $app) {throw '原 App 进程身份发生变化。'}
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot 'ready'),'ready')
    if(!$parent.WaitForExit(90000)){throw '原 App 仍在退出，本次更新已停止。'}
    $closed=$true
    if ((Check-Checkout) -cne $state.branch -or (Read-Git @('rev-parse','HEAD')) -cne $state.old) {
        throw '下载期间工作区发生变化，请重新检查更新。'
    }
    Read-Git @('merge','--ff-only','--no-overwrite-ignore','--no-edit',$Commit) | Out-Null
    $merged=$true
    if ((Read-Git @('rev-parse','HEAD')) -cne $Commit) {throw '实际版本发生变化，请核对仓库。'}
    Verify-Package $package
    Start-App
} catch {
    $message=$_.Exception.Message
    if($Phase -eq 'Prepare') {[Console]::Error.WriteLine($message);exit 1}
    if($closed) {
        try {
            if ($merged -and (Check-Checkout) -ceq $state.branch -and (Read-Git @('rev-parse','HEAD')) -ceq $Commit) {
                Read-Git @('reset','--keep',$state.old) | Out-Null
            }
            if((Read-Git @('rev-parse','HEAD')) -ceq $state.old) {Verify-Package $package; Start-App}
        } catch { $message += "`n"+$_.Exception.Message }
    }
    Add-Type -AssemblyName PresentationFramework
    [Windows.MessageBox]::Show($message,'ABYA 桌面工具更新') | Out-Null
    exit 1
} finally {
    if($Phase -eq 'Apply') {
        $dir=[IO.Path]::GetFullPath($PSScriptRoot)
        if((Split-Path $dir -Parent) -eq [IO.Path]::GetTempPath().TrimEnd('\') -and (Split-Path $dir -Leaf) -match '^abya-update-[0-9a-f-]{36}$'){
            Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}
