param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Athanor\bin"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ManifestPath = Join-Path $ScriptDir "SHA256SUMS"
$ExpectedFiles = @("ath.exe", "athd.exe")

if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
    throw "Missing checksum manifest: $ManifestPath"
}

$ExpectedHashes = @{}
foreach ($Line in Get-Content -LiteralPath $ManifestPath) {
    if ([string]::IsNullOrWhiteSpace($Line)) {
        continue
    }
    if ($Line -notmatch '^([0-9A-Fa-f]{64})\s+\*?(.+)$') {
        throw "Invalid checksum manifest line: $Line"
    }

    $FileName = $Matches[2].Trim()
    if ($ExpectedFiles -notcontains $FileName) {
        throw "Unexpected file in checksum manifest: $FileName"
    }
    if ($ExpectedHashes.ContainsKey($FileName)) {
        throw "Duplicate checksum manifest entry: $FileName"
    }
    $ExpectedHashes[$FileName] = $Matches[1].ToLowerInvariant()
}

foreach ($FileName in $ExpectedFiles) {
    if (-not $ExpectedHashes.ContainsKey($FileName)) {
        throw "Missing checksum manifest entry: $FileName"
    }

    $SourcePath = Join-Path $ScriptDir $FileName
    if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) {
        throw "Missing packaged binary: $SourcePath"
    }

    $ActualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $SourcePath).Hash.ToLowerInvariant()
    if ($ActualHash -ne $ExpectedHashes[$FileName]) {
        throw "Checksum mismatch for $FileName"
    }
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
foreach ($FileName in $ExpectedFiles) {
    Copy-Item -Force -LiteralPath (Join-Path $ScriptDir $FileName) -Destination (Join-Path $InstallDir $FileName)
}

Write-Output "Verified and installed Athanor binaries to $InstallDir"
Write-Output "Add this directory to PATH, then register a project and run athd service install."
