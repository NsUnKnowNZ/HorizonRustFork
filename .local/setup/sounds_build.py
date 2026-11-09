# Builds the client content mod horizon-mods/sounds from the Sounds mod's jar (IMB11, ARR: local
# use only, never shared): its sound files and sounds.json, and its block sound groups
# (assets/minecraft/sounds/blocks), which Horizon applies to vanilla's blocks. The jar is fetched
# from Modrinth once into .local/ports/better-oneblock/jars.
import json, os, sys, urllib.request, zipfile, shutil
root = sys.argv[1]
jars = os.path.join(root, ".local/ports/better-oneblock/jars")
jar = os.path.join(jars, "sounds-2.4.22.jar")
ua = {"User-Agent": "horizon-port-pack"}
if not os.path.exists(jar):
    api = "https://api.modrinth.com/v2/project/sound/version?game_versions=%5B%221.21.1%22%5D&loaders=%5B%22fabric%22%5D"
    versions = json.load(urllib.request.urlopen(urllib.request.Request(api, headers=ua)))
    v = next(v for v in versions if v["version_number"].startswith("2.4.22"))
    with urllib.request.urlopen(urllib.request.Request(v["files"][0]["url"], headers=ua)) as r, open(jar, "wb") as f:
        shutil.copyfileobj(r, f)
out = os.path.join(root, "horizon-mods/sounds")
shutil.rmtree(out, ignore_errors=True)
n = 0
with zipfile.ZipFile(jar) as z:
    for name in z.namelist():
        keep = name.startswith("assets/sounds/") or name.startswith("assets/minecraft/sounds/blocks/")
        if not keep or name.endswith("/"):
            continue
        dst = os.path.join(out, name)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        with open(dst, "wb") as f:
            f.write(z.read(name))
        n += 1
with open(os.path.join(out, "horizon-mod.toml"), "w") as f:
    f.write('# Carried over from the Sounds mod 2.4.22 (IMB11) by .local/setup/sounds_build.py, for local\n'
            '# use only (All Rights Reserved: never share this folder).\n'
            'id = "sounds"\nname = "Sounds"\nversion = "2.4.22"\napi = "0.2"\nside = "client"\n'
            'capabilities = ["registries"]\n')
print(n, "files")
