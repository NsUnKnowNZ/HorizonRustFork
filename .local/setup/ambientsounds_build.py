# Builds the client content mod horizon-mods/ambientsounds from AmbientSounds' jar (CreativeMD,
# LGPL-3.0, local use): its sounds, sounds.json and region definitions, plus durations.json (each
# sound's length in ticks) for the native mod `ambientsounds`. The jar is fetched from Modrinth
# once into .local/ports/better-oneblock/jars.
import io, json, os, struct, sys, urllib.request, zipfile, shutil
root = sys.argv[1]
jars = os.path.join(root, ".local/ports/better-oneblock/jars")
jar = os.path.join(jars, "ambientsounds-6.3.11.jar")
if not os.path.exists(jar):
    api = "https://api.modrinth.com/v2/project/ambientsounds/version?game_versions=%5B%221.21.1%22%5D&loaders=%5B%22fabric%22%5D"
    req = urllib.request.Request(api, headers={"User-Agent": "horizon-port-pack"})
    versions = json.load(urllib.request.urlopen(req))
    v = next(v for v in versions if v["version_number"] == "6.3.11")
    url = v["files"][0]["url"]
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "horizon-port-pack"})) as r, open(jar, "wb") as f:
        shutil.copyfileobj(r, f)
out = os.path.join(root, "horizon-mods/ambientsounds")
shutil.rmtree(out, ignore_errors=True)

def ogg_seconds(data):
    # Sample rate from the Vorbis identification header, samples from the last page's granule.
    i = data.find(b"\x01vorbis")
    rate = struct.unpack_from("<I", data, i + 12)[0] if i >= 0 else 44100
    j = data.rfind(b"OggS")
    granule = struct.unpack_from("<q", data, j + 6)[0] if j >= 0 else 0
    return granule / rate if rate else 0.0

durations = {}
with zipfile.ZipFile(jar) as z:
    for n in z.namelist():
        if not n.startswith("assets/ambientsounds/") or n.endswith("/"):
            continue
        dst = os.path.join(out, n)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        data = z.read(n)
        with open(dst, "wb") as f:
            f.write(data)
        if n.endswith(".ogg"):
            key = n[len("assets/ambientsounds/sounds/"):-4]
            durations[key] = round(ogg_seconds(data) * 20)
    licence = z.read("LICENSE") if "LICENSE" in z.namelist() else b""
with open(os.path.join(out, "assets/ambientsounds/durations.json"), "w") as f:
    json.dump(durations, f, indent=0, sort_keys=True)
with open(os.path.join(out, "LICENSE"), "wb") as f:
    f.write(licence)
with open(os.path.join(out, "horizon-mod.toml"), "w") as f:
    f.write('# Carried over from AmbientSounds 6.3.11 (CreativeMD) by .local/setup/ambientsounds_build.py,\n'
            '# for local use only. Licence: LGPL-3.0\n'
            'id = "ambientsounds"\nname = "AmbientSounds"\nversion = "6.3.11"\napi = "0.2"\nside = "client"\n'
            'capabilities = ["registries"]\n')
print(len(durations), "sounds;", sum(durations.values()) // 20, "seconds")
