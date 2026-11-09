// Mutant Monsters (Fuzs, AGPL-3.0, local use) for Horizon: the game half. The goals and the
// state they share are `horizon_client::mutantmonsters_ai` (the library module); here is what
// needs the world: each mutant's tick after its level ticked (`tick` in Java, `tickDeath`,
// the attacks' blocks, explosions, damage and spawns), its hooks (damage, death, the corpse
// lit with flint and steel), the synced state its clients draw from, the renderers, and the
// mod's items, spawn eggs, natural spawns and Chemical X.
use super::Game;
use super::mutantmonsters_models as models;
use super::native_block_entity_draw::{BlockEntityDraw, NativeModelPaint};
use super::native_explosions::NativeBlast;
use super::native_mob_draw::NativeMobRender;
use horizon_client::items::ItemStack;
use horizon_client::mobs::model_layers::ModelPart;
use horizon_client::mobs::{MobEvent, Who};
use horizon_client::mod_api::{AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host, Tag};
use horizon_client::mutantmonsters_ai::{self as ai, Kind, Mutant, anim};
use horizon_client::particles::ParticleType;
use horizon_core::random::RandomSource;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const NS: &str = "mutantmonsters:";
const HOSTILE: usize = 5;

// --- the synced state --------------------------------------------------------------

/// What a client needs to draw a mutant and what it saves (`Lives`, `Powered`): its
/// animation and tick, its own death, its flags and the model's extras.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Synced {
    anim: u8,
    tick: i32,
    death_time: i32,
    vanish: i32,
    lives: i32,
    /// `CHARGED`, `JUMPING`, `CHARGING`, `DYING`.
    flags: i32,
    jump_tick: i32,
    last_jump_tick: i32,
    e: [f32; 12],
    /// The blocks a mutant enderman holds, by arm ("" none).
    held: Vec<String>,
}

const CHARGED: i32 = 1;
const JUMPING: i32 = 2;
const CHARGING: i32 = 4;
const DYING: i32 = 8;

fn encode(s: &Synced) -> Result<Compound, AttachmentError> {
    let ints = vec![
        i32::from(s.anim),
        s.tick,
        s.death_time,
        s.vanish,
        s.lives,
        s.flags,
        s.jump_tick,
        s.last_jump_tick,
    ];
    let e = s.e.iter().map(|v| v.to_bits() as i32).collect();
    let held = s.held.iter().map(|b| Tag::String(b.as_str().into())).collect();
    Ok([("i".into(), Tag::IntArray(ints)), ("e".into(), Tag::IntArray(e)), ("h".into(), Tag::List(held))].into())
}

fn decode(data: &Compound) -> Result<Synced, AttachmentError> {
    let Some(Tag::IntArray(i)) = data.get(&"i".into()) else {
        return Err(AttachmentError::Decode);
    };
    let at = |n: usize| i.get(n).copied().unwrap_or(0);
    let mut e = [0.0; 12];
    if let Some(Tag::IntArray(bits)) = data.get(&"e".into()) {
        for (slot, b) in e.iter_mut().zip(bits) {
            *slot = f32::from_bits(*b as u32);
        }
    }
    let held = match data.get(&"h".into()) {
        Some(Tag::List(list)) => list
            .iter()
            .map(|t| match t {
                Tag::String(s) => s.to_string_lossy(),
                _ => String::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok(Synced {
        held,
        anim: at(0) as u8,
        tick: at(1),
        death_time: at(2),
        vanish: at(3),
        lives: at(4),
        flags: at(5),
        jump_tick: at(6),
        last_jump_tick: at(7),
        e,
    })
}

pub(super) static STATE: AttachmentType<Synced> = AttachmentType {
    namespace: "mutantmonsters",
    name: "state",
    default: Synced::default,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

fn synced(st: &Mutant) -> Synced {
    let flags = i32::from(st.charged) * CHARGED
        | i32::from(st.jump_attacking) * JUMPING
        | i32::from(st.charging) * CHARGING
        | i32::from(st.dying) * DYING;
    let mut e = st.e;
    e[0] = st.death_time as f32;
    e[1] = st.throw_hit_tick as f32;
    if st.kind == Kind::Enderman {
        e[3] = f32::from(st.active_arm);
        for i in 0..4 {
            e[5 + i] = if st.held[i].is_empty() { 0.0 } else { 1.0 };
        }
        e[10] = st.prev_arm_scale as f32;
        e[11] = st.arm_scale as f32;
    }
    e[9] = if st.throwing { 1.0 } else { 0.0 };
    let tick = if st.kind == Kind::SnowGolem { st.throwing_tick } else { st.anim_tick };
    Synced {
        anim: st.anim,
        tick,
        death_time: st.death_time,
        vanish: st.vanish,
        lives: st.lives,
        flags,
        jump_tick: st.jump_tick,
        last_jump_tick: st.last_jump_tick,
        e,
        held: if st.kind == Kind::Enderman { st.held.to_vec() } else { Vec::new() },
    }
}

// --- the client: what it was synced, and the renderers --------------------------------

/// Each mutant the client sees: what the server last synced.
static CLIENT: Mutex<Option<HashMap<u32, Synced>>> = Mutex::new(None);

/// Copies the synced state of the mutants in view for the renderers (`native_client_tick_post`).
pub(super) fn client_tick(game: &mut Game<'_>) {
    let ids: Vec<u32> = game.mobs.iter().filter(|m| m.kind.id.starts_with(NS)).map(|m| m.id).collect();
    let mut seen = HashMap::new();
    for id in ids {
        let host: &mut dyn Host = game;
        if let Ok(Some(s)) = STATE.get_existing(host, Holder::Entity(id)) {
            seen.insert(id, s);
        }
    }
    *CLIENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(seen);
}

type Anim = fn(&mut [[f32; 10]], f32, f32, f32, f32, f32, f32, u8, f32, &[f32; 12]);

/// How one mutant is drawn: its layer, texture, the parts its animations set.
struct Look {
    kind: &'static str,
    layer: &'static str,
    sprite: &'static str,
    /// Extra layers over the model: (texture, whether only the head draws, full bright).
    overlays: &'static [(&'static str, bool, bool)],
    /// `LivingEntityRenderer.scale`.
    scale: f32,
    /// `getFlipDegrees`: how far it tips over dying.
    flip: f32,
    parts: &'static [&'static str],
    anim: Anim,
}

const LOOKS: [Look; 7] = [
    Look {
        kind: ai::ZOMBIE,
        layer: "mutantmonsters:mutant_zombie#main",
        sprite: "mutantmonsters:entity/mutant_zombie",
        overlays: &[],
        scale: 1.3,
        flip: 80.0,
        parts: &models::MUTANT_ZOMBIE_PARTS,
        anim: models::anim_mutant_zombie,
    },
    Look {
        kind: ai::SKELETON,
        layer: "mutantmonsters:mutant_skeleton#main",
        sprite: "mutantmonsters:entity/mutant_skeleton",
        overlays: &[],
        scale: 1.0,
        flip: 0.0,
        parts: &models::MUTANT_SKELETON_PARTS,
        anim: models::anim_mutant_skeleton,
    },
    Look {
        kind: ai::CREEPER,
        layer: "mutantmonsters:mutant_creeper#main",
        sprite: "mutantmonsters:entity/mutant_creeper",
        overlays: &[],
        scale: 1.2,
        flip: 0.0,
        parts: &models::MUTANT_CREEPER_PARTS,
        anim: models::anim_mutant_creeper,
    },
    Look {
        kind: ai::ENDERMAN,
        layer: "mutantmonsters:mutant_enderman#main",
        sprite: "mutantmonsters:entity/mutant_enderman/mutant_enderman",
        overlays: &[("mutantmonsters:entity/mutant_enderman/eyes", false, true)],
        scale: 1.0,
        flip: 0.0,
        parts: &models::MUTANT_ENDERMAN_PARTS,
        anim: models::anim_mutant_enderman,
    },
    Look {
        kind: ai::SNOW_GOLEM,
        layer: "mutantmonsters:mutant_snow_golem#main",
        sprite: "mutantmonsters:entity/mutant_snow_golem/mutant_snow_golem",
        overlays: &[("mutantmonsters:entity/mutant_snow_golem/jack_o_lantern", true, false)],
        scale: 1.0,
        flip: 90.0,
        parts: &models::MUTANT_SNOW_GOLEM_PARTS,
        anim: models::anim_mutant_snow_golem,
    },
    Look {
        kind: ai::SPIDER_PIG,
        layer: "mutantmonsters:spider_pig#main",
        sprite: "mutantmonsters:entity/spider_pig/spider_pig",
        overlays: &[],
        scale: 1.2,
        flip: 180.0,
        parts: &models::SPIDER_PIG_PARTS,
        anim: models::anim_spider_pig,
    },
    Look {
        kind: ai::MINION,
        layer: "mutantmonsters:creeper_minion#main",
        sprite: "entity/creeper/creeper",
        overlays: &[],
        scale: 1.0,
        flip: 90.0,
        parts: &models::CREEPER_MINION_PARTS,
        anim: models::anim_creeper_minion,
    },
];

/// A baked model, the head layer's for a look with a head-only overlay, and the charged
/// creeper's armour.
struct Baked {
    tree: ModelPart,
    head: Option<ModelPart>,
    armor: Option<ModelPart>,
    /// The mutant skeleton's crossbow (`MutantCrossbowModel`) and its bolt (`MutantArrowModel`).
    crossbow: Option<ModelPart>,
    arrow: Option<ModelPart>,
}

static BAKED: Mutex<Option<HashMap<&'static str, Option<Baked>>>> = Mutex::new(None);

fn part_mut<'a>(root: &'a mut ModelPart, path: &str) -> Option<&'a mut ModelPart> {
    path.split('/').try_fold(root, |p, name| p.get_child_mut(name))
}

fn part<'a>(root: &'a ModelPart, path: &str) -> Option<&'a ModelPart> {
    path.split('/').try_fold(root, |p, name| p.get_child(name))
}

fn pose_of(p: &ModelPart) -> [f32; 10] {
    let i = p.initial_pose();
    [i.x, i.y, i.z, i.x_rot, i.y_rot, i.z_rot, i.x_scale, i.y_scale, i.z_scale, 1.0]
}

fn apply(root: &mut ModelPart, paths: &[&str], poses: &[[f32; 10]]) {
    for (path, v) in paths.iter().zip(poses) {
        if let Some(p) = part_mut(root, path) {
            (p.x, p.y, p.z) = (v[0], v[1], v[2]);
            (p.x_rot, p.y_rot, p.z_rot) = (v[3], v[4], v[5]);
            (p.x_scale, p.y_scale, p.z_scale) = (v[6], v[7], v[8]);
            p.visible = v[9] != 0.0;
        }
    }
}

fn initial(tree: &ModelPart, parts: &[&str]) -> Vec<[f32; 10]> {
    parts
        .iter()
        .map(|path| part(tree, path).map_or([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], pose_of))
        .collect()
}

/// Only the head draws (`setRenderHeadOnly`): every other part skips its own faces.
fn head_only(p: &mut ModelPart, path: &str) {
    let keep = path.ends_with("head") || path.contains("/head/");
    p.skip_draw = !keep;
    let names: Vec<String> = p.children().map(|(n, _)| n.to_owned()).collect();
    for name in names {
        if let Some(c) = p.get_child_mut(&name) {
            let sub = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
            head_only(c, &sub);
        }
    }
}

const FULL_BRIGHT: u32 = 240 | 240 << 8;

