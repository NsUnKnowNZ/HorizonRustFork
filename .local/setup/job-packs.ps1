$ErrorActionPreference = "Continue"
$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe'" | ForEach-Object {
  "running: $($_.ProcessId) $($_.CommandLine)"
  if ($_.CommandLine -like "*horizon-native-test*") { "  -> stuck test copy, closing it"; Stop-Process -Id $_.ProcessId -Force }
}
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
$ctl = "$repo\.local\native\target\fast\horizon-ctl.exe"
function C { "`n>>> $args"; & $ctl --socket "$env:TEMP\horizon-native-test.json" --timeout 180 @args 2>&1 | ForEach-Object { "$_" } }
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { "TEST SKIPPED: a Horizon game is running"; exit 0 }
$shots = "$repo\.local\native\shots"
function Scene {
  C command "gamemode creative"
  C command "weather clear"
  C command "execute in minecraft:overworld run fill 24 119 -6 40 119 6 minecraft:grass_block"
  C command "fill 24 120 -6 40 128 6 minecraft:air"
  C command "fill 26 120 2 28 120 4 minecraft:short_grass"
  C command "fill 30 119 2 32 119 4 minecraft:farmland"
  C command "fill 30 120 2 32 120 4 minecraft:wheat[age=7]"
  C command "setblock 34 120 3 minecraft:carrots[age=7]"
  C command "setblock 35 120 3 minecraft:potatoes[age=7]"
  C command "fill 36 120 2 38 124 4 minecraft:oak_leaves[persistent=true]"
  C command "fill 24 120 4 25 122 4 minecraft:diamond_ore"
  C command "setblock 26 120 5 minecraft:redstone_ore"
  C command "setblock 27 120 5 minecraft:emerald_ore"
  C command "setblock 28 120 5 minecraft:netherrack"
  C command "setblock 28 121 5 minecraft:fire"
  C command "setblock 33 120 5 minecraft:water"
  C command "setblock 30 120 -1 minecraft:glowstone"
}
# Shader packs (one at a time), noon then sunset.
$packs = @(
  @("photon", "photon_v1.4.zip"), @("solas", "Solas Shader V3.9.zip"),
  @("bliss", "Bliss_v2.1.2_(Chocapic13_Shaders_edit).zip"), @("rvoxels", "rethinking-voxels_r0.1-beta9.zip"),
  @("unbound", "ComplementaryUnbound_r5.9.3.zip"), @("sdv", "superDuperVanilla.zip"),
  @("nostalgia", "Nostalgia_v5.1.zip"), @("miniature", "miniature-shader-2.19.zip")
)
foreach ($p in $packs) {
  $env:HORIZON_SHADER_PACK = $p[1]
  C launch --world NativeTest --wait
  C wait 40
  Scene
  C command "time set 6000"
  C command "tp @s 31.5 122 -6.5 0 20"
  C wait 120
  C screenshot "$shots\shader-$($p[0]).png"
  C stats
  C command "time set 12300"
  C command "tp @s 31.5 122 -6.5 -60 5"
  C wait 60
  C screenshot "$shots\shader-$($p[0])-2.png"
  C logs
  C quit
  Start-Sleep 5
  Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$shots\shader-$($p[0]).log" -ErrorAction SilentlyContinue
}
Remove-Item Env:HORIZON_SHADER_PACK -ErrorAction SilentlyContinue
# Resource packs: options.txt is the player's, so it is put back afterwards.
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
  C wait 40
  Scene
  C command "time set 6000"
  if ($s[0] -eq "blocks") {
    C command "tp @s 31.5 122 -6.5 0 20"; C wait 100; C screenshot "$shots\packs-blocks.png"
    C command "time set 12500"; C command "tp @s 31.5 122 -6.5 -60 -10"; C wait 60; C screenshot "$shots\packs-sky.png"
  }
  if ($s[0] -eq "gui") {
    C command "item replace entity @s hotbar.0 with minecraft:diamond_sword[enchantments={sharpness:5}]"
    C command "item replace entity @s hotbar.1 with minecraft:enchanted_book[stored_enchantments={mending:1}]"
    C command "item replace entity @s armor.chest with minecraft:diamond_chestplate[enchantments={protection:4}]"
    C command "gamemode survival"
    C command "tp @s 31.5 122 -6.5 0 20"; C wait 40
    C open-inventory; C wait 20; C screenshot "$shots\packs-gui.png"; C close
    C screenshot "$shots\packs-gui-hud.png"
  }
  if ($s[0] -eq "mobs") {
    C command "summon minecraft:cow 31 120 0"; C command "summon minecraft:sheep 33 120 0"; C command "summon minecraft:zombie 29 120 0"
    C command "summon minecraft:pig 35 120 0"; C command "summon minecraft:chicken 27 120 0"
    C command "tp @s 31.5 121 -5.5 0 10"; C wait 100; C screenshot "$shots\packs-mobs.png"
    C key f5 tap; C wait 20; C screenshot "$shots\packs-player.png"; C key f5 tap; C key f5 tap
  }
  C logs
  C quit
  Start-Sleep 5
  Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$shots\packs-$($s[0]).log" -ErrorAction SilentlyContinue
}
Copy-Item -Force "$repo\.local\setup\options.txt.bak" "$repo\options.txt"
"=== PACKS DONE"
