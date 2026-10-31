@echo off
setlocal DisableDelayedExpansion
set "ABYA_PACKAGE=%~dp0Publish"
powershell -NoProfile -Command "$ErrorActionPreference='Stop'; $m=Get-Content -LiteralPath (Join-Path $env:ABYA_PACKAGE 'build-manifest.json') -Raw | ConvertFrom-Json; if($m.edition -ne 'mcp'){throw 'This is an inherited CLI package, not a qualified MCP release.'}"
if errorlevel 1 goto failed
if not exist "%ABYA_PACKAGE%\runtime\node.exe" goto missing
if /i "%~1"=="--check" goto verify
"%ABYA_PACKAGE%\runtime\node.exe" "%~dp0scripts\portable-package.mjs" --package "%ABYA_PACKAGE%" --quick
if errorlevel 1 goto failed
set "ABYA_TEST_MODE="
start "" "%SystemRoot%\explorer.exe" "%ABYA_PACKAGE%\ABYA Desktop Development Tool.exe"
exit /b 0
:verify
"%ABYA_PACKAGE%\runtime\node.exe" "%~dp0scripts\portable-package.mjs" --package "%ABYA_PACKAGE%"
exit /b %errorlevel%
:missing
echo The portable runtime is missing. Pull the complete repository and retry.
:failed
echo Please see README.md for setup instructions. No application was launched.
pause
exit /b 1
