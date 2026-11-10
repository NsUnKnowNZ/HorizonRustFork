"""Convert the 'One Block' data pack (Better OneBlock, MC 1.21.1) to Minecraft 26.3 for Horizon.

usage: python3 convert_oneblock.py <One Block.zip> <prepared 26.3 root> <rules.json> <out folder>
Local use only: the pack is All Rights Reserved; do not share the output.
"""
import json, os, re, shutil, sys, zipfile

src_zip, prepared, rules_path, out = sys.argv[1:5]
VANILLA = os.path.join(prepared, "data", "minecraft", "worldgen")
report = []

if os.path.exists(out):
    shutil.rmtree(out)
os.makedirs(out)
with zipfile.ZipFile(src_zip) as z:
    z.extractall(out)

# --- pack.mcmeta -----------------------------------------------------------
meta_p = os.path.join(out, "pack.mcmeta")
meta = json.load(open(meta_p, encoding="utf-8"))
meta["pack"].update(pack_format=121, min_format=121, max_format=121)
meta["pack"]["description"] = "OneBlock (Better OneBlock), converted to 26.3 for Horizon"
json.dump(meta, open(meta_p, "w", encoding="utf-8"), indent=2)

# --- game rules in functions ------------------------------------------------
rules = {old: (new, conv) for old, new, conv in json.load(open(rules_path))}
REMOVED = {"spawnChunkRadius"}  # no 26.3 equivalent (spawn chunks are gone)
rule_re = re.compile(r"\bgamerule ([A-Za-z]+)(?: (true|false|-?\d+))?")

def fix_rule(m):
    name, value = m.group(1), m.group(2)
    if name not in rules:
        return m.group(0)
    new, conv = rules[name]
    if value is not None and conv == "NotBool":
        value = {"true": "false", "false": "true"}[value]
    return f"gamerule minecraft:{new}" + (f" {value}" if value is not None else "")

changed = 0
for root, _, files in os.walk(os.path.join(out, "data")):
    for f in files:
        if not f.endswith(".mcfunction"):
            continue
        p = os.path.join(root, f)
        lines = open(p, encoding="utf-8").read().split("\n")
        new_lines = []
        for line in lines:
            if any(re.search(rf"\bgamerule {r}\b", line) for r in REMOVED):
                new_lines.append("# removed for 26.3: " + line)
                report.append(f"{os.path.relpath(p, out)}: commented out (rule gone in 26.3): {line.strip()}")
                continue
            line2 = rule_re.sub(fix_rule, line)
            # 26.3: the flash particle takes a colour (ColorParticleOption); white as before
            line2 = line2.replace("particle minecraft:flash ", "particle minecraft:flash{color:[1.0,1.0,1.0,1.0]} ")
            # 1.21.5+: text components in NBT are components, not JSON strings (text displays
            # showed the raw JSON).
            line2 = re.sub(r"text:'(\{.*?\})'(?=[,}])", lambda m: "text:" + m.group(1), line2)
            new_lines.append(line2)
        if new_lines != lines:
            changed += 1
            open(p, "w", encoding="utf-8").write("\n".join(new_lines))
report.append(f"game rules rewritten in {changed} function files")

# --- world generation: rebuild from vanilla 26.3 -------------------------
wg = os.path.join(out, "data", "minecraft", "worldgen")
old_features = set()
cf = os.path.join(wg, "configured_feature")
if os.path.isdir(cf):
    old_features = {f[:-5] for f in os.listdir(cf) if f.endswith(".json")}
shutil.rmtree(wg)
os.makedirs(wg)

