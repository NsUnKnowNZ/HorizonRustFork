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
git add -- crates/horizon-client-render/src/gfx.rs 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 5 | ForEach-Object { "$_" }
$msg = @"
Renderer: the world's pictures no longer stall a frame

The world icon, Seamless Loading Screen's picture (taken every ten
seconds of play) and a native mod's world grab are pictures with a size
of their own. The frame that took one waited for the GPU, then
converted, scaled and encoded the PNG on the render thread: about 90 ms
at 2560x1440, a hitch every ten seconds while playing.

Such a frame now only records the copy and is submitted like any other.
The next frames ask for the buffer's mapping without waiting; the first
that finds it mapped hands the buffer to a thread, which writes the PNG
beside its place and then moves it there, so whoever polls for the file
finds it whole once it exists. Full-frame screenshots (F2's, an agent's,
a screenshot sequence's) still wait for theirs, since they report the
result.

Tested offscreen at 2560x1440: 30 and 40 s standing in a world, its
picture taken every ten seconds: no frame over 20 ms, the slowest 10 ms
(before: a 95 ms frame at each picture, 88 ms of it the readback and the
PNG); the world icon (deleted first) and the loading screen's picture
written; a full-frame screenshot still saved.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | Select-Object -First 6 | ForEach-Object { "$_" }
git add -u -- crates docs 2>&1 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/setup/make-game-folder.ps1 .local/native/mods/farmersdelight .local/native/mods/handson_delight .local/setup/body-hod6.txt .local/setup/body-hod7.txt .local/setup/body-launch-off.txt .local/setup/body-perf3.txt
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Hands-On Delight: the skillet, sandwiches, a livelier pot and board

Hands-On Delight (written from its page and showcase pictures; none of
its code), over the farmersdelight native mod:
- Skillet: a right click frames it from the side, its handle reaching
  out to the right. Food a campfire cooks goes in from the hotbar strip
  and fries a side at a time; flick the handle up to toss it over. Two
  bars show how far each side has come; the book lists what it fries.
- Cutting board: bread first, then the fillings stacked on it in any
  order; the last one turns the stack into the sandwich (from the
  crafting recipes of mods' foods with bread in them: the hamburger, the
  sandwiches, the wrap). A click with something that can't go on what
  the board holds takes the top of it back.
- Cooking pot: an open pot, its broth coloured by what is in it and its
  ingredients floating; the spoon turns as it is stirred; bubbles and
  steam over hot pots.
- Items fly between the hotbar strip and the station; a click on a
  station beside the one in view glides the camera there.

Farmer's Delight: the skillet fries up to eight of one food a side at a
time, half the campfire's time a side, and turns it by itself two
seconds after a side is done. The board keeps up to six layers. The
pot's spoon is drawn by its block entity as a model of its own. Clients
see the pot's stage, heat and last stir.

Tested: a hamburger built on the board from bread, a patty, cabbage,
tomato and onion, then taken back with the onion still in hand; three
beef fried on a lit stove, tossed twice, came out as three steaks.

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
& cargo clippy --locked -p horizon-client-render --lib --no-deps 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^(warning|error)|-->" | Select-Object -First 40 | ForEach-Object { "$_" }
"CLIPPY EXIT $LASTEXITCODE"
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