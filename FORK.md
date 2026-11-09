# HorizonRust fork: Better OneBlock on Horizon

This is [Horizon](https://github.com/horizon-rust/horizon) (Minecraft Java 26.3 in Rust) with the work to
play the Better OneBlock modpack on it natively. No Mojang files are here: prepare them from your own
Minecraft as upstream Horizon does.

## What is where
- `crates/...`: Horizon source changes (Java shader/resource pack import, Armor Hider, shader-pack crash
  guard, FabricSkyboxes/Nuit skies, Enhanced Visuals drawing fix, decorative blocks in `horizon-port-pack`, ...).
- `.local/native/mods/`: the native mods (Better OneBlock's mods rewritten for Horizon), applied by
  `.local/native/build-native.ps1`.
- `.local/patches/horizon-portals/`: seamless, see-through portals of any size, applied by the same build
  (`enabled.txt`, `apply_patches.py`; regenerate with `make_portal_patch.py`).
- `.local/setup/`: the build and test jobs (`job.bat`, `job-runner.bat` and the `job-*.ps1` scripts).
- `.local/ports/better-oneblock/NATIVE-PORTS.md`: the ledger of every mod in the pack and how far it is.
- `horizon-mods/biome_paint/`: the Biome Paint content mod (CC0). The other content mods (Handcrafted,
  Supplementaries, Charm, Bits and Balance, Chipped, ...) are carried over from their jars by
  `horizon-port-pack` and are not in this repository.

`.local/` is ignored by upstream's `.gitignore`; the files above are added on purpose.

## Playing from a fresh clone
`cargo build -p horizon-client` builds plain Horizon. The `Play ... (modded).bat` shortcuts start the build
with the native mods applied and open a Better OneBlock world; neither is in this repository. They need:

1. **Minecraft's files**, prepared from your own Java Edition 26.3 as the README says (`horizon-prepare`).
   They land in `.local/horizon-client/26.3/`.
2. **The modded build**: clone https://github.com/horizon-rust/horizon-mixins into
   `.local/native/horizon-mixins` (this fork is built with its commit `5a64af9`), then run
   `powershell -ExecutionPolicy Bypass -File .local\native\build-native.ps1` (needs Python). It leaves
   `.local\native\target\fast\horizon-client.exe`. `.local\setup\job.bat` is a test job for the machine
   this fork is developed on, not a setup step.
3. **The content mods** (`horizon-mods/<id>/` and `horizon-mods.txt`): the blocks, items, textures and sounds
   carried over from the pack's mod jars by `horizon-port-pack` (`.local/setup/oneblock.ps1` downloads the
   pack from Modrinth and ports it). Only `biome_paint` is here; the rest are their authors' files.
4. **The world** (`saves/OneBlockFresh` or `saves/Better OneBlock`), converted from the pack's world by
   `.local/ports/better-oneblock/convert/convert_oneblock.py`.

Steps 3 and 4 were done by hand over many sessions and have no one-command setup yet.
