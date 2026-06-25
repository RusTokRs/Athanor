param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Athanor\bin"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force -LiteralPath ".\ath.exe" -Destination (Join-Path $InstallDir "ath.exe")
Copy-Item -Force -LiteralPath ".\athd.exe" -Destination (Join-Path $InstallDir "athd.exe")
Write-Output "Installed Athanor binaries to $InstallDir"
Write-Output "Add this directory to PATH, then register a project and run athd service install."
