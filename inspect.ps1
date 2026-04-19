# Run the live-inspector against a running sekiro.exe.
#
# Usage:
#   .\inspect.ps1                # text report
#   .\inspect.ps1 -Json out.json # write JSON alongside

[CmdletBinding()]
param(
    [int]$Pid,
    [string]$Json = "",
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $root

$bin = Join-Path $root "target\release\live-inspector.exe"
if (-not (Test-Path $bin)) {
    Write-Host "building live-inspector ..." -ForegroundColor Yellow
    & cargo build -p live-inspector --release
    if ($LASTEXITCODE -ne 0) { throw "build failed" }
}

$args = @()
if ($Pid -ne 0) { $args += @("--pid", $Pid) }
if ($Json -ne "") { $args += @("--json", $Json) }
if ($Verbose) { $args += @("-v") }

& $bin @args
