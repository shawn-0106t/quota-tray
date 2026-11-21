# Measures the idle memory footprint of one app's process TREE - the exe plus
# every child it spawns - in private working set (the number Task Manager's
# "Memory" column shows; unlike total working set it does not double-count
# shared pages across processes).
#
#   pwsh -File spike/p5/mem_footprint.ps1 -Exe ..\kimi\quota-tray.exe
#   pwsh -File spike/p5/mem_footprint.ps1 -ProcessId 12345
#
# -Exe starts the app, waits -SettleSeconds (default 25: the two pollers fire at
# 2 s and their first fetch settles well before that), measures, and stops what
# it started. -ProcessId measures an already-running tree instead, which is what
# you want when the app has been up for a while.
#
# Measured 2026-11-21 (225% DPI, both apps idle with all four windows created,
# no panel/settings open) - see spike/HANDOFF.md §9.2 and PARITY.md §10:
#   quota-tray.exe  (Tauri 1.0.4)  10 processes, 204.6 MB private (184.8 in WebView2)
#   quota-tray-slint.exe (0.1.0)    1 process,   82.2 MB private
param(
    [string]$Exe,
    [int]$ProcessId,
    [int]$SettleSeconds = 25,
    [switch]$KeepRunning
)

$ErrorActionPreference = 'Stop'

function Get-Tree([int]$Root) {
    $procs = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
    $tree = New-Object System.Collections.Generic.List[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($Root)
    while ($queue.Count -gt 0) {
        $cur = $queue.Dequeue()
        if ($tree.Contains($cur)) { continue }
        $tree.Add($cur)
        foreach ($child in $procs | Where-Object { $_.ParentProcessId -eq $cur }) {
            $queue.Enqueue([int]$child.ProcessId)
        }
    }
    return $tree
}

$started = $null
if ($Exe) {
    if (-not (Test-Path -LiteralPath $Exe)) { throw "not found: $Exe" }
    $started = Start-Process -FilePath $Exe -PassThru
    Write-Output "started $($started.Id), settling for $SettleSeconds s"
    Start-Sleep -Seconds $SettleSeconds
    $ProcessId = $started.Id
}
if (-not $ProcessId) { throw 'pass -Exe or -ProcessId' }

# Private working set lives in the perf counters, not in Get-Process.
$private = @{}
Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $_.IDProcess -ne 0 } |
    ForEach-Object { $private[[int]$_.IDProcess] = [int64]$_.WorkingSetPrivate }

$tree = Get-Tree $ProcessId
$sumPrivate = 0
$sumWorkingSet = 0
foreach ($childPid in $tree) {
    $proc = Get-Process -Id $childPid -ErrorAction SilentlyContinue
    if (-not $proc) { continue }
    $priv = if ($private.ContainsKey($childPid)) { $private[$childPid] } else { 0 }
    $sumPrivate += $priv
    $sumWorkingSet += $proc.WorkingSet64
    '{0,-22} pid={1,-6} private={2,7:N1} MB  workingSet={3,7:N1} MB' -f $proc.ProcessName, $childPid, ($priv / 1MB), ($proc.WorkingSet64 / 1MB)
}
'--- {0} processes, private total {1:N1} MB, workingSet total {2:N1} MB' -f $tree.Count, ($sumPrivate / 1MB), ($sumWorkingSet / 1MB)

if ($started -and -not $KeepRunning) {
    Get-Process -Id $started.Id -ErrorAction SilentlyContinue | Stop-Process -Force
    'stopped the instance this script started'
}
