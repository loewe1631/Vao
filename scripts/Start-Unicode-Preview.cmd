@echo off
setlocal
cd /d "%~dp0"
if not exist "%~dp0vao.exe" exit /b 2
start "" "%~dp0vao.exe" --windows-unicode-preview --windows-input-report "%~dp0input-preview.trace"
