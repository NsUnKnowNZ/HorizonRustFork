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
git add -f .local/setup/job-commit.ps1 .local/setup/make-game-folder.ps1 .gitignore .local/native/mods/ftbultimine .local/setup/body-ult2.txt
git add -u -- "Play OneBlock Fresh (modded).bat" 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
FTB Ultimine for Horizon; mods' world lines over shader packs

FTB Ultimine (a native mod, written from its documented behaviour): hold `
while breaking a block and the matching blocks joined to it go too, at most
64, each broken as the player breaks one (drops, experience, the tool's wear)
for 0.1 exhaustion; it stops when the tool breaks or the food bar is empty.
Shapes, picked by scrolling while the key is held: Shapeless, Small Tunnel,
Small Square, Large Tunnel, Mining Tunnel, Escape Tunnel. The blocks that will
go are outlined, and the shape and count are shown at the top left. Right-
clicking a grown crop with the key held harvests its field and plants it again.
The server half runs on Horizon servers (channel ftbultimine:state).

Native mods' world lines and box outlines were drawn into the world before a
shader pack's programs, which then drew over them: they now also go over the
pack's finished picture, depth tested against its depth, as the debug lines do.

horizon-ctl `key grave` presses the ` key.

Tested: a 5x3x3 coal vein broke at once with the stone beside it kept (45
blocks, 61 coal); a 5x3 wheat field was harvested and replanted, the unripe
plant left as it was.

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
# Fresh Animations' extensions need the base pack under them, and the extensions that match it.
$o = 'D:\HorizonOneBlock\options.txt'
if (Test-Path $o) {
  $t = [IO.File]::ReadAllText($o)
  $n = $t.Replace('"file/FA+All_Extensions-v1.8.1.zip"', '"file/FA+All_Extensions-v1.10.1.zip"')
  if ($n -notmatch 'FreshAnimations_v1\.11\.2') { $n = $n.Replace('"vanilla","file/FA+All_Extensions', '"vanilla","file/FreshAnimations_v1.11.2_1.20+.zip","file/FA+All_Extensions') }
  if ($n -ne $t) { [IO.File]::WriteAllText($o, $n); "game folder: Fresh Animations base pack on" }
  ([regex]::Match($n, '(?m)^resourcePacks:.*$').Value).Substring(0, 160)
}