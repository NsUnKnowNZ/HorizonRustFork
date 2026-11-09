// Carry Block: sneak and right click with an empty hand to pick up a block with
// everything in it (a chest, a furnace...) or a small mob, carry it around and
// right click to put it down again.
//
// Original implementation for Horizon (a horizon-mixins native mod). This file
// is both the standalone, unit-tested Cargo library and the injected
// `crate::game::carryblock` module. It has no Horizon dependency: it decides
// (what may be picked up, the state a block is put down with, the settings
// page, the commands, the HUD text), and `native.rs` does what it decides in
// the running game.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

/// A player's UUID as the game keeps it.
pub type Uuid = [i32; 4];

/// The mod's id: its row in Options > Mods and its settings folder
/// (`horizon-mods/.horizon-mod-data/carryblock/`).
pub const ID: &str = "carryblock";
pub const VERSION: &str = "0.1.2";
pub const DESCRIPTION: &str = "Sneak and right click with an empty hand to pick up a chest, a \
furnace or a small animal with everything in it; right click a block to put it down.";
/// The configuration, in the mod's settings folder: what the settings page
/// writes, and where the deny lists are edited by hand.
pub const CONFIG_FILE: &str = "config.json";
/// The chat command (without its slash).
pub const COMMAND: &str = "carryblock";
pub const USAGE: &str = "Usage: /carryblock [status|reload]";

/// The key of the player's saved compound that holds what they carry. It is
/// saved and loaded with the player, exactly like their inventory.
pub const CARRY_KEY: &str = "carryblock:carried";
/// The keys of the carried compound.
pub const KEY_FORMAT: &str = "format";
pub const KEY_KIND: &str = "kind";
/// A block: its state (`minecraft:chest[facing=north,...]`).
pub const KEY_STATE: &str = "state";
/// A block: its block entity as the game saves it, when it has one.
pub const KEY_BLOCK_ENTITY: &str = "block_entity";
/// A mob: its entity compound as the game saves it.
pub const KEY_ENTITY: &str = "entity";
/// What the HUD and `/carryblock` call it.
pub const KEY_NAME: &str = "name";
pub const KIND_BLOCK: &str = "block";
pub const KIND_MOB: &str = "mob";
/// The carried compound's layout.
pub const FORMAT: i32 = 1;

/// A use this many ticks or less after the player's previous one is the use
/// button held down (it repeats every 4 ticks): it neither picks up nor puts
/// down, so holding the button never flips between the two.
pub const REPEAT_TICKS: u64 = 5;
/// The slowness a carrier gets lasts this long and is renewed once it has
/// `SLOWNESS_REFRESH` ticks left, so it wears off soon after putting down.
pub const SLOWNESS_TICKS: i32 = 40;
pub const SLOWNESS_REFRESH: i32 = 10;
/// How often a remote carrier is reminded in their action bar (ticks).
pub const ACTION_BAR_EVERY: u64 = 40;
/// How long the HUD box shows after the server last said what is carried
/// (a server that stops saying it, without a put down, a drop or a death:
/// those take the box away at once).
pub const HUD_FRESH_MS: u64 = 1500;
/// The gap `Corners::place` leaves between a box and the screen's edge or
/// the box before it.
pub const HUD_MARGIN: f32 = 2.0;
/// Half the width of the column over the hotbar, from the screen's centre:
/// the hotbar's 91, plus the off hand's slot (29) on either side (the left,
/// or the right for a left-handed player; the hotbar attack indicator fits
/// in it too). The hearts, armour, food, air and XP bars, the held item's
/// name and the action bar are all inside it (or centred on it).
pub const HOTBAR_HALF_WIDTH: f32 = 120.0;
/// How far up from the screen's bottom the column over the hotbar is taken:
/// the action bar (`Hud.extractOverlayMessage`: its line 68 up, text from 4
/// above, its backdrop 2 more), over the held item's name (59), armour and
/// air (49), hearts and food (39), the XP bar and the hotbar; two rows of
/// hearts still fit under it.
pub const ABOVE_HOTBAR: f32 = 74.0;
/// How far from a dead carrier's feet a block is put down.
pub const RELEASE_RADIUS: i32 = 2;

/// What may be picked up.
pub const DEFAULT_DENY_BLOCKS: &[&str] = &[
    "minecraft:bedrock",
    "minecraft:end_portal",
    "minecraft:end_portal_frame",
    "minecraft:end_gateway",
    "minecraft:nether_portal",
    "minecraft:barrier",
    "minecraft:light",
    "minecraft:structure_void",
    "minecraft:structure_block",
    "minecraft:jigsaw",
    "minecraft:command_block",
    "minecraft:chain_command_block",
    "minecraft:repeating_command_block",
    "minecraft:test_block",
    "minecraft:test_instance_block",
    "minecraft:reinforced_deepslate",
    // No tool ever gets these in survival, silk touch included: carrying
    // one would move what vanilla only lets a player destroy (a dungeon's
    // spawner next to another, a trial chamber's vault).
    "minecraft:spawner",
    "minecraft:trial_spawner",
    "minecraft:vault",
    "minecraft:budding_amethyst",
];
pub const DEFAULT_DENY_ENTITIES: &[&str] = &[
    "minecraft:player",
    "minecraft:ender_dragon",
    "minecraft:wither",
    "minecraft:warden",
    "minecraft:elder_guardian",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockRule {
    /// Blocks with a block entity: chests, furnaces, barrels, signs...
    WithData,
    /// Any single block that has an item.
    Any,
    Off,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MobRule {
    /// Mobs that are not monsters.
    Passive,
    Any,
    Off,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HudCorner {
    Off,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Config {
    /// Picking up at all (putting down always works).
    pub enabled: bool,
    /// Survival players may pick up.
    pub survival: bool,
    /// Creative players may pick up.
    pub creative: bool,
    /// Creative players ignore the deny lists, the mob size and the monster
    /// rule (never the rules that keep the world consistent).
    pub creative_ignores_rules: bool,
    pub blocks: BlockRule,
    pub mobs: MobRule,
    /// The largest mob (its width or height, in blocks).
    pub max_entity_size: f32,
    /// Slowness level while carrying, 0 for none.
    pub slowness: u8,
    /// Where the window shows what its player carries.
    pub hud: HudCorner,
    /// Remind other players what they carry in their action bar.
    pub action_bar: bool,
    /// Blocks never picked up: ids (`minecraft:spawner`, `spawner`), `*`
    /// globs (`minecraft:*_shulker_box`) and block tags (`#minecraft:beds`).
    pub deny_blocks: Vec<String>,
    /// Mobs never picked up: ids and `*` globs.
    pub deny_entities: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            survival: true,
            creative: true,
            creative_ignores_rules: false,
            blocks: BlockRule::WithData,
            mobs: MobRule::Passive,
            max_entity_size: 1.5,
            slowness: 1,
            hud: HudCorner::BottomRight,
            action_bar: true,
            deny_blocks: DEFAULT_DENY_BLOCKS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            deny_entities: DEFAULT_DENY_ENTITIES
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}
impl Config {
    pub fn from_json(source: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    /// The file the settings page writes: every key, readable by `from_json`.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(self.max_entity_size.is_finite() && (0.1..=16.0).contains(&self.max_entity_size)) {
            return Err("maxEntitySize must be 0.1..=16".into());
        }
        if self.slowness > 4 {
            return Err("slowness must be 0..=4".into());
        }
        for (key, list, tags) in [
            ("denyBlocks", &self.deny_blocks, true),
            ("denyEntities", &self.deny_entities, false),
        ] {
            if let Some(bad) = list.iter().find(|p| !valid_pattern(p, tags)) {
                return Err(format!("{key}: bad entry {bad:?}"));
            }
        }
        Ok(())
    }
}

/// `ns:path` ids, `*` globs and (for blocks) `#tag` entries.
fn valid_pattern(pattern: &str, tags: bool) -> bool {
    let body = match pattern.strip_prefix('#') {
        Some(rest) if tags => rest,
        Some(_) => return false,
        None => pattern,
    };
    !body.is_empty()
        && body.len() <= 128
        && body.matches(':').count() <= 1
        && body.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '_' | '-' | '.' | '/' | ':')
                || c == '*' && !pattern.starts_with('#')
        })
}

/// `id` with the `minecraft:` namespace when it has none.
pub fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// Whether `text` matches `pattern`, where `*` stands for any run of characters.
pub fn glob(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Whether `id` is on a deny list. `in_tag` answers for `#tag` entries
/// (namespaced tag names).
pub fn denied(patterns: &[String], id: &str, in_tag: &dyn Fn(&str) -> bool) -> bool {
    let id = namespaced(id);
    patterns.iter().any(|p| match p.strip_prefix('#') {
        Some(tag) => in_tag(&namespaced(tag)),
        None => glob(&namespaced(p), &id),
    })
}

/// The game mode of the player acting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

/// What to do with a sneaking empty-handed right click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Pick it up.
    Allow,
    /// Not ours: the game uses the click as usual.
    Pass,
    /// Tell the player why not, then let the game use the click.
    Refuse(Refusal),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Denied,
    Unbreakable,
    TwoBlocks,
    NoItem,
    Locked,
    Hostile,
    TooBig,
    Riding,
    Leashed,
    Inventory,
    NotYours,
    /// A game master block (command, structure, jigsaw, test blocks) and a
    /// player who may not use them (`Player.canUseGameMasterBlocks`).
    GameMaster,
    /// Another player (or this one) has its container open.
    InUse,
    /// `Invulnerable`, and a player who is not in creative.
    Invulnerable,
    /// It holds other mobs' leads (a wandering trader's llamas).
    Leading,
    /// A player is trading with it.
    Trading,
    /// It sleeps in a bed.
    Sleeping,
    /// Under spawn protection (the game's own red message is shown instead,
    /// `ServerPlayer.sendSpawnProtectionMessage`).
    SpawnProtection,
    /// Outside the world border.
    Border,
    /// In an unloaded chunk or above or below the level.
    OutOfWorld,
    /// Out of the player's reach.
    TooFar,
    /// A player who may not build (adventure mode).
    CantBuild,
}
impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Self::GameMaster => "Only operators in creative can move that",
            Self::InUse => "Someone is using that",
            Self::Invulnerable => "That can't be carried",
            Self::Leading => "Unleash what it leads first",
            Self::Trading => "Someone is trading with it",
            Self::Sleeping => "Let it sleep",
            Self::SpawnProtection => "That's under spawn protection",
            Self::Border => "That's outside the world border",
            Self::OutOfWorld => "That's outside the world",
            Self::TooFar => "That's too far away",
            Self::CantBuild => "You can't build here",
            Self::Denied => "That can't be carried",
            Self::Unbreakable => "That can't be moved",
            Self::TwoBlocks => "Only single blocks can be carried",
            Self::NoItem => "That can't be carried",
            Self::Locked => "It's locked",
            Self::Hostile => "Monsters can't be carried",
            Self::TooBig => "That's too big to carry",
            Self::Riding => "Something is riding it, or it rides something",
            Self::Leashed => "Unleash it first",
            Self::Inventory => "Take its saddle and chest off first",
            Self::NotYours => "That pet isn't yours",
        }
    }
}

