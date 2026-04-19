# Auto-enter a Sekiro save.
#
# After me3 launches Sekiro with skip_logos, the first visible screen
# is the main menu.  "Continue" is the default-selected entry, so a
# single Enter loads the most-recent save.  A confirmation dialog may
# appear; we keep pressing Enter until the player_position chain
# resolves (i.e. player is loaded in world).
#
# Usage:
#   .\auto-enter.ps1                   # auto-detect PID
#   .\auto-enter.ps1 -ProcId 1234      # specific PID

[CmdletBinding()]
param(
    [int]$ProcId = 0,
    # Sekiro sometimes pops extra dialogs (unclean-quit warning,
    # version-mismatch, save-slot confirmation) that require more key
    # presses than a fresh first-launch.  100 tries at 500ms covers
    # the slowest realistic path (cutscene skip + save load).
    [int]$MaxAttempts = 100,
    [int]$PollMs = 500
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Definition
$inspector = Join-Path $root "target\release\live-inspector.exe"
if (-not (Test-Path $inspector)) {
    throw "live-inspector not built: $inspector"
}

# P/Invoke for SendInput + SetForegroundWindow.  keybd_event is simpler
# and sufficient for our purposes (menu nav).
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class WinInput {
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);

    public const int SW_RESTORE = 9;
    public const byte VK_RETURN = 0x0D;
    public const byte VK_SPACE = 0x20;
    public const byte VK_ESCAPE = 0x1B;
    public const uint KEYEVENTF_KEYUP = 0x0002;
}
"@

function Find-SekiroPid {
    $p = Get-Process -Name sekiro -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $p) { return 0 }
    return $p.Id
}

function Send-Enter {
    [WinInput]::keybd_event([WinInput]::VK_RETURN, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [WinInput]::keybd_event([WinInput]::VK_RETURN, 0, [WinInput]::KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

function Test-InSave {
    param([int]$ProcId)
    # Run the inspector; look for the player HP line being OK.
    $out = & $inspector --pid $ProcId 2>&1
    return ($out | Select-String "^\[ok\]\s+player HP").Count -gt 0
}

if ($ProcId -eq 0) {
    $ProcId = Find-SekiroPid
    if ($ProcId -eq 0) {
        throw "sekiro.exe not running - launch it first"
    }
}

Write-Host "auto-enter target pid: $ProcId"
$proc = Get-Process -Id $ProcId

# Bring the window foreground.
[WinInput]::ShowWindow($proc.MainWindowHandle, [WinInput]::SW_RESTORE) | Out-Null
[WinInput]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 300

# Check whether we're already in a save (re-invoked, for instance).
if (Test-InSave -ProcId $ProcId) {
    Write-Host "already in-game; nothing to do"
    return
}

Write-Host "pressing Enter until player HP chain resolves..."
for ($i = 1; $i -le $MaxAttempts; $i++) {
    # Re-focus in case the user clicked away.
    [WinInput]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
    Send-Enter
    Start-Sleep -Milliseconds $PollMs
    if (Test-InSave -ProcId $ProcId) {
        Write-Host "in-game after $i Enter presses"
        return
    }
}
Write-Warning "did not reach in-game state within $MaxAttempts attempts"
