@echo off
setlocal
cd /d "%~dp0"
if not defined STRATOS_ENV set "STRATOS_ENV=base"
where conda >nul 2>nul
if not errorlevel 1 goto launch
for %%D in ("%USERPROFILE%\miniconda3" "%USERPROFILE%\anaconda3" "%LOCALAPPDATA%\miniconda3" "%ProgramData%\miniconda3") do (
  if exist "%%~D\condabin\conda.bat" set "PATH=%%~D\condabin;%PATH%"
)
:launch
call conda run --no-capture-output -n "%STRATOS_ENV%" python scripts/launch.py %*
if errorlevel 1 pause
