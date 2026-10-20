@echo off
setlocal
chcp 65001 >nul
"%~dp0vao-uia-capability.exe" --interactive "%~dp0reports"
echo.
echo 接口检查已结束，退出码：%ERRORLEVEL%
pause
endlocal
