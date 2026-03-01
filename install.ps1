param(
    [string]$InstallDir = "$env:LOCALAPPDATA\hakai",
    [switch]$AddToPath  = $true
)

Write-Host "💀 Installing hakai..." -ForegroundColor Red

# 1. Create install directory
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

# 2. Build from source
Write-Host "Building Rust core..." -ForegroundColor Yellow
cargo build --release

Write-Host "Building Bun TUI..." -ForegroundColor Yellow
Push-Location packages\hakai-tui
bun build --compile --target=bun src/index.ts --outfile="$InstallDir\hakai-tui.exe"
Pop-Location

# 3. Copy Rust binary
Copy-Item "target\release\hakai.exe" "$InstallDir\hakai.exe" -Force

# 4. Add to PATH if requested
if ($AddToPath) {
    $currentPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable('PATH', "$currentPath;$InstallDir", 'User')
        Write-Host "Added $InstallDir to PATH" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "💀 hakai installed to $InstallDir" -ForegroundColor Green
Write-Host "   Run: hakai" -ForegroundColor Cyan
Write-Host "   Run: hakai --help for options" -ForegroundColor Cyan
