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
Remove-Item "$repo\horizon-mods\.horizon-mod-data\armor_hider\config.json" -ErrorAction SilentlyContinue
C launch --world NativeTest --wait
C wait 40
C command "gamemode creative"
C command "execute in minecraft:overworld run tp @s 31.5 120 5.5 0 0"
C wait 40
C command "setblock 30 120 2 minecraft:chest"
C command "setblock 33 120 2 minecraft:barrel"
C command "setblock 35 120 2 minecraft:chest"
C command "item replace block 30 120 2 container.0 with minecraft:diamond 5"
C command "item replace block 33 120 2 container.4 with minecraft:diamond 2"
C command "item replace block 35 120 2 container.0 with minecraft:dirt 2"
C command "item replace entity @s hotbar.0 with minecraft:diamond 3"
C command "summon minecraft:cow 31 -100 5"
C wait 10
C entities
C command "gamemode survival"
C wait 5
C open-inventory
C click-at 568 427
C key y tap
C wait 10
C click-at 568 427
C wait 10
C close
C look-at 32 120 2
C wait 10
C screenshot "$repo\.local\native\shots\findme.png"
C look-at-block 34 119 7
C mouse middle click
C wait 10
C look-at 31 119 12
C wait 5
C screenshot "$repo\.local\native\shots\ping.png"
C wait 40
C entities
C command "item replace entity @s armor.head with minecraft:diamond_helmet"
C command "item replace entity @s armor.chest with minecraft:golden_chestplate"
C wait 5
C command "gamemode survival"
C open-inventory
C click-at 568 293
C wait 10
C click-at 568 293
C wait 5
C key h tap
C wait 5
C close
C key f5 tap
C look-at 31 121 12
C wait 10
C screenshot "$repo\.local\native\shots\armor.png"
C key f5 tap
C key f5 tap
C command "effect give @s minecraft:hunger 6 255"
C wait 130
C command "item replace entity @s weapon.mainhand with minecraft:cooked_beef 4"
C wait 10
C screenshot "$repo\.local\native\shots\appleskin.png"
C mouse right down
C wait 14
C screenshot "$repo\.local\native\shots\eating.png"
C mouse right up
C command "gamemode creative"
C command "fill 40 120 2 44 120 6 minecraft:obsidian"
C command "fill 41 120 3 43 120 5 horizonportals:horizontal_portal"
C command "tp @s 40.5 121 4.5"
C look-at 42.5 119 4.5
C wait 60
C screenshot "$repo\.local\native\shots\flatview.png"
C messages
C logs
C quit
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\last-game.log" -ErrorAction SilentlyContinue
