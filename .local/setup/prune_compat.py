"""Finds (and with --apply moves out) content-mod items and blocks made for mods that are not
installed: their item model can't be drawn because a model or texture it needs comes from a
namespace no enabled content mod (nor Minecraft) has. The Java mods register those only when the
other mod is loaded; horizon-port-pack carries every file of the jar.

usage: python3 prune_compat.py [--apply] [mod ids...]
Moved files go to .local/pruned-content/<mod>/..., same relative paths (move them back to undo)."""
import json, os, sys, shutil
repo = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
mods_dir = os.path.join(repo, 'horizon-mods')
vanilla = os.path.join(repo, '.local', 'horizon-client', '26.3', 'assets')
apply = '--apply' in sys.argv
only = [a for a in sys.argv[1:] if not a.startswith('--')]
enabled = []
for line in open(os.path.join(repo, 'horizon-mods.txt'), encoding='utf-8'):
    line = line.strip()
    if line.startswith('mod.') and line.endswith(':true'):
        enabled.append(line[4:-5])
roots = [os.path.join(mods_dir, m, 'assets') for m in enabled if os.path.isdir(os.path.join(mods_dir, m, 'assets'))] + [vanilla]

def find(kind, rid, ext):
    ns, _, path = rid.partition(':') if ':' in rid else ('minecraft', ':', rid)
    for r in roots:
        p = os.path.join(r, ns, kind, path + ext)
        if os.path.isfile(p):
            return p
    return None

cache = {}
def model_problem(mid, depth=0):
    """None when the model and every texture it names can be found."""
    if mid in cache:
        return cache[mid]
    cache[mid] = None
    if depth > 12:
        return None
    if mid.split(':')[-1].startswith('builtin/'):
        return None
    p = find('models', mid, '.json')
    if not p:
        cache[mid] = 'model ' + mid
        return cache[mid]
    try:
        j = json.load(open(p, encoding='utf-8-sig'))
    except Exception as e:
        cache[mid] = 'unreadable ' + mid
        return cache[mid]
    for v in (j.get('textures') or {}).values():
        if isinstance(v, str) and not v.startswith('#'):
            if not find('textures', v, '.png'):
                cache[mid] = 'texture ' + v
                return cache[mid]
    parent = j.get('parent')
    if isinstance(parent, str):
        r = model_problem(parent, depth + 1)
        if r:
            cache[mid] = r
            return r
    return None

def models_of(node, out):
    if isinstance(node, dict):
        if node.get('type', '').endswith('model') and isinstance(node.get('model'), str):
            out.append(node['model'])
        for v in node.values():
            models_of(v, out)
    elif isinstance(node, list):
        for v in node:
            models_of(v, out)

total = 0
for mod in enabled:
    if only and mod not in only:
        continue
    base = os.path.join(mods_dir, mod)
    bad = {}
    for ns in sorted(os.listdir(os.path.join(base, 'assets'))) if os.path.isdir(os.path.join(base, 'assets')) else []:
        items = os.path.join(base, 'assets', ns, 'items')
        if not os.path.isdir(items):
            continue
        for f in sorted(os.listdir(items)):
            if not f.endswith('.json'):
                continue
            try:
                j = json.load(open(os.path.join(items, f), encoding='utf-8-sig'))
            except Exception:
                continue
            ms = []
            models_of(j.get('model'), ms)
            probs = [model_problem(m) for m in ms]
            if ms and all(probs):
                bad[(ns, f[:-5])] = probs[0]
    if not bad:
        continue
    why = {}
    for (ns, name), p in bad.items():
        key = p.split(' ')[0] + ' ' + (p.split(' ')[1].split(':')[0] if ':' in p else '(own)')
        why[key] = why.get(key, 0) + 1
    print(f'{mod}: {len(bad)} items can\'t be drawn: ' + ', '.join(f'{k} x{v}' for k, v in sorted(why.items(), key=lambda kv: -kv[1])[:8]))
    total += len(bad)
    if len(bad) <= 40:
        for (ns, name), why_ in sorted(bad.items()):
            print(f'    {ns}:{name}: missing {why_}')
    if apply:
        moved = trimmed = 0
        ids = {f'{ns}:{name}' for (ns, name) in bad}
        def move(rel):
            global_moved = 0
            src = os.path.join(base, rel)
            if not os.path.isfile(src):
                return 0
            dst = os.path.join(repo, '.local', 'pruned-content', mod, rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy2(src, dst)
            try:
                os.remove(src)
            except OSError:
                print('  could not remove', rel)
                return 0
            return 1
        for (ns, name) in bad:
            for rel in [f'assets/{ns}/items/{name}.json', f'data/{ns}/horizon/item/{name}.json', f'data/{ns}/horizon/block/{name}.json',
                        f'assets/{ns}/blockstates/{name}.json', f'data/{ns}/loot_table/blocks/{name}.json', f'data/{ns}/loot_tables/blocks/{name}.json']:
                moved += move(rel)
        def mentions(node):
            if isinstance(node, str):
                return node in ids or node.lstrip('#') in ids
            if isinstance(node, dict):
                return any(mentions(v) for v in node.values())
            if isinstance(node, list):
                return any(mentions(v) for v in node)
            return False
        data = os.path.join(base, 'data')
        for dirpath, _, files in os.walk(data):
            rel_dir = os.path.relpath(dirpath, base).replace(os.sep, '/')
            parts = rel_dir.split('/')
            for f in files:
                if not f.endswith('.json'):
                    continue
                path = os.path.join(dirpath, f)
                try:
                    j = json.load(open(path, encoding='utf-8-sig'))
                except Exception:
                    continue
                if 'tags' in parts:
                    # A tag keeps its other entries.
                    values = j.get('values') if isinstance(j, dict) else None
                    if isinstance(values, list):
                        kept = [v for v in values if not ((v if isinstance(v, str) else (v.get('id') if isinstance(v, dict) else None)) in ids)]
                        if len(kept) != len(values):
                            dst = os.path.join(repo, '.local', 'pruned-content', mod, rel_dir, f)
                            os.makedirs(os.path.dirname(dst), exist_ok=True)
                            if not os.path.exists(dst):
                                shutil.copy2(path, dst)
                            j['values'] = kept
                            json.dump(j, open(path, 'w', encoding='utf-8'), indent=2)
                            trimmed += 1
                elif any(k in parts for k in ('recipe', 'recipes', 'advancement', 'advancements')) and mentions(j):
                    moved += move(rel_dir + '/' + f)
        print(f'  moved {moved} files to .local/pruned-content/{mod}, trimmed {trimmed} tags')
print('total', total)
