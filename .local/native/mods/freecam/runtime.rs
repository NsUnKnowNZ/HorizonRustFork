// Horizon Freecam: control your camera separately from your player, ported
// from the Freecam mod (https://github.com/MinecraftFreecam/Freecam, MIT).
//
// The toggle key detaches the camera at the player's eye; the movement keys
// then fly it through blocks (no-clip, the original's default), and disabling
// returns the view to the player. F4 plus a hotbar digit opens one of nine
// tripod cameras whose positions are kept until reset. The state machine,
// movement, settings and messages live here; the helper files bridge this
// module to the game (keys, the camera view, the HUD). This file is both the
// mod's Cargo library (its tests) and the module the bundle adds to the game.
//
// Settings are kept as the original's config was (`horizon-mods/
// .horizon-mod-data/freecam/<key>`), with the original's defaults:
// static-velocity flight at one block a tick, no-clip, the inside perspective,
// the player shown, exiting on damage, no interaction, notifications on.
//
// The original's collision modes past `ignoreAll` (transparent, openable,
// custom lists), the hand, submersion fog, the outline, "control player",
// full brightness and server restrictions are not ported yet; see README.md.
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The mod's id: its settings folder and its keys' lines.
pub const ID: &str = "freecam";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str =
    "Control your camera separately from your player. Ported from the Freecam mod.";
/// The mod's keys. The reset key is unbound by default, as the original's
/// tripod reset is (`key.keyboard.unknown` is no key's name).
pub const TOGGLE: &str = "toggle";
pub const TRIPOD_RESET: &str = "tripod_reset";
pub const DEFAULT_TOGGLE_KEY: &str = "key.keyboard.f4";
pub const DEFAULT_RESET_KEY: &str = "key.keyboard.unknown";

/// The vanilla key names the camera flies with. The movement keys never
/// reach the player while the freecam is active; the shift stays the
/// player's, as the original's dummy input kept it (a sneaking player
/// stands while the camera sinks with it).
pub const KEY_FORWARD: &str = "key.keyboard.w";
pub const KEY_BACK: &str = "key.keyboard.s";
pub const KEY_LEFT: &str = "key.keyboard.a";
pub const KEY_RIGHT: &str = "key.keyboard.d";
pub const KEY_UP: &str = "key.keyboard.space";
pub const KEY_DOWN: &str = "key.keyboard.left.shift";
pub const KEY_SPRINT: &str = "key.keyboard.left.control";
/// The hotbar digits, which the held toggle and reset keys turn into tripod
/// slots (F4 + 1..9 opens camera n; the reset key held, 1..9 resets it).
pub const DIGITS: [&str; 9] = [
    "key.keyboard.1",
    "key.keyboard.2",
    "key.keyboard.3",
    "key.keyboard.4",
    "key.keyboard.5",
    "key.keyboard.6",
    "key.keyboard.7",
    "key.keyboard.8",
    "key.keyboard.9",
];

/// The original's messages.
pub const MSG_ENABLED: &str = "Freecam has been enabled.";
pub const MSG_DISABLED: &str = "Freecam has been disabled.";
/// How long a toast stays on screen.
pub const TOAST_TIME: Duration = Duration::from_secs(3);

/// The horizontal spread of a diagonal move, as the original's
/// `Motion.DIAGONAL_MULTIPLIER` (`sin(45 degrees)`).
pub const DIAGONAL_MULTIPLIER: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// The sprint multiplier, as the original's `Motion.doMotion`.
pub const SPRINT_MULTIPLIER: f64 = 1.5;
/// Ticks per second: speeds are blocks per tick, frames are seconds.
pub const TICKS_PER_SECOND: f64 = 20.0;
/// The creative flight's ease, per tick, toward its target velocity; and how
/// far it targets past the set speeds. This port's approximation of vanilla's
/// creative flight, which the original reuses for its Creative mode.
pub const CREATIVE_EASE: f64 = 0.5;
pub const CREATIVE_HORIZONTAL_FACTOR: f64 = 2.0;
pub const CREATIVE_VERTICAL_FACTOR: f64 = 1.5;

/// How the camera flies. `Default` is the original's static velocity: exactly
/// the set speed a tick, no inertia. `Creative` eases toward its target with
/// momentum, as vanilla's creative flight feels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightMode {
    Default,
    Creative,
}
impl FlightMode {
    const NAMES: [&'static str; 2] = ["Default", "Creative"];
    fn from_stored(value: &str) -> Self {
        if value.trim() == "creative" {
            FlightMode::Creative
        } else {
            FlightMode::Default
        }
    }
    fn stored(self) -> &'static str {
        match self {
            FlightMode::Default => "default",
            FlightMode::Creative => "creative",
        }
    }
    fn label(self) -> &'static str {
        Self::NAMES[self as usize]
    }
    fn next(self) -> Self {
        match self {
            FlightMode::Default => FlightMode::Creative,
            FlightMode::Creative => FlightMode::Default,
        }
    }
}

/// Where the camera starts, as the original's `Perspective` (offsets along
/// the view, without collision checks: the original skips them unless the
/// collision config asks, and this port always flies no-clip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perspective {
    Inside,
    FirstPerson,
    ThirdPerson,
    ThirdPersonMirror,
}
impl Perspective {
    const NAMES: [&'static str; 4] = ["Inside", "First person", "Third person", "Mirror"];
    fn from_stored(value: &str) -> Self {
        match value.trim() {
            "first_person" => Perspective::FirstPerson,
            "third_person" => Perspective::ThirdPerson,
            "third_person_mirror" => Perspective::ThirdPersonMirror,
            _ => Perspective::Inside,
        }
    }
    fn stored(self) -> &'static str {
        match self {
            Perspective::Inside => "inside",
            Perspective::FirstPerson => "first_person",
            Perspective::ThirdPerson => "third_person",
            Perspective::ThirdPersonMirror => "third_person_mirror",
        }
    }
    fn label(self) -> &'static str {
        Self::NAMES[self as usize]
    }
    fn next(self) -> Self {
        match self {
            Perspective::Inside => Perspective::FirstPerson,
            Perspective::FirstPerson => Perspective::ThirdPerson,
            Perspective::ThirdPerson => Perspective::ThirdPersonMirror,
            Perspective::ThirdPersonMirror => Perspective::Inside,
        }
    }
}

