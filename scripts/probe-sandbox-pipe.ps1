$ErrorActionPreference = 'Stop'
$name = 'abya-probe-' + [guid]::NewGuid().ToString('N')
$security = [System.IO.Pipes.PipeSecurity]::new()
$owner = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$group = [System.Security.Principal.NTAccount]::new($env:COMPUTERNAME, 'CodexSandboxOffline').Translate([System.Security.Principal.SecurityIdentifier])
foreach ($sid in @($owner, $group)) {
    $security.AddAccessRule([System.IO.Pipes.PipeAccessRule]::new($sid, [System.IO.Pipes.PipeAccessRights]::ReadWrite, [System.Security.AccessControl.AccessControlType]::Allow))
}
$server = [System.IO.Pipes.NamedPipeServerStreamAcl]::Create($name, [System.IO.Pipes.PipeDirection]::InOut, 1, [System.IO.Pipes.PipeTransmissionMode]::Byte, [System.IO.Pipes.PipeOptions]::Asynchronous, 4096, 4096, $security, [System.IO.HandleInheritability]::None, [System.IO.Pipes.PipeAccessRights]0)
$wait = $server.WaitForConnectionAsync()
$clientScript = '$p=[System.IO.Pipes.NamedPipeClientStream]::new(".","' + $name + '",[System.IO.Pipes.PipeDirection]::InOut);$p.Connect(5000);$p.WriteByte(42);$p.Dispose();whoami'
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($clientScript))
$codex = 'C:\Users\Indiegamespass\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe'
try {
    Push-Location C:\
    & $codex sandbox 'C:\Program Files\PowerShell\7\pwsh.exe' -NoProfile -EncodedCommand $encoded
    if ($wait.Wait(1000)) { 'PipeByte=' + $server.ReadByte() } else { 'PipeConnection=FAILED' }
} finally { Pop-Location; $server.Dispose() }



