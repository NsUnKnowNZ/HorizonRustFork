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
git add -- crates/horizon-client/src/game/control.rs crates/horizon-client/src/bin/horizon-ctl/commands.rs docs/horizon-ctl-commands.md 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 5 | ForEach-Object { "$_" }
$msg = @"
horizon-ctl: the Left Alt key

``key alt`` (or ``lalt``) holds, taps and releases Left Alt, so a harness
can drive what a native mod reads from the keys held (Cinematic
Interactions shows the player at their station while it is down).

Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | Select-Object -First 6 | ForEach-Object { "$_" }
git add -u -- crates docs 2>&1 | ForEach-Object { "$_" }
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/native/mods/cinematic_interactions .local/setup/body-ci1.txt .local/setup/body-ci2.txt .local/setup/body-ci3.txt .local/setup/body-ci4.txt .local/setup/body-ci5.txt .local/setup/body-ci5b.txt .local/setup/body-ci6.txt .local/setup/body-ci7.txt .local/setup/body-ob1.txt .local/setup/body-ob2.txt 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Cinematic Interactions for Horizon; the OneBlock bar for every player

Cinematic Interactions (written from its page; none of its code), a
native mod: a right click on a workstation flies the camera to it, hides
the HUD behind letterbox bars and puts the inventory in a panel below;
the station is worked in the world. Escape, E, a movement key or jump
flies back, Left Alt shows the player at the station, a click on a
station nearby glides over to it, and sneak + right click still opens
the usual menu.

- Crafting table: its top is the 3x3 grid; what lies on it stays, every
  player sees it and it drops with the table (a synced chunk
  attachment). A recipe guide lists what can be crafted, searches by
  name, marks what is missing and lays a recipe out from the inventory.
- Furnace, smoker, blast furnace, brewing stand, campfire: their slots
  are parts of the block to click; meters show the fire, the cooking and
  the brew, whose ingredient shrinks into the bottles. H by a campfire
  sits the player down beside it.
- Anvil, grindstone, smithing table, stonecutter, loom, cartography
  table: stacks laid on them stay in the inventory, the result floats
  over the station and a click makes it, paying what the menu would.
- Enchanting table: the three offers for the item and lapis laid on it,
  from the player's own enchantment seed; the item rises to the book as
  it is enchanted.
- Lectern: its book opens over it; the pages turn (the block's page and
  redstone pulse follow), Take Book takes it.
- Jukebox and chiseled bookshelf: discs and books go in and come out by
  clicks; the disc sinks into the jukebox.
- Crafter: its grid lies on its top; an empty cell clicked turns off or
  on. Beacon: a payment laid before it, its powers picked and confirmed.

Tested offscreen, station by station: a pickaxe crafted and a recipe
laid out; iron smelted and its experience paid; three water bottles
brewed awkward; a sword mended on the anvil for 2 levels, its sharpness
ground off, a diamond sword smithed to netherite, stone cut; an iron
sword enchanted for 1 level and 1 lapis; a lectern's page turned and its
book taken; a disc played and taken back; two books shelved; a red
pattern woven on a banner; a crafter's cell filled and another turned
off; a beacon given Speed for an ingot.

OneBlock: the pack turned its progress bar on, and enabled its settings
trigger, only for players whose first join was in spectator. Anyone
joining in survival, or under a new name (scores go by name), had
neither. The converter now adds a tick line and a function that give
every player without a setting the bar and the trigger; a player who
turns it off keeps it off. Tested on a copy of a world under a new name:
the bar showed on the next block, the settings opened, off and on again
worked.

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

& "$PSScriptRoot\make-game-folder.ps1" 2>&1 | ForEach-Object { "$_" }
