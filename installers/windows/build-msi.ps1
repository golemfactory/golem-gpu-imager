# PowerShell script to build MSI installer for Golem GPU Imager
# Requirements: WiX Toolset v3 must be installed

param(
    [string]$Configuration = "release",
    [string]$OutputDir = "dist"
)

# Set error action preference
$ErrorActionPreference = "Stop"

Write-Host "Building Golem GPU Imager MSI Installer..." -ForegroundColor Green

# Check if WiX is installed
$wixPath = Get-Command "candle.exe" -ErrorAction SilentlyContinue
if (-not $wixPath) {
    Write-Error "WiX Toolset not found. Please install WiX Toolset v3 and ensure candle.exe is in your PATH."
    exit 1
}

# Check if the executable exists
$exePath = "target\x86_64-pc-windows-gnu\release\golem-gpu-imager.exe"
if (-not (Test-Path $exePath)) {
    Write-Error "Executable not found at $exePath. Please build the project first with 'cargo build --release --target x86_64-pc-windows-gnu'"
    exit 1
}

# Create output directory if it doesn't exist
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# Get version from Cargo.toml
$cargoToml = Get-Content "Cargo.toml" -Raw
$version = [regex]::Match($cargoToml, 'version = "([^"]+)"').Groups[1].Value
if (-not $version) {
    Write-Error "Could not extract version from Cargo.toml"
    exit 1
}

Write-Host "Version: $version" -ForegroundColor Cyan
Write-Host "Building installer..." -ForegroundColor Yellow

try {
    # Compile WiX source
    Write-Host "Compiling WiX source..." -ForegroundColor Yellow
    & candle.exe installers\windows\installer.wxs -out "$OutputDir\installer.wixobj"
    if ($LASTEXITCODE -ne 0) {
        throw "candle.exe failed with exit code $LASTEXITCODE"
    }

    # Link to create MSI
    Write-Host "Linking MSI..." -ForegroundColor Yellow
    $msiName = "GolemGpuImager-$version-x64.msi"
    & light.exe "$OutputDir\installer.wixobj" -out "$OutputDir\$msiName"
    if ($LASTEXITCODE -ne 0) {
        throw "light.exe failed with exit code $LASTEXITCODE"
    }

    Write-Host "Successfully created: $OutputDir\$msiName" -ForegroundColor Green
    
    # Display file info
    $msiFile = Get-Item "$OutputDir\$msiName"
    Write-Host "File size: $([math]::Round($msiFile.Length / 1MB, 2)) MB" -ForegroundColor Cyan
    Write-Host "Created: $($msiFile.CreationTime)" -ForegroundColor Cyan

} catch {
    Write-Error "Build failed: $_"
    exit 1
} finally {
    # Clean up intermediate files
    if (Test-Path "$OutputDir\installer.wixobj") {
        Remove-Item "$OutputDir\installer.wixobj" -Force
    }
}

Write-Host "MSI installer build completed successfully!" -ForegroundColor Green