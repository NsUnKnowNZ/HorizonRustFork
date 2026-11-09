// Carry Block's adapter to the game (Horizon 142388e1c136e622652ec7505af15d8ad195e041,
// see horizon-mixin.toml). Injected as `crate::game::carryblock_native`; it
// does what the game-independent rules in `crate::game::carryblock`
// (runtime.rs) decide, in the game that simulates the world: in-frame single
// player, single player's server thread (a LAN host's too) and the dedicated
// server. A window joined to a server only draws its HUD box.
//
// What a player carries lives in their own saved compound (`CARRY_KEY` of
// `Level::player_extra` while they act, `Guest::saved` otherwise), so it is
// saved, loaded, swapped in for a LAN guest's turn and kept through a
// disconnect exactly like their inventory. Nothing is ever an item while it
// is carried.
//
// A pick up is checked as the game checks a player breaking that block (a
// mob: interacting with it), a put down as placing a block where it goes
// (runtime.rs `may_break`, `may_take_mob`, `may_place`): in the level, within
// reach, spawn protection, the world border, `mayBuild`, game master blocks.
// What a guest's turn cannot see of the hub (spawn protection's radius and
// operators, permission levels, open containers, trades) is read at every
// tick (`HubView`).
use super::carryblock::{
    self as ca, BlockInfo, Command, Config, Effect, HudBoard, HudCorner, Loaded, MobInfo, Mode,
    PickUp, Refusal, SpawnRules, Spot, State, Uuid, Verdict,
};
use super::{Dim, Game, GuestUse, Hit, Screen};
use horizon_client::blocks::Fluid;
use horizon_client::command::args::GameMode;
use horizon_client::containers::BlockEntity;
use horizon_client::effects::{EffectInstance, PlayerEffects};
use horizon_client::extras::Corner;
use horizon_client::extras_hud::Corners;
use horizon_client::first_person::Hand;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::lan::menu::Slots;
use horizon_client::mobs::leash::Holder;
use horizon_client::text::Text;
use horizon_core::geometry::{Direction, Vec3};
use horizon_core::position::BlockPos;
use horizon_protocol::packets::play::{self, Clientbound as C};
use horizon_world::level::{LevelEvent, PlaceContext, flags};
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

/// What the window's game and single player's server thread share (one
/// process): the configuration, and what each player carries as the server
/// last said, for the window's HUD box.
struct Shared {
    /// `config.json` as last read; None until something needs it.
    config: Option<Loaded>,
    hud: HudBoard,
}
static SHARED: std::sync::Mutex<Shared> = std::sync::Mutex::new(Shared {
    config: None,
    hud: HudBoard::new(),
});
/// Options > Mods' on/off as the window's game last saw it: single player's
/// server thread keeps the options it started with, so it follows the
/// window when there is one.
static WINDOW: AtomicU8 = AtomicU8::new(WINDOW_UNSEEN);
const WINDOW_UNSEEN: u8 = 0;
const WINDOW_ON: u8 = 1;
const WINDOW_OFF: u8 = 2;

