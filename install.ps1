# Aether Installation Script for Windows

param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:USERPROFILE\.aether\bin"
)

Write-Host "🛡️  Installing Aether..." -ForegroundColor Green

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }

# Get latest version
if ($Version -eq "latest") {
    $LatestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/WyattAu/aether-core/releases/latest"
    $Version = $LatestRelease.tag_name
}

Write-Host "   Version: $Version"
Write-Host "   Install: $InstallDir"

# Create install directory
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# Download
$Url = "https://github.com/WyattAu/aether-core/releases/download/$Version/aether-$($Arch)-pc-windows-msvc.zip"
$ZipPath = "$env:TEMP\aether.zip"

Write-Host "   Downloading..."
Invoke-WebRequest -Uri $Url -OutFile $ZipPath

# Extract
Write-Host "   Extracting..."
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force

# Add to PATH
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($CurrentPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$CurrentPath;$InstallDir", "User")
}

# Cleanup
Remove-Item $ZipPath

Write-Host ""
Write-Host "✅ Aether installed!" -ForegroundColor Green
Write-Host ""
Write-Host "Quick start:"
Write-Host "  aether dev     # Start development environment"
Write-Host "  aether deploy  # Deploy to cluster"
