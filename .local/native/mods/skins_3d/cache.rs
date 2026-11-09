use super::umbx_skins3d_build::{self as build, Image, MAX_FACES};
use crate::mobs::{MobType, Part, Polygon};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

pub const PLAYER: &str = "minecraft:player";
pub const HAT: u8 = 1;
pub const JACKET_AND_SLEEVES: u8 = 2 | 4 | 8;
pub const PANTS: u8 = 16 | 32;
pub const ALL: u8 = HAT | JACKET_AND_SLEEVES | PANTS;
const LAYERS: [&str; 6] = ["hat", "jacket", "left_sleeve", "right_sleeve", "left_pants", "right_pants"];
const MAX_SKINS: usize = 96;
const MAX_WANTED: usize = 64;

struct Skin {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    models: HashMap<(bool, u8), Arc<Part>>,
}

struct Cache {
    skins: HashMap<String, Skin>,
    order: VecDeque<String>,
    wanted: HashSet<String>,
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache { skins: HashMap::new(), order: VecDeque::new(), wanted: HashSet::new() }));

fn cache() -> MutexGuard<'static, Cache> {
    CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn wanted() -> Vec<String> {
    cache().wanted.drain().collect()
}

pub fn offer(location: &str, width: usize, height: usize, rgba: &[u8]) {
    if Image::new(width, height, rgba).is_none() {
        return;
    }
    let mut cache = cache();
    if cache.skins.contains_key(location) {
        return;
    }
    while cache.order.len() >= MAX_SKINS {
        if let Some(old) = cache.order.pop_front() {
            cache.skins.remove(&old);
        }
    }
    cache.order.push_back(location.to_owned());
    cache.skins.insert(location.to_owned(), Skin { width, height, rgba: rgba.to_vec(), models: HashMap::new() });
}

fn layered(part: &mut Part, image: &Image, shown: u8, faces: &mut usize) {
    if let Some(bit) = LAYERS.iter().position(|layer| *layer == part.name)
        && shown & (1 << bit) != 0
    {
        let quads: Vec<build::Quad> = part.polygons.iter().map(|p| (p.normal, p.vertices)).collect();
        if let Some(built) = build::voxels(&quads, image, MAX_FACES) {
            *faces += built.len();
            part.polygons = built.into_iter().map(|(normal, vertices)| Polygon { normal, vertices, cull_back: false }).collect();
        }
    }
    for child in &mut part.children {
        layered(child, image, shown, faces);
    }
}

pub fn model(kind: &MobType, location: &str, slim: bool, shown: u8) -> Option<Arc<Part>> {
    if shown == 0 {
        return None;
    }
    let mut cache = cache();
    let Some(skin) = cache.skins.get_mut(location) else {
        if cache.wanted.len() < MAX_WANTED {
            cache.wanted.insert(location.to_owned());
        }
        return None;
    };
    if let Some(model) = skin.models.get(&(slim, shown)) {
        return Some(model.clone());
    }
    let image = Image::new(skin.width, skin.height, &skin.rgba)?;
    let key = if slim { "player_slim#main" } else { "main" };
    let mut part = kind.layer(key).map_or_else(|| (*kind.model).clone(), |model| (**model).clone());
    let mut faces = 0;
    layered(&mut part, &image, shown, &mut faces);
    if shown == ALL {
        log::info!("3D Skins: {location} ({}) has {faces} faces", if slim { "slim" } else { "wide" });
    }
    let model = Arc::new(part);
    skin.models.insert((slim, shown), model.clone());
    Some(model)
}
