# Builds Horizon from this clone with the native mods in .local\native\mods applied.
# Output: .local\native\target\fast\horizon-client.exe (+ horizon-ctl.exe). The clone is never modified.
param([switch]$Fresh)
$ErrorActionPreference = "Continue"
$native = $PSScriptRoot
$repo = Resolve-Path "$native\..\.."
$source = "$native\source"; $stage = "$native\stage"; $mods = "$native\mods"
$env:CARGO_TARGET_DIR = "$native\target"
function Step($m) { "`n### $m  ($(Get-Date -Format HH:mm:ss))" }

Step "horizon-mixins tool"
$tool = "$native\horizon-mixins\target\release\horizon-mixins.exe"
if (-not (Test-Path $tool)) {
  Push-Location "$native\horizon-mixins"
  $env:CARGO_TARGET_DIR = "$native\horizon-mixins\target"
  & cargo build --locked --release -p horizon-mixins-cli 2>&1 | ForEach-Object { "$_" } | Select-String "error|Finished" | ForEach-Object { "$_" }
  $env:CARGO_TARGET_DIR = "$native\target"
  Pop-Location
}
if (-not (Test-Path $tool)) { "FAILED: no horizon-mixins"; exit 1 }

Step "pristine source (HEAD + local changes)"
$head = (& git -C $repo rev-parse HEAD).Trim()
$diff = (& git -C $repo diff HEAD -- crates | Out-String)
# New source files (not yet known to git) under crates go along too.
$added = @(& git -C $repo ls-files --others --exclude-standard -- crates)
foreach ($f in $added) { $diff += "`n+++ $f`n" + (Get-Content -Raw "$repo\$f") }
# Patches kept apart from the engine (.local\patches, those listed in enabled.txt).
$patches = "$repo\.local\patches"
if (Test-Path "$patches\enabled.txt") {
  $diff += "`nenabled: " + (Get-Content -Raw "$patches\enabled.txt")
  foreach ($n in (Get-Content "$patches\enabled.txt" | Where-Object { $_.Trim() -and -not $_.StartsWith("#") })) {
    $diff += "`n" + (Get-Content -Raw "$patches\$($n.Trim())\$($n.Trim()).patch")
  }
}
$sig = "$head`n$($diff.GetHashCode())"
$stamp = "$native\source.sig"
if ($Fresh -or -not (Test-Path $stamp) -or (Get-Content $stamp -Raw) -ne $sig) {
  if (Test-Path $source) { Remove-Item -Recurse -Force $source }
  New-Item -ItemType Directory $source | Out-Null
  & git -C $repo archive --format=tar -o "$native\source.tar" HEAD
  & tar -xf "$native\source.tar" -C $source --exclude CLAUDE.md 2>&1 | Out-Null
  Remove-Item "$native\source.tar"
  if ($diff.Trim()) {
    foreach ($f in @(& git -C $repo diff --name-only HEAD -- crates) + $added) {
      New-Item -ItemType Directory -Force (Split-Path "$source\$f") | Out-Null
      Copy-Item -Force "$repo\$f" "$source\$f"
    }
    if (Select-String -Quiet -Path "$source\crates\horizon-client\src\game\functions.rs" -Pattern "sync_tag_registries") { "local changes applied" } else { "WARNING: local changes NOT applied" }
  }
  & python "$patches\apply_patches.py" $source $patches 2>&1 | ForEach-Object { "$_" }
  if ($LASTEXITCODE -ne 0) { "FAILED: patches"; exit 1 }
  Set-Content -NoNewline -Path "$source\.horizon-revision" -Value $head
  Set-Content -NoNewline -Path $stamp -Value $sig
  "exported $head"
} else { "unchanged $head" }

Step "apply mods: $((Get-ChildItem -Directory $mods).Name -join ', ')"
$applied = "$native\stage-new"
if (Test-Path $applied) { Remove-Item -Recurse -Force $applied }
& $tool apply --workspace $source --mods $mods --out $applied 2>&1 | ForEach-Object { "$_" }
if ($LASTEXITCODE -ne 0) { "FAILED: horizon-mixins apply"; exit 1 }
& python "$native\sync_tree.py" $applied $stage

Step "cargo build (fast)"
Push-Location $stage
& cargo build --locked --profile fast -p horizon-client -p horizon-client-lib -p horizon-server -p horizon-prepare --bin horizon-client --bin horizon-ctl --bin horizon-server --bin horizon-prepare 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath "$native\build-full.log" | Select-String -Pattern "^error|Finished" | ForEach-Object { "$_" }
$ok = $LASTEXITCODE -eq 0
Pop-Location
if (-not $ok) { "FAILED: cargo build"; exit 1 }
"BUILD OK"
