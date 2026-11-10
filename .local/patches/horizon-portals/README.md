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

- Stage 4 (2026-11-10): the portal connects straight through.
  - A portal's blocks draw nothing of their own (`mesher.rs`): the surface shows the other side,
    or that level's fog colour until its chunks are here. No purple anywhere.
  - With a `shaders/` pack the view is drawn over the pack's picture (`gfx.rs` `portal_over`),
    behind what the pack's depth has in front (its `depthtex1` where it keeps one).
  - A Horizon client's player goes through as its middle crosses the portal's plane
    (`server_travel.rs` `crossed_portal_plane`): no wait, no cooldown, landed in the same tick,
    its place, look and motion kept (`Hub::land_through`, a landing relative to where the
    client stands). Other clients keep vanilla's rules.
  - The client keeps the other side's columns as the new level's (`take_other_side`), sets the
    player aside across the Respawn (`carry`) and holds the last frame (200 ms at most) until
    the section it came out in is drawn. Once through the plane the whole view ahead is the
    other side until the server has moved the player (`PortalView::entered`).
  - The shader pack for the other side is built ahead while a portal is in view
    (`shader_packs::build_ahead_for`, engine side), so the first frame there has it.

To change the portal code: work in a tree apart from the working tree, so the engine's own
uncommitted changes are never touched (scripts in `dev/`, run from the repo root):
`python dev/portal_dev.py init` makes `.local/tmp/portal-dev/base` (HEAD's copies of the patched
files) and `work` (base + this patch); edit `work`; `python dev/portal_dev.py patch` writes
`candidate.patch`; `python dev/check_apply.py` tries it on copies of the working tree's files;
then copy it over `horizon-portals.patch`. (The older `make_portal_patch.py` puts the patch into
the working tree and takes it out again: it reverts the patched files to HEAD, so only with no
other uncommitted change in them.)