/// Draws a mutant with its own model (`native_mob_submit`), as its renderer does.
pub(super) fn submit(game: &Game<'_>, mob: &NativeMobRender<'_>, draw: &mut BlockEntityDraw, drawn: &mut bool) {
    if !mob.kind.starts_with(NS) {
        return;
    }
    let Some(look) = LOOKS.iter().find(|l| l.kind == mob.kind) else {
        return;
    };
    let s = CLIENT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|all| all.get(&mob.id).cloned())
        .unwrap_or_default();
    let kind = ai::kind_of(look.kind);
    let mut baked = BAKED.lock().unwrap_or_else(|e| e.into_inner());
    let baked = baked.get_or_insert_with(HashMap::new).entry(look.kind).or_insert_with(|| {
        let tree = game.native_bake_layer(look.layer)?;
        let head = look.overlays.iter().any(|o| o.1).then(|| {
            let layer = look.layer.replace("#main", "#head");
            game.native_bake_layer(&layer).map(|mut h| {
                head_only(&mut h, "");
                h
            })
        });
        let armor = game.native_bake_layer(&look.layer.replace("#main", "#armor"));
        let skeleton = look.kind == ai::SKELETON;
        let crossbow = skeleton.then(|| game.native_bake_layer("mutantmonsters:mutant_crossbow#main")).flatten();
        let arrow = skeleton.then(|| game.native_bake_layer("mutantmonsters:mutant_arrow#main")).flatten();
        Some(Baked { tree, head: head.flatten(), armor, crossbow, arrow })
    });
    let Some(model) = baked.as_mut() else {
        return;
    };
    let dying = s.flags & DYING != 0;
    let death_time = if dying { s.death_time } else { mob.death_time };
    let mut e = s.e;
    e[0] = death_time as f32;
    let pt = mob.partial;
    if kind == Some(Kind::Enderman) {
        e[4] = (e[10] + (e[11] - e[10]) * pt) / 10.0;
    }
    model.tree.reset_all_poses();
    let mut poses = initial(&model.tree, look.parts);
    let (ws, wa) = if dying { (0.0, 0.0) } else { (mob.walk_position, mob.walk_speed) };
    (look.anim)(&mut poses, ws, wa, mob.age, mob.head_yaw, mob.pitch, pt, s.anim, s.tick as f32, &e);
    apply(&mut model.tree, look.parts, &poses);
    if let Some(head) = model.head.as_mut() {
        head.reset_all_poses();
        apply(head, look.parts, &poses);
    }
    draw.push();
    // `getRenderOffset`, `setupRotations`, `scale`, the alpha and the overlay.
    let mut scale = look.scale;
    let mut alpha = 1.0;
    let mut white = 0.0;
    let mut red = mob.hurt_time > 0 || death_time > 0;
    match kind {
        Some(Kind::Zombie) => {
            draw.translate([0.0, -0.0975, 0.0]);
            if death_time > 0 {
                let reviving = death_time > 100;
                let pitch = if reviving { 140 - death_time } else { death_time.min(20) };
                if pitch > 0 {
                    let amount = if reviving {
                        (pitch as f32 - pt) / 40.0 * 1.6
                    } else {
                        (pitch as f32 + pt - 1.0) / 20.0 * 1.6
                    };
                    let amount = amount.max(0.0).sqrt().min(1.0);
                    // `Axis.NEGATIVE_X`: face down (the frame here is turned half round).
                    draw.rotate_about([1.0, 0.0, 0.0], amount * look.flip, [0.0, 0.0, 0.0]);
                }
            }
            if s.vanish > 0 {
                alpha = 1.0 - (s.vanish as f32 + pt) / 100.0 * 0.6;
            }
            red = mob.hurt_time > 0 || (death_time > 0 && s.lives <= 0);
        }
        Some(Kind::Creeper) => {
            if death_time > 0 {
                scale -= death_time as f32 / 100.0 * 0.4;
            }
            // MutantCreeper.getOverlayColor, flashing while it leaps.
            let charging = s.flags & CHARGING != 0;
            let jumping = s.flags & JUMPING != 0;
            let color = if death_time > 0 {
                death_time as f32 / 100.0
            } else if charging {
                if (mob.age as i32) % 20 < 10 { 0.6 } else { 0.0 }
            } else {
                (s.last_jump_tick as f32 + (s.jump_tick - s.last_jump_tick) as f32 * pt) / 28.0
            };
            white = if jumping && death_time == 0 {
                if (color * 10.0) as i32 % 2 == 0 { 0.0 } else { color.clamp(0.5, 1.0) }
            } else {
                color
            };
            red = mob.hurt_time > 0;
        }
        Some(Kind::Enderman) => {
            // getRenderOffset: it shakes while it stares, screams, smashes and dies.
            let shaking = matches!(s.anim, anim::ENDER_STARE | anim::ENDER_SCREAM | anim::ENDER_DEATH)
                || (s.anim == anim::ENDER_TELESMASH && s.tick < 18);
            if shaking {
                let shake = if s.anim == anim::ENDER_DEATH {
                    if s.tick < 80 { 0.02 } else { 0.05 }
                } else if s.tick >= 40 {
                    0.015
                } else {
                    0.03
                };
                let n = (mob.age * 7.31).sin() * 43758.545;
                let (gx, gz) = ((n.fract() - 0.5) * 2.0, ((n * 1.7).fract() - 0.5) * 2.0);
                draw.translate([gx * shake, 0.0, gz * shake]);
            }
            red = mob.hurt_time > 0;
        }
        Some(Kind::Minion) => {
            // CreeperMinionRenderer.scale and getWhiteOverlayProgress, from its fuse.
            let f = game
                .mobs
                .iter()
                .find(|m| m.id == mob.id)
                .map_or(0.0, |m| (m.previous_swell as f32 + (m.swell - m.previous_swell) as f32 * pt) / (m.max_swell - 2).max(1) as f32);
            let wobble = 1.0 + (f * 100.0).sin() * f * 0.01;
            let c = f.clamp(0.0, 1.0).powi(4);
            draw.scale([(1.0 + c * 0.4) * wobble, (1.0 + c * 0.1) / wobble, (1.0 + c * 0.4) * wobble]);
            white = if (f * 10.0) as i32 % 2 == 0 { 0.0 } else { f.clamp(0.5, 1.0) };
            if death_time > 0 {
                let t = ((death_time as f32 + pt - 1.0) / 20.0 * 1.6).max(0.0).sqrt().min(1.0);
                draw.rotate_about([0.0, 0.0, 1.0], -t * look.flip, [0.0, 0.0, 0.0]);
            }
        }
        _ => {
            if death_time > 0 && look.flip != 0.0 {
                let t = ((death_time as f32 + pt - 1.0) / 20.0 * 1.6).max(0.0).sqrt().min(1.0);
                draw.rotate_about([0.0, 0.0, 1.0], -t * look.flip, [0.0, 0.0, 0.0]);
            }
        }
    }
    draw.scale([scale, -scale, -scale]);
    draw.translate([0.0, -1.501, 0.0]);
    let overlay = horizon_client::overlay::living(white, red);
    let body = NativeModelPaint {
        color: ((alpha.clamp(0.0, 1.0) * 255.0) as u32) << 24 | 0xFF_FFFF,
        overlay,
        light: None,
        translucent: alpha < 1.0,
        swirl: None,
    };
    let ender_death = if kind == Some(Kind::Enderman) && death_time > 80 {
        Some(((death_time as f32 - 80.0) / 200.0).clamp(0.0, 1.0))
    } else {
        None
    };
    match ender_death {
        // hasAlternateRender: the body goes to its death texture.
        Some(p) => {
            let fade = |a: f32| NativeModelPaint { color: ((a * 255.0) as u32) << 24 | 0xFF_FFFF, translucent: true, ..body };
            game.native_draw_model_part_with(draw, &model.tree, "mutantmonsters:entity/mutant_enderman/death", &fade(p));
            game.native_draw_model_part_with(draw, &model.tree, look.sprite, &fade(1.0 - p));
        }
        // CreeperMinionModel.renderToBuffer: the head at full size, the body and legs at half.
        None if kind == Some(Kind::Minion) => {
            let show = |tree: &mut ModelPart, head: bool| {
                for name in ["head", "body", "leg1", "leg2", "leg3", "leg4"] {
                    if let Some(p) = tree.get_child_mut(name) {
                        p.visible = (name == "head") == head;
                    }
                }
            };
            show(&mut model.tree, true);
            draw.push();
            draw.translate([0.0, 0.5625, 0.0]);
            game.native_draw_model_part_with(draw, &model.tree, look.sprite, &body);
            draw.pop();
            show(&mut model.tree, false);
            draw.push();
            draw.scale([0.5, 0.5, 0.5]);
            draw.translate([0.0, 1.5, 0.0]);
            game.native_draw_model_part_with(draw, &model.tree, look.sprite, &body);
            draw.pop();
        }
        None => {
            game.native_draw_model_part_with(draw, &model.tree, look.sprite, &body);
        }
    }
    for &(sprite, head, bright) in look.overlays {
        let alpha = ender_death.map_or(alpha, |p| 1.0 - p);
        let paint = NativeModelPaint {
            color: ((alpha.clamp(0.0, 1.0) * 255.0) as u32) << 24 | 0xFF_FFFF,
            light: bright.then_some(FULL_BRIGHT),
            translucent: bright || alpha < 1.0,
            overlay: if bright { 0 } else { overlay },
            ..body
        };
        match (head, model.head.as_ref()) {
            (true, Some(h)) => game.native_draw_model_part_with(draw, h, sprite, &paint),
            (true, None) => false,
            (false, _) => game.native_draw_model_part_with(draw, &model.tree, sprite, &paint),
        };
    }
    match kind {
        Some(Kind::Skeleton) => skeleton_crossbow(game, draw, model, look, &poses, &s, &body),
        Some(Kind::Enderman) => ender_layers(game, draw, model, look, &s, mob.age, pt),
        Some(Kind::SnowGolem) => golem_layers(game, draw, model, &s, mob.age, pt),
        Some(Kind::SpiderPig) => {
            let saddled = game.mobs.iter().find(|m| m.id == mob.id).is_some_and(|m| m.extra.saddled);
            if saddled {
                game.native_draw_model_part_with(draw, &model.tree, "mutantmonsters:entity/spider_pig/saddle", &body);
            }
        }
        _ => {}
    }
    // CreeperChargeLayer: the armour model, scrolling, while charged.
    if kind == Some(Kind::Creeper)
        && s.flags & CHARGED != 0
        && let Some(armor) = model.armor.as_mut()
    {
        armor.reset_all_poses();
        let mut poses = initial(armor, look.parts);
        (look.anim)(&mut poses, ws, wa, mob.age, mob.head_yaw, mob.pitch, pt, s.anim, s.tick as f32, &e);
        apply(armor, look.parts, &poses);
        let t = mob.age * 0.01;
        let swirl = NativeModelPaint { color: 0xFF80_8080, overlay: 0, light: None, translucent: true, swirl: Some([t, t]) };
        game.native_draw_model_part_with(draw, armor, "entity/creeper/creeper_armor", &swirl);
    }
    draw.pop();
    *drawn = true;
}

/// The parts from the mutant skeleton's root to its right hand (`MutantSkeletonModel.translateHand`).
const SKELETON_HAND: [&str; 11] = [
    "base",
    "base/pelvis",
    "base/pelvis/waist",
    "base/pelvis/waist/middle1",
    "base/pelvis/waist/middle1/middle2",
    "base/pelvis/waist/middle1/middle2/middle3",
    "base/pelvis/waist/middle1/middle2/middle3/shoulder1",
    "base/pelvis/waist/middle1/middle2/middle3/shoulder1/arm1",
    "base/pelvis/waist/middle1/middle2/middle3/shoulder1/arm1/inner_arm1",
    "base/pelvis/waist/middle1/middle2/middle3/shoulder1/arm1/inner_arm1/fore_arm1",
    "base/pelvis/waist/middle1/middle2/middle3/shoulder1/arm1/inner_arm1/fore_arm1/inner_fore_arm1",
];

/// `ModelPart.translateAndRotate` of each part from the root to the hand.
fn translate_hand(draw: &mut BlockEntityDraw, tree: &ModelPart) {
    for path in SKELETON_HAND {
        let Some(p) = part(tree, path) else { return };
        draw.translate([p.x / 16.0, p.y / 16.0, p.z / 16.0]);
        draw.rotate_about([0.0, 0.0, 1.0], p.z_rot.to_degrees(), [0.0; 3]);
        draw.rotate_about([0.0, 1.0, 0.0], p.y_rot.to_degrees(), [0.0; 3]);
        draw.rotate_about([1.0, 0.0, 0.0], p.x_rot.to_degrees(), [0.0; 3]);
        draw.scale([p.x_scale, p.y_scale, p.z_scale]);
    }
}

/// `MutantSkeletonRenderer.CrossbowLayer`: the crossbow in its hand, and the bolt while it
/// loads a shot.
fn skeleton_crossbow(game: &Game<'_>, draw: &mut BlockEntityDraw, model: &mut Baked, look: &Look, poses: &[[f32; 10]], s: &Synced, paint: &NativeModelPaint) {
    let Some(crossbow) = model.crossbow.as_mut() else { return };
    crossbow.reset_all_poses();
    for (path, pose) in look.parts.iter().zip(poses) {
        if let Some(rest) = path.strip_prefix("EXT/") {
            apply(crossbow, &[rest], std::slice::from_ref(pose));
        }
    }
    draw.push();
    translate_hand(draw, &model.tree);
    game.native_draw_model_part_with(draw, crossbow, "mutantmonsters:entity/mutant_crossbow", paint);
    draw.pop();
    let loading = (s.anim == anim::SKELETON_SHOOT && s.tick > 10 && s.tick < 26)
        || (s.anim == anim::SKELETON_MULTI_SHOT && s.tick > 16 && s.tick < 24);
    if loading && let Some(arrow) = model.arrow.as_ref() {
        draw.push();
        translate_hand(draw, &model.tree);
        draw.translate([-0.2, 0.4, -1.8]);
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
        draw.scale([1.2, 1.2, 1.2]);
        game.native_draw_model_part_with(draw, arrow, "mutantmonsters:entity/mutant_arrow", &NativeModelPaint { overlay: 0, ..*paint });
        draw.pop();
    }
}

/// `ModelPart.translateAndRotate` along `paths` from the root.
fn translate_parts(draw: &mut BlockEntityDraw, tree: &ModelPart, paths: &[&str]) {
    for path in paths {
        let Some(p) = part(tree, path) else { return };
        draw.translate([p.x / 16.0, p.y / 16.0, p.z / 16.0]);
        draw.rotate_about([0.0, 0.0, 1.0], p.z_rot.to_degrees(), [0.0; 3]);
        draw.rotate_about([0.0, 1.0, 0.0], p.y_rot.to_degrees(), [0.0; 3]);
        draw.rotate_about([1.0, 0.0, 0.0], p.x_rot.to_degrees(), [0.0; 3]);
        draw.scale([p.x_scale, p.y_scale, p.z_scale]);
    }
}

/// `BlockRenderDispatcher.renderSingleBlock` at the pose, as an entity's layer draws a block.
fn draw_block_as_entity(game: &Game<'_>, draw: &mut BlockEntityDraw, block: &str) {
    draw.translate([-0.5, -0.5, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0; 3]);
    game.native_draw_block(&mut *draw, block, &[]);
}

/// `MutantEndermanRenderer`'s layers: the blocks in its hands (`HeldBlocksLayer`) and the
/// soul about it while it teleports or screams (`SoulLayer`).
fn ender_layers(game: &Game<'_>, draw: &mut BlockEntityDraw, model: &mut Baked, look: &Look, s: &Synced, age: f32, pt: f32) {
    const ARMS: [&str; 4] = ["right", "left", "lower_right", "lower_left"];
    if s.anim != anim::ENDER_CLONE {
        for (i, block) in s.held.iter().enumerate().take(4) {
            if block.is_empty() {
                continue;
            }
            let a = ARMS[i];
            let chest = "pelvis/abdomen/chest";
            let paths = [
                "pelvis".to_owned(),
                "pelvis/abdomen".to_owned(),
                chest.to_owned(),
                format!("{chest}/{a}_arm"),
                format!("{chest}/{a}_arm/{a}_fore_arm"),
                format!("{chest}/{a}_arm/{a}_fore_arm/{a}_hand"),
            ];
            let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
            draw.push();
            translate_parts(draw, &model.tree, &paths);
            draw.translate([0.0, 1.2, 0.0]);
            let tick = age.floor() + (i as f32 + 1.0) * 2.0 * std::f32::consts::PI + pt;
            draw.rotate_about([1.0, 0.0, 0.0], tick * 10.0, [0.0; 3]);
            draw.rotate_about([0.0, 1.0, 0.0], tick * 8.0, [0.0; 3]);
            draw.rotate_about([0.0, 0.0, 1.0], tick * 6.0, [0.0; 3]);
            draw.scale([-0.75, -0.75, 0.75]);
            draw_block_as_entity(game, draw, block);
            draw.pop();
        }
    }
    let teleport = s.anim == anim::ENDER_TELEPORT && s.tick < 10;
    let scream = s.anim == anim::ENDER_SCREAM;
    if teleport || scream {
        let t = s.tick as f32 + pt;
        let scale = if teleport {
            1.2 + t / 10.0
        } else if s.tick < 40 {
            1.2 + t / 40.0
        } else if s.tick < 160 {
            2.2
        } else {
            (2.2 - t / 10.0).max(0.0)
        };
        draw.push();
        draw.scale([scale, scale * 0.8, scale]);
        // EndersoulLayer: the soul texture over the model in (0.9, 0.3, 1.0) at the layer's alpha.
        let alpha = if teleport {
            if s.tick >= 8 { 1.0 - (t - 8.0) / 2.0 } else { 1.0 }
        } else if s.tick < 40 {
            t / 40.0
        } else if s.tick >= 160 {
            1.0 - t / 40.0
        } else {
            1.0
        };
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
        let swirl = NativeModelPaint { color: a << 24 | 0xE6_4CFF, overlay: 0, light: Some(FULL_BRIGHT), translucent: true, swirl: None };
        let _ = age;
        game.native_draw_model_part_with(draw, &model.tree, "mutantmonsters:entity/endersoul", &swirl);
        draw.pop();
    }
    let _ = look;
}

/// `MutantSnowGolemRenderer`'s layers: the jack o'lantern's glow over the whole model, and the
/// ice in its hand as it winds up a throw.
fn golem_layers(game: &Game<'_>, draw: &mut BlockEntityDraw, model: &mut Baked, s: &Synced, age: f32, pt: f32) {
    let green = (0.8 + 0.05 * (age * 0.15).cos()).max(0.0);
    let blue = (0.15 + 0.2 * (age * 0.1).cos()).max(0.0);
    let rgb = 0xFF << 16 | ((green * 255.0) as u32) << 8 | (blue * 255.0) as u32;
    let glow = NativeModelPaint { color: 0xFF00_0000 | rgb, overlay: 0, light: Some(FULL_BRIGHT), translucent: true, swirl: None };
    game.native_draw_model_part_with(draw, &model.tree, "mutantmonsters:entity/mutant_snow_golem/glow", &glow);
    if s.e[9] != 0.0 && s.tick < 7 {
        let scale = ((s.tick as f32 + pt) / 7.0).min(0.8);
        let arm = "pelvis/abdomen/chest/arm1";
        let paths = [
            "pelvis",
            "pelvis/abdomen",
            "pelvis/abdomen/chest",
            arm,
            "pelvis/abdomen/chest/arm1/inner_arm1",
            "pelvis/abdomen/chest/arm1/inner_arm1/fore_arm1",
            "pelvis/abdomen/chest/arm1/inner_arm1/fore_arm1/inner_fore_arm1",
        ];
        draw.push();
        draw.translate([0.4, 0.0, 0.0]);
        translate_parts(draw, &model.tree, &paths);
        draw.translate([0.0, 0.9, 0.0]);
        draw.scale([-scale, -scale, scale]);
        draw_block_as_entity(game, draw, "minecraft:ice");
        draw.pop();
    }
}

// --- the server half: items, spawns, Chemical X ----------------------------------------

static SEED: AtomicU64 = AtomicU64::new(0x2545_F491_4F6C_DD1D);

fn rand() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn rand_int(n: i32) -> i32 {
    ((rand() * f64::from(n)) as i32).min(n - 1)
}

fn rand_float() -> f32 {
    rand() as f32
}

/// What a vanilla mob becomes: on a natural spawn, sometimes; hit by Chemical X, always.
const MUTANTS: [(&str, &str); 6] = [
    ("minecraft:zombie", ai::ZOMBIE),
    ("minecraft:skeleton", ai::SKELETON),
    ("minecraft:creeper", ai::CREEPER),
    ("minecraft:enderman", ai::ENDERMAN),
    ("minecraft:snow_golem", ai::SNOW_GOLEM),
    ("minecraft:pig", ai::SPIDER_PIG),
];
/// The share of naturally spawned zombies, skeletons, creepers and endermen that are mutants.
const NATURAL_CHANCE: f64 = 0.012;

enum Job {
    /// A mob of `kind` at `pos` (a spawn egg).
    Spawn { dimension: String, kind: String, pos: [f64; 3] },
    /// The vanilla mob of `from` nearest `pos` becomes `to`.
    Mutate { dimension: String, from: String, to: String, pos: [f64; 3] },
    /// Every vanilla cousin within 4 blocks of `pos` mutates (Chemical X).
    Splash { dimension: String, pos: [f64; 3] },
}

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

fn push(job: Job) {
    JOBS.lock().unwrap_or_else(|e| e.into_inner()).push(job);
}

fn use_up_held(game: &mut Game<'_>) {
    if game.creative {
        return;
    }
    let cell = game.held_mut();
    if let Some(s) = cell.as_mut() {
        if s.count > 1 {
            s.count -= 1;
        } else {
            *cell = None;
        }
    }
    game.guest_inventory_resend = true;
}

