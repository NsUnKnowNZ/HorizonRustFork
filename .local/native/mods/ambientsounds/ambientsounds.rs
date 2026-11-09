// AmbientSounds for Horizon: regions weighed from where the player stands, and their sounds.
use super::{Dim, Game};
use horizon_core::position::BlockPos;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static SEED: AtomicU64 = AtomicU64::new(0x9FB2_1C65_1E98_DF25);

fn rand() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// The mod's data, read once.
struct Data {
    regions: Vec<Value>,
    categories: HashMap<String, Value>,
    collections: HashMap<String, Vec<Value>>,
    /// Sound event (`animals.owl`) to its length in ticks.
    lengths: HashMap<String, u32>,
    /// Features (`tree`, `sand`...) and the block groups they count.
    features: HashMap<String, Value>,
    groups: HashMap<String, Vec<String>>,
}

/// What the player stands in, weighed every two seconds.
#[derive(Default, Clone)]
struct Env {
    biome: String,
    temperature: f64,
    sky_light: f64,
    sky: f64,
    relative_height: f64,
    absolute_height: f64,
    underwater: f64,
    raining: bool,
    overall_raining: bool,
    storming: bool,
    hour: f64,
    /// Each feature's volume, from the blocks around.
    features: Option<HashMap<String, f64>>,
    /// The nearest mob of each type (`warden`), in blocks.
    entities: Option<HashMap<String, f64>>,
}

/// One sound of a region: its volume now, and ticks to its next start.
#[derive(Default)]
struct Playing {
    volume: f64,
    timer: u32,
    current: Option<String>,
}

struct State {
    data: Option<Data>,
    tried: bool,
    clock: u32,
    env: Env,
    /// Region name to volume.
    regions: Option<HashMap<String, f64>>,
    playing: Option<HashMap<String, Playing>>,
}

static STATE: Mutex<State> = Mutex::new(State {
    data: None,
    tried: false,
    clock: 0,
    env: Env {
        biome: String::new(),
        temperature: 0.0,
        sky_light: 0.0,
        sky: 0.0,
        relative_height: 0.0,
        absolute_height: 0.0,
        underwater: 0.0,
        raining: false,
        overall_raining: false,
        storming: false,
        hour: 0.0,
        features: None,
        entities: None,
    },
    regions: None,
    playing: None,
});

fn read_dir(dir: &std::path::Path) -> Vec<Value> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if let Some(v) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            out.push(v);
        }
    }
    out
}

