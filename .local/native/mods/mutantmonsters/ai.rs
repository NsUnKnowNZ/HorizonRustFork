// Mutant Monsters (Fuzs, AGPL-3.0, local use) for Horizon: the mutants' goals (each Java
// `registerGoals` with its priorities and flags), their sounds, and the state the goals and
// the game half share. Lives in horizon-client's library (`horizon_client::mutantmonsters_ai`):
// the goals run inside the mobs' AI there, and the game half (`game::mutantmonsters`) reads the
// same state each level tick for what needs the world (blocks, explosions, damage, spawns).
// Goals tick only on the server, so everything here is the server's; a client draws from what
// the game half syncs (`mutantmonsters:state`).
use crate::mobs::goals::{self, Cx, Goal, JUMP, LOOK, MOVE};
use crate::mobs::{Mob, Species, Who};
use horizon_core::random::RandomSource;
use std::collections::HashMap;
use std::sync::Mutex;

pub const ZOMBIE: &str = "mutantmonsters:mutant_zombie";
pub const SKELETON: &str = "mutantmonsters:mutant_skeleton";
pub const CREEPER: &str = "mutantmonsters:mutant_creeper";
pub const ENDERMAN: &str = "mutantmonsters:mutant_enderman";
pub const SNOW_GOLEM: &str = "mutantmonsters:mutant_snow_golem";
pub const SPIDER_PIG: &str = "mutantmonsters:spider_pig";
pub const MINION: &str = "mutantmonsters:creeper_minion";

