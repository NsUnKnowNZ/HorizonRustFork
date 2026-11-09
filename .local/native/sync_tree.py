"""Mirror SRC into DST by content: copy only files whose bytes differ, delete files SRC lacks.
Unchanged files keep their mtimes, so Cargo's incremental build sees only real changes."""
import os, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
copied = deleted = 0
seen = set()
for root, dirs, files in os.walk(src):
    rel = os.path.relpath(root, src)
    os.makedirs(os.path.join(dst, rel), exist_ok=True)
    for f in files:
        r = os.path.normpath(os.path.join(rel, f)); seen.add(r)
        a, b = os.path.join(src, r), os.path.join(dst, r)
        if os.path.exists(b) and os.path.getsize(a) == os.path.getsize(b):
            with open(a, 'rb') as fa, open(b, 'rb') as fb:
                if fa.read() == fb.read():
                    continue
        shutil.copyfile(a, b); copied += 1
for root, dirs, files in os.walk(dst):
    rel = os.path.relpath(root, dst)
    for f in files:
        r = os.path.normpath(os.path.join(rel, f))
        if r not in seen:
            os.remove(os.path.join(dst, r)); deleted += 1
print(f"synced: {copied} changed, {deleted} removed")
