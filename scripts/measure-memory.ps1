# Starts EzAddon, then reports memory and CPU for the app and all of its
# WebView2 child processes: while active, after minimizing, and after the
# window is closed to the system tray.
param([Parameter(Mandatory)] [string] $Exe, [int] $SettleSeconds = 15)

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win { [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd); }
"@

$app = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds $SettleSeconds

function Get-Tree([int] $id) {
  foreach ($c in Get-CimInstance Win32_Process -Filter "ParentProcessId=$id") { $c; Get-Tree $c.ProcessId }
}

function Snapshot {
  $ids = @($app.Id) + @(Get-Tree $app.Id | ForEach-Object ProcessId)
  foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $p }
  }
}

function Report([string] $label) {
  $procs = @(Snapshot)
  $cpu0 = ($procs | Measure-Object { $_.TotalProcessorTime.TotalMilliseconds } -Sum).Sum
  Start-Sleep -Seconds 10
  $procs = @(Snapshot)
  $cpu1 = ($procs | Measure-Object { $_.TotalProcessorTime.TotalMilliseconds } -Sum).Sum
  $ws = ($procs | Measure-Object WorkingSet64 -Sum).Sum / 1MB
  $priv = ($procs | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
  $cpuPct = ($cpu1 - $cpu0) / 10000 / [Environment]::ProcessorCount * 100
  $lines = @("### $label", "", "| Process | PID | Working set (MB) | Private (MB) |", "|---|---|---|---|")
  foreach ($p in $procs) {
    $lines += "| $($p.ProcessName) | $($p.Id) | $([math]::Round($p.WorkingSet64 / 1MB, 1)) | $([math]::Round($p.PrivateMemorySize64 / 1MB, 1)) |"
  }
  $lines += "| **Total ($($procs.Count) processes)** | | **$([math]::Round($ws, 1))** | **$([math]::Round($priv, 1))** |"
  $lines += ""
  $lines += "Average CPU over 10 s idle: $([math]::Round($cpuPct, 2)) %"
  $lines += ""
  $text = $lines -join "`n"
  Write-Host $text
  if ($env:GITHUB_STEP_SUMMARY) { Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value $text }
}

if ($env:GITHUB_STEP_SUMMARY) { Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value "## EzAddon memory use (first-run screen)`n" }
Report "Active window"

$app.Refresh()
[void][Win]::ShowWindow($app.MainWindowHandle, 6)   # SW_MINIMIZE
Start-Sleep -Seconds 5
Report "Minimized (WebView2 memory target: low)"

[void]$app.CloseMainWindow()   # WM_CLOSE, like clicking the X
Start-Sleep -Seconds 5
if ($app.HasExited) {
  Write-Host "App exited on close (tray unavailable on this machine)."
} else {
  Report "Closed to tray (window and WebView2 shut down)"
}

if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
