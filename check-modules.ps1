param([int]$ProcId)
Get-Process -Id $ProcId |
    Select-Object -ExpandProperty Modules |
    Where-Object { $_.ModuleName -like 'sekiro_coop*' -or $_.ModuleName -like 'me3*' -or $_.ModuleName -like 'MinHook*' } |
    Format-Table ModuleName, @{L='Base';E={"0x{0:X}" -f $_.BaseAddress.ToInt64()}}, FileName -AutoSize