fn load(game: &Game<'_>) -> Option<Data> {
    let root = game.native_instance_dir().join("horizon-mods/ambientsounds/assets/ambientsounds");
    let basic = root.join("basic");
    let regions = read_dir(&basic.join("regions"));
    if regions.is_empty() {
        return None;
    }
    let named = |dir: &str| -> HashMap<String, Value> {
        let mut out = HashMap::new();
        let Ok(entries) = std::fs::read_dir(basic.join(dir)) else { return out };
        for e in entries.flatten() {
            let name = e.path().file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if let Some(v) = std::fs::read(e.path()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                out.insert(name, v);
            }
        }
        out
    };
    let categories = named("sound_categories");
    let collections = named("sound_collections")
        .into_iter()
        .map(|(k, v)| {
            let list = v.get("sounds").and_then(Value::as_array).cloned().or_else(|| v.as_array().cloned()).unwrap_or_default();
            (k, list)
        })
        .collect();
    // Events to files (sounds.json), files to lengths (durations.json).
    let files: HashMap<String, u32> = std::fs::read(root.join("durations.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let sounds: HashMap<String, Value> = std::fs::read(root.join("sounds.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let lengths = sounds
        .iter()
        .filter_map(|(event, v)| {
            let name = v["sounds"].get(0).and_then(|s| s["name"].as_str().or(s.as_str()))?;
            let file = name.strip_prefix("ambientsounds:")?;
            Some((event.clone(), *files.get(file)?))
        })
        .collect();
    let features = named("features");
    let groups = named("blockgroups")
        .into_iter()
        .map(|(k, v)| (k, v.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()))
        .collect();
    Some(Data { regions, categories, collections, lengths, features, groups })
}

/// A range condition (`{"min": 1, "max": 5, "fade": 2}`) as a volume from 0 to 1.
fn range(cond: &Value, v: f64) -> f64 {
    let fade = cond["fade"].as_f64().unwrap_or(0.0);
    let mut out: f64 = 1.0;
    if let Some(min) = cond["min"].as_f64() {
        out *= if v >= min { 1.0 } else if fade > 0.0 { ((v - (min - fade)) / fade).clamp(0.0, 1.0) } else { 0.0 };
    }
    if let Some(max) = cond["max"].as_f64() {
        out *= if v <= max { 1.0 } else if fade > 0.0 { (((max + fade) - v) / fade).clamp(0.0, 1.0) } else { 0.0 };
    }
    out
}

/// A biome pattern (`forest`, `mushroom*fields`, `#is_ocean`, `lush hills`) against the biome.
fn biome_matches(pattern: &str, biome: &str) -> bool {
    let p = pattern.trim_start_matches('#').trim_start_matches("is_").to_lowercase().replace(' ', "_");
    let b = biome.rsplit(':').next().unwrap_or(biome);
    if p.contains('*') {
        let parts: Vec<&str> = p.split('*').collect();
        let mut at = 0;
        for part in parts {
            match b[at..].find(part) {
                Some(i) => at += i + part.len(),
                None => return false,
            }
        }
        true
    } else {
        b.contains(&p)
    }
}

fn any_biome(list: &Value, biome: &str) -> bool {
    list.as_array().is_some_and(|l| l.iter().filter_map(Value::as_str).any(|p| biome_matches(p, biome)))
}

/// The time of day curve (`{"6": 0, "8": 1, ...}`) at `hour`, or `"day"`/`"night"`.
fn time_curve(curve: &Value, hour: f64) -> f64 {
    match curve {
        Value::String(s) if s == "day" => if (6.0..18.0).contains(&hour) { 1.0 } else { 0.0 },
        Value::String(s) if s == "night" => if (6.0..18.0).contains(&hour) { 0.0 } else { 1.0 },
        Value::Object(map) => {
            let mut points: Vec<(f64, f64)> = map
                .iter()
                .filter_map(|(k, v)| Some((k.parse::<f64>().ok()?, v.as_f64()?)))
                .collect();
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            if points.is_empty() {
                return 1.0;
            }
            if hour <= points[0].0 {
                return points[0].1;
            }
            for w in points.windows(2) {
                if hour <= w[1].0 {
                    let t = (hour - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                    return w[0].1 + (w[1].1 - w[0].1) * t;
                }
            }
            points.last().map_or(1.0, |p| p.1)
        }
        _ => 1.0,
    }
}

/// The conditions an object (a region, a variant, a sound) puts on the place, as a volume.
fn conditions(o: &Value, env: &Env, regions: &HashMap<String, f64>) -> f64 {
    let mut v = 1.0;
    let ranged = [
        ("temperature", env.temperature),
        ("sky-light", env.sky_light),
        ("sky", env.sky),
        ("relative-height", env.relative_height),
        ("absolute-height", env.absolute_height),
        ("min-height-relative", env.relative_height),
        ("air", env.sky),
    ];
    for (key, value) in ranged {
        if let Some(c) = o.get(key) {
            v *= if key == "min-height-relative" { range(&serde_json::json!({ "min": c }), value) } else { range(c, value) };
        }
    }
    if let Some(c) = o.get("underwater") {
        v *= if c.is_boolean() { f64::from(u8::from(c.as_bool() == Some(env.underwater > 0.0))) } else { range(c, env.underwater) };
    }
    for (key, now) in [("raining", env.raining), ("overall-raining", env.overall_raining), ("storming", env.storming)] {
        if let Some(want) = o.get(key).and_then(Value::as_bool) && want != now {
            return 0.0;
        }
    }
    // `day` and `night`: a volume by day and by night (true 1, false 0); both, the day's by day
    // and the night's by night.
    let daylight = time_curve(&Value::String("day".into()), env.hour);
    let part = |key: &str| o.get(key).and_then(|x| x.as_f64().or_else(|| x.as_bool().map(|b| f64::from(u8::from(b)))));
    match (part("day"), part("night")) {
        (None, None) => {}
        (d, n) => {
            let day = d.unwrap_or(if n.is_some() { 1.0 - n.unwrap_or(0.0).min(1.0) } else { 1.0 });
            let night = n.unwrap_or(if d.is_some() { 1.0 - d.unwrap_or(0.0).min(1.0) } else { 1.0 });
            let (day, night) = match (d, n) {
                (Some(d), Some(n)) => (d, n),
                _ => (day, night),
            };
            v *= day * daylight + night * (1.0 - daylight);
        }
    }
    // `entity`: a mob of a type within a distance (a warden near).
    if let Some(e) = o.get("entity") {
        let kind = e["type"].as_str().unwrap_or("");
        let kind = kind.rsplit(':').next().unwrap_or(kind);
        let d = env.entities.as_ref().and_then(|m| m.get(kind)).copied().unwrap_or(f64::MAX);
        v *= match e.get("distance") {
            Some(c) => range(c, d),
            None => f64::from(u8::from(d < f64::MAX)),
        };
    }
    if let Some(c) = o.get("time") {
        v *= time_curve(c, env.hour);
    }
    if let Some(list) = o.get("features").and_then(Value::as_array) {
        for f in list.iter().filter_map(Value::as_str) {
            v *= env.features.as_ref().and_then(|m| m.get(f)).copied().unwrap_or(0.0);
        }
    }
    if let Some(list) = o.get("biomes") && !any_biome(list, &env.biome) {
        return 0.0;
    }
    if let Some(list) = o.get("bad-biomes") && any_biome(list, &env.biome) {
        return 0.0;
    }
    if let Some(list) = o.get("regions").and_then(Value::as_array) {
        for r in list.iter().filter_map(Value::as_str) {
            v *= regions.get(r).copied().unwrap_or(0.0);
        }
    }
    if let Some(list) = o.get("bad-regions").and_then(Value::as_array) {
        for r in list.iter().filter_map(Value::as_str) {
            v *= 1.0 - regions.get(r).copied().unwrap_or(0.0);
        }
    }
    v
}

/// A region's volume: its own conditions, then the best of its variants (if any).
fn region_volume(r: &Value, env: &Env, regions: &HashMap<String, f64>) -> f64 {
    let mut base = r.clone();
    // A region's biomes and its variants are alternatives: either may hold it.
    let variants = base.get("variants").and_then(Value::as_array).cloned();
    let biomes = base.get("biomes").cloned();
    if let Some(map) = base.as_object_mut() {
        map.remove("variants");
        if variants.is_some() {
            map.remove("biomes");
        }
        map.remove("sounds");
    }
    let own = conditions(&base, env, regions);
    let Some(variants) = variants else { return own };
    let from_biomes = biomes.map_or(0.0, |b| f64::from(u8::from(any_biome(&b, &env.biome))));
    let best = variants
        .iter()
        .map(|var| conditions(var, env, regions) * var["volume"].as_f64().unwrap_or(1.0))
        .fold(from_biomes, f64::max);
    own * best
}

fn weigh(game: &Game<'_>) -> Option<Env> {
    let p = game.player.as_ref()?;
    let (x, y, z) = (p.position.x, p.position.y + 1.62, p.position.z);
    let (bx, by, bz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let chunk = game.world.chunks.get(&(bx >> 4, bz >> 4))?;
    let biomes = world_biomes!(game);
    let info = biomes.get(chunk.biome(bx, by, bz));
    let ground = super::render_tick::motion_blocking_height(&game.world, &game.pack.assets.properties, bx, bz);
    let sky_light = f64::from(game.world.light(bx, by, bz) >> 4);
    let open = f64::from(ground) <= y;
    let (rain, thunder) = game.view_weather(0.0);
    let snows = info.snows_at(bx, by, bz);
    let mut underwater = 0.0;
    if game.eye_in_water(x, y, z) {
        let mut d = 1.0;
        while d < 32.0 && game.eye_in_water(x, y + d, z) {
            d += 1.0;
        }
        underwater = d;
    }
    let t = game.world.day_time.rem_euclid(24000) as f64;
    Some(Env {
        biome: info.name.clone(),
        temperature: f64::from(info.temperature_at(BlockPos::new(bx, by, bz), 63)),
        sky_light,
        sky: if open { 1.0 } else { sky_light / 15.0 * 0.5 },
        relative_height: y - f64::from(ground),
        absolute_height: y,
        underwater,
        raining: rain > 0.2 && open && snows == Some(false),
        overall_raining: rain > 0.2,
        storming: thunder > 0.2,
        hour: (t / 1000.0 + 6.0) % 24.0,
        features: None,
        entities: Some({
            let mut near: HashMap<String, f64> = HashMap::new();
            for m in game.native_mobs(game.dim.id()) {
                let d = ((m.pos[0] - x).powi(2) + (m.pos[1] - y).powi(2) + (m.pos[2] - z).powi(2)).sqrt();
                let kind = m.kind.rsplit(':').next().unwrap_or(&m.kind).to_owned();
                let e = near.entry(kind).or_insert(f64::MAX);
                *e = e.min(d);
            }
            near
        }),
    })
}

/// Whether block `name` is in a group's entry (`minecraft:sand`, `#minecraft:logs`, a tag read
/// from its name: `logs` matches `oak_log`).
fn in_group(entry: &str, name: &str) -> bool {
    let entry = entry.split('[').next().unwrap_or(entry);
    if let Some(tag) = entry.strip_prefix('#') {
        let path = tag.rsplit(':').next().unwrap_or(tag);
        let stem = path.strip_suffix('s').unwrap_or(path);
        let block = name.rsplit(':').next().unwrap_or(name);
        match path {
            "base_stone_overworld" => matches!(block, "stone" | "granite" | "diorite" | "andesite" | "tuff" | "deepslate"),
            "logs" => block.ends_with("_log") || block.ends_with("_wood") || block.ends_with("_stem") || block.ends_with("_hyphae"),
            _ => block.contains(stem),
        }
    } else {
        entry == name
    }
}

/// The blocks around: 3000 random spots within 40 across and 20 up and down, counted per block
/// group, as the full scan's count (scaled) and share of the solid blocks.
fn scan(game: &Game<'_>, data: &Data) -> HashMap<String, f64> {
    let Some(p) = game.player.as_ref() else { return HashMap::new() };
    let (px, py, pz) = (p.position.x.floor() as i32, p.position.y.floor() as i32, p.position.z.floor() as i32);
    const SAMPLES: usize = 3000;
    let volume = 81.0 * 41.0 * 81.0;
    let mut cache: HashMap<u32, Vec<String>> = HashMap::new();
    let mut counts: HashMap<String, f64> = HashMap::new();
    let mut solid = 0.0f64;
    for _ in 0..SAMPLES {
        let x = px + (rand() * 81.0) as i32 - 40;
        let y = py + (rand() * 41.0) as i32 - 20;
        let z = pz + (rand() * 81.0) as i32 - 40;
        if !game.world.loaded(x, z) {
            continue;
        }
        let state = game.world.block(x, y, z);
        let groups = cache.entry(state.0).or_insert_with(|| {
            let Some(name) = game.native_state_block(state) else { return Vec::new() };
            data.groups
                .iter()
                .filter(|(_, entries)| entries.iter().any(|e| in_group(e, name)))
                .map(|(g, _)| g.clone())
                .collect()
        });
        if !game.res.facts().is_air(state) {
            solid += 1.0;
        }
        for g in groups.iter() {
            *counts.entry(g.clone()).or_default() += 1.0;
        }
    }
    let scale = volume / SAMPLES as f64;
    let solid = solid.max(1.0);
    let ramp = |v: f64, lo: f64, hi: f64| if hi > lo { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) } else if v >= lo { 1.0 } else { 0.0 };
    fn value(f: &Value, counts: &HashMap<String, f64>, scale: f64, solid: f64, ramp: &dyn Fn(f64, f64, f64) -> f64) -> f64 {
        let mut v = 1.0;
        if let Some(groups) = f.get("groups").and_then(Value::as_array) {
            let hits: f64 = groups.iter().filter_map(Value::as_str).map(|g| counts.get(g).copied().unwrap_or(0.0)).sum();
            let count = hits * scale;
            let low = f["low-count"].as_f64().unwrap_or(0.0);
            v *= match f["high-count"].as_f64() {
                Some(high) => ramp(count, low, high),
                None => f64::from(u8::from(count >= low)),
            };
            if let Some(lw) = f["low-weight"].as_f64() {
                let hw = f["high-weight"].as_f64().unwrap_or(lw);
                v *= ramp(hits / solid, lw, hw);
            }
        }
        if let Some(all) = f.get("and").and_then(Value::as_array) {
            for sub in all {
                v *= value(sub, counts, scale, solid, ramp);
            }
        }
        if let Some(any) = f.get("or").and_then(Value::as_array) {
            let best = any.iter().map(|sub| value(sub, counts, scale, solid, ramp)).fold(0.0, f64::max);
            v *= best;
        }
        v
    }
    data.features
        .iter()
        .map(|(name, f)| (name.clone(), value(f, &counts, scale, solid, &ramp)))
        .collect()
}

/// A sound category's curve, with its parents'.
fn category_volume(data: &Data, name: Option<&str>, hour: f64) -> f64 {
    let mut v = 1.0;
    let mut next = name.map(str::to_owned);
    for _ in 0..4 {
        let Some(n) = next.take() else { break };
        let Some(c) = data.categories.get(&n) else { break };
        if let Some(t) = c.get("time") {
            v *= time_curve(t, hour);
        }
        next = c["parent"].as_str().map(str::to_owned);
    }
    v
}

pub(super) fn tick(game: &mut Game<'_>) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = &mut *guard;
    let playing = s.playing.get_or_insert_with(HashMap::new);
    let _ = playing;
    if s.regions.is_none() {
        s.regions = Some(HashMap::new());
    }
    if !s.tried {
        s.tried = true;
        s.data = load(game);
        match &s.data {
            Some(d) => log::info!("AmbientSounds: {} regions, {} sounds", d.regions.len(), d.lengths.len()),
            None => log::warn!("AmbientSounds: horizon-mods/ambientsounds isn't there; no ambience"),
        }
    }
    if s.data.is_none() || game.title.is_some() || game.player.is_none() {
        return;
    }
    let mut starts: Vec<(String, f32)> = Vec::new();
    let mut stops: Vec<String> = Vec::new();
    if game.dim != Dim::Overworld {
        for (_, p) in s.playing.as_mut().expect("set").iter_mut() {
            if let Some(c) = p.current.take() {
                stops.push(c);
            }
            p.volume = 0.0;
        }
    } else {
        s.clock = s.clock.wrapping_add(1);
        if s.clock % 40 == 1 && let Some(mut env) = weigh(game) {
            env.features = Some(scan(game, s.data.as_ref().expect("loaded")));
            s.env = env;
            // Twice, so a region's bad regions and required regions are weighed first.
            let data = s.data.as_ref().expect("loaded");
            let mut regions: HashMap<String, f64> = HashMap::new();
            for _ in 0..3 {
                let mut next = HashMap::new();
                for r in &data.regions {
                    if let Some(name) = r["name"].as_str() {
                        next.insert(name.to_owned(), region_volume(r, &s.env, &regions));
                    }
                }
                regions = next;
            }
            // Each sound's volume.
            let mut volumes: Vec<(String, f64)> = Vec::new();
            for r in &data.regions {
                let Some(name) = r["name"].as_str() else { continue };
                let rv = regions.get(name).copied().unwrap_or(0.0);
                let mut sounds: Vec<Value> = r["sounds"].as_array().cloned().unwrap_or_default();
                for c in r["sound-collections"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    sounds.extend(data.collections.get(c).cloned().unwrap_or_default());
                }
                for snd in &sounds {
                    let Some(sname) = snd["name"].as_str() else { continue };
                    let v = rv
                        * conditions(snd, &s.env, &regions)
                        * snd["volume"].as_f64().unwrap_or(1.0)
                        * category_volume(data, snd["category"].as_str(), s.env.hour);
                    volumes.push((format!("{name}/{sname}"), v));
                }
            }
            s.regions = Some(regions);
            for (key, v) in volumes {
                s.playing.as_mut().expect("set").entry(key).or_default().volume = v;
            }
        }
    }
    // Starting and stopping.
    let data = s.data.take().expect("loaded");
    let keys: Vec<String> = s.playing.as_ref().expect("set").keys().cloned().collect();
    for key in keys {
        let (region, sname) = key.split_once('/').unwrap_or((&key, ""));
        let p = s.playing.as_mut().expect("set").get_mut(&key).expect("present");
        if p.volume < 0.03 {
            if let Some(c) = p.current.take() {
                stops.push(c);
            }
            p.timer = 0;
            continue;
        }
        if p.timer > 0 {
            p.timer -= 1;
            continue;
        }
        // The sound's definition.
        let Some(r) = data.regions.iter().find(|r| r["name"].as_str() == Some(region)) else { continue };
        let mut all: Vec<&Value> = r["sounds"].as_array().into_iter().flatten().collect();
        for c in r["sound-collections"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            all.extend(data.collections.get(c).into_iter().flatten());
        }
        let Some(def) = all.into_iter().find(|d| d["name"].as_str() == Some(sname)) else { continue };
        let files: Vec<&str> = def["files"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if files.is_empty() {
            continue;
        }
        let file = files[(rand() * files.len() as f64) as usize % files.len()];
        let event = file.to_owned();
        let len = data.lengths.get(file.strip_prefix("ambientsounds:").unwrap_or(file)).copied().unwrap_or(200).max(20);
        let pause = def.get("pause").map_or(0, |pz| {
            let lo = pz["min"].as_f64().unwrap_or(0.0);
            let hi = pz["max"].as_f64().unwrap_or(lo);
            (lo + (hi - lo) * rand()) as u32
        });
        p.timer = len + pause;
        p.current = Some(event.clone());
        starts.push((event, p.volume.min(1.0) as f32));
    }
    s.data = Some(data);
    drop(guard);
    for event in stops {
        game.audio.stop_sounds(Some(&event), None);
    }
    for (event, volume) in starts {
        log::info!("AmbientSounds: {event} at {volume:.2}");
        game.audio.play(&event, None, volume, 1.0);
    }
}