/// What the hub knows that a LAN guest's own turn cannot see (the hub is out
/// of `Game::lan` while a guest acts): read at every tick and before the
/// in-frame player's own click, used by the next guest's.
#[derive(Default)]
struct HubView {
    /// `spawn-protection` and the operators.
    spawn: SpawnRules,
    /// Each player's permission level (`Player.canUseGameMasterBlocks`).
    permissions: HashMap<u128, u8>,
    /// The blocks whose container a player has open, by level.
    open: HashSet<(&'static str, (i32, i32, i32))>,
    /// The mobs a player trades with, by level.
    trading: HashSet<(&'static str, u32)>,
}

thread_local! {
    /// Each player's last use, on the thread of the game that simulates
    /// the world (a held use button must not flip between pick up and put
    /// down).
    static CLICKS: RefCell<ca::Clicks> = RefCell::new(ca::Clicks::default());
    /// The hub as last read, on the same thread.
    static HUB: RefCell<HubView> = RefCell::new(HubView::default());
}

/// Never held while the game runs.
fn shared<T>(run: impl FnOnce(&mut Shared) -> T) -> T {
    let mut shared = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    run(&mut shared)
}

/// `config.json` in the mod's settings folder; a missing one is written with
/// the defaults, so there is a file to edit the deny lists in.
fn read_config(game: &Game<'_>) -> Loaded {
    match game.native_setting(ca::ID, ca::CONFIG_FILE) {
        Some(text) => Config::from_json(&text).map_err(|e| format!("{}: {e}", ca::CONFIG_FILE)),
        None => {
            let config = Config::default();
            game.native_set_setting(ca::ID, ca::CONFIG_FILE, &config.to_json());
            Ok(config)
        }
    }
}
/// Puts `loaded` in effect, for every player at once.
fn apply(loaded: Loaded) {
    if let Err(error) = &loaded {
        eprintln!("Carry Block: no picking up: {error}");
    }
    shared(|s| s.config = Some(loaded));
}
/// The configuration as read (once, by whichever thread needs it first).
fn loaded(game: &Game<'_>) -> Loaded {
    if let Some(loaded) = shared(|s| s.config.clone()) {
        return loaded;
    }
    let read = read_config(game);
    shared(|s| {
        s.config
            .get_or_insert_with(|| {
                if let Err(error) = &read {
                    eprintln!("Carry Block: no picking up: {error}");
                }
                read
            })
            .clone()
    })
}
/// The configuration in effect: a broken file stops new pick ups (putting
/// down still works) until it is fixed.
fn config(game: &Game<'_>) -> Config {
    loaded(game).unwrap_or_else(|_| Config {
        enabled: false,
        ..Config::default()
    })
}
/// Not turned off in Options > Mods: every hook passes through untouched
/// when this is false (the list and settings hooks still run, so the row
/// can turn it back on).
fn on(game: &Game<'_>) -> bool {
    let own = game.native_on(ca::ID);
    if game.native_window() {
        let seen = if own { WINDOW_ON } else { WINDOW_OFF };
        WINDOW.store(seen, Ordering::Relaxed);
        return own;
    }
    match WINDOW.load(Ordering::Relaxed) {
        WINDOW_UNSEEN => own,
        seen => seen == WINDOW_ON,
    }
}

/// What a player carries, from their saved compound.
fn carried_in(extra: Option<&Tag>) -> Option<&Tag> {
    extra?.get(ca::CARRY_KEY)
}
/// Sets (Some) or clears (None) what a player carries.
fn put_carried(extra: &mut Option<Tag>, carried: Option<Tag>) {
    match carried {
        Some(tag) => {
            let root = extra.get_or_insert_with(|| Tag::Compound(Compound::new()));
            if let Tag::Compound(map) = root {
                map.insert(ca::CARRY_KEY.into(), tag);
            }
        }
        None => {
            if let Some(Tag::Compound(map)) = extra.as_mut() {
                map.remove(&JavaString::from(ca::CARRY_KEY));
            }
        }
    }
}
fn text_of(tag: &Tag, key: &str) -> Option<String> {
    horizon_client::save::text(tag.get(key))
}
/// What the HUD calls what is carried.
fn carried_summary(carried: &Tag) -> String {
    text_of(carried, ca::KEY_NAME).unwrap_or_else(|| "something".into())
}
/// A container's lock (`lock`, before 1.21.2 `Lock`).
fn is_locked(data: &Tag) -> bool {
    match (data.get("lock"), data.get("Lock")) {
        (Some(Tag::Compound(c)), _) => !c.is_empty(),
        (Some(_), _) => true,
        (None, Some(Tag::String(s))) => !s.0.is_empty(),
        _ => false,
    }
}
/// The window's HUD box hears what `who` carries.
fn note_hud(who: Uuid, summary: &str) {
    shared(|s| s.hud.note(who, summary, Instant::now()));
}
/// The window's HUD box hears `who` carries nothing any more (put down,
/// dropped, dead): it goes at once.
fn forget_hud(who: Uuid) {
    shared(|s| s.hud.forget(who));
}
/// Gives a carrier their slowness when it is missing or about to end; true
/// when it changed.
fn give_slowness(effects: &mut PlayerEffects, amplifier: i32) -> bool {
    let current = effects.get("slowness").map(|e| (e.amplifier, e.duration));
    if !ca::slowness_wanted(current, amplifier) {
        return false;
    }
    let mut effect = EffectInstance::new("slowness", ca::SLOWNESS_TICKS, amplifier);
    effect.ambient = true;
    effect.visible = false;
    effects.add(effect)
}
/// The item of a block, holding its block entity's data (Ctrl + Pick Block's
/// form): what a dead carrier drops when there is no room to leave the block.
fn item_with_data(item: &str, data: Option<&Tag>) -> ItemStack {
    let mut stack = ItemStack::new(item, 1);
    if let Some(data) = data {
        let patch = horizon_client::block_entity_item::pick(data);
        if !patch.is_empty() {
            let mut all = match horizon_client::item_data::encode_components(&stack) {
                Some(Tag::Compound(c)) => c,
                _ => Compound::new(),
            };
            all.extend(patch);
            (stack.damage, stack.extra) =
                horizon_client::item_data::decode_components(Some(&Tag::Compound(all)));
        }
    }
    stack
}
fn direction(face: [i32; 3]) -> Direction {
    match face {
        [0, 1, 0] => Direction::Up,
        [0, -1, 0] => Direction::Down,
        [0, 0, -1] => Direction::North,
        [0, 0, 1] => Direction::South,
        [-1, 0, 0] => Direction::West,
        _ => Direction::East,
    }
}
fn doubles(values: &[f64]) -> Tag {
    Tag::List(values.iter().map(|v| Tag::Double(v.to_bits())).collect())
}

impl Game<'_> {
    fn ca_mode(&self) -> Mode {
        match self.mode {
            GameMode::Survival => Mode::Survival,
            GameMode::Creative => Mode::Creative,
            GameMode::Adventure => Mode::Adventure,
            GameMode::Spectator => Mode::Spectator,
        }
    }
    /// Sneaking: the left shift key, onto which a LAN guest's sneak flag is
    /// mapped for their turn.
    fn ca_sneaking(&self) -> bool {
        self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
            || self.player.as_ref().is_some_and(|p| p.shift_key_down)
    }
    fn ca_in_world(&self, p: [i32; 3]) -> bool {
        p[1] >= self.world.shape.min_y && p[1] < self.world.shape.max_y() && self.world.loaded(p[0], p[2])
    }
    /// A line in the acting player's action bar: a guest gets it at the
    /// end of their turn.
    fn ca_tell(&mut self, text: &str) {
        self.ca_say(Text::literal(text.to_owned()));
    }
    fn ca_say(&mut self, line: Text) {
        if self.lan_acting.is_some() {
            self.guest_packets.push(C::SystemChat(play::SystemChat {
                content: horizon_client::lan::data::component_nbt(&line.component()),
                overlay: true,
            }));
        } else {
            self.titles.set_overlay(line, false);
        }
    }
    /// Whether block `name` is in block tag `tag` (with or without the
    /// `minecraft:` namespace on either).
    fn ca_block_in_tag(&self, tag: &str, name: &str) -> bool {
        let items = self.res.items();
        let short_tag = tag.strip_prefix("minecraft:").unwrap_or(tag);
        let short_name = name.strip_prefix("minecraft:").unwrap_or(name);
        [tag, short_tag].iter().any(|t| {
            items
                .block_tags
                .get(*t)
                .is_some_and(|set| set.contains(name) || set.contains(short_name))
        })
    }

    /// Keeps what a guest's turn needs from the hub ([`HubView`]): read when
    /// the hub is in place (never during a guest's turn), with the in-frame
    /// player's own open container and trade.
    fn ca_read_hub(&self) {
        if self.lan_acting.is_some() {
            return;
        }
        let mut view = HubView::default();
        if let Some(host) = self.lan.as_ref() {
            view.spawn = SpawnRules {
                radius: host.hub.spawn_protection,
                ops: host.hub.ops.keys().copied().collect(),
                known: true,
            };
            for g in &host.hub.guests {
                view.permissions.insert(g.uuid, g.permission);
                let level = Dim::from_id(&g.dimension).id();
                if let Some(menu) = &g.menu {
                    let halves = match menu.slots {
                        Slots::Block(k) => vec![k],
                        Slots::Double(a, b) => vec![a, b],
                        Slots::Ender => Vec::new(),
                        Slots::Native => Vec::new(),
                    };
                    for k in menu.pos.into_iter().chain(halves) {
                        view.open.insert((level, k));
                    }
                }
                let trading = host
                    .play
                    .get(&g.conn)
                    .and_then(|p| p.merchant.as_ref())
                    .and_then(|m| m.merchant.mob);
                if let Some(mob) = trading {
                    view.trading.insert((level, mob));
                }
            }
        }
        if self.player.is_some() {
            let level = self.dim.id();
            for (k, _) in self.container_parts() {
                view.open.insert((level, k));
            }
            if let Some(k) = self.container {
                view.open.insert((level, k));
            }
            let trading = self
                .station
                .as_ref()
                .and_then(|s| s.merchant.as_ref())
                .and_then(|m| m.mob);
            if let Some(mob) = trading {
                view.trading.insert((level, mob));
            }
        }
        HUB.with(|h| *h.borrow_mut() = view);
    }
    /// `Player.canUseGameMasterBlocks` for the player acting: creative with
    /// permission level 2. A guest's level is the hub's: the game's own is
    /// the host's during a guest's turn.
    fn ca_game_master(&self) -> bool {
        let permission = match self.lan_acting_uuid {
            Some(uuid) => HUB.with(|h| h.borrow().permissions.get(&uuid).copied().unwrap_or(0)),
            None => self.ui.registries.permission,
        };
        self.creative && permission >= 2
    }
    /// [`ca::under_spawn_protection`] for `who` (a guest's UUID, None for
    /// single player's own player) at `pos` in this level: Horizon's
    /// `under_spawn_protection` (game/lan_actions.rs), the world spawn's
    /// level being the Overworld.
    fn ca_spawn_protected(&self, who: Option<u128>, pos: [i32; 3]) -> bool {
        let spawn = if self.dim == Dim::Overworld {
            self.level.world_spawn
        } else {
            None
        };
        let dedicated = self.options.dedicated.is_some();
        HUB.with(|h| ca::under_spawn_protection(&h.borrow().spawn, dedicated, who, spawn, pos))
    }
    /// What the game checks before `who`, the player acting, changes the
    /// block at `pos`: in the level, within reach of their eyes
    /// (`isWithinBlockInteractionRange(pos, 1.0)`, as `may_reach_block`),
    /// spawn protection, the world border (as `lan_guest_packet` checks it).
    fn ca_spot(&self, who: Option<u128>, pos: [i32; 3]) -> Spot {
        Spot {
            in_world: self.ca_in_world(pos),
            reachable: self
                .eye_position()
                .is_some_and(|eye| ca::within_block_range(eye, self.reach() + 1.0, pos)),
            spawn_protected: self.ca_spawn_protected(who, pos),
            inside_border: self.border.contains(f64::from(pos[0]), f64::from(pos[2])),
        }
    }
    /// Says why not; under spawn protection with the game's own red line
    /// (`ServerPlayer.sendSpawnProtectionMessage`).
    fn ca_refuse(&mut self, why: Refusal, at: [i32; 3]) {
        if why == Refusal::SpawnProtection {
            let place = Text::literal(format!("{}, {}, {}", at[0], at[1], at[2]));
            let line = Text::translate("build.spawn_protection", &[place])
                .with_color(horizon_client::text::RED);
            self.ca_say(line);
        } else {
            self.ca_tell(why.message());
        }
    }

    /// The block entity at `key` as the game saves it (with its custom
    /// name), or the saved compound of one Horizon keeps as loaded.
    fn ca_block_entity_tag(&self, key: (i32, i32, i32), block: &str) -> Option<Tag> {
        if let Some(entity) = self.world.block_entities.get(&key) {
            let mut tag = horizon_client::save::encode_block_entity(key, entity, block);
            if let (Some(name), Tag::Compound(map)) = (self.world.block_names.get(&key), &mut tag) {
                map.insert("CustomName".into(), name.clone());
            }
            return Some(horizon_client::block_entity_nbt::canonical(&tag).unwrap_or(tag));
        }
        super::spill::saved_entity(&self.world, [key.0, key.1, key.2])
    }
    /// Nothing of the block entity that was at `key` stays: not the live
    /// one, its name, a saved compound, the compound it was loaded from
    /// (whose unmodelled keys a later block entity there would inherit) or
    /// a loot roll still waiting there.
    fn ca_forget_block_entity(&mut self, key: (i32, i32, i32)) {
        self.world.block_entities.remove(&key);
        self.world.block_names.remove(&key);
        let at = |t: &Tag| {
            t.get("x") == Some(&Tag::Int(key.0))
                && t.get("y") == Some(&Tag::Int(key.1))
                && t.get("z") == Some(&Tag::Int(key.2))
        };
        let chunk_key = (key.0 >> 4, key.2 >> 4);
        let kept = self
            .world
            .chunks
            .get(&chunk_key)
            .and_then(|c| c.saved.as_ref())
            .is_some_and(|s| {
                s.block_entities.iter().any(at)
                    || s.block_entity_sources.iter().any(|(p, _)| *p == key)
                    || s.rolled_loot.iter().any(|r| r.pos == key)
            });
        if !kept {
            return;
        }
        if let Some(chunk) = self.world.chunks.get_mut(&chunk_key) {
            let chunk = Arc::make_mut(chunk);
            if let Some(saved) = chunk.saved.as_mut() {
                let saved = Arc::make_mut(saved);
                saved.block_entities.retain(|t| !at(t));
                saved.block_entity_sources.retain(|(p, _)| *p != key);
                saved.rolled_loot.retain(|r| r.pos != key);
            }
        }
        self.world.modified.insert(chunk_key);
    }
    /// Gives the block at `pos` the carried block entity `data`; without
    /// data, the fresh one placing the block gives it.
    fn ca_put_block_entity(&mut self, pos: [i32; 3], data: Option<&Tag>) {
        let key = (pos[0], pos[1], pos[2]);
        let Some(data) = data else {
            let name = self
                .res
                .facts()
                .get(self.world.block(pos[0], pos[1], pos[2]))
                .name
                .clone();
            if let Some(entity) = BlockEntity::for_block(&name) {
                self.world.block_entities.insert(key, entity);
                self.mark_block_entity_updated(key);
            }
            return;
        };
        let mut tag = data.clone();
        if let Tag::Compound(c) = &mut tag {
            for (k, v) in [("x", pos[0]), ("y", pos[1]), ("z", pos[2])] {
                c.insert(k.into(), Tag::Int(v));
            }
        }
        match horizon_client::save::decode_block_entity(&tag) {
            Some((_, entity)) => {
                self.world.block_entities.insert(key, entity);
                match tag.get("CustomName").and_then(horizon_client::block_entity_nbt::component) {
                    Some(name) => {
                        self.world.block_names.insert(key, name);
                    }
                    None => {
                        self.world.block_names.remove(&key);
                    }
                }
            }
            None => super::spill::put_saved_entity(&mut self.world, pos, tag),
        }
        self.mark_block_entity_updated(key);
    }
    /// A carried mob back in the level in place, at `at` facing `yaw`.
    fn ca_spawn_mob(&mut self, entity: &Tag, at: [f64; 3], yaw: f32) -> Option<u32> {
        let mut tag = entity.clone();
        if let Tag::Compound(c) = &mut tag {
            c.insert("Pos".into(), doubles(&at));
            c.insert("Motion".into(), doubles(&[0.0; 3]));
            c.insert(
                "Rotation".into(),
                Tag::List(vec![Tag::Float(yaw.to_bits()), Tag::Float(0f32.to_bits())]),
            );
            for key in ["fall_distance", "FallDistance"] {
                if let Some(Tag::Double(_)) = c.get(&JavaString::from(key)) {
                    c.insert(key.into(), Tag::Double(0f64.to_bits()));
                }
            }
        }
        self.load_mob(tag, None)
    }
    /// The slowness carrying gave goes with what was carried.
    fn ca_clear_slowness(&mut self) {
        let list = &mut self.potions.effects.list;
        let before = list.len();
        list.retain(|e| {
            !(e.id == "slowness"
                && ca::is_carry_slowness(e.duration, e.ambient, e.visible, e.hidden.is_some()))
        });
        if list.len() != before {
            self.apply_effect_status();
        }
    }

    /// `Game::use_item`'s HEAD: Some when the click was Carry Block's (the
    /// hand that acted, None for a click swallowed while carrying), None to
    /// let the game use it as usual. Runs for the player acting: a LAN
    /// guest in their turn, or in-frame single player's own player.
    pub(super) fn carryblock_use(&mut self) -> Option<Option<Hand>> {
        if self.remote.is_some() || self.player.is_none() || self.spectator() || !on(self) {
            return None;
        }
        // A server's own (console) use is nobody's.
        if self.lan_acting.is_none() && !self.native_window() {
            return None;
        }
        // The in-frame player's own click, the hub in place: read it now (a
        // guest's turn uses what the last tick read).
        if self.lan_acting.is_none() && self.lan.is_some() {
            self.ca_read_hub();
        }
        let carried = carried_in(self.level.player_extra.as_ref()).cloned();
        let who = self.acting_player_uuid();
        let now = self.ticks;
        let fresh = CLICKS.with(|c| c.borrow_mut().fresh(who, now));
        let guest = self.guest_use;
        if let Some(carried) = carried {
            // The hands are full: every right click either puts down or
            // does nothing (no item is used, nothing is opened).
            let on_block = match guest {
                Some(GuestUse::OnBlock(Hand::Main)) => true,
                Some(_) => false,
                None => !self.entity_targeted(),
            };
            if !fresh || !on_block {
                return Some(None);
            }
            let Some(hit) = self.pick() else {
                return Some(None);
            };
            let put = self.with_player_actor(|game| game.ca_put_down(&carried, &hit));
            return Some(put.then_some(Hand::Main));
        }
        let config = config(self);
        let hand_empty = self
            .inventory
            .get(self.selected)
            .is_none_or(Option::is_none);
        let pick = ca::pick_up(&config, self.ca_mode(), self.ca_sneaking(), hand_empty);
        if !fresh || pick == PickUp::No {
            return None;
        }
        let mob = match guest {
            Some(GuestUse::Entity(Hand::Main, _)) => true,
            Some(GuestUse::OnBlock(Hand::Main)) => false,
            Some(_) => return None,
            None => self.entity_targeted(),
        };
        let picked = self.with_player_actor(|game| {
            if mob {
                game.ca_pick_mob(&config, pick)
            } else {
                game.ca_pick_block(&config, pick)
            }
        });
        picked.then_some(Some(Hand::Main))
    }

    /// Picks up the block aimed at, with its block entity: true when it
    /// did. The block goes without dropping anything (its block entity is
    /// carried, not spilled); a waterlogged one leaves its water. A player
    /// who may not build (`pick`) only hears so.
    fn ca_pick_block(&mut self, config: &Config, pick: PickUp) -> bool {
        let Some(hit) = self.pick() else {
            return false;
        };
        let pos = hit.pos;
        if !self.ca_in_world(pos) {
            return false;
        }
        let key = (pos[0], pos[1], pos[2]);
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let facts = self.res.facts().get(state).clone();
        let name = facts.name.to_string();
        let data = self.ca_block_entity_tag(key, &name);
        let in_use = HUB.with(|h| h.borrow().open.contains(&(self.dim.id(), key)));
        let info = BlockInfo {
            name: name.clone(),
            properties: self
                .pack
                .assets
                .registry
                .state(state)
                .map(|s| s.properties.clone().into_iter().collect())
                .unwrap_or_default(),
            block_entity: facts.block_entity || data.is_some(),
            hardness: facts.hardness,
            item: (&*facts.item != "minecraft:air").then(|| facts.item.to_string()),
            empty: self.world.is_air(state)
                || matches!(
                    &*facts.name,
                    "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
                ),
            locked: data.as_ref().is_some_and(is_locked),
            game_master: ca::is_game_master_block(&name, &|class| facts.has_class(class)),
            in_use,
        };
        let mode = self.ca_mode();
        let game_master = self.ca_game_master();
        let verdict = ca::block_verdict(config, mode, game_master, &info, &|tag| {
            self.ca_block_in_tag(tag, &name)
        });
        match ca::verdict_for(pick, verdict) {
            Verdict::Allow => {}
            Verdict::Pass => return false,
            Verdict::Refuse(why) => {
                self.ca_tell(why.message());
                return false;
            }
        }
        // Picking up is breaking it: who may break this block, here.
        let who = self.lan_acting_uuid;
        if let Err(why) = ca::may_break(mode, self.ca_spot(who, pos)) {
            self.ca_refuse(why, pos);
            return false;
        }
        let state_text = self.state_string(state);
        let after = if facts.waterlogged {
            self.pack
                .assets
                .registry
                .default_state("minecraft:water")
                .unwrap_or(self.world.air)
        } else {
            self.world.air
        };
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        // The game's own removal, without the block entity's side effects:
        // nothing spills, nothing pops; neighbours still hear of it.
        let removed = self.with_level(|b, level, ticks| {
            b.set_block(
                level,
                ticks,
                at,
                after,
                flags::UPDATE_ALL | flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS,
            )
        });
        if !removed {
            let _ = self.process_level_events();
            self.ca_tell("That can't be moved right now");
            return false;
        }
        self.ca_forget_block_entity(key);
        let _ = self.process_level_events();
        let stacks = data.as_ref().and_then(|d| match d.get("Items") {
            Some(Tag::List(items)) => Some(items.len()),
            _ => None,
        });
        let summary = ca::summary(&ca::display_name(&name), stacks);
        let mut entries = vec![
            (ca::KEY_FORMAT, Tag::Int(ca::FORMAT)),
            (ca::KEY_KIND, Tag::String(ca::KIND_BLOCK.into())),
            (ca::KEY_STATE, Tag::String(state_text.as_str().into())),
            (ca::KEY_NAME, Tag::String(summary.as_str().into())),
        ];
        if let Some(data) = data {
            entries.push((ca::KEY_BLOCK_ENTITY, data));
        }
        put_carried(&mut self.level.player_extra, Some(Tag::compound(entries)));
        self.swing_arm();
        true
    }

    /// Picks up the mob aimed at, with all its data: true when it did. A
    /// player who may not build (`pick`) only hears so.
    fn ca_pick_mob(&mut self, config: &Config, pick: PickUp) -> bool {
        let Some((index, distance, _)) = self.mob_hit() else {
            return false;
        };
        // In-frame, a block in front of the mob takes the click.
        if self.guest_use.is_none()
            && let (Some(hit), Some(eye)) = (self.pick(), self.eye_position())
        {
            let d = (0..3)
                .map(|k| (hit.point[k] - eye[k]).powi(2))
                .sum::<f64>()
                .sqrt();
            if d < distance {
                return false;
            }
        }
        let me = self.acting_player_uuid();
        let mode = self.ca_mode();
        let level = self.dim.id();
        let m = &self.mobs[index];
        let at = [
            m.position.x.floor() as i32,
            m.position.y.floor() as i32,
            m.position.z.floor() as i32,
        ];
        let info = MobInfo {
            id: m.kind.id.clone(),
            width: m.width(),
            height: m.height(),
            hostile: m.hostile(),
            alive: m.alive(),
            removed: m.removed,
            riding: m.extra.mount.is_some() || self.riding == Some(m.id),
            ridden: m.extra.rider.is_some() || m.rider.is_some() || m.extra.jockey,
            leashed: m.extra.leash.is_some() || m.extra.leash_pending.is_some(),
            inventory: m.extra.chested
                || m.extra.saddled
                || m.extra.saddle_stack.is_some()
                || !m.extra.inventory.is_empty(),
            owned_by_other: m.extra.owner.is_some()
                && !horizon_client::mobs::tame::owned_by(m.extra.owner, me),
            invulnerable: m.invulnerable_flag,
            leading: self.mobs.iter().any(|n| {
                !n.removed && matches!(n.extra.leash, Some(Holder::Mob(holder)) if holder == m.id)
            }),
            trading: HUB.with(|h| h.borrow().trading.contains(&(level, m.id))),
            sleeping: horizon_client::mobs::villager::sleeping(m),
        };
        let name = match &m.custom_name {
            Some(custom) => format!("{custom} ({})", ca::display_name(&m.kind.id)),
            None => ca::display_name(&m.kind.id),
        };
        match ca::verdict_for(pick, ca::mob_verdict(config, mode, &info)) {
            Verdict::Allow => {}
            Verdict::Pass => return false,
            Verdict::Refuse(why) => {
                self.ca_tell(why.message());
                return false;
            }
        }
        // `handleInteract`'s world border, and spawn protection, where it is.
        let who = self.lan_acting_uuid;
        if let Err(why) = ca::may_take_mob(self.ca_spot(who, at)) {
            self.ca_refuse(why, at);
            return false;
        }
        let entity = self.mob_tree_nbt(index);
        // `remove`: gone without dying (no drops, no experience).
        self.mobs[index].removed = true;
        let carried = Tag::compound([
            (ca::KEY_FORMAT, Tag::Int(ca::FORMAT)),
            (ca::KEY_KIND, Tag::String(ca::KIND_MOB.into())),
            (ca::KEY_NAME, Tag::String(name.as_str().into())),
            (ca::KEY_ENTITY, entity),
        ]);
        put_carried(&mut self.level.player_extra, Some(carried));
        self.swing_arm();
        true
    }

    /// Puts what the acting player carries down against the block face
    /// `hit`: true when it is in the world again (and no longer carried).
    fn ca_put_down(&mut self, carried: &Tag, hit: &Hit) -> bool {
        match text_of(carried, ca::KEY_KIND).as_deref() {
            Some(ca::KIND_BLOCK) => self.ca_put_block(carried, hit),
            Some(ca::KIND_MOB) => self.ca_put_mob(carried, hit),
            _ => {
                self.ca_tell("Carry Block doesn't know what you carry");
                false
            }
        }
    }

    /// A carried block placed as its item would be (vanilla's placement
    /// rules: room, survival, what is in the way, which way it faces), with
    /// its own properties and block entity.
    fn ca_put_block(&mut self, carried: &Tag, hit: &Hit) -> bool {
        // Putting down is placing: using the clicked block, then the block
        // it goes into.
        let mode = self.ca_mode();
        let who = self.lan_acting_uuid;
        let clicked = self.ca_spot(who, hit.pos);
        if let Err(why) = ca::may_use_on(mode, clicked) {
            self.ca_refuse(why, hit.pos);
            return false;
        }
        let registry = self.pack.assets.registry;
        let Some(text) = text_of(carried, ca::KEY_STATE) else {
            self.ca_tell("Carry Block doesn't know what you carry");
            return false;
        };
        let (Some(carried_state), Ok(carried_id)) = (
            State::parse(&text),
            super::render_control::parse_state(registry, &text),
        ) else {
            self.ca_tell("This game doesn't know the block you carry");
            return false;
        };
        let item = self.res.facts().get(carried_id).item.to_string();
        if item == "minecraft:air" {
            self.ca_tell("This game doesn't know the block you carry");
            return false;
        }
        let ctx = PlaceContext {
            clicked_pos: BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]),
            face: direction(hit.face),
            hit: Vec3::new(hit.point[0], hit.point[1], hit.point[2]),
            inside: false,
            yaw: self.yaw,
            pitch: self.pitch,
            sneaking: self.ca_sneaking(),
            item,
            can_use_game_master_blocks: self.ca_game_master(),
        };
        let Some((at, placed)) =
            self.with_level(|b, level, ticks| b.placement_state(level, ticks, &ctx))
        else {
            self.ca_tell("There's no room to put it down there");
            return false;
        };
        let target = [at.x, at.y, at.z];
        if let Err(why) = ca::may_place(mode, clicked, self.ca_spot(who, target)) {
            self.ca_refuse(why, target);
            return false;
        }
        let placed_text = self.state_string(placed);
        let state: BlockStateId = State::parse(&placed_text)
            .map(|p| ca::placed_state(&carried_state, &p).text())
            .and_then(|t| super::render_control::parse_state(registry, &t).ok())
            .unwrap_or(placed);
        let wrote = self.with_level(|b, level, ticks| {
            b.set_block(level, ticks, at, state, flags::UPDATE_ALL_IMMEDIATE)
        });
        if !wrote {
            let _ = self.process_level_events();
            self.ca_tell("There's no room to put it down there");
            return false;
        }
        // The block is in the world: from here on it is not carried any
        // more, whatever happens to it next.
        put_carried(&mut self.level.player_extra, None);
        let me = self.acting_player_uuid();
        forget_hud(me);
        let now = self.world.block(target[0], target[1], target[2]);
        if self.res.facts().get(now).block_entity {
            self.ca_put_block_entity(target, carried.get(ca::KEY_BLOCK_ENTITY));
        }
        let sounds = self.res.facts().get(state).sounds.clone();
        self.level_events.push(LevelEvent::Sound {
            pos: at,
            sound: Arc::clone(&sounds.place),
            volume: (sounds.volume + 1.0) / 2.0,
            pitch: sounds.pitch * 0.8,
        });
        let _ = self.process_level_events();
        self.ca_clear_slowness();
        self.swing_arm();
        true
    }

    /// A carried mob let go on the block face `hit`, facing the player.
    fn ca_put_mob(&mut self, carried: &Tag, hit: &Hit) -> bool {
        let Some(entity) = carried.get(ca::KEY_ENTITY) else {
            self.ca_tell("Carry Block doesn't know what you carry");
            return false;
        };
        let p = [
            hit.pos[0] + hit.face[0],
            hit.pos[1] + hit.face[1],
            hit.pos[2] + hit.face[2],
        ];
        // As a spawn egg used on the clicked block, the mob into `p`.
        let mode = self.ca_mode();
        let who = self.lan_acting_uuid;
        let clicked = self.ca_spot(who, hit.pos);
        if let Err(why) = ca::may_place(mode, clicked, self.ca_spot(who, p)) {
            let at = if clicked.spawn_protected { hit.pos } else { p };
            self.ca_refuse(why, at);
            return false;
        }
        if self
            .res
            .facts()
            .get(self.world.block(p[0], p[1], p[2]))
            .full_collision
        {
            self.ca_tell("There's no room to put it down there");
            return false;
        }
        let at = [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]),
            f64::from(p[2]) + 0.5,
        ];
        let yaw = self.yaw + 180.0;
        if self.ca_spawn_mob(entity, at, yaw).is_none() {
            self.ca_tell("It couldn't be put down");
            return false;
        }
        put_carried(&mut self.level.player_extra, None);
        let me = self.acting_player_uuid();
        forget_hud(me);
        self.ca_clear_slowness();
        self.swing_arm();
        true
    }

    /// What a dead carrier carried, back in the level in place near their
    /// feet: a block on the nearest free spot it can stand on, else dropped
    /// as its item holding all its data; a mob at their feet. False when it
    /// could not be done (it stays carried and is tried again).
    /// `who`: the dead carrier, a guest's UUID (None for single player's own
    /// player), whose spawn protection a block is not left under.
    fn ca_release(&mut self, carried: &Tag, feet: [f64; 3], who: Option<u128>) -> bool {
        match text_of(carried, ca::KEY_KIND).as_deref() {
            Some(ca::KIND_MOB) => match carried.get(ca::KEY_ENTITY) {
                Some(entity) => self.ca_spawn_mob(entity, feet, 0.0).is_some(),
                None => false,
            },
            Some(ca::KIND_BLOCK) => self.ca_release_block(carried, feet, who),
            _ => false,
        }
    }
    fn ca_release_block(&mut self, carried: &Tag, feet: [f64; 3], who: Option<u128>) -> bool {
        let registry = self.pack.assets.registry;
        let Some(text) = text_of(carried, ca::KEY_STATE) else {
            return false;
        };
        let (Some(carried_state), Ok(carried_id)) = (
            State::parse(&text),
            super::render_control::parse_state(registry, &text),
        ) else {
            return false;
        };
        let data = carried.get(ca::KEY_BLOCK_ENTITY);
        let base = feet.map(|v| v.floor() as i32);
        for offset in ca::release_spots(ca::RELEASE_RADIUS) {
            let pos = [
                base[0] + offset[0],
                base[1] + offset[1],
                base[2] + offset[2],
            ];
            // Never where the carrier could not have placed it: under spawn
            // protection or outside the border, it is dropped instead.
            if !ca::may_release_at(self.ca_spot(who, pos)) {
                continue;
            }
            let here = self
                .res
                .facts()
                .get(self.world.block(pos[0], pos[1], pos[2]));
            if !here.replaceable || here.block_entity {
                continue;
            }
            let water = here.fluid == Fluid::Water && !here.fluid_falling && here.fluid_level == 8;
            let state = super::render_control::parse_state(
                registry,
                &ca::released_state(&carried_state, water).text(),
            )
            .unwrap_or(carried_id);
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            if !self.with_level(|b, level, ticks| b.can_survive(level, ticks, at, state)) {
                continue;
            }
            let wrote = self.with_level(|b, level, ticks| {
                b.set_block(level, ticks, at, state, flags::UPDATE_ALL)
            });
            let _ = self.process_level_events();
            if !wrote {
                continue;
            }
            let now = self.world.block(pos[0], pos[1], pos[2]);
            if self.res.facts().get(now).block_entity {
                self.ca_put_block_entity(pos, data);
            }
            return true;
        }
        let item = self.res.facts().get(carried_id).item.clone();
        if &*item == "minecraft:air" {
            return false;
        }
        let stack = item_with_data(&item, data);
        self.spawn_item(
            stack,
            [feet[0], feet[1] + 0.5, feet[2]],
            [0.0, 0.2, 0.0],
            40,
        );
        true
    }

    /// `native_tick` (a hook point): carriers get their slowness, the HUD
    /// and action bar hear what they carry, and what a dead carrier carried
    /// is left where they died. On the window, it notes Options > Mods'
    /// on/off for single player's server thread.
    pub(super) fn carryblock_tick(&mut self) {
        if !on(self) || self.remote.is_some() {
            return;
        }
        self.ca_read_hub();
        let config = config(self);
        let now = self.ticks;
        if self.player.is_some() && self.lan_acting.is_none() && self.native_window() {
            self.ca_local_tick(&config, now);
        }
        self.ca_guests_tick(&config, now);
    }
    /// In-frame single player's own player.
    fn ca_local_tick(&mut self, config: &Config, now: u64) {
        let Some(carried) = carried_in(self.level.player_extra.as_ref()).cloned() else {
            return;
        };
        let Some((feet, dead)) = self.player.as_ref().map(|p| {
            (
                [p.position.x, p.position.y, p.position.z],
                p.health.dead_or_dying(),
            )
        }) else {
            return;
        };
        if dead {
            forget_hud(self.level.ensure_player_uuid());
            if self.ca_release(&carried, feet, None) {
                put_carried(&mut self.level.player_extra, None);
            }
            return;
        }
        if let Some(amplifier) = ca::slowness_amplifier(config)
            && give_slowness(&mut self.potions.effects, amplifier)
        {
            self.apply_effect_status();
        }
        let summary = carried_summary(&carried);
        let me = self.level.ensure_player_uuid();
        note_hud(me, &summary);
        if config.hud == HudCorner::Off
            && config.action_bar
            && now.is_multiple_of(ca::ACTION_BAR_EVERY)
        {
            let line = Text::literal(ca::action_bar_text(&summary));
            self.titles.set_overlay(line, false);
        }
    }
    /// A server's players (single player's own window is one of its server
    /// thread's, the hub's owner).
    fn ca_guests_tick(&mut self, config: &Config, now: u64) {
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let owner = host.owner.as_ref().map(|o| o.uuid);
        let amplifier = ca::slowness_amplifier(config);
        let mut dead = Vec::new();
        let mut bars = Vec::new();
        for g in host.hub.guests.iter_mut() {
            if !g.placed {
                continue;
            }
            let Some(carried) = carried_in(g.saved.as_ref()) else {
                continue;
            };
            if g.dead {
                forget_hud(horizon_client::lan::playerdata::uuid_ints(g.uuid));
                dead.push((
                    g.conn,
                    g.uuid,
                    g.dimension.clone(),
                    g.position,
                    carried.clone(),
                ));
                continue;
            }
            let summary = carried_summary(carried);
            if let Some(amplifier) = amplifier {
                give_slowness(&mut g.effects, amplifier);
            }
            note_hud(horizon_client::lan::playerdata::uuid_ints(g.uuid), &summary);
            let local = Some(g.uuid) == owner;
            if config.action_bar
                && (!local || config.hud == HudCorner::Off)
                && now.is_multiple_of(ca::ACTION_BAR_EVERY)
            {
                bars.push((g.conn, summary));
            }
        }
        for (conn, summary) in bars {
            let line = Text::literal(ca::action_bar_text(&summary));
            host.hub.send_to(
                conn,
                &C::SystemChat(play::SystemChat {
                    content: horizon_client::lan::data::component_nbt(&line.component()),
                    overlay: true,
                }),
            );
        }
        for (conn, uuid, dimension, feet, carried) in dead {
            self.ca_release_guest(conn, uuid, &dimension, feet, &carried);
        }
    }
    /// Releases a dead guest's load in their level (put in place for it, as
    /// `execute in` does), then takes it from them.
    fn ca_release_guest(
        &mut self,
        conn: u32,
        uuid: u128,
        dimension: &str,
        feet: [f64; 3],
        carried: &Tag,
    ) {
        let target = Dim::from_id(dimension);
        let back = if target == self.dim {
            None
        } else {
            self.enter_command_level(target)
        };
        if self.dim == target
            && self.ca_release(carried, feet, Some(uuid))
            && let Some(g) = self.lan.as_mut().and_then(|h| h.hub.guest_mut(conn))
        {
            put_carried(&mut g.saved, None);
        }
        if let Some(back) = back {
            self.enter_command_level(back);
        }
    }

    /// `native_hud` (a hook point): the window's box saying what its player
    /// carries, as the game that simulates the world last said.
    pub(super) fn carryblock_hud(&mut self, batch: &mut HudBatch, corners: &mut Corners) {
        if !on(self) {
            return;
        }
        let hud = config(self).hud;
        let corner = match hud {
            HudCorner::Off => return,
            HudCorner::TopLeft => Corner::TopLeft,
            HudCorner::TopRight => Corner::TopRight,
            HudCorner::BottomLeft => Corner::BottomLeft,
            HudCorner::BottomRight => Corner::BottomRight,
        };
        if !self.native_corner_shown(corner) {
            return;
        }
        let me = match &self.remote {
            Some(remote) => horizon_client::lan::playerdata::uuid_ints(remote.profile()),
            None => match self.level.player_uuid {
                Some(uuid) => uuid,
                None => return,
            },
        };
        // The death screen: what was carried is being left where they died.
        let dead = self.screen == Screen::Dead
            || self
                .player
                .as_ref()
                .is_some_and(|p| p.health.dead_or_dying());
        let Some(summary) = shared(|s| s.hud.shown(me, Instant::now(), dead).map(str::to_owned))
        else {
            return;
        };
        let [first, second] = ca::hud_lines(&summary);
        let width = self
            .native_text_width(&first)
            .max(self.native_text_width(&second))
            + 6.0;
        let height = 22.0;
        let (x, y) = if ca::hud_crosses_hotbar(hud, batch.width, width) {
            // A bottom box that would cover the hotbar, the bars over it or
            // the action bar goes up over them. How far up its corner is
            // taken already (the minimap, boxes placed before it) is
            // `Corners`' own: an empty box finds out, then the room left
            // under the box makes up the rest.
            let (_, probe) = corners.place(corner, width, 0.0);
            let room = ca::hud_room_under(batch.height - probe);
            corners.place(corner, width, room + height)
        } else {
            corners.place(corner, width, height)
        };
        self.native_fill(batch, [x, y, width, height], 0x7800_0000);
        self.native_text(batch, &first, [x + 3.0, y + 3.0], 0xFF_FFFF, true);
        self.native_text(batch, &second, [x + 3.0, y + 12.0], 0xAA_AAAA, true);
    }

    /// `native_command` (a hook point): `/carryblock [status|reload]`;
    /// true when it was Carry Block's. The game that simulates the world
    /// runs it, as the player who typed it.
    pub(super) fn carryblock_command(&mut self, command: &str) -> bool {
        if self.remote.is_some() || !on(self) {
            return false;
        }
        let Some(parsed) = ca::parse_command(command) else {
            return false;
        };
        match parsed {
            Err(usage) => self.command_failure(Text::literal(usage)),
            Ok(Command::Status) => {
                if self.player.is_none() {
                    self.command_failure(Text::literal("Only players carry things"));
                    return true;
                }
                let summary = carried_in(self.level.player_extra.as_ref()).map(carried_summary);
                self.system_message(Text::literal(ca::status_text(summary.as_deref())));
            }
            Ok(Command::Reload) => {
                if self.ui.registries.permission < 2 {
                    self.command_failure(Text::literal(
                        "Carry Block reload requires permission level 2",
                    ));
                    return true;
                }
                match read_config(self) {
                    Ok(config) => {
                        apply(Ok(config));
                        self.system_message(Text::literal("Carry Block configuration reloaded"));
                    }
                    Err(error) => self.command_failure(Text::literal(error)),
                }
            }
        }
        true
    }

    /// `native_settings` (a hook point): the settings page's buttons.
    pub(super) fn carryblock_settings(&self) -> Vec<String> {
        ca::labels(&loaded(self))
    }

    /// `native_settings_click` (a hook point): change the setting, write
    /// `config.json` and put it in effect at once (or read the file again).
    pub(super) fn carryblock_settings_click(&mut self, button: usize) {
        match ca::click(&loaded(self), button) {
            Some(Effect::Save(config)) => {
                self.native_set_setting(ca::ID, ca::CONFIG_FILE, &config.to_json());
                apply(Ok(config));
            }
            Some(Effect::Reload) => apply(read_config(self)),
            None => {}
        }
    }
}
