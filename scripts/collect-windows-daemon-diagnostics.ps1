$ErrorActionPreference = "Stop"

$temp = $env:RUNNER_TEMP
if ([string]::IsNullOrWhiteSpace($temp)) {
  throw "RUNNER_TEMP is not set"
}

$diagnostics = Join-Path $temp "athanor-diagnostics"
$runtime = Join-Path $temp "athanor-runtime"
$registry = Join-Path $temp "projects.json"
New-Item -ItemType Directory -Force -Path $diagnostics | Out-Null

if (Test-Path -LiteralPath $registry) {
  Copy-Item -LiteralPath $registry -Destination (Join-Path $diagnostics "projects.json")
}
if (Test-Path -LiteralPath $runtime) {
  Copy-Item -Recurse -Force -LiteralPath $runtime -Destination (Join-Path $diagnostics "runtime")
  Get-ChildItem -Recurse -Force -LiteralPath $runtime |
    Select-Object FullName,Length,LastWriteTime |
    Format-Table -AutoSize |
    Out-File -LiteralPath (Join-Path $diagnostics "runtime-tree.txt") -Encoding utf8
}
Get-Process athd -ErrorAction SilentlyContinue |
  Select-Object Id,ProcessName,Path,StartTime |
  Format-Table -AutoSize |
  Out-File -LiteralPath (Join-Path $diagnostics "athd-processes.txt") -Encoding utf8
