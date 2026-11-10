# Makes a zip of the clean game folder to pass to a friend privately: the game, its mods and its
# settings, without the worlds, the player's own files, the shader packs and bin\resources
# (Minecraft's assets: every player prepares their own; "Horizon OneBlock.exe" finds them or
# says how). Never put the zip anywhere public: the mods' files belong to their authors.
param([string]$Game = 'D:\HorizonOneBlock', [string]$Out = '')
$ErrorActionPreference = 'Continue'
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
$share = "$repo\.local\share"
if (-not $Out) { $Out = "$share\HorizonOneBlock-friends.zip" }
"=== FRIEND ZIP $Game -> $Out $(Get-Date)"
Push-Location "$repo\.local\game-launcher"
& cargo build --release 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|^warning: unused|Finished|-->" | ForEach-Object { "$_" }
Pop-Location
$exe = "$repo\.local\game-launcher\target\release\horizon-oneblock.exe"
if (-not (Test-Path $exe)) { "FAILED: the launcher was not built"; exit 1 }
if (Test-Path "$share\stage") { Remove-Item -Recurse -Force "$share\stage" }
$stage = "$share\stage\HorizonOneBlock"
New-Item -ItemType Directory -Force $stage | Out-Null
& robocopy $Game $stage /E /MT:16 /NFL /NDL /NJH /NJS /NP /XD "$Game\saves" "$Game\screenshots" "$Game\shaderpacks" "$Game\xaero" "$Game\bin\resources" "$Game\.horizon-cache" "$Game\mod-browser" /XF horizon-player.txt horizon-profiles.txt servers.dat command_history.txt horizon-java-folders.txt "Horizon OneBlock.exe" "Horizon OneBlock - *.txt" | Out-Null
"copied: robocopy $LASTEXITCODE"
Copy-Item -Force $exe "$stage\Horizon OneBlock.exe"
New-Item -ItemType Directory -Force "$stage\saves", "$stage\shaderpacks" | Out-Null
$lines = @(
  '# The folder of the game resources you prepared from your own Minecraft: Java Edition.',
  '# Write its path on a line of its own (your Horizon source folder, or its',
  '# .local\horizon-client\26.3 folder). Leave this as it is to let the game look for it.',
  ''
)
[IO.File]::WriteAllLines("$stage\horizon-resources.txt", $lines)
$lines = @(
  'Horizon OneBlock',
  '',
  '1. Unzip this folder anywhere (a short path such as D:\HorizonOneBlock is best).',
  '2. Start "Horizon OneBlock.exe".',
  '',
  'The first start needs the game resources made from your own Minecraft: Java Edition 26.3.',
  'They are not in this folder. The game looks for them by itself:',
  '  - in a Horizon source folder next to this one, in your home folder, on your desktop,',
  '    in your documents or at the top of a drive (its .local\horizon-client\26.3 folder),',
  '  - in the Horizon launcher''s data (playhorizon.dev).',
  'If it finds none, a note opens that says what to do; you can also write the path yourself',
  'in horizon-resources.txt.',
  '',
  'To play together everyone needs this same folder: the same game program and the same mods.',
  'When the host updates the game, ask for the new zip and unzip it over this folder',
  '(your worlds in saves\ and your settings are kept).',
  '',
  'Not included: worlds, shader packs (put your own in shaderpacks\), your player profile.',
  'Please keep this zip private: the mods'' files belong to their authors.'
)
[IO.File]::WriteAllLines("$stage\READ ME FIRST.txt", $lines)
if (Test-Path $Out) { Remove-Item -Force $Out }
& tar -a -c -f $Out -C "$share\stage" HorizonOneBlock 2>&1 | Select-Object -First 5 | ForEach-Object { "$_" }
"tar exit $LASTEXITCODE"
"--- in the zip"
Get-ChildItem $stage | ForEach-Object {
  $size = if ($_.PSIsContainer) { (Get-ChildItem -Recurse -File $_.FullName | Measure-Object Length -Sum).Sum } else { $_.Length }
  "{0,8:N1} MB  {1}" -f ($size / 1MB), $_.Name
}
"zip: {0:N0} MB  {1}" -f ((Get-Item $Out).Length / 1MB), $Out
# The program's own check, in a copy of the folder as a friend unzips it: the resources it
# would start the game with (here: this computer's own, found by looking).
$try = "$env:TEMP\horizon-friend-check\HorizonOneBlock"
if (Test-Path "$env:TEMP\horizon-friend-check") { Remove-Item -Recurse -Force "$env:TEMP\horizon-friend-check" }
New-Item -ItemType Directory -Force "$try\bin" | Out-Null
Copy-Item "$stage\Horizon OneBlock.exe" $try
Start-Process "$try\Horizon OneBlock.exe" -ArgumentList '--find-resources' -Wait
"launcher check: resources found at $(Get-Content -Raw "$try\Horizon OneBlock - resources found.txt" -ErrorAction SilentlyContinue)"
Remove-Item -Recurse -Force "$env:TEMP\horizon-friend-check"
Remove-Item -Recurse -Force "$share\stage"
"=== FRIEND ZIP DONE"