/// A spawn egg on a block, or the hulk hammer's wave from it.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    let Some(name) = stack.item.strip_prefix(NS) else {
        return;
    };
    if let Some(mob) = name.strip_suffix("_spawn_egg") {
        if ai::kind_of(&format!("{NS}{mob}")).is_none() {
            return;
        }
        let at = [
            f64::from(pos[0] + face[0]) + 0.5,
            f64::from(pos[1] + face[1]),
            f64::from(pos[2] + face[2]) + 0.5,
        ];
        push(Job::Spawn { dimension: game.dim.id().to_owned(), kind: format!("{NS}{mob}"), pos: at });
        use_up_held(game);
        *claimed = true;
        return;
    }
    if name == "hulk_hammer" && face == [0, 1, 0] {
        *claimed = hammer_wave(game);
    }
}

/// Per player (acting id): the tick their item may be used again.
static COOLDOWNS: Mutex<Option<HashMap<(u128, &'static str), u64>>> = Mutex::new(None);

fn cooling(game: &mut Game<'_>, item: &'static str, ticks: u64) -> bool {
    let who = game.native_acting_id();
    let now = game.world.game_time as u64;
    let mut map = COOLDOWNS.lock().unwrap_or_else(|e| e.into_inner());
    let map = map.get_or_insert_with(HashMap::new);
    if map.get(&(who, item)).is_some_and(|&until| now < until) {
        return true;
    }
    map.insert((who, item), now + ticks);
    false
}

/// The mobs (not the acting player) within `radius` of `at`.
fn mobs_near(game: &Game<'_>, at: [f64; 3], radius: f64) -> Vec<(u32, [f64; 3])> {
    game.mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && &*m.kind.id != "minecraft:player")
        .map(|m| (m.id, [m.position.x, m.position.y, m.position.z]))
        .filter(|(_, p)| (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>() <= radius * radius)
        .collect()
}

/// `HulkHammerItem.useOn`: a wave along the ground the way the player faces, 8 blocks out,
/// that throws up and hurts the mobs in its path (cooldown 25 ticks, 1 durability).
fn hammer_wave(game: &mut Game<'_>) -> bool {
    let Some(player) = game.native_player() else {
        return false;
    };
    if cooling(game, "hulk_hammer", 25) {
        return true;
    }
    let dimension = player.dimension;
    let yaw = player.yaw.to_radians();
    let (dx, dz) = (-f64::from(yaw.sin()), f64::from(yaw.cos()));
    let me = Some(game.native_acting_entity()).filter(|&id| id != 0);
    let mut hit = Vec::new();
    for step in 1..=8 {
        let at = [player.x + dx * f64::from(step), player.y, player.z + dz * f64::from(step)];
        for (id, _) in mobs_near(game, at, 1.5) {
            if !hit.contains(&id) {
                hit.push(id);
            }
        }
    }
    for id in hit {
        game.native_hurt_entity(dimension, id, 8.0, "mutantmonsters:player_seismic_wave", me);
        game.native_push_mob(dimension, id, [dx * 0.4, 0.8, dz * 0.4]);
    }
    game.damage_held_tool();
    true
}

/// The creeper shard and the endersoul hand used in the air.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    let Some(name) = stack.item.strip_prefix(NS) else {
        return;
    };
    let Some(player) = game.native_player() else {
        return;
    };
    let dimension = player.dimension;
    match name {
        // `HulkHammerItem.useOn`, here from the air too.
        "hulk_hammer" => *claimed = hammer_wave(game),
        // `CreeperShardItem.use`: a burst that hurts and throws back what is within 5 blocks.
        "creeper_shard" => {
            *claimed = true;
            if cooling(game, "creeper_shard", 20) {
                return;
            }
            let at = [player.x, player.y + 1.0, player.z];
            let me = Some(game.native_acting_entity()).filter(|&id| id != 0);
            for (id, p) in mobs_near(game, at, 5.0) {
                let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
                let len = (dx * dx + dz * dz).sqrt().max(0.1);
                game.native_hurt_entity(dimension, id, 6.0, "minecraft:explosion", me);
                game.native_push_mob(dimension, id, [dx / len * 0.9, 0.4, dz / len * 0.9]);
            }
            game.damage_held_tool();
        }
        // `EndersoulHandItem.use`: teleports to the block looked at, up to 64 blocks away.
        "endersoul_hand" => {
            *claimed = true;
            if cooling(game, "endersoul_hand", 20) {
                return;
            }
            let (yaw, pitch) = (player.yaw.to_radians(), player.pitch.to_radians());
            let dir = [
                -f64::from(yaw.sin() * pitch.cos()),
                -f64::from(pitch.sin()),
                f64::from(yaw.cos() * pitch.cos()),
            ];
            let eye = [player.x, player.y + 1.62, player.z];
            let mut last = None;
            let mut t = 0.5;
            while t <= 64.0 {
                let p = [eye[0] + dir[0] * t, eye[1] + dir[1] * t, eye[2] + dir[2] * t];
                let block = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                if !is_air(game, block) {
                    last = Some(block);
                    break;
                }
                t += 0.25;
            }
            let Some(block) = last else {
                return;
            };
            // On top of the block hit, where two blocks of air stand above it.
            let mut y = block[1] + 1;
            while y < block[1] + 8
                && !(is_air(game, [block[0], y, block[2]]) && is_air(game, [block[0], y + 1, block[2]]))
            {
                y += 1;
            }
            let to = [f64::from(block[0]) + 0.5, f64::from(y), f64::from(block[2]) + 0.5];
            let who = game.native_acting_id();
            if game.native_teleport_player(who, dimension, to, player.yaw, player.pitch) {
                game.damage_held_tool();
            }
        }
        _ => {}
    }
}

fn is_air(game: &Game<'_>, pos: [i32; 3]) -> bool {
    matches!(game.native_block(pos).as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// A mob joined the world: a natural zombie, skeleton, creeper or enderman is sometimes a
/// mutant (the mod's spawn entries, here as a share of its cousins' spawns).
pub(super) fn spawned(dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    if !natural || !matches!(kind, "minecraft:zombie" | "minecraft:skeleton" | "minecraft:creeper" | "minecraft:enderman") {
        return;
    }
    if rand() >= NATURAL_CHANCE {
        return;
    }
    let Some((_, to)) = MUTANTS.iter().find(|(from, _)| *from == kind) else {
        return;
    };
    push(Job::Mutate { dimension: dimension.to_owned(), from: kind.to_owned(), to: (*to).to_owned(), pos });
}

/// Splash Chemical X broke at `at`.
pub(super) fn impact(dimension: &str, kind: &str, at: [f64; 3]) {
    if kind == "mutantmonsters:splash_chemical_x" {
        push(Job::Splash { dimension: dimension.to_owned(), pos: at });
    }
}

fn mutate(game: &mut Game<'_>, dimension: &str, id: u32, to: &str, at: [f64; 3]) {
    if game.native_spawn_mob(dimension, to, at, &Default::default()).is_some() {
        game.native_remove_mob(dimension, id);
    }
}

fn tick_jobs(game: &mut Game<'_>, dimension: &str) {
    let jobs = {
        let mut all = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        let (here, rest): (Vec<Job>, Vec<Job>) = std::mem::take(&mut *all).into_iter().partition(|j| match j {
            Job::Spawn { dimension: d, .. } | Job::Mutate { dimension: d, .. } | Job::Splash { dimension: d, .. } => d == dimension,
        });
        *all = rest;
        here
    };
    for job in jobs {
        match job {
            Job::Spawn { kind, pos, .. } => {
                game.native_spawn_mob(dimension, &kind, pos, &Default::default());
            }
            Job::Mutate { from, to, pos, .. } => {
                let nearest = game
                    .native_mobs(dimension)
                    .into_iter()
                    .filter(|m| m.kind == from)
                    .map(|m| (m.id, m.pos, (0..3).map(|i| (m.pos[i] - pos[i]).powi(2)).sum::<f64>()))
                    .filter(|m| m.2 < 4.0)
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((id, at, _)) = nearest {
                    mutate(game, dimension, id, &to, at);
                }
            }
            Job::Splash { pos, .. } => {
                for m in game.native_mobs(dimension) {
                    let near = (0..3).map(|i| (m.pos[i] - pos[i]).powi(2)).sum::<f64>() <= 16.0;
                    if let Some((_, to)) = MUTANTS.iter().find(|(from, _)| *from == m.kind).filter(|_| near) {
                        mutate(game, dimension, m.id, to, m.pos);
                    }
                }
            }
        }
    }
}

// --- the hooks a hit goes through ------------------------------------------------------

/// The zombies a mutant zombie never hurts and is never hurt by (`canHarm`), by id: kept
/// each tick for the damage hook, which has no game to look in.
static ZOMBIES: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
/// Set while one of the mutants' own blasts goes off (`MutatedExplosionHelper.explode`): a
/// mutant creeper is left out of it, and doesn't heal from it.
static OWN_BLAST: AtomicBool = AtomicBool::new(false);

/// What the damage hooks leave for the next level tick.
enum After {
    /// `MutantCreeper.hurt`: half an explosion's damage heals it.
    Heal { mob: u32, amount: f32 },
    /// Charging, it thorns whoever hits it.
    Thorns { mob: u32, attacker: u32 },
    /// `MutantEnderman.hurt`: it may teleport away (to its attacker), more likely from a hit
    /// with no one behind it.
    Dodge { mob: u32, attacker: Option<u32>, indirect: bool },
}
static AFTER: Mutex<Vec<After>> = Mutex::new(Vec::new());

fn is_skeleton(id: u32) -> bool {
    ai::with_id(id, |s| s.kind == Kind::Skeleton).unwrap_or(false)
}

fn is_zombie(id: u32) -> bool {
    ZOMBIES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|z| z.contains(&id))
}

/// Each mutant's `hurt` before the hit lands (`native_living_incoming_damage_v3`).
pub(super) fn incoming_damage(victim: u32, kind: &str, attacker: Option<u32>, damage_type: &str, amount: &mut f32, cancel: &mut bool) {
    let Some(k) = ai::kind_of(kind) else {
        return;
    };
    let kill = damage_type == "minecraft:generic_kill";
    let explosion = damage_type.contains("explosion") || damage_type == "minecraft:fireworks";
    let fire = matches!(
        damage_type,
        "minecraft:in_fire" | "minecraft:on_fire" | "minecraft:lava" | "minecraft:hot_floor" | "minecraft:campfire"
    );
    let amount = *amount;
    let by_skeleton = attacker.is_some_and(is_skeleton);
    let is_ender = |id: u32| ai::with_id(id, |s| s.kind == Kind::Enderman).unwrap_or(false);
    let by_ender = attacker.is_some_and(is_ender);
    ai::with_id(victim, |s| {
        // A mutant lying dead is not hurt (`isDeadOrDying`); `kill` takes its lives.
        if s.dying {
            if kill {
                s.lives = 0;
            }
            *cancel = true;
            return;
        }
        match k {
            Kind::Zombie => {
                if kill {
                    s.lives = 0;
                }
                if attacker.is_some_and(is_zombie) {
                    *cancel = true;
                }
                if s.anim == anim::ZOMBIE_THROW && attacker.is_some() && attacker == s.target {
                    *cancel = true;
                }
            }
            // MutantEnderman.hurt: no projectile, nothing while it teleports or screams, and a
            // hit ends its stare.
            Kind::Enderman => {
                let projectile = matches!(
                    damage_type,
                    "minecraft:arrow" | "minecraft:trident" | "minecraft:thrown" | "minecraft:mob_projectile" | "minecraft:fireball" | "minecraft:wind_charge" | "minecraft:spit"
                );
                if projectile || by_ender {
                    *cancel = true;
                } else if matches!(s.anim, anim::ENDER_TELEPORT | anim::ENDER_SCREAM) && damage_type != "minecraft:out_of_world" {
                    *cancel = true;
                } else if s.anim == anim::ENDER_STARE && amount > 0.0 {
                    s.anim = anim::NONE;
                }
                if !*cancel && !s.animating() {
                    AFTER.lock().unwrap_or_else(|e| e.into_inner()).push(After::Dodge { mob: victim, attacker, indirect: attacker.is_none() || damage_type.contains("explosion") || damage_type == "minecraft:fall" });
                }
            }
            // MutantSnowGolem.hurt: a snowball heals it; no fall or freeze damage.
            Kind::SnowGolem => {
                if damage_type == "minecraft:fall" || damage_type == "minecraft:freeze" {
                    *cancel = true;
                } else if damage_type == "minecraft:thrown" && amount <= 0.0 {
                    *cancel = true;
                    AFTER.lock().unwrap_or_else(|e| e.into_inner()).push(After::Heal { mob: victim, amount: 1.0 });
                }
            }
            Kind::SpiderPig => {
                if damage_type == "minecraft:fall" {
                    *cancel = true;
                }
            }
            Kind::Skeleton => {
                // Other mutant skeletons can't hurt it; it takes no fall damage.
                if damage_type == "minecraft:fall" || by_skeleton {
                    *cancel = true;
                }
            }
            Kind::Creeper => {
                if explosion {
                    *cancel = true;
                    if !OWN_BLAST.load(Ordering::Relaxed) {
                        AFTER.lock().unwrap_or_else(|e| e.into_inner()).push(After::Heal { mob: victim, amount: amount / 2.0 });
                    }
                } else if damage_type == "minecraft:lightning_bolt" {
                    // MutantCreeper.thunderHit: only charged.
                    s.charged = true;
                    *cancel = true;
                } else if fire && s.charged {
                    *cancel = true;
                }
                if !*cancel
                    && s.charging
                    && let Some(a) = attacker
                {
                    AFTER.lock().unwrap_or_else(|e| e.into_inner()).push(After::Thorns { mob: victim, attacker: a });
                }
            }
            _ => {}
        }
    });
}

/// After a hit landed (`native_living_damage_post`): a charging mutant creeper loses one of
/// its charge hits.
pub(super) fn damage_post(entity: i32, kind: &str, amount: f32) {
    if kind == ai::CREEPER && amount > 0.0 {
        ai::with_id(entity as u32, |s| {
            if s.charging {
                s.charge_hits -= 1;
            }
        });
    }
}

/// A mutant's `die`: the zombie and the creeper die their own way (`tickDeath`) before the
/// engine's death runs (`native_living_death`, canceled until their own death is done).
pub(super) fn death(game: &mut Game<'_>, entity: u32, kind: &str, cancel: &mut bool) {
    let Some(k) = ai::kind_of(kind) else {
        return;
    };
    if k == Kind::Skeleton {
        // MutantSkeleton.die: it bursts into its bones, which hurt what is near.
        if let Some(m) = game.mobs.iter().find(|m| m.id == entity) {
            let at = [m.position.x, m.position.y, m.position.z];
            SHATTERS.lock().unwrap_or_else(|e| e.into_inner()).push((entity, at, m.fire_ticks));
        }
        ai::with_id(entity, |s| s.final_death = true);
        return;
    }
    if !matches!(k, Kind::Zombie | Kind::Creeper | Kind::Enderman) {
        return;
    }
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == entity) else {
        return;
    };
    let (on_fire, yaw) = (m.is_on_fire(), m.body_yaw);
    let at = [m.position.x, m.position.y, m.position.z];
    let Some(started) = ai::with(m, |s| {
        if s.final_death {
            return false;
        }
        *cancel = true;
        if s.dying {
            return false;
        }
        s.dying = true;
        s.death_time = 0;
        s.death_yaw = yaw;
        s.play(anim::NONE);
        if k == Kind::Zombie && on_fire {
            s.lives = 0;
        }
        s.charging = false;
        true
    }) else {
        return;
    };
    if started && k == Kind::Creeper {
        game.native_play_sound_in_place("mutantmonsters:entity.mutant_creeper.death", at, HOSTILE, 2.0, 1.0);
    }
}

/// `MutantZombie.interactAt`: flint and steel or a fire charge on its corpse burns it for
/// good.
pub(super) fn interact(game: &mut Game<'_>, entity: i32, kind: &str, stack: Option<&ItemStack>, claimed: &mut bool) {
    if kind != ai::ZOMBIE {
        return;
    }
    let Some(stack) = stack else { return };
    let item = &*stack.item;
    if !matches!(item, "minecraft:flint_and_steel" | "minecraft:fire_charge") {
        return;
    }
    let id = entity as u32;
    let dead = ai::with_id(id, |s| s.dying).unwrap_or(false);
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    if !dead || m.is_on_fire() || m.fluids.in_water {
        return;
    }
    m.fire_ticks = 160;
    m.needs_sync = true;
    let at = [m.position.x, m.position.y, m.position.z];
    let pitch = rand_float() * 0.4 + 0.8;
    ai::with_id(id, |s| s.lives = 0);
    game.native_play_sound_in_place("minecraft:item.flintandsteel.use", at, HOSTILE, 1.0, pitch);
    if item == "minecraft:fire_charge" {
        use_up_held(game);
    } else {
        game.damage_held_tool();
    }
    *claimed = true;
}

