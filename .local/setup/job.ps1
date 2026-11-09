$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
$env:GIT_TERMINAL_PROMPT = "0"
$env:GCM_INTERACTIVE = "never"
if (Test-Path "$repo\.git\index.lock") { Remove-Item -Force "$repo\.git\index.lock"; "removed a stale index.lock" }
"CLAUDE.md: $((Get-Item CLAUDE.md -Force).Attributes) $((Get-Item CLAUDE.md -Force).LinkType)"
$changed = & git diff --name-only | Where-Object { $_ -ne "CLAUDE.md" }
$changed | ForEach-Object { "  $_" }
& git add -- @changed 2>&1 | Where-Object { $_ -notmatch "LF will be replaced" } | ForEach-Object { "$_" }
& git add -f -- horizon-mods/biome_paint 2>&1 | Where-Object { $_ -notmatch "LF will be replaced" } | ForEach-Object { "$_" }
& git diff --cached --stat 2>&1 | Select-Object -Last 3 | ForEach-Object { "$_" }
$msg = @"
Engine changes for Better OneBlock and the Biome Paint content mod

The Horizon source changes the first commit left out: Java pack import,
Armor Hider, crafting tweaks, the shader-pack crash guard, FabricSkyboxes/Nuit
skies, the HUD vignette fix, decorative blocks and shapes in horizon-port-pack,
and horizon-mods/biome_paint (CC0 content).

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bcrac8CAckUphGV
"@
& git commit -q -m $msg 2>&1 | ForEach-Object { "$_" }
& git log --oneline -3 2>&1 | ForEach-Object { "$_" }
& git push fork fork:main 2>&1 | ForEach-Object { "$_" }
"push exit: $LASTEXITCODE"
& git status --short --untracked-files=no 2>&1 | ForEach-Object { "left: $_" }
"=== JOB DONE"