/// What a sneaking right click with an empty main hand may do for a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickUp {
    /// Nothing: the game uses the click as usual.
    No,
    /// Pick up what they aim at, if Carry Block's rules let them.
    Yes,
    /// They may not build (adventure mode, `Abilities.mayBuild`): what a
    /// survival player would pick up, they hear they can't ("You can't build
    /// here", as putting down says), and the game uses the click as usual (a
    /// chest opens).
    CantBuild,
}

/// What this click may do: sneaking, the main hand empty, and their game
/// mode allowed (adventure players as survival players are, see
/// [`PickUp::CantBuild`]).
pub fn pick_up(config: &Config, mode: Mode, sneaking: bool, hand_empty: bool) -> PickUp {
    if !(config.enabled && sneaking && hand_empty) {
        return PickUp::No;
    }
    match mode {
        Mode::Survival if config.survival => PickUp::Yes,
        Mode::Creative if config.creative => PickUp::Yes,
        Mode::Adventure if config.survival => PickUp::CantBuild,
        _ => PickUp::No,
    }
}

/// The verdict for a player [`pick_up`] answered `pick` for: one who may
/// not build hears so where they would have picked it up; a refusal stays
/// what it is (they would hear it in survival too), and what Carry Block
/// leaves to the game stays silent.
pub fn verdict_for(pick: PickUp, verdict: Verdict) -> Verdict {
    match (pick, verdict) {
        (PickUp::Yes, v) => v,
        (PickUp::CantBuild, Verdict::Allow) => Verdict::Refuse(Refusal::CantBuild),
        (PickUp::CantBuild, v) => v,
        (PickUp::No, _) => Verdict::Pass,
    }
}

fn ignores_rules(config: &Config, mode: Mode) -> bool {
    mode == Mode::Creative && config.creative_ignores_rules
}

/// `Abilities.mayBuild`: survival and creative players build, adventure and
/// spectator players don't. Carry Block never has an item in the hand, so an
/// adventure player's `CanBreak`/`CanPlaceOn` (`Player.blockActionRestricted`,
/// `ItemStack.useOn`) never lets them either.
pub fn may_build(mode: Mode) -> bool {
    matches!(mode, Mode::Survival | Mode::Creative)
}

/// The game master blocks (vanilla's `GameMasterBlock` blocks: `CommandBlock`,
/// `StructureBlock`, `JigsawBlock`, `TestBlock`, `TestInstanceBlock`), by id
/// and by class for subclasses: only a player who may use them
/// (`Player.canUseGameMasterBlocks`: creative, permission level 2) breaks
/// (`ServerPlayerGameMode.destroyBlock`) or places one.
pub const GAME_MASTER_BLOCKS: &[&str] = &[
    "minecraft:command_block",
    "minecraft:chain_command_block",
    "minecraft:repeating_command_block",
    "minecraft:structure_block",
    "minecraft:jigsaw",
    "minecraft:test_block",
    "minecraft:test_instance_block",
];
pub const GAME_MASTER_CLASSES: &[&str] = &[
    "CommandBlock",
    "StructureBlock",
    "JigsawBlock",
    "TestBlock",
    "TestInstanceBlock",
];
pub fn is_game_master_block(name: &str, has_class: &dyn Fn(&str) -> bool) -> bool {
    GAME_MASTER_BLOCKS.contains(&namespaced(name).as_str())
        || GAME_MASTER_CLASSES.iter().any(|c| has_class(c))
}

/// `server.properties`' `spawn-protection` and `ops.json`, as the server
/// last said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpawnRules {
    /// The radius around the world spawn (`spawn-protection`).
    pub radius: i32,
    /// The operators' UUIDs.
    pub ops: BTreeSet<u128>,
    /// Read from the server at least once: until it is, a dedicated
    /// server's player is under protection everywhere near (fails closed).
    pub known: bool,
}

/// `DedicatedServer.isUnderSpawnProtection` (and Horizon's
/// `under_spawn_protection`): whether `player` may not change the block at
/// `pos` because of spawn protection. `spawn` is the world spawn when the
/// block is in the world spawn's level (the Overworld), else None. Only a
/// dedicated server protects its spawn (an integrated one, single player and
/// LAN, never does: `MinecraftServer.isUnderSpawnProtection`); `player` is
/// None for single player's own player. No operators at all, an operator, or
/// a radius of 0 or less: not protected.
pub fn under_spawn_protection(
    rules: &SpawnRules,
    dedicated: bool,
    player: Option<u128>,
    spawn: Option<[i32; 3]>,
    pos: [i32; 3],
) -> bool {
    let (true, Some(player), Some(spawn)) = (dedicated, player, spawn) else {
        return false;
    };
    if !rules.known {
        return true;
    }
    if rules.ops.is_empty() || rules.ops.contains(&player) || rules.radius <= 0 {
        return false;
    }
    let distance = pos[0].abs_diff(spawn[0]).max(pos[2].abs_diff(spawn[2]));
    distance <= rules.radius.unsigned_abs()
}

/// `Player.isWithinBlockInteractionRange(pos, buffer)` as
/// `range + buffer`: `eye` closer than `range` to the box of the block at
/// `pos` (`AABB.distanceToSqr`).
pub fn within_block_range(eye: [f64; 3], range: f64, pos: [i32; 3]) -> bool {
    (0..3)
        .map(|k| {
            let lo = f64::from(pos[k]);
            (lo - eye[k]).max(eye[k] - lo - 1.0).max(0.0).powi(2)
        })
        .sum::<f64>()
        < range * range
}

/// A block a player changes, as the game checks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    /// In a loaded chunk, within the level's height.
    pub in_world: bool,
    /// Within the player's reach (`isWithinBlockInteractionRange(pos, 1.0)`).
    pub reachable: bool,
    /// Under spawn protection for this player ([`under_spawn_protection`]).
    pub spawn_protected: bool,
    /// Inside the world border (`WorldBorder.isWithinBounds`).
    pub inside_border: bool,
}

/// `ServerLevel.mayInteract`: not under spawn protection, inside the world
/// border.
fn may_interact(at: Spot) -> Result<(), Refusal> {
    if at.spawn_protected {
        return Err(Refusal::SpawnProtection);
    }
    if !at.inside_border {
        return Err(Refusal::Border);
    }
    Ok(())
}

/// Picking a block up is breaking it: `ServerPlayerGameMode.handleBlockBreakAction`
/// (in the level, within reach, spawn protection, `mayInteract`) and
/// `destroyBlock` (`blockActionRestricted`: a player who may build).
pub fn may_break(mode: Mode, at: Spot) -> Result<(), Refusal> {
    if !at.in_world {
        return Err(Refusal::OutOfWorld);
    }
    if !at.reachable {
        return Err(Refusal::TooFar);
    }
    may_interact(at)?;
    if !may_build(mode) {
        return Err(Refusal::CantBuild);
    }
    Ok(())
}

/// Putting down starts as using the clicked block:
/// `ServerGamePacketListenerImpl.handleUseItemOn` (within reach, spawn
/// protection, `mayInteract`) and a player who may build (`ItemStack.useOn`).
pub fn may_use_on(mode: Mode, clicked: Spot) -> Result<(), Refusal> {
    if !clicked.reachable {
        return Err(Refusal::TooFar);
    }
    may_interact(clicked)?;
    if !may_build(mode) {
        return Err(Refusal::CantBuild);
    }
    Ok(())
}

/// Putting a block (or a mob) down is placing it at `target`: the clicked
/// block as [`may_use_on`], and the block it goes into inside the world, not
/// under spawn protection and inside the world border (stricter than
/// vanilla, which only checks the clicked block: the protected area's edge is
/// not one block wider for Carry Block).
pub fn may_place(mode: Mode, clicked: Spot, target: Spot) -> Result<(), Refusal> {
    may_use_on(mode, clicked)?;
    if !target.in_world {
        return Err(Refusal::OutOfWorld);
    }
    may_interact(target)
}

/// Picking a mob up, at the block it stands in: in the level,
/// `ServerGamePacketListenerImpl.handleInteract`'s world border, and spawn
/// protection (stricter than vanilla, which lets anyone use a mob at spawn:
/// taking one away is more than using it). Reach is the mob ray's own (3
/// blocks, 5 in creative).
pub fn may_take_mob(at: Spot) -> Result<(), Refusal> {
    if !at.in_world {
        return Err(Refusal::OutOfWorld);
    }
    may_interact(at)
}

/// Whether a dead carrier's block may be left at `at` (else it is dropped
/// as its item, as a dead player's inventory is): in the world, not under
/// their spawn protection, inside the world border.
pub fn may_release_at(at: Spot) -> bool {
    at.in_world && may_interact(at).is_ok()
}

/// A block as the game describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockInfo {
    pub name: String,
    pub properties: BTreeMap<String, String>,
    /// It has a block entity (`BlockState.hasBlockEntity`).
    pub block_entity: bool,
    /// `destroyTime`: below 0 is unbreakable.
    pub hardness: f32,
    /// Its item (`minecraft:air` is none).
    pub item: Option<String>,
    /// Air or a fluid: nothing to pick up.
    pub empty: bool,
    /// Its block entity has a lock (`lock`).
    pub locked: bool,
    /// A game master block ([`is_game_master_block`]).
    pub game_master: bool,
    /// A player has its container open (a menu over its block entity).
    pub in_use: bool,
}

