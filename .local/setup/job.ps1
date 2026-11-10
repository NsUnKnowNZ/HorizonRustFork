$ErrorActionPreference = "Continue"
$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
$jobStamp = Get-Date -Format yyyyMMdd-HHmmss
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | Where-Object { $_.CommandLine -like "*horizon-native-test*" } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; "closed a test game left running" }
& "$repo\.local\native\build-native.ps1"
if ($LASTEXITCODE -ne 0) { "BUILD FAILED"; exit 1 }
$ctl = "$repo\.local\native\target\fast\horizon-ctl.exe"
function C { "`n>>> $args"; & $ctl --socket "$env:TEMP\horizon-native-test.json" --timeout 180 @args 2>&1 | ForEach-Object { "$_" } }
$shots = "$repo\.local\native\shots"
function Launch {
  C launch --world NativeTest --wait
  for ($i = 0; $i -lt 120; $i++) { $r = & $ctl --socket "$env:TEMP\horizon-native-test.json" --timeout 10 status 2>&1 | Out-String; if ($r -match '"joined":true|"chunks"') { break }; Start-Sleep 5 }
  "ready after $($i*5)s more"
  # The job runs below normal (builds); the game it starts plays at normal priority.
  Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | Where-Object { $_.CommandLine -like "*horizon-native-test*" } | ForEach-Object { (Get-Process -Id $_.ProcessId).PriorityClass = "Normal" }
  C wait 20
  C respawn
  C wait 20
}
function Platform {
  C command "gamemode creative"
  C command "execute in minecraft:overworld run tp @s 203.5 152 195.5 0 25"
  C wait 40
  C command "fill 190 149 190 216 149 216 minecraft:grass_block"
  C command "fill 190 150 190 216 156 216 minecraft:air"
  C command "fill 190 148 190 216 148 216 minecraft:stone"
  C command "weather clear"
  C command "time set 6000"
}
"running games: $((Get-Process horizon-client -ErrorAction SilentlyContinue | Measure-Object).Count)"
& "$PSScriptRoot\job-commit.ps1"

C quit
Start-Sleep 5
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$shots\last-test.log" -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | Where-Object { $_.CommandLine -like "*horizon-native-test*" } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force; "closed a test game left running" }
"=== JOB DONE"
Copy-Item -Force "$PSScriptRoot\job-run.log" "$PSScriptRoot\logs\job-$jobStamp.log" -ErrorAction SilentlyContinue
