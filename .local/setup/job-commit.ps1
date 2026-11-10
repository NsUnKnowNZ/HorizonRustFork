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
& rustfmt --edition 2024 crates/horizon-client/src/game/lan_entities.rs crates/horizon-client/src/game/native_world_draw.rs crates/horizon-client/src/game/commands/data.rs crates/horizon-shaders/src/post.rs crates/horizon-client-render/src/gfx.rs 2>&1 | Select-Object -First 8 | ForEach-Object { "$_" }
"rustfmt exit $LASTEXITCODE"
git add -u -- crates 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 12 | ForEach-Object { "$_" }
$msg = @"
Shader packs: see-through text, text displays and mod draws; display data

Three things were missing or wrong under a shader pack, and two about
display entities with or without one.

- See-through text (a name tag's copy through walls, a text display with
  see_through) was drawn by Horizon into its own picture with no depth,
  and the pack's programs then drew over it wherever nothing had written
  depth: it never showed under any pack. It is now drawn over the pack's
  picture, before the hand.
- Translucent entity parts drawn by a program that does not blend keep
  the cutout test at 0.1. Bliss turns gbuffers_entities_translucent off
  by default and falls back to gbuffers_entities, whose output alpha
  packs the lightmap beside the texel's alpha, so the test at 0 passed
  every texel: text displays were solid boxes of their colour.
- A native mod's world draws go to the pack's block-entity program
  instead of Horizon's own draw, which a pack with packed gbuffers
  (Bliss, Photon) never showed; text, shadows and the other special
  layers stay Horizon's.
- A text display's ``score`` text pieces show the score, resolved when
  its data is sent (Display.TextDisplay resolves its text for the
  entity).
- ``data modify entity`` on a text, block or item display applies; it
  failed for every kept entity but a marker.

Tested offscreen with screenshots under Bliss, BSL, Complementary
Reimagined, Photon and no pack: a text display, a see-through one with
a score piece recoloured by data modify, and Cinematic Interactions'
items on a crafting table.

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | Select-Object -First 6 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/setup/make-friend-zip.ps1 .local/setup/body-zip.txt .local/native/mods/cinematic_interactions .local/setup/body-obnew.txt .local/setup/body-shpack.txt .local/setup/body-dbg.txt .local/setup/body-dbg2.txt .local/setup/body-hand.txt 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Cinematic Interactions: the player's arm in the scene; OneBlock's displays

Cinematic Interactions (from its public description; none of its code):
the player's own arm, with their skin, reaches into every scene but the
lectern's. The hand follows the pointer over the station, holds the
stack picked up, presses down on a click, and takes the hammer's handle
at the anvil, whose blow shakes the view. What it lets go of drops the
last of the way onto the table. The icon on the pointer and the icons
flying to the station give way to the hand while it is out.

OneBlock: the converter turns ``text set value '{json}'`` into a text
compound (1.21.5's form), so the pack's hint and its upgrade countdown
keep their text when the pack updates them.

The game folder for friends: make-friend-zip.ps1 zips the clean game
folder without the worlds, the player's own files, the shader packs and
bin\resources (Minecraft's assets). "Horizon OneBlock.exe" then starts
the game with the resources that player prepared themselves: the folder
named in horizon-resources.txt, a Horizon source folder near the game
folder, or the Horizon launcher's; with none it opens a note that says
what to do. --find-resources only writes down which it would use.

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

$made = & "$PSScriptRoot\make-game-folder.ps1" 2>&1 | ForEach-Object { "$_" }
$made
if ($made -match 'GAME FOLDER DONE') { & "$PSScriptRoot\make-friend-zip.ps1" 2>&1 | ForEach-Object { "$_" } } else { "the friends' zip was left as it is: the game folder was not refreshed" }