/// `EntityAnimation`s by the numbers the models take (`*_ANIMS`, from 1; 0 is none).
pub mod anim {
    pub const NONE: u8 = 0;
    pub const ZOMBIE_SLAM: u8 = 1;
    pub const ZOMBIE_ROAR: u8 = 2;
    pub const ZOMBIE_THROW: u8 = 3;
    /// Their durations (`EntityAnimation(duration)`).
    pub const fn zombie_duration(a: u8) -> i32 {
        match a {
            ZOMBIE_SLAM => 25,
            ZOMBIE_ROAR => 120,
            ZOMBIE_THROW => 15,
            _ => 0,
        }
    }
    /// An animation's duration for a mutant of `kind`.
    pub const fn duration(kind: super::Kind, a: u8) -> i32 {
        match kind {
            super::Kind::Zombie => zombie_duration(a),
            super::Kind::Skeleton => skeleton_duration(a),
            super::Kind::Enderman => ender_duration(a),
            // The snow golem's throw (`throwingTick`, 20 ticks).
            super::Kind::SnowGolem => 20,
            _ => 0,
        }
    }
    pub const ENDER_MELEE: u8 = 1;
    pub const ENDER_THROW: u8 = 2;
    pub const ENDER_SCREAM: u8 = 3;
    pub const ENDER_TELESMASH: u8 = 4;
    pub const ENDER_DEATH: u8 = 5;
    pub const ENDER_STARE: u8 = 6;
    pub const ENDER_TELEPORT: u8 = 7;
    pub const ENDER_CLONE: u8 = 8;
    pub const fn ender_duration(a: u8) -> i32 {
        match a {
            ENDER_MELEE => 10,
            ENDER_THROW => 14,
            ENDER_SCREAM => 165,
            ENDER_TELESMASH => 30,
            ENDER_DEATH => 280,
            ENDER_STARE => 100,
            ENDER_TELEPORT => 10,
            ENDER_CLONE => 600,
            _ => 0,
        }
    }
    pub const SKELETON_MELEE: u8 = 1;
    pub const SKELETON_SHOOT: u8 = 2;
    pub const SKELETON_MULTI_SHOT: u8 = 3;
    pub const SKELETON_CONSTRICT: u8 = 4;
    pub const fn skeleton_duration(a: u8) -> i32 {
        match a {
            SKELETON_MELEE => 14,
            SKELETON_SHOOT => 32,
            SKELETON_MULTI_SHOT => 30,
            SKELETON_CONSTRICT => 20,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zombie,
    Skeleton,
    Creeper,
    Enderman,
    SnowGolem,
    SpiderPig,
    Minion,
}

pub fn kind_of(id: &str) -> Option<Kind> {
    Some(match id {
        ZOMBIE => Kind::Zombie,
        SKELETON => Kind::Skeleton,
        CREEPER => Kind::Creeper,
        ENDERMAN => Kind::Enderman,
        SNOW_GOLEM => Kind::SnowGolem,
        SPIDER_PIG => Kind::SpiderPig,
        MINION => Kind::Minion,
        _ => return None,
    })
}

/// A queued seismic wave (`SeismicWave`): the ground block, whether it is the first of its
/// slam (bigger and harder) and whether it changes the terrain.
#[derive(Clone, Copy, Debug)]
pub struct Wave {
    pub pos: [i32; 3],
    pub first: bool,
    pub terrain: bool,
}

/// What one mutant is doing, by its id: the fields of its Java class the goals and the
/// game half share. The server's only.
#[derive(Clone, Debug)]
pub struct Mutant {
    pub kind: Kind,
    /// `animation` and `animationTick` (`AnimatedEntity`).
    pub anim: u8,
    pub anim_tick: i32,
    /// Its own death (`die` without `super.die`): it lies or shrinks for `death_time`.
    pub dying: bool,
    pub death_time: i32,
    /// The real death is under way (`super.die`): let the engine have it.
    pub final_death: bool,
    /// The attack target's entity id, which the game half finds each tick.
    pub target: Option<u32>,
    // MutantZombie
    pub lives: i32,
    pub vanish: i32,
    pub throw_hit: bool,
    pub throw_finished: bool,
    /// `throwHitTick`: ticks since the throw hit (-1 before).
    pub throw_hit_tick: i32,
    pub waves: Vec<Wave>,
    /// `ZombieResurrection`s: the ground block and the ticks left.
    pub resurrections: Vec<([i32; 3], i32)>,
    /// The body's turn when it fell, held while it lies.
    pub death_yaw: f32,
    // MutantCreeper
    pub charged: bool,
    pub jump_attacking: bool,
    pub charging: bool,
    pub charge_time: i32,
    pub charge_hits: i32,
    pub jump_tick: i32,
    pub last_jump_tick: i32,
    pub summon_lightning: bool,
    /// Whether a player hurt it last (`lastHurtByPlayer`), for the minion egg.
    pub killer: Option<u128>,
    // MutantEnderman
    /// `ACTIVE_ARM`: the arm of the swing or throw.
    pub active_arm: u8,
    /// `armScale` (0 to 10) and the tick before's.
    pub arm_scale: i32,
    pub prev_arm_scale: i32,
    /// `hasTarget`: ticks since it last was aggressive, from 20.
    pub has_target: i32,
    /// `heldBlock`: the block each of the four arms holds ("" none), and `heldBlockTick`.
    pub held: [String; 4],
    pub held_tick: [i32; 4],
    pub trigger_throw: bool,
    pub block_frenzy: i32,
    pub scream_delay: i32,
    /// `TELEPORT_POSITION`: where its teleport goes.
    pub teleport_to: Option<[i32; 3]>,
    /// Whether the target is looking at it (`isBeingLookedAtBy`), as the game half found.
    pub looked_at: bool,
    // MutantSnowGolem
    pub throwing: bool,
    pub throwing_tick: i32,
    pub swim_jump: bool,
    // SpiderPig
    pub leap_cooldown: i32,
    pub leap_tick: i32,
    pub leaping: bool,
    /// Its cobwebs and the ticks each has left (`WebPos`).
    pub webs: Vec<([i32; 3], i32)>,
    /// Model extras (`e` of the generated animations).
    pub e: [f32; 12],
}

impl Mutant {
    pub fn new(kind: Kind, random: &mut impl RandomSource) -> Self {
        Self {
            kind,
            anim: 0,
            anim_tick: 0,
            dying: false,
            death_time: 0,
            final_death: false,
            target: None,
            lives: 3,
            vanish: 0,
            throw_hit: false,
            throw_finished: false,
            throw_hit_tick: -1,
            waves: Vec::new(),
            resurrections: Vec::new(),
            death_yaw: 0.0,
            charged: false,
            jump_attacking: false,
            charging: false,
            charge_time: 0,
            charge_hits: 3 + random.next_int_bound(3).unwrap_or(0),
            jump_tick: 0,
            last_jump_tick: 0,
            summon_lightning: false,
            killer: None,
            active_arm: 0,
            arm_scale: 0,
            prev_arm_scale: 0,
            has_target: 0,
            held: Default::default(),
            held_tick: [0; 4],
            trigger_throw: false,
            block_frenzy: 0,
            scream_delay: 0,
            teleport_to: None,
            looked_at: false,
            throwing: false,
            throwing_tick: 0,
            swim_jump: false,
            leap_cooldown: 0,
            leap_tick: 0,
            leaping: false,
            webs: Vec::new(),
            e: [0.0; 12],
        }
    }
    /// `isAnimationPlaying`.
    pub fn animating(&self) -> bool {
        self.anim != anim::NONE
    }
    /// `sendAnimationPacket`: the animation from its first tick.
    pub fn play(&mut self, a: u8) {
        self.anim = a;
        self.anim_tick = 0;
    }
}

/// Every mutant the server knows, by id.
pub static SERVER: Mutex<Option<HashMap<u32, Mutant>>> = Mutex::new(None);

/// Runs `f` on mob `m`'s state, made on first use.
pub fn with<R>(m: &mut Mob, f: impl FnOnce(&mut Mutant) -> R) -> Option<R> {
    let kind = kind_of(&m.kind.id)?;
    let mut all = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    let st = all.get_or_insert_with(HashMap::new).entry(m.id).or_insert_with(|| Mutant::new(kind, &mut m.random));
    Some(f(st))
}

/// Mob `id`'s state, when it has one.
pub fn with_id<R>(id: u32, f: impl FnOnce(&mut Mutant) -> R) -> Option<R> {
    let mut all = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    all.get_or_insert_with(HashMap::new).get_mut(&id).map(f)
}

fn dying(m: &mut Mob) -> bool {
    with(m, |s| s.dying).unwrap_or(false)
}

/// What a goal hands the game half for the world.
#[derive(Clone, Debug)]
pub enum Job {
    /// `playSound` at the mob.
    Sound { mob: u32, name: &'static str, volume: f32, pitch: f32 },
    /// `doHurtTarget`: the mob's own melee hit.
    Melee { mob: u32, target: Who },
    /// `SlamGroundGoal` at tick 12: waves from `from` to `to` (blocks, x and z) at `y`.
    Slam { mob: u32, from: [i32; 2], to: [i32; 2], y: i32 },
    /// `RoarGoal` at tick 10: the roar's push and hurt around the mob.
    Roar { mob: u32 },
    /// `RoarGoal` from tick 20: a zombie may rise near `at`.
    Resurrect { mob: u32, at: [i32; 3] },
    /// `ThrowAttackGoal.start`: the target is flung up.
    Fling { mob: u32, target: Who },
    /// The throw lands on the target.
    ThrowHit { mob: u32, target: Who },
    /// `ChargeAttackGoal.stop`: the bolt, or the heal of a full charge.
    ChargeEnd { mob: u32, lightning: bool, heal: bool },
    /// `SpawnMinionsGoal.start`.
    Minions { mob: u32, target: Who },
    /// `MutantSkeleton.MeleeGoal` at tick 3: the sweep of its arm.
    Sweep { mob: u32 },
    /// `ConstrictRibsGoal` at tick 6: the target crushed and flung.
    Constrict { mob: u32, target: Who },
    /// A `MutantArrow` at `target`: `count` of them at `speed` with `spread`, and the
    /// single shot's chance of effects.
    Arrows { mob: u32, target: Who, count: u32, speed: f32, spread: f32, effects: bool },
    /// `MutantEnderman.doHurtTarget`: teleport near the target, or pick a swing, a smash or a throw.
    EnderAttack { mob: u32, target: Who },
    /// `MutantEnderman.MeleeGoal` at tick 3: the swipe of the active arm.
    EnderSwipe { mob: u32 },
    /// `ThrowBlockGoal.start`: the block of the active arm flies.
    EnderThrow { mob: u32, target: Option<Who> },
    /// `StareGoal.stop`: the one who stared is hurt, blinded and pulled.
    StareEnd { mob: u32, target: Who },
    /// `TeleportGoal.start` and `stop`.
    TeleportStart { mob: u32 },
    TeleportEnd { mob: u32 },
    /// `ScreamGoal` at tick 40.
    Scream { mob: u32 },
    /// `TeleSmashGoal`: its start (slowness and nausea) and tick 18 (lifted and hurt).
    TeleSmashStart { mob: u32, target: Who },
    TeleSmash { mob: u32, target: Who },
    /// `MutantSnowGolem.ThrowIceGoal` at throwing tick 7: an ice block lobbed at the target.
    ThrowIce { mob: u32, target: Who },
    /// `SwimJumpGoal.start`: out of the water, hurt by it.
    SwimJump { mob: u32 },
    /// `SpiderPig.doHurtTarget`: maybe a web, then the bite.
    PigBite { mob: u32, target: Who },
}

pub static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

pub fn push(job: Job) {
    JOBS.lock().unwrap_or_else(|e| e.into_inner()).push(job);
}

/// `0.8 + rand.nextFloat() * 0.4`, the mod's usual pitch.
fn pitch(m: &mut Mob) -> f32 {
    0.8 + m.random.next_float() * 0.4
}

// --- shared goals ---------------------------------------------------------

/// A vanilla goal that rests while its mutant lies dead (vanilla runs no AI for a mob
/// that `isDeadOrDying`).
#[derive(Clone, Debug)]
struct Live<G>(G);
impl<G: Goal + Clone + 'static> Goal for Live<G> {
    fn flags(&self) -> u8 {
        self.0.flags()
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        !dying(m) && self.0.can_use(m, cx)
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        !dying(m) && self.0.can_continue(m, cx)
    }
    fn interruptable(&self) -> bool {
        self.0.interruptable()
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.0.start(m, cx);
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.0.stop(m, cx);
    }
    fn every_tick(&self) -> bool {
        self.0.every_tick()
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.0.tick(m, cx);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        self.0.name()
    }
}

/// `MutantMeleeAttackGoal`: chases its target with no path check and hits it
/// (`doHurtTarget`) whenever it is in reach, every `max_attack_tick` ticks.
#[derive(Clone, Debug)]
pub struct MutantMelee {
    speed: f64,
    max_attack_tick: i32,
    recalc: i32,
    attack_tick: i32,
}
impl MutantMelee {
    pub fn new(speed: f64) -> Self {
        Self { speed, max_attack_tick: 20, recalc: 0, attack_tick: 0 }
    }
    pub fn max_attack_tick(mut self, ticks: i32) -> Self {
        self.max_attack_tick = ticks;
        self
    }
}
impl Goal for MutantMelee {
    fn flags(&self) -> u8 {
        MOVE | LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if dying(m) {
            return false;
        }
        let Some(t) = m.target_info(cx.w) else {
            return false;
        };
        if !t.alive {
            m.ai.target = None;
            return false;
        }
        true
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if dying(m) {
            return false;
        }
        let Some(t) = m.target_info(cx.w) else {
            return false;
        };
        t.alive && !(t.is_player && t.creative)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        m.ai.aggressive = true;
        self.recalc = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
        if m.ai.target.is_none() {
            m.ai.aggressive = false;
        }
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(who) = m.ai.target else { return };
        let Some(t) = cx.w.target_info(who) else { return };
        m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
        self.recalc -= 1;
        if self.recalc <= 0 {
            self.recalc = 4 + m.random.next_int_bound(7).unwrap_or(0);
            m.nav_move_to_entity(cx.w, [t.position.x, t.position.y, t.position.z], 0, self.speed);
        }
        self.attack_tick = (self.attack_tick - 1).max(0);
        let reaches = m.within_melee_range(cx.w, &t) || m.bounds().intersects(t.bounds);
        if reaches && self.attack_tick <= 0 && m.sees(cx.w, who) {
            self.attack_tick = self.max_attack_tick;
            m.swing();
            do_hurt_target(m, who);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// Each mutant's `doHurtTarget`.
fn do_hurt_target(m: &mut Mob, who: Who) {
    match kind_of(&m.kind.id) {
        // MutantZombie.doHurtTarget: picks an attack animation, never hurts by itself.
        Some(Kind::Zombie) => {
            let on_ground = m.on_ground || m.fluids.in_water || m.fluids.in_lava;
            let throw = m.random.next_int_bound(5).unwrap_or(1) == 0;
            with(m, |s| {
                if !s.animating() {
                    if throw {
                        s.anim = anim::ZOMBIE_THROW;
                    } else if on_ground {
                        s.anim = anim::ZOMBIE_SLAM;
                    }
                }
            });
        }
        // MutantSkeleton.doHurtTarget: the sweep three times in four, else the ribs.
        Some(Kind::Skeleton) => {
            let melee = m.random.next_int_bound(4).unwrap_or(0) != 0;
            with(m, |s| {
                if !s.animating() {
                    s.anim = if melee { anim::SKELETON_MELEE } else { anim::SKELETON_CONSTRICT };
                }
            });
        }
        // MutantEnderman.doHurtTarget: the game half picks (it may teleport instead).
        Some(Kind::Enderman) => push(Job::EnderAttack { mob: m.id, target: who }),
        Some(Kind::SpiderPig) => push(Job::PigBite { mob: m.id, target: who }),
        _ => push(Job::Melee { mob: m.id, target: who }),
    }
}

/// `AvoidDamageGoal` (a `PanicGoal`): burning, it looks for water; hurt by something with no
/// attacker (a cactus, fire, a fall), it walks away from where it came from.
#[derive(Clone, Debug)]
pub struct AvoidDamage {
    speed: f64,
    to: [f64; 3],
}
impl AvoidDamage {
    pub fn new(speed: f64) -> Self {
        Self { speed, to: [0.0; 3] }
    }
}
impl Goal for AvoidDamage {
    fn flags(&self) -> u8 {
        MOVE
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if dying(m) {
            return false;
        }
        let burning = m.is_on_fire();
        let recent = m.hurt_time > 0
            && m.last_damage.as_ref().is_some_and(|(d, _)| d.attacker.is_none() && !matches!(d.kind, crate::mobs::DamageKind::Drown | crate::mobs::DamageKind::Starve | crate::mobs::DamageKind::Fall | crate::mobs::DamageKind::OutOfWorld));
        if !burning && !recent {
            return false;
        }
        let away = m.last_damage.and_then(|(d, _)| d.from);
        let pos = match away {
            Some(from) if !burning => m.default_random_pos_away(cx.w, 8, 5, from),
            _ => None,
        }
        .or_else(|| m.default_random_pos(cx.w, 5, 4));
        match pos {
            Some(p) => {
                self.to = p;
                true
            }
            None => false,
        }
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        !m.ai.nav.is_done()
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        m.nav_move_to(cx.w, self.to, self.speed);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// The vanilla goals every mutant ends its list with, resting while it lies dead.
fn wander(m: &mut Mob, from: i32, look: bool) {
    let g = &mut m.ai.goals;
    g.add(from, Live(goals::Stroll::avoiding_water(1.0)));
    if look {
        g.add(from + 1, Live(goals::LookAtPlayer::new(8.0)));
        g.add(from + 2, Live(goals::RandomLookAround::default()));
    }
}

// --- MutantZombie -----------------------------------------------------------

/// `MutantZombie.SlamGroundGoal`: a stomp that sends seismic waves at the target.
#[derive(Clone, Debug)]
struct Slam {
    dir: [f64; 2],
}
impl Goal for Slam {
    fn flags(&self) -> u8 {
        MOVE | LOOK | JUMP
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.ai.target.is_some() && with(m, |s| !s.dying && s.anim == anim::ZOMBIE_SLAM).unwrap_or(false)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        with(m, |s| !s.dying && s.anim == anim::ZOMBIE_SLAM && s.anim_tick < anim::zombie_duration(anim::ZOMBIE_SLAM))
            .unwrap_or(false)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::ZOMBIE_SLAM));
        m.ambient_timer = -m.ambient_interval();
        let pitch = pitch(m);
        push(Job::Sound { mob: m.id, name: "mutantmonsters:entity.mutant_zombie.attack", volume: 0.3, pitch });
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(t) = m.target_info(cx.w) else { return };
        m.nav_stop();
        let tick = with(m, |s| s.anim_tick).unwrap_or(0);
        if tick < 8 {
            m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
        }
        if tick == 8 {
            let (dx, dz) = (t.position.x - m.position.x, t.position.z - m.position.z);
            let d = (dx * dx + dz * dz).sqrt();
            self.dir = [dx / d, dz / d];
        }
        if tick == 12 {
            let [x, z] = [m.position.x, m.position.z];
            let from = [(x + self.dir[0] * 2.0).floor() as i32, (z + self.dir[1] * 2.0).floor() as i32];
            let to = [(x + self.dir[0] * 8.0).floor() as i32, (z + self.dir[1] * 8.0).floor() as i32];
            let y = m.position.y.floor() as i32;
            push(Job::Slam { mob: m.id, from, to, y });
            let pitch = pitch(m);
            push(Job::Sound { mob: m.id, name: "minecraft:entity.generic.explode", volume: 0.5, pitch });
        }
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::NONE));
        self.dir = [-1.0, -1.0];
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantZombie.RoarGoal`: now and then it roars, throwing back what is near, and calls
/// zombies out of the ground.
#[derive(Clone, Debug)]
struct Roar;
impl Goal for Roar {
    fn flags(&self) -> u8 {
        MOVE | LOOK | JUMP
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if m.age % 3 != 0 || !m.on_ground {
            return false;
        }
        let Some(t) = m.target_info(cx.w) else { return false };
        let free = with(m, |s| !s.dying && !s.animating() && s.resurrections.is_empty()).unwrap_or(false);
        free && m.distance_sqr(t.position) > 16.0 && m.random.next_float() * 100.0 < 0.35
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        with(m, |s| !s.dying && s.anim == anim::ZOMBIE_ROAR && s.anim_tick < anim::zombie_duration(anim::ZOMBIE_ROAR))
            .unwrap_or(false)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::ZOMBIE_ROAR));
        m.invulnerable = 20;
        m.ambient_timer = -m.ambient_interval();
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        m.nav_stop();
        let tick = with(m, |s| s.anim_tick).unwrap_or(0);
        if tick < 75
            && let Some(t) = m.target_info(cx.w)
        {
            m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
        }
        if tick == 10 {
            let pitch = 0.7 + m.random.next_float() * 0.2;
            push(Job::Sound { mob: m.id, name: "mutantmonsters:entity.mutant_zombie.roar", volume: 3.0, pitch });
            push(Job::Roar { mob: m.id });
        }
        if (20..80).contains(&tick) && tick % 10 == 0 {
            let mut off = || {
                let n = 1 + m.random.next_int_bound(8).unwrap_or(0);
                if m.random.next_boolean() { n } else { -n }
            };
            let x = m.position.x.floor() as i32 + off();
            let z = m.position.z.floor() as i32 + off();
            let y = m.position.y.floor() as i32;
            push(Job::Resurrect { mob: m.id, at: [x, y, z] });
        }
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::NONE));
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantZombie.ThrowAttackGoal`: flings its target up, leaps after it and smashes it down.
#[derive(Clone, Debug)]
struct Throw {
    finish: i32,
}
impl Goal for Throw {
    fn flags(&self) -> u8 {
        MOVE | LOOK | JUMP
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.ai.target.is_some() && with(m, |s| !s.dying && s.anim == anim::ZOMBIE_THROW).unwrap_or(false)
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if self.finish >= 10 || dying(m) {
            return false;
        }
        match m.target_info(cx.w) {
            Some(t) => t.alive && !(t.is_player && t.creative),
            None => false,
        }
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::ZOMBIE_THROW));
        if let Some(target) = m.ai.target {
            push(Job::Fling { mob: m.id, target });
        }
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(who) = m.ai.target else { return };
        let Some(t) = cx.w.target_info(who) else { return };
        m.nav_stop();
        m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
        let tick = with(m, |s| s.anim_tick).unwrap_or(0);
        if tick == 15 {
            m.stuck_multiplier = horizon_core::geometry::Vec3::ZERO;
            let (dx, dy, dz) = (t.position.x - m.position.x, t.position.y - m.position.y, t.position.z - m.position.z);
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            m.velocity = horizon_core::geometry::Vec3::new(dx / d * 3.4, dy / d * 1.4, dz / d * 3.4);
            m.needs_sync = true;
        } else if tick > 15 {
            let reach = f64::from((m.kind.width as f32 * 2.0) * (m.kind.width as f32 * 2.0));
            let (dx, dy, dz) = (t.position.x - m.position.x, t.bounds.min(horizon_core::geometry::Axis::Y) - m.position.y, t.position.z - m.position.z);
            let hit = with(m, |s| s.throw_hit).unwrap_or(true);
            if dx * dx + dy * dy + dz * dz < reach && !hit {
                with(m, |s| s.throw_hit = true);
                push(Job::ThrowHit { mob: m.id, target: who });
                let pitch = pitch(m);
                push(Job::Sound { mob: m.id, name: "mutantmonsters:entity.mutant_zombie.grunt", volume: 0.3, pitch });
            }
            let landed = m.on_ground || m.fluids.in_water || m.fluids.in_lava;
            if landed && with(m, |s| !s.throw_finished).unwrap_or(false) {
                self.finish = 0;
                with(m, |s| s.throw_finished = true);
            }
            if self.finish >= 0 {
                self.finish += 1;
            }
        }
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| {
            s.play(anim::NONE);
            s.throw_hit = false;
            s.throw_finished = false;
        });
        self.finish = -1;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `TrackSummonerGoal`: a zombie the roar called takes its summoner's target, and keeps
/// near it (8 blocks, 16 with a target) while the summoner lives.
#[derive(Clone, Debug)]
pub struct TrackSummoner {
    summoner: u32,
}
impl TrackSummoner {
    pub fn new(summoner: u32) -> Self {
        Self { summoner }
    }
}
impl Goal for TrackSummoner {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, _: &mut Mob, cx: &mut Cx) -> bool {
        cx.w.seen(self.summoner).is_some_and(|s| s.alive)
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(s) = cx.w.seen(self.summoner) else { return };
        let hurt_by = m.ai.last_hurt_by.is_some();
        if !hurt_by && s.target.is_some() && m.ai.target != s.target {
            m.ai.target = s.target;
        }
        let range = if m.ai.target.is_none() { 8.0 } else { 16.0 };
        if m.ai.target.is_none() && m.distance_sqr(s.position) > range * range {
            m.nav_move_to(cx.w, [s.position.x, s.position.y, s.position.z], 1.0);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- MutantSkeleton ---------------------------------------------------------

/// The `AnimationGoal`s of the mutant skeleton: each plays its animation while it runs.
#[derive(Clone, Debug)]
struct SkeletonAttack {
    anim: u8,
}
impl SkeletonAttack {
    fn playing(&self, m: &mut Mob) -> bool {
        with(m, |s| !s.dying && s.anim == self.anim).unwrap_or(false)
    }
}
impl Goal for SkeletonAttack {
    fn flags(&self) -> u8 {
        if self.anim == anim::SKELETON_MULTI_SHOT { MOVE | LOOK | JUMP } else { MOVE | LOOK }
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        match self.anim {
            anim::SKELETON_MELEE => "MeleeGoal",
            anim::SKELETON_SHOOT => "ShootGoal",
            anim::SKELETON_MULTI_SHOT => "MultiShotGoal",
            _ => "ConstrictRibsGoal",
        }
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        match self.anim {
            // MeleeGoal: triggered by doHurtTarget, with or without a target.
            anim::SKELETON_MELEE => self.playing(m),
            anim::SKELETON_CONSTRICT => m.ai.target.is_some() && self.playing(m),
            // ShootGoal: one in 12, beyond 2 blocks, in sight.
            anim::SKELETON_SHOOT => {
                let Some(who) = m.ai.target else { return false };
                let Some(t) = cx.w.target_info(who) else { return false };
                let idle = with(m, |s| !s.dying && !s.animating()).unwrap_or(false);
                idle && m.random.next_int_bound(12).unwrap_or(1) == 0 && m.distance_sqr(t.position) > 4.0 && m.sees(cx.w, who)
            }
            // MultiShotGoal: one in 26 on the ground, every third tick, in sight.
            _ => {
                let Some(who) = m.ai.target else { return false };
                let idle = with(m, |s| !s.dying && !s.animating()).unwrap_or(false);
                m.age % 3 == 0 && idle && m.on_ground && m.random.next_int_bound(26).unwrap_or(1) == 0 && m.sees(cx.w, who)
            }
        }
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        let anim = self.anim;
        with(m, |s| !s.dying && s.anim == anim && s.anim_tick < anim::skeleton_duration(anim)).unwrap_or(false)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        let anim = self.anim;
        with(m, |s| s.play(anim));
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.play(anim::NONE));
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let tick = with(m, |s| s.anim_tick).unwrap_or(0);
        let target = m.ai.target.and_then(|w| cx.w.target_info(w).map(|t| (w, t)));
        m.nav_stop();
        let id = m.id;
        match self.anim {
            anim::SKELETON_MELEE => {
                if let Some((_, t)) = &target
                    && t.alive
                {
                    m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
                }
                if tick == 3 {
                    push(Job::Sweep { mob: id });
                    let pitch = 1.0 / (m.random.next_float() * 0.4 + 1.2);
                    push(Job::Sound { mob: id, name: "minecraft:entity.player.attack.knockback", volume: 1.0, pitch });
                }
            }
            anim::SKELETON_CONSTRICT => {
                let Some((who, t)) = target else { return };
                if tick < 6 {
                    m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
                }
                if tick == 6 {
                    push(Job::Constrict { mob: id, target: who });
                    let pitch = 0.8 + m.random.next_float() * 0.4;
                    push(Job::Sound { mob: id, name: "minecraft:entity.generic.explode", volume: 0.5, pitch });
                }
            }
            anim::SKELETON_SHOOT => {
                let Some((who, t)) = target else { return };
                m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
                if tick == 5 {
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.quick_charge_2", volume: 1.0, pitch: 1.0 });
                }
                if tick == 20 {
                    let pitch = 1.0 / (m.random.next_float() * 0.5 + 1.0) + 0.2;
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.loading_end", volume: 1.0, pitch });
                }
                if tick == 26 && t.alive {
                    // Hit by something lately, it aims worse; out of sight, a little.
                    let mut spread = m.hurt_time as f32 / 2.0;
                    let by_entity = m.hurt_time > 0 && m.last_damage.is_some_and(|(d, _)| d.attacker.is_some());
                    if !by_entity && !m.sees(cx.w, who) {
                        spread = 0.5 + m.random.next_float();
                    }
                    push(Job::Arrows { mob: id, target: who, count: 1, speed: 2.4, spread, effects: true });
                    let pitch = 1.0 / (m.random.next_float() * 0.4 + 1.2) + 0.25;
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.shoot", volume: 1.0, pitch });
                }
            }
            _ => {
                let Some((who, t)) = target else { return };
                m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
                if tick == 10 {
                    // The leap: at it, or away from it when it is close.
                    let (mut x, mut z) = (t.position.x - m.position.x, t.position.z - m.position.z);
                    let mut scale = 0.06 + f64::from(m.random.next_float()) * 0.03;
                    if m.distance_sqr(t.position) < 16.0 {
                        x = -x;
                        z = -z;
                        scale *= 5.0;
                    }
                    m.stuck_multiplier = horizon_core::geometry::Vec3::ZERO;
                    m.velocity = horizon_core::geometry::Vec3::new(x * scale, 1.1, z * scale);
                    m.on_ground = false;
                    m.needs_sync = true;
                }
                if tick == 15 {
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.quick_charge_3", volume: 1.0, pitch: 1.0 });
                }
                if tick == 20 {
                    let pitch = 1.0 / (m.random.next_float() * 0.5 + 1.0) + 0.2;
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.loading_end", volume: 1.0, pitch });
                }
                if (24..28).contains(&tick) {
                    // Each volley is made a tick before it flies: those of ticks 24 to 26 fly.
                    if tick < 27 {
                        push(Job::Arrows { mob: id, target: who, count: 6, speed: 2.0 - m.random.next_float() * 0.1, spread: 3.0, effects: false });
                    }
                    let pitch = 1.0 / (m.random.next_float() * 0.4 + 1.2) + 0.25;
                    push(Job::Sound { mob: id, name: "minecraft:item.crossbow.shoot", volume: 1.0, pitch });
                }
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- MutantEnderman ---------------------------------------------------------

/// The mutant enderman's `AnimationGoal`s: each runs while its animation plays, which the
/// game half (`doHurtTarget`, `teleportToPosition`, `FindTargetGoal`) or its own `can_use`
/// starts.
#[derive(Clone, Debug)]
struct EnderAttack {
    anim: u8,
}
impl Goal for EnderAttack {
    fn flags(&self) -> u8 {
        match self.anim {
            anim::ENDER_MELEE | anim::ENDER_THROW => 0,
            _ => MOVE | LOOK,
        }
    }
    fn interruptable(&self) -> bool {
        false
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        match self.anim {
            anim::ENDER_MELEE => "MeleeGoal",
            anim::ENDER_THROW => "ThrowBlockGoal",
            anim::ENDER_STARE => "StareGoal",
            anim::ENDER_TELEPORT => "TeleportGoal",
            anim::ENDER_SCREAM => "ScreamGoal",
            _ => "TeleSmashGoal",
        }
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let a = self.anim;
        let target = m.ai.target.and_then(|w| cx.w.target_info(w));
        match a {
            // ThrowBlockGoal: one in 28 (or when told to), with a block to throw, the
            // target in sight.
            anim::ENDER_THROW => {
                let roll = m.random.next_int_bound(28).unwrap_or(1) == 0;
                let sees = match m.ai.target {
                    Some(w) => m.sees(cx.w, w),
                    None => true,
                };
                let r = m.random.next_int_bound(2).unwrap_or(0) as usize;
                with(m, |s| {
                    if s.dying || s.animating() || (!s.trigger_throw && !roll) || !sees {
                        return false;
                    }
                    // getThrowingHand: a lower arm holding a block, else an upper.
                    let pick = |pair: [usize; 2]| {
                        let full: Vec<usize> = pair.into_iter().filter(|&i| !s.held[i].is_empty()).collect();
                        (!full.is_empty()).then(|| full[r.min(full.len() - 1)])
                    };
                    match pick([2, 3]).or_else(|| pick([0, 1])) {
                        Some(arm) => {
                            s.active_arm = arm as u8;
                            true
                        }
                        None => false,
                    }
                })
                .unwrap_or(false)
            }
            // ScreamGoal: rarely, within 20 blocks of its target.
            anim::ENDER_SCREAM => {
                let Some(t) = target else { return false };
                let wet = m.fluids.in_water || cx.w.raining;
                let roll = m.random.next_int_bound(if wet { 400 } else { 1200 }).unwrap_or(1) == 0;
                let near = m.distance_sqr(t.position) < 400.0;
                with(m, |s| !s.dying && !s.animating() && s.scream_delay <= 0).unwrap_or(false) && near && roll
            }
            anim::ENDER_MELEE | anim::ENDER_TELEPORT => with(m, |s| !s.dying && s.anim == a).unwrap_or(false),
            _ => target.is_some() && with(m, |s| !s.dying && s.anim == a).unwrap_or(false),
        }
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let a = self.anim;
        let running = with(m, |s| !s.dying && s.anim == a && s.anim_tick < anim::ender_duration(a)).unwrap_or(false);
        if a == anim::ENDER_STARE {
            let alive = m.target_info(cx.w).is_some_and(|t| t.alive);
            return running && alive && with(m, |s| s.looked_at).unwrap_or(false);
        }
        running
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        let a = self.anim;
        with(m, |s| s.play(a));
        let id = m.id;
        match a {
            anim::ENDER_THROW => push(Job::EnderThrow { mob: id, target: m.ai.target }),
            anim::ENDER_STARE => {
                m.ambient_timer = -200;
                let pitch = 0.7 + m.random.next_float() * 0.2;
                push(Job::Sound { mob: id, name: "mutantmonsters:entity.mutant_enderman.stare", volume: 2.5, pitch });
            }
            anim::ENDER_TELEPORT => push(Job::TeleportStart { mob: id }),
            anim::ENDER_TELESMASH => {
                if let Some(target) = m.ai.target {
                    push(Job::TeleSmashStart { mob: id, target });
                }
            }
            _ => {}
        }
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let tick = with(m, |s| s.anim_tick).unwrap_or(0);
        let id = m.id;
        let target = m.ai.target.and_then(|w| cx.w.target_info(w).map(|t| (w, t)));
        match self.anim {
            anim::ENDER_MELEE => {
                if tick == 3 {
                    push(Job::EnderSwipe { mob: id });
                }
            }
            anim::ENDER_STARE => {
                m.nav_stop();
                if let Some((_, t)) = target {
                    m.look_at_speed([t.position.x, t.eye_y, t.position.z], 45.0, 45.0);
                }
            }
            anim::ENDER_TELEPORT => m.nav_stop(),
            anim::ENDER_SCREAM => {
                m.nav_stop();
                if tick == 40 {
                    m.ambient_timer = -200;
                    push(Job::Scream { mob: id });
                }
            }
            anim::ENDER_TELESMASH => {
                let Some((who, t)) = target else { return };
                m.nav_stop();
                if tick < 20 {
                    m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
                }
                if tick == 18 {
                    push(Job::TeleSmash { mob: id, target: who });
                }
            }
            _ => {}
        }
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        let id = m.id;
        match self.anim {
            anim::ENDER_STARE => {
                if let Some(target) = m.ai.target {
                    push(Job::StareEnd { mob: id, target });
                }
                with(m, |s| s.play(anim::NONE));
            }
            // The teleport moves it in the game half, which ends the animation.
            anim::ENDER_TELEPORT => push(Job::TeleportEnd { mob: id }),
            anim::ENDER_SCREAM => {
                with(m, |s| {
                    s.play(anim::NONE);
                    s.scream_delay = 600;
                });
            }
            anim::ENDER_THROW => {
                with(m, |s| {
                    s.play(anim::NONE);
                    s.trigger_throw = false;
                });
            }
            _ => {
                with(m, |s| s.play(anim::NONE));
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- MutantSnowGolem --------------------------------------------------------

/// `RangedAttackGoal(speed 1.1, interval 30, radius 12)`: it keeps 12 blocks from its target
/// and throws every 30 ticks (`performRangedAttack`: starts the throw).
#[derive(Clone, Debug)]
struct GolemRanged {
    see_time: i32,
    attack_time: i32,
}
impl Goal for GolemRanged {
    fn flags(&self) -> u8 {
        MOVE | LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        m.target_info(cx.w).is_some_and(|t| t.alive) && !dying(m)
    }
    fn stop(&mut self, _: &mut Mob, _: &mut Cx) {
        self.see_time = 0;
        self.attack_time = -1;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(who) = m.ai.target else { return };
        let Some(t) = cx.w.target_info(who) else { return };
        let d = m.distance_sqr(t.position);
        let sees = m.sees(cx.w, who);
        self.see_time = if sees { self.see_time.max(0) + 1 } else { 0 };
        if d <= 144.0 && self.see_time >= 5 {
            m.nav_stop();
        } else {
            m.nav_move_to_entity(cx.w, [t.position.x, t.position.y, t.position.z], 0, 1.1);
        }
        m.look_at_speed([t.position.x, t.eye_y, t.position.z], 30.0, 30.0);
        self.attack_time -= 1;
        if self.attack_time == 0 {
            if !sees {
                return;
            }
            with(m, |s| {
                if !s.throwing {
                    s.throwing = true;
                    s.throwing_tick = 0;
                }
            });
            self.attack_time = 30;
        } else if self.attack_time < 0 {
            self.attack_time = 30;
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantSnowGolem.ThrowIceGoal`: while it throws, it stands, and the ice leaves its hand at
/// throwing tick 7.
#[derive(Clone, Debug)]
struct ThrowIce;
impl Goal for ThrowIce {
    fn flags(&self) -> u8 {
        0
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.ai.target.is_some() && with(m, |s| s.throwing).unwrap_or(false)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        with(m, |s| s.throwing && s.throwing_tick < 20).unwrap_or(false)
    }
    fn tick(&mut self, m: &mut Mob, _: &mut Cx) {
        let Some(target) = m.ai.target else { return };
        m.nav_stop();
        m.body_yaw = m.yaw;
        if with(m, |s| s.throwing_tick).unwrap_or(0) == 7 {
            push(Job::ThrowIce { mob: m.id, target });
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantSnowGolem.SwimJumpGoal`: in water it leaps out (hurting itself) and leaves ice.
#[derive(Clone, Debug)]
struct SwimJump {
    jump_tick: i32,
}
impl Goal for SwimJump {
    fn flags(&self) -> u8 {
        JUMP
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.fluids.in_water
    }
    fn can_continue(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        self.jump_tick > 0
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        let (a, b) = (m.random.next_float(), m.random.next_float());
        let (c, d) = (m.random.next_float(), m.random.next_float());
        m.velocity = horizon_core::geometry::Vec3::new(f64::from((a - b) * 0.9), 1.5, f64::from((c - d) * 0.9));
        m.on_ground = false;
        m.needs_sync = true;
        with(m, |s| s.swim_jump = true);
        push(Job::SwimJump { mob: m.id });
    }
    fn tick(&mut self, _: &mut Mob, _: &mut Cx) {
        self.jump_tick -= 1;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.jump_tick = 20;
        with(m, |s| s.swim_jump = false);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- SpiderPig ----------------------------------------------------------------

/// `SpiderPig.LeapAttackGoal`: it springs at its target, within 8 blocks now and then, within
/// 2.5 always.
#[derive(Clone, Debug)]
struct PigLeap;
impl Goal for PigLeap {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let Some(t) = m.target_info(cx.w) else { return false };
        let ready = with(m, |s| s.leap_cooldown <= 0).unwrap_or(false);
        let d = m.distance_sqr(t.position);
        ready
            && (m.on_ground || m.fluids.in_water || m.fluids.in_lava)
            && ((d < 64.0 && m.random.next_int_bound(8).unwrap_or(1) == 0) || d < 6.25)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        with(m, |s| s.leaping && s.leap_tick < 40).unwrap_or(false)
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(t) = m.target_info(cx.w) else { return };
        with(m, |s| {
            s.leaping = true;
            s.leap_cooldown = 15;
        });
        let (x, y, z) = (t.position.x - m.position.x, t.position.y - m.position.y, t.position.z - m.position.z);
        let d = ((x * x + y * y + z * z) as f32).sqrt().max(1e-4) as f64;
        let r = f64::from(m.random.next_float());
        let scale = 2.0 + 0.2 * r * r;
        m.velocity = horizon_core::geometry::Vec3::new(x / d * scale, y / d * scale * 0.5 + 0.3, z / d * scale);
        m.on_ground = false;
        m.needs_sync = true;
    }
    fn tick(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.leap_tick += 1);
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        with(m, |s| s.leap_tick = 0);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- MutantCreeper ----------------------------------------------------------

/// `MutantCreeper.JumpAttackGoal`: now and then it leaps at its target, to burst where it
/// lands (the game half's tick).
#[derive(Clone, Debug)]
struct JumpAttack;
impl Goal for JumpAttack {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let Some(t) = m.target_info(cx.w) else { return false };
        let free = with(m, |s| !s.dying && !s.jump_attacking && !s.charging).unwrap_or(false);
        m.on_ground && free && m.distance_sqr(t.position) <= 1024.0 && m.random.next_float() * 100.0 < 0.9
    }
    fn can_continue(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        false
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(t) = m.target_info(cx.w) else { return };
        with(m, |s| s.jump_attacking = true);
        m.velocity = horizon_core::geometry::Vec3::new(
            (t.position.x - m.position.x) * 0.2,
            1.4,
            (t.position.z - m.position.z) * 0.2,
        );
        m.on_ground = false;
        m.needs_sync = true;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantCreeper.ChargeAttackGoal`: hurt, it stands and charges for 5 seconds, healing at
/// the end unless hit enough; it may call lightning on itself when its target comes close.
#[derive(Clone, Debug)]
struct ChargeAttack;
impl Goal for ChargeAttack {
    fn flags(&self) -> u8 {
        LOOK | MOVE
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if dying(m) {
            return false;
        }
        if with(m, |s| s.charging).unwrap_or(false) {
            return true;
        }
        let Some(t) = m.target_info(cx.w) else { return false };
        let max = m.attrs.max_health;
        let d = m.distance_sqr(t.position);
        with(m, |s| !s.jump_attacking).unwrap_or(false)
            && !(max - m.health < max / 6.0)
            && (25.0..=1024.0).contains(&d)
            && m.random.next_float() * 100.0 < 0.7
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let near = m.target_info(cx.w).is_some_and(|t| m.distance_sqr(t.position) < 25.0);
        with(m, |s| {
            if s.dying || (s.summon_lightning && near) {
                false
            } else {
                s.charge_time < 100 && s.charge_hits > 0
            }
        })
        .unwrap_or(false)
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        let roll = m.random.next_int_bound(if cx.w.thundering { 2 } else { 6 }).unwrap_or(1) == 0;
        with(m, |s| {
            s.charging = true;
            if roll && !s.charged {
                s.summon_lightning = true;
            }
        });
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
        let time = with(m, |s| s.charge_time).unwrap_or(0);
        if time % 20 == 0 {
            let pitch = 0.7 + m.random.next_float() * 0.6;
            push(Job::Sound { mob: m.id, name: "mutantmonsters:entity.mutant_creeper.charge", volume: 0.6, pitch });
        }
        with(m, |s| s.charge_time += 1);
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        let near = m.target_info(cx.w).is_some_and(|t| m.distance_sqr(t.position) < 25.0);
        let hits = 4 + m.random.next_int_bound(3).unwrap_or(0);
        let (lightning, heal) = with(m, |s| {
            let lightning = s.summon_lightning && near;
            let heal = !lightning && s.charge_time >= 100;
            s.charge_time = 0;
            s.charge_hits = hits;
            s.charging = false;
            s.summon_lightning = false;
            (lightning, heal)
        })
        .unwrap_or((false, false));
        if lightning || heal {
            push(Job::ChargeEnd { mob: m.id, lightning, heal });
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

/// `MutantCreeper.SpawnMinionsGoal`: now and then it lobs creeper minions at its target,
/// more often when it stands or was shot.
#[derive(Clone, Debug)]
struct SpawnMinions;
impl Goal for SpawnMinions {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let Some(who) = m.ai.target else { return false };
        let Some(t) = cx.w.target_info(who) else { return false };
        let shot = m.last_damage.is_some_and(|(d, _)| matches!(d.kind, crate::mobs::DamageKind::Arrow | crate::mobs::DamageKind::Thrown | crate::mobs::DamageKind::Fireball));
        let chance = if m.ai.nav.is_done() || shot { 0.9 } else { 0.6 };
        let free = with(m, |s| !s.dying && !s.charging && !s.jump_attacking).unwrap_or(false);
        free && m.distance_sqr(t.position) <= 1024.0 && m.random.next_float() * 100.0 < chance
    }
    fn can_continue(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        false
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        if let Some(target) = m.ai.target {
            push(Job::Minions { mob: m.id, target });
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
}

// --- registerGoals -----------------------------------------------------------

/// Each mutant's `registerGoals`, in place of its template's (`native_register_goals`).
pub fn register_goals(m: &mut Mob) {
    let Some(kind) = kind_of(&m.kind.id) else { return };
    // The creeper minion keeps its template's (a creeper's) goals: it swells and bursts.
    if kind == Kind::Minion {
        return;
    }
    m.ai.goals.clear();
    m.ai.targets.clear();
    // AbstractMutantMonster: `navigation.setCanFloat(true)`.
    m.ai.goals.add(-100, goals::Float);
    let player = || Live(goals::NearestTarget::player(true));
    match kind {
        Kind::Zombie => {
            let g = &mut m.ai.goals;
            g.add(0, Slam { dir: [-1.0, -1.0] });
            g.add(0, Roar);
            g.add(0, Throw { finish: -1 });
            g.add(1, MutantMelee::new(1.2).max_attack_tick(0));
            g.add(2, AvoidDamage::new(1.0));
            wander(m, 4, true);
            let t = &mut m.ai.targets;
            t.add(0, Live(goals::HurtByTarget::new().alert().ignoring(&[Species::Wither])));
            t.add(1, Live(goals::NearestTarget::species(&[Species::IronGolem], true)));
            t.add(2, Live(goals::NearestTarget::player(true).unseen_memory(300)));
            t.add(3, Live(goals::NearestTarget::species(&[Species::Villager, Species::WanderingTrader], true)));
        }
        Kind::Skeleton => {
            let g = &mut m.ai.goals;
            g.add(0, SkeletonAttack { anim: anim::SKELETON_MELEE });
            g.add(0, SkeletonAttack { anim: anim::SKELETON_SHOOT });
            g.add(0, SkeletonAttack { anim: anim::SKELETON_MULTI_SHOT });
            g.add(0, SkeletonAttack { anim: anim::SKELETON_CONSTRICT });
            g.add(1, MutantMelee::new(1.1).max_attack_tick(5));
            g.add(2, AvoidDamage::new(1.0));
            wander(m, 3, true);
            let t = &mut m.ai.targets;
            t.add(0, Live(goals::HurtByTarget::new().ignoring(&[Species::Wither])));
            t.add(1, Live(goals::NearestTarget::player(true).unseen_memory(300)));
            t.add(2, Live(goals::NearestTarget::species(&[Species::IronGolem], true)));
            t.add(3, Live(goals::NearestTarget::species(&[Species::Wolf], true)));
        }
        Kind::Creeper => {
            let g = &mut m.ai.goals;
            g.add(1, JumpAttack);
            g.add(1, SpawnMinions);
            g.add(1, ChargeAttack);
            g.add(2, MutantMelee::new(1.3));
            g.add(3, AvoidDamage::new(1.0));
            wander(m, 4, true);
            let t = &mut m.ai.targets;
            t.add(0, Live(goals::HurtByTarget::new()));
            t.add(1, Live(goals::NearestTarget::player(true).unseen_memory(200)));
            t.add(2, Live(goals::NearestTarget::species(&[Species::Cat, Species::Ocelot], true).interval(100).reach()));
        }
        Kind::Enderman => {
            let g = &mut m.ai.goals;
            for a in [anim::ENDER_MELEE, anim::ENDER_THROW, anim::ENDER_STARE, anim::ENDER_TELEPORT, anim::ENDER_SCREAM, anim::ENDER_TELESMASH] {
                g.add(1, EnderAttack { anim: a });
            }
            g.add(2, MutantMelee::new(1.2).max_attack_tick(15));
            g.add(3, AvoidDamage::new(1.0));
            wander(m, 4, true);
            let t = &mut m.ai.targets;
            t.add(0, Live(goals::HurtByTarget::new()));
            t.add(2, Live(goals::NearestTarget::species(&[Species::Endermite], true)));
        }
        Kind::SnowGolem => {
            let g = &mut m.ai.goals;
            g.add(0, SwimJump { jump_tick: 20 });
            g.add(2, GolemRanged { see_time: 0, attack_time: -1 });
            g.add(3, ThrowIce);
            g.add(4, AvoidDamage::new(1.1));
            g.add(7, Live(goals::Stroll::avoiding_water(1.0)));
            g.add(8, Live(goals::LookAtPlayer::new(6.0)));
            g.add(9, Live(goals::LookAtPlayer::mobs(6.0)));
            g.add(10, Live(goals::RandomLookAround::default()));
            let t = &mut m.ai.targets;
            t.add(0, Live(goals::HurtByTarget::new()));
            t.add(1, Live(goals::NearestTarget::new(goals::Prey::Filter(golem_prey), true)));
        }
        Kind::SpiderPig => {
            let g = &mut m.ai.goals;
            g.add(1, MutantMelee::new(1.1).max_attack_tick(16));
            g.add(2, PigLeap);
            g.add(3, AvoidDamage::new(1.1));
            g.add(5, Live(goals::Breed::new(1.0)));
            g.add(8, Live(goals::Stroll::avoiding_water(1.0)));
            g.add(9, Live(goals::LookAtPlayer::new(8.0)));
            g.add(10, Live(goals::RandomLookAround::default()));
            let t = &mut m.ai.targets;
            t.add(1, Live(goals::HurtByTarget::new().alert()));
            t.add(3, Live(goals::NearestTarget::species(&[Species::Spider, Species::Pig], true)));
        }
        Kind::Minion => {}
    }
}

/// The snow golem's `NearestAttackableTargetGoal` filter: an `Enemy`, never a creeper.
fn golem_prey(s: &crate::mobs::Seen) -> bool {
    s.hostile && s.species != Species::Creeper
}

// --- sounds ------------------------------------------------------------------

/// Each mutant's `getAmbientSound`, `getHurtSound` and `getDeathSound` (`native_mob_voice`).
pub fn voice(m: &Mob, sounds: &mut [&'static str; 3]) {
    match kind_of(&m.kind.id) {
        Some(Kind::Zombie) => {
            *sounds = [
                "mutantmonsters:entity.mutant_zombie.ambient",
                "mutantmonsters:entity.mutant_zombie.hurt",
                "mutantmonsters:entity.mutant_zombie.death",
            ];
        }
        Some(Kind::Skeleton) => {
            *sounds = [
                "mutantmonsters:entity.mutant_skeleton.ambient",
                "mutantmonsters:entity.mutant_skeleton.hurt",
                "mutantmonsters:entity.mutant_skeleton.death",
            ];
        }
        Some(Kind::Enderman) => {
            *sounds = [
                "mutantmonsters:entity.mutant_enderman.ambient",
                "mutantmonsters:entity.mutant_enderman.hurt",
                "mutantmonsters:entity.mutant_enderman.hurt",
            ];
        }
        // AbstractGolem: no ambient sound.
        Some(Kind::SnowGolem) => {
            *sounds = ["", "mutantmonsters:entity.mutant_snow_golem.hurt", "mutantmonsters:entity.mutant_snow_golem.death"];
        }
        Some(Kind::SpiderPig) => {
            *sounds = [
                "mutantmonsters:entity.spider_pig.ambient",
                "mutantmonsters:entity.spider_pig.hurt",
                "mutantmonsters:entity.spider_pig.death",
            ];
        }
        Some(Kind::Minion) => {
            *sounds = [
                "mutantmonsters:entity.creeper_minion.ambient",
                "mutantmonsters:entity.creeper_minion.hurt",
                "mutantmonsters:entity.creeper_minion.death",
            ];
        }
        // MutantCreeper.playAmbientSound: only without a target; its death sound is its hurt.
        Some(Kind::Creeper) => {
            *sounds = [
                if m.ai.target.is_some() { "" } else { "mutantmonsters:entity.mutant_creeper.ambient" },
                "mutantmonsters:entity.mutant_creeper.hurt",
                "mutantmonsters:entity.mutant_creeper.hurt",
            ];
        }
        _ => {}
    }
}

/// Each mutant's `playStepSound` (`native_mob_step_sound`).
pub fn step_sound(m: &Mob, sound: &mut Option<(&'static str, f32, f32)>) {
    match kind_of(&m.kind.id) {
        Some(Kind::Skeleton) => *sound = Some(("mutantmonsters:entity.mutant_skeleton.step", 0.15, 1.0)),
        Some(Kind::SnowGolem) => *sound = Some(("block.snow.step", 0.15, 1.0)),
        Some(Kind::SpiderPig) => *sound = Some(("entity.spider.step", 0.15, 1.0)),
        _ => {}
    }
    if kind_of(&m.kind.id) == Some(Kind::Zombie) {
        // MutantZombie.playStepSound: a zombie's step, not while it lies.
        let lying = SERVER
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|all| all.get(&m.id))
            .is_some_and(|s| s.dying);
        *sound = (!lying).then_some(("entity.zombie.step", 0.15, 1.0));
    }
}

