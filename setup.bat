@echo off
setlocal
cd /d "%~dp0"
if not defined STRATOS_ENV set "STRATOS_ENV=base"
call conda env update -n "%STRATOS_ENV%" -f environment.yml
if errorlevel 1 (pause & exit /b 1)
echo Runtime installed. Install MiKTeX or TeX Live for plots; see README.md.
echo Double-click start-gui.bat to start.
pause
