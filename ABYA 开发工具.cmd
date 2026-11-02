@echo off
setlocal DisableDelayedExpansion
if /i "%~1"=="--check" (
 powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\launch-mcp.ps1" -VerifyOnly
) else (
 powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\launch-mcp.ps1"
)
exit /b %errorlevel%
