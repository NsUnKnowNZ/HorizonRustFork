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
& "$repo\.local\native\build-native.ps1"
if ($LASTEXITCODE -ne 0) { "BUILD FAILED"; exit 1 }
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
C interact 199 149 199 up
C wait 10
C screenshot "$shots\biome-paint-first.png"
C interact 208 149 208 up
C wait 30
C command "execute if biome 203 149 203 minecraft:cherry_grove run say BIOME_PAINT_OK"
C command "data get entity @s SelectedItem"
C command "data get entity @s equipment.offhand"
C screenshot "$shots\biome-paint-after.png"
C chat-log
C quit
Start-Sleep 5
C launch --world NativeTest --wait
C wait 20
C respawn
C wait 20
C command "gamemode creative"
C command "execute in minecraft:overworld run tp @s 203.5 152 195.5 0 25"
C wait 40
C command "fill 190 149 190 216 149 216 minecraft:grass_block"
C command "fill 190 150 190 216 156 216 minecraft:air"
C command "tp @s 203.5 150 195.5 0 10"
C command "gamemode survival"
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
Start-Sleep 5
# 2. Resource packs again, on a platform in the sky (the last run fell into its own hole).
Copy-Item -Force "$repo\options.txt" "$repo\.local\setup\options.txt.bak"
$sets = @(
  @("blocks", '"vanilla","file/Better-Leaves-9.6.zip","file/Fast Better Grass.zip","file/Fancy Crops v1.4.zip","file/NewGlowingOres-§6[Border]§r.zip","file/LowOnFire v26.2§8.zip","file/Dramatic Skys Demo 1.5.3.36.6.zip"'),
  @("gui", '"vanilla","file/Default-Dark-Mode-26.3-2026.9.0.zip","file/Enchantment Outlines.zip"'),
  @("mobs", '"vanilla","file/FreshAnimations_v1.11.2_1.20+.zip","file/FA+All_Extensions-v1.10.1.zip","file/+1.21.3 Fresh Moves v3.1.1 (With Animated Eyes).zip"')
)
foreach ($s in $sets) {
  $lines = (Get-Content -Encoding UTF8 "$repo\.local\setup\options.txt.bak") -replace '^resourcePacks:.*$', "resourcePacks:[$($s[1])]"
  [IO.File]::WriteAllLines("$repo\options.txt", [string[]]$lines, (New-Object Text.UTF8Encoding $false))
  C launch --world NativeTest --wait
  C wait 20
  C respawn
  C wait 20
  Scene
  C command "time set 6000"
  if ($s[0] -eq "blocks") {
    C command "tp @s 203.5 152 195.5 0 25"; C wait 100; C screenshot "$shots\packs-blocks.png"
    C command "time set 12800"; C command "tp @s 203.5 152 195.5 -90 -15"; C wait 60; C screenshot "$shots\packs-sky.png"
    C command "time set 18000"; C command "tp @s 203.5 152 195.5 0 -60"; C wait 40; C screenshot "$shots\packs-night.png"
  }
  if ($s[0] -eq "gui") {
    C command "item replace entity @s hotbar.0 with minecraft:diamond_sword[enchantments={sharpness:5}]"
    C command "item replace entity @s hotbar.1 with minecraft:enchanted_book[stored_enchantments={mending:1}]"
    C command "item replace entity @s armor.chest with minecraft:diamond_chestplate[enchantments={protection:4}]"
    C command "gamemode survival"
    C command "tp @s 203.5 150 195.5 0 20"; C wait 40
    C open-inventory; C wait 20; C screenshot "$shots\packs-gui.png"; C close
    C screenshot "$shots\packs-gui-hud.png"
  }
  if ($s[0] -eq "mobs") {
    C command "summon minecraft:cow 201 150 200"; C command "summon minecraft:sheep 204 150 200"; C command "summon minecraft:zombie 198 150 201 {NoAI:1b}"
    C command "summon minecraft:pig 206 150 199"; C command "summon minecraft:chicken 200 150 198"
    C command "tp @s 203.5 151 194.5 0 15"; C wait 100; C screenshot "$shots\packs-mobs.png"
    C key f5 tap; C wait 20; C screenshot "$shots\packs-player.png"; C key f5 tap; C key f5 tap
  }
  C logs
  C quit
  Start-Sleep 5
  Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$shots\packs-$($s[0]).log" -ErrorAction SilentlyContinue
}
Copy-Item -Force "$repo\.local\setup\options.txt.bak" "$repo\options.txt"
"=== JOB DONE"
Copy-Item -Force "$PSScriptRoot\job-run.log" "$PSScriptRoot\logs\job-$jobStamp.log" -ErrorAction SilentlyContinue