// --- the level tick ----------------------------------------------------------------

/// A living thing a mutant's attack can reach: a mob or the player.
#[derive(Clone, Copy)]
struct Body {
    id: u32,
    pos: [f64; 3],
    /// Lowest then highest corner.
    bounds: [f64; 6],
    zombie: bool,
}

/// The mobs and the player but `except`, creative and spectator players left out
/// (`EntitySelector.NO_CREATIVE_OR_SPECTATOR`).
fn bodies(game: &Game<'_>, except: u32) -> Vec<Body> {
    use horizon_core::geometry::Axis::{X, Y, Z};
    let mut out: Vec<Body> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.id != except)
        .map(|m| {
            let b = m.bounds();
            Body {
                id: m.id,
                pos: [m.position.x, m.position.y, m.position.z],
                bounds: [b.min(X), b.min(Y), b.min(Z), b.max(X), b.max(Y), b.max(Z)],
                zombie: zombie_family(&m.kind.id),
            }
        })
        .collect();
    if let Some(p) = game.player.as_ref()
        && !game.creative
        && !p.spectator
    {
        let [x, y, z] = [p.position.x, p.position.y, p.position.z];
        out.push(Body { id: p.entity_id as u32, pos: [x, y, z], bounds: [x - 0.3, y, z - 0.3, x + 0.3, y + 1.8, z + 0.3], zombie: false });
    }
    out
}

fn zombie_family(kind: &str) -> bool {
    matches!(kind, "minecraft:zombie" | "minecraft:zombie_villager" | "minecraft:husk" | "minecraft:drowned" | ai::ZOMBIE)
}

fn meets(a: [f64; 6], b: [f64; 6]) -> bool {
    a[0] < b[3] && a[3] > b[0] && a[1] < b[4] && a[4] > b[1] && a[2] < b[5] && a[5] > b[2]
}

/// `Entity.setDeltaMovement` for a mob or the player (whose client is told, as
/// `sendPlayerVelocityPacket`).
fn set_velocity(game: &mut Game<'_>, id: u32, v: [f64; 3]) {
    if !v.iter().all(|c| c.is_finite()) {
        return;
    }
    let v3 = horizon_core::geometry::Vec3::new(v[0], v[1], v[2]);
    if let Some(p) = game.player.as_mut()
        && p.entity_id as u32 == id
    {
        p.velocity = v3;
        p.health.sync_velocity = true;
        if v[1] > 0.0 {
            p.ground.on_ground = false;
        }
        return;
    }
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
        m.velocity = v3;
        m.needs_sync = true;
        if v[1] > 0.0 {
            m.on_ground = false;
        }
    }
}

fn velocity(game: &Game<'_>, id: u32) -> [f64; 3] {
    if let Some(p) = game.player.as_ref()
        && p.entity_id as u32 == id
    {
        return [p.velocity.x, p.velocity.y, p.velocity.z];
    }
    game.mobs.iter().find(|m| m.id == id).map_or([0.0; 3], |m| [m.velocity.x, m.velocity.y, m.velocity.z])
}

fn position(game: &Game<'_>, id: u32) -> Option<[f64; 3]> {
    if let Some(p) = game.player.as_ref()
        && p.entity_id as u32 == id
    {
        return Some([p.position.x, p.position.y, p.position.z]);
    }
    game.mobs.iter().find(|m| m.id == id && !m.removed).map(|m| [m.position.x, m.position.y, m.position.z])
}

fn who_id(game: &Game<'_>, who: Who) -> Option<u32> {
    match who {
        Who::Player => game.player.as_ref().map(|p| p.entity_id as u32),
        Who::Mob(id) => Some(id),
    }
}

/// `addEffect` on a mob or the player.
fn effect(game: &mut Game<'_>, id: u32, effect: &'static str, ticks: i32, amplifier: i32) {
    let player = game.player.as_ref().is_some_and(|p| p.entity_id as u32 == id);
    let event = if player {
        MobEvent::EffectPlayer { effect, ticks, amplifier, ambient: false }
    } else {
        MobEvent::EffectMob { target: id, effect, ticks, amplifier }
    };
    let _ = game.handle_mob_events(vec![event]);
}

fn griefing(game: &Game<'_>, dimension: &str) -> bool {
    use horizon_client::mod_api::GameRuleValue;
    let Some(level) = game.native_level(dimension) else { return true };
    !matches!(game.native_level_game_rule(level, "mob_griefing"), Some(GameRuleValue::Bool(false)))
}

/// `MutatedExplosionHelper.explode`: at the feet, blocks broken as mob griefing allows, the
/// mutant creepers left out.
fn mutant_explode(game: &mut Game<'_>, dimension: &str, at: [f64; 3], radius: f32, fire: bool) {
    let blast = if griefing(game, dimension) { NativeBlast::Block } else { NativeBlast::Keep };
    OWN_BLAST.store(true, Ordering::Relaxed);
    game.native_explode(dimension, at, radius, fire, blast);
    OWN_BLAST.store(false, Ordering::Relaxed);
}

fn particles(game: &mut Game<'_>, kind: ParticleType, at: [f64; 3], spread: [f64; 3], count: i32) {
    let options = horizon_client::particles::ParticleOptions::simple(kind);
    game.native_send_particles_in_place(&options, None, at, spread, 0.02, count);
}

fn sound(game: &mut Game<'_>, name: &str, at: [f64; 3], volume: f32, pitch: f32) {
    game.native_play_sound_in_place(name, at, HOSTILE, volume, pitch);
}

/// `Level.levelEvent(2001, pos, Block.getId(state))`: the break particles and sound of the
/// block at `ground`, shown at `pos`.
fn crumble(game: &mut Game<'_>, pos: [i32; 3], ground: [i32; 3]) {
    let state = game.world.block(ground[0], ground[1], ground[2]);
    game.level_event_at(2001, pos, state.0 as i32);
}

/// `Level.isEmptyBlock`.
fn empty(game: &Game<'_>, pos: [i32; 3]) -> bool {
    is_air(game, pos)
}

/// Whether the block at `pos` has no collision shape.
fn no_collision(game: &Game<'_>, dimension: &str, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos.map(f64::from);
    game.native_collision_boxes(dimension, [x + 0.001, y + 0.001, z + 0.001, x + 0.999, y + 0.999, z + 0.999]).is_empty()
}

/// The fluid at `pos`: Some(true) lava, Some(false) water, None none.
fn fluid_at(game: &Game<'_>, pos: [i32; 3]) -> Option<bool> {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts().get(state);
    let name = &*facts.name;
    if facts.waterlogged || name == "minecraft:water" || name == "minecraft:bubble_column" {
        Some(false)
    } else if name == "minecraft:lava" {
        Some(true)
    } else {
        None
    }
}

fn is_day(game: &Game<'_>) -> bool {
    let t = game.world.day_time.rem_euclid(24000);
    !(13_000..23_000).contains(&t)
}

/// `ZombieResurrection.getSuitableGround`: the ground block near `y` in its column, -1 for
/// none (fire, too far, or in daylight under the open sky two times in three).
fn suitable_ground(game: &Game<'_>, dimension: &str, x: i32, y: i32, z: i32, range: i32, check_day: bool) -> i32 {
    let mut i = y;
    loop {
        if (y - i).abs() > range {
            return -1;
        }
        let pos = [x, i, z];
        let up = [x, i + 1, z];
        if game.native_block(pos).ends_with("fire") {
            return -1;
        }
        if let Some(lava) = fluid_at(game, pos)
            && (!check_day || lava)
        {
            break;
        }
        if empty(game, pos) {
            i -= 1;
            continue;
        }
        if empty(game, up) && no_collision(game, dimension, pos) {
            i -= 1;
            break;
        }
        if !empty(game, up) {
            if no_collision(game, dimension, up) {
                break;
            }
            i += 1;
            continue;
        }
        break;
    }
    if check_day && is_day(game) {
        let sky = game.world.light(x, y + 1, z);
        if (sky >> 4) >= 15 && rand_int(3) != 0 {
            return -1;
        }
    }
    i
}

/// `SeismicWave.createWaves`: a line of waves from `from` to `to` (Bresenham, in x and z).
fn create_waves(game: &Game<'_>, dimension: &str, waves: &mut Vec<ai::Wave>, from: [i32; 2], to: [i32; 2], y: i32) {
    let (x1, z1) = (from[0], from[1]);
    let (dx, dz) = (to[0] - x1, to[1] - z1);
    let sx = if dx < 0 { -1 } else { 1 };
    let sz = if dz < 0 { -1 } else { 1 };
    let (dx, dz) = (dx.abs(), dz.abs());
    let (mut x, mut z) = (x1, z1);
    let (dx2, dz2) = (dx * 2, dz * 2);
    let gy = suitable_ground(game, dimension, x1, y, z1, 3, false);
    waves.push(ai::Wave { pos: [x1, if gy != -1 { gy } else { y }, z1], first: true, terrain: true });
    let add = |x: i32, z: i32, waves: &mut Vec<ai::Wave>| {
        let gy = suitable_ground(game, dimension, x, y, z, 3, false);
        if gy != -1 {
            waves.push(ai::Wave { pos: [x, gy, z], first: false, terrain: true });
        }
        if rand_int(2) == 0 {
            waves.push(ai::Wave { pos: [x, gy + 1, z], first: false, terrain: false });
        }
    };
    if dx2 >= dz2 {
        let mut err = dx;
        for _ in 0..dx {
            x += sx;
            err += dz2;
            if err > dx2 {
                z += sz;
                err -= dx2;
            }
            add(x, z, waves);
        }
    } else {
        let mut err = dz;
        for _ in 0..dz {
            z += sz;
            err += dx2;
            if err > dz2 {
                x += sx;
                err -= dz2;
            }
            add(x, z, waves);
        }
    }
}

/// `SeismicWave.affectBlocks`: the ground trampled to dirt, small plants above it broken,
/// TNT lit, a bell rung, and the ground's crumbs thrown up.
fn wave_blocks(game: &mut Game<'_>, dimension: &str, w: ai::Wave) {
    if !w.terrain {
        return;
    }
    let pos = w.pos;
    let above = [pos[0], pos[1] + 1, pos[2]];
    let name = game.native_block(pos);
    let ground = game.world.block(pos[0], pos[1], pos[2]);
    if griefing(game, dimension) {
        if matches!(
            name.as_str(),
            "minecraft:grass_block" | "minecraft:dirt_path" | "minecraft:farmland" | "minecraft:podzol" | "minecraft:mycelium"
        ) && let Some(dirt) = game.native_default_state("minecraft:dirt")
        {
            game.native_set_block(dimension, pos, dirt, 3);
        }
        let up_state = game.world.block(above[0], above[1], above[2]);
        let hardness = game.res.facts().get(up_state).hardness;
        if !empty(game, above)
            && no_collision(game, dimension, above)
            && hardness > -1.0
            && hardness <= 1.0
            && fluid_at(game, above).is_none()
        {
            let _ = game.handle_mob_events(vec![MobEvent::BreakBlock { pos: above, drop: false }]);
        }
        if name.ends_with("_door") {
            game.level_event_at(if name == "minecraft:iron_door" { 1020 } else { 1019 }, pos, 0);
        }
        if name == "minecraft:tnt" {
            if let Some(air) = game.native_default_state("minecraft:air") {
                game.native_set_block(dimension, pos, air, 3);
            }
            let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5];
            game.native_summon(dimension, "minecraft:tnt", at, Compound::new());
        }
    }
    if name == "minecraft:bell" {
        let _ = game.handle_mob_events(vec![MobEvent::RingBell { bell: pos }]);
    }
    if fluid_at(game, pos).is_none() {
        game.level_event_at(2001, above, ground.0 as i32);
    }
}

/// `MutantZombie.updateMeleeGrounds`: one queued wave a tick hurts and throws what stands on it.
fn zombie_wave(game: &mut Game<'_>, dimension: &str, id: u32, at: [f64; 3], w: ai::Wave) {
    wave_blocks(game, dimension, w);
    let [x, y, z] = w.pos.map(f64::from);
    let mut b = [x, y + 1.0, z, x + 1.0, y + 2.0, z + 1.0];
    if w.first {
        let d = rand() * 0.75;
        let (h, v) = (0.25 + d, 0.25 + d * 0.5);
        b = [b[0] - h, b[1] - v, b[2] - h, b[3] + h, b[4] + v, b[5] + h];
    }
    for body in bodies(game, id) {
        if body.zombie || !meets(b, body.bounds) {
            continue;
        }
        let amount = if w.first { 9 + rand_int(4) } else { 6 + rand_int(3) } as f32;
        let hurt = game.native_hurt_entity(dimension, body.id, amount, "mutantmonsters:mutant_zombie_seismic_wave", Some(id));
        if hurt && rand_int(5) == 0 {
            effect(game, body.id, "minecraft:hunger", 160, 1);
        }
        let (dx, dz) = (body.pos[0] - at[0], body.pos[2] - at[2]);
        let d = (dx * dx + dz * dz).sqrt().max(1e-4);
        set_velocity(game, body.id, [dx / d * 0.3, 0.04, dz / d * 0.3]);
    }
}

/// `ZombieResurrection.getZombieByLocation`, and its ambient sound.
fn risen_kind(game: &Game<'_>, up: [i32; 3]) -> (&'static str, &'static str) {
    let biome = game.native_biome(up).unwrap_or_default();
    if (biome.contains("ocean") || biome.contains("river")) && fluid_at(game, up) == Some(false) {
        ("minecraft:drowned", "minecraft:entity.drowned.ambient")
    } else if is_day(game) && (game.world.light(up[0], up[1], up[2]) >> 4) >= 15 {
        ("minecraft:husk", "minecraft:entity.husk.ambient")
    } else if rand_float() < 0.05 {
        ("minecraft:zombie_villager", "minecraft:entity.zombie_villager.ambient")
    } else {
        ("minecraft:zombie", "minecraft:entity.zombie.ambient")
    }
}

/// `ZombieResurrection.update`: the ground crumbles, then a zombie climbs out. False when it
/// is done or the ground is gone.
fn resurrect(game: &mut Game<'_>, dimension: &str, mutant: u32, yaw: f32, target: Option<Who>, r: &mut ([i32; 3], i32)) -> bool {
    let ground = r.0;
    let up = [ground[0], ground[1] + 1, ground[2]];
    if empty(game, ground) || !empty(game, up) {
        return false;
    }
    if rand_int(15) == 0 {
        crumble(game, up, ground);
    }
    r.1 -= 1;
    if r.1 > 0 {
        return true;
    }
    let (kind, ambient) = risen_kind(game, up);
    let at = [f64::from(up[0]) + 0.5, f64::from(up[1]), f64::from(up[2]) + 0.5];
    crumble(game, up, ground);
    if let Some(id) = game.native_spawn_mob(dimension, kind, at, &Default::default()) {
        if let Some(z) = game.mobs.iter_mut().find(|m| m.id == id) {
            z.health = z.attrs.max_health * (0.6 + 0.4 * z.random.next_float());
            z.yaw = yaw;
            z.body_yaw = yaw;
            z.head_yaw = yaw;
            z.ai.target = target;
            z.ai.goals.add(0, ai::TrackSummoner::new(mutant));
            z.needs_sync = true;
        }
        sound(game, ambient, at, 1.0, (rand_float() - rand_float()) * 0.2 + 1.0);
    }
    false
}

