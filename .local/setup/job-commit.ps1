$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT + SERVER SYNC $(Get-Date)"
# The offline server gets the new engine and the both-sides content mods.
if (Test-Path server-oneblock\engine) {
  Copy-Item .local\native\target\fast\horizon-client.exe server-oneblock\engine\horizon-client.exe -Force
  "engine copied"
}
foreach ($m in 'bedrockify','biome_paint','bitsandbalance','charm','chipped','handcrafted','diversity','horizonportals','more_babies','puddleflood','supplementaries','suppsquared','yo_hooks') {
  & robocopy "horizon-mods\$m" "server-oneblock\horizon-mods\$m" /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
}
Copy-Item horizon-mods.txt server-oneblock\horizon-mods.txt -Force
(Get-Content server-oneblock\horizon-mods.txt) | Where-Object { $_ -notmatch "^mod.(beb|ambientsounds):" } | Set-Content server-oneblock\horizon-mods.txt
"server mods synced"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add -u -- crates 2>&1 | ForEach-Object { "$_" }
git add -f .local/native/mods/bitsandbalance .local/native/mods/diversity .local/native/mods/ambientsounds .local/native/mods/visualkeys .local/native/mods/sihywtcamd .local/setup/ambientsounds_build.py .local/setup/body-bnb.txt .local/setup/body-bnb2.txt .local/setup/body-bnb3.txt .local/setup/job-commit.ps1 .local/ports/better-oneblock/NATIVE-PORTS.md 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | ForEach-Object { "$_" }
$msg = @"
AmbientSounds, Bits and Balance's gameplay, Diversity's bundles, VisualKeys, more of the Difficulty mod

- ambientsounds: its ambience engine from its own region files (biomes, temperature, sky, height,
  rain, storms, underwater, block features sampled round the player, day/night curves); sounds
  from a client content mod built locally by ambientsounds_build.py
- bitsandbalance: automatic restock, biome titles, Cascading, Aerodynamic
- diversity: Vacuum, Selective Vacuum and Refill bundles
- visualkeys: a keyboard of the key bindings on the Key Binds page (hold Left Alt)
- sihywtcamd: spawn-time health and attributes
- ledger updated

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -3 2>&1 | ForEach-Object { "$_" }
"=== JOB DONE"
