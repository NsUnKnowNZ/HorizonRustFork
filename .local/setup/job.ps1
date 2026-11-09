$repo = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $repo
"=== JOB START $(Get-Date)"
# Commit this fork's work and push it to the private repository NsUnKnowNZ/HorizonRustFork.
$env:GIT_TERMINAL_PROMPT = "0"
$env:GCM_INTERACTIVE = "never"
if (Get-Process git -ErrorAction SilentlyContinue) { "a git process is running: stopping here"; exit 1 }
if (Test-Path "$repo\.git\index.lock") { Remove-Item -Force "$repo\.git\index.lock"; "removed a stale index.lock" }
& git switch fork 2>&1 | ForEach-Object { "$_" }
& git add -u 2>&1 | ForEach-Object { "$_" }
& git add FORK.md .gitignore "Play OneBlock Fresh (modded).bat" crates/horizon-client-render/src/java_packs.rs crates/horizon-client/src/game/armor_hider.rs crates/horizon-client/src/game/crafting_tweaks.rs horizon-mods/biome_paint 2>&1 | ForEach-Object { "$_" }
$local = @(
  ".local/native/build-native.ps1", ".local/native/sync_tree.py", ".local/native/local.patch", ".local/native/mods",
  ".local/patches/apply_patches.py", ".local/patches/make_portal_patch.py", ".local/patches/enabled.txt", ".local/patches/horizon-portals",
  ".local/ports/better-oneblock/NATIVE-PORTS.md", ".local/ports/better-oneblock/convert"
)
& git add -f -- @local 2>&1 | ForEach-Object { "$_" }
Get-ChildItem "$PSScriptRoot" -File -Include *.ps1, *.bat, *.py -Name | ForEach-Object { & git add -f -- ".local/setup/$_" }
"--- staged"
& git diff --cached --stat 2>&1 | Select-Object -Last 5 | ForEach-Object { "$_" }
& git diff --cached --name-only 2>&1 | Where-Object { $_ -match "\.(jar|png|zip|class|bin)$" } | Select-Object -First 20 | ForEach-Object { "binary: $_" }
$msg = @"
Better OneBlock on Horizon: native mods, portals and engine changes

Horizon with the work to play the Better OneBlock modpack natively (see FORK.md):
the native mods in .local/native/mods, the seamless see-through portal patch,
the build and test jobs, the mod ledger, and Horizon source changes (Java pack
import, shader-pack crash guard, FabricSkyboxes/Nuit skies, decorative blocks in
horizon-port-pack, Biome Paint content). No Mojang files.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PvVsmM7bcrac8CAckUphGV
"@
& git commit -q -m $msg 2>&1 | ForEach-Object { "$_" }
& git log --oneline -2 2>&1 | ForEach-Object { "$_" }
& git remote remove fork 2>$null
& git remote add fork https://github.com/NsUnKnowNZ/HorizonRustFork.git
"--- push"
& git push fork fork:main 2>&1 | ForEach-Object { "$_" }
"push exit: $LASTEXITCODE"
"=== JOB DONE"