/// Where clicks go while the freecam is active. `Off` (the original's
/// default) stops attacks and uses; `Camera` aims them along the camera's
/// view; `Player` leaves the game's own targeting from the player's eyes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Off,
    Camera,
    Player,
}
impl Interaction {
    const NAMES: [&'static str; 3] = ["Off", "From camera", "From player"];
    fn from_stored(value: &str) -> Self {
        match value.trim() {
            "camera" => Interaction::Camera,
            "player" => Interaction::Player,
            _ => Interaction::Off,
        }
    }
    fn stored(self) -> &'static str {
        match self {
            Interaction::Off => "off",
            Interaction::Camera => "camera",
            Interaction::Player => "player",
        }
    }
    fn label(self) -> &'static str {
        Self::NAMES[self as usize]
    }
    fn next(self) -> Self {
        match self {
            Interaction::Off => Interaction::Camera,
            Interaction::Camera => Interaction::Player,
            Interaction::Player => Interaction::Off,
        }
    }
}

/// The speeds the settings page cycles through (blocks per tick; the
/// original's config GUI takes any number, the page takes these steps).
pub const SPEEDS: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];

/// The mod's settings, with the original's defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub flight_mode: FlightMode,
    pub horizontal_speed: f64,
    pub vertical_speed: f64,
    pub perspective: Perspective,
    pub show_player: bool,
    pub disable_on_damage: bool,
    pub freeze_player: bool,
    pub interaction: Interaction,
    pub notify: bool,
    pub notify_tripod: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            flight_mode: FlightMode::Default,
            horizontal_speed: 1.0,
            vertical_speed: 1.0,
            perspective: Perspective::Inside,
            show_player: true,
            disable_on_damage: true,
            freeze_player: false,
            interaction: Interaction::Off,
            notify: true,
            notify_tripod: true,
        }
    }
}
impl Settings {
    /// From the stored values; the original's defaults for the ones missing.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut settings = Self::default();
        if let Some(v) = get("flight_mode") {
            settings.flight_mode = FlightMode::from_stored(&v);
        }
        if let Some(v) = get("horizontal_speed").and_then(|v| v.trim().parse::<f64>().ok()) {
            settings.horizontal_speed = positive(v).unwrap_or(settings.horizontal_speed);
        }
        if let Some(v) = get("vertical_speed").and_then(|v| v.trim().parse::<f64>().ok()) {
            settings.vertical_speed = positive(v).unwrap_or(settings.vertical_speed);
        }
        if let Some(v) = get("perspective") {
            settings.perspective = Perspective::from_stored(&v);
        }
        if let Some(v) = get("show_player") {
            settings.show_player = v.trim() != "false";
        }
        if let Some(v) = get("disable_on_damage") {
            settings.disable_on_damage = v.trim() != "false";
        }
        if let Some(v) = get("freeze_player") {
            settings.freeze_player = v.trim() == "true";
        }
        if let Some(v) = get("interaction") {
            settings.interaction = Interaction::from_stored(&v);
        }
        if let Some(v) = get("notify") {
            settings.notify = v.trim() != "false";
        }
        if let Some(v) = get("notify_tripod") {
            settings.notify_tripod = v.trim() != "false";
        }
        settings
    }
    /// The settings page's buttons, in order.
    pub fn labels(&self) -> Vec<String> {
        let on = |b: bool| if b { "ON" } else { "OFF" };
        vec![
            format!("Flight mode: {}", self.flight_mode.label()),
            format!("Horizontal speed: {}", self.horizontal_speed),
            format!("Vertical speed: {}", self.vertical_speed),
            format!("Initial perspective: {}", self.perspective.label()),
            format!("Show player: {}", on(self.show_player)),
            format!("Disable on damage: {}", on(self.disable_on_damage)),
            format!("Freeze player: {}", on(self.freeze_player)),
            format!("Interaction: {}", self.interaction.label()),
            format!("Notifications: {}", on(self.notify)),
            format!("Tripod notifications: {}", on(self.notify_tripod)),
        ]
    }
    /// A click on button `i` of `labels`: cycles the setting.
    pub fn click(&mut self, i: usize) {
        let speed = |speed: f64| {
            let at = SPEEDS
                .iter()
                .position(|&s| (s - speed).abs() < 1e-9)
                .map_or(0, |at| (at + 1) % SPEEDS.len());
            SPEEDS[at]
        };
        match i {
            0 => self.flight_mode = self.flight_mode.next(),
            1 => self.horizontal_speed = speed(self.horizontal_speed),
            2 => self.vertical_speed = speed(self.vertical_speed),
            3 => self.perspective = self.perspective.next(),
            4 => self.show_player = !self.show_player,
            5 => self.disable_on_damage = !self.disable_on_damage,
            6 => self.freeze_player = !self.freeze_player,
            7 => self.interaction = self.interaction.next(),
            8 => self.notify = !self.notify,
            9 => self.notify_tripod = !self.notify_tripod,
            _ => {}
        }
    }
    /// What to store, key by key.
    pub fn stored(&self) -> [(&'static str, String); 10] {
        let flag = |b: bool| if b { "true".into() } else { "false".into() };
        [
            ("flight_mode", self.flight_mode.stored().into()),
            ("horizontal_speed", number(self.horizontal_speed)),
            ("vertical_speed", number(self.vertical_speed)),
            ("perspective", self.perspective.stored().into()),
            ("show_player", flag(self.show_player)),
            ("disable_on_damage", flag(self.disable_on_damage)),
            ("freeze_player", flag(self.freeze_player)),
            ("interaction", self.interaction.stored().into()),
            ("notify", flag(self.notify)),
            ("notify_tripod", flag(self.notify_tripod)),
        ]
    }
}

