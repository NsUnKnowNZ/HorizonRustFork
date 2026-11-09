"""Moves Horizon Portals out of the working tree into .local/patches/horizon-portals.
Run from the repo root. Backs every touched file up first."""
import difflib, os, shutil, subprocess, sys, time

ROOT = os.getcwd()
OUT = os.path.join(ROOT, '.local', 'patches', 'horizon-portals')
BACKUP = os.path.join(ROOT, '.local', 'patches', 'backup-' + time.strftime('%Y%m%d-%H%M%S'))

PORTAL_ONLY = [
    'crates/horizon-client-render/src/gfx.rs',
    'crates/horizon-client-render/src/streaming.rs',
    'crates/horizon-client-render/src/terrain.wgsl',
    'crates/horizon-client/src/animate_tick.rs',
    'crates/horizon-client/src/game/lan.rs',
    'crates/horizon-client/src/game/lan_actions.rs',
    'crates/horizon-client/src/game/remote.rs',
    'crates/horizon-client/src/game/remote_chunks.rs',
    'crates/horizon-client/src/game/render_tick.rs',
    'crates/horizon-client/src/game/server_travel.rs',
    'crates/horizon-client/src/lan/hub.rs',
    'crates/horizon-world/src/level/hooks.rs',
    'crates/horizon-world/src/level/portal.rs',
    'crates/horizon-world/src/level/portal_travel.rs',
    'crates/horizon-world/src/level/shapes.rs',
]
NEW = ['crates/horizon-client/src/game/portal_view.rs']


def read(p):
    with open(os.path.join(ROOT, p), encoding='utf-8') as f:
        return f.read().replace('\r\n', '\n')


def head(p):
    out = subprocess.run(['git', 'show', 'HEAD:' + p], cwd=ROOT, capture_output=True)
    if out.returncode != 0:
        sys.exit('git show failed for ' + p)
    return out.stdout.decode('utf-8').replace('\r\n', '\n')


def undo(text, pairs, p):
    """`text` with each (new, old) edit taken back; each must occur exactly once."""
    for new, old, n in pairs:
        if text.count(new) != n:
            sys.exit(f'{p}: expected {n} of {new[:60]!r}, found {text.count(new)}')
        text = text.replace(new, old)
    return text


def mixed_bases():
    g = 'crates/horizon-client/src/game.rs'
    np_ = 'crates/horizon-client/src/game/nether_portal.rs'
    game = undo(read(g), [
        ('mod portal_view;\n', '', 1),
        ('        let portal_view = self.portal_view_for_frame(camera);\n', '', 1),
        ('                renderer.set_portal_view(portal_view);\n', '', 1),
    ], g)
    nether = undo(read(np_), [
        ('    pub(super) fn border_bounds_of(&self, dim: Dim) -> BorderBounds {',
         '    fn border_bounds_of(&self, dim: Dim) -> BorderBounds {', 1),
        ('    /// NetherPortalFix: the corner of the portal the one at `corner` is linked to.\n'
         '    pub(super) fn portal_link(&self, nether: bool, corner: BlockPos) -> Option<BlockPos> {\n'
         '        portal_links::linked(&self.world_root, nether, corner)\n'
         '    }\n', '', 1),
        ('                    64,\n                    Axis::Y,\n                    64,\n',
         '                    21,\n                    Axis::Y,\n                    21,\n', 1),
        ('            64,\n            Axis::Y,\n            64,\n',
         '            21,\n            Axis::Y,\n            21,\n', 1),
    ], np_)
    return {g: game, np_: nether}


def diff(path, old, new):
    a = old.splitlines(keepends=True)
    b = new.splitlines(keepends=True)
    body = list(difflib.unified_diff(a, b, n=3))
    if not body:
        return ''
    header = f'diff --git a/{path} b/{path}\n'
    if not old:
        header += 'new file mode 100644\n--- /dev/null\n'
    else:
        header += f'--- a/{path}\n'
    header += f'+++ b/{path}\n'
    return header + ''.join(body[2:])


def main():
    bases = mixed_bases()
    for p in PORTAL_ONLY:
        bases[p] = head(p)
    for p in NEW:
        bases[p] = ''
    # Back up everything first.
    for p in bases:
        dst = os.path.join(BACKUP, p)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(os.path.join(ROOT, p), dst)
    patch = ''.join(diff(p, bases[p], read(p)) for p in sorted(bases))
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, 'horizon-portals.patch'), 'w', encoding='utf-8', newline='\n') as f:
        f.write(patch)
    # The working tree without the portals.
    for p, base in bases.items():
        full = os.path.join(ROOT, p)
        if base:
            with open(full, 'w', encoding='utf-8', newline='\n') as f:
                f.write(base)
        else:
            os.remove(full)
    print('patch', len(patch), 'bytes;', len(bases), 'files; backup in', BACKUP)


main()
