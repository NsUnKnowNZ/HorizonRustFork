# Packed resource roots

## Why

On Windows the first open of a file that hasn't been opened for about ten minutes costs
2.5–5 ms (the anti-virus scan), however small the file. A modded start used to read a
merged folder of about 110,000 files and links (`.horizon-cache/mod-content/<stamp>`), and
the mods' own 80,000 loose files under `horizon-mods/`: 140–190 s before the title screen
whenever the files had gone cold.

## What a root is now

- **One data file per root.** `horizon-data.bin` (`horizon_core::prepared_files`) holds every
  file under the root's `data/`, `assets/` and `worldgen/structure` (sounds aside) behind one
  index. The prepared vanilla folder, a resource-pack root and a content root each have one.
- **Content roots are packed.** The merge (`mod_content::build`) no longer mirrors the base
  file by file: what it writes, what it shares from a mod and what it keeps from the base
  are only noted, then written into the new root's data file in one pass. The bytes the
  merge wrote come from memory, the base's from the base's own data file, a mod's from its
  `.hmod` or folder. The folder keeps a few thousand links (sounds, tables) and a
  `horizon-packed` marker.
- **One read layer.** Everything that reads a resource by path goes through
  `horizon_core::rfs` (`read`, `read_to_string`, `read_dir`, `metadata`, `exists`, `is_file`,
  `is_dir`, `File::open`), which answers from the data file above the path when there is one
  and from the disk otherwise. Callers don't know which kind of root they read.
- **Mods are one file each in the game folder.** `make-game-folder.ps1` has the game pack
  each mod folder into `<id>.hmod` (`HORIZON_PACK_MODS`); the start-up check of the mods for
  changes then looks at 18 files, not 80,000.

## Measured (RX 7800 XT machine, cold files)

| Start | Before | Packed |
|---|---|---|
| First start after a mod change, repo (loose mods) | ~190 s | 32 s to in-world |
| First start, game folder (`.hmod` mods) | — | 36 s to in-world |
| Next start, game folder | 22–26 s | 10 s to in-world (resources 4.8 s) |

## Switches

- `HORIZON_LOOSE_ROOT=1` builds the old mirror, to look at a root's files or compare.
- `HORIZON_NO_DATA_FILE=1` reads the folders only (no data files written or read).
