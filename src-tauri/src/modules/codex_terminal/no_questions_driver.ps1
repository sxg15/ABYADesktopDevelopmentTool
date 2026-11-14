param([Parameter(Mandatory)][string]$Payload)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$inputData=Get-Content -Raw -Encoding utf8 -LiteralPath $Payload|ConvertFrom-Json
$deep=[Windows.Automation.TreeScope]::Descendants
function NameIs([string]$n){[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$n)}
$window=$null
for($attempt=0;$attempt -lt 30;$attempt++){
 foreach($p in @(Get-Process -Name ChatGPT,Codex -ErrorAction SilentlyContinue|Where-Object {$_.MainWindowHandle -ne 0})){
  $w=[Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
  $headers=@($w.FindAll($deep,(NameIs $inputData.heading))|Where-Object {!$_.Current.IsOffscreen -and $_.Current.ControlType.ProgrammaticName -eq "ControlType.Text" -and $_.Current.BoundingRectangle.Y -lt $w.Current.BoundingRectangle.Y+170})
  if($headers.Count -eq 1){$window=$w;break}
 }
 if($window){break};Start-Sleep -Milliseconds 200
}
if(!$window){throw '无法确认当前 Codex 窗口属于本任务。请打开任务对话后重试。'}
$buttons=$window.FindAll($deep,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Button))
if($inputData.action -eq 'stop'){
 $stop=@($buttons|Where-Object {!$_.Current.IsOffscreen -and $_.Current.IsEnabled -and $_.Current.Name -in @('停止','停止生成','停止响应','Stop','Stop generating')})
 if($stop.Count -eq 1){$stop[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()}
 'stopped';exit 0
}
$edits=$window.FindAll($deep,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Edit))
$composer=@($edits|Where-Object {!$_.Current.IsOffscreen -and $_.Current.IsEnabled -and $_.Current.ClassName -match 'ProseMirror'})
if($composer.Count -ne 1){throw '任务对话输入框尚未就绪。'}
$value=$composer[0].GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
if($value.Current.Value.Trim() -and $value.Current.Value.Trim() -ne $composer[0].Current.Name -and $value.Current.Value.Trim() -ne $inputData.prompt.Trim()){throw 'Codex 输入框已有草稿，待该草稿处理后再自动继续。'}
$send=@($buttons|Where-Object {!$_.Current.IsOffscreen -and $_.Current.IsEnabled -and $_.Current.Name -in @('发送','Send')})
# Enablement follows input. The button can be disabled until SetValue completes.
$composer[0].SetFocus()
$value.SetValue($inputData.prompt)
Start-Sleep -Milliseconds 200
if($value.Current.Value.Trim() -cne $inputData.prompt.Trim()){throw '未能核对任务指令，未发送。'}
$send=@($window.FindAll($deep,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Button))|Where-Object {!$_.Current.IsOffscreen -and $_.Current.IsEnabled -and $_.Current.Name -in @('发送','Send')})
if($send.Count -ne 1){throw '当前对话正在执行或输入尚未就绪。'}
$send[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
'sent'
