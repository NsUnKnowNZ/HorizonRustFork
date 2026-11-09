"""Applies the enabled patches of .local/patches to an exported source tree.
usage: apply_patches.py SOURCE PATCHES_DIR
Each line of PATCHES_DIR/enabled.txt names a folder holding <name>.patch (a unified diff,
paths a/... b/...). Line endings are ignored; hunks are found by their old lines, nearest
to where the diff says. Any hunk that does not apply fails the build."""
import os, re, sys

HUNK = re.compile(r'^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@')


def parse(text):
    files, cur = [], None
    lines = text.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith('diff --git '):
            cur = {'path': line.split(' b/', 1)[1], 'new': False, 'hunks': []}
            files.append(cur)
        elif line.startswith('new file mode'):
            cur['new'] = True
        elif line.startswith('@@'):
            m = HUNK.match(line)
            hunk = {'start': int(m.group(1)), 'old': [], 'new': []}
            cur['hunks'].append(hunk)
            i += 1
            while i < len(lines) and lines[i][:1] in (' ', '-', '+', '\\') and not lines[i].startswith('diff --git '):
                l = lines[i]
                if l.startswith('\\'):
                    pass
                elif l[0] == ' ':
                    hunk['old'].append(l[1:]); hunk['new'].append(l[1:])
                elif l[0] == '-':
                    hunk['old'].append(l[1:])
                else:
                    hunk['new'].append(l[1:])
                i += 1
            continue
        i += 1
    return files


def apply_file(source, f):
    path = os.path.join(source, f['path'])
    if f['new']:
        content = []
    else:
        with open(path, encoding='utf-8') as h:
            content = h.read().replace('\r\n', '\n').split('\n')
    offset = 0
    for hunk in f['hunks']:
        old, new = hunk['old'], hunk['new']
        want = max(hunk['start'] - 1 + offset, 0)
        if not old:
            at = want if f['new'] else min(want, len(content))
        else:
            spots = [k for k in range(len(content) - len(old) + 1) if content[k:k + len(old)] == old]
            if not spots:
                sys.exit(f'FAILED: a hunk of {f["path"]} near line {hunk["start"]} does not apply')
            at = min(spots, key=lambda k: abs(k - want))
        content[at:at + len(old)] = new
        offset += len(new) - len(old)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    text = '\n'.join(content)
    if f['new'] and not text.endswith('\n'):
        text += '\n'
    with open(path, 'w', encoding='utf-8', newline='\n') as h:
        h.write(text)


def main():
    source, patches = sys.argv[1], sys.argv[2]
    enabled = os.path.join(patches, 'enabled.txt')
    if not os.path.exists(enabled):
        return
    for name in open(enabled, encoding='utf-8').read().split():
        if name.startswith('#'):
            continue
        p = os.path.join(patches, name, name + '.patch')
        files = parse(open(p, encoding='utf-8').read())
        for f in files:
            apply_file(source, f)
        print(f'patch {name}: {len(files)} files')


main()
