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
Remove-Item Env:HORIZON_PORTAL_VIEW_TEST -ErrorAction SilentlyContinue
C launch --world NativeTest --wait
C wait 40
C command "gamemode creative"
C command "execute in minecraft:the_nether run forceload add -16 -16 16 16"
C command "execute in minecraft:the_nether run tp @s 4.5 72 -3.5"
C wait 120
C command "execute in minecraft:the_nether run fill -2 69 -6 10 69 12 minecraft:glowstone"
C command "execute in minecraft:the_nether run fill -2 70 -6 10 79 12 minecraft:air"
C command "execute in minecraft:the_nether run fill 2 70 1 7 75 1 minecraft:obsidian"
C command "execute in minecraft:the_nether run fill 3 71 1 6 74 1 minecraft:nether_portal[axis=x]"
C command "execute in minecraft:the_nether run fill -2 70 9 10 79 9 minecraft:diamond_block"
C command "execute in minecraft:the_nether run fill 3 72 8 6 75 8 minecraft:glowstone"
C command "execute in minecraft:the_nether run fill 0 70 5 1 73 6 minecraft:lava"
C wait 20
C command "execute in minecraft:overworld run tp @s 0.5 100 0.5"
C wait 60
C command "execute in minecraft:the_nether run forceload remove -16 -16 16 16"
C command "fill 28 119 2 36 119 8 minecraft:glass"
C command "fill 29 120 10 34 125 10 minecraft:obsidian"
C command "fill 30 121 10 33 124 10 minecraft:nether_portal[axis=x]"
C command "tp @s 31.5 120 5.5 0 0"
C wait 200
C screenshot "$repo\.local\native\shots\portalview.png"
C command "tp @s 29.5 121 3.5 -30 10"
C wait 40
C screenshot "$repo\.local\native\shots\portalview2.png"
C command "execute in minecraft:the_nether run tp @s 4.5 71 -3.5 0 0"
C wait 30
C screenshot "$repo\.local\native\shots\portalview3a.png"
C wait 170
C screenshot "$repo\.local\native\shots\portalview3.png"
C messages
C logs
C quit
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\last-game.log" -ErrorAction SilentlyContinue
