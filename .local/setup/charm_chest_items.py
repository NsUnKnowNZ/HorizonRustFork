"""Charm's wood chests as items: the jar's item models are `builtin/entity` (drawn by Java code),
so they showed as nothing in slots. This writes each one's item definition as the game's own
special chest model with Charm's texture (`charm:<wood>_normal` / `_trapped`, under
textures/entity/chest/). Run again after a new carry of the charm content mod.
usage: python3 charm_chest_items.py"""
import json, os
repo = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
items = os.path.join(repo, 'horizon-mods', 'charm', 'assets', 'charm', 'items')
chests = os.path.join(repo, 'horizon-mods', 'charm', 'assets', 'charm', 'textures', 'entity', 'chest')
n = 0
for f in sorted(os.listdir(items)):
    name = f[:-5]
    if not name.endswith('_chest'):
        continue
    trapped = name.startswith('trapped_')
    wood = name[len('trapped_'):-len('_chest')] if trapped else name[:-len('_chest')]
    texture = f'{wood}_{"trapped" if trapped else "normal"}'
    if not os.path.isfile(os.path.join(chests, texture + '.png')):
        print('no texture for', name)
        continue
    definition = {'model': {'type': 'minecraft:special',
                            'base': 'minecraft:item/trapped_chest' if trapped else 'minecraft:item/chest',
                            'model': {'type': 'minecraft:chest', 'texture': f'charm:{texture}'}}}
    json.dump(definition, open(os.path.join(items, f), 'w', encoding='utf-8'), indent=2)
    n += 1
print(n, 'chest items written')
