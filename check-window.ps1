param([int]$ProcId)
$p = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
if ($null -eq $p) { Write-Host "process not found"; exit 1 }
"PID:         $($p.Id)"
"Name:        $($p.ProcessName)"
"Main window: '$($p.MainWindowTitle)' handle=0x$('{0:X}' -f $p.MainWindowHandle.ToInt64())"
"Working set: $([math]::Round($p.WorkingSet64 / 1MB)) MB"
"CPU time:    $($p.TotalProcessorTime)"
"Responding:  $($p.Responding)"
