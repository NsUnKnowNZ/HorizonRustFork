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
# Mods: one .hmod file each, packed by the game itself from the source's mod folders (it knows
# what its native mods declare). A start then asks the disk about a file a mod, not about the
# 80,000 files of the folders. A mod that can't be packed (one with code) stays a folder.
New-Item -ItemType Directory -Force "$Game\horizon-mods" | Out-Null
$ctl = "$repo\.local\native\target\fast\horizon-ctl.exe"
$sock = "$env:TEMP\horizon-pack-mods.json"
Remove-Item $sock -ErrorAction SilentlyContinue
$env:HORIZON_PACK_MODS = "$Game\horizon-mods"
$p = Start-Process "$repo\.local\native\target\fast\horizon-client.exe" -WorkingDirectory $repo -ArgumentList '--control','--control-socket',"`"$sock`"" -PassThru
$env:HORIZON_PACK_MODS = $null
$packed = ''
for ($i = 0; $i -lt 120 -and -not $p.HasExited; $i++) {
  Start-Sleep 5
  $l = & $ctl --socket $sock --timeout 20 logs --lines 6000 2>&1 | Out-String
  $m = [regex]::Match($l, 'mods packed into[^"\r\n]*|mods not packed[^"\r\n]*')
  if ($m.Success) { $packed = $m.Value -replace '\\\\', '\'; break }
}
& $ctl --socket $sock --timeout 30 quit 2>&1 | Out-Null
Start-Sleep 4
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
"mods: $packed"
$ids = @()
foreach ($d in Get-ChildItem -Directory "$repo\horizon-mods" | Where-Object { -not $_.Name.StartsWith('.') }) {
  $id = $d.Name
  $toml = Join-Path $d.FullName 'horizon-mod.toml'
  if (Test-Path $toml) {
    $m = [regex]::Match((Get-Content -Raw $toml), '(?m)^\s*id\s*=\s*"([^"]+)"')
    if ($m.Success) { $id = $m.Groups[1].Value }
  }
  $ids += $id
  if ($packed -like 'mods packed into*' -and (Test-Path "$Game\horizon-mods\$id.hmod")) {
    if (Test-Path "$Game\horizon-mods\$($d.Name)") { Remove-Item -Recurse -Force "$Game\horizon-mods\$($d.Name)" }
  } else {
    & robocopy $d.FullName "$Game\horizon-mods\$($d.Name)" /MIR /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
    "mod $($d.Name): kept as a folder"
  }
}
# A mod packed here earlier that the source no longer has.
foreach ($s in Get-ChildItem "$Game\horizon-mods\.packed\*.txt" -ErrorAction SilentlyContinue) {
  if ($ids -notcontains $s.BaseName) { Remove-Item -Force "$Game\horizon-mods\$($s.BaseName).hmod" -ErrorAction SilentlyContinue; Remove-Item -Force $s.FullName; "mod $($s.BaseName): removed" }
}
foreach ($f in Get-ChildItem -File "$repo\horizon-mods") { Copy-Item -Force $f.FullName "$Game\horizon-mods\$($f.Name)" }
"mods: $((Get-ChildItem "$Game\horizon-mods\*.hmod").Count) packed files, $((Get-ChildItem -Directory "$Game\horizon-mods" | Where-Object { -not $_.Name.StartsWith('.') }).Count) folders"
foreach ($f in 'horizon-mods.txt','options.txt','horizon-graphics.txt','horizon-extras.txt','horizon-player.txt','servers.dat','horizon.txt','horizon-java-folders.txt','horizon_official_server.txt','horizon_partner_servers.txt') {
  if ((Test-Path "$repo\$f") -and -not (Test-Path "$Game\$f")) { Copy-Item "$repo\$f" "$Game\$f"; "copied $f" }
}
# The native mods' own settings and data.
if ((Test-Path "$repo\horizon-mods\.horizon-mod-data") -and -not (Test-Path "$Game\horizon-mods\.horizon-mod-data")) { Copy-Item -Recurse "$repo\horizon-mods\.horizon-mod-data" "$Game\horizon-mods\.horizon-mod-data"; "copied mod data" }
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
