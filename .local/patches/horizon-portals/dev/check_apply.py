"""Applies candidate.patch to copies of the working tree's files (in /tmp/portal-check): the standard build's patch step, tried apart."""
import os, shutil, sys
ROOT = os.getcwd(); DEV = os.path.join(ROOT, '.local', 'tmp', 'portal-dev')
src = open(os.path.join(ROOT, '.local', 'patches', 'apply_patches.py'), encoding='utf-8').read()
ns = {}; exec(src.rsplit('\nmain()', 1)[0], ns)
out = '/tmp/portal-check'; shutil.rmtree(out, ignore_errors=True)
files = ns['parse'](open(os.path.join(DEV, sys.argv[1] if len(sys.argv) > 1 else 'candidate.patch'), encoding='utf-8').read())
for f in files:
    dst = os.path.join(out, f['path']); os.makedirs(os.path.dirname(dst), exist_ok=True)
    if not f['new']:
        shutil.copy(os.path.join(ROOT, f['path']), dst)
    ns['apply_file'](out, f)
    print('ok', f['path'], len(f['hunks']), 'hunks')
