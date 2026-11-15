# Which WebView2 browsers belong to which host? An interrupted sign-in or
# self-check can leave `msedgewebview2.exe` orphans holding a profile, and the
# next child then fails with HRESULT(0x800700AA) "the resource is in use".
#
#   pwsh -NoProfile -File spike/p4/list_webview_procs.ps1
#   pwsh -NoProfile -File spike/p4/list_webview_procs.ps1 -Filter quota-tray-slint
#
# Kill an orphan tree with `taskkill /PID <pid> /T /F` after checking the host
# name in its command line.
param([string]$Filter = "")

Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | ForEach-Object {
  $cmd = $_.CommandLine
  if ($null -eq $cmd) { return }
  if ($Filter -and $cmd -notlike "*$Filter*") { return }
  $short = $cmd.Substring(0, [Math]::Min(260, $cmd.Length))
  Write-Output ("pid={0} ppid={1} {2}" -f $_.ProcessId, $_.ParentProcessId, $short)
}
