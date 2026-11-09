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
$packs = @(
  @("compl", "ComplementaryReimagined_r5.9.3.zip"),
  @("bsl", "BSL_v10.1.8.zip"),
  @("sildur", "Sildur's Vibrant Shaders v2.02 Extreme.zip"),
  @("makeup", "MakeUp-UltraFast-9.6.zip")
)
foreach ($p in $packs) {
  $env:HORIZON_SHADER_PACK = $p[1]
  C launch --world NativeTest --wait
  C wait 60
  C command "gamemode creative"
  C command "time set 1000"
  C command "weather clear"
  C command "execute in minecraft:overworld run tp @s 31.5 121 -2.5 0 10"
  C wait 120
  C screenshot "$repo\.local\native\shots\shader-$($p[0]).png"
  C command "tp @s 20.5 125 -20.5 -40 25"
  C wait 60
  C screenshot "$repo\.local\native\shots\shader-$($p[0])-2.png"
  C logs
  C quit
  Start-Sleep 5
  Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\shader-$($p[0]).log" -ErrorAction SilentlyContinue
}
Remove-Item Env:HORIZON_SHADER_PACK -ErrorAction SilentlyContinue
# Resource packs: options.txt is the player's, so it is put back afterwards.
Copy-Item -Force "$repo\options.txt" "$repo\.local\setup\options.txt.bak"
(Get-Content "$repo\options.txt") -replace '^resourcePacks:.*$', 'resourcePacks:["vanilla","file/Faithful 32x - 1.21.11.zip","file/FreshAnimations_v1.11.2_1.20+.zip"]' | Set-Content "$repo\options.txt"
C launch --world NativeTest --wait
C wait 60
C command "gamemode creative"
C command "time set 1000"
C command "execute in minecraft:overworld run tp @s 31.5 121 -2.5 0 10"
C command "summon minecraft:cow 31 120 1"
C command "summon minecraft:sheep 33 120 1"
C command "summon minecraft:zombie 29 120 1"
C command "item replace entity @s weapon.mainhand with minecraft:diamond_sword"
C wait 100
C screenshot "$repo\.local\native\shots\packs-faithful.png"
C logs
C quit
Start-Sleep 5
Copy-Item -Force "$repo\.local\setup\options.txt.bak" "$repo\options.txt"
Copy-Item -Force "$env:TEMP\horizon-native-test.log" "$repo\.local\native\shots\packs.log" -ErrorAction SilentlyContinue