/// The goals' requests for the world (`ai::JOBS`) for the mobs of this level.
fn ai_jobs(game: &mut Game<'_>, dimension: &str) {
    let here: HashSet<u32> = game.mobs.iter().map(|m| m.id).collect();
    let jobs = {
        let mut all = ai::JOBS.lock().unwrap_or_else(|e| e.into_inner());
        let (mine, rest): (Vec<ai::Job>, Vec<ai::Job>) =
            std::mem::take(&mut *all).into_iter().partition(|j| here.contains(&job_mob(j)));
        *all = rest;
        mine
    };
    for job in jobs {
        let mob = job_mob(&job);
        let Some(at) = position(game, mob) else { continue };
        match job {
            ai::Job::Sound { name, volume, pitch, .. } => sound(game, name, at, volume, pitch),
            ai::Job::Melee { target, .. } => melee(game, dimension, mob, at, target),
            ai::Job::Slam { from, to, y, .. } => {
                let mut waves = Vec::new();
                create_waves(game, dimension, &mut waves, from, to, y);
                ai::with_id(mob, |s| s.waves.extend(waves));
            }
            ai::Job::Roar { .. } => {
                for body in bodies(game, mob) {
                    let (dx, dy, dz) = (body.pos[0] - at[0], body.pos[1] - at[1], body.pos[2] - at[2]);
                    if body.zombie || dx * dx + dy * dy + dz * dz > 196.0 || dy.abs() > 11.2 {
                        continue;
                    }
                    let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                    set_velocity(game, body.id, [dx / d * 0.7, 0.3, dz / d * 0.7]);
                    let amount = (2 + rand_int(2)) as f32;
                    game.native_hurt_entity(dimension, body.id, amount, "mutantmonsters:piercing_mob_attack", Some(mob));
                }
            }
            ai::Job::Resurrect { at: [x, y, z], .. } => {
                let gy = suitable_ground(game, dimension, x, y - 1, z, 4, true);
                if gy != -1 {
                    let tick = 100 + rand_int(40);
                    ai::with_id(mob, |s| s.resurrections.push(([x, gy, z], tick)));
                }
            }
            ai::Job::Fling { target, .. } => {
                if let Some(t) = who_id(game, target)
                    && let Some(p) = position(game, t)
                {
                    let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
                    let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                    set_velocity(game, t, [dx / d * 0.8, 1.6, dz / d * 0.8]);
                }
            }
            ai::Job::ThrowHit { target, .. } => {
                if let Some(t) = who_id(game, target)
                    && let Some(p) = position(game, t)
                {
                    let damage = game.mobs.iter().find(|m| m.id == mob).map_or(12.0, |m| m.attrs.attack_damage as f32);
                    game.native_hurt_entity(dimension, t, damage, "minecraft:mob_attack", Some(mob));
                    let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
                    let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                    set_velocity(game, t, [dx / d * 0.6, -1.2, dz / d * 0.6]);
                    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == t) {
                        m.invulnerable = 10;
                    }
                }
            }
            ai::Job::ChargeEnd { lightning, heal, .. } => {
                if lightning {
                    ai::with_id(mob, |s| s.charged = true);
                    game.native_summon(dimension, "minecraft:lightning_bolt", at, Compound::new());
                } else if heal {
                    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob) {
                        m.heal(30.0);
                        m.needs_sync = true;
                    }
                    particles(game, ParticleType::Heart, [at[0], at[1] + 1.5, at[2]], [1.0, 1.0, 1.0], 15);
                }
            }
            ai::Job::Minions { target, .. } => creeper_minions(game, dimension, mob, at, target),
            ai::Job::Sweep { .. } => skeleton_sweep(game, dimension, mob, at),
            ai::Job::Constrict { target, .. } => {
                if let Some(t) = who_id(game, target) {
                    let damage = game.mobs.iter().find(|m| m.id == mob).map_or(3.0, |m| m.attrs.attack_damage as f32);
                    game.native_hurt_entity(dimension, t, damage + 6.0, "minecraft:mob_attack", Some(mob));
                    let sign = |b: bool| if b { 1.0 } else { -1.0 };
                    let vx = (1.0 + rand() * 0.4) * sign(rand() < 0.5);
                    let vy = 0.4 + rand() * 0.8;
                    let vz = (1.0 + rand() * 0.4) * sign(rand() < 0.5);
                    set_velocity(game, t, [vx, vy, vz]);
                }
            }
            ai::Job::Arrows { target, count, speed, spread, effects, .. } => {
                skeleton_arrows(game, dimension, mob, target, count, speed, spread, effects);
            }
            ai::Job::EnderAttack { target, .. } => ender_attack(game, dimension, mob, target),
            ai::Job::EnderSwipe { .. } => ender_swipe(game, dimension, mob, at),
            ai::Job::EnderThrow { target, .. } => ender_throw(game, dimension, mob, at, target),
            ai::Job::StareEnd { target, .. } => {
                if let Some(t) = who_id(game, target)
                    && let Some(p) = position(game, t)
                {
                    game.native_hurt_entity(dimension, t, 2.0, "mutantmonsters:piercing_mob_attack", Some(mob));
                    effect(game, t, "minecraft:blindness", 160 + rand_int(140), 0);
                    set_velocity(game, t, [(at[0] - p[0]) * 0.1, 0.3, (at[2] - p[2]) * 0.1]);
                }
            }
            ai::Job::TeleportStart { .. } => {
                let to = ai::with_id(mob, |s| s.teleport_to).flatten();
                teleport_attack(game, dimension, mob, at);
                sound(game, "mutantmonsters:entity.mutant_enderman.teleport", at, 1.0, 1.0);
                if let Some(to) = to {
                    let dest = [f64::from(to[0]) + 0.5, f64::from(to[1]), f64::from(to[2]) + 0.5];
                    sound(game, "mutantmonsters:entity.mutant_enderman.teleport", dest, 1.0, 1.0);
                    teleport_attack(game, dimension, mob, dest);
                    endersoul_burst(game, dest, 256);
                }
                endersoul_burst(game, [at[0], at[1], at[2]], 256);
            }
            ai::Job::TeleportEnd { .. } => {
                let to = ai::with_id(mob, |s| {
                    s.play(anim::NONE);
                    s.teleport_to.take()
                })
                .flatten();
                if let Some(to) = to {
                    let dest = [f64::from(to[0]) + 0.5, f64::from(to[1]), f64::from(to[2]) + 0.5];
                    game.native_teleport_mob(dimension, mob, dest);
                    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob) {
                        m.fall_distance = 0.0;
                    }
                }
            }
            ai::Job::Scream { .. } => ender_scream(game, dimension, mob, at),
            ai::Job::TeleSmashStart { target, .. } => {
                if let Some(t) = who_id(game, target) {
                    effect(game, t, "minecraft:slowness", 20, 5);
                    effect(game, t, "minecraft:nausea", 160 + rand_int(160), 0);
                }
            }
            ai::Job::TeleSmash { target, .. } => ender_telesmash(game, dimension, mob, target),
            ai::Job::ThrowIce { target, .. } => {
                if let Some(t) = who_id(game, target)
                    && let Some(p) = position(game, t)
                {
                    let from = [at[0], at[1] + 2.855, at[2]];
                    let (x, y, z) = (p[0] - from[0], p[1] - from[1], p[2] - from[2]);
                    let d = (x * x + y * y + z * z).sqrt();
                    let v = shoot([x, y + d * 0.5, z], 0.9, 1.0);
                    let yaw = game.mobs.iter().find(|m| m.id == mob).map_or(0.0, |m| m.yaw);
                    spawn_flying(game, dimension, mob, true, "minecraft:ice", from, v, 0.06, yaw);
                }
            }
            ai::Job::SwimJump { .. } => {
                game.native_hurt_entity(dimension, mob, 16.0, "minecraft:drown", None);
            }
            ai::Job::PigBite { target, .. } => pig_bite(game, dimension, mob, target),
        }
    }
}

/// `MutantSkeleton.MeleeGoal` at tick 3: what is within 3 blocks and 60 degrees of its head
/// is hit and thrown away.
fn skeleton_sweep(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3]) {
    let Some(m) = game.mobs.iter().find(|m| m.id == mob) else { return };
    let (head, damage) = (m.head_yaw, m.attrs.attack_damage as f32);
    let skeletons: HashSet<u32> = game.mobs.iter().filter(|m| &*m.kind.id == ai::SKELETON).map(|m| m.id).collect();
    for body in bodies(game, mob) {
        if skeletons.contains(&body.id) {
            continue;
        }
        let (dx, dy, dz) = (at[0] - body.pos[0], at[1] - body.pos[1], at[2] - body.pos[2]);
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist > 3.0 || dist <= 0.0 || head_angle(head, dx, dz) >= 60.0 {
            continue;
        }
        let power = 1.8 + f64::from(rand_int(5)) * 0.15;
        let amount = if damage > 0.0 { damage + rand_int(2) as f32 } else { 0.0 };
        game.native_hurt_entity(dimension, body.id, amount, "minecraft:mob_attack", Some(mob));
        let vy = velocity(game, body.id)[1].max(0.28);
        set_velocity(game, body.id, [-dx / dist * power, vy, -dz / dist * power]);
    }
}

/// `EntityUtil.getHeadAngle`: how far `(x, z)` (the mob less the target) is from the way its
/// head faces.
fn head_angle(head_yaw: f32, x: f64, z: f64) -> f32 {
    let yaw = (z.atan2(x).to_degrees() + 90.0) as f32;
    wrap(yaw - head_yaw).abs()
}

/// `MutantArrow`s from the mutant skeleton's eyes at `target`: no gravity, critical, each aimed
/// at a point within `spread` of 80% of its height, and a single shot may carry poison,
/// hunger or slowness.
#[allow(clippy::too_many_arguments)]
fn skeleton_arrows(game: &mut Game<'_>, dimension: &str, mob: u32, target: Who, count: u32, speed: f32, spread: f32, effects: bool) {
    let Some(t) = who_id(game, target) else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == mob) else { return };
    let from = [m.position.x, m.position.y + m.kind.eye_height - 0.1, m.position.z];
    let Some(body) = bodies(game, mob).into_iter().find(|b| b.id == t) else { return };
    let height = body.bounds[4] - body.bounds[1];
    let owner = game.native_entity(mob).and_then(|e| game.native_entity_uuid(e));
    let owner = owner.map(|u| format!(",Owner:[I;{},{},{},{}]", (u >> 96) as i32, (u >> 64) as i32, (u >> 32) as i32, u as i32)).unwrap_or_default();
    let spread = f64::from(spread);
    for _ in 0..count {
        let aim = [
            body.pos[0] + (rand() - 0.5) * spread * 2.0,
            body.pos[1] + height * 0.8 + (rand() - 0.5) * spread * 2.0,
            body.pos[2] + (rand() - 0.5) * spread * 2.0,
        ];
        let v = [aim[0] - from[0], aim[1] - from[1], aim[2] - from[2]];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
        let v = v.map(|c| c / len * f64::from(speed));
        let mut potion = String::new();
        if effects {
            let mut list = Vec::new();
            if rand_int(4) == 0 {
                list.push(format!("{{id:\"minecraft:poison\",duration:{},amplifier:0b}}", 80 + rand_int(60)));
            }
            if rand_int(4) == 0 {
                list.push(format!("{{id:\"minecraft:hunger\",duration:{},amplifier:1b}}", 120 + rand_int(60)));
            }
            if rand_int(4) == 0 {
                list.push(format!("{{id:\"minecraft:slowness\",duration:{},amplifier:1b}}", 120 + rand_int(60)));
            }
            if !list.is_empty() {
                potion = format!(
                    ",item:{{id:\"minecraft:tipped_arrow\",count:1,components:{{\"minecraft:potion_contents\":{{custom_effects:[{}]}}}}}}",
                    list.join(",")
                );
            }
        }
        let yaw = (180.0 + v[0].atan2(v[2]).to_degrees()) as f32;
        let pitch = v[1].atan2((v[0] * v[0] + v[2] * v[2]).sqrt()).to_degrees() as f32;
        game.native_run_command(&format!(
            "execute in {dimension} run summon minecraft:arrow {} {} {} {{Motion:[{}d,{}d,{}d],Rotation:[{yaw}f,{pitch}f],NoGravity:1b,crit:1b,pickup:0b,damage:2.0d,Tags:[\"mutantmonsters.arrow\"]{owner}{potion}}}",
            from[0], from[1], from[2], v[0], v[1], v[2]
        ));
    }
}

fn job_mob(j: &ai::Job) -> u32 {
    match *j {
        ai::Job::Sound { mob, .. }
        | ai::Job::Melee { mob, .. }
        | ai::Job::Slam { mob, .. }
        | ai::Job::Roar { mob }
        | ai::Job::Resurrect { mob, .. }
        | ai::Job::Fling { mob, .. }
        | ai::Job::ThrowHit { mob, .. }
        | ai::Job::ChargeEnd { mob, .. }
        | ai::Job::Minions { mob, .. }
        | ai::Job::Sweep { mob }
        | ai::Job::Constrict { mob, .. }
        | ai::Job::Arrows { mob, .. }
        | ai::Job::EnderAttack { mob, .. }
        | ai::Job::EnderSwipe { mob }
        | ai::Job::EnderThrow { mob, .. }
        | ai::Job::StareEnd { mob, .. }
        | ai::Job::TeleportStart { mob }
        | ai::Job::TeleportEnd { mob }
        | ai::Job::Scream { mob }
        | ai::Job::TeleSmashStart { mob, .. }
        | ai::Job::TeleSmash { mob, .. }
        | ai::Job::ThrowIce { mob, .. }
        | ai::Job::SwimJump { mob }
        | ai::Job::PigBite { mob, .. } => mob,
    }
}

/// Each mutant's `doHurtTarget` that hurts (the zombie's only picks an animation).
fn melee(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3], target: Who) {
    let Some(t) = who_id(game, target) else { return };
    let Some(p) = position(game, t) else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == mob) else { return };
    let kind = ai::kind_of(&m.kind.id);
    let damage = m.attrs.attack_damage as f32;
    let hurt = game.native_hurt_entity(dimension, t, damage, "minecraft:mob_attack", Some(mob));
    if kind == Some(Kind::Creeper) && hurt {
        // MutantCreeper.doHurtTarget: a push away and up.
        let (dx, dy, dz) = (p[0] - at[0], p[1] - at[1], p[2] - at[2]);
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        if d > 0.0 {
            let v = velocity(game, t);
            set_velocity(game, t, [v[0] + dx / d * 0.5, v[1] + dy / d * 0.05 + 0.15, v[2] + dz / d * 0.5]);
        }
    }
}

/// What the damage hooks left: heals and thorns.
fn after_hits(game: &mut Game<'_>, dimension: &str) {
    let after = std::mem::take(&mut *AFTER.lock().unwrap_or_else(|e| e.into_inner()));
    for a in after {
        match a {
            After::Heal { mob, amount } => {
                let Some(m) = game.mobs.iter_mut().find(|m| m.id == mob && m.alive()) else { continue };
                if m.health >= m.attrs.max_health {
                    continue;
                }
                m.heal(amount);
                m.needs_sync = true;
                let at = [m.position.x, m.position.y + m.kind.height / 2.0, m.position.z];
                let (w, h) = (m.kind.width, m.kind.height);
                particles(game, ParticleType::Heart, at, [w / 2.0, h / 2.0, w / 2.0], (amount / 2.0) as i32);
            }
            After::Thorns { mob, attacker } => {
                game.native_hurt_entity(dimension, attacker, 2.0, "minecraft:thorns", Some(mob));
            }
            After::Dodge { mob, attacker, indirect } => {
                if game.mobs.iter().any(|m| m.id == mob && m.alive()) && !ender_teleport_chance(game, dimension, mob, if indirect { 3 } else { 6 }, attacker) && !indirect {
                    ender_teleport_chance(game, dimension, mob, 5, attacker);
                }
            }
        }
    }
}

/// Mutant skeletons that just died: id, where, and its fire, for the shatter.
static SHATTERS: Mutex<Vec<(u32, [f64; 3], i32)>> = Mutex::new(Vec::new());

/// The body part ids (`MutantSkeletonBodyPart.PART`) a mutant skeleton bursts into, and their items.
const BODY_PARTS: [u8; 18] = [0, 1, 2, 7, 8, 13, 14, 19, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30];

fn part_item(part: u8) -> &'static str {
    match part {
        0 => "mutantmonsters:mutant_skeleton_pelvis",
        1..=18 => "mutantmonsters:mutant_skeleton_rib",
        19 => "mutantmonsters:mutant_skeleton_skull",
        21..=28 => "mutantmonsters:mutant_skeleton_limb",
        _ => "mutantmonsters:mutant_skeleton_shoulder_pad",
    }
}

/// `MutantSkeleton.die`: the shatter hurts what is within 3 blocks (7, `mutant_skeleton_shatter`),
/// and its 18 bones fly out.
fn shatter(game: &mut Game<'_>, dimension: &str) {
    let all = std::mem::take(&mut *SHATTERS.lock().unwrap_or_else(|e| e.into_inner()));
    for (id, at, fire) in all {
        let b = [at[0] - 3.6, at[1] - 2.0, at[2] - 3.6, at[0] + 3.6, at[1] + 5.6, at[2] + 3.6];
        for body in bodies(game, id) {
            if meets(b, body.bounds) {
                game.native_hurt_entity(dimension, body.id, 7.0, "mutantmonsters:mutant_skeleton_shatter", None);
            }
        }
        for part in BODY_PARTS {
            let y = at[1] + 3.2 * (0.25 + rand() * 0.5);
            let v = [rand() * 1.6 - 0.8, rand() * 0.25 + 0.1, rand() * 1.6 - 0.8];
            let fire = if fire > 0 { format!(",Fire:{fire}s") } else { String::new() };
            game.native_run_command(&format!(
                "execute in {dimension} run summon minecraft:item {} {y} {} {{Item:{{id:\"{}\",count:1}},Motion:[{}d,{}d,{}d],PickupDelay:10{fire}}}",
                at[0],
                at[2],
                part_item(part),
                v[0],
                v[1],
                v[2]
            ));
        }
    }
}