/// The player's snapshot the key helper passes in (the game's own player).
#[derive(Clone, Copy, Debug)]
pub struct Player {
    /// The feet position.
    pub pos: [f64; 3],
    pub eye_height: f32,
    pub yaw: f32,
    pub pitch: f32,
}
impl Player {
    /// The eye the camera starts at.
    fn eye(&self) -> [f64; 3] {
        [
            self.pos[0],
            self.pos[1] + f64::from(self.eye_height),
            self.pos[2],
        ]
    }
    /// The camera as the player looks, moved per the initial perspective.
    fn camera(&self, settings: &Settings) -> Cam {
        let (mut yaw, mut pitch) = (self.yaw, self.pitch);
        let view = direction(yaw, pitch);
        let pos = match settings.perspective {
            Perspective::Inside => self.eye(),
            // Just in front of the player's eyes, as the original's 0.4.
            Perspective::FirstPerson => add(self.eye(), view, 0.4),
            // Back as per F5, as the original's -4.0.
            Perspective::ThirdPerson => add(self.eye(), view, -4.0),
            // The rotation inverted (Camera.update's mirror), which moves the
            // other way along the same line.
            Perspective::ThirdPersonMirror => {
                yaw = wrap(yaw + 180.0);
                pitch = -pitch;
                add(self.eye(), view, 4.0)
            }
        };
        Cam { pos, yaw, pitch }
    }
}

/// What a hook asks the helper to do with the game's camera type: `Enter`
/// detaches (the helper remembers the type and forces third person, which
/// shows the player's body and hides the hand), `Exit` restores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Enter,
    Exit,
}

/// The flying keys, as `native_key` events keep them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

/// A camera: where it is and how it looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// Which camera is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Active {
    None,
    Freecam,
    Tripod(u8),
}

struct State {
    settings: Option<Settings>,
    active: Active,
    /// The tripod slots' saved cameras, 1..=9.
    saved: HashMap<u8, Cam>,
    cam: Cam,
    held: Held,
    /// The toggle key held: digits become tripod slots while it is. Kept
    /// apart from `held` so the exits' key clearings never lose a combo
    /// whose key is still physically down.
    combo_toggle: bool,
    /// The tripod reset key held: digits reset slots while it is.
    combo_reset: bool,
    /// The creative flight's momentum, in blocks per tick.
    velocity: [f64; 3],
    /// The player's camera type when the freecam began (0 first person,
    /// 1 third person behind, 2 third person front), restored on exit.
    camera_kind: u8,
    toast: Option<(String, Instant)>,
    /// The player's health last seen, for the damage exit.
    health: Option<f32>,
    /// The user's mouse buttons down, counted from the presses and releases
    /// the mouse helper sees. A bot's forced clicks never pass through
    /// `play_mouse`, so a button down says the player's own hand is there:
    /// the camera's aim follows only those, and bots keep their targeting.
    clicks: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            settings: None,
            active: Active::None,
            saved: HashMap::new(),
            cam: Cam {
                pos: [0.0; 3],
                yaw: 0.0,
                pitch: 0.0,
            },
            held: Held::default(),
            combo_toggle: false,
            combo_reset: false,
            velocity: [0.0; 3],
            camera_kind: 0,
            toast: None,
            health: None,
            clicks: 0,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The settings, read once through `get`.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    STATE.with_borrow_mut(|s| *s.settings.get_or_insert_with(|| Settings::read(get)))
}

/// Replaces the settings (the settings page changed them).
pub fn set_settings(settings: Settings) {
    STATE.with_borrow_mut(|s| s.settings = Some(settings));
}

/// The camera type the player had when the freecam began (for the helper).
pub fn camera_kind() -> u8 {
    STATE.with_borrow(|s| s.camera_kind)
}

/// Remembers the player's camera type (the helper, as the freecam begins).
pub fn remember_camera(kind: u8) {
    STATE.with_borrow_mut(|s| s.camera_kind = kind);
}

/// Whether the freecam holds the camera.
pub fn is_active() -> bool {
    STATE.with_borrow(|s| s.active != Active::None)
}

/// The mod was turned off in Options > Mods: the camera returns and the
/// tripods forget, silently.
pub fn disable() -> Option<Command> {
    STATE.with_borrow_mut(|s| {
        if s.active == Active::None {
            return None;
        }
        s.active = Active::None;
        stop(s);
        s.saved.clear();
        s.toast = None;
        Some(Command::Exit)
    })
}

/// A key went down or up. `toggle_key` and `reset_key` are the player's
/// bound keys for the mod's two (the helper reads them from the game).
/// Returns the command for the helper: the toggle key enters and exits,
/// the toggle key held plus a digit opens a tripod camera, the reset key
/// held plus a digit resets one.
pub fn key(
    playing: bool,
    name: &str,
    pressed: bool,
    toggle_key: &str,
    reset_key: &str,
    player: Option<Player>,
    get: impl Fn(&str) -> Option<String>,
) -> Option<Command> {
    let digit = DIGITS.iter().position(|k| *k == name).map(|i| i as u8 + 1);
    let (command, toast) = STATE.with_borrow_mut(|s| {
        if name == toggle_key {
            s.combo_toggle = pressed;
        } else if name == reset_key {
            s.combo_reset = pressed;
        }
        match name {
            KEY_FORWARD => s.held.forward = pressed,
            KEY_BACK => s.held.back = pressed,
            KEY_LEFT => s.held.left = pressed,
            KEY_RIGHT => s.held.right = pressed,
            KEY_UP => s.held.up = pressed,
            KEY_DOWN => s.held.down = pressed,
            KEY_SPRINT => s.held.sprint = pressed,
            _ => {}
        }
        if !playing {
            return (None, None);
        }
        let settings = *s.settings.get_or_insert_with(|| Settings::read(get));
        if name == toggle_key && pressed {
            return toggle(s, &settings, player);
        }
        let Some(slot) = digit else {
            return (None, None);
        };
        // A digit release is nothing; only its press opens or resets.
        if !pressed {
            return (None, None);
        }
        if s.combo_toggle {
            return tripod(s, &settings, slot, player);
        }
        if s.combo_reset {
            return reset_tripod(s, &settings, slot, player);
        }
        (None, None)
    });
    toast_now(toast);
    command
}

