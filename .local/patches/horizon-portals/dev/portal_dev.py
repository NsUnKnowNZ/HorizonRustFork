"""Horizon Portals dev tree, apart from the working tree (which other sessions edit).
  portal_dev.py init      base/ = HEAD's copies of the patched files, work/ = base + the patch
  portal_dev.py patch     candidate.patch = diff(base, work), in the patch's own format
Run from the repo root."""
import difflib, os, subprocess, sys
ROOT = os.getcwd()
DEV = os.path.join(ROOT, '.local', 'tmp', 'portal-dev')
PATCH = os.path.join(ROOT, '.local', 'patches', 'horizon-portals', 'horizon-portals.patch')
src = open(os.path.join(ROOT, '.local', 'patches', 'apply_patches.py'), encoding='utf-8').read()
ns = {}
exec(src.rsplit('\nmain()', 1)[0].replace("if __name__ == '__main__':", ''), ns)

def files():
    return ns['parse'](open(PATCH, encoding='utf-8').read())

def init():
    for f in files():
        p = f['path']
        for sub in ('base', 'work'):
            os.makedirs(os.path.dirname(os.path.join(DEV, sub, p)), exist_ok=True)
        if not f['new']:
            out = subprocess.run(['git', 'show', 'HEAD:' + p], capture_output=True, timeout=100)
            assert out.returncode == 0, p
            text = out.stdout.decode('utf-8').replace('\r\n', '\n')
            for sub in ('base', 'work'):
                open(os.path.join(DEV, sub, p), 'w', encoding='utf-8', newline='\n').write(text)
        ns['apply_file'](os.path.join(DEV, 'work'), f)
        print('ok', p)

def diff(path, old, new):
    a = old.splitlines(keepends=True); b = new.splitlines(keepends=True)
    body = list(difflib.unified_diff(a, b, n=3))
    if not body:
        return ''
    header = f'diff --git a/{path} b/{path}\n'
    header += 'new file mode 100644\n--- /dev/null\n' if not old else f'--- a/{path}\n'
    header += f'+++ b/{path}\n'
    return header + ''.join(body[2:])

def patch():
    out = ''
    for dirpath, _, names in os.walk(os.path.join(DEV, 'work')):
        for n in sorted(names):
            full = os.path.join(dirpath, n)
            rel = os.path.relpath(full, os.path.join(DEV, 'work')).replace(os.sep, '/')
            base = os.path.join(DEV, 'base', rel)
            old = open(base, encoding='utf-8').read() if os.path.exists(base) else ''
            out += diff(rel, old, open(full, encoding='utf-8').read())
    # the patch's own order: by path
    parts = ['diff --git ' + p for p in out.split('diff --git ') if p]
    parts.sort(key=lambda p: p.split('\n', 1)[0])
    open(os.path.join(DEV, 'candidate.patch'), 'w', encoding='utf-8', newline='\n').write(''.join(parts))
    print('candidate.patch', sum(len(p) for p in parts), 'bytes,', len(parts), 'files')

{'init': init, 'patch': patch}[sys.argv[1]]()