/// Half of a block that is two blocks (beds, doors, tall plants), or an
/// extended piston: the other half would break on its own and drop an item.
pub fn is_two_blocks(properties: &BTreeMap<String, String>) -> bool {
    let is = |k: &str, values: &[&str]| {
        properties
            .get(k)
            .is_some_and(|v| values.contains(&v.as_str()))
    };
    is("part", &["head", "foot"]) || is("half", &["upper", "lower"]) || is("extended", &["true"])
}

/// Whether `block` may be picked up. `in_tag` answers block tags for the
/// deny list; `game_master` is whether the player may use game master
/// blocks. Where it is and who may change it there is [`may_break`]'s.
pub fn block_verdict(
    config: &Config,
    mode: Mode,
    game_master: bool,
    block: &BlockInfo,
    in_tag: &dyn Fn(&str) -> bool,
) -> Verdict {
    if block.empty || config.blocks == BlockRule::Off {
        return Verdict::Pass;
    }
    let bypass = ignores_rules(config, mode);
    let deny_listed = !bypass && denied(&config.deny_blocks, &block.name, in_tag);
    // A block the setting leaves out (only blocks with data, by default):
    // the click is the game's, silently (dirt, stone), unless the block is
    // on the deny list (bedrock), which says so whatever the setting.
    if config.blocks == BlockRule::WithData && !block.block_entity {
        return if deny_listed {
            Verdict::Refuse(Refusal::Denied)
        } else {
            Verdict::Pass
        };
    }
    // Permissions and the world's consistency first: no setting lifts them.
    if block.game_master && !game_master {
        return Verdict::Refuse(Refusal::GameMaster);
    }
    if block.in_use {
        return Verdict::Refuse(Refusal::InUse);
    }
    if deny_listed {
        return Verdict::Refuse(Refusal::Denied);
    }
    if block.item.is_none() {
        return Verdict::Refuse(Refusal::NoItem);
    }
    if is_two_blocks(&block.properties) {
        return Verdict::Refuse(Refusal::TwoBlocks);
    }
    if block.hardness < 0.0 && mode != Mode::Creative {
        return Verdict::Refuse(Refusal::Unbreakable);
    }
    if block.locked {
        return Verdict::Refuse(Refusal::Locked);
    }
    Verdict::Allow
}

/// A mob as the game describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobInfo {
    pub id: String,
    pub width: f32,
    pub height: f32,
    /// In the `monster` category.
    pub hostile: bool,
    pub alive: bool,
    /// Already gone (picked up, despawned) this tick.
    pub removed: bool,
    pub riding: bool,
    pub ridden: bool,
    pub leashed: bool,
    /// A saddle, a chest or an inventory (horses, llamas...).
    pub inventory: bool,
    /// Tamed by another player.
    pub owned_by_other: bool,
    /// `Invulnerable` (map makers' mobs): only a creative player can hurt it
    /// (`Entity.isInvulnerableToBase`), so only one can take it away.
    pub invulnerable: bool,
    /// It holds other mobs' leads: they would be left tied to nothing.
    pub leading: bool,
    /// A player is trading with it (its offers are written back on close).
    pub trading: bool,
    /// It sleeps in a bed, which would stay occupied.
    pub sleeping: bool,
}

pub fn mob_verdict(config: &Config, mode: Mode, mob: &MobInfo) -> Verdict {
    if mob.removed || !mob.alive || config.mobs == MobRule::Off {
        return Verdict::Pass;
    }
    let bypass = ignores_rules(config, mode);
    if !bypass {
        if denied(&config.deny_entities, &mob.id, &|_| false) {
            return Verdict::Refuse(Refusal::Denied);
        }
        if config.mobs == MobRule::Passive && mob.hostile {
            return Verdict::Refuse(Refusal::Hostile);
        }
        if mob.width.max(mob.height) > config.max_entity_size + 1e-4 {
            return Verdict::Refuse(Refusal::TooBig);
        }
    }
    if mob.riding || mob.ridden {
        return Verdict::Refuse(Refusal::Riding);
    }
    if mob.leashed {
        return Verdict::Refuse(Refusal::Leashed);
    }
    if mob.inventory {
        return Verdict::Refuse(Refusal::Inventory);
    }
    if mob.owned_by_other {
        return Verdict::Refuse(Refusal::NotYours);
    }
    if mob.invulnerable && mode != Mode::Creative {
        return Verdict::Refuse(Refusal::Invulnerable);
    }
    if mob.leading {
        return Verdict::Refuse(Refusal::Leading);
    }
    if mob.trading {
        return Verdict::Refuse(Refusal::Trading);
    }
    if mob.sleeping {
        return Verdict::Refuse(Refusal::Sleeping);
    }
    Verdict::Allow
}

/// A block state: its name and properties (`name[k=v,...]` as text).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}
impl State {
    pub fn parse(text: &str) -> Option<Self> {
        let (name, rest) = match text.split_once('[') {
            Some((n, rest)) => (n, Some(rest.strip_suffix(']')?)),
            None => (text, None),
        };
        if name.is_empty() || name.contains([']', ',', '=']) {
            return None;
        }
        let mut properties = BTreeMap::new();
        for pair in rest.unwrap_or("").split(',').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=')?;
            if k.is_empty() || v.is_empty() {
                return None;
            }
            properties.insert(k.to_owned(), v.to_owned());
        }
        Some(Self {
            name: namespaced(name),
            properties,
        })
    }
    pub fn text(&self) -> String {
        if self.properties.is_empty() {
            return self.name.clone();
        }
        let props: Vec<String> = self
            .properties
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{}[{}]", self.name, props.join(","))
    }
}

/// Properties the spot and the player decide when a block is put down
/// (which way it faces, what it connects to, water, redstone power): every
/// other property (a furnace's `lit`, a hive's `honey_level`, a lectern's
/// `has_book`...) is the carried block's own, so it matches its contents.
pub const PLACEMENT_KEYS: &[&str] = &[
    "facing",
    "axis",
    "rotation",
    "type",
    "waterlogged",
    "face",
    "attachment",
    "hanging",
    "half",
    "shape",
    "north",
    "south",
    "east",
    "west",
    "up",
    "down",
    "attached",
    "in_wall",
    "hinge",
    "open",
    "powered",
    "triggered",
    "enabled",
    // A natural sculk shrieker summons wardens and a natural creaking heart
    // spawns a creaking; placed ones (silk touch's) don't. Put down, one is
    // a placed one, as vanilla's only way to move it gives.
    "can_summon",
    "natural",
];

/// The state to put a carried block down with: `placed` (what placing its
/// item there gives) with the carried block's other properties. A double
/// slab stays double.
pub fn placed_state(carried: &State, placed: &State) -> State {
    let mut state = placed.clone();
    for (k, v) in &carried.properties {
        let decided_here = PLACEMENT_KEYS.contains(&k.as_str())
            && !(k == "type" && v == "double" && carried.name == placed.name);
        if !decided_here && state.properties.contains_key(k) {
            state.properties.insert(k.clone(), v.clone());
        }
    }
    state
}

/// The state to leave a carried block in where its carrier died: as carried,
/// alone (no chest half), closed, placed (a shrieker that summons nothing, a
/// creaking heart that is not natural) and holding water when it is put in
/// water.
pub fn released_state(carried: &State, water: bool) -> State {
    let mut state = carried.clone();
    let p = &mut state.properties;
    if p.get("type").is_some_and(|t| t == "left" || t == "right") {
        p.insert("type".into(), "single".into());
    }
    for key in ["open", "powered", "can_summon", "natural"] {
        if p.contains_key(key) {
            p.insert(key.into(), "false".into());
        }
    }
    if p.contains_key("waterlogged") {
        p.insert("waterlogged".into(), water.to_string());
    }
    state
}

/// `minecraft:oak_wall_sign` as `Oak Wall Sign`.
pub fn display_name(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    path.split(['_', '/'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_uppercase().chain(c).collect::<String>()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What the HUD calls a carried thing: its name and, for a container, how
/// many stacks are in it.
pub fn summary(name: &str, stacks: Option<usize>) -> String {
    match stacks {
        Some(1) => format!("{name} (1 stack)"),
        Some(n) if n > 0 => format!("{name} ({n} stacks)"),
        _ => name.to_owned(),
    }
}
/// The window's HUD box.
pub fn hud_lines(summary: &str) -> [String; 2] {
    [
        format!("Carrying: {summary}"),
        "Right click a block to put it down".into(),
    ]
}
/// Whether a box `w` wide in `corner` of a screen `screen_w` GUI pixels
/// wide, placed as `Corners::place` places it (its side's edge
/// [`HUD_MARGIN`] away), would cover the column over the hotbar (a bottom
/// corner only).
pub fn hud_crosses_hotbar(corner: HudCorner, screen_w: f32, w: f32) -> bool {
    let x = match corner {
        HudCorner::BottomLeft => HUD_MARGIN,
        HudCorner::BottomRight => screen_w - HUD_MARGIN - w,
        HudCorner::Off | HudCorner::TopLeft | HudCorner::TopRight => return false,
    };
    let centre = screen_w / 2.0;
    x < centre + HOTBAR_HALF_WIDTH && x + w > centre - HOTBAR_HALF_WIDTH
}
/// For a box [`hud_crosses_hotbar`]: the empty room to reserve under it, in
/// its corner, so that it sits [`HUD_MARGIN`] over [`ABOVE_HOTBAR`]. `taken`
/// is how far up from the bottom the corner is taken already (where an
/// empty `Corners::place` puts its box, from the bottom: the boxes placed
/// before this one and one margin); 0 when they lift it high enough.
pub fn hud_room_under(taken: f32) -> f32 {
    (ABOVE_HOTBAR - taken).max(0.0)
}

/// What the window's HUD box says each player carries, as the game that
/// simulates the world last said (it may be single player's server thread).
#[derive(Clone, Debug, Default)]
pub struct HudBoard {
    lines: BTreeMap<Uuid, (String, Instant)>,
}
impl HudBoard {
    pub const fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
        }
    }
    /// `who` carries `summary`, as of `now`.
    pub fn note(&mut self, who: Uuid, summary: &str, now: Instant) {
        self.lines.insert(who, (summary.to_owned(), now));
        if self.lines.len() > 64 {
            let fresh = Duration::from_millis(HUD_FRESH_MS);
            self.lines
                .retain(|_, (_, at)| now.saturating_duration_since(*at) < fresh);
        }
    }
    /// `who` carries nothing any more (put down, dropped, dead): the box
    /// goes at once.
    pub fn forget(&mut self, who: Uuid) {
        self.lines.remove(&who);
    }
    /// What the box shows for `who` at `now`: nothing while they are `dead`
    /// (the death screen), nor once the server stopped saying it.
    pub fn shown(&self, who: Uuid, now: Instant, dead: bool) -> Option<&str> {
        if dead {
            return None;
        }
        let fresh = Duration::from_millis(HUD_FRESH_MS);
        self.lines
            .get(&who)
            .filter(|(_, at)| now.saturating_duration_since(*at) < fresh)
            .map(|(text, _)| text.as_str())
    }
}

/// A remote player's action bar.
pub fn action_bar_text(summary: &str) -> String {
    format!("Carrying {summary} - right click a block to put it down")
}
/// `/carryblock status`.
pub fn status_text(summary: Option<&str>) -> String {
    match summary {
        Some(s) => format!("Carry Block {VERSION}: you are carrying {s}"),
        None => format!("Carry Block {VERSION}: you are not carrying anything"),
    }
}

/// Where around a dead carrier's feet their block goes, nearest first (never
/// more than one block below the feet).
pub fn release_spots(radius: i32) -> Vec<[i32; 3]> {
    let mut spots = Vec::new();
    for dy in -1..=radius {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                spots.push([dx, dy, dz]);
            }
        }
    }
    spots.sort_by_key(|&[dx, dy, dz]| (dx * dx + dy * dy + dz * dz, dy < 0, dy.abs(), dx, dz));
    spots
}