/// The toggle key: off becomes freecam, freecam and tripods become off.
fn toggle(
    s: &mut State,
    settings: &Settings,
    player: Option<Player>,
) -> (Option<Command>, Option<String>) {
    match s.active {
        Active::None => {
            let Some(player) = player else {
                return (None, None);
            };
            s.cam = player.camera(settings);
            s.active = Active::Freecam;
            stop(s);
            (
                Some(Command::Enter),
                settings.notify.then(|| MSG_ENABLED.into()),
            )
        }
        Active::Freecam => {
            s.active = Active::None;
            stop(s);
            (
                Some(Command::Exit),
                settings.notify.then(|| MSG_DISABLED.into()),
            )
        }
        Active::Tripod(slot) => {
            s.saved.insert(slot, s.cam);
            s.active = Active::None;
            stop(s);
            (
                Some(Command::Exit),
                settings
                    .notify_tripod
                    .then(|| format!("Closing camera #{slot}")),
            )
        }
    }
}

/// The toggle key held and digit `slot` pressed: open that tripod camera,
/// keeping whatever was active first (a tripod's own spot is saved; the
/// plain freecam's flown spot is not, as the original's toggle loses it).
fn tripod(
    s: &mut State,
    settings: &Settings,
    slot: u8,
    player: Option<Player>,
) -> (Option<Command>, Option<String>) {
    if slot == 0 {
        return (None, None);
    }
    let Some(player) = player else {
        return (None, None);
    };
    let mut command = None;
    match s.active {
        Active::Tripod(open) if open == slot => {
            s.saved.insert(slot, s.cam);
            s.active = Active::None;
            stop(s);
            let toast = settings
                .notify_tripod
                .then(|| format!("Closing camera #{slot}"));
            return (Some(Command::Exit), toast);
        }
        Active::Tripod(open) => {
            s.saved.insert(open, s.cam);
        }
        Active::Freecam => {
            // The plain freecam closes first, its flown spot not kept, as
            // the original's toggle() does (whose toast the opening
            // immediately replaces).
        }
        Active::None => {
            command = Some(Command::Enter);
        }
    }
    s.active = Active::Tripod(slot);
    stop(s);
    s.cam = s
        .saved
        .get(&slot)
        .copied()
        .unwrap_or_else(|| player.camera(settings));
    let toast = settings
        .notify_tripod
        .then(|| format!("Opening camera #{slot}"));
    (command, toast)
}

/// The reset key held and digit `slot` pressed: the active camera `slot`
/// returns to the player, the others forget their saved spot. The original
/// says so either way (`freecam.msg.tripod.reset`).
fn reset_tripod(
    s: &mut State,
    settings: &Settings,
    slot: u8,
    player: Option<Player>,
) -> (Option<Command>, Option<String>) {
    if slot == 0 {
        return (None, None);
    }
    if s.active == Active::Tripod(slot)
        && let Some(player) = player
    {
        s.cam = player.camera(settings);
    } else {
        s.saved.remove(&slot);
    }
    let toast = settings
        .notify_tripod
        .then(|| format!("Reset camera #{slot}"));
    (None, toast)
}

/// The view the frame renders from: the freecam's, or none.
pub fn view() -> Option<([f64; 3], f32, f32)> {
    STATE.with_borrow(|s| (s.active != Active::None).then_some((s.cam.pos, s.cam.yaw, s.cam.pitch)))
}

/// The ray interactions aim along when they come from the camera: the
/// camera's, but only while a click of the player's own is down — the rest
/// of the time the game keeps its own targeting from the player's eyes,
/// which is what a bot's clicks (that never pass through the mouse helper)
/// must find there.
pub fn aim() -> Option<([f64; 3], [f64; 3])> {
    STATE.with_borrow(|s| {
        let settings = s.settings?;
        (settings.interaction == Interaction::Camera && s.active != Active::None && s.clicks > 0)
            .then(|| (s.cam.pos, direction(s.cam.yaw, s.cam.pitch)))
    })
}

/// A mouse button went down or up (the mouse helper sees the user's clicks;
/// a bot's forced ones never pass through `play_mouse`).
pub fn mouse_click(pressed: bool) {
    STATE.with_borrow_mut(|s| {
        if pressed {
            s.clicks = s.clicks.saturating_add(1);
        } else {
            s.clicks = s.clicks.saturating_sub(1);
        }
    });
}

/// The keys the mod takes from the player: the movement keys while the
/// freecam is active (presses never reach the player; the shift stays the
/// player's), and the hotbar digits while a combo key is held, tripods or
/// not (F4 + 2 opens camera #2 from a closed state too, as the original's
/// hold-action does).
pub fn swallows(name: &str) -> bool {
    if matches!(name, KEY_FORWARD | KEY_BACK | KEY_LEFT | KEY_RIGHT | KEY_UP) {
        return is_active();
    }
    if DIGITS.contains(&name) {
        return STATE.with_borrow(|s| s.combo_toggle || s.combo_reset);
    }
    false
}

/// A view turn (the mouse, arrows, a pad, an agent): the freecam's, not the
/// player's, while it is active. True when it took the turn.
pub fn turn(xo: f64, yo: f64) -> bool {
    if !is_active() {
        return false;
    }
    STATE.with_borrow_mut(|s| {
        // Entity.turn: 0.15 degrees per unit, the pitch clamped; the yaw
        // stays unwrapped, as the game's own turns.
        s.cam.yaw += (xo as f32) * 0.15;
        s.cam.pitch = ((s.cam.pitch + (yo as f32) * 0.15) % 360.0).clamp(-90.0, 90.0);
    });
    true
}

/// Once a frame: the camera flies while playing, and stops when it does not.
pub fn frame(elapsed: Duration, playing: bool) {
    STATE.with_borrow_mut(|s| {
        if s.active == Active::None {
            return;
        }
        // A screen opened: the keys it cleared would otherwise fly the
        // camera on re-focus until their next release.
        if !playing {
            s.held = Held::default();
            s.velocity = [0.0; 3];
            return;
        }
        // A paused frame (a stall, a breakpoint) moves nothing huge.
        let ticks = (elapsed.as_secs_f64() * TICKS_PER_SECOND).min(10.0);
        let settings = s.settings.unwrap_or_default();
        match settings.flight_mode {
            FlightMode::Default => {
                let v = default_velocity(s.held, &settings, s.cam.yaw);
                for (p, d) in s.cam.pos.iter_mut().zip(v) {
                    *p += d * ticks;
                }
                s.velocity = [0.0; 3];
            }
            FlightMode::Creative => {
                let target = creative_target(s.held, &settings, s.cam.yaw);
                let ease = 1.0 - CREATIVE_EASE.powf(ticks);
                for ((p, vel), t) in s.cam.pos.iter_mut().zip(&mut s.velocity).zip(target) {
                    *vel += (t - *vel) * ease;
                    *p += *vel * ticks;
                }
            }
        }
    });
}

