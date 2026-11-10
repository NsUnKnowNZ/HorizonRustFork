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
git add -u -- crates docs 2>&1 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/patches/horizon-portals/dev/portal_dev.py .local/patches/horizon-portals/dev/check_apply.py .local/setup/body-modsync.txt .local/setup/body-jei-custom.txt .local/setup/body-portal-seamless.txt docs/design/mod-mismatch-download.md crates/horizon-client/src/game/mod_sync.rs crates/horizon-client/src/mod_transfer.rs crates/horizon-client/src/mod_transfer/tests.rs crates/horizon-client/tests/mod_download.rs crates/horizon-protocol/src/build_id.rs crates/horizon-protocol/src/mod_files.rs 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Portals connect straight through; a mod mismatch screen that downloads; JEI mod recipes

Portals (the Horizon Portals patch)
- a portal's surface shows the other side with a shaders/ pack on too: the view goes over
  the pack's picture, behind what the pack's depth has in front of it
- a portal's blocks draw nothing of their own (no purple); until the other side is here the
  surface shows that level's fog colour
- a Horizon client's player goes through as its middle crosses the portal's plane: no wait,
  no cooldown, landed in the same tick where the other side is loaded already, and it
  keeps its place, look and motion (the landing is relative to where its client stands)
- the level seen through the portal is the level entered: its columns come along, and the
  last frame stays up (200 ms at most) until the section the player came out in is drawn
- the arrival is laid out as the view is: the portals' middles together, a quarter turn
  between portals facing different ways

Shader packs
- the pack built for a dimension is kept when the player leaves it and runs again at once
  on the way back; with a portal in view, the pack for its other side is built ahead. Every
  change of dimension used to build it again for 2 to 3 seconds
- debug lines (chunk borders, hitboxes, gizmos) are drawn over the pack's picture; drawn
  into the world, the pack's fog and antialiasing thinned them out

Joining a server whose mods differ (docs/design/mod-mismatch-download.md)
- the screen lists what is missing, different and extra, with both builds' ids (release,
  revision, native mod count, source fingerprint)
- "Download and restart" takes the missing and different content mods from the server
  (horizon:mod_files, 256 KiB chunks, checked by hash), installs them, turns extra ones
  off and joins again; horizon-serve-mods=false in server.properties turns serving off

JEI
- recipes of mod stations are listed: Chipped's workbenches (a group per tag), Charm's
  woodcutter and kiln. A stack with components falls back to its item's recipes

Also
- tag files: an entry without a namespace is minecraft:'s (Chipped's tags name vanilla
  blocks that way and lost them)
- the mod content root is no longer built again for every new build of the game: its
  stamp names the code that merges, not the game's file

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>
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
Pop-Location
Remove-Item Env:CARGO_TARGET_DIR
git worktree remove --force $wt 2>&1 | ForEach-Object { "$_" }
Remove-Item -Recurse -Force "$env:TEMP\horizon-verify-target" -ErrorAction SilentlyContinue
git status --short --untracked-files=no 2>&1 | Select-Object -First 20 | ForEach-Object { "$_" }
"=== JOB DONE"
