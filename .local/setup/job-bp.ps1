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
# 1. Biome Paint: the brush in the main hand, a Cherry Grove paint in the other.
C launch --world NativeTest --wait
C wait 20
C respawn
C wait 20
Platform
C command "gamemode survival"
C command "tp @s 203.5 151 195.5 0 30"
C command "item replace entity @s hotbar.0 with biome_paint:biome_brush"
C command "item replace entity @s weapon.offhand with biome_paint:biome_paint[dyed_color=16737146,custom_data={biome_paint:'minecraft:cherry_grove'}]"
C select 1
C wait 10
C screenshot "$shots\biome-paint-before.png"
C interact 202 149 197 up
C wait 10
C screenshot "$shots\biome-paint-first.png"
C command "tp @s 206.5 150 205.5 0 40"
C wait 5
C interact 206 149 207 up
C wait 5
C command "tp @s 203.5 156 190.5 0 35"
C wait 30
C command "execute if biome 204 149 202 minecraft:cherry_grove run say BIOME_PAINT_OK"
C command "data get entity @s SelectedItem"
C command "data get entity @s equipment.offhand"
C screenshot "$shots\biome-paint-after.png"
C chat-log
C quit
"=== JOB DONE"
Copy-Item -Force "$PSScriptRoot\job-run.log" "$PSScriptRoot\logs\job-$jobStamp.log" -ErrorAction SilentlyContinue
