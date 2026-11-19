# Sums the private memory of a process tree (read only): the root process and every descendant by ParentProcessId.
# Usage: powershell -File tools/mem-tree.ps1 [-RootPid <pid>] [-Name decka-agent]
param(
  [int]$RootPid = 0,
  [string]$Name = "decka-agent"
)

$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
if ($RootPid -eq 0) {
  $roots = @($all | Where-Object { $_.Name -eq "$Name.exe" })
  if ($roots.Count -eq 0) { Write-Output "no process $Name.exe"; exit 1 }
  $RootPid = ($roots | Select-Object -First 1).ProcessId
}

# Breadth-first walk down from the root.
$ids = New-Object System.Collections.Generic.List[int]
$ids.Add($RootPid)
for ($i = 0; $i -lt $ids.Count; $i++) {
  foreach ($child in ($all | Where-Object { $_.ParentProcessId -eq $ids[$i] })) {
    if (-not $ids.Contains([int]$child.ProcessId)) { $ids.Add([int]$child.ProcessId) }
  }
}

$total = 0
foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($null -eq $p) { continue }
  $mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
  $total += $mb
  Write-Output ("{0,7} {1,-22} {2,8} MB" -f $id, $p.ProcessName, $mb)
}
Write-Output ("TREE private total: {0} MB in {1} processes" -f [math]::Round($total, 1), $ids.Count)