/// The original's `Motion.doMotion`: the set speed along the view's
/// horizontal plane, half as far again while sprinting forward, the
/// diagonal spread, and the set speed straight up and down.
fn default_velocity(held: Held, settings: &Settings, yaw: f32) -> [f64; 3] {
    let forward = direction(yaw, 0.0);
    let side = direction(wrap(yaw + 90.0), 0.0);
    let h = settings.horizontal_speed
        * if held.sprint && held.forward {
            SPRINT_MULTIPLIER
        } else {
            1.0
        };
    let mut v = [0.0; 3];
    let mut straight = false;
    if held.forward {
        v[0] += forward[0] * h;
        v[2] += forward[2] * h;
        straight = true;
    }
    if held.back {
        v[0] -= forward[0] * h;
        v[2] -= forward[2] * h;
        straight = true;
    }
    let mut strafing = false;
    if held.right {
        v[0] += side[0] * h;
        v[2] += side[2] * h;
        strafing = true;
    }
    if held.left {
        v[0] -= side[0] * h;
        v[2] -= side[2] * h;
        strafing = true;
    }
    if straight && strafing {
        v[0] *= DIAGONAL_MULTIPLIER;
        v[2] *= DIAGONAL_MULTIPLIER;
    }
    if held.up {
        v[1] += settings.vertical_speed;
    }
    if held.down {
        v[1] -= settings.vertical_speed;
    }
    v
}

/// The creative flight's target velocity, in blocks per tick.
fn creative_target(held: Held, settings: &Settings, yaw: f32) -> [f64; 3] {
    let h = settings.horizontal_speed * CREATIVE_HORIZONTAL_FACTOR;
    let straight = held.forward || held.back;
    let strafing = held.left || held.right;
    let (x, z) = if straight || strafing {
        let forward = direction(yaw, 0.0);
        let side = direction(wrap(yaw + 90.0), 0.0);
        let (mut x, mut z) = (0.0, 0.0);
        if held.forward {
            x += forward[0];
            z += forward[2];
        }
        if held.back {
            x -= forward[0];
            z -= forward[2];
        }
        if held.right {
            x += side[0];
            z += side[2];
        }
        if held.left {
            x -= side[0];
            z -= side[2];
        }
        if straight && strafing {
            x *= DIAGONAL_MULTIPLIER;
            z *= DIAGONAL_MULTIPLIER;
        }
        (x * h, z * h)
    } else {
        (0.0, 0.0)
    };
    let up = (u8::from(held.up) - u8::from(held.down)) as f64;
    [
        x,
        up * settings.vertical_speed * CREATIVE_VERTICAL_FACTOR,
        z,
    ]
}

/// Once a tick, from the game's: exits on damage, and on the world's close.
/// `health` is the player's now, `world_open` whether one is.
pub fn tick(world_open: bool, health: Option<f32>) -> Option<Command> {
    let (command, toast) = STATE.with_borrow_mut(|s| {
        let was = s.health;
        s.health = health;
        if s.active == Active::None {
            return (None, None);
        }
        // The world closed under the camera: reset silently.
        if !world_open {
            s.active = Active::None;
            stop(s);
            s.saved.clear();
            return (Some(Command::Exit), None);
        }
        let settings = s.settings.unwrap_or_default();
        if settings.disable_on_damage
            && let (Some(was), Some(now)) = (was, health)
            && now < was
        {
            s.active = Active::None;
            stop(s);
            return (
                Some(Command::Exit),
                settings.notify.then(|| MSG_DISABLED.into()),
            );
        }
        (None, None)
    });
    toast_now(toast);
    command
}

/// The player stands while the freecam is active and the setting asks.
pub fn freezes_player() -> bool {
    STATE.with_borrow(|s| s.active != Active::None && s.settings.is_some_and(|st| st.freeze_player))
}

/// The player's body hides while the freecam is active and the setting asks.
pub fn hides_player() -> bool {
    STATE.with_borrow(|s| s.active != Active::None && s.settings.is_some_and(|st| !st.show_player))
}

/// The player's clicks do nothing while the freecam is active and the
/// interaction is Off (the default): the helper eats them.
pub fn blocks_interaction() -> bool {
    STATE.with_borrow(|s| {
        s.active != Active::None
            && s.settings
                .is_none_or(|st| st.interaction == Interaction::Off)
    })
}

/// The toast on screen now, if it is fresh.
pub fn toast() -> Option<String> {
    STATE.with_borrow(|s| {
        let (text, at) = s.toast.as_ref()?;
        (at.elapsed() < TOAST_TIME).then(|| text.clone())
    })
}

/// Keeps a toast on screen; the producers gate theirs by the notification
/// settings before handing one here.
fn toast_now(text: Option<String>) {
    let Some(text) = text else {
        return;
    };
    STATE.with_borrow_mut(|s| s.toast = Some((text, Instant::now())));
}

/// The settings page's click: changes a setting and gives the settings back
/// to store.
pub fn click(button: usize, get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut settings = settings(get);
    settings.click(button);
    set_settings(settings);
    settings
}