# Overworld / Nether variants: pure air, no material rules (as the pack's 1.21.1 settings).
os.makedirs(os.path.join(wg, "noise_settings"))
for name in ["overworld", "amplified", "large_biomes", "caves", "floating_islands", "nether"]:
    d = json.load(open(os.path.join(VANILLA, "noise_settings", name + ".json")))
    d["default_block"] = "minecraft:air"
    d["default_fluid"] = "minecraft:air"
    # material_rule kept: Horizon reads it as a named rule only, and nothing is solid to apply it to
    d["noise_router"]["final_density"] = -1.0
    # 26.3 keeps aquifers as their own object: left in, they fill the void's bottom with lava.
    d.pop("aquifers", None)
    # Below min(-54, sea_level) the noise generator fills air with lava (createFluidPicker):
    # the sea at the bottom keeps the void empty.
    if d["noise"]["min_y"] < 0:
        d["sea_level"] = d["noise"]["min_y"]
    json.dump(d, open(os.path.join(wg, "noise_settings", name + ".json"), "w"), indent=2)

# Structures: none in the void Overworld (the pack spawns its own End portal); the Nether's
# fortresses and fossils and the End's cities stay. A random spread with frequency 0 never places.
os.makedirs(os.path.join(wg, "structure_set"), exist_ok=True)
for f in os.listdir(os.path.join(VANILLA, "structure_set")):
    if f[:-5] in ("end_cities", "nether_complexes", "nether_fossils"):
        continue
    d = json.load(open(os.path.join(VANILLA, "structure_set", f)))
    d["placement"] = {"type": "minecraft:random_spread", "salt": 1, "spacing": 64,
                      "separation": 32, "frequency": 0.0}
    json.dump(d, open(os.path.join(wg, "structure_set", f), "w"), indent=2)

# Biomes: every non-End biome loses its decorations and carvers (pack: "features": [], carvers empty).
os.makedirs(os.path.join(wg, "biome"))
END = {"the_end", "end_highlands", "end_midlands", "small_end_islands", "end_barrens"}
nb = 0
for f in os.listdir(os.path.join(VANILLA, "biome")):
    if not f.endswith(".json") or f[:-5] in END:
        continue
    d = json.load(open(os.path.join(VANILLA, "biome", f)))
    d["features"] = []
    d["carvers"] = []
    json.dump(d, open(os.path.join(wg, "biome", f), "w"), indent=2)
    nb += 1

# Features the pack switched off that still exist in 26.3 (End islands, chorus, gateways...).
vanilla_features = {f[:-5] for f in os.listdir(os.path.join(VANILLA, "feature"))}
os.makedirs(os.path.join(wg, "feature"))
kept = sorted(old_features & vanilla_features)
for n in kept:
    json.dump({"type": "minecraft:no_op"}, open(os.path.join(wg, "feature", n + ".json"), "w"))
report.append(f"worldgen: 6 void noise settings, {nb} biomes stripped, {len(kept)} features no-op "
              f"({len(old_features - vanilla_features)} old names renamed in 26.3, covered by stripped biomes)")

# --- Horizon: the progress bar for every player -----------------------------
# The pack turns the bar on, and enables its settings trigger, only for a player whose first join
# is in spectator: anyone joining in survival (a server's players, a player under a new name) had
# neither. Every player without a setting now gets it on; one who turns it off keeps it off.
fn = os.path.join(out, "data", "one-block", "function")
with open(os.path.join(fn, "tick.mcfunction"), "a", encoding="utf-8", newline="\n") as t:
    t.write("\n# Horizon: every player has the progress bar, on until they turn it off in the settings, and the settings\n"
            "# themselves (the pack gave both only to players who start in spectator: anyone joining in\n"
            "# survival, or under a new name, had neither).\n"
            "execute as @a[gamemode=!spectator] unless score @s ob-progressbar = @s ob-progressbar "
            "run function one-block:events/progressbar-default\n")
open(os.path.join(fn, "events", "progressbar-default.mcfunction"), "w", encoding="utf-8", newline="\n").write(
    "# Horizon: a player the pack has no progress bar setting for gets it on, and the settings trigger.\n"
    "scoreboard players set @s ob-progressbar 1\n"
    "scoreboard players enable @s ob-trigger\n")
report.append("progress bar: on for every player with no setting yet (the pack gave it to spectator starts only)")

open(os.path.join(out, "CONVERSION.txt"), "w", encoding="utf-8").write("\n".join(report) + "\n")
print("\n".join(report))
