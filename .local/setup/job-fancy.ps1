$ErrorActionPreference = "Continue"
$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\previous-game.log" -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | ForEach-Object {
  "running: $($_.ProcessId) $($_.CommandLine)"
  if ($_.CommandLine -like "*horizon-native-test*") { "  -> stuck test copy, closing it"; Stop-Process -Id $_.ProcessId -Force }
}
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
& "$repo\.local\native\build-native.ps1"
if ($LASTEXITCODE -ne 0) { "BUILD FAILED"; exit 1 }
$ctl = "$repo\.local\native\target\fast\horizon-ctl.exe"
function C { "`n>>> $args"; & $ctl --socket "$env:TEMP\horizon-native-test.json" --timeout 180 @args 2>&1 | ForEach-Object { "$_" } }
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { "TEST SKIPPED: a Horizon game is running"; exit 0 }
$shots = "$repo\.local\native\shots"
C launch --world NativeTest --wait
C wait 40
C command "gamemode survival"
C command "tp @s 31.5 122 -6.5 0 10"
C command "advancement revoke @s only minecraft:story/mine_stone"
C command "advancement revoke @s only minecraft:story/cure_zombie_villager"
C command "advancement revoke @s only minecraft:adventure/arbalistic"
C wait 20
C command "advancement grant @s only minecraft:story/mine_stone"
C wait 8
C screenshot "$shots\fancy-toast-in.png"
C wait 40
C screenshot "$shots\fancy-toast-task.png"
C command "advancement grant @s only minecraft:adventure/arbalistic"
C command "advancement grant @s only minecraft:story/cure_zombie_villager"
C wait 160
C screenshot "$shots\fancy-toast-challenge.png"
C wait 130
C screenshot "$shots\fancy-toast-goal.png"
C logs
C quit
"=== FANCY DONE"