/// Each player's last use, to tell a click from the use button held down.
#[derive(Clone, Debug, Default)]
pub struct Clicks {
    last: HashMap<Uuid, u64>,
}
impl Clicks {
    /// Notes a use by `who` at tick `now`: true when it is a new click.
    pub fn fresh(&mut self, who: Uuid, now: u64) -> bool {
        let previous = self.last.insert(who, now);
        if self.last.len() > 256 {
            self.last
                .retain(|_, t| now.saturating_sub(*t) <= REPEAT_TICKS);
        }
        previous.is_none_or(|p| now < p || now - p > REPEAT_TICKS)
    }
}

/// The slowness amplifier carrying gives, if any.
pub fn slowness_amplifier(config: &Config) -> Option<i32> {
    (config.slowness > 0).then(|| i32::from(config.slowness) - 1)
}
/// Whether to (re)give a carrier slowness `amplifier`, given the slowness
/// they have (amplifier, ticks left; -1 forever). A stronger or longer one
/// from elsewhere is left alone.
pub fn slowness_wanted(current: Option<(i32, i32)>, amplifier: i32) -> bool {
    match current {
        None => true,
        Some((a, _)) if a > amplifier => false,
        Some((a, _)) if a < amplifier => true,
        Some((_, d)) => d != -1 && d <= SLOWNESS_REFRESH,
    }
}
/// Whether an effect instance is the slowness carrying gave (so putting
/// down may take it away): faint, without particles, nothing under it, and
/// no longer than carrying gives it.
pub fn is_carry_slowness(duration: i32, ambient: bool, visible: bool, hides_other: bool) -> bool {
    ambient && !visible && !hides_other && (0..=SLOWNESS_TICKS).contains(&duration)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Status,
    Reload,
}
pub fn is_command(line: &str) -> bool {
    let line = line.trim().trim_start_matches('/');
    line == COMMAND || line.starts_with(&format!("{COMMAND} "))
}
/// `/carryblock ...`: None when the line is not Carry Block's.
pub fn parse_command(line: &str) -> Option<Result<Command, &'static str>> {
    if !is_command(line) {
        return None;
    }
    let mut words = line
        .trim()
        .trim_start_matches('/')
        .split_whitespace()
        .skip(1);
    let command = match (words.next(), words.next()) {
        (None | Some("status"), None) => Ok(Command::Status),
        (Some("reload"), None) => Ok(Command::Reload),
        _ => Err(USAGE),
    };
    Some(command)
}

/// The configuration as read from [`CONFIG_FILE`]: Err when the file is
/// broken (no new pick ups until it is fixed or reset).
pub type Loaded = Result<Config, String>;

/// A button of the settings page (Options > Mods > Carry Block > Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// The file is broken: its error. A click reads it again.
    Broken,
    Enabled,
    Survival,
    Creative,
    CreativeRules,
    Blocks,
    Mobs,
    MaxSize,
    Slowness,
    Hud,
    ActionBar,
    /// The deny lists, edited in the file. A click reads it again.
    Lists,
    /// Every setting back to its default (the lists kept).
    Defaults,
}
/// What a click on the settings page asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Write this configuration to the file; it applies at once.
    Save(Config),
    /// Read the file again (after editing it by hand).
    Reload,
}

/// The steps `maxEntitySize` cycles through (the default among them).
pub const MAX_ENTITY_SIZE_STEPS: [f32; 7] = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0];
/// Longest error shown on the page.
const ERROR_WIDTH: usize = 40;

/// The next step above `now`, back to the first after the last.
fn step<T: Copy + PartialOrd>(steps: &[T], now: T) -> T {
    steps.iter().copied().find(|&s| s > now).unwrap_or(steps[0])
}
fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}
fn roman(level: u8) -> &'static str {
    ["none", "I", "II", "III", "IV"][usize::from(level.min(4))]
}

/// The page's buttons, in order.
pub fn page(loaded: &Loaded) -> Vec<Setting> {
    if loaded.is_err() {
        return vec![Setting::Broken, Setting::Defaults];
    }
    vec![
        Setting::Enabled,
        Setting::Survival,
        Setting::Creative,
        Setting::CreativeRules,
        Setting::Blocks,
        Setting::Mobs,
        Setting::MaxSize,
        Setting::Slowness,
        Setting::Hud,
        Setting::ActionBar,
        Setting::Lists,
        Setting::Defaults,
    ]
}

impl Setting {
    /// Its button's label.
    pub fn label(self, loaded: &Loaded) -> String {
        let c = match loaded {
            Ok(c) => c,
            Err(error) => {
                return match self {
                    Self::Defaults => "Reset to defaults".into(),
                    _ => {
                        let mut short: String = error.chars().take(ERROR_WIDTH).collect();
                        if short.len() < error.len() {
                            short = format!("{}...", short.trim_end());
                        }
                        format!("{CONFIG_FILE}: {short} (click to reload)")
                    }
                };
            }
        };
        match self {
            Self::Broken => String::new(),
            Self::Enabled => format!("Picking up: {}", on_off(c.enabled)),
            Self::Survival => format!("Survival players may carry: {}", on_off(c.survival)),
            Self::Creative => format!("Creative players may carry: {}", on_off(c.creative)),
            Self::CreativeRules => if c.creative_ignores_rules {
                "Creative: ignores lists, size and monster rule"
            } else {
                "Creative: same rules as survival"
            }
            .into(),
            Self::Blocks => match c.blocks {
                BlockRule::WithData => "Blocks: with contents or data only",
                BlockRule::Any => "Blocks: any single block",
                BlockRule::Off => "Blocks: none",
            }
            .into(),
            Self::Mobs => match c.mobs {
                MobRule::Passive => "Mobs: animals, no monsters",
                MobRule::Any => "Mobs: monsters too",
                MobRule::Off => "Mobs: none",
            }
            .into(),
            Self::MaxSize => format!("Largest mob: {} blocks", c.max_entity_size),
            Self::Slowness => format!("Slowness while carrying: {}", roman(c.slowness)),
            Self::Hud => match c.hud {
                HudCorner::Off => "HUD box: off",
                HudCorner::TopLeft => "HUD box: top left",
                HudCorner::TopRight => "HUD box: top right",
                HudCorner::BottomLeft => "HUD box: bottom left",
                HudCorner::BottomRight => "HUD box: bottom right",
            }
            .into(),
            Self::ActionBar => format!("Action bar for other players: {}", on_off(c.action_bar)),
            Self::Lists => format!(
                "Deny lists: {} blocks, {} mobs ({CONFIG_FILE}, click to reload)",
                c.deny_blocks.len(),
                c.deny_entities.len()
            ),
            Self::Defaults => "Reset to defaults (deny lists kept)".into(),
        }
    }

    /// What a click on it does to `loaded`.
    pub fn click(self, loaded: &Loaded) -> Effect {
        let Ok(c) = loaded else {
            return match self {
                Self::Defaults => Effect::Save(Config::default()),
                _ => Effect::Reload,
            };
        };
        let mut c = c.clone();
        match self {
            Self::Broken | Self::Lists => return Effect::Reload,
            Self::Enabled => c.enabled = !c.enabled,
            Self::Survival => c.survival = !c.survival,
            Self::Creative => c.creative = !c.creative,
            Self::CreativeRules => c.creative_ignores_rules = !c.creative_ignores_rules,
            Self::Blocks => {
                c.blocks = match c.blocks {
                    BlockRule::WithData => BlockRule::Any,
                    BlockRule::Any => BlockRule::Off,
                    BlockRule::Off => BlockRule::WithData,
                }
            }
            Self::Mobs => {
                c.mobs = match c.mobs {
                    MobRule::Passive => MobRule::Any,
                    MobRule::Any => MobRule::Off,
                    MobRule::Off => MobRule::Passive,
                }
            }
            Self::MaxSize => c.max_entity_size = step(&MAX_ENTITY_SIZE_STEPS, c.max_entity_size),
            Self::Slowness => c.slowness = (c.slowness + 1) % 5,
            Self::Hud => {
                c.hud = match c.hud {
                    HudCorner::Off => HudCorner::TopLeft,
                    HudCorner::TopLeft => HudCorner::TopRight,
                    HudCorner::TopRight => HudCorner::BottomLeft,
                    HudCorner::BottomLeft => HudCorner::BottomRight,
                    HudCorner::BottomRight => HudCorner::Off,
                }
            }
            Self::ActionBar => c.action_bar = !c.action_bar,
            Self::Defaults => {
                c = Config {
                    deny_blocks: c.deny_blocks,
                    deny_entities: c.deny_entities,
                    ..Config::default()
                }
            }
        }
        Effect::Save(c)
    }
}

