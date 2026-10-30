@echo off
rem Builds XbloxRewind.exe (release) and copies it next to the content folders.
rem Runs the compiler at low priority with 2 jobs so the PC stays usable.
setlocal
cd /d "%~dp0launcher"

where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust is not installed. Get it from https://rustup.rs and run this again.
    pause
    exit /b 1
)

echo Building XbloxRewind.exe ...
start "" /b /low /wait cargo build --release
if errorlevel 1 (
    echo.
    echo Build failed.
    pause
    exit /b 1
)

copy /y "target\release\XbloxRewind.exe" "%~dp0XbloxRewind.exe" >nul
if errorlevel 1 (
    echo Built, but could not copy XbloxRewind.exe - is it still running?
    pause
    exit /b 1
)

echo.
echo Done: %~dp0XbloxRewind.exe
pause