/// The mutants whose saved state was read back.
static LOADED: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

/// The level ticked: the goals' jobs, then each mutant's own tick.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str) {
    tick_jobs(game, dimension);
    *ZOMBIES.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(game.mobs.iter().filter(|m| zombie_family(&m.kind.id)).map(|m| m.id).collect());
    after_hits(game, dimension);
    shatter(game, dimension);
    ai_jobs(game, dimension);
    tick_flying(game, dimension);
    let mutants: Vec<u32> = game.mobs.iter().filter(|m| !m.removed && ai::kind_of(&m.kind.id).is_some()).map(|m| m.id).collect();
    for id in mutants {
        mutant_tick(game, dimension, id);
    }
    // Forget the mutants that left this level.
    let here: HashSet<u32> = game.mobs.iter().filter(|m| !m.removed).map(|m| m.id).collect();
    if let Some(all) = ai::SERVER.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        all.retain(|id, _| here.contains(id));
    }
}

/// One mutant's tick after its level's: its `tick` after `super.tick`, and its own death.
fn mutant_tick(game: &mut Game<'_>, dimension: &str, id: u32) {
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    if ai::with(m, |_| ()).is_none() {
        return;
    }
    // Its saved lives and charge, read back once.
    let first = LOADED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(id);
    if first {
        let host: &mut dyn Host = game;
        if let Ok(Some(saved)) = STATE.get_existing(host, Holder::Entity(id)) {
            ai::with_id(id, |s| {
                s.lives = saved.lives;
                s.charged = saved.flags & CHARGED != 0;
            });
        }
    }
    let Some(mut st) = ai::with_id(id, |s| s.clone()) else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == id) else { return };
    st.target = m.ai.target.and_then(|w| who_id(game, w));
    if !m.alive() {
        // The engine's own death, the last one: taken away with the mod's poof (entity
        // event 60, `makePoofParticles`).
        if st.final_death {
            let (at, w, h, fire) = ([m.position.x, m.position.y, m.position.z], m.kind.width, m.kind.height, m.is_on_fire());
            let kind = if fire { ParticleType::Flame } else { ParticleType::Poof };
            particles(game, kind, [at[0], at[1] + h / 2.0, at[2]], [w / 2.0, h / 2.0, w / 2.0], 30);
            game.native_remove_mob(dimension, id);
        }
        return;
    }
    ai::with_id(id, |s| s.target = st.target);
    test_trigger(game, id, &mut st);
    if !st.dying && matches!(st.kind, Kind::Zombie | Kind::Skeleton | Kind::Creeper | Kind::Enderman) {
        break_leaves(game, dimension, id);
    }
    match st.kind {
        Kind::Zombie => zombie_tick(game, dimension, id, &mut st),
        Kind::Creeper => creeper_tick(game, dimension, id, &mut st),
        Kind::Skeleton => night_heal(game, dimension, id),
        Kind::Enderman => ender_tick(game, dimension, id, &mut st),
        Kind::SnowGolem => golem_tick(game, dimension, id, &mut st),
        Kind::SpiderPig => pig_tick(game, dimension, id, &mut st),
        Kind::Minion => {}
    }
    // Back into the shared state: what this tick owns; then `updateAnimation`.
    let now = ai::with_id(id, |s| {
        if s.animating() {
            s.anim_tick += 1;
        }
        s.dying = st.dying;
        s.death_time = st.death_time;
        s.vanish = st.vanish;
        s.lives = st.lives.min(s.lives);
        s.final_death |= st.final_death;
        s.throw_hit_tick = st.throw_hit_tick;
        if st.kind == Kind::Zombie {
            s.waves.splice(0..0, st.waves.drain(..));
            s.resurrections.splice(0..0, st.resurrections.drain(..));
        }
        s.jump_tick = st.jump_tick;
        s.last_jump_tick = st.last_jump_tick;
        s.e = st.e;
        synced(s)
    });
    let Some(now) = now else { return };
    let host: &mut dyn Host = game;
    let old = STATE.get_existing(&mut *host, Holder::Entity(id)).ok().flatten();
    if old.as_ref() != Some(&now)
        && let Err(e) = STATE.set(host, Holder::Entity(id), now)
    {
        log::warn!("mutantmonsters: state of {id} not synced: {e:?}");
    }
}

/// `AbstractMutantMonster.aiStep`: chasing something and walled in, it tears through leaves
/// (mob griefing) or else jumps.
fn break_leaves(game: &mut Game<'_>, dimension: &str, id: u32) {
    use horizon_core::geometry::Axis::{X, Y, Z};
    let Some(m) = game.mobs.iter().find(|m| m.id == id) else { return };
    if !m.horizontal_collision || m.ai.target.is_none() || !griefing(game, dimension) {
        return;
    }
    let b = m.bounds();
    let lo = [(b.min(X) - 0.2).floor() as i32, (b.min(Y) - 0.2).floor() as i32, (b.min(Z) - 0.2).floor() as i32];
    let hi = [(b.max(X) + 0.2).floor() as i32, (b.max(Y) + 0.2).floor() as i32, (b.max(Z) + 0.2).floor() as i32];
    let mut leaves = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                if game.native_block([x, y, z]).ends_with("_leaves") {
                    leaves.push([x, y, z]);
                }
            }
        }
    }
    if leaves.is_empty() {
        if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id)
            && m.on_ground
        {
            // jumpFromGround.
            m.velocity.y = 0.42;
            m.on_ground = false;
            m.needs_sync = true;
        }
        return;
    }
    let events = leaves.into_iter().map(|pos| MobEvent::BreakBlock { pos, drop: true }).collect();
    let _ = game.handle_mob_events(events);
}

/// For tests and screenshots: `/tag <mutant> add mutantmonsters.anim.<n>` plays its animation
/// `n` from its first tick, also on a mutant with `NoAI` (whose goals don't run, so here it
/// ends after its duration).
fn test_trigger(game: &mut Game<'_>, id: u32, st: &mut Mutant) {
    let Some(e) = game.native_entity(id) else { return };
    for tag in game.native_entity_tags(e).unwrap_or_default() {
        if tag == "mutantmonsters.charged" {
            ai::with_id(id, |s| s.charged = true);
            st.charged = true;
            game.native_entity_remove_tag(e, &tag);
        }
        if tag == "mutantmonsters.hold" {
            ai::with_id(id, |s| {
                s.held = ["minecraft:stone".into(), "minecraft:dirt".into(), "minecraft:oak_planks".into(), String::new()];
                s.has_target = 20;
            });
            st.held = ["minecraft:stone".into(), "minecraft:dirt".into(), "minecraft:oak_planks".into(), String::new()];
            game.native_entity_remove_tag(e, &tag);
        }
        if let Some(n) = tag.strip_prefix("mutantmonsters.anim.").and_then(|n| n.parse::<u8>().ok()) {
            ai::with_id(id, |s| s.play(n));
            st.play(n);
            game.native_entity_remove_tag(e, &tag);
        }
    }
    let no_ai = game.mobs.iter().find(|m| m.id == id).is_some_and(|m| m.no_ai);
    if no_ai && st.animating() && st.anim_tick >= anim::duration(st.kind, st.anim) {
        ai::with_id(id, |s| s.play(anim::NONE));
        st.play(anim::NONE);
    }
}

/// `MutantZombie.tick` after `super.tick`, and its `tickDeath`.
fn zombie_tick(game: &mut Game<'_>, dimension: &str, id: u32, st: &mut Mutant) {
    let day = is_day(game);
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    let at = [m.position.x, m.position.y, m.position.z];
    let (yaw, target_who) = (m.body_yaw, m.ai.target);
    if st.dying {
        // Lying: no AI, the body as it fell (`tickHeadTurn` while `deathTime > 0`).
        m.nav_stop();
        m.body_yaw = st.death_yaw;
        m.previous_body_yaw = st.death_yaw;
        if m.is_on_fire() {
            st.lives = 0;
        }
        if st.lives > 0 {
            st.death_time += 1;
            if st.death_time >= 140 {
                // Back on its feet with a quarter of its health and a life fewer.
                st.death_time = 0;
                st.dying = false;
                m.health = m.attrs.max_health / 4.0;
                m.needs_sync = true;
                st.lives -= 1;
            }
        } else {
            if st.death_time > 100 {
                st.death_time -= 1;
            } else if st.death_time < 100 {
                st.death_time += 1;
            }
            st.vanish += 1;
            if st.vanish >= 100 {
                // super.die: the loot, and gone.
                st.final_death = true;
                ai::with_id(id, |s| s.final_death = true);
                m.silent = true;
                game.native_kill_mob(dimension, id);
            }
        }
    } else {
        // fixRotation: the body follows the head, faster in a slam.
        let d = wrap(m.head_yaw - m.body_yaw);
        let f = if st.anim == anim::ZOMBIE_SLAM { 0.2 } else { 0.1 };
        m.body_yaw += d * f;
        // At night it heals 2 every 5 seconds.
        if !day && dimension == "minecraft:overworld" && m.age % 100 == 0 && m.health < m.attrs.max_health {
            m.heal(2.0);
            m.needs_sync = true;
        }
    }
    // The throw's hit tick, which the model counts up from (the client's `throwHitTick`).
    let (anim_now, hit) = ai::with_id(id, |s| (s.anim, s.throw_hit)).unwrap_or((0, false));
    if anim_now == anim::ZOMBIE_THROW {
        if hit {
            st.throw_hit_tick = st.throw_hit_tick.max(0) + 1;
        }
    } else {
        st.throw_hit_tick = -1;
    }
    // updateMeleeGrounds: one wave a tick.
    st.waves = ai::with_id(id, |s| std::mem::take(&mut s.waves)).unwrap_or_default();
    if !st.waves.is_empty() {
        let w = st.waves.remove(0);
        zombie_wave(game, dimension, id, at, w);
    }
    // The zombies it called rise, even while it lies.
    let resurrections = ai::with_id(id, |s| std::mem::take(&mut s.resurrections)).unwrap_or_default();
    st.resurrections = resurrections
        .into_iter()
        .filter_map(|mut r| resurrect(game, dimension, id, yaw, target_who, &mut r).then_some(r))
        .collect();
}

/// `MutantCreeper.tick` after `super.tick`, and its `tickDeath`: a 5-second implosion that
/// pulls everything in, then the blast.
fn creeper_tick(game: &mut Game<'_>, dimension: &str, id: u32, st: &mut Mutant) {
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    let at = [m.position.x, m.position.y, m.position.z];
    let on_fire = m.is_on_fire();
    let charged = st.charged;
    if charged && on_fire {
        // fireImmune while charged.
        m.fire_ticks = 0;
    }
    if st.dying {
        m.nav_stop();
        st.death_time += 1;
        let power: f32 = if charged { 12.0 } else { 8.0 };
        let radius = f64::from(power) * 1.5;
        let s = f64::from(st.death_time) / 100.0;
        // The shaking of `setPosRaw`.
        m.position.x += f64::from(rand_float() * 0.2 - 0.1);
        m.position.z += f64::from(rand_float() * 0.2 - 0.1);
        m.needs_sync = true;
        for body in bodies(game, id) {
            let (dx, dy, dz) = (at[0] - body.pos[0], at[1] - body.pos[1], at[2] - body.pos[2]);
            if dx.abs() > radius + 1.0 || dy.abs() > radius + 1.5 || dz.abs() > radius + 1.0 {
                continue;
            }
            let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-4);
            let v = velocity(game, body.id);
            set_velocity(game, body.id, [v[0] + dx / d * s * 0.09, v[1] + dy / d * s * 0.09, v[2] + dz / d * s * 0.09]);
        }
        if st.death_time >= 100 {
            mutant_explode(game, dimension, at, power, on_fire);
            st.final_death = true;
            ai::with_id(id, |s| s.final_death = true);
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.silent = true;
            }
            game.native_kill_mob(dimension, id);
        }
        return;
    }
    st.last_jump_tick = st.jump_tick;
    if st.jump_attacking {
        if st.jump_tick == 0 {
            let pitch = (rand_float() - rand_float()) * 0.2 + 1.0;
            sound(game, "mutantmonsters:entity.mutant_creeper.primed", at, 2.0, pitch);
        }
        st.jump_tick += 1;
        let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
        m.stuck_multiplier = horizon_core::geometry::Vec3::ZERO;
        let landed = (m.on_ground && st.jump_tick > 1) || m.fluids.in_water || m.fluids.in_lava;
        if landed {
            mutant_explode(game, dimension, at, if charged { 6.0 } else { 4.0 }, false);
            ai::with_id(id, |s| s.jump_attacking = false);
        }
    } else if st.jump_tick > 0 {
        st.jump_tick = 0;
    }
}

// --- thrown blocks (`ThrowableBlock`) ----------------------------------------------

/// A block a mutant threw: the enderman's held blocks and the snow golem's ice, flown here
/// (`ThrowableProjectile`) and drawn by a `block_display` that tumbles as the mod's
/// `ThrowableBlockRenderer` turns it.
struct Flying {
    dimension: String,
    owner: u32,
    golem: bool,
    block: String,
    pos: [f64; 3],
    vel: [f64; 3],
    gravity: f64,
    yaw: f32,
    display: Option<u32>,
    age: i32,
}

static FLYING: Mutex<Vec<Flying>> = Mutex::new(Vec::new());

/// `Projectile.shoot(x, y, z, velocity, inaccuracy)`: the direction with a triangular jitter.
fn shoot(d: [f64; 3], velocity: f64, inaccuracy: f64) -> [f64; 3] {
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
    let tri = || (rand() - rand()) * 0.017_227_5 * inaccuracy;
    [(d[0] / len + tri()) * velocity, (d[1] / len + tri()) * velocity, (d[2] / len + tri()) * velocity]
}

#[allow(clippy::too_many_arguments)]
fn spawn_flying(game: &mut Game<'_>, dimension: &str, owner: u32, golem: bool, block: &str, pos: [f64; 3], vel: [f64; 3], gravity: f64, yaw: f32) {
    let state: Compound = [("Name".into(), Tag::String(block.into()))].into();
    let nbt: Compound = [("block_state".into(), Tag::Compound(state))].into();
    let display = game.native_summon(dimension, "minecraft:block_display", pos, nbt);
    let mut f = Flying { dimension: dimension.to_owned(), owner, golem, block: block.to_owned(), pos, vel, gravity, yaw, display, age: 0 };
    place_display(game, &mut f);
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).push(f);
}

/// The display where the block is, tumbling about its centre.
fn place_display(game: &mut Game<'_>, f: &mut Flying) {
    let Some(id) = f.display else { return };
    use glam::{Quat, Vec3};
    let t = f.age as f32;
    let scale = if f.golem { 1.0 } else { 0.75 };
    let yaw = if f.golem { f.yaw } else { 0.0 };
    let r = Quat::from_rotation_y((yaw + 45.0).to_radians())
        * Quat::from_rotation_x((t * 20.0).to_radians())
        * Quat::from_rotation_z((-t * 12.0).to_radians());
    let centre = r * Vec3::splat(0.5 * scale);
    let display = super::native_entities::NativeDisplay {
        translation: [-centre.x, 0.5 - centre.y, -centre.z],
        left_rotation: r.to_array(),
        scale: [scale; 3],
        interpolation: 1,
        teleport: 1,
        ..Default::default()
    };
    game.native_set_display(&f.dimension, id, &display);
    game.native_move_entity(&f.dimension, id, f.pos, 0.0, 0.0);
}

