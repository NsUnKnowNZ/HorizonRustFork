# Horizon Portals (patch)

Seamless Nether portals for Horizon, kept apart from the engine so it can be switched off or
carried to a new Horizon version. Applied by `.local/native/build-native.ps1` to the exported
source when `horizon-portals` is listed in `.local/patches/enabled.txt` (delete the line to turn
it off). Needs the content mod `horizon-mods/horizonportals` for the flat portal block.

- Stage 1: portal frames up to 64x64; flat (horizontal) portals lit with fire in an obsidian ring.
- Stage 2: no loading screen through a Nether portal.
- Stage 3: looking into a portal shows the other dimension live (server `horizon:portal_view`
  channel, client chunks kept at x + 2^20 chunks, a second render pass cut at the exit portal);
  flat portals show it looking down; no purple overlay while standing in one.

To change the portal code: from the repo root, `python .local/patches/apply_patches.py . .local/patches`
puts it into the working tree; edit; then `python .local/patches/make_portal_patch.py` takes it back
out into this patch (it backs the files up first, under `.local/patches/backup-*`).
