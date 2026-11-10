$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT + SERVER SYNC $(Get-Date)"
# The offline server gets the new engine and the both-sides content mods.
if (Test-Path server-oneblock\engine) {
  Copy-Item .local\native\target\fast\horizon-client.exe server-oneblock\engine\horizon-client.exe -Force
  "engine copied"
}
foreach ($m in 'bedrockify','biome_paint','bitsandbalance','camerapture','charm','chipped','handcrafted','diversity','horizonportals','farmersdelight','more_babies','mutantmonsters','puddleflood','supplementaries','suppsquared','yo_hooks') {
  & robocopy "horizon-mods\$m" "server-oneblock\horizon-mods\$m" /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
}
Copy-Item horizon-mods.txt server-oneblock\horizon-mods.txt -Force
(Get-Content server-oneblock\horizon-mods.txt) | Where-Object { $_ -notmatch "^mod.(beb|ambientsounds|sounds):" } | Set-Content server-oneblock\horizon-mods.txt
"server mods synced"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add -u -- crates docs 2>&1 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/setup/make-game-folder.ps1 .local/native/mods/farmersdelight .local/native/mods/handson_delight .local/setup/body-hod2.txt .local/setup/body-hod3.txt
git add -u -- "Play OneBlock Fresh (modded).bat" 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Hands-On Delight for Horizon: the pot and the board worked in place

Hands-On Delight (a native mod over the farmersdelight one, written from its
page and its showcase pictures; none of its code): a right click on a
cooking pot or a cutting board flies the camera to it, hides the HUD and
lays the hotbar along the station's front, with a recipe book beside it.
Escape, E, a movement key or jump flies back.
- Cooking pot: pick a hotbar stack and click the pot to tip it in (a right
  click drops one); the ingredients float and swirl in a broth that browns
  as it fills. Hold the button and circle in the pot to stir with a spoon:
  each round hurries the cooking, steady wide rounds fill three stars, each
  adding a quarter more servings. The book shows the meal the ingredients
  point to and what is still missing; the meal card beside the pot serves
  one into its container from the inventory.
- Cutting board: put an item on it from the hotbar, pick up its tool and
  drag it across the board; the pieces go to the inventory, the tool wears.
Actions go to the server as mod messages and run on the player's tick, so
LAN guests and servers check them.

Farmer's Delight: the pot's contents and stars are sent to clients when
they change; stirring speeds the pot up and its stars add servings.

Tested: carrots, potatoes and beef dropped in from the hotbar (a whole
stack, then one); twelve rounds of stirring gave two stars and a beef stew
of two servings; a cabbage cut on the board into two leaves in the
inventory; leaving restored the camera and the HUD.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | Select-Object -First 12 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -3 2>&1 | ForEach-Object { "$_" }
"=== PUSHED; clean checkout check"
$wt = "$env:TEMP\horizon-verify"
if (Test-Path $wt) { git worktree remove --force $wt 2>&1 | ForEach-Object { "$_" } }
git worktree add --detach $wt HEAD 2>&1 | Select-Object -Last 1 | ForEach-Object { "$_" }
Push-Location $wt
$env:CARGO_TARGET_DIR = "$env:TEMP\horizon-verify-target"
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
& cargo check --locked -p horizon-client 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|Finished|-->|could not|failed|panicked" | Select-Object -First 40 | ForEach-Object { "$_" }
"CLEAN CHECK EXIT $LASTEXITCODE"
& cargo test --locked -p horizon-client --bin horizon-client a_folder_mods_file_is_shared 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|test result|FAILED|panicked|failed" | Select-Object -First 20 | ForEach-Object { "$_" }
"LINK TEST EXIT $LASTEXITCODE"
& cargo test --locked -p horizon-core --lib -- rfs prepared_files 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|test result|FAILED|panicked|failed" | Select-Object -First 20 | ForEach-Object { "$_" }
"CORE TEST EXIT $LASTEXITCODE"
Pop-Location
Remove-Item Env:CARGO_TARGET_DIR
git worktree remove --force $wt 2>&1 | ForEach-Object { "$_" }
Remove-Item -Recurse -Force "$env:TEMP\horizon-verify-target" -ErrorAction SilentlyContinue
git status --short --untracked-files=no 2>&1 | Select-Object -First 20 | ForEach-Object { "$_" }
"=== JOB DONE"

& "$PSScriptRoot\make-game-folder.ps1" 2>&1 | ForEach-Object { "$_" }

$hm = 'D:\HorizonOneBlock\horizon-mods.txt'
if ((Test-Path $hm) -and -not (Select-String -Path $hm -Pattern '^mod\.farmersdelight:' -Quiet)) { Add-Content $hm 'mod.farmersdelight:true'; "game folder: Farmer's Delight on" }
# Fresh Animations' extensions need the base pack under them, and the extensions that match it.
$o = 'D:\HorizonOneBlock\options.txt'
if (Test-Path $o) {
  $t = [IO.File]::ReadAllText($o)
  $n = $t.Replace('"file/FA+All_Extensions-v1.8.1.zip"', '"file/FA+All_Extensions-v1.10.1.zip"')
  if ($n -notmatch 'FreshAnimations_v1\.11\.2') { $n = $n.Replace('"vanilla","file/FA+All_Extensions', '"vanilla","file/FreshAnimations_v1.11.2_1.20+.zip","file/FA+All_Extensions') }
  if ($n -ne $t) { [IO.File]::WriteAllText($o, $n); "game folder: Fresh Animations base pack on" }
  ([regex]::Match($n, '(?m)^resourcePacks:.*$').Value).Substring(0, 160)
}