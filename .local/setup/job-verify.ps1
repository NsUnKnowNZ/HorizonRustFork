$ErrorActionPreference = "Continue"
$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
$jobStamp = Get-Date -Format yyyyMMdd-HHmmss
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\previous-game.log" -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | ForEach-Object {
  "running: $($_.ProcessId) $($_.CommandLine)"
  if ($_.CommandLine -like "*horizon-native-test*") { "  -> stuck test copy, closing it"; Stop-Process -Id $_.ProcessId -Force }
}
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
$ctl = "$repo\.local\native\target\fast\horizon-ctl.exe"
function C { "`n>>> $args"; & $ctl --socket "$env:TEMP\horizon-native-test.json" --timeout 180 @args 2>&1 | ForEach-Object { "$_" } }
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { "TEST SKIPPED: a Horizon game is running"; exit 0 }
$shots = "$repo\.local\native\shots"
function Platform {
  C command "gamemode creative"
  C command "execute in minecraft:overworld run tp @s 203.5 152 195.5 0 25"
  C wait 40
  C command "fill 190 149 190 216 149 216 minecraft:grass_block"
  C command "fill 190 150 190 216 156 216 minecraft:air"
  C command "fill 190 148 190 216 148 216 minecraft:stone"
  C command "weather clear"
}
function Scene {
  Platform
  C command "fill 199 149 199 201 149 201 minecraft:farmland"
  C command "fill 199 150 199 201 150 201 minecraft:wheat[age=7]"
  C command "setblock 202 149 199 minecraft:farmland"
  C command "setblock 202 150 199 minecraft:carrots[age=7]"
  C command "setblock 203 149 199 minecraft:farmland"
  C command "setblock 203 150 199 minecraft:potatoes[age=7]"
  C command "fill 205 150 200 207 153 202 minecraft:oak_leaves[persistent=true]"
  C command "fill 197 150 203 197 152 203 minecraft:diamond_ore"
  C command "setblock 198 150 203 minecraft:redstone_ore"
  C command "setblock 199 150 203 minecraft:emerald_ore"
  C command "setblock 200 150 203 minecraft:iron_ore"
  C command "setblock 202 150 203 minecraft:netherrack"
  C command "setblock 202 151 203 minecraft:fire"
  C command "setblock 204 150 203 minecraft:campfire"
  C command "setblock 205 150 198 minecraft:lantern"
  C command "fill 196 150 197 197 150 198 minecraft:short_grass"
}
# Enhanced Visuals and AppleSkin, seen at last.
C launch --world NativeTest --wait
C wait 20
C respawn
C wait 20
Platform
C command "difficulty easy"
C command "gamemode survival"
C command "tp @s 203.5 150 195.5 0 10"
C command "effect clear @s"
C wait 20
C command "damage @s 5 minecraft:generic"
C wait 2
C screenshot "$shots\ev-hit.png"
C wait 40
C command "damage @s 10 minecraft:generic"
C wait 30
C screenshot "$shots\ev-low.png"
C command "effect give @s minecraft:hunger 3 255 true"
C wait 70
C command "item replace entity @s hotbar.0 with minecraft:cooked_beef 8"
C select 1
C wait 10
C screenshot "$shots\appleskin.png"
C state
C command "effect give @s minecraft:instant_health 1 5"
C quit
"=== JOB DONE"
Copy-Item -Force "$PSScriptRoot\job-run.log" "$PSScriptRoot\logs\job-$jobStamp.log" -ErrorAction SilentlyContinue
