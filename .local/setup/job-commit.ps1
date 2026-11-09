$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT + SERVER SYNC $(Get-Date)"
# The offline server gets the new engine and the both-sides content mods.
if (Test-Path server-oneblock\engine) {
  Copy-Item .local\native\target\fast\horizon-client.exe server-oneblock\engine\horizon-client.exe -Force
  "engine copied"
}
foreach ($m in 'bedrockify','biome_paint','bitsandbalance','charm','chipped','handcrafted','horizonportals','more_babies','puddleflood','supplementaries','suppsquared','yo_hooks') {
  & robocopy "horizon-mods\$m" "server-oneblock\horizon-mods\$m" /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
}
Copy-Item horizon-mods.txt server-oneblock\horizon-mods.txt -Force
(Get-Content server-oneblock\horizon-mods.txt) | Where-Object { $_ -notmatch "^mod.beb:" } | Set-Content server-oneblock\horizon-mods.txt
"server mods synced"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add -u -- crates 2>&1 | ForEach-Object { "$_" }
git add -f .local/native/mods/handcrafted .local/native/mods/charm_tweaks .local/native/mods/yo_hooks .local/native/mods/more_babies .local/native/mods/punchy .local/setup/body-joins.txt .local/setup/body-hooks.txt .local/setup/body-babies.txt .local/setup/job-commit.ps1 .local/ports/better-oneblock/NATIVE-PORTS.md 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | ForEach-Object { "$_" }
$msg = @"
Furniture joining, Grappling Hooks, More Babies, Punchy, more of Charm

- handcrafted: tables join tables round them (16 shapes); couches, benches, shelves and fancy
  beds join their kind beside them facing the same way, on placement and on shape updates
- yo_hooks: cast, hang, swing, climb and jump off a grappling hook, its chain drawn
- more_babies: babies of mobs vanilla has none of (scale attribute), Cursed Clock On A Stick
- punchy: first-person hands lag, bob, sprint, fall and punch
- charm_tweaks: crop replanting, doors open together, chairs on stairs, anvils last longer
- ledger updated

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -3 2>&1 | ForEach-Object { "$_" }
"=== JOB DONE"
