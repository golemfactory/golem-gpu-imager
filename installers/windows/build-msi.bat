@echo off
REM Batch script to build MSI installer for Golem GPU Imager
REM Requirements: WiX Toolset v3 must be installed

setlocal enabledelayedexpansion

echo Building Golem GPU Imager MSI Installer...

REM Check if WiX is installed
where candle.exe >nul 2>&1
if errorlevel 1 (
    echo Error: WiX Toolset not found. Please install WiX Toolset v3 and ensure candle.exe is in your PATH.
    exit /b 1
)

REM Check if the executable exists
set "EXE_PATH=target\x86_64-pc-windows-gnu\release\golem-gpu-imager.exe"
if not exist "%EXE_PATH%" (
    echo Error: Executable not found at %EXE_PATH%
    echo Please build the project first with: cargo build --release --target x86_64-pc-windows-gnu
    exit /b 1
)

REM Create output directory
if not exist "dist" mkdir dist

REM Extract version from Cargo.toml
for /f "tokens=3 delims= """ %%a in ('findstr "version = " Cargo.toml') do set VERSION=%%a

echo Version: %VERSION%
echo Building installer...

REM Compile WiX source
echo Compiling WiX source...
candle.exe installer.wxs -out dist\installer.wixobj
if errorlevel 1 (
    echo Error: candle.exe failed
    exit /b 1
)

REM Link to create MSI
echo Linking MSI...
set "MSI_NAME=GolemGpuImager-%VERSION%-x64.msi"
light.exe dist\installer.wixobj -out "dist\%MSI_NAME%"
if errorlevel 1 (
    echo Error: light.exe failed
    exit /b 1
)

REM Clean up intermediate files
del dist\installer.wixobj

echo Successfully created: dist\%MSI_NAME%
echo MSI installer build completed successfully!

endlocal