/// The thrown blocks of this level fly a tick, and burst on what they meet.
fn tick_flying(game: &mut Game<'_>, dimension: &str) {
    let mine: Vec<Flying> = {
        let mut all = FLYING.lock().unwrap_or_else(|e| e.into_inner());
        let (here, rest) = std::mem::take(&mut *all).into_iter().partition(|f| f.dimension == dimension);
        *all = rest;
        here
    };
    let mut keep = Vec::new();
    for mut f in mine {
        f.age += 1;
        let from = f.pos;
        let mut hit_block = None;
        let mut hit_body = None;
        for step in 1..=4 {
            let k = f64::from(step) / 4.0;
            let p = [from[0] + f.vel[0] * k, from[1] + f.vel[1] * k, from[2] + f.vel[2] * k];
            let b = [p[0] - 0.5, p[1], p[2] - 0.5, p[0] + 0.5, p[1] + 1.0, p[2] + 0.5];
            if let Some(body) = bodies(game, f.owner).into_iter().find(|body| meets(b, body.bounds) && flying_hits(game, &f, body.id)) {
                hit_body = Some((body.id, p));
                break;
            }
            let c = [p[0].floor() as i32, (p[1] + 0.5).floor() as i32, p[2].floor() as i32];
            if !is_air(game, c) && !no_collision(game, dimension, c) {
                // The face it came through: the last cell it was in.
                let q = [from[0] + f.vel[0] * (k - 0.25), from[1] + 0.5 + f.vel[1] * (k - 0.25), from[2] + f.vel[2] * (k - 0.25)];
                let before = q.map(|v| v.floor() as i32);
                hit_block = Some((c, if before == c { [c[0], c[1] + 1, c[2]] } else { before }, p));
                break;
            }
        }
        if let Some((target, p)) = hit_body {
            f.pos = p;
            flying_impact(game, dimension, &f, Some(target), None);
            continue;
        }
        if let Some((_, place, p)) = hit_block {
            f.pos = p;
            flying_impact(game, dimension, &f, None, Some(place));
            continue;
        }
        f.pos = [from[0] + f.vel[0], from[1] + f.vel[1], from[2] + f.vel[2]];
        let inertia = if fluid_at(game, f.pos.map(|v| v.floor() as i32)) == Some(false) { 0.8 } else { 0.99 };
        f.vel = [f.vel[0] * inertia, f.vel[1] * inertia - f.gravity, f.vel[2] * inertia];
        if f.age > 400 || f.pos[1] < -128.0 {
            if let Some(id) = f.display {
                game.native_remove_entity(dimension, id);
            }
            continue;
        }
        place_display(game, &mut f);
        keep.push(f);
    }
    FLYING.lock().unwrap_or_else(|e| e.into_inner()).extend(keep);
}

/// `ThrowableBlock.canHitEntity`: the golem's ice passes through all but enemies and those
/// after the golem; the enderman's blocks hit anything but itself.
fn flying_hits(game: &Game<'_>, f: &Flying, id: u32) -> bool {
    if !f.golem {
        return true;
    }
    let owner_target = ai::with_id(f.owner, |s| s.target).flatten();
    if owner_target == Some(id) {
        return true;
    }
    game.mobs.iter().find(|m| m.id == id).is_some_and(|m| {
        m.kind.category == "monster" && &*m.kind.id != ai::MINION || m.ai.target == Some(Who::Mob(f.owner))
    })
}

/// `ThrowableBlock.onHit`: the golem's ice shatters on mobs around (4 to 6) and the one hit
/// (4); the enderman's block is set down where it lands, or breaks, and its splash hurts what
/// is within 2 blocks (6 to 8).
fn flying_impact(game: &mut Game<'_>, dimension: &str, f: &Flying, target: Option<u32>, place: Option<[i32; 3]>) {
    if let Some(id) = f.display {
        game.native_remove_entity(dimension, id);
    }
    let at = f.pos;
    let centre = [at[0], at[1] + 0.5, at[2]];
    let state = game.native_default_state(&f.block);
    if f.golem {
        for body in bodies(game, f.owner) {
            let (dx, dy, dz) = (body.pos[0] - at[0], body.pos[1] - at[1], body.pos[2] - at[2]);
            let player = game.player.as_ref().is_some_and(|p| p.entity_id as u32 == body.id);
            if !player && dx * dx + dy * dy + dz * dz <= 6.25 && flying_hits(game, f, body.id) {
                game.native_hurt_entity(dimension, body.id, (4 + rand_int(3)) as f32, "minecraft:mob_projectile", Some(f.owner));
            }
        }
        if let Some(t) = target {
            game.native_hurt_entity(dimension, t, 4.0, "minecraft:thrown", Some(f.owner));
        }
        if let Some(state) = state {
            game.level_event_at(2001, centre.map(|v| v.floor() as i32), state.0 as i32);
        }
        let pitch = (rand_float() - rand_float()) * 0.2 + 0.8;
        game.native_play_sound_in_place("minecraft:block.glass.break", centre, 4, 0.8, pitch);
        return;
    }
    let griefing = griefing(game, dimension);
    if let Some(pos) = place {
        let replaceable = game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).replaceable;
        if griefing && replaceable && let Some(state) = state {
            game.native_set_block(dimension, pos, state, 3);
        } else if let Some(state) = state {
            game.level_event_at(2001, pos, state.0 as i32);
        }
    }
    if let Some(t) = target {
        game.native_hurt_entity(dimension, t, 4.0, "minecraft:thrown", Some(f.owner));
        if let Some(state) = state {
            game.level_event_at(2001, centre.map(|v| v.floor() as i32), state.0 as i32);
        }
    }
    for body in bodies(game, f.owner) {
        let (dx, dy, dz) = (body.pos[0] - at[0], body.pos[1] - at[1], body.pos[2] - at[2]);
        if dx * dx + dy * dy + dz * dz <= 4.0 {
            game.native_hurt_entity(dimension, body.id, (6 + rand_int(3)) as f32, "minecraft:mob_projectile", Some(f.owner));
        }
    }
}

// --- MutantEnderman ------------------------------------------------------------------

/// `EndersoulParticle` bursts (entity event 0): the mod's endersoul particles, as portal ones.
fn endersoul_burst(game: &mut Game<'_>, at: [f64; 3], count: i32) {
    let options = horizon_client::particles::ParticleOptions::simple(ParticleType::Portal);
    game.native_send_particles_in_place(&options, None, [at[0], at[1] + 1.5, at[2]], [0.6, 2.1, 0.6], 0.9, count);
}

/// `MutantEnderman.teleportAttack`: what is within 3 blocks is hurt (4), sometimes blinded,
/// and thrown out of the way.
fn teleport_attack(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3]) {
    let b = [at[0] - 3.6, at[1] - 3.0, at[2] - 3.6, at[0] + 3.6, at[1] + 7.2, at[2] + 3.6];
    let duration = 140 + rand_int(60);
    for body in bodies(game, mob) {
        if !meets(b, body.bounds) || is_ender_family(game, body.id) {
            continue;
        }
        if game.native_hurt_entity(dimension, body.id, 4.0, "minecraft:mob_attack", Some(mob)) && rand_int(3) == 0 {
            effect(game, body.id, "minecraft:blindness", duration, 0);
        }
        let (dx, dz) = (body.pos[0] - at[0], body.pos[2] - at[2]);
        let (sx, sz) = (dx.signum(), dz.signum());
        set_velocity(game, body.id, [(3.0 * sx * 2.0 - dx) * 0.2, 0.2, (3.0 * sz * 2.0 - dz) * 0.2]);
    }
}

fn is_ender_family(game: &Game<'_>, id: u32) -> bool {
    game.mobs.iter().find(|m| m.id == id).is_some_and(|m| matches!(&*m.kind.id, "minecraft:enderman" | ai::ENDERMAN | "minecraft:ender_dragon"))
}

/// `MutantEnderman.teleportToPosition`: the ground below `(x, y, z)` where it fits, as the
/// teleport's goal; the TELEPORT animation moves it there.
fn ender_teleport_to(game: &Game<'_>, dimension: &str, id: u32, x: f64, y: f64, z: f64) -> bool {
    let Some(m) = game.mobs.iter().find(|m| m.id == id) else { return false };
    let (w, h) = (m.kind.width, m.kind.height);
    let (bx, mut by, bz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    if !game.world.loaded(bx, bz) {
        return false;
    }
    let bottom = game.world.bottom_y();
    while by > bottom && no_collision(game, dimension, [bx, by, bz]) && fluid_at(game, [bx, by, bz]).is_none() {
        by -= 1;
    }
    by += 1;
    let (cx, cz) = (f64::from(bx) + 0.5, f64::from(bz) + 0.5);
    let fy = f64::from(by);
    let free = game.native_collision_boxes(dimension, [cx - w / 2.0, fy, cz - w / 2.0, cx + w / 2.0, fy + h, cz + w / 2.0]).is_empty();
    let dry = (0..=(h.ceil() as i32)).all(|dy| fluid_at(game, [bx, by + dy, bz]).is_none());
    if !free || !dry {
        return false;
    }
    ai::with_id(id, |s| {
        if s.animating() {
            return false;
        }
        s.anim = anim::ENDER_TELEPORT;
        s.teleport_to = Some([bx, by, bz]);
        true
    })
    .unwrap_or(false)
}

/// `teleportRandomly` (24 blocks) or `teleportToEntity` (16 blocks of it).
fn ender_teleport_near(game: &Game<'_>, dimension: &str, id: u32, target: Option<u32>) -> bool {
    let Some(at) = position(game, id) else { return false };
    let r = || rand() - 0.5;
    match target.and_then(|t| position(game, t).map(|p| (t, p))) {
        None => ender_teleport_to(game, dimension, id, at[0] + r() * 48.0, at[1] + f64::from(rand_int(48) - 24), at[2] + r() * 48.0),
        Some((_, p)) => {
            let d2 = (0..3).map(|i| (at[i] - p[i]).powi(2)).sum::<f64>();
            if d2 < 100.0 {
                ender_teleport_to(game, dimension, id, p[0] + r() * 32.0, p[1] + rand() * 16.0, p[2] + r() * 32.0)
            } else {
                let v = [at[0] - p[0], at[1] + 2.1 - (p[1] + 1.62), at[2] - p[2]];
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
                let v = v.map(|c| c / len);
                ender_teleport_to(game, dimension, id, at[0] + r() * 8.0 - v[0] * 16.0, at[1] + f64::from(rand_int(8)) - v[1] * 16.0, at[2] + r() * 8.0 - v[2] * 16.0)
            }
        }
    }
}

/// `teleportByChance`: one in `chance`, and not during another animation.
fn ender_teleport_chance(game: &Game<'_>, dimension: &str, id: u32, chance: i32, target: Option<u32>) -> bool {
    let busy = ai::with_id(id, |s| s.animating()).unwrap_or(true);
    !busy && rand_int(chance.max(1)) == 0 && ender_teleport_near(game, dimension, id, target)
}

/// `MutantEnderman.doHurtTarget`.
fn ender_attack(game: &mut Game<'_>, dimension: &str, mob: u32, target: Who) {
    let t = who_id(game, target);
    if ai::with_id(mob, |s| s.animating()).unwrap_or(true) {
        return;
    }
    if ender_teleport_chance(game, dimension, mob, 6, t) {
        return;
    }
    let pick = rand_int(4) as usize;
    let (r10, r7) = (rand_int(10), rand_int(7));
    ai::with_id(mob, |s| {
        // getAvailableHand: a random empty arm.
        let free: Vec<usize> = (0..4).filter(|&i| s.held[i].is_empty()).collect();
        if free.is_empty() {
            s.trigger_throw = true;
            return;
        }
        let arm = free[pick % free.len()];
        let upper_free = s.held[0].is_empty() && s.held[1].is_empty();
        // (The CLONE turn of 1 in 10 is left out: the clone mode isn't ported.)
        if upper_free && r10 != 0 && r7 == 0 {
            s.anim = anim::ENDER_TELESMASH;
        } else {
            s.active_arm = arm as u8;
            s.anim = anim::ENDER_MELEE;
        }
    });
}

/// `MutantEnderman.MeleeGoal` at tick 3: the active arm sweeps what is before it.
fn ender_swipe(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3]) {
    let Some(m) = game.mobs.iter().find(|m| m.id == mob) else { return };
    let (head, damage, min_y) = (m.head_yaw, m.attrs.attack_damage as f32, m.position.y);
    let lower = ai::with_id(mob, |s| s.active_arm >= 2).unwrap_or(false);
    let pitch = (rand_float() - rand_float()) * 0.2 + 1.0;
    sound(game, "minecraft:entity.player.attack.sweep", at, 1.0, pitch);
    for body in bodies(game, mob) {
        if is_ender_family(game, body.id) {
            continue;
        }
        let (dx, dy, dz) = (at[0] - body.pos[0], at[1] - body.pos[1], at[2] - body.pos[2]);
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if min_y > body.bounds[4] || dist > 4.0 || dist <= 0.0 || f64::from(head_angle(head, dx, dz)) >= 3.0 + (1.0 - dist / 4.0) * 40.0 {
            continue;
        }
        let amount = if damage > 0.0 { damage + if lower { 1.0 } else { 3.0 } } else { 0.0 };
        game.native_hurt_entity(dimension, body.id, amount, "minecraft:mob_attack", Some(mob));
        let power = 0.4 + rand() * 0.2 + if lower { 0.0 } else { 0.2 };
        set_velocity(game, body.id, [-dx / dist * power, power * 0.6, -dz / dist * power]);
    }
}

/// `ThrowBlockGoal.start`: the active arm's block flies at the target (or ahead).
fn ender_throw(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3], target: Option<Who>) {
    let Some((arm, block)) = ai::with_id(mob, |s| {
        let arm = usize::from(s.active_arm).min(3);
        (arm, std::mem::take(&mut s.held[arm]))
    }) else {
        return;
    };
    if block.is_empty() {
        return;
    }
    let outer = arm <= 1;
    let right = arm % 2 == 0;
    // Its offsets as if it faced south (the projectile's own yaw is still 0).
    let forward = if outer { 2.7 } else { 1.4 };
    let strafe = (if outer { 2.2 } else { 2.0 }) * if right { -1.0 } else { 1.0 };
    let from = [at[0] + strafe, at[1] + if outer { 2.7 } else { 1.0 }, at[2] + forward];
    let v = match target.and_then(|w| who_id(game, w)).and_then(|t| bodies(game, mob).into_iter().find(|b| b.id == t)) {
        Some(t) => {
            let (d0, d1, d2) = (t.pos[0] - from[0], t.pos[1] + (t.bounds[4] - t.bounds[1]) * 0.33 - from[1], t.pos[2] - from[2]);
            let d3 = (d0 * d0 + d2 * d2).sqrt();
            shoot([d0, d1 + d3 * 0.1, d2], 1.4, 1.0)
        }
        None => {
            let (yaw, pitch) = game.mobs.iter().find(|m| m.id == mob).map_or((0.0, 0.0), |m| (m.yaw, m.head_pitch));
            let (y, p) = (f64::from(yaw.to_radians()), f64::from(pitch.to_radians()));
            shoot([-y.sin() * p.cos() * 0.4, -p.sin() * 0.4, y.cos() * p.cos() * 0.4], 1.4, 1.0)
        }
    };
    spawn_flying(game, dimension, mob, false, &block, from, v, 0.01, 0.0);
}

/// `ScreamGoal` at tick 40: everything within 20 blocks is hurt (4) and, a mob, sickened.
fn ender_scream(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3]) {
    let pitch = 0.7 + rand_float() * 0.2;
    sound(game, "mutantmonsters:entity.mutant_enderman.scream", at, 5.0, pitch);
    endersoul_burst(game, at, 256);
    for body in bodies(game, mob) {
        let (dx, dy, dz) = (body.pos[0] - at[0], body.pos[1] - at[1], body.pos[2] - at[2]);
        if dx * dx + dy * dy + dz * dz >= 400.0 || is_ender_family(game, body.id) {
            continue;
        }
        game.native_hurt_entity(dimension, body.id, 4.0, "mutantmonsters:piercing_mob_attack", Some(mob));
        let player = game.player.as_ref().is_some_and(|p| p.entity_id as u32 == body.id);
        if player {
            continue;
        }
        effect(game, body.id, "minecraft:slowness", 120, 3);
        if rand_int(2) != 0 {
            effect(game, body.id, "minecraft:poison", 120 + rand_int(180), rand_int(2));
        }
        if rand_int(4) != 0 {
            effect(game, body.id, "minecraft:weakness", 300 + rand_int(300), rand_int(2));
        }
        if rand_int(3) != 0 {
            effect(game, body.id, "minecraft:hunger", 120 + rand_int(60), 10 + rand_int(2));
        }
        if rand_int(4) != 0 {
            effect(game, body.id, "minecraft:nausea", 120 + rand_int(400), 0);
        }
    }
}

