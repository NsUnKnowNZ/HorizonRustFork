# Makes or refreshes the clean game folder: only what the game needs, apart from the source.
#   Horizon OneBlock.exe     starts the game (no console window)
#   bin\                     the game program and its prepared resources (never share these)
#   horizon-mods\ ...        mods, worlds, packs and settings
# The first run moves the player's worlds, packs and screenshots over (NativeTest stays for
# tests) and leaves links in the source folder so tests see the same packs. Later runs only
# refresh the program, the resources and the mods; settings and worlds are never overwritten.
param([string]$Game = 'D:\HorizonOneBlock')
$ErrorActionPreference = 'Continue'
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
"=== GAME FOLDER $Game $(Get-Date)"
if (Get-Process horizon-client -ErrorAction SilentlyContinue) { "A game is running: close it first."; exit 1 }
New-Item -ItemType Directory -Force $Game, "$Game\bin", "$Game\saves" | Out-Null
Push-Location "$repo\.local\game-launcher"
& cargo build --release 2>&1 | ForEach-Object { "$_" } | Select-String -Pattern "^error|Finished" | ForEach-Object { "$_" }
Pop-Location
Copy-Item -Force "$repo\.local\game-launcher\target\release\horizon-oneblock.exe" "$Game\Horizon OneBlock.exe"
Copy-Item -Force "$repo\.local\native\target\fast\horizon-client.exe" "$Game\bin\horizon-client.exe"
& robocopy "$repo\.local\horizon-client\26.3" "$Game\bin\resources" /MIR /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
"resources: robocopy $LASTEXITCODE"
& robocopy "$repo\horizon-mods" "$Game\horizon-mods" /MIR /MT:16 /XD .replaced .downloads .horizon-mod-data /NFL /NDL /NJH /NJS /NP | Out-Null
"mods: robocopy $LASTEXITCODE"
foreach ($f in 'horizon-mods.txt','options.txt','horizon-graphics.txt','horizon-extras.txt','horizon-player.txt','servers.dat','horizon.txt','horizon-java-folders.txt','horizon_official_server.txt','horizon_partner_servers.txt') {
  if ((Test-Path "$repo\$f") -and -not (Test-Path "$Game\$f")) { Copy-Item "$repo\$f" "$Game\$f"; "copied $f" }
}
if ((Test-Path "$repo\config") -and -not (Test-Path "$Game\config")) { Copy-Item -Recurse "$repo\config" "$Game\config"; "copied config" }
# The player's own folders move over once; the source keeps a link to each.
foreach ($d in 'resourcepacks','shaderpacks','screenshots') {
  $from = Get-Item "$repo\$d" -ErrorAction SilentlyContinue
  if ($from -and -not $from.LinkType -and -not (Test-Path "$Game\$d")) {
    Move-Item "$repo\$d" "$Game\$d"; "moved $d"
    & cmd /c mklink /J "$repo\$d" "$Game\$d" | Out-Null
  }
}
foreach ($w in Get-ChildItem -Directory "$repo\saves" | Where-Object { $_.Name -ne 'NativeTest' }) {
  if (-not (Test-Path "$Game\saves\$($w.Name)")) { Move-Item $w.FullName "$Game\saves\$($w.Name)"; "moved world $($w.Name)" }
}
"--- $Game"
Get-ChildItem $Game | ForEach-Object { $_.Name }
"=== GAME FOLDER DONE"
