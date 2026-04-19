# sekiro-coop launcher.
#
# Preflight-checks the build output, stages the DLL into me3's mods
# directory, and invokes me3 to launch Sekiro with Arxan disabled and
# online blocked.
#
# Usage:
#   .\launch.ps1                    # launch as host
#   .\launch.ps1 -Peer client       # launch as client
#   .\launch.ps1 -NoLaunch          # preflight only

[CmdletBinding()]
param(
    [ValidateSet("host", "client")]
    [string]$Peer = "host",

    [string]$BindAddr = "0.0.0.0:28000",
    [string]$PeerAddr = "127.0.0.1:28001",

    [string]$LogFilter = "info,sekiro_coop=debug",

    [switch]$NoLaunch,
    [switch]$RebuildDll,
    [switch]$Overlay,
    # Opt-in only.  Auto-enter spams SendKeys which steals focus
    # and interrupts anything the user is doing in another window.
    # Prefer the DLL's in-process save-load path once implemented.
    [switch]$AutoEnter
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $root

function Preflight-Check {
    Write-Host "=== preflight ===" -ForegroundColor Cyan

    # MinHook
    $lib = Join-Path $root "vendor\lib\MinHook.x64.lib"
    if (-not (Test-Path $lib)) {
        throw "MinHook not vendored: $lib missing. Re-run this setup."
    }
    Write-Host "  [ok]  MinHook.x64.lib present"

    # me3
    $me3 = Join-Path $root "tools\me3\bin\me3.exe"
    if (-not (Test-Path $me3)) {
        throw "me3 not installed: $me3 missing."
    }
    Write-Host "  [ok]  me3 installed"

    # Sekiro
    $sekiro = "Z:\SteamLibrary\steamapps\common\Sekiro\sekiro.exe"
    if (-not (Test-Path $sekiro)) {
        throw "sekiro.exe not found at $sekiro"
    }
    Write-Host "  [ok]  sekiro.exe at $sekiro"

    # Steam - me3 refuses to launch without it.  If not running, start
    # it silently and wait briefly for the client to initialise.
    $steamProc = Get-Process -Name steam -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $steamProc) {
        $steamExe = "C:\Program Files (x86)\Steam\steam.exe"
        if (Test-Path $steamExe) {
            Write-Host "  [..]  Steam not running; starting silently..."
            Start-Process -FilePath $steamExe -ArgumentList '-silent' -WindowStyle Minimized | Out-Null
            for ($i = 0; $i -lt 30; $i++) {
                Start-Sleep -Seconds 1
                $steamProc = Get-Process -Name steam -ErrorAction SilentlyContinue | Select-Object -First 1
                if ($null -ne $steamProc) { break }
            }
            # Give Steam a few more seconds to finish login + cache handoff.
            Start-Sleep -Seconds 5
        }
    }
    if ($null -ne $steamProc) {
        Write-Host "  [ok]  Steam running (pid $($steamProc.Id))"
    } else {
        Write-Warning "Steam is NOT running - me3 will likely refuse to launch Sekiro"
    }

    # DLL
    $dllFeatures = @()
    if ($Overlay) { $dllFeatures = @("--features", "overlay") }
    $dllPath = Join-Path $root "target\release\sekiro_coop.dll"
    if ($RebuildDll -or (-not (Test-Path $dllPath))) {
        Write-Host "  [..]  building sekiro_coop.dll ..."
        Push-Location $root
        $buildCmd = @("build", "-p", "sekiro-coop-dll", "--release") + $dllFeatures
        & cargo @buildCmd
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
        Pop-Location
    }
    if (-not (Test-Path $dllPath)) { throw "DLL not built: $dllPath" }
    Write-Host "  [ok]  sekiro_coop.dll built"

    # Copy DLL into me3 mods dir.  If a previous Sekiro is still holding
    # the file (common race after a quick kill), wait briefly + retry.
    $modsDir = Join-Path $root "tools\me3\sekiro-coop-mods"
    if (-not (Test-Path $modsDir)) { New-Item -ItemType Directory -Path $modsDir | Out-Null }
    $dst = Join-Path $modsDir "sekiro_coop.dll"
    $copied = $false
    for ($attempt = 1; $attempt -le 10; $attempt++) {
        try {
            Copy-Item -Force -Path $dllPath -Destination $dst -ErrorAction Stop
            $copied = $true
            break
        } catch {
            # Probably still locked by a dying sekiro.exe; wait and try again.
            if ($attempt -eq 1) {
                Write-Host "  [..]  DLL locked; waiting for previous sekiro.exe to release..."
                $p = Get-Process -Name sekiro -ErrorAction SilentlyContinue
                if ($p) {
                    $p | Stop-Process -Force -ErrorAction SilentlyContinue
                }
            }
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $copied) { throw "DLL copy failed after 10 attempts: $dst locked" }
    Write-Host "  [ok]  DLL staged to $modsDir"

    # Profile
    $prof = Join-Path $root "tools\me3\sekiro-coop.me3"
    if (-not (Test-Path $prof)) {
        throw "me3 profile missing: $prof. Re-run this setup."
    }
    Write-Host "  [ok]  sekiro-coop.me3 profile present"
}

function Set-SessionEnv {
    $env:SEKIRO_COOP_PEER = $Peer
    $env:SEKIRO_COOP_BIND = $BindAddr
    if ($PeerAddr -ne "") {
        $env:SEKIRO_COOP_PEER_ADDR = $PeerAddr
    }
    $env:SEKIRO_COOP_LOG = $LogFilter
    $env:SEKIRO_COOP_APPLY_REMOTE = "1"
    Write-Host "=== session env ===" -ForegroundColor Cyan
    Write-Host "  SEKIRO_COOP_PEER          = $env:SEKIRO_COOP_PEER"
    Write-Host "  SEKIRO_COOP_BIND          = $env:SEKIRO_COOP_BIND"
    Write-Host "  SEKIRO_COOP_PEER_ADDR     = $env:SEKIRO_COOP_PEER_ADDR"
    Write-Host "  SEKIRO_COOP_LOG           = $env:SEKIRO_COOP_LOG"
    Write-Host "  SEKIRO_COOP_APPLY_REMOTE  = $env:SEKIRO_COOP_APPLY_REMOTE"
}

function Launch-Sekiro-Foreground {
    Write-Host "=== launching sekiro via me3 (foreground) ===" -ForegroundColor Cyan
    Push-Location (Join-Path $root "tools\me3")
    & .\bin\me3.exe launch `
        --disable-arxan true `
        --online false `
        -p .\sekiro-coop.me3
    $code = $LASTEXITCODE
    Pop-Location
    Write-Host "me3 exited with code $code"
    return $code
}

function Launch-Sekiro-Background {
    Write-Host "=== launching sekiro via me3 (background) ===" -ForegroundColor Cyan
    $me3Dir = Join-Path $root "tools\me3"
    $me3Exe = Join-Path $me3Dir "bin\me3.exe"
    $procArgs = @(
        "launch",
        "--disable-arxan", "true",
        "--online", "false",
        "-p", ".\sekiro-coop.me3"
    )
    $p = Start-Process -FilePath $me3Exe `
        -ArgumentList $procArgs `
        -WorkingDirectory $me3Dir `
        -PassThru
    Write-Host "me3 PID: $($p.Id)"
    return $p
}

function Run-AutoEnter {
    Write-Host "=== auto-enter save ===" -ForegroundColor Cyan
    $autoEnter = Join-Path $root "auto-enter.ps1"
    if (-not (Test-Path $autoEnter)) {
        Write-Warning "auto-enter.ps1 missing; skipping"
        return
    }
    # Wait for sekiro.exe to exist (give me3 time to start it + decrypt BHDs).
    $deadline = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $deadline) {
        $p = Get-Process -Name sekiro -ErrorAction SilentlyContinue
        if ($null -ne $p) { break }
        Start-Sleep -Milliseconds 500
    }
    if ($null -eq $p) {
        Write-Warning "sekiro.exe never appeared; skipping auto-enter"
        return
    }
    # Let the main menu render before spamming Enter.
    Start-Sleep -Seconds 8
    & powershell -ExecutionPolicy Bypass -File $autoEnter -ProcId $p.Id
}

Preflight-Check
Set-SessionEnv
if ($NoLaunch) {
    Write-Host "[--NoLaunch] skipped game launch; env is set in this shell"
} elseif ($AutoEnter) {
    Launch-Sekiro-Background | Out-Null
    Run-AutoEnter
    Write-Host "launch.ps1 done; me3 + sekiro keep running in background"
} else {
    Launch-Sekiro-Foreground
}