/// The settings page's labels (`native_settings`).
pub fn labels(loaded: &Loaded) -> Vec<String> {
    page(loaded).into_iter().map(|s| s.label(loaded)).collect()
}
/// A click on button `button` of the page `labels` gave (`native_settings_click`).
pub fn click(loaded: &Loaded, button: usize) -> Option<Effect> {
    page(loaded).get(button).map(|s| s.click(loaded))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    fn chest() -> BlockInfo {
        BlockInfo {
            name: "minecraft:chest".into(),
            properties: props(&[("facing", "north"), ("type", "single")]),
            block_entity: true,
            hardness: 2.5,
            item: Some("minecraft:chest".into()),
            empty: false,
            locked: false,
            ..BlockInfo::default()
        }
    }
    fn dirt() -> BlockInfo {
        BlockInfo {
            name: "minecraft:dirt".into(),
            block_entity: false,
            hardness: 0.5,
            item: Some("minecraft:dirt".into()),
            ..BlockInfo::default()
        }
    }
    fn pig() -> MobInfo {
        MobInfo {
            id: "minecraft:pig".into(),
            width: 0.9,
            height: 0.9,
            alive: true,
            ..MobInfo::default()
        }
    }
    fn no_tags(_: &str) -> bool {
        false
    }
    /// Anywhere a player may change a block.
    const FREE: Spot = Spot {
        in_world: true,
        reachable: true,
        spawn_protected: false,
        inside_border: true,
    };

    #[test]
    fn default_config_is_valid_and_round_trips() {
        let c = Config::default();
        c.validate().unwrap();
        assert_eq!(Config::from_json(&c.to_json()).unwrap(), c);
        assert_eq!(Config::from_json("{}").unwrap(), c);
        assert!(c.deny_blocks.contains(&"minecraft:bedrock".to_owned()));
        assert!(c.deny_entities.contains(&"minecraft:player".to_owned()));
        assert!(MAX_ENTITY_SIZE_STEPS.contains(&c.max_entity_size));
    }

    #[test]
    fn broken_configs_are_refused() {
        assert!(Config::from_json("{\"nope\": 1}").is_err());
        assert!(Config::from_json("{\"maxEntitySize\": 0}").is_err());
        assert!(Config::from_json("{\"maxEntitySize\": 99}").is_err());
        assert!(Config::from_json("{\"slowness\": 5}").is_err());
        assert!(Config::from_json("{\"blocks\": \"everything\"}").is_err());
        assert!(Config::from_json("{\"denyBlocks\": [\"\"]}").is_err());
        assert!(Config::from_json("{\"denyBlocks\": [\"Minecraft:Stone\"]}").is_err());
        assert!(Config::from_json("{\"denyEntities\": [\"#minecraft:raiders\"]}").is_err());
        assert!(Config::from_json("{\"denyBlocks\": [\"#minecraft:beds\"]}").is_ok());
        assert!(Config::from_json("{\"denyBlocks\": [\"minecraft:*_shulker_box\"]}").is_ok());
        assert!(Config::from_json("not json").is_err());
    }

    #[test]
    fn globs_and_namespaces() {
        assert!(glob("minecraft:*_bed", "minecraft:red_bed"));
        assert!(glob("*", "anything"));
        assert!(glob("a*b*c", "aXbYc"));
        assert!(!glob("a*b*c", "aXbY"));
        assert!(!glob("minecraft:chest", "minecraft:trapped_chest"));
        assert_eq!(namespaced("chest"), "minecraft:chest");
        assert_eq!(namespaced("mod:thing"), "mod:thing");
        let list = vec!["spawner".to_owned(), "#minecraft:beds".to_owned()];
        assert!(denied(&list, "minecraft:spawner", &no_tags));
        assert!(denied(&list, "spawner", &no_tags));
        assert!(!denied(&list, "minecraft:red_bed", &no_tags));
        assert!(denied(&list, "minecraft:red_bed", &|t| t == "minecraft:beds"));
    }

    #[test]
    fn who_may_pick_up() {
        let c = Config::default();
        assert_eq!(pick_up(&c, Mode::Survival, true, true), PickUp::Yes);
        assert_eq!(pick_up(&c, Mode::Creative, true, true), PickUp::Yes);
        assert_eq!(pick_up(&c, Mode::Survival, false, true), PickUp::No);
        assert_eq!(pick_up(&c, Mode::Survival, true, false), PickUp::No);
        assert_eq!(pick_up(&c, Mode::Adventure, true, true), PickUp::CantBuild);
        assert_eq!(pick_up(&c, Mode::Adventure, false, true), PickUp::No);
        assert_eq!(pick_up(&c, Mode::Adventure, true, false), PickUp::No);
        assert_eq!(pick_up(&c, Mode::Spectator, true, true), PickUp::No);
        let off = Config {
            survival: false,
            ..Config::default()
        };
        assert_eq!(pick_up(&off, Mode::Survival, true, true), PickUp::No);
        assert_eq!(pick_up(&off, Mode::Adventure, true, true), PickUp::No);
        assert_eq!(pick_up(&off, Mode::Creative, true, true), PickUp::Yes);
        let disabled = Config {
            enabled: false,
            ..Config::default()
        };
        assert_eq!(pick_up(&disabled, Mode::Creative, true, true), PickUp::No);
        assert_eq!(pick_up(&disabled, Mode::Adventure, true, true), PickUp::No);
    }

    #[test]
    fn adventure_players_hear_they_cant_build() {
        let c = Config::default();
        let pick = pick_up(&c, Mode::Adventure, true, true);
        // What a survival player would pick up: "You can't build here" (the
        // click is still the game's: the chest opens).
        let chested = block_verdict(&c, Mode::Adventure, false, &chest(), &no_tags);
        assert_eq!(chested, Verdict::Allow);
        assert_eq!(
            verdict_for(pick, chested),
            Verdict::Refuse(Refusal::CantBuild)
        );
        assert_eq!(Refusal::CantBuild.message(), "You can't build here");
        let piggy = mob_verdict(&c, Mode::Adventure, &pig());
        assert_eq!(
            verdict_for(pick, piggy),
            Verdict::Refuse(Refusal::CantBuild)
        );
        // Dirt stays silent; a refusal says what it says in survival.
        let dirt = block_verdict(&c, Mode::Adventure, false, &dirt(), &no_tags);
        assert_eq!(verdict_for(pick, dirt), Verdict::Pass);
        let locked = BlockInfo {
            locked: true,
            ..chest()
        };
        let locked = block_verdict(&c, Mode::Adventure, false, &locked, &no_tags);
        assert_eq!(verdict_for(pick, locked), Verdict::Refuse(Refusal::Locked));
        // Survival and creative players' verdicts are their own.
        for v in [
            Verdict::Allow,
            Verdict::Pass,
            Verdict::Refuse(Refusal::Denied),
        ] {
            assert_eq!(verdict_for(PickUp::Yes, v), v);
            assert_eq!(verdict_for(PickUp::No, v), Verdict::Pass);
        }
    }

    #[test]
    fn deny_listed_blocks_say_so_whatever_the_setting() {
        // Default settings (blocks with data only): bedrock has no block
        // entity, but it is on the deny list, so it says so.
        let c = Config::default();
        assert_eq!(c.blocks, BlockRule::WithData);
        let bedrock = BlockInfo {
            name: "minecraft:bedrock".into(),
            hardness: -1.0,
            item: Some("minecraft:bedrock".into()),
            ..dirt()
        };
        let refused = block_verdict(&c, Mode::Survival, false, &bedrock, &no_tags);
        assert_eq!(refused, Verdict::Refuse(Refusal::Denied));
        assert_eq!(Refusal::Denied.message(), "That can't be carried");
        assert_eq!(
            block_verdict(&c, Mode::Creative, false, &bedrock, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        // By a block tag too.
        let listed = Config {
            deny_blocks: vec!["#minecraft:dirt".into()],
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&listed, Mode::Survival, false, &dirt(), &|t| {
                t == "minecraft:dirt"
            }),
            Verdict::Refuse(Refusal::Denied)
        );
        // A plain block the setting leaves out stays silent (the click is
        // the game's, as vanilla's right click on dirt).
        assert_eq!(
            block_verdict(&c, Mode::Survival, false, &dirt(), &no_tags),
            Verdict::Pass
        );
        // Creative ignoring the rules: bedrock is like dirt then.
        let bypass = Config {
            creative_ignores_rules: true,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&bypass, Mode::Creative, false, &bedrock, &no_tags),
            Verdict::Pass
        );
        assert_eq!(
            block_verdict(&bypass, Mode::Survival, false, &bedrock, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        // Picking up blocks off: nothing is said.
        let off = Config {
            blocks: BlockRule::Off,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&off, Mode::Survival, false, &bedrock, &no_tags),
            Verdict::Pass
        );
    }

    #[test]
    fn a_bottom_box_clears_the_hotbar() {
        // The box the tester saw: 1280x720 at GUI scale 3 is 427 x 240 GUI
        // pixels, the hotbar 122.5..304.5; a box about 170 wide at the
        // bottom right starts at 255, over the hotbar's right end.
        assert!(hud_crosses_hotbar(HudCorner::BottomRight, 427.0, 170.0));
        assert!(hud_crosses_hotbar(HudCorner::BottomLeft, 427.0, 170.0));
        // A narrow one fits beside it (the column ends at 333.5).
        assert!(!hud_crosses_hotbar(HudCorner::BottomRight, 427.0, 80.0));
        assert!(!hud_crosses_hotbar(HudCorner::BottomLeft, 427.0, 80.0));
        // Top corners never meet it.
        assert!(!hud_crosses_hotbar(HudCorner::TopRight, 427.0, 400.0));
        assert!(!hud_crosses_hotbar(HudCorner::TopLeft, 427.0, 400.0));
        assert!(!hud_crosses_hotbar(HudCorner::Off, 427.0, 400.0));
        // Every GUI scale and window size: a box crosses exactly when its
        // side's room beside the column is too narrow for it.
        for (pixels, scale) in [
            ([1280.0, 720.0], 3.0),
            ([1920.0, 1080.0], 4.0),
            ([1920.0, 1080.0], 2.0),
            ([854.0, 480.0], 2.0),
            ([3840.0, 2160.0], 6.0),
            ([640.0, 480.0], 1.0),
        ] {
            let screen_w: f32 = pixels[0] / scale;
            let room = screen_w / 2.0 - HOTBAR_HALF_WIDTH - HUD_MARGIN;
            for w in [40.0, 80.0, 120.0, 170.0, 260.0] {
                for corner in [HudCorner::BottomLeft, HudCorner::BottomRight] {
                    assert_eq!(
                        hud_crosses_hotbar(corner, screen_w, w),
                        w > room,
                        "{pixels:?} at {scale}, {w} wide, {corner:?}"
                    );
                }
            }
        }
        // Lifted: an empty corner (one margin taken) reserves the rest of
        // the way up, so the box's bottom is a margin over the action bar.
        let height = 240.0;
        let taken = HUD_MARGIN;
        let room = hud_room_under(taken);
        assert_eq!(room, ABOVE_HOTBAR - HUD_MARGIN);
        // As `Corners::place` then places a box `h` high with `room` under it.
        let h = 22.0;
        let edge = taken + HUD_MARGIN;
        let y = height - edge - (room + h);
        assert_eq!(y + h, height - ABOVE_HOTBAR - HUD_MARGIN);
        // A corner already taken higher than that lifts it on its own.
        assert_eq!(hud_room_under(ABOVE_HOTBAR), 0.0);
        assert_eq!(hud_room_under(ABOVE_HOTBAR + 30.0), 0.0);
        assert_eq!(hud_room_under(50.0), ABOVE_HOTBAR - 50.0);
    }

    #[test]
    fn the_hud_box_goes_with_the_load() {
        let me = [1, 2, 3, 4];
        let other = [5, 6, 7, 8];
        let t0 = Instant::now();
        let mut board = HudBoard::new();
        assert_eq!(board.shown(me, t0, false), None);
        board.note(me, "Chest (8 stacks)", t0);
        board.note(other, "Pig", t0);
        assert_eq!(board.shown(me, t0, false), Some("Chest (8 stacks)"));
        // Not while dead (the death screen).
        assert_eq!(board.shown(me, t0, true), None);
        // Put down or dropped: gone at once, not after the timeout.
        board.forget(me);
        assert_eq!(board.shown(me, t0, false), None);
        assert_eq!(board.shown(other, t0, false), Some("Pig"));
        // A server that stops saying it: gone after the timeout.
        let later = t0 + Duration::from_millis(HUD_FRESH_MS);
        assert_eq!(
            board.shown(other, later - Duration::from_millis(1), false),
            Some("Pig")
        );
        assert_eq!(board.shown(other, later, false), None);
    }

    #[test]
    fn blocks_with_data_by_default() {
        let c = Config::default();
        assert_eq!(
            block_verdict(&c, Mode::Survival, false, &chest(), &no_tags),
            Verdict::Allow
        );
        assert_eq!(
            block_verdict(&c, Mode::Survival, false, &dirt(), &no_tags),
            Verdict::Pass
        );
        let any = Config {
            blocks: BlockRule::Any,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&any, Mode::Survival, false, &dirt(), &no_tags),
            Verdict::Allow
        );
        let none = Config {
            blocks: BlockRule::Off,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&none, Mode::Survival, false, &chest(), &no_tags),
            Verdict::Pass
        );
        let air = BlockInfo {
            empty: true,
            ..chest()
        };
        assert_eq!(
            block_verdict(&any, Mode::Survival, false, &air, &no_tags),
            Verdict::Pass
        );
    }

    #[test]
    fn denied_and_unsafe_blocks() {
        let c = Config::default();
        let portal = BlockInfo {
            name: "minecraft:end_portal".into(),
            block_entity: true,
            hardness: -1.0,
            item: None,
            ..BlockInfo::default()
        };
        assert_eq!(
            block_verdict(&c, Mode::Survival, false, &portal, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        let bypass = Config {
            creative_ignores_rules: true,
            ..Config::default()
        };
        // Even ignoring the lists, a block without an item stays put.
        assert_eq!(
            block_verdict(&bypass, Mode::Creative, false, &portal, &no_tags),
            Verdict::Refuse(Refusal::NoItem)
        );
        let bed = BlockInfo {
            name: "minecraft:red_bed".into(),
            properties: props(&[("part", "foot"), ("facing", "east")]),
            item: Some("minecraft:red_bed".into()),
            ..chest()
        };
        assert_eq!(
            block_verdict(&c, Mode::Creative, false, &bed, &no_tags),
            Verdict::Refuse(Refusal::TwoBlocks)
        );
        let any = Config {
            blocks: BlockRule::Any,
            ..Config::default()
        };
        let door = BlockInfo {
            name: "minecraft:oak_door".into(),
            properties: props(&[("half", "lower")]),
            ..dirt()
        };
        assert_eq!(
            block_verdict(&any, Mode::Survival, false, &door, &no_tags),
            Verdict::Refuse(Refusal::TwoBlocks)
        );
        let stairs = BlockInfo {
            name: "minecraft:oak_stairs".into(),
            properties: props(&[("half", "top")]),
            ..dirt()
        };
        assert_eq!(
            block_verdict(&any, Mode::Survival, false, &stairs, &no_tags),
            Verdict::Allow
        );
        let unbreakable = BlockInfo {
            name: "mod:vault".into(),
            hardness: -1.0,
            ..chest()
        };
        assert_eq!(
            block_verdict(&c, Mode::Survival, false, &unbreakable, &no_tags),
            Verdict::Refuse(Refusal::Unbreakable)
        );
        assert_eq!(
            block_verdict(&c, Mode::Creative, false, &unbreakable, &no_tags),
            Verdict::Allow
        );
        let locked = BlockInfo {
            locked: true,
            ..chest()
        };
        assert_eq!(
            block_verdict(&c, Mode::Creative, false, &locked, &no_tags),
            Verdict::Refuse(Refusal::Locked)
        );
        let bedrock = BlockInfo {
            name: "minecraft:bedrock".into(),
            hardness: -1.0,
            item: Some("minecraft:bedrock".into()),
            ..dirt()
        };
        assert_eq!(
            block_verdict(&any, Mode::Creative, false, &bedrock, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        let any_bypass = Config {
            creative_ignores_rules: true,
            ..any
        };
        assert_eq!(
            block_verdict(&any_bypass, Mode::Creative, false, &bedrock, &no_tags),
            Verdict::Allow
        );
        assert_eq!(
            block_verdict(&any_bypass, Mode::Survival, false, &bedrock, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
    }

    #[test]
    fn mob_rules() {
        let c = Config::default();
        assert_eq!(mob_verdict(&c, Mode::Survival, &pig()), Verdict::Allow);
        let zombie = MobInfo {
            id: "minecraft:zombie".into(),
            width: 0.6,
            height: 1.95,
            hostile: true,
            ..pig()
        };
        assert_eq!(
            mob_verdict(&c, Mode::Survival, &zombie),
            Verdict::Refuse(Refusal::Hostile)
        );
        let any = Config {
            mobs: MobRule::Any,
            max_entity_size: 2.0,
            ..Config::default()
        };
        assert_eq!(mob_verdict(&any, Mode::Survival, &zombie), Verdict::Allow);
        let cow = MobInfo {
            id: "minecraft:cow".into(),
            width: 0.9,
            height: 1.4,
            ..pig()
        };
        assert_eq!(mob_verdict(&c, Mode::Survival, &cow), Verdict::Allow);
        let villager = MobInfo {
            id: "minecraft:villager".into(),
            width: 0.6,
            height: 1.95,
            ..pig()
        };
        assert_eq!(
            mob_verdict(&c, Mode::Survival, &villager),
            Verdict::Refuse(Refusal::TooBig)
        );
        let wither = MobInfo {
            id: "minecraft:wither".into(),
            hostile: true,
            ..pig()
        };
        assert_eq!(
            mob_verdict(&any, Mode::Creative, &wither),
            Verdict::Refuse(Refusal::Denied)
        );
        let bypass = Config {
            creative_ignores_rules: true,
            ..Config::default()
        };
        assert_eq!(
            mob_verdict(&bypass, Mode::Creative, &villager),
            Verdict::Allow
        );
        assert_eq!(
            mob_verdict(&bypass, Mode::Survival, &villager),
            Verdict::Refuse(Refusal::TooBig)
        );
        for (mob, refusal) in [
            (
                MobInfo {
                    riding: true,
                    ..pig()
                },
                Refusal::Riding,
            ),
            (
                MobInfo {
                    ridden: true,
                    ..pig()
                },
                Refusal::Riding,
            ),
            (
                MobInfo {
                    leashed: true,
                    ..pig()
                },
                Refusal::Leashed,
            ),
            (
                MobInfo {
                    inventory: true,
                    ..pig()
                },
                Refusal::Inventory,
            ),
            (
                MobInfo {
                    owned_by_other: true,
                    ..pig()
                },
                Refusal::NotYours,
            ),
        ] {
            // These hold in creative too, whatever the settings.
            assert_eq!(
                mob_verdict(&bypass, Mode::Creative, &mob),
                Verdict::Refuse(refusal)
            );
        }
        let gone = MobInfo {
            removed: true,
            ..pig()
        };
        assert_eq!(mob_verdict(&c, Mode::Survival, &gone), Verdict::Pass);
        let dead = MobInfo {
            alive: false,
            ..pig()
        };
        assert_eq!(mob_verdict(&c, Mode::Survival, &dead), Verdict::Pass);
        let off = Config {
            mobs: MobRule::Off,
            ..Config::default()
        };
        assert_eq!(mob_verdict(&off, Mode::Survival, &pig()), Verdict::Pass);
    }

    #[test]
    fn states_parse_and_print() {
        let s = State::parse("minecraft:chest[facing=north,type=left,waterlogged=false]").unwrap();
        assert_eq!(s.name, "minecraft:chest");
        assert_eq!(s.properties.get("type").map(String::as_str), Some("left"));
        assert_eq!(
            s.text(),
            "minecraft:chest[facing=north,type=left,waterlogged=false]"
        );
        assert_eq!(State::parse("stone").unwrap().text(), "minecraft:stone");
        assert!(State::parse("").is_none());
        assert!(State::parse("minecraft:chest[facing]").is_none());
        assert!(State::parse("minecraft:chest[facing=north").is_none());
        assert!(State::parse("minecraft:chest[=north]").is_none());
    }

    #[test]
    fn put_down_state_keeps_contents_properties() {
        let carried = State::parse("minecraft:furnace[facing=north,lit=true]").unwrap();
        let placed = State::parse("minecraft:furnace[facing=east,lit=false]").unwrap();
        assert_eq!(
            placed_state(&carried, &placed).text(),
            "minecraft:furnace[facing=east,lit=true]"
        );
        let hive = State::parse("minecraft:beehive[facing=north,honey_level=5]").unwrap();
        let placed = State::parse("minecraft:beehive[facing=south,honey_level=0]").unwrap();
        assert_eq!(
            placed_state(&hive, &placed).text(),
            "minecraft:beehive[facing=south,honey_level=5]"
        );
        let chest =
            State::parse("minecraft:chest[facing=north,type=left,waterlogged=true]").unwrap();
        let placed =
            State::parse("minecraft:chest[facing=west,type=single,waterlogged=false]").unwrap();
        assert_eq!(placed_state(&chest, &placed), placed);
        // A wall sign put down on top of a block becomes a standing sign.
        let wall = State::parse("minecraft:oak_wall_sign[facing=north,waterlogged=false]").unwrap();
        let standing = State::parse("minecraft:oak_sign[rotation=4,waterlogged=false]").unwrap();
        assert_eq!(placed_state(&wall, &standing), standing);
        let double = State::parse("minecraft:stone_slab[type=double,waterlogged=false]").unwrap();
        let single = State::parse("minecraft:stone_slab[type=bottom,waterlogged=false]").unwrap();
        assert_eq!(
            placed_state(&double, &single).text(),
            "minecraft:stone_slab[type=double,waterlogged=false]"
        );
    }

    #[test]
    fn released_state_is_alone_and_closed() {
        let chest =
            State::parse("minecraft:chest[facing=north,type=right,waterlogged=false]").unwrap();
        assert_eq!(
            released_state(&chest, true).text(),
            "minecraft:chest[facing=north,type=single,waterlogged=true]"
        );
        let barrel = State::parse("minecraft:barrel[facing=up,open=true]").unwrap();
        assert_eq!(
            released_state(&barrel, true).text(),
            "minecraft:barrel[facing=up,open=false]"
        );
    }

    #[test]
    fn names_and_texts() {
        assert_eq!(display_name("minecraft:oak_wall_sign"), "Oak Wall Sign");
        assert_eq!(display_name("chest"), "Chest");
        assert_eq!(display_name("mod:big/thing_x"), "Big Thing X");
        assert_eq!(summary("Chest", Some(5)), "Chest (5 stacks)");
        assert_eq!(summary("Chest", Some(1)), "Chest (1 stack)");
        assert_eq!(summary("Chest", Some(0)), "Chest");
        assert_eq!(summary("Pig", None), "Pig");
        assert_eq!(hud_lines("Pig")[0], "Carrying: Pig");
        assert!(action_bar_text("Pig").starts_with("Carrying Pig"));
        assert!(status_text(Some("Pig")).ends_with("carrying Pig"));
        assert!(status_text(None).contains("not carrying"));
        for r in [Refusal::Denied, Refusal::TooBig, Refusal::Locked] {
            assert!(!r.message().is_empty());
        }
    }

    #[test]
    fn release_spots_start_at_the_feet() {
        let spots = release_spots(RELEASE_RADIUS);
        assert_eq!(spots[0], [0, 0, 0]);
        assert_eq!(spots.len(), 5 * 5 * 4);
        let mut unique = spots.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), spots.len());
        assert!(spots.iter().all(|s| s[1] >= -1));
        // Above the feet before below them, at the same distance.
        let up = spots.iter().position(|s| *s == [0, 1, 0]).unwrap();
        let down = spots.iter().position(|s| *s == [0, -1, 0]).unwrap();
        assert!(up < down);
    }

    #[test]
    fn held_use_button_is_not_a_click() {
        let mut clicks = Clicks::default();
        let me = [1, 2, 3, 4];
        assert!(clicks.fresh(me, 100));
        assert!(!clicks.fresh(me, 100)); // the second packet of the same click
        assert!(!clicks.fresh(me, 104)); // held: repeats every 4 ticks
        assert!(!clicks.fresh(me, 108));
        assert!(clicks.fresh(me, 120));
        assert!(clicks.fresh([9, 9, 9, 9], 121));
        assert!(clicks.fresh(me, 50)); // a new world counts again
        for i in 0..300 {
            clicks.fresh([i, 0, 0, 0], 1000);
        }
        assert!(clicks.last.len() <= 301);
        clicks.fresh([0, 0, 0, 1], 2000);
        assert!(clicks.last.len() < 10);
    }

    #[test]
    fn slowness_rules() {
        let c = Config::default();
        assert_eq!(slowness_amplifier(&c), Some(0));
        let none = Config {
            slowness: 0,
            ..Config::default()
        };
        assert_eq!(slowness_amplifier(&none), None);
        assert!(slowness_wanted(None, 0));
        assert!(slowness_wanted(Some((0, 5)), 0));
        assert!(!slowness_wanted(Some((0, 30)), 0));
        assert!(!slowness_wanted(Some((0, -1)), 0));
        assert!(!slowness_wanted(Some((2, 5)), 0));
        assert!(slowness_wanted(Some((0, 600)), 1));
        assert!(is_carry_slowness(30, true, false, false));
        assert!(!is_carry_slowness(30, false, true, false));
        assert!(!is_carry_slowness(600, true, false, false));
        assert!(!is_carry_slowness(30, true, false, true));
        assert!(!is_carry_slowness(-1, true, false, false));
    }

    #[test]
    fn commands() {
        assert_eq!(parse_command("carryblock"), Some(Ok(Command::Status)));
        assert_eq!(
            parse_command("/carryblock status"),
            Some(Ok(Command::Status))
        );
        assert_eq!(
            parse_command("carryblock reload"),
            Some(Ok(Command::Reload))
        );
        assert_eq!(parse_command("carryblock drop"), Some(Err(USAGE)));
        assert_eq!(parse_command("carryblock reload now"), Some(Err(USAGE)));
        assert_eq!(parse_command("carryblockx"), None);
        assert_eq!(parse_command("give @s stone"), None);
    }

    #[test]
    fn settings_page() {
        let loaded: Loaded = Ok(Config::default());
        let labels = labels(&loaded);
        assert_eq!(labels.len(), page(&loaded).len());
        assert_eq!(labels[0], "Picking up: ON");
        assert!(labels.iter().all(|l| !l.is_empty()));
        assert_eq!(click(&loaded, 99), None);
        let Some(Effect::Save(c)) = click(&loaded, 0) else {
            panic!("a save");
        };
        assert!(!c.enabled);
        // Each cycling button comes back to its default.
        for (button, steps) in [(4, 3), (5, 3), (6, 7), (7, 5), (8, 5)] {
            let mut now: Loaded = Ok(Config::default());
            for _ in 0..steps {
                let Some(Effect::Save(c)) = click(&now, button) else {
                    panic!("a save");
                };
                c.validate().unwrap();
                now = Ok(c);
            }
            assert_eq!(now, Ok(Config::default()), "button {button}");
        }
        let custom = Config {
            deny_blocks: vec!["minecraft:spawner".into()],
            slowness: 3,
            ..Config::default()
        };
        let lists = page(&Ok(custom.clone()))
            .iter()
            .position(|s| *s == Setting::Lists)
            .unwrap();
        assert_eq!(click(&Ok(custom.clone()), lists), Some(Effect::Reload));
        let defaults = page(&Ok(custom.clone())).len() - 1;
        let Some(Effect::Save(reset)) = click(&Ok(custom.clone()), defaults) else {
            panic!("a save");
        };
        assert_eq!(reset.slowness, 1);
        assert_eq!(reset.deny_blocks, custom.deny_blocks);
    }

    #[test]
    fn broken_file_page() {
        let loaded: Loaded = Err("expected value at line 1 column 1 and some more words".into());
        let labels = labels(&loaded);
        assert_eq!(labels.len(), 2);
        assert!(labels[0].contains("click to reload"));
        assert!(labels[0].contains("..."));
        assert_eq!(click(&loaded, 0), Some(Effect::Reload));
        assert_eq!(click(&loaded, 1), Some(Effect::Save(Config::default())));
    }

    fn mode_and_spot_cases() -> Vec<(Mode, Spot, Result<(), Refusal>)> {
        let free = FREE;
        vec![
            (Mode::Survival, free, Ok(())),
            (Mode::Creative, free, Ok(())),
            (Mode::Adventure, free, Err(Refusal::CantBuild)),
            (Mode::Spectator, free, Err(Refusal::CantBuild)),
            (
                Mode::Creative,
                Spot {
                    spawn_protected: true,
                    ..free
                },
                Err(Refusal::SpawnProtection),
            ),
            (
                Mode::Creative,
                Spot {
                    inside_border: false,
                    ..free
                },
                Err(Refusal::Border),
            ),
            (
                Mode::Survival,
                Spot {
                    reachable: false,
                    ..free
                },
                Err(Refusal::TooFar),
            ),
            // Spawn protection is told before the border, as the game does.
            (
                Mode::Survival,
                Spot {
                    spawn_protected: true,
                    inside_border: false,
                    ..free
                },
                Err(Refusal::SpawnProtection),
            ),
        ]
    }

    #[test]
    fn picking_up_a_block_is_breaking_it() {
        for (mode, spot, expected) in mode_and_spot_cases() {
            assert_eq!(may_break(mode, spot), expected, "{mode:?} {spot:?}");
        }
        let unloaded = Spot {
            in_world: false,
            ..FREE
        };
        assert_eq!(
            may_break(Mode::Creative, unloaded),
            Err(Refusal::OutOfWorld)
        );
        assert!(may_build(Mode::Survival) && may_build(Mode::Creative));
        assert!(!may_build(Mode::Adventure) && !may_build(Mode::Spectator));
    }

    #[test]
    fn putting_down_is_placing_at_the_target() {
        let free = FREE;
        for (mode, clicked, expected) in mode_and_spot_cases() {
            assert_eq!(may_use_on(mode, clicked), expected, "{mode:?} {clicked:?}");
            assert_eq!(may_place(mode, clicked, free), expected);
        }
        // The clicked block is fine, the block it goes into is not.
        for (target, refusal) in [
            (
                Spot {
                    spawn_protected: true,
                    ..free
                },
                Refusal::SpawnProtection,
            ),
            (
                Spot {
                    inside_border: false,
                    ..free
                },
                Refusal::Border,
            ),
            (
                Spot {
                    in_world: false,
                    ..free
                },
                Refusal::OutOfWorld,
            ),
        ] {
            assert_eq!(may_place(Mode::Survival, free, target), Err(refusal));
        }
        // Only the clicked block needs to be in reach (vanilla).
        let far = Spot {
            reachable: false,
            ..free
        };
        assert_eq!(may_place(Mode::Survival, free, far), Ok(()));
        assert_eq!(may_place(Mode::Survival, far, free), Err(Refusal::TooFar));
        // Adventure: nothing is put down, even somewhere free.
        assert_eq!(
            may_place(Mode::Adventure, free, free),
            Err(Refusal::CantBuild)
        );
    }

    #[test]
    fn taking_a_mob_and_releasing_a_load() {
        let free = FREE;
        assert_eq!(may_take_mob(free), Ok(()));
        // Reach is the mob ray's: an unreachable flag does not matter.
        assert_eq!(
            may_take_mob(Spot {
                reachable: false,
                ..free
            }),
            Ok(())
        );
        for (spot, refusal) in [
            (
                Spot {
                    spawn_protected: true,
                    ..free
                },
                Refusal::SpawnProtection,
            ),
            (
                Spot {
                    inside_border: false,
                    ..free
                },
                Refusal::Border,
            ),
            (
                Spot {
                    in_world: false,
                    ..free
                },
                Refusal::OutOfWorld,
            ),
        ] {
            assert_eq!(may_take_mob(spot), Err(refusal));
            assert!(!may_release_at(spot));
        }
        assert!(may_release_at(free));
    }

    #[test]
    fn spawn_protection_as_a_dedicated_server_has_it() {
        let me = 7u128;
        let rules = SpawnRules {
            radius: 16,
            ops: [1u128].into_iter().collect(),
            known: true,
        };
        let spawn = Some([100, 64, -50]);
        let at = [100, 64, -50];
        // Within the square of the radius (inclusive), at any height.
        assert!(under_spawn_protection(&rules, true, Some(me), spawn, at));
        for pos in [[116, -60, -34], [84, 300, -66]] {
            assert!(under_spawn_protection(&rules, true, Some(me), spawn, pos));
        }
        for pos in [[117, 64, -50], [100, 64, -67]] {
            assert!(!under_spawn_protection(&rules, true, Some(me), spawn, pos));
        }
        // An operator is exempt.
        assert!(!under_spawn_protection(&rules, true, Some(1), spawn, at));
        // Single player and LAN (an integrated server), and single player's
        // own player, never are.
        assert!(!under_spawn_protection(&rules, false, Some(me), spawn, at));
        assert!(!under_spawn_protection(&rules, true, None, spawn, at));
        // Not the world spawn's level (the Nether, the End).
        assert!(!under_spawn_protection(&rules, true, Some(me), None, at));
        // No operators at all, or a radius of 0 or less: nobody is.
        let no_ops = SpawnRules {
            ops: BTreeSet::new(),
            ..rules.clone()
        };
        assert!(!under_spawn_protection(&no_ops, true, Some(me), spawn, at));
        for radius in [0, -3] {
            let off = SpawnRules {
                radius,
                ..rules.clone()
            };
            assert!(!under_spawn_protection(&off, true, Some(me), spawn, at));
        }
        // Before the server said anything, a dedicated server's player is
        // protected (it fails closed); single player still is not.
        let unknown = SpawnRules::default();
        let far = [5000, 64, 5000];
        assert!(under_spawn_protection(&unknown, true, Some(me), spawn, far));
        assert!(!under_spawn_protection(
            &unknown,
            false,
            Some(me),
            spawn,
            at
        ));
        // Far coordinates don't overflow.
        let edge = [i32::MIN, 0, i32::MAX];
        assert!(!under_spawn_protection(&rules, true, Some(me), spawn, edge));
    }

    #[test]
    fn block_interaction_range() {
        let eye = [0.5, 1.62, 0.5];
        // 4.5 + 1 in survival, to the box of the block, not its centre.
        assert!(within_block_range(eye, 5.5, [5, 1, 0]));
        assert!(!within_block_range(eye, 5.5, [6, 1, 0]));
        assert!(within_block_range(eye, 5.5, [0, 0, 0]));
        assert!(within_block_range(eye, 6.0, [-6, 1, 0]));
        assert!(!within_block_range(eye, 6.0, [-7, 1, 0]));
        assert!(!within_block_range(eye, 5.5, [4, 5, 4]));
    }

    #[test]
    fn game_master_blocks_and_open_containers() {
        let c = Config::default();
        assert!(is_game_master_block("minecraft:command_block", &|_| false));
        assert!(is_game_master_block("jigsaw", &|_| false));
        assert!(is_game_master_block("mod:better_command_block", &|c| c == "CommandBlock"));
        assert!(!is_game_master_block("minecraft:chest", &|c| c == "ChestBlock"));
        let command = BlockInfo {
            name: "minecraft:command_block".into(),
            hardness: -1.0,
            item: Some("minecraft:command_block".into()),
            game_master: true,
            ..chest()
        };
        let bypass = Config {
            creative_ignores_rules: true,
            deny_blocks: Vec::new(),
            ..Config::default()
        };
        // No setting lets a player who may not use them move one.
        for config in [&c, &bypass] {
            assert_eq!(
                block_verdict(config, Mode::Creative, false, &command, &no_tags),
                Verdict::Refuse(Refusal::GameMaster)
            );
        }
        // An operator in creative may, unless the deny list says no.
        assert_eq!(
            block_verdict(&bypass, Mode::Creative, true, &command, &no_tags),
            Verdict::Allow
        );
        assert_eq!(
            block_verdict(&c, Mode::Creative, true, &command, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        let open = BlockInfo {
            in_use: true,
            ..chest()
        };
        for (config, mode) in [(&c, Mode::Survival), (&bypass, Mode::Creative)] {
            assert_eq!(
                block_verdict(config, mode, true, &open, &no_tags),
                Verdict::Refuse(Refusal::InUse)
            );
        }
    }

    #[test]
    fn unobtainable_blocks_are_denied_by_default() {
        let c = Config::default();
        for name in [
            "minecraft:spawner",
            "minecraft:trial_spawner",
            "minecraft:vault",
            "minecraft:end_gateway",
        ] {
            let block = BlockInfo {
                name: name.into(),
                item: Some(name.into()),
                ..chest()
            };
            assert_eq!(
                block_verdict(&c, Mode::Survival, false, &block, &no_tags),
                Verdict::Refuse(Refusal::Denied),
                "{name}"
            );
        }
        let budding = BlockInfo {
            name: "minecraft:budding_amethyst".into(),
            item: Some("minecraft:budding_amethyst".into()),
            ..dirt()
        };
        let any = Config {
            blocks: BlockRule::Any,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&any, Mode::Survival, false, &budding, &no_tags),
            Verdict::Refuse(Refusal::Denied)
        );
        // An end gateway has no item: refused even when the list allows it.
        let gateway = BlockInfo {
            name: "minecraft:end_gateway".into(),
            item: None,
            ..chest()
        };
        let bypass = Config {
            creative_ignores_rules: true,
            ..Config::default()
        };
        assert_eq!(
            block_verdict(&bypass, Mode::Creative, false, &gateway, &no_tags),
            Verdict::Refuse(Refusal::NoItem)
        );
    }

    #[test]
    fn mobs_nobody_could_take() {
        let bypass = Config {
            creative_ignores_rules: true,
            ..Config::default()
        };
        for (mob, refusal) in [
            (
                MobInfo {
                    leading: true,
                    ..pig()
                },
                Refusal::Leading,
            ),
            (
                MobInfo {
                    trading: true,
                    ..pig()
                },
                Refusal::Trading,
            ),
            (
                MobInfo {
                    sleeping: true,
                    ..pig()
                },
                Refusal::Sleeping,
            ),
        ] {
            for mode in [Mode::Survival, Mode::Creative] {
                assert_eq!(
                    mob_verdict(&bypass, mode, &mob),
                    Verdict::Refuse(refusal),
                    "{mode:?}"
                );
            }
        }
        let invulnerable = MobInfo {
            invulnerable: true,
            ..pig()
        };
        assert_eq!(
            mob_verdict(&Config::default(), Mode::Survival, &invulnerable),
            Verdict::Refuse(Refusal::Invulnerable)
        );
        assert_eq!(
            mob_verdict(&Config::default(), Mode::Creative, &invulnerable),
            Verdict::Allow
        );
        for r in [
            Refusal::GameMaster,
            Refusal::InUse,
            Refusal::SpawnProtection,
            Refusal::Border,
            Refusal::OutOfWorld,
            Refusal::TooFar,
            Refusal::CantBuild,
        ] {
            assert!(!r.message().is_empty());
        }
    }

    #[test]
    fn natural_spawners_are_put_down_as_placed_ones() {
        let shrieker =
            State::parse("minecraft:sculk_shrieker[can_summon=true,shrieking=false]").unwrap();
        let placed =
            State::parse("minecraft:sculk_shrieker[can_summon=false,shrieking=false]").unwrap();
        assert_eq!(placed_state(&shrieker, &placed), placed);
        assert_eq!(released_state(&shrieker, false), placed);
        let heart = State::parse("minecraft:creaking_heart[axis=y,natural=true]").unwrap();
        let placed = State::parse("minecraft:creaking_heart[axis=x,natural=false]").unwrap();
        assert_eq!(placed_state(&heart, &placed), placed);
        let released = released_state(&heart, false);
        assert_eq!(
            released.properties.get("natural").map(String::as_str),
            Some("false")
        );
    }
}
