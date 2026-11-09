$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT + SERVER SYNC $(Get-Date)"
# The offline server gets the new engine and the both-sides content mods.
if (Test-Path server-oneblock\engine) {
  Copy-Item .local\native\target\fast\horizon-client.exe server-oneblock\engine\horizon-client.exe -Force
  "engine copied"
}
foreach ($m in 'bedrockify','biome_paint','bitsandbalance','charm','chipped','handcrafted','horizonportals','puddleflood','supplementaries','suppsquared') {
  & robocopy "horizon-mods\$m" "server-oneblock\horizon-mods\$m" /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
}
"server mods synced"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add -u -- crates 2>&1 | ForEach-Object { "$_" }
git add -f .local/native/mods/effectual .local/native/mods/sihywtcamd .local/native/mods/fog .local/setup/body-effectual.txt .local/setup/body-grass.txt .local/setup/body-difficulty.txt .local/setup/job-commit.ps1 .local/ports/better-oneblock/NATIVE-PORTS.md 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | ForEach-Object { "$_" }
$msg = @"
LambdaBetterGrass in the mesher, Effectual, and part of So I Heard You Want To Customize Mob Difficulty

- mesher: a grass block's (mycelium's, podzol's, nylium's) side shows its top where the block
  under that side is the same block (LambdaBetterGrass's fancy mode), grass keeping its colour
- effectual: breath steam in the cold, breath bubbles under water, dripping after the water,
  wither smoke, cave dust; kept out of the first-person view
- sihywtcamd: husks fire-proof, arthropods no fall damage, evokers stop arrows, undead heal by
  their hits, endermen can blind
- ledger updated

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -3 2>&1 | ForEach-Object { "$_" }
"=== JOB DONE"
