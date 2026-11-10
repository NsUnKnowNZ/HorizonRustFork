# Horizon Portals (patch)

Nether portals of any size for Horizon, kept apart from the engine so it can be switched off or
carried to a new Horizon version. Applied by `.local/native/build-native.ps1` to the exported
source when `horizon-portals` is listed in `.local/patches/enabled.txt` (delete the line to turn
it off). Needs the content mod `horizon-mods/horizonportals` for the flat portal block.

- Stage 1: portal frames up to 64x64; flat (horizontal) portals lit with fire in an obsidian ring.
- Stage 2: no loading screen through a Nether portal.
- A Horizon client's player goes through a Nether portal at once on touch (no 4 s wait); other
  clients keep the rules' wait.

Stages 3 and 4 (looking through a portal into the other dimension, walking straight through)
were taken out on 2026-11-10 at the user's request: third-person cameras, half-loaded other
sides and boxed-in exits made it glitchy in play. The last patch with them is in git history
(commit 205989ab1b) and in `.local/tmp/portal-dev/see-through-last.patch`.

To change the portal code: work in a tree apart from the working tree, so the engine's own
uncommitted changes are never touched (scripts in `dev/`, run from the repo root):
`python dev/portal_dev.py init` makes `.local/tmp/portal-dev/base` (HEAD's copies of the patched
files) and `work` (base + this patch); edit `work`; `python dev/portal_dev.py patch` writes
`candidate.patch`; `python dev/check_apply.py` tries it on copies of the working tree's files;
then copy it over `horizon-portals.patch`. (The older `make_portal_patch.py` puts the patch into
the working tree and takes it out again: it reverts the patched files to HEAD, so only with no
other uncommitted change in them.)
