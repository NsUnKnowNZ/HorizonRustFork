// More Babies for Horizon: baby versions of the mobs vanilla has none of.
use super::Game;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Commands wait for the level's tick: a player's hook can't run them.
static QUEUE: Mutex<Vec<Job>> = Mutex::new(Vec::new());

enum Job {
    Line(String),
    /// Turn the mob of this kind here into a baby, or a baby back.
    Toggle { dimension: String, kind: String, pos: [f64; 3] },
}

fn queue(line: String) {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(Job::Line(line));
}

pub(super) fn tick(game: &mut Game<'_>) {
    let jobs = std::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()));
    for job in jobs {
        match job {
            Job::Line(line) => {
                game.native_run_command(&line);
            }
            Job::Toggle { dimension, kind, pos } => {
                let baby = target(&kind, pos, &format!(",tag={TAG}"));
                let is_baby = game
                    .native_run_command_captured(&format!("execute in {dimension} {baby} run tag @s list"))
                    .result
                    > 0;
                let at = target(&kind, pos, "");
                let lines = if is_baby {
                    [
                        format!("execute in {dimension} {at} run attribute @s minecraft:scale base set 1"),
                        format!("execute in {dimension} {at} run attribute @s minecraft:movement_speed modifier remove more_babies:baby_speed"),
                        format!("execute in {dimension} {at} run tag @s remove {TAG}"),
                    ]
                } else {
                    [
                        format!("execute in {dimension} {at} run attribute @s minecraft:scale base set 0.5"),
                        format!("execute in {dimension} {at} run attribute @s minecraft:movement_speed modifier add more_babies:baby_speed 0.25 add_multiplied_base"),
                        format!("execute in {dimension} {at} run tag @s add {TAG}"),
                    ]
                };
                for line in lines {
                    game.native_run_command(&line);
                }
            }
        }
    }
}

const BABIES: [&str; 33] = [
    "bat", "blaze", "bogged", "breeze", "cave_spider", "creeper", "dolphin", "elder_guardian", "enderman",
    "evoker", "ghast", "giant", "glow_squid", "guardian", "illusioner", "iron_golem", "parrot", "piglin_brute",
    "pillager", "ravager", "salmon", "shulker", "skeleton", "snow_golem", "spider", "squid", "stray",
    "vindicator", "wandering_trader", "warden", "witch", "wither", "wither_skeleton"
];
/// The share of such mobs the world spawns as babies.
const CHANCE: f64 = 0.08;
const TAG: &str = "more_babies.baby";
const CLOCK: &str = "more_babies:cursed_clock_on_a_stick";

static SEED: AtomicU64 = AtomicU64::new(0xA076_1D64_78BD_642F);

fn rand() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn babies(kind: &str) -> bool {
    kind.strip_prefix("minecraft:").is_some_and(|k| BABIES.contains(&k))
}

/// The mob of `kind` at `pos`, as a command's target.
fn target(kind: &str, pos: [f64; 3], extra: &str) -> String {
    let [x, y, z] = pos;
    format!("positioned {x} {y} {z} as @e[type={kind},limit=1,sort=nearest,distance=..1{extra}]")
}

fn make_baby(dimension: &str, kind: &str, pos: [f64; 3]) {
    let at = target(kind, pos, "");
    for line in [
        format!("execute in {dimension} {at} run attribute @s minecraft:scale base set 0.5"),
        format!("execute in {dimension} {at} run attribute @s minecraft:movement_speed modifier add more_babies:baby_speed 0.25 add_multiplied_base"),
        format!("execute in {dimension} {at} run tag @s add {TAG}"),
    ] {
        queue(line);
    }
}

pub(super) fn spawned(_game: &mut Game<'_>, dimension: &str, kind: &str, pos: [f64; 3], natural: bool) {
    if natural && babies(kind) && rand() < CHANCE {
        make_baby(dimension, kind, pos);
    }
}

/// The Cursed Clock On A Stick on a mob: grown to baby, or baby to grown.
pub(super) fn interact(
    game: &mut Game<'_>,
    entity: i32,
    kind: &str,
    stack: Option<&horizon_client::items::ItemStack>,
    claimed: &mut bool,
) {
    if *claimed || stack.is_none_or(|s| &*s.item != CLOCK) || !babies(kind) || game.remote.is_some() {
        return;
    }
    let dimension = game.dim.id().to_owned();
    let Some(mob) = game.native_mob(&dimension, entity as u32) else { return };
    *claimed = true;
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(Job::Toggle {
        dimension: dimension.clone(),
        kind: kind.to_owned(),
        pos: mob.pos,
    });
    let at = mob.pos;
    game.native_play_local_sound("minecraft:block.bell.resonate", at, 0.6, 1.6, 0);
}
