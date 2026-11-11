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
git add -u -- .local 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job-commit.ps1 .local/native/mods/cinematic_interactions .local/native/mods/sihywtcamd .local/native/mods/mob_ai_tweaks .local/setup/body-sih.txt .local/setup/body-sih2.txt .local/setup/body-hand2.txt .local/setup/body-hand3.txt .local/setup/body-golem.txt 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
Cinematic Interactions: a glide to a crafting table froze the game; more of So I Heard and Mob AI Tweaks

Cinematic Interactions: a click on a crafting table from another
station's scene (a furnace's, an anvil's) froze the game for good. The
glide read the table's side through the client's state while its caller
held that state. It reads the caller's copy now. Tested under BSL: from
the anvil's scene, a click on the table beside it glides over and the
game goes on.

So I Heard (0.2), from the list on its page: a creeper takes no blast
and lights up instead, and its own blast leaves those near it slow to
dig and weak; a blaze's and a magma cube's hit set on fire; an
endermite's bite moves the player as a chorus fruit does (not seen in a
test yet); a slime in the Nether's lava becomes a magma cube and a magma
cube in the Overworld's water a slime; a wither at half its health calls
three or four wither skeletons, bursts and keeps the weather stormy;
zombies call for help more and a grown one may be a caller, a runner, a
tank or an unstoppable one; skeletons, wither skeletons, drowned,
zombified piglins and pillagers may carry other weapons; a blaze follows
from half as far; silverfish and endermites are quicker.

Mob AI Tweaks (0.2), the rules of its list (its reworked fights are not
here): vexes and phantoms take three times a projectile's damage; iron
golems, shulkers and silverfish more from the right tool; a wither below
half its health half an arrow's; an elder guardian has 150 health, 8
more armour and takes no blast; elder guardians and ravagers show a boss
bar; bosses are announced; a wither leaves far more experience; a
skeleton the wither effect kills rises as a wither skeleton; a snow
golem has 10 armour; some zombies carry a baby zombie; a wandering
trader blinds who strikes it; the dragon's blow sets on fire; pets and
villagers mend slowly; chickens shed feathers; a glow squid's ink makes
mobs glow.

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