/// The direction a view looks, as the game's `view_vector` (yaw 0 faces
/// +Z, pitch down is positive).
pub fn direction(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

/// `pos` moved `distance` along `view`.
fn add(pos: [f64; 3], view: [f64; 3], distance: f64) -> [f64; 3] {
    [
        pos[0] + view[0] * distance,
        pos[1] + view[1] * distance,
        pos[2] + view[2] * distance,
    ]
}

/// The degrees in -180..180, as the game's `wrap`.
fn wrap(degrees: f32) -> f32 {
    let degrees = degrees % 360.0;
    if degrees >= 180.0 {
        degrees - 360.0
    } else if degrees < -180.0 {
        degrees + 360.0
    } else {
        degrees
    }
}

/// Positive, or none for zero and less (and not a number).
fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

/// A speed as the config kept it (`1.0`, `0.25`).
fn number(value: f64) -> String {
    format!("{value}")
}

/// The flying keys stop, the momentum stops (the camera keeps its spot).
fn stop(s: &mut State) {
    s.held = Held::default();
    s.velocity = [0.0; 3];
    s.clicks = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A player at the origin's feet, standing, facing yaw 0 (+Z).
    fn player(yaw: f32, pitch: f32) -> Player {
        Player {
            pos: [10.0, 64.0, 20.0],
            eye_height: 1.62,
            yaw,
            pitch,
        }
    }

    /// The eye height is f32 in the game and f64 once added to the feet.
    fn eye_y(feet: f64) -> f64 {
        feet + f64::from(1.62_f32)
    }

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + Copy + 'a {
        move |key| pairs.iter().find(|p| p.0 == key).map(|p| p.1.to_owned())
    }

    fn reset() {
        STATE.with_borrow_mut(|s| *s = State::default());
    }

    #[test]
    fn reads_the_originals_defaults_and_stored_values() {
        assert_eq!(
            Settings::read(stored(&[])),
            Settings {
                flight_mode: FlightMode::Default,
                horizontal_speed: 1.0,
                vertical_speed: 1.0,
                perspective: Perspective::Inside,
                show_player: true,
                disable_on_damage: true,
                freeze_player: false,
                interaction: Interaction::Off,
                notify: true,
                notify_tripod: true,
            }
        );
        let s = Settings::read(stored(&[
            ("flight_mode", "creative"),
            ("horizontal_speed", "2.5\n"),
            ("vertical_speed", "0.5"),
            ("perspective", "third_person_mirror"),
            ("show_player", "false"),
            ("disable_on_damage", "false"),
            ("freeze_player", "true"),
            ("interaction", "camera"),
            ("notify", "false"),
            ("notify_tripod", "false"),
        ]));
        assert_eq!(
            s,
            Settings {
                flight_mode: FlightMode::Creative,
                horizontal_speed: 2.5,
                vertical_speed: 0.5,
                perspective: Perspective::ThirdPersonMirror,
                show_player: false,
                disable_on_damage: false,
                freeze_player: true,
                interaction: Interaction::Camera,
                notify: false,
                notify_tripod: false,
            }
        );
        // Zero, negative and nonsense speeds keep the default.
        let bad = Settings::read(stored(&[
            ("horizontal_speed", "0"),
            ("vertical_speed", "-1"),
            ("perspective", "sideways"),
            ("interaction", "sideways"),
            ("flight_mode", "sideways"),
        ]));
        assert_eq!(bad.horizontal_speed, 1.0);
        assert_eq!(bad.vertical_speed, 1.0);
        assert_eq!(bad.perspective, Perspective::Inside);
        assert_eq!(bad.interaction, Interaction::Off);
        assert_eq!(bad.flight_mode, FlightMode::Default);
        // The round trip keeps every setting.
        let round: Vec<(&str, String)> = s.stored().into();
        let back: Vec<(&str, &str)> = round.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(Settings::read(stored(&back)), s);
    }

    #[test]
    fn the_settings_page_cycles_every_setting() {
        let mut s = Settings::default();
        s.click(0);
        assert_eq!(s.flight_mode, FlightMode::Creative);
        s.click(1);
        assert_eq!(s.horizontal_speed, 2.0);
        // A value off the steps comes back to the first.
        s.horizontal_speed = 99.0;
        s.click(1);
        assert_eq!(s.horizontal_speed, 0.25);
        s.horizontal_speed = 2.0;
        s.click(1);
        assert_eq!(s.horizontal_speed, 4.0);
        s.click(2);
        assert_eq!(s.vertical_speed, 2.0);
        s.click(3);
        assert_eq!(s.perspective, Perspective::FirstPerson);
        s.click(4);
        assert!(!s.show_player);
        s.click(5);
        assert!(!s.disable_on_damage);
        s.click(6);
        assert!(s.freeze_player);
        s.click(7);
        assert_eq!(s.interaction, Interaction::Camera);
        s.click(8);
        assert!(!s.notify);
        s.click(9);
        assert!(!s.notify_tripod);
        s.click(10);
        assert_eq!(
            s,
            Settings {
                flight_mode: FlightMode::Creative,
                horizontal_speed: 4.0,
                vertical_speed: 2.0,
                perspective: Perspective::FirstPerson,
                show_player: false,
                disable_on_damage: false,
                freeze_player: true,
                interaction: Interaction::Camera,
                notify: false,
                notify_tripod: false,
            }
        );
        // The labels name all ten.
        assert_eq!(s.labels().len(), 10);
    }

    #[test]
    fn toggling_detaches_at_the_eye_and_returns() {
        reset();
        let get = stored(&[]);
        // Only while playing.
        assert_eq!(
            key(
                false,
                DEFAULT_TOGGLE_KEY,
                true,
                DEFAULT_TOGGLE_KEY,
                DEFAULT_RESET_KEY,
                Some(player(0.0, 0.0)),
                get
            ),
            None
        );
        assert!(!is_active());
        assert_eq!(
            key(
                true,
                DEFAULT_TOGGLE_KEY,
                true,
                DEFAULT_TOGGLE_KEY,
                DEFAULT_RESET_KEY,
                Some(player(0.0, 10.0)),
                get
            ),
            Some(Command::Enter)
        );
        assert_eq!(camera_kind(), 0, "the player was in first person");
        // The camera sits at the player's eye, looking as they look.
        assert_eq!(view(), Some(([10.0, eye_y(64.0), 20.0], 0.0, 10.0)));
        assert_eq!(toast(), Some(MSG_ENABLED.into()));
        // The movement keys fly the camera; the player's keys never move it.
        assert!(swallows(KEY_FORWARD));
        assert!(!swallows(KEY_DOWN));
        assert!(turn(100.0, -200.0));
        let (pos, yaw, pitch) = view().unwrap();
        // The game's f32 turns: 100 * 0.15 from the player's 0, and the
        // player's pitch 10 minus 200 * 0.15.
        assert!((yaw - 15.0).abs() < 1e-4);
        assert!((pitch - (10.0 - 30.0)).abs() < 1e-4);
        let _ = pos;
        // The mouse's turn never reaches the player: the view comes back
        // to the player's own yaw on exit.
        assert_eq!(
            key(
                true,
                DEFAULT_TOGGLE_KEY,
                true,
                DEFAULT_TOGGLE_KEY,
                DEFAULT_RESET_KEY,
                Some(player(0.0, 10.0)),
                get
            ),
            Some(Command::Exit)
        );
        assert_eq!(view(), None);
        assert_eq!(toast(), Some(MSG_DISABLED.into()));
        assert!(!swallows(KEY_FORWARD));
        assert!(!turn(10.0, 10.0));
    }

    #[test]
    fn the_initial_perspective_moves_the_camera_along_the_view() {
        reset();
        let get = stored(&[]);
        // First person: 0.4 along the view.
        set_settings(Settings {
            perspective: Perspective::FirstPerson,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        assert_eq!(view(), Some(([10.0, eye_y(64.0), 20.4], 0.0, 0.0)));
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        // Third person: 4.0 back along the view.
        set_settings(Settings {
            perspective: Perspective::ThirdPerson,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        assert_eq!(view(), Some(([10.0, eye_y(64.0), 16.0], 0.0, 0.0)));
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        // Mirror: the rotation inverted, 4.0 along the original view.
        set_settings(Settings {
            perspective: Perspective::ThirdPersonMirror,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        // The mirror yaw is the game's wrapDegrees(180): -180.
        assert_eq!(view(), Some(([10.0, eye_y(64.0), 24.0], -180.0, 0.0)));
    }

    #[test]
    fn tripods_keep_and_reset_their_spots() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        // F4 + 2 opens camera #2; the plain freecam's flown spot is not kept
        // (the original's toggle() closes it first).
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert!(turn(900.0, 0.0), "the freecam's yaw is now 135");
        key(
            true,
            DIGITS[1],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Opening camera #2".into()));
        let (_, yaw, _) = view().unwrap();
        assert_eq!(yaw, 0.0, "a fresh camera starts at the player");
        // The held toggle takes the digits from the hotbar; a digit release
        // does nothing.
        STATE.with_borrow_mut(|s| s.combo_toggle = true);
        assert!(swallows(DIGITS[1]));
        STATE.with_borrow_mut(|s| s.combo_toggle = false);
        key(
            true,
            DIGITS[1],
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(
            toast(),
            Some("Opening camera #2".into()),
            "the release kept the camera"
        );
        // Camera #2 looks elsewhere; F4 + 3 opens #3 and keeps #2's spot.
        assert!(turn(900.0, 0.0), "camera #2's yaw is now 135");
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[2],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Opening camera #3".into()));
        let (pos, yaw, _) = view().unwrap();
        assert_eq!((pos, yaw), (p.eye(), 0.0));
        // F4 + 2 while #2 is closed and reopened: it comes back at the spot
        // it was kept with (the toggle press closes it first, as the
        // original's key does).
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[1],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        let (_, yaw, _) = view().unwrap();
        assert_eq!(yaw, 135.0);
        // The reset key + 2 puts the active camera #2 back at the player.
        key(
            true,
            DEFAULT_RESET_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[1],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_RESET_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Reset camera #2".into()));
        let (pos, yaw, _) = view().unwrap();
        assert_eq!((pos, yaw), (p.eye(), 0.0));
        // The reset key + 3 forgets the idle camera #3's spot.
        key(
            true,
            DEFAULT_RESET_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[2],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_RESET_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Reset camera #3".into()));
        // F4 (the plain toggle) closes the tripod and keeps its spot.
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            false,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Closing camera #2".into()));
        assert_eq!(view(), None);
        // Opening #2 once more restores the spot it closed with (the reset
        // had moved it to the player).
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[1],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        let (pos, _, _) = view().unwrap();
        assert_eq!(pos, p.eye());
    }

    #[test]
    fn the_freecam_becomes_a_tripod_and_back() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[4],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(toast(), Some("Opening camera #5".into()));
        assert_eq!(camera_kind(), 0, "still detached, once");
        // The plain toggle closes the tripod.
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(view(), None);
        // A closed tripod's spot reopens as the freecam's start is its own.
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        let (pos, _, _) = view().unwrap();
        assert_eq!(pos, p.eye());
    }

    #[test]
    fn static_velocity_flies_as_the_originals_motion() {
        reset();
        let get = stored(&[]);
        set_settings(Settings {
            horizontal_speed: 1.0,
            vertical_speed: 1.0,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        // Forward a tick's worth: one block along +Z.
        STATE.with_borrow_mut(|s| s.held.forward = true);
        frame(Duration::from_millis(50), true);
        assert_eq!(view().unwrap().0, [10.0, eye_y(64.0), 21.0]);
        // Back and strafe right at once: the diagonal spread.
        STATE.with_borrow_mut(|s| {
            s.held.forward = false;
            s.held.back = true;
            s.held.right = true;
        });
        frame(Duration::from_millis(50), true);
        let (pos, _, _) = view().unwrap();
        assert!((pos[0] - (10.0 - DIAGONAL_MULTIPLIER)).abs() < 1e-9);
        assert!((pos[2] - (21.0 - DIAGONAL_MULTIPLIER)).abs() < 1e-9);
        // Straight up and down.
        STATE.with_borrow_mut(|s| {
            s.held.back = false;
            s.held.right = false;
            s.held.up = true;
        });
        frame(Duration::from_millis(50), true);
        assert_eq!(view().unwrap().0[1], eye_y(65.0));
        // A screen opens: the keys it cleared stop flying the camera.
        frame(Duration::from_millis(50), false);
        STATE.with_borrow_mut(|s| s.held.forward = true);
        frame(Duration::from_millis(50), false);
        assert_eq!(view().unwrap().0[1], eye_y(65.0));
        // Sprinting forward goes half as far again.
        set_settings(Settings {
            horizontal_speed: 1.0,
            ..Settings::default()
        });
        STATE.with_borrow_mut(|s| {
            s.held = Held::default();
            s.held.forward = true;
            s.held.sprint = true;
        });
        frame(Duration::from_millis(50), true);
        // From where the diagonal left it: half as far again forward.
        assert!(
            (view().unwrap().0[2] - (21.0 - DIAGONAL_MULTIPLIER + SPRINT_MULTIPLIER)).abs() < 1e-9
        );
    }

    #[test]
    fn creative_flight_eases_toward_its_target() {
        reset();
        let get = stored(&[]);
        set_settings(Settings {
            flight_mode: FlightMode::Creative,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        STATE.with_borrow_mut(|s| s.held.forward = true);
        // The first half-tick moves less than the steady speed; it converges
        // on the target and keeps going.
        let mut last = view().unwrap().0[2];
        for _ in 0..40 {
            frame(Duration::from_millis(50), true);
            let z = view().unwrap().0[2];
            assert!(z > last, "the momentum carries forward");
            last = z;
        }
        let per_tick = {
            frame(Duration::from_millis(50), true);
            let a = view().unwrap().0[2];
            frame(Duration::from_millis(50), true);
            view().unwrap().0[2] - a
        };
        assert!(
            (per_tick - CREATIVE_HORIZONTAL_FACTOR).abs() < 0.1,
            "settled at the target: {per_tick}"
        );
    }

    #[test]
    fn damage_exits_and_the_worlds_close_resets() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(tick(true, Some(20.0)), None);
        assert!(is_active());
        // Damage drops the health: the freecam exits.
        assert_eq!(tick(true, Some(17.0)), Some(Command::Exit));
        assert!(!is_active());
        assert_eq!(toast(), Some(MSG_DISABLED.into()));
        // A fall heals back up, then re-entering tracks again.
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert_eq!(tick(true, Some(19.0)), None, "healing is not damage");
        assert!(is_active());
        // The world closes: reset, silently, and the tripods forget.
        key(
            true,
            DIGITS[0],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert!(is_active());
        assert_eq!(tick(false, None), Some(Command::Exit));
        assert!(!is_active());
        assert_ne!(toast(), Some(MSG_DISABLED.into()), "silent");
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        key(
            true,
            DIGITS[0],
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        let (pos, _, _) = view().unwrap();
        assert_eq!(pos, p.eye(), "the slot forgot its spot with the world");
    }

    #[test]
    fn turning_the_mod_off_restores_silently() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        assert!(is_active());
        // Disabled in Options > Mods: the camera returns, no toast, the
        // tripods forget.
        assert_eq!(disable(), Some(Command::Exit));
        assert!(!is_active());
        assert_eq!(view(), None);
        assert_eq!(toast(), None);
        // Off once, off twice: nothing.
        assert_eq!(disable(), None);
    }

    #[test]
    fn directions_follow_the_views_convention() {
        // Yaw 0 faces +Z, 90 faces -X; pitch down is positive.
        assert_eq!(direction(0.0, 0.0), [0.0, 0.0, 1.0]);
        assert!((direction(90.0, 0.0)[0] + 1.0).abs() < 1e-9);
        assert!((direction(0.0, 90.0)[1] + 1.0).abs() < 1e-9);
        assert!((direction(180.0, 0.0)[2] + 1.0).abs() < 1e-9);
        // The wrap keeps the yaw in -180..180.
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
    }

    #[test]
    fn interactions_follow_the_setting() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        // Off (the default): no aim of the mod's own.
        assert_eq!(aim(), None);
        // From camera: the ray starts at the camera along its view — but
        // only while a click of the player's own is down; without one the
        // game keeps its own targeting (a bot's clicks find it there).
        turn(600.0, 0.0);
        set_settings(Settings {
            interaction: Interaction::Camera,
            ..Settings::default()
        });
        assert_eq!(aim(), None, "no button held: the game's own targeting");
        mouse_click(true);
        let (from, along) = aim().unwrap();
        assert_eq!(from, view().unwrap().0);
        assert!((along[0] + 1.0).abs() < 1e-9, "yaw 90 looks along -X");
        mouse_click(false);
        assert_eq!(aim(), None, "the button went up again");
        // From player: the game's own.
        set_settings(Settings {
            interaction: Interaction::Player,
            ..Settings::default()
        });
        assert_eq!(aim(), None);
    }

    #[test]
    fn clicks_count_and_the_exit_lets_go() {
        reset();
        let get = stored(&[]);
        let p = player(0.0, 0.0);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(p),
            get,
        );
        set_settings(Settings {
            interaction: Interaction::Camera,
            ..Settings::default()
        });
        // Two buttons down, one up: the aim still follows the camera.
        mouse_click(true);
        mouse_click(true);
        mouse_click(false);
        assert!(aim().is_some(), "one button still down");
        // More releases than presses stay at none.
        mouse_click(false);
        assert_eq!(aim(), None);
        // A press lost to a state reset (the mod went off mid-hold) can
        // never leave a phantom button down.
        mouse_click(true);
        assert_eq!(disable(), Some(Command::Exit));
        assert_eq!(aim(), None, "off: nothing held");
        mouse_click(false);
        assert_eq!(aim(), None);
    }

    #[test]
    fn the_player_freezes_only_while_asked_and_active() {
        reset();
        let get = stored(&[]);
        assert!(!freezes_player());
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        assert!(!freezes_player(), "off by default");
        set_settings(Settings {
            freeze_player: true,
            ..Settings::default()
        });
        assert!(freezes_player());
        set_settings(Settings {
            freeze_player: true,
            ..Settings::default()
        });
        STATE.with_borrow_mut(|s| s.active = Active::None);
        assert!(!freezes_player());
    }

    #[test]
    fn hides_the_body_only_while_asked_and_active() {
        reset();
        let get = stored(&[]);
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        assert!(!hides_player(), "the body shows by default");
        set_settings(Settings {
            show_player: false,
            ..Settings::default()
        });
        assert!(hides_player());
    }

    #[test]
    fn notifications_silence_their_toasts() {
        reset();
        let get = stored(&[]);
        set_settings(Settings {
            notify: false,
            ..Settings::default()
        });
        key(
            true,
            DEFAULT_TOGGLE_KEY,
            true,
            DEFAULT_TOGGLE_KEY,
            DEFAULT_RESET_KEY,
            Some(player(0.0, 0.0)),
            get,
        );
        assert_eq!(toast(), None);
        assert_eq!(
            key(
                true,
                DEFAULT_TOGGLE_KEY,
                true,
                DEFAULT_TOGGLE_KEY,
                DEFAULT_RESET_KEY,
                Some(player(0.0, 0.0)),
                get
            ),
            Some(Command::Exit)
        );
        assert_eq!(toast(), None);
    }
}
