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
