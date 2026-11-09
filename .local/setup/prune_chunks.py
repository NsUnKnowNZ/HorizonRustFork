"""Prunes the chunks of a world nobody spent time in, so they generate again (as void, in a
OneBlock world). Keeps every chunk within RADIUS blocks of the centre, and any chunk whose
InhabitedTime (ticks players were near it) is at least MIN_TICKS. Backs the world up first.
usage: prune_chunks.py WORLD_DIR [RADIUS=64] [MIN_TICKS=2400] [--dry-run]"""
import os, re, shutil, struct, sys, time, zlib, gzip

def inhabited(raw):
    i = raw.find(b'\x04\x00\x0dInhabitedTime')
    if i < 0:
        return 0
    return struct.unpack('>q', raw[i + 16:i + 24])[0]

def chunk_bytes(data, offset, sectors):
    start = offset * 4096
    length = struct.unpack('>I', data[start:start + 4])[0]
    kind = data[start + 4]
    body = data[start + 5:start + 4 + length]
    if kind == 2:
        return zlib.decompress(body)
    if kind == 1:
        return gzip.decompress(body)
    if kind == 3:
        return body
    return b''

def prune(world, radius, min_ticks, dry):
    kept = removed = 0
    dims = []
    for root, dirs, files in os.walk(world):
        if os.path.basename(root) == 'region':
            dims.append(os.path.dirname(root))
    for dim in dims:
        for name in sorted(os.listdir(os.path.join(dim, 'region'))):
            m = re.match(r'r\.(-?\d+)\.(-?\d+)\.mca$', name)
            if not m:
                continue
            rx, rz = int(m.group(1)), int(m.group(2))
            path = os.path.join(dim, 'region', name)
            data = bytearray(open(path, 'rb').read())
            if len(data) < 8192:
                continue
            gone = []
            for i in range(1024):
                loc = struct.unpack('>I', data[i * 4:i * 4 + 4])[0]
                offset, sectors = loc >> 8, loc & 0xff
                if offset == 0:
                    continue
                cx, cz = rx * 32 + (i & 31), rz * 32 + (i >> 5)
                # The chunk's nearest point to the centre.
                nx = min(max(0, cx * 16), cx * 16 + 15)
                nz = min(max(0, cz * 16), cz * 16 + 15)
                near = nx * nx + nz * nz <= radius * radius
                try:
                    ticks = inhabited(chunk_bytes(data, offset, sectors))
                except Exception:
                    ticks = 0
                if near or ticks >= min_ticks:
                    kept += 1
                    continue
                gone.append(i)
            removed += len(gone)
            if dry or not gone:
                continue
            for i in gone:
                data[i * 4:i * 4 + 4] = b'\0\0\0\0'
                data[4096 + i * 4:4096 + i * 4 + 4] = b'\0\0\0\0'
            open(path, 'wb').write(data)
            # The same chunks' entities and points of interest go too.
            for extra in ('entities', 'poi'):
                other = os.path.join(dim, extra, name)
                if os.path.exists(other):
                    od = bytearray(open(other, 'rb').read())
                    if len(od) >= 8192:
                        for i in gone:
                            od[i * 4:i * 4 + 4] = b'\0\0\0\0'
                        open(other, 'wb').write(od)
    return kept, removed

def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    dry = '--dry-run' in sys.argv
    world = args[0]
    radius = int(args[1]) if len(args) > 1 else 64
    min_ticks = int(args[2]) if len(args) > 2 else 2400
    if not dry:
        backup = world.rstrip('/\\') + '-backup-' + time.strftime('%Y%m%d-%H%M%S')
        shutil.copytree(world, backup)
        print('backup:', backup)
    kept, removed = prune(world, radius, min_ticks, dry)
    print(f'{"would remove" if dry else "removed"} {removed} chunks, kept {kept}')

main()
