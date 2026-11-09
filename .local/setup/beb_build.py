# Builds the client content mod horizon-mods/beb from Beautiful Enchanted Books' jar (Apache-2.0):
# its book textures and models for the enchantments this game has, and an item definition for
# minecraft:enchanted_book that picks the book of the first enchantment it stores.
import json, os, sys, zipfile, shutil
root = sys.argv[1]
jar = os.path.join(root, ".local/ports/better-oneblock/jars/BEB-Fabric-1.21-6.0.0.jar")
out = os.path.join(root, "horizon-mods/beb")
shutil.rmtree(out, ignore_errors=True)
known = {"minecraft"} | {
    ns for m in os.listdir(os.path.join(root, "horizon-mods"))
    for ns in (os.listdir(os.path.join(root, "horizon-mods", m, "data")) if os.path.isdir(os.path.join(root, "horizon-mods", m, "data")) else [])
    if os.path.isdir(os.path.join(root, "horizon-mods", m, "data", ns, "enchantment"))
}
def exists(ns, name):
    if ns == "minecraft":
        return True
    return any(os.path.isfile(os.path.join(root, "horizon-mods", m, "data", ns, "enchantment", name + ".json"))
               for m in os.listdir(os.path.join(root, "horizon-mods")))
books = []
with zipfile.ZipFile(jar) as z:
    for n in sorted(z.namelist()):
        parts = n.split("/")
        if len(parts) == 6 and parts[0] == "assets" and parts[2] == "models" and parts[3] == "item" and parts[4] == "enchanted_book" and n.endswith(".json"):
            ns, name = parts[1], parts[5][:-5]
            if ns not in known or not exists(ns, name):
                continue
            tex = f"assets/{ns}/textures/item/enchanted_book/{name}.png"
            if tex not in z.namelist():
                continue
            for src in (n, tex):
                dst = os.path.join(out, src)
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                with open(dst, "wb") as f:
                    f.write(z.read(src))
            books.append((ns, name))
# Vanilla's books first (the jar lists them in name order), then the mods'.
books.sort(key=lambda b: (b[0] != "minecraft", b))
model = {"type": "minecraft:model", "model": "minecraft:item/enchanted_book"}
for ns, name in reversed(books):
    model = {
        "type": "minecraft:condition",
        "property": "minecraft:component",
        "predicate": "minecraft:stored_enchantments",
        "value": [{"enchantments": f"{ns}:{name}"}],
        "on_true": {"type": "minecraft:model", "model": f"{ns}:item/enchanted_book/{name}"},
        "on_false": model,
    }
items = os.path.join(out, "assets/minecraft/items")
os.makedirs(items, exist_ok=True)
with open(os.path.join(items, "enchanted_book.json"), "w") as f:
    json.dump({"model": model}, f, indent=1)
with open(os.path.join(out, "horizon-mod.toml"), "w") as f:
    f.write('# Carried over from BEB-Fabric-1.21-6.0.0.jar (Beautiful Enchanted Books, CerbonXD) by\n'
            '# .local/setup/beb_build.py, for local use only. Licence: Apache-2.0\n'
            'id = "beb"\nname = "Beautiful Enchanted Books"\nversion = "6.0.0"\napi = "0.2"\nside = "client"\ncapabilities = [\"registries\"]\n')
print(len(books), "books")
