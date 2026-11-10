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
git add -f .local/setup/job-commit.ps1 .local/setup/make-game-folder.ps1 .local/game-launcher/Cargo.toml .local/game-launcher/src/main.rs .gitignore FORK.md
git add -u -- "Play Better OneBlock (modded).bat" "Play Better OneBlock.bat" "Play OneBlock Fresh (modded).bat" 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
A clean game folder with one program to start it; the .bat shortcuts are gone

.local/setup/make-game-folder.ps1 makes D:\HorizonOneBlock (or -Game <folder>):
Horizon OneBlock.exe (.local/game-launcher, starts bin\horizon-client.exe with
no console window), the prepared resources in bin\, the mods, and on its first
run the player's worlds, packs and screenshots moved over from the source
folder (NativeTest stays for tests; the packs keep a link here). Later runs
refresh the program, the resources and the mods only.

Supplementaries (local content): six blocks its jar has stale files for but the
mod never registers are taken out; raked gravel has its model.

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
