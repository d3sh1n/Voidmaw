# PowerShell build script for Voidmaw Rust project

param(
    [Parameter(Mandatory=$false)]
    [ValidateSet("debug", "release")]
    [string]$Config = "release",

    [Parameter(Mandatory=$false)]
    [ValidateSet("x86_64", "i686", "both")]
    [string]$Arch = "x86_64"
)

Write-Host "Building Voidmaw (Rust 1.80 Version)" -ForegroundColor Cyan
Write-Host "Configuration: $Config" -ForegroundColor Yellow
Write-Host "Architecture: $Arch" -ForegroundColor Yellow
Write-Host ""

$ErrorActionPreference = "Stop"

# Function to build for a specific target
function Build-Target {
    param(
        [string]$Target,
        [string]$Config
    )

    Write-Host "Building for $Target..." -ForegroundColor Green

    # Add target if not already installed
    rustup target add $Target 2>$null

    if ($Config -eq "release") {
        cargo build --release --target $Target
    } else {
        cargo build --target $Target
    }

    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed for $Target" -ForegroundColor Red
        exit 1
    }

    Write-Host "Build successful for $Target" -ForegroundColor Green
    Write-Host ""
}

# Build based on architecture parameter
switch ($Arch) {
    "x86_64" {
        Build-Target -Target "x86_64-pc-windows-msvc" -Config $Config
    }
    "i686" {
        Build-Target -Target "i686-pc-windows-msvc" -Config $Config
    }
    "both" {
        Build-Target -Target "x86_64-pc-windows-msvc" -Config $Config
        Build-Target -Target "i686-pc-windows-msvc" -Config $Config
    }
}

Write-Host "All builds completed successfully!" -ForegroundColor Cyan
Write-Host ""
Write-Host "Binaries location:" -ForegroundColor Yellow

if ($Config -eq "release") {
    if ($Arch -eq "x86_64" -or $Arch -eq "both") {
        Write-Host "  x64: target\x86_64-pc-windows-msvc\release\dismantle.exe" -ForegroundColor Gray
        Write-Host "       target\x86_64-pc-windows-msvc\release\voidmaw.exe" -ForegroundColor Gray
    }
    if ($Arch -eq "i686" -or $Arch -eq "both") {
        Write-Host "  x86: target\i686-pc-windows-msvc\release\dismantle.exe" -ForegroundColor Gray
        Write-Host "       target\i686-pc-windows-msvc\release\voidmaw.exe" -ForegroundColor Gray
    }
} else {
    if ($Arch -eq "x86_64" -or $Arch -eq "both") {
        Write-Host "  x64: target\x86_64-pc-windows-msvc\debug\dismantle.exe" -ForegroundColor Gray
        Write-Host "       target\x86_64-pc-windows-msvc\debug\voidmaw.exe" -ForegroundColor Gray
    }
    if ($Arch -eq "i686" -or $Arch -eq "both") {
        Write-Host "  x86: target\i686-pc-windows-msvc\debug\dismantle.exe" -ForegroundColor Gray
        Write-Host "       target\i686-pc-windows-msvc\debug\voidmaw.exe" -ForegroundColor Gray
    }
}
