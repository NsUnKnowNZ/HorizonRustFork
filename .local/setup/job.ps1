$ErrorActionPreference = 'Continue'
$env:GIT_TERMINAL_PROMPT = '0'; $env:GCM_INTERACTIVE = 'never'
Set-Location D:\HorizonRust
"=== COMMIT $(Get-Date)"
if (Test-Path .git\index.lock) { Remove-Item .git\index.lock -Force }
git add FORK.md "Play OneBlock Fresh (modded).bat" "Play Better OneBlock (modded).bat" 2>&1 | ForEach-Object { "$_" }
git add -f .local/setup/job.ps1 2>&1 | ForEach-Object { "$_" }
git diff --cached --stat 2>&1 | ForEach-Object { "$_" }
$msg = @"
FORK.md: what a fresh clone needs to play the modded worlds

The section and the shortcuts' corrected hint were meant for the previous commit
but were not staged. Also adds the Better OneBlock shortcut.

Co-Authored-By: Claude <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bCrac8CAckUphGV
"@
[IO.File]::WriteAllText("$PWD\.local\setup\commit-msg.txt", $msg)
git commit -F .local\setup\commit-msg.txt 2>&1 | ForEach-Object { "$_" }
git push fork fork:main 2>&1 | ForEach-Object { "$_" }
git log --oneline -2 2>&1 | ForEach-Object { "$_" }
"--- left uncommitted (tracked):"
git status --short --untracked-files=no 2>&1 | Select-Object -First 30 | ForEach-Object { "$_" }
"--- untracked, not ignored:"
git ls-files --others --exclude-standard 2>&1 | Select-Object -First 30 | ForEach-Object { "$_" }
"=== JOB DONE"
