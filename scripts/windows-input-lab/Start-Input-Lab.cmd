@echo off
setlocal
chcp 65001 >nul
if not exist "%~dp0reports" mkdir "%~dp0reports"
set "LAB_LAUNCH_LOG=%~dp0reports\launcher-%RANDOM%-%RANDOM%.log"
echo Starting isolated input lab.>"%LAB_LAUNCH_LOG%"
"%~dp0vao-input-lab.exe" --interactive "%~dp0reports" 2>>"%LAB_LAUNCH_LOG%"
set "LAB_EXIT_CODE=%ERRORLEVEL%"
echo LAB_EXIT_CODE=%LAB_EXIT_CODE%>>"%LAB_LAUNCH_LOG%"
echo.
echo 验证器已结束。退出码：%LAB_EXIT_CODE%
if not "%LAB_EXIT_CODE%"=="0" type "%LAB_LAUNCH_LOG%"
echo 诊断已保存在本目录 reports 文件夹中。
pause
endlocal