/// `TeleSmashGoal` at tick 18: the target is lifted 7 blocks (13 a player) and hurt (6).
fn ender_telesmash(game: &mut Game<'_>, dimension: &str, mob: u32, target: Who) {
    let Some(t) = who_id(game, target) else { return };
    let Some(p) = position(game, t) else { return };
    let player = game.player.as_ref().is_some_and(|pl| pl.entity_id as u32 == t);
    let to = [p[0] + (rand() - 0.5) * 14.0, p[1] + rand() + if player { 13.0 } else { 7.0 }, p[2] + (rand() - 0.5) * 14.0];
    endersoul_burst(game, p, 256);
    if player {
        if let Some(pl) = game.native_player() {
            let who = game.native_acting_id();
            game.native_teleport_player(who, dimension, to, pl.yaw, pl.pitch);
        }
    } else {
        game.native_teleport_mob(dimension, t, to);
    }
    let pitch = 0.9 + rand_float() * 0.2;
    game.native_play_sound_in_place("minecraft:entity.generic.explode", to, HOSTILE, 1.2, pitch);
    game.native_hurt_entity(dimension, t, 6.0, "mutantmonsters:piercing_mob_attack", Some(mob));
}

/// `isBeingLookedAtBy` for the player: its view meets the enderman's eyes, in sight.
fn player_stares(game: &Game<'_>, at: [f64; 3], eye: f64) -> bool {
    let Some(p) = game.native_player() else { return false };
    if game.creative {
        return false;
    }
    let (yaw, pitch) = (f64::from(p.yaw.to_radians()), f64::from(p.pitch.to_radians()));
    let look = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let v = [at[0] - p.x, at[1] + eye - (p.y + 1.62), at[2] - p.z];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 96.0 || len <= 0.0 {
        return false;
    }
    let dot = (look[0] * v[0] + look[1] * v[1] + look[2] * v[2]) / len;
    dot > 1.0 - 0.08 / len
}

/// Blocks a mutant enderman can't pick up (`#mutantmonsters:mutant_enderman_holdable_immune`).
const HOLD_IMMUNE: [&str; 15] = [
    "minecraft:barrier",
    "minecraft:bedrock",
    "minecraft:end_portal",
    "minecraft:end_portal_frame",
    "minecraft:end_gateway",
    "minecraft:command_block",
    "minecraft:repeating_command_block",
    "minecraft:chain_command_block",
    "minecraft:structure_block",
    "minecraft:jigsaw",
    "minecraft:moving_piston",
    "minecraft:obsidian",
    "minecraft:crying_obsidian",
    "minecraft:respawn_anchor",
    "minecraft:reinforced_deepslate",
];

/// `MutantEnderman.aiStep` and `customServerAiStep`, and its 280-tick death.
fn ender_tick(game: &mut Game<'_>, dimension: &str, id: u32, st: &mut Mutant) {
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    let at = [m.position.x, m.position.y, m.position.z];
    let (aggressive, in_water, fall, eye) = (m.ai.aggressive, m.fluids.in_water, m.fall_distance, m.kind.eye_height);
    let path_done = m.ai.nav.is_done();
    if st.dying {
        // tickDeath: frozen but for falling; souls fly; the vortex pulls; loot rolls.
        m.velocity.x = 0.0;
        m.velocity.z = 0.0;
        m.velocity.y = m.velocity.y.min(0.0);
        m.nav_stop();
        m.body_yaw = st.death_yaw;
        st.death_time += 1;
        let t = st.death_time;
        ai::with_id(id, |s| {
            s.anim = anim::ENDER_DEATH;
            s.anim_tick = t;
        });
        if t == 80 {
            let pitch = (rand_float() - rand_float()) * 0.2 + 1.0;
            sound(game, "mutantmonsters:entity.mutant_enderman.death", at, 5.0, pitch);
        }
        if t >= 60 && rand_int(3) != 0 {
            endersoul_burst(game, [at[0], at[1] + 2.3, at[2]], 8);
        }
        if (80..260).contains(&t) {
            for body in bodies(game, id) {
                let (dx, dy, dz) = (at[0] - body.pos[0], at[1] - body.pos[1], at[2] - body.pos[2]);
                if dx.abs() > 10.6 || dy.abs() > 12.2 || dz.abs() > 10.6 || dx * dx + dy * dy + dz * dz <= 64.0 || is_ender_family(game, body.id) {
                    continue;
                }
                let d = (dx * dx + dz * dz).sqrt().max(1e-4);
                let mut vy = velocity(game, body.id)[1];
                if at[1] + 4.0 > body.pos[1] {
                    vy = vy.max(0.4);
                }
                set_velocity(game, body.id, [0.8 * dx / d, vy, 0.8 * dz / d]);
            }
        }
        if (100..150).contains(&t) && t % 6 == 0 {
            // `entities/mutant_enderman_continuous`: an ender pearl or an eye of ender.
            let item = if rand_int(2) == 0 { "minecraft:ender_pearl" } else { "minecraft:ender_eye" };
            game.native_run_command(&format!(
                "execute in {dimension} run summon minecraft:item {} {} {} {{Item:{{id:\"{item}\",count:1}}}}",
                at[0],
                at[1] + 3.84,
                at[2]
            ));
        }
        if t >= 280 {
            st.final_death = true;
            ai::with_id(id, |s| s.final_death = true);
            if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
                m.silent = true;
            }
            game.native_kill_mob(dimension, id);
        }
        return;
    }
    let target = st.target;
    let griefing = griefing(game, dimension);
    // updateTargetTick: the arms reach out while it fights; calm, it sets its blocks down.
    let (calm_drops, frenzy_pick) = ai::with_id(id, |s| {
        s.prev_arm_scale = s.arm_scale;
        if aggressive {
            s.has_target = 20;
        }
        let mut none = true;
        for i in 0..4 {
            if !s.held[i].is_empty() {
                none = false;
            }
            if s.has_target > 0 {
                if !s.held[i].is_empty() {
                    s.held_tick[i] = (s.held_tick[i] + 1).min(10);
                }
            } else {
                s.held_tick[i] = (s.held_tick[i] - 1).max(0);
            }
        }
        let mut drops = Vec::new();
        if s.has_target > 0 {
            s.arm_scale = (s.arm_scale + 1).min(10);
        } else if none {
            s.arm_scale = (s.arm_scale - 1).max(0);
        } else {
            drops = (0..4).filter(|&i| !s.held[i].is_empty() && s.held_tick[i] == 0).collect();
        }
        s.has_target = (s.has_target - 1).max(0);
        s.scream_delay = (s.scream_delay - 1).max(0);
        // updateBlockFrenzy.
        s.block_frenzy = (s.block_frenzy - 1).max(0);
        let mut pick = None;
        if target.is_some() && !s.animating() {
            let chance = if path_done { 300 } else { 600 };
            if s.block_frenzy == 0 && rand_int(chance) == 0 {
                s.block_frenzy = 200 + rand_int(80);
            }
            if s.block_frenzy > 0 && rand_int(8) == 0 {
                // getFavorableHand: an empty upper arm, else an empty lower one.
                let pair = |p: [usize; 2]| {
                    let free: Vec<usize> = p.into_iter().filter(|&i| s.held[i].is_empty()).collect();
                    (!free.is_empty()).then(|| free[rand_int(free.len() as i32) as usize])
                };
                pick = pair([0, 1]).or_else(|| pair([2, 3]));
            }
        }
        (drops, pick)
    })
    .unwrap_or_default();
    for arm in calm_drops {
        let pos = [
            (at[0] - 1.5 + rand() * 4.0).floor() as i32,
            (at[1] - 0.5 + rand() * 2.5).floor() as i32,
            (at[2] - 1.5 + rand() * 4.0).floor() as i32,
        ];
        let below = [pos[0], pos[1] - 1, pos[2]];
        let can_place = is_air(game, pos) && !is_air(game, below) && game.res.facts().get(game.world.block(below[0], below[1], below[2])).full_collision;
        let block = ai::with_id(id, |s| s.held[arm].clone()).unwrap_or_default();
        if griefing && can_place && let Some(state) = game.native_default_state(&block) {
            game.native_set_block(dimension, pos, state, 3);
            ai::with_id(id, |s| {
                s.held[arm].clear();
                s.held_tick[arm] = 0;
            });
        } else if !griefing || rand_int(50) == 0 {
            ai::with_id(id, |s| s.trigger_throw = true);
        }
    }
    if let Some(arm) = frenzy_pick {
        let pos = [
            (at[0] - 2.5 + rand() * 5.0).floor() as i32,
            (at[1] - 0.5 + rand() * 3.0).floor() as i32,
            (at[2] - 2.5 + rand() * 5.0).floor() as i32,
        ];
        let state = game.world.block(pos[0], pos[1], pos[2]);
        let facts = game.res.facts().get(state);
        let name = facts.name.to_string();
        if facts.full_collision && !facts.block_entity && !HOLD_IMMUNE.contains(&name.as_str()) && name != "minecraft:end_stone" {
            ai::with_id(id, |s| {
                s.held[arm] = name;
                s.held_tick[arm] = 0;
            });
            if griefing && let Some(air) = game.native_default_state("minecraft:air") {
                game.native_set_block(dimension, pos, air, 3);
            }
        }
    }
    // FindTargetGoal: a player who stares at it is its target, and the stare begins.
    let stares = player_stares(game, at, eye);
    let player_id = game.player.as_ref().map(|p| p.entity_id as u32);
    let starer = stares && player_id.is_some();
    if starer && target.is_none() {
        if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) {
            m.ai.target = Some(Who::Player);
        }
        ai::with_id(id, |s| {
            if !s.animating() {
                s.anim = anim::ENDER_STARE;
            }
        });
    }
    ai::with_id(id, |s| s.looked_at = starer && s.target.is_some() && s.target == player_id);
    // updateTeleport.
    let chance = if target.is_none() { 1600 } else { 800 };
    if !ender_teleport_chance(game, dimension, id, chance, target) {
        let far = target.and_then(|t| position(game, t)).is_some_and(|p| (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>() > 1024.0);
        if in_water || fall > 3.0 || (target.is_some() && (far || path_done)) {
            ender_teleport_chance(game, dimension, id, 10, target);
        }
    }
    let _ = st;
}

// --- MutantSnowGolem -----------------------------------------------------------------

/// `MutantSnowGolem.tick`: its throw's clock, and the snow it leaves and the water it
/// freezes where it walks.
fn golem_tick(game: &mut Game<'_>, dimension: &str, id: u32, st: &mut Mutant) {
    ai::with_id(id, |s| {
        if s.throwing {
            s.throwing_tick += 1;
            if s.throwing_tick >= 20 {
                s.throwing = false;
                s.throwing_tick = 0;
            }
        }
    });
    let Some(m) = game.mobs.iter().find(|m| m.id == id) else { return };
    let (on_ground, x, y, z) = (m.on_ground, m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32);
    if !on_ground || dimension == "minecraft:the_nether" || !griefing(game, dimension) {
        return;
    }
    let (Some(snow), Some(ice)) = (game.native_default_state("minecraft:snow"), game.native_default_state("minecraft:ice")) else { return };
    for i in -2i32..=2 {
        for j in -2i32..=2 {
            if i.abs() == 2 && j.abs() == 2 {
                continue;
            }
            let pos = [x + i, y, z + j];
            let below = [pos[0], y - 1, pos[2]];
            let ground = game.res.facts().get(game.world.block(below[0], below[1], below[2]));
            let place_snow = is_air(game, pos) && ground.full_collision;
            let place_ice = &*ground.name == "minecraft:water";
            let edge2 = i.abs() == 2 || j.abs() == 2;
            let edge1 = i.abs() == 1 || j.abs() == 1;
            let ok_snow = !place_snow || ((!edge2 || rand_int(20) == 0) && (!edge1 || rand_int(10) == 0));
            let ok_ice = !place_ice || ((!edge2 || rand_int(14) == 0) && (!edge1 || rand_int(6) == 0));
            if ok_snow && ok_ice {
                if place_snow {
                    game.native_set_block(dimension, pos, snow, 3);
                }
                if place_ice {
                    game.native_set_block(dimension, below, ice, 3);
                }
            }
        }
    }
    let _ = st;
}

// --- SpiderPig ---------------------------------------------------------------------------

/// `SpiderPig.doHurtTarget`: half the time a cobweb under its target (it hops), then the bite,
/// harder on a webbed target.
fn pig_bite(game: &mut Game<'_>, dimension: &str, mob: u32, target: Who) {
    let Some(t) = who_id(game, target) else { return };
    let Some(p) = position(game, t) else { return };
    ai::with_id(mob, |s| s.leaping = false);
    let v = velocity(game, t);
    if rand_int(2) == 0 && griefing(game, dimension) {
        let y = game.mobs.iter().find(|m| m.id == mob).map_or(p[1], |m| m.position.y).floor() as i32;
        let pos = [(p[0] + v[0] * 0.5) as i32, y, (p[2] + v[2] * 0.5) as i32];
        let facts = game.res.facts().get(game.world.block(pos[0], pos[1], pos[2]));
        if !facts.full_collision && fluid_at(game, pos).is_none() && &*facts.name != "minecraft:cobweb"
            && let Some(web) = game.native_default_state("minecraft:cobweb")
        {
            game.native_set_block(dimension, pos, web, 3);
            ai::with_id(mob, |s| {
                s.webs.push((pos, 1200));
                while s.webs.len() > 12 {
                    s.webs.remove(0);
                }
            });
            let my_v = velocity(game, mob);
            set_velocity(game, mob, [0.0, my_v[1].max(0.25), 0.0]);
        }
    }
    let damage = game.mobs.iter().find(|m| m.id == mob).map_or(3.0, |m| m.attrs.attack_damage as f32);
    let webbed = {
        let b = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        game.native_block(b) == "minecraft:cobweb" || game.native_block([b[0], b[1] + 1, b[2]]) == "minecraft:cobweb"
    };
    let spider = game.mobs.iter().find(|m| m.id == t).is_some_and(|m| matches!(&*m.kind.id, "minecraft:spider" | "minecraft:cave_spider" | ai::SPIDER_PIG));
    let amount = if webbed && !spider { damage + 4.0 } else { damage };
    game.native_hurt_entity(dimension, t, amount, "minecraft:mob_attack", Some(mob));
}

/// `SpiderPig.tick`: it climbs walls, its leap cools, its webs wear away.
fn pig_tick(game: &mut Game<'_>, dimension: &str, id: u32, st: &mut Mutant) {
    let Some(m) = game.mobs.iter_mut().find(|m| m.id == id) else { return };
    m.climbing = m.horizontal_collision;
    let on_ground = m.on_ground;
    let expired = ai::with_id(id, |s| {
        s.leap_cooldown = (s.leap_cooldown - 1).max(0);
        if s.leap_tick > 10 && on_ground {
            s.leaping = false;
        }
        for w in &mut s.webs {
            w.1 -= 1;
        }
        let mut gone = Vec::new();
        if s.webs.first().is_some_and(|w| w.1 < 0) {
            gone.push(s.webs.remove(0).0);
        }
        gone
    })
    .unwrap_or_default();
    if griefing(game, dimension) {
        for pos in expired {
            if game.native_block(pos) == "minecraft:cobweb" {
                let _ = game.handle_mob_events(vec![MobEvent::BreakBlock { pos, drop: false }]);
            }
        }
    }
    let _ = st;
}

/// `SpawnMinionsGoal.start`: 1 to 4 creeper minions lobbed at the target.
fn creeper_minions(game: &mut Game<'_>, dimension: &str, mob: u32, at: [f64; 3], target: Who) {
    let Some(t) = who_id(game, target).and_then(|t| position(game, t)) else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == mob) else { return };
    let n = (m.health / m.attrs.max_health * 4.0).ceil().max(1.0) as i32;
    let charged = ai::with_id(mob, |s| s.charged).unwrap_or(false);
    for _ in 0..n {
        let pos = [at[0] + rand() - rand(), at[1] + rand() * 0.5, at[2] + rand() - rand()];
        let v = [(t[0] - at[0]) * 0.15 + rand() * 0.05, (t[1] - at[1]) * 0.15 + rand() * 0.05, (t[2] - at[2]) * 0.15 + rand() * 0.05];
        let powered = if charged { ",powered:1b" } else { "" };
        game.native_run_command(&format!(
            "execute in {dimension} run summon {} {} {} {} {{Motion:[{}d,{}d,{}d],Fuse:26s,ExplosionRadius:2b{powered}}}",
            ai::MINION,
            pos[0],
            pos[1],
            pos[2],
            v[0],
            v[1],
            v[2]
        ));
    }
}

/// `if (level.isNight() && tickCount % 100 == 0 && isAlive() && health < maxHealth) heal(2)`.
fn night_heal(game: &mut Game<'_>, dimension: &str, id: u32) {
    if is_day(game) || dimension != "minecraft:overworld" {
        return;
    }
    if let Some(m) = game.mobs.iter_mut().find(|m| m.id == id)
        && m.age % 100 == 0
        && m.health < m.attrs.max_health
    {
        m.heal(2.0);
        m.needs_sync = true;
    }
}

fn wrap(mut d: f32) -> f32 {
    while d >= 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}
