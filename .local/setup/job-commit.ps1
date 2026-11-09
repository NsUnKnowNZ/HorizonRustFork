$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT + SERVER SYNC $(Get-Date)"
# The offline server gets the new engine and the both-sides content mods.
if (Test-Path server-oneblock\engine) {
  Copy-Item .local\native\target\fast\horizon-client.exe server-oneblock\engine\horizon-client.exe -Force
  "engine copied"
}
foreach ($m in 'bedrockify','biome_paint','bitsandbalance','camerapture','charm','chipped','handcrafted','diversity','horizonportals','more_babies','mutantmonsters','puddleflood','supplementaries','suppsquared','yo_hooks') {
  & robocopy "horizon-mods\$m" "server-oneblock\horizon-mods\$m" /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
}
Copy-Item horizon-mods.txt server-oneblock\horizon-mods.txt -Force
(Get-Content server-oneblock\horizon-mods.txt) | Where-Object { $_ -notmatch "^mod.(beb|ambientsounds|sounds):" } | Set-Content server-oneblock\horizon-mods.txt
"server mods synced"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add -u -- crates docs tools 2>&1 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -u -- FORK.md "Play Better OneBlock (modded).bat" "Play OneBlock Fresh (modded).bat" 2>&1 | ForEach-Object { "$_" }
git add crates/horizon-client/src/game/mod_textures.rs crates/horizon-client/src/game/seamless_loading.rs tools/native-hook-fragments/mob_sounds.json tools/native-hook-fragments/model_paint.json 2>&1 | ForEach-Object { "$_" }
git add -f .local/native/mods/mutantmonsters .local/native/mods/underlay .local/native/mods/camerapture 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/mutantmonsters_gen/README.md .local/setup/mutantmonsters_gen/layers.py .local/setup/mutantmonsters_gen/anim.py .local/setup/mutantmonsters_gen/gen.py .local/setup/cam_job.py .local/setup/body-underlay.txt .local/setup/body-final.txt .local/setup/body-final2.txt .local/setup/body-final3.txt .local/setup/job-commit.ps1 .local/ports/better-oneblock/NATIVE-PORTS.md 2>&1 | ForEach-Object { "$_" }
Get-ChildItem .local\setup\body-mm-*.txt, .local\setup\body-cam-*.txt | ForEach-Object { git add -f $_.FullName 2>&1 | ForEach-Object { "$_" } }
git diff --cached --stat 2>&1 | Select-Object -Last 70 | ForEach-Object { "$_" }
$msg = @"
Mutant Monsters, Camerapture, Underlay, Seamless Loading Screen

Engine
- content mods: living entity types may copy a giant, skeleton, creeper, enderman,
  snow golem or pig and set their own width, height and eye height
- mobs: native_mob_voice / native_mob_step_sound give a mob a mod's own sounds;
  mob_kind no longer names mod mobs minecraft:<ns>:<id>; native_draw_model_part_with
  paints model parts translucent, flashed, full-bright or as an energy swirl
- mods' own textures in the HUD atlas (mod_textures), a world-only picture for a mod
  (native_grab_world), WebP encoding, mods' own special item models, lecterns take
  #minecraft:lectern_books
- seamless_loading: a picture of the world, taken every 10 s of play, is the
  background of its next loading screen
- mod_api: listening() for mixin-only builds (death cancel, damage parties,
  attachment sync)

Native mods
- mutantmonsters: the six mutants and the creeper minion with the mod's models,
  animations, goals, deaths and sounds, written from the mod's source
- camerapture: camera, pictures, picture frames, albums, lectern view
- underlay: carpets, pressure plates and rails under blocks that don't fill their space

The ledger (NATIVE-PORTS.md) lists what differs from each Java mod. FORK.md says what
a fresh clone needs to play the modded worlds.

Co-Authored-By: Claude <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -3 2>&1 | ForEach-Object { "$_" }
"=== PUSHED; clean checkout check"
$wt = "$env:TEMP\horizon-verify"
if (Test-Path $wt) { git worktree remove --force $wt 2>&1 | ForEach-Object { "$_" } }
git worktree add --detach $wt HEAD 2>&1 | Select-Object -Last 1 | ForEach-Object { "$_" }
Push-Location $wt
$env:CARGO_TARGET_DIR = "$env:TEMP\horizon-verify-target"
(Get-Process -Id $PID).PriorityClass = "BelowNormal"
& cargo check --locked -p horizon-client 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|Finished|-->|could not|failed" | Select-Object -First 60 | ForEach-Object { "$_" }
"CLEAN CHECK EXIT $LASTEXITCODE"
Pop-Location
Remove-Item Env:CARGO_TARGET_DIR
git worktree remove --force $wt 2>&1 | ForEach-Object { "$_" }
Remove-Item -Recurse -Force "$env:TEMP\horizon-verify-target" -ErrorAction SilentlyContinue
git status --short -- crates docs tools 2>&1 | Select-Object -First 20 | ForEach-Object { "$_" }
"=== JOB DONE"
