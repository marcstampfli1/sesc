$p = Get-Process -Name sekiro -ErrorAction SilentlyContinue
if ($p) {
    $p | Stop-Process -Force -ErrorAction SilentlyContinue
    $p.WaitForExit(30000) | Out-Null
}
$remaining = (Get-Process -Name sekiro -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Host "sekiro processes remaining: $remaining"
