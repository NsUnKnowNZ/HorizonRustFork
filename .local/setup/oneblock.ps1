$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
$repo = Resolve-Path "$here\..\.."
$work = "$repo\.local\ports\better-oneblock"
New-Item -ItemType Directory -Force "$here\packs" | Out-Null
$versions = Invoke-RestMethod "https://api.modrinth.com/v2/project/jllZmspO/version" -Headers @{"User-Agent"="horizon-local-port"}
$v = $versions | Sort-Object date_published -Descending | Select-Object -First 1
$f = $v.files | Where-Object primary | Select-Object -First 1
if (-not $f) { $f = $v.files[0] }
"VERSION $($v.version_number) mc=$($v.game_versions -join ',') loaders=$($v.loaders -join ',') file=$($f.filename) size=$($f.size)"
$pack = "$here\packs\$($f.filename)"
if (-not (Test-Path $pack)) { Invoke-WebRequest -UseBasicParsing $f.url -OutFile $pack }
$sha = (Get-FileHash -Algorithm SHA1 $pack).Hash.ToLower()
if ($sha -ne $f.hashes.sha1) { throw "SHA1 mismatch $sha vs $($f.hashes.sha1)" }
"SHA1 OK"
Set-Location $repo
$ErrorActionPreference = "Continue"
& cargo run --release -p horizon-port-pack -- port $pack $work --resources "$repo\.local\horizon-client\26.3" 2>&1 | ForEach-Object { "$_" }
"PORT_EXIT $LASTEXITCODE"
