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
git add -f .local/setup/job-commit.ps1 .local/setup/body-portal-door.txt .local/patches/horizon-portals/horizon-portals.patch
git diff --cached --stat 2>&1 | Select-Object -Last 30 | ForEach-Object { "$_" }
$msg = @"
A portal's surface stands in the middle of its frame

Horizon Portals: the surface showing the other side stood at the face of the
portal's blocks, so it covered the inner faces of the frame and the bottom
obsidian looked removed. It now stands where the purple was, in the middle of
the blocks, and the other side is cut at the same plane: the frame's inner
faces show before it on this side and after it on the other, as one doorway.
Tested with Complementary on (door-*.png).

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
