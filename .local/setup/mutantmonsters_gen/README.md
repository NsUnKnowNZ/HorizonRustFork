# Mutant Monsters model generator

Writes `.local/native/mods/mutantmonsters/models.rs` from the Mutant Monsters jar's model classes
(no decompiler: `javap -c` output, run symbolically).

1. Unzip the jar's `fuzs/` classes into `<work>/cls` (`unzip MutantMonsters-*.jar 'fuzs/*' -d <work>/cls`).
2. `cd <work> && python3 layers.py cls > layers.json` evaluates each `createBodyLayer` (the
   `LayerDefinition`: parts, cubes, poses, texture size).
3. `python3 gen.py` (from `<work>`, with `anim.py` beside it in `../mmtools/` or edit the path)
   translates each model's `setupAnim` into straight-line Rust over a pose array, with no attack
   animation under way (`getAnimation()` null), branching on the walk and head values where the
   Java does, and writes `generated.rs`: copy it to `models.rs`.

Needs a JDK's `javap`.
