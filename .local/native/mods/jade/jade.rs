// Jade (snownee.jade, Jade-mc26.3-Fabric-26.3.5): what the player is looking at, in a box at the top of the screen
// (the theme's frame, jade_theme.rs): the block or mob's icon and name, what Jade and its plugins say about it, and
// the mod it comes from. Its options are kept by the settings API and edited on the config screens (jade_config.rs). Plugins add lines on the client (`register_client`) and from
// the server's state (`register_server`), as Jade's IBlockComponentProvider and IServerDataProvider do; the
// server's lines reach the client over `jade:data`, asked for while the player looks.
//
// A line is a row of elements (JadeUI): text, items, the progress arrow, a bar, hearts and armour. A server line
// carries them in its text (`encode`), so a plugin's plain text lines stay plain text.
use crate::game::Game;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use std::cell::RefCell;
use std::sync::Mutex;

pub const ID: &str = "jade";
const NET_REQUEST: &str = "jade:request";
const NET_DATA: &str = "jade:data";

/// What the player looks at.
#[derive(Clone, Debug)]
pub enum Target {
    Block { pos: [i32; 3], block: String },
    Mob { index: usize },
    /// An item frame, painting, armour stand or dropped item, by its index in the game's list.
    Frame { index: usize },
    Painting { index: usize },
    Stand { index: usize },
    Item { index: usize },
    /// A falling block (FallingBlockProvider), by its index in the game's list.
    Falling { index: usize },
    /// A chest minecart or chest boat and such (ContainerEntityProvider), by its index.
    Cart { index: usize },
    Boat { index: usize },
    /// A block or item display (BlockDisplayProvider, ItemDisplayProvider), by its id in the client's list.
    Display { id: i32 },
}

/// One of a line's elements (JadeUI.text, item, smallItem, progressArrow, a progress bar, HealthElement,
/// ArmorElement, spacer).
#[derive(Clone, Debug)]
pub enum El {
    /// Text at three quarters (Mod Name: Smaller).
    Small(String),
    Text(String),
    Item { id: String, count: u32, small: bool },
    Arrow(f32),
    Bar { frac: f32, color: u32, text: String },
    Hearts { health: f32, max: f32, absorb: f32 },
    Armor(f32),
    Space(f32),
    /// Jade's energy icon (`jade:energy`).
    Energy,
    /// The tick or cross after the harvest tool's icon (JadeUI.text(...).scale(0.75F).size(0, 0).offset(-3, 3)).
    Mark(String),
    /// Not drawn: the box's icon becomes this item (IComponentProvider.getIcon, for what only the server knows).
    Icon(String),
    /// Not drawn: a honeycomb over the icon (WaxedProvider.waxedIcon).
    Badge,
    /// Not drawn: the title with `{0}` as what it is now (ITooltip.replace(CORE_OBJECT_NAME, ...)).
    Title(String),
    /// Not drawn: EntityHealthAndArmorProvider's absorption and LivingEntity.getArmorValue from the server.
    Stats { absorb: f32, armor: f32 },
}

/// One line of the box.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub els: Vec<El>,
}

impl Line {
    pub fn text(t: impl Into<String>) -> Self {
        Self { els: vec![El::Text(t.into())] }
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with(mut self, e: El) -> Self {
        self.els.push(e);
        self
    }
    pub fn item(id: &str, count: u32) -> El {
        El::Item { id: id.to_owned(), count, small: false }
    }
    pub fn small(id: &str) -> El {
        El::Item { id: id.to_owned(), count: 1, small: true }
    }
}

/// The size Jade's energy icon is drawn, and its sprite (`assets/jade/textures/gui/sprites/energy.png`), x, y and ARGB.
const ENERGY_ICON: f32 = 9.0;
const ENERGY_PIXELS: [(u8, u8, u32); 64] = [(9, 0, 0x16F5CF43), (10, 0, 0x18F5C949), (8, 1, 0x01F2EA0C), (9, 1, 0x8AF5CC44), (8, 2, 0x72F5CD43), (9, 2, 0x75F5CC44), (7, 3, 0x33F5CB45), (8, 3, 0xF6F5CC44), (9, 3, 0x1AF5CC44), (6, 4, 0x0CF5C949), (7, 4, 0xDBF5CD44), (8, 4, 0xB9F5CC44), (6, 5, 0xA2F5CC43), (7, 5, 0xFFF5CC44), (8, 5, 0x5CF5CC43), (5, 6, 0x5DF5CC45), (6, 6, 0xFFF5CC44), (7, 6, 0xFFF5CC44), (8, 6, 0xB9F5CC44), (9, 6, 0xB0F5CC44), (10, 6, 0xB0F5CC44), (11, 6, 0xB0F5CC44), (12, 6, 0x4CF5CB45), (4, 7, 0x24F5C942), (5, 7, 0xF3F5CC44), (6, 7, 0xFFF5CC44), (7, 7, 0xFFF5CC44), (8, 7, 0xFFF5CC44), (9, 7, 0xFFF5CC44), (10, 7, 0xFFF5CC44), (11, 7, 0xCCF5CC44), (12, 7, 0x06F4C735), (3, 8, 0x06F4C735), (4, 8, 0xCCF5CC44), (5, 8, 0xFFF5CC44), (6, 8, 0xFFF5CC44), (7, 8, 0xFFF5CC44), (8, 8, 0xFFF5CC44), (9, 8, 0xFFF5CC44), (10, 8, 0xF3F5CC44), (11, 8, 0x24F5C942), (3, 9, 0x4CF5CB45), (4, 9, 0xB0F5CC44), (5, 9, 0xB0F5CC44), (6, 9, 0xB0F5CC44), (7, 9, 0xB9F5CC44), (8, 9, 0xFFF5CC44), (9, 9, 0xFFF5CC44), (10, 9, 0x5DF5CC45), (7, 10, 0x5CF5CC43), (8, 10, 0xFFF5CC44), (9, 10, 0xA2F5CC43), (7, 11, 0xB9F5CC44), (8, 11, 0xDBF5CD44), (9, 11, 0x0CF5C949), (6, 12, 0x1AF5CC44), (7, 12, 0xF6F5CC44), (8, 12, 0x33F5CB45), (6, 13, 0x75F5CC44), (7, 13, 0x72F5CD43), (6, 14, 0x8AF5CC44), (7, 14, 0x01F2EA0C), (5, 15, 0x18F5C949), (6, 15, 0x16F5CF43)];

/// A line as a server line's text: elements apart by `\u{1f}`, each but text starting `\u{1}`.
pub fn encode(line: &Line) -> String {
    let parts: Vec<String> = line
        .els
        .iter()
        .map(|e| match e {
            El::Text(t) => t.replace(['\u{1}', '\u{1f}', '\n'], " "),
            El::Small(t) => format!("\u{1}m {}", t.replace(['\u{1}', '\u{1f}', '\n'], " ")),
            El::Mark(t) => format!("\u{1}k {}", t.replace(['\u{1}', '\u{1f}', '\n'], " ")),
            El::Icon(id) => format!("\u{1}I {id}"),
            El::Badge => "\u{1}W".to_owned(),
            El::Title(t) => format!("\u{1}T {}", t.replace(['\u{1}', '\u{1f}', '\n'], " ")),
            El::Stats { absorb, armor } => format!("\u{1}S {absorb} {armor}"),
            El::Item { id, count, small } => format!("\u{1}i {id} {count} {}", u8::from(*small)),
            El::Arrow(f) => format!("\u{1}a {f}"),
            El::Bar { frac, color, text } => format!("\u{1}b {frac} {color} {text}"),
            El::Hearts { health, max, absorb } => format!("\u{1}h {health} {max} {absorb}"),
            El::Armor(a) => format!("\u{1}r {a}"),
            El::Space(w) => format!("\u{1}s {w}"),
            El::Energy => "\u{1}e".to_owned(),
        })
        .collect();
    parts.join("\u{1f}")
}

fn decode(s: &str) -> Line {
    let mut line = Line::new();
    for part in s.split('\u{1f}') {
        let Some(cmd) = part.strip_prefix('\u{1}') else {
            if !part.is_empty() {
                line.els.push(El::Text(part.to_owned()));
            }
            continue;
        };
        let w: Vec<&str> = cmd.splitn(4, ' ').collect();
        let f = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        match w.first().copied() {
            Some("i") => line.els.push(El::Item {
                id: w.get(1).copied().unwrap_or("minecraft:air").to_owned(),
                count: f(2) as u32,
                small: w.get(3) == Some(&"1"),
            }),
            Some("a") => line.els.push(El::Arrow(f(1))),
            Some("b") => line.els.push(El::Bar { frac: f(1), color: f(2) as u32, text: w.get(3).copied().unwrap_or("").to_owned() }),
            Some("h") => line.els.push(El::Hearts { health: f(1), max: f(2), absorb: f(3) }),
            Some("r") => line.els.push(El::Armor(f(1))),
            Some("s") => line.els.push(El::Space(f(1))),
            Some("e") => line.els.push(El::Energy),
            Some("m") => line.els.push(El::Small(cmd.get(2..).unwrap_or("").to_owned())),
            Some("k") => line.els.push(El::Mark(cmd.get(2..).unwrap_or("").to_owned())),
            Some("I") => line.els.push(El::Icon(w.get(1).copied().unwrap_or("minecraft:air").to_owned())),
            Some("W") => line.els.push(El::Badge),
            Some("T") => line.els.push(El::Title(cmd.get(2..).unwrap_or("").to_owned())),
            Some("S") => line.els.push(El::Stats { absorb: f(1), armor: f(2) }),
            _ => {}
        }
    }
    line
}

/// A plugin's lines on the client, from what the client knows.
pub type ClientProvider = fn(&Game<'_>, &Target, &mut Vec<Line>);
/// A plugin's lines from the server's state, for the block at a position (plain text, or `encode`d lines).
pub type ServerProvider = fn(&mut Game<'_>, [i32; 3], &str, &mut Vec<String>);

/// A plugin's name for the block at a position, where the crosshair meets it: its icon's item, title and mod.
pub type NameProvider = fn(&Game<'_>, [i32; 3], &str, [f64; 3]) -> Option<(String, String, String)>;

static NAME_PROVIDERS: Mutex<Vec<NameProvider>> = Mutex::new(Vec::new());
static CLIENT_PROVIDERS: Mutex<Vec<ClientProvider>> = Mutex::new(Vec::new());

/// Adds a name provider (once).
pub fn register_name(f: NameProvider) {
    if let Ok(mut v) = NAME_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}
static SERVER_PROVIDERS: Mutex<Vec<ServerProvider>> = Mutex::new(Vec::new());

/// Adds a client provider (once; calling again does nothing).
pub fn register_client(f: ClientProvider) {
    if let Ok(mut v) = CLIENT_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}

/// Adds a server provider (once).
pub fn register_server(f: ServerProvider) {
    if let Ok(mut v) = SERVER_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}

/// `IServerExtensionProvider<ProgressView.Data>`: a machine's progress bars at a position, from the server's state:
/// how far along (0 to 1) and the text above the bar (ProgressView's `text`), if any.
pub type ProgressProvider = fn(&mut Game<'_>, [i32; 3], &str) -> Vec<(f32, Option<String>)>;

static PROGRESS_PROVIDERS: Mutex<Vec<ProgressProvider>> = Mutex::new(Vec::new());

/// `registerProgress`: adds a progress provider (once).
pub fn register_progress(f: ProgressProvider) {
    if let Ok(mut v) = PROGRESS_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}

/// A plugin's own options (IWailaClientRegistration.addConfig), after Jade's own in the options page.
static PLUGIN_OPTIONS: Mutex<Vec<(&'static str, &'static str, bool)>> = Mutex::new(Vec::new());

/// The options a plugin added: key, label and default, in the order they came.
pub fn plugin_options() -> Vec<(&'static str, &'static str, bool)> {
    PLUGIN_OPTIONS.lock().map(|v| v.clone()).unwrap_or_default()
}

/// Adds an on/off option to Jade's list, kept with Jade's settings under `key` (once; calling again does nothing):
/// a plugin's toggle for one of its providers. Pick a key no other mod does (`ae2_power_storage`).
pub fn register_option(key: &'static str, label: &'static str, default_on: bool) {
    let added = PLUGIN_OPTIONS.lock().is_ok_and(|mut v| {
        let new = !v.iter().any(|(k, _, _)| *k == key);
        if new {
            v.push((key, label, default_on));
        }
        new
    });
    if added {
        SETTINGS_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A plugin's own option. In a server provider this answers for the player who asked (the request names the toggles
/// they have off); elsewhere it is this game's setting.
pub fn plugin_on(game: &Game<'_>, key: &'static str) -> bool {
    on(game, key)
}

/// The options a plugin added, as their keys.
fn plugin_keys() -> Vec<&'static str> {
    PLUGIN_OPTIONS.lock().map(|v| v.iter().map(|(k, _, _)| *k).collect()).unwrap_or_default()
}

/// `assets/jade/lang/<code>.json` of the jar (this bundle's `data/lang`), compiled in.
fn jar_lang(code: &str) -> Option<&'static [u8]> {
    let bytes: &'static [u8] = match code {
        "de_de" => horizon_client::native_data!("jade", "lang/de_de.json"),
        "en_us" => horizon_client::native_data!("jade", "lang/en_us.json"),
        "es_es" => horizon_client::native_data!("jade", "lang/es_es.json"),
        "fr_fr" => horizon_client::native_data!("jade", "lang/fr_fr.json"),
        "it_it" => horizon_client::native_data!("jade", "lang/it_it.json"),
        "ja_jp" => horizon_client::native_data!("jade", "lang/ja_jp.json"),
        "ko_kr" => horizon_client::native_data!("jade", "lang/ko_kr.json"),
        "nl_nl" => horizon_client::native_data!("jade", "lang/nl_nl.json"),
        "pl_pl" => horizon_client::native_data!("jade", "lang/pl_pl.json"),
        "pt_br" => horizon_client::native_data!("jade", "lang/pt_br.json"),
        "ru_ru" => horizon_client::native_data!("jade", "lang/ru_ru.json"),
        "tr_tr" => horizon_client::native_data!("jade", "lang/tr_tr.json"),
        "uk_ua" => horizon_client::native_data!("jade", "lang/uk_ua.json"),
        "zh_cn" => horizon_client::native_data!("jade", "lang/zh_cn.json"),
        "zh_tw" => horizon_client::native_data!("jade", "lang/zh_tw.json"),
        _ => return None,
    };
    Some(bytes)
}

/// A key only a language this mod has added to has.
const LANG_MARK: &str = "hz.jade.lang";

/// The language as it is, on top of the jar's files: `Language::load` over these layers keeps the game's (a resource
/// pack's Jade strings included) and adds what only the jar has.
struct JarLanguage {
    code: String,
    current: Vec<u8>,
    bidirectional: bool,
}

impl horizon_client::lang::ResourceSource for JarLanguage {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        if let Some(code) = path.strip_prefix("assets/jade/lang/").and_then(|p| p.strip_suffix(".json")) {
            return jar_lang(code).map(<[u8]>::to_vec);
        }
        if path == format!("assets/zz_loaded/lang/{}.json", self.code) {
            return Some(self.current.clone());
        }
        // Language::load asks the language's direction of the packs.
        if path == "assets/pack.mcmeta" && self.bidirectional {
            return serde_json::to_vec(&serde_json::json!({"language": {self.code.as_str(): {"name": self.code.as_str(), "region": "", "bidirectional": true}}})).ok();
        }
        None
    }
    fn exists(&self, path: &str) -> bool {
        path == format!("assets/minecraft/lang/{}.json", self.code) || self.read(path).is_some()
    }
    fn namespaces(&self) -> Vec<String> {
        vec!["jade".to_owned(), "zz_loaded".to_owned()]
    }
}

/// The jar's strings in the game's language, again whenever the game has loaded a language (a change in Options, a
/// resource reload). A lookup a frame once done. Every `game.translate` of this mod reads them from then on.
pub fn ensure_lang() {
    use horizon_client::lang;
    if lang::has(LANG_MARK) {
        return;
    }
    let Some(loaded) = lang::current() else { return };
    let code = loaded.code.clone();
    let mut map = loaded.map().clone();
    map.insert(LANG_MARK.to_owned(), code.clone());
    let Ok(current) = serde_json::to_vec(&map) else { return };
    let source = JarLanguage { code: code.clone(), current, bidirectional: lang::bidirectional() };
    lang::set(lang::Language::load(&[&source], &code));
}

/// `Component.translatable` in the game's language, with `ensure_lang` having added the jar's.
pub fn tr(key: &str, fallback: &str) -> String {
    horizon_client::lang::get_or(key, fallback)
}

/// `tr` with its arguments in (`fill`).
pub fn trf(key: &str, fallback: &str, args: &[&str]) -> String {
    fill(&tr(key, fallback), args)
}

/// A language file's pattern with its arguments: `%s`, `%d`, `%1$s` and `%%` (Component.translatable, String.format),
/// `{0}`, `{1,choice,...}` and `{1,select,...}` (JadeClient.formatString's MessageFormat; `select` always takes `other`, as
/// this port has no JadeLanguages name classes). What an argument says isn't read again.
pub fn fill(pattern: &str, args: &[&str]) -> String {
    let arg = |n: usize| args.get(n).copied().unwrap_or("");
    let c: Vec<char> = pattern.chars().collect();
    let (mut out, mut i, mut next) = (String::new(), 0, 0);
    while i < c.len() {
        if c[i] == '%' {
            let digits = c[i + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
            let numbered = digits > 0 && c.get(i + 1 + digits) == Some(&'$');
            let at = if numbered { i + 2 + digits } else { i + 1 };
            match c.get(at) {
                Some('%') if !numbered => out.push('%'),
                Some('s' | 'd') if numbered => {
                    let n: usize = c[i + 1..i + 1 + digits].iter().collect::<String>().parse().unwrap_or(1);
                    out.push_str(arg(n.saturating_sub(1)));
                }
                Some('s' | 'd') => {
                    out.push_str(arg(next));
                    next += 1;
                }
                _ => {
                    out.push('%');
                    i += 1;
                    continue;
                }
            }
            i = at + 1;
        } else if c[i] == '{' {
            let mut depth = 0;
            let close = c[i..].iter().position(|ch| {
                depth += match ch {
                    '{' => 1,
                    '}' => -1,
                    _ => 0,
                };
                depth == 0
            });
            let Some(close) = close else {
                out.push('{');
                i += 1;
                continue;
            };
            let body: String = c[i + 1..i + close].iter().collect();
            let mut parts = body.splitn(3, ',');
            let number = parts.next().and_then(|n| n.trim().parse::<usize>().ok());
            match (number, parts.next().map(str::trim), parts.next()) {
                (Some(n), None, _) => out.push_str(arg(n)),
                (Some(n), Some("choice"), Some(choices)) => out.push_str(choice(choices, arg(n).parse::<f64>().unwrap_or(0.0))),
                (Some(_), Some("select"), Some(cases)) => out.push_str(cases.rsplit_once("other {").map_or("", |(_, rest)| rest.strip_suffix('}').unwrap_or(rest))),
                _ => {
                    out.push('{');
                    out.push_str(&body);
                    out.push('}');
                }
            }
            i += close + 1;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// ChoiceFormat: the last choice whose limit the number reaches (`1#`, `2≤`, `1.0<`), else the first.
fn choice(choices: &str, number: f64) -> &str {
    let mut chosen = "";
    for (i, part) in choices.split('|').enumerate() {
        let Some((at, mark)) = part.char_indices().find(|(_, m)| matches!(m, '#' | '≤' | '<')) else { continue };
        let limit: f64 = part[..at].trim().parse().unwrap_or(0.0);
        if i == 0 || number > limit || (mark != '<' && number == limit) {
            chosen = &part[at + mark.len_utf8()..];
        }
    }
    chosen
}

/// An option's key in the jar's language files (`OptionsList.Entry.makeKey`, `plugin_<id>` for a provider's). None for the
/// few this port added, which have no such key.
pub fn option_lang_key(key: &str) -> Option<String> {
    let minecraft = |path: &str| Some(format!("config.jade.plugin_minecraft.{path}"));
    let core = |path: &str| Some(format!("config.jade.plugin_jade.{path}"));
    match key {
        "display_tooltip" | "display_mode" | "display_blocks" | "display_entities" | "display_bosses" | "display_fluids" | "hide_from_tab_list" | "hide_from_guis" | "boss_bar_overlap" | "display_item" | "tts_mode"
        | "perspective_mode" | "reach_distance" | "item_mod_name" | "text_background_opacity" | "flip_main_hand" | "overlay_alpha" | "overlay_theme" | "theme_style" | "animation" | "narrate_keys" | "enable_profiles"
        | "debug_mode" | "accessibility_plugin" | "always_list_mode" => Some(format!("config.jade.{key}")),
        "scale" => Some("config.jade.overlay_scale".to_owned()),
        "object_name" | "mod_name" | "mod_name_smaller" | "distance" | "coordinates" | "block_face" | "registry_name" | "block_states" | "block_properties" | "loot_table" => core(key),
        "mod_name_translated" => core("mod_name.translated"),
        "coordinates_rel" => core("coordinates.rel"),
        "registry_special" => core("registry_name.special"),
        "entity_health" | "entity_armor" | "potion_effects" | "item_frame" | "container_entity" | "mob_growth" | "mob_breeding" | "armor_stand" | "painting" | "next_entity_drop" | "animal_owner" | "sulfur_cube"
        | "villager_profession" | "villager_restock" | "zombie_villager" | "pet_armor" | "item_tooltip" | "breaking_progress" | "harvest_tool" | "brewing_stand" | "tnt_stability" | "beehive" | "note_block" | "command_block"
        | "enchantment_power" | "total_enchantment_power" | "player_head" | "item_storage" | "energy_storage" | "fluid_storage" | "lectern" | "furnace" | "mob_spawner" | "crop_progress" | "redstone" | "jukebox" | "shelf"
        | "waxed" => minecraft(key),
        "health_max" => minecraft("entity_health.max_for_render"),
        "health_per_line" => minecraft("entity_health.icons_per_line"),
        "health_fractions" => minecraft("entity_health.show_fractions"),
        "armor_max" => minecraft("entity_armor.max_for_render"),
        "entity_stats" => minecraft("horse_stats"),
        "effects_limit" => minecraft("potion_effects.limit"),
        "harvest_new_line" => minecraft("harvest_tool.new_line"),
        "harvest_effective" => minecraft("harvest_tool.effective_tool"),
        "harvest_unbreakable" => minecraft("harvest_tool.show_unbreakable"),
        "harvest_creative" => minecraft("harvest_tool.creative"),
        "storage_detailed" => minecraft("item_storage.detailed_amount"),
        "storage_normal" => minecraft("item_storage.normal_amount"),
        "storage_names" => minecraft("item_storage.show_name_amount"),
        "storage_per_line" => minecraft("item_storage.items_per_line"),
        "storage_sort" => minecraft("item_storage.sort"),
        "energy_detailed" => minecraft("energy_storage.detailed"),
        "energy_style" => minecraft("energy_storage.style"),
        "fluid_detailed" => minecraft("fluid_storage.detailed"),
        "fluid_style" => minecraft("fluid_storage.style"),
        "access_sign" => Some("config.jade.plugin_jade_access.sign".to_owned()),
        "access_block" => Some("config.jade.plugin_jade_access.block".to_owned()),
        "access_entity" => Some("config.jade.plugin_jade_access.entity".to_owned()),
        "access_variant" => Some("config.jade.plugin_jade_access.entity_variant".to_owned()),
        "access_held" => Some("config.jade.plugin_jade_access.held_item".to_owned()),
        "access_npc" => Some("config.jade.plugin_jade_access.npc_description".to_owned()),
        _ => None,
    }
}

/// An option's title in the game's language.
pub fn option_label(o: &Opt) -> String {
    match option_lang_key(o.key) {
        Some(key) => tr(&key, o.label),
        None => o.label.to_owned(),
    }
}

/// What a value of an option is called in the jar's language files (`OptionsList.choices`: the option's key and the
/// enum's name).
fn value_lang_key(option: &str, value: &str) -> Option<String> {
    let enum_name = match (option, value) {
        (_, "true") => return Some("options.on".to_owned()),
        (_, "false") => return Some("options.off".to_owned()),
        ("reach_distance", "0") => return Some("options.off".to_owned()),
        ("display_mode", "hold") => "hold_key",
        ("display_mode" | "display_fluids" | "display_item" | "tts_mode" | "perspective_mode", v) => v,
        ("boss_bar_overlap", "none") => "no_operation",
        ("boss_bar_overlap", "hide_bar") => "hide_boss_bar",
        ("boss_bar_overlap", "hide_tip") => "hide_tooltip",
        ("boss_bar_overlap", "push") => "push_down",
        ("registry_name", "advanced") => "advanced_tooltips",
        ("pet_armor_mode", "damageable") => return Some("config.jade.plugin_minecraft.pet_armor_show_damageable".to_owned()),
        ("pet_armor_mode", "all") => return Some("config.jade.plugin_minecraft.pet_armor_show_all".to_owned()),
        ("energy_style" | "fluid_style", "bar") => "progress_bar",
        ("energy_style" | "fluid_style", "plain") => "plain_text",
        ("energy_style" | "fluid_style", "icon") => "icon",
        ("profile", "1") => return Some("jade.profile_preset.accessibility".to_owned()),
        ("profile", "2") => return Some("jade.profile_preset.minimalism".to_owned()),
        _ => return None,
    };
    Some(format!("{}_{enum_name}", option_lang_key(option)?))
}

/// One of Jade's settings: the file it is kept in, its label, its default and its values with their labels.
pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub values: &'static [(&'static str, &'static str)],
}

/// One of Jade's own settings, by its key.
pub fn option(key: &str) -> Option<&'static Opt> {
    OPTIONS.iter().find(|o| o.key == key)
}

const ONOFF: &[(&str, &str)] = &[("true", "On"), ("false", "Off")];
const fn on_by_default(key: &'static str, label: &'static str) -> Opt {
    Opt { key, label, default: "true", values: ONOFF }
}
const fn off_by_default(key: &'static str, label: &'static str) -> Opt {
    Opt { key, label, default: "false", values: ONOFF }
}

/// WailaConfig and the plugin options, in the order of Jade's settings screen.
pub const OPTIONS: &[Opt] = &[
    on_by_default("display_tooltip", "Enabled"),
    Opt { key: "display_mode", label: "Display Mode", default: "toggle", values: &[("toggle", "Toggle"), ("hold", "Hold"), ("lite", "Lite")] },
    on_by_default("display_blocks", "Display Blocks"),
    on_by_default("display_entities", "Display Entities"),
    on_by_default("display_bosses", "Display Bosses"),
    Opt { key: "display_fluids", label: "Display Fluids", default: "none", values: &[("none", "None"), ("any", "Any"), ("fallback", "Fallback")] },
    on_by_default("hide_from_tab_list", "Hide from Player List"),
    on_by_default("hide_from_guis", "Hide from GUIs"),
    Opt { key: "boss_bar_overlap", label: "Boss Bar Overlap", default: "push", values: &[("none", "No Operation"), ("hide_bar", "Hide Boss Bar"), ("hide_tip", "Hide Tooltip"), ("push", "Push Down Tooltip")] },
    Opt { key: "display_item", label: "Icon Position", default: "top", values: &[("hide", "Hide"), ("top", "Top"), ("centered", "Centered"), ("inline", "In-line")] },
    Opt { key: "tts_mode", label: "Text to Speech Mode", default: "press", values: &[("press", "Press"), ("toggle", "Toggle")] },
    Opt { key: "perspective_mode", label: "Perspective Mode", default: "camera", values: &[("camera", "From Camera"), ("eye", "From Eye")] },
    Opt { key: "reach_distance", label: "Extended Reach", default: "0", values: &[("0", "Off"), ("5", "5"), ("10", "10"), ("15", "15"), ("20", "20")] },
    on_by_default("item_mod_name", "Mod Name on Item Tooltip"),
    Opt { key: "overlay_x", label: "Overlay Position X", default: "center", values: &[("left", "Left"), ("center", "Center"), ("right", "Right")] },
    Opt { key: "position", label: "Overlay Position Y", default: "0", values: &[("0", "Top"), ("1", "Upper"), ("2", "Middle"), ("3", "Bottom")] },
    Opt { key: "scale", label: "Display Scale", default: "1", values: &[("0.5", "0.5"), ("0.75", "0.75"), ("1", "1"), ("1.25", "1.25"), ("1.5", "1.5")] },
    Opt { key: "text_background_opacity", label: "Text Background Opacity", default: "0", values: &[("0", "0%"), ("25", "25%"), ("50", "50%"), ("75", "75%"), ("100", "100%")] },
    off_by_default("flip_main_hand", "Flip Position with Main Hand"),
    Opt { key: "overlay_alpha", label: "Background Opacity", default: "70", values: &[("0", "0%"), ("25", "25%"), ("50", "50%"), ("70", "70%"), ("85", "85%"), ("100", "100%")] },
    Opt { key: "overlay_theme", label: "Theme", default: "dark", values: &[("dark", "Dark"), ("waila", "Classic"), ("create", "Create"), ("top", "One Probe")] },
    Opt { key: "theme_style", label: "Theme Style", default: "default", values: &[("default", "Default"), ("slim", "Slim")] },
    on_by_default("animation", "Overlay Animation"),
    off_by_default("narrate_keys", "Narrate Key Binds"),
    off_by_default("enable_profiles", "Profiles Enabled"),
    Opt { key: "profile", label: "Profile", default: "0", values: &[("0", "#0"), ("1", "#1 Accessibility Mode"), ("2", "#2 Minimalism"), ("3", "#3")] },
    off_by_default("debug_mode", "Debug Mode"),
    // WailaConfig.General.builtinCamouflage (no screen in Jade; in the file there).
    on_by_default("builtin_camouflage", "Built-in Camouflage"),
    // Core Features.
    on_by_default("object_name", "Target Name"),
    on_by_default("mod_name", "Mod Name"),
    off_by_default("mod_name_smaller", "Mod Name: Smaller"),
    on_by_default("mod_name_translated", "Translate Mod Name"),
    off_by_default("distance", "Distance"),
    off_by_default("coordinates", "Coordinates"),
    off_by_default("coordinates_rel", "Relative Coordinates"),
    off_by_default("block_face", "Block Face"),
    Opt { key: "registry_name", label: "Registry Name", default: "false", values: &[("false", "Off"), ("true", "On"), ("advanced", "Advanced Tooltips")] },
    off_by_default("registry_special", "Special IDs"),
    off_by_default("block_states", "Block States"),
    off_by_default("block_properties", "Block Properties"),
    off_by_default("loot_table", "Loot Table"),
    // Minecraft - Entity.
    on_by_default("entity_health", "Entity Health"),
    Opt { key: "health_max", label: "Max Health for Render", default: "40", values: &[("20", "20"), ("40", "40"), ("60", "60"), ("100", "100")] },
    Opt { key: "health_per_line", label: "Icons per Line", default: "10", values: &[("5", "5"), ("10", "10"), ("15", "15"), ("20", "20")] },
    off_by_default("health_fractions", "Show Fractions"),
    on_by_default("entity_armor", "Entity Armor"),
    Opt { key: "armor_max", label: "Max Armors for Render", default: "20", values: &[("10", "10"), ("20", "20"), ("40", "40")] },
    on_by_default("entity_stats", "Horse Stats"),
    on_by_default("potion_effects", "Entity Effects"),
    Opt { key: "effects_limit", label: "Effects Limit", default: "5", values: &[("1", "1"), ("3", "3"), ("5", "5"), ("10", "10"), ("99", "No limit")] },
    on_by_default("item_frame", "Item Frame"),
    on_by_default("container_entity", "Container Entity"),
    on_by_default("mob_growth", "Mob Growth"),
    on_by_default("mob_breeding", "Mob Breeding"),
    on_by_default("armor_stand", "Armor Stand"),
    on_by_default("painting", "Painting"),
    on_by_default("next_entity_drop", "Next Entity Drop"),
    on_by_default("animal_owner", "Animal Owner"),
    on_by_default("sulfur_cube", "Sulfur Cube"),
    on_by_default("villager_profession", "Villager Profession"),
    off_by_default("villager_restock", "Villager Restock"),
    on_by_default("zombie_villager", "Zombie Villager Conversion"),
    on_by_default("pet_armor", "Pet Armor"),
    Opt { key: "pet_armor_mode", label: "Pet Armor Shows", default: "damageable", values: &[("damageable", "Show Damageable"), ("all", "Show All")] },
    on_by_default("item_tooltip", "Item Description"),
    // Minecraft - Block.
    on_by_default("breaking_progress", "Breaking Progress"),
    on_by_default("harvest_tool", "Harvest Tool"),
    off_by_default("harvest_new_line", "Harvest Tool: Start in New Line"),
    on_by_default("harvest_effective", "Harvest Tool: Effective Tool"),
    off_by_default("harvest_unbreakable", "Harvest Tool: Show \"Unbreakable\""),
    off_by_default("harvest_creative", "Harvest Tool: Show in Creative Mode"),
    on_by_default("brewing_stand", "Brewing Stand"),
    on_by_default("tnt_stability", "TNT Stability"),
    on_by_default("beehive", "Beehive"),
    on_by_default("note_block", "Note Block"),
    on_by_default("command_block", "Command Block"),
    on_by_default("enchantment_power", "Enchantment Power"),
    on_by_default("total_enchantment_power", "Total Enchantment Power"),
    on_by_default("player_head", "Player Head"),
    on_by_default("item_storage", "Item Storage"),
    Opt { key: "storage_detailed", label: "Item Storage: Detailed Amount", default: "54", values: &[("27", "27"), ("54", "54"), ("108", "108")] },
    Opt { key: "storage_normal", label: "Item Storage: Normal Amount", default: "9", values: &[("0", "0"), ("9", "9"), ("18", "18"), ("27", "27"), ("54", "54")] },
    Opt { key: "storage_names", label: "Item Storage: Show Name Threshold", default: "5", values: &[("0", "0"), ("2", "2"), ("5", "5"), ("9", "9")] },
    Opt { key: "storage_per_line", label: "Item Storage: Items per Line", default: "9", values: &[("5", "5"), ("9", "9"), ("12", "12")] },
    off_by_default("storage_sort", "Item Storage: Sort by Count"),
    on_by_default("energy_storage", "Energy Storage"),
    off_by_default("energy_detailed", "Energy Storage: Detailed-Only"),
    Opt { key: "energy_style", label: "Energy Storage: Display Style", default: "bar", values: &[("bar", "Progress Bar"), ("plain", "Plain Text"), ("icon", "Icon")] },
    on_by_default("fluid_storage", "Fluid Storage"),
    off_by_default("fluid_detailed", "Fluid Storage: Detailed-Only"),
    Opt { key: "fluid_style", label: "Fluid Storage: Display Style", default: "bar", values: &[("bar", "Progress Bar"), ("plain", "Plain Text"), ("icon", "Icon")] },
    on_by_default("lectern", "Lectern"),
    on_by_default("furnace", "Furnace Contents"),
    on_by_default("mob_spawner", "Mob Spawner"),
    on_by_default("crop_progress", "Crop Progress"),
    on_by_default("redstone", "Redstone Info"),
    on_by_default("jukebox", "Jukebox Record"),
    on_by_default("shelf", "Shelf"),
    on_by_default("waxed", "Waxed State"),
    // Accessibility.
    // AccessorClientHandler: without the Accessibility Plugin none of the access providers run.
    off_by_default("accessibility_plugin", "Accessibility Plugin"),
    off_by_default("always_list_mode", "Always Use List Mode"),
    on_by_default("access_sign", "Sign Content"),
    on_by_default("access_block", "Block Details"),
    on_by_default("access_entity", "Entity Details"),
    on_by_default("access_variant", "Entity Variant"),
    on_by_default("access_held", "Entity Held Item"),
    on_by_default("access_npc", "NPC Description"),
];

/// The options the server reads while it answers: the request names those the asking player has off.
const SERVER_KEYS: [&str; 21] = ["accessibility_plugin", "access_entity", "item_storage", "energy_storage", "energy_detailed", "fluid_storage", "fluid_detailed", "furnace", "brewing_stand", "jukebox", "lectern", "beehive", "command_block", "mob_spawner", "shelf", "redstone", "waxed", "loot_table", "storage_sort", "always_list_mode", "pet_armor"];

/// AccessibilityPlugin: an access provider, only with the plugin on.
fn access(game: &Game<'_>, key: &'static str) -> bool {
    on(game, "accessibility_plugin") && on(game, key)
}

/// Bumped when a setting changes, for every thread's copy.
static SETTINGS_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Which settings are current: moves when one changes.
pub fn settings_epoch() -> u32 {
    SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
}

thread_local! {
    static VALUES: RefCell<(u32, std::collections::HashMap<&'static str, String>)> = RefCell::new((u32::MAX, std::collections::HashMap::new()));
    static IGNORED: RefCell<(u32, std::collections::HashSet<String>, std::collections::HashSet<String>)> = RefCell::new((u32::MAX, Default::default(), Default::default()));
    /// Block Properties' fire numbers (the game's own block behaviours), read when first asked for.
    static BEHAVIOURS: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
    /// The toggles the player being answered has off, while the server answers them.
    static REQUEST_OFF: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    /// HarvestToolProvider.resultCache: each block's tools, and the epoch it was filled at.
    static HARVEST_CACHE: RefCell<(u32, std::collections::HashMap<String, std::rc::Rc<[String]>>)> = RefCell::new((u32::MAX, Default::default()));
    /// ModIdentification's names, the same way.
    static MOD_NAMES: RefCell<(u32, std::collections::HashMap<String, String>)> = RefCell::new((u32::MAX, Default::default()));
}

/// While the server answers a player: that player's own say on a toggle it was told about.
fn request_says(key: &str) -> Option<bool> {
    REQUEST_OFF.with_borrow(|r| {
        let off = r.as_ref()?;
        if off.iter().any(|k| k == key) {
            Some(false)
        } else if SERVER_KEYS.iter().any(|k| *k == key) || plugin_keys().iter().any(|k| *k == key) {
            Some(true)
        } else {
            None
        }
    })
}

/// Every toggle the server asks about that this player has off, for a request's end (`;a,b`).
fn request_off_list(game: &Game<'_>) -> String {
    let mut off = String::from(";");
    for k in SERVER_KEYS.into_iter().chain(plugin_keys()) {
        if !on(game, k) {
            if off.len() > 1 {
                off.push(',');
            }
            off.push_str(k);
        }
    }
    off
}

/// Reads a request's off list: what a client sent is checked by nothing, so only known toggles, and few of them.
fn set_request_off(list: &str) {
    let plugins = plugin_keys();
    let off: Vec<String> = list.split(',').filter(|k| SERVER_KEYS.iter().any(|s| s == k) || plugins.iter().any(|p| p == k)).take(128).map(str::to_owned).collect();
    REQUEST_OFF.with_borrow_mut(|r| *r = Some(off));
}

fn clear_request_off() {
    REQUEST_OFF.with_borrow_mut(|r| *r = None);
}

/// A setting's value now.
pub fn opt(game: &Game<'_>, key: &'static str) -> String {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    VALUES.with_borrow_mut(|(at, values)| {
        if *at != epoch {
            *at = epoch;
            values.clear();
            for o in OPTIONS {
                values.insert(o.key, game.native_setting(ID, o.key).unwrap_or_else(|| o.default.to_owned()));
            }
            if let Ok(plugins) = PLUGIN_OPTIONS.lock() {
                for (key, _, default_on) in plugins.iter() {
                    values.insert(*key, game.native_setting(ID, key).unwrap_or_else(|| default_on.to_string()));
                }
            }
            // config/jade/server-plugin-overrides.json (`/jadec generate_server_overrides`) wins.
            let overrides = Game::native_config_dir().ok().and_then(|d| std::fs::read_to_string(d.join("jade/server-plugin-overrides.json")).ok()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            if let Some(map) = overrides.as_ref().and_then(|v| v.as_object()) {
                for o in OPTIONS {
                    if let Some(v) = map.get(o.key).and_then(|v| v.as_str()) {
                        values.insert(o.key, v.to_owned());
                    }
                }
            }
        }
        values.get(key).cloned().unwrap_or_default()
    })
}

pub fn on(game: &Game<'_>, key: &'static str) -> bool {
    if request_says(key) == Some(false) {
        return false;
    }
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    // The common case without a copy: the overlay asks for many each frame.
    let quick = VALUES.with_borrow(|(at, values)| if *at == epoch { values.get(key).map(|v| v != "false") } else { None });
    quick.unwrap_or_else(|| opt(game, key) != "false")
}

pub fn num(game: &Game<'_>, key: &'static str) -> f64 {
    opt(game, key).parse().unwrap_or(0.0)
}

/// What a slider or a dragged overlay says while it moves: read back by `opt` at once, with no file written and no
/// caches dropped. `commit_live` keeps it.
pub fn set_live(game: &Game<'_>, key: &'static str, value: String) {
    let _ = opt(game, key);
    VALUES.with_borrow_mut(|(_, values)| {
        values.insert(key, value);
    });
}

/// Keeps a value `set_live` said. `settings_changed` once the last is kept.
pub fn commit_live(game: &Game<'_>, key: &'static str) {
    game.native_set_setting(ID, key, &opt(game, key));
}

/// `config/jade/hide-blocks.json` and `hide-entities.json`: `{"values": ["minecraft:barrier", "#tag"]}`, as Jade's.
pub fn ignore_files() -> Option<[std::path::PathBuf; 2]> {
    let dir = Game::native_config_dir().ok()?.join("jade");
    let _ = std::fs::create_dir_all(&dir);
    let files = [dir.join("hide-blocks.json"), dir.join("hide-entities.json")];
    let defaults = ["{\n  \"values\": [\n    \"minecraft:barrier\",\n    \"minecraft:light\",\n    \"minecraft:structure_void\"\n  ]\n}\n", "{\n  \"values\": [\n    \"minecraft:area_effect_cloud\",\n    \"minecraft:firework_rocket\",\n    \"minecraft:interaction\",\n    \"minecraft:text_display\",\n    \"minecraft:lightning_bolt\"\n  ]\n}\n"];
    for (f, d) in files.iter().zip(defaults) {
        if !f.exists() {
            let _ = std::fs::write(f, d);
        }
    }
    Some(files)
}

fn ignored(game: &Game<'_>, block: Option<&str>, entity: Option<&str>) -> bool {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    IGNORED.with_borrow_mut(|(at, blocks, entities)| {
        if *at != epoch {
            *at = epoch;
            blocks.clear();
            entities.clear();
            if let Some(files) = ignore_files() {
                let read = |f: &std::path::Path| -> Vec<String> {
                    std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| Some(v.get("values")?.as_array()?.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())).unwrap_or_default()
                };
                blocks.extend(read(&files[0]));
                entities.extend(read(&files[1]));
            }
        }
        if let Some(b) = block
            && (blocks.contains(b) || blocks.iter().any(|t| t.strip_prefix('#').is_some_and(|tag| game.res.items().block_in_tag(tag, b))))
        {
            return true;
        }
        entity.is_some_and(|e| entities.contains(e))
    })
}

#[derive(Default)]
struct Client {
    /// Fading in (Overlay Animation): how far along, and when it was last drawn.
    fade: f32,
    fade_at: Option<std::time::Instant>,
    /// The overlay was drawn this frame (Boss Bar Overlap: Hide Boss Bar).
    showing: bool,
    /// `/jadec pin`: this target stays on the box.
    pinned: Option<Target>,
    /// Text to Speech Mode Toggle: reads each new target out.
    tts_on: bool,
    tts_last: String,
    /// A line to say for a key (Narrate Key Binds).
    say: Option<String>,
    /// Where the crosshair meets the block it is on.
    hit_point: [f64; 3],
    /// The server's lines for a block, and when they came.
    data: Option<([i32; 3], Vec<String>, std::time::Instant)>,
    /// The mob last asked about, and when.
    mob_asked: Option<(u32, std::time::Instant)>,
    /// A mob's lines from the server.
    mob_data: Option<(u32, Vec<String>, std::time::Instant)>,
    asked: Option<([i32; 3], std::time::Instant)>,
    asked_details: bool,
    asked_slot: i32,
    /// Jade's key toggles: the overlay shown (Show Overlay), fluids targeted (Toggle Fluid).
    hidden: bool,
    liquids: bool,
    /// The mappings down last frame (each toggle acts on its press).
    keys_down: Vec<&'static str>,
    /// Jade's breaking progress fade (BreakingProgressProvider.progressAlpha).
    progress_alpha: f32,
    /// The narrate key was pressed: the next box is read out (JadeClient.narrate).
    narrate: bool,
    /// The recipes (false) or uses (true) key was pressed, and the item it asks about once the inventory is open
    /// (JEICompat.lookup: JEI's recipe view of the looked-at block's item).
    lookup: Option<bool>,
    lookup_item: Option<(String, bool)>,
    /// The config key or `/jadec config`: HomeConfigScreen.
    open_config: bool,
    /// The box built this frame for a config screen to draw (OverlayRenderer under PreviewOptionsScreen).
    built: Option<Box<BuiltBox>>,
    /// A profile key was pressed (JadeClient.profiles).
    use_profile: Option<u8>,
    /// The first time a key closes the overlay (0) or opens the narrator (1): its hint in chat.
    hint: Option<u8>,
    /// How far the crosshair is from the entity it is on, and where it meets it.
    entity_hit: (f64, [f64; 3]),
    /// When the player was last in powder snow (JadeClient.inPowderSnowTime).
    powder_at: Option<std::time::Instant>,
    /// ItemBERProvider's icon for a head or pot, kept a moment.
    ber_icon: Option<([i32; 3], Option<ItemStack>, std::time::Instant)>,
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::default());
}

/// A mod id's name as Jade shows it under the box (ModIdentification.getModName: with Translate Mod Name on, the
/// language's `jade.modName.<id>` or `itemGroup.<id>` before the loader's name).
pub fn mod_name(game: &Game<'_>, ns: &str) -> String {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(name) = MOD_NAMES.with_borrow_mut(|(at, names)| {
        if *at != epoch {
            *at = epoch;
            names.clear();
        }
        names.get(ns).cloned()
    }) {
        return name;
    }
    let loader = match ns {
        "minecraft" => "Minecraft".to_owned(),
        "ae2" => "Applied Energistics 2".to_owned(),
        "jade" => "Jade".to_owned(),
        other => other
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
            })
            .collect::<Vec<_>>()
            .join(" "),
    };
    let mut name = loader;
    if on(game, "mod_name_translated") {
        for key in [format!("jade.modName.{ns}"), format!("itemGroup.{ns}")] {
            let t = game.translate(&key, "");
            if !t.is_empty() && t != key {
                name = t;
                break;
            }
        }
    }
    MOD_NAMES.with_borrow_mut(|(_, names)| names.insert(ns.to_owned(), name.clone()));
    name
}

/// Legacy-coded text that may also carry `§#RRGGBB` colours (what follows takes the colour until the next code).
pub fn rich(s: &str) -> horizon_client::text::Text {
    let mut parts = s.split("§#");
    let mut out = horizon_client::text::Text::legacy(parts.next().unwrap_or(""));
    for part in parts {
        let (hex, rest) = part.split_at(part.len().min(6));
        let color = u32::from_str_radix(hex, 16).unwrap_or(0xFFFFFF);
        let mut t = horizon_client::text::Text::legacy(rest);
        // Spans before any colour code of their own take the hex colour (`§l` etc. keep it).
        for span in &mut t.spans {
            if span.style.color.is_some() {
                break;
            }
            span.style.color = Some(color);
        }
        out.spans.extend(t.spans);
    }
    out
}

/// Text without its `§` codes, for measuring.
pub fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut skip = 0u8;
    for c in s.chars() {
        if skip > 0 {
            // `§#RRGGBB` skips the six digits too.
            skip = if skip == 1 && c == '#' { 6 } else { skip - 1 };
        } else if c == '§' {
            skip = 1;
        } else {
            out.push(c);
        }
    }
    out
}

/// An item's name as its tooltip's first line.
pub fn item_name(game: &Game<'_>, id: &str) -> String {
    game.stack_tooltip(&ItemStack::new(id, 1)).first().map_or_else(|| id.to_owned(), |(t, _)| t.plain())
}

/// IThemeHelper.seconds: a duration in ticks as Jade writes it ("12.5s", "2:05").
pub fn seconds(ticks: i64) -> String {
    // ThemeHelper.seconds: whole seconds, floored, then the two largest units.
    let mut s = (ticks as f32 / 20.0).floor() as i64;
    if s >= 3600 {
        let h = s / 3600;
        s %= 3600;
        let m = s / 60;
        if m == 0 {
            return trf("jade.hours", "{0}h", &[&h.to_string()]);
        }
        return trf("jade.hours_minutes", "{0}h {1}m", &[&h.to_string(), &m.to_string()]);
    }
    if s >= 60 {
        let m = s / 60;
        s %= 60;
        if s == 0 {
            return trf("jade.minutes", "{0}m", &[&m.to_string()]);
        }
        return trf("jade.minutes_seconds", "{0}m {1}s", &[&m.to_string(), &s.to_string()]);
    }
    trf("jade.seconds", "{0}s", &[&s.to_string()])
}

/// What the player looks at: the nearer of the block and the mob under the crosshair (fluids only with Toggle
/// Fluid on).
fn target(game: &Game<'_>) -> Option<Target> {
    if !on(game, "display_tooltip") {
        return None;
    }
    // Extended Reach, and Perspective Mode's camera: the block on the crosshair from there.
    let extra = num(game, "reach_distance");
    let block_reach = game.block_reach + extra; // RayTracing.fire: the player's own range, Extended Reach on top.
    let camera_ray = opt(game, "perspective_mode") == "camera" && !game.camera_type.first_person();
    let mut block = game.native_view_hit().filter(|(d, _)| f64::from(*d) <= block_reach);
    if camera_ray
        && let Some((eye, dir)) = game.aim()
    {
        let (origin, look) = match game.camera_type {
            crate::game::perspective::CameraType::ThirdPersonFront => ([eye[0] + dir[0] * 4.0, eye[1] + dir[1] * 4.0, eye[2] + dir[2] * 4.0], [-dir[0], -dir[1], -dir[2]]),
            _ => ([eye[0] - dir[0] * 4.0, eye[1] - dir[1] * 4.0, eye[2] - dir[2] * 4.0], dir),
        };
        if let (Some(hit), Some((d, mut base))) = (game.clip_ray(origin, look, block_reach + 4.0), game.native_view_hit()) {
            base.block_x = hit.pos[0];
            base.block_y = hit.pos[1];
            base.block_z = hit.pos[2];
            base.x = hit.point[0];
            base.y = hit.point[1];
            base.z = hit.point[2];
            let id = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            if let Some(state) = game.pack.assets.registry.state(id) {
                base.block = state.name.to_string();
            }
            block = Some((d, base));
        }
    }
    if let Some((_, hit)) = &block {
        CLIENT.with_borrow_mut(|c| c.hit_point = [hit.x, hit.y, hit.z]);
    }
    // The nearest entity Jade can show: mobs, frames, paintings, armour stands, dropped items.
    let mut entity: Option<(f64, Target)> = None;
    let mut offer = |d: f64, t: Target| {
        // RayTracing.getEntityHitResult: a nearer entity wins.
        if entity.as_ref().is_none_or(|(b, _)| d < *b) {
            entity = Some((d, t));
        }
    };
    if let Some((i, d, _)) = game.mob_hit() {
        offer(d, Target::Mob { index: i });
    }
    if let Some((i, d)) = game.frame_hit() {
        offer(d, Target::Frame { index: i });
    }
    if let Some((i, d)) = game.painting_hit() {
        offer(d, Target::Painting { index: i });
    }
    if let Some((i, d, _)) = game.armor_stand_hit() {
        offer(d, Target::Stand { index: i });
    }
    if let Some((i, d)) = item_hit(game) {
        offer(d, Target::Item { index: i });
    }
    if let Some((i, d)) = falling_hit(game) {
        offer(d, Target::Falling { index: i });
    }
    if let Some((i, d)) = game.cart_hit() {
        offer(d, Target::Cart { index: i });
    }
    if let Some((i, d)) = game.boat_hit() {
        offer(d, Target::Boat { index: i });
    }
    // RayTracing.getEntityHitResult widens a box this small by 0.3: how a display, which has none, gets hit at all.
    if let Some((eye, dir)) = game.aim() {
        let reach = game.entity_reach + extra;
        for (id, d) in &game.text_displays {
            if d.hz_jade_block().is_none() && d.hz_jade_item().is_none() {
                continue;
            }
            let p = d.position();
            if let Some(t) = ray_box(eye, dir, [p.x - 0.3, p.y - 0.3, p.z - 0.3], [p.x + 0.3, p.y + 0.3, p.z + 0.3], reach) {
                offer(t, Target::Display { id: *id });
            }
        }
    }
    // Extended Reach: mobs past the hand's reach.
    if extra > 0.0
        && let Some((eye, dir)) = game.aim()
    {
        for (i, m) in game.mobs.iter().enumerate() {
            let (hw, h) = (f64::from(m.kind.width) / 2.0, f64::from(m.kind.height));
            let lo = [m.position.x - hw, m.position.y, m.position.z - hw];
            let hi = [m.position.x + hw, m.position.y + h, m.position.z + hw];
            if let Some(t) = ray_box(eye, dir, lo, hi, extra) {
                offer(t, Target::Mob { index: i });
            }
        }
    }
    let cfg = config(game);
    let block = block.filter(|(_, hit)| cfg.blocks && !ignored(game, Some(&hit.block), None));
    let entity = entity.filter(|_| cfg.entities);
    let entity = entity.filter(|(_, t)| {
        let id = match t {
            Target::Mob { index } => game.mobs.get(*index).map(|m| m.kind.id.to_string()),
            Target::Painting { .. } => Some("minecraft:painting".to_owned()),
            Target::Item { .. } => Some("minecraft:item".to_owned()),
            Target::Stand { .. } => Some("minecraft:armor_stand".to_owned()),
            Target::Cart { index } => game.carts.get(*index).map(|c| c.kind.id().to_owned()),
            _ => None,
        };
        !id.is_some_and(|id| ignored(game, None, Some(&id)))
    });
    let entity = entity.filter(|(_, t)| {
        cfg.bosses || !matches!(t, Target::Mob { index } if game.mobs.get(*index).is_some_and(|m| matches!(m.kind.id.as_ref(), "minecraft:ender_dragon" | "minecraft:wither")))
    });
    // RayTracing.rayTrace: fluids count from outside them with Any (or Toggle Fluid), and with Fallback only when no
    // block is there.
    let fluids_mode = opt(game, "display_fluids");
    let any_fluid = fluids_mode == "any" || CLIENT.with_borrow(|c| c.liquids);
    let block = block.filter(|(_, hit)| !hit.block.ends_with(":air"));
    let fluid = if cfg.blocks && (any_fluid || (fluids_mode == "fallback" && block.is_none())) {
        fluid_hit(game, block_reach, any_fluid)
    } else {
        None
    };
    let on_fluid = |d: f64| {
        if let Some((eye, dir)) = game.aim() {
            CLIENT.with_borrow_mut(|c| c.hit_point = [eye[0] + dir[0] * d, eye[1] + dir[1] * d, eye[2] + dir[2] * d]);
        }
    };
    let looked_at = match (block, fluid) {
        (Some((bd, _)), Some((fd, pos, name))) if fd < f64::from(bd) => {
            on_fluid(fd);
            Some((fd, Target::Block { pos, block: name }))
        }
        (Some((bd, hit)), _) => Some((f64::from(bd), Target::Block { pos: [hit.block_x, hit.block_y, hit.block_z], block: hit.block })),
        (None, Some((fd, pos, name))) => {
            on_fluid(fd);
            Some((fd, Target::Block { pos, block: name }))
        }
        (None, None) => None,
    };
    let chosen = match (looked_at, entity) {
        (Some((bd, _)), Some((ed, t))) if ed < bd => Some((Some(ed), t)),
        (None, Some((ed, t))) => Some((Some(ed), t)),
        (Some((_, t)), _) => Some((None, t)),
        (None, None) => None,
    };
    let (entity_d, chosen) = chosen?;
    if let Some(d) = entity_d
        && let Some((eye, dir)) = game.aim()
    {
        CLIENT.with_borrow_mut(|c| c.entity_hit = (d, [eye[0] + dir[0] * d, eye[1] + dir[1] * d, eye[2] + dir[2] * d]));
    }
    Some(match chosen {
        Target::Block { pos, block } => {
            let block = camouflage(game, &block).unwrap_or(block);
            Target::Block { pos, block }
        }
        other => other,
    })
}

/// The first fluid along the crosshair (ClipContext.Fluid.ANY), its distance, position and id. None with the eye in
/// a fluid unless it comes as Fallback (`from_outside`: RayTracing's check of the eye's block).
fn fluid_hit(game: &Game<'_>, reach: f64, from_outside: bool) -> Option<(f64, [i32; 3], String)> {
    let (eye, dir) = game.aim()?;
    let fluid_at = |p: [i32; 3]| -> Option<&'static str> {
        let facts = game.res.facts().get(game.world.block(p[0], p[1], p[2]));
        match &*facts.name {
            "minecraft:water" => Some("minecraft:water"),
            "minecraft:lava" => Some("minecraft:lava"),
            _ if facts.waterlogged => Some("minecraft:water"),
            _ => None,
        }
    };
    let mut pos = eye.map(|v| v.floor() as i32);
    if from_outside && fluid_at(pos).is_some() {
        return None;
    }
    // BlockGetter.traverseBlocks.
    let step = dir.map(|d| if d > 0.0 { 1 } else { -1 });
    let delta = dir.map(|d| if d == 0.0 { f64::INFINITY } else { (1.0 / d).abs() });
    let mut t_max = [0.0; 3];
    for a in 0..3 {
        let edge = f64::from(pos[a]) + if step[a] > 0 { 1.0 } else { 0.0 };
        t_max[a] = if dir[a] == 0.0 { f64::INFINITY } else { (edge - eye[a]) / dir[a] };
    }
    loop {
        let a = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] { 0 } else if t_max[1] <= t_max[2] { 1 } else { 2 };
        let t = t_max[a];
        if t > reach {
            return None;
        }
        pos[a] += step[a];
        t_max[a] += delta[a];
        if let Some(name) = fluid_at(pos) {
            return Some((t, pos, name.to_owned()));
        }
    }
}

/// JadeClient.builtInOverrides: what a block passes for, for a player who should not see through it: a trapped chest
/// as its chest, an infested block as its host, powder snow as snow (unless the player was in some these 8 seconds),
/// a suspicious block as what it turns into. Not in creative or spectator, nor for a low-vision user (narration on).
fn camouflage(game: &Game<'_>, block: &str) -> Option<String> {
    if game.creative || game.spectator() || !on(game, "builtin_camouflage") || CLIENT.with_borrow(|c| c.tts_on) {
        return None;
    }
    let (ns, path) = block.split_once(':')?;
    let host = if let Some(rest) = path.strip_prefix("trapped_") {
        format!("{ns}:{rest}")
    } else if let Some(rest) = path.strip_suffix("_trapped_chest") {
        format!("{ns}:{rest}_chest")
    } else if let Some(rest) = path.strip_prefix("infested_") {
        format!("{ns}:{rest}")
    } else if let Some(rest) = path.strip_prefix("suspicious_") {
        format!("{ns}:{rest}")
    } else if block == "minecraft:powder_snow" && CLIENT.with_borrow(|c| c.powder_at.is_none_or(|at| at.elapsed().as_millis() > 8000)) {
        "minecraft:snow_block".to_owned()
    } else {
        return None;
    };
    game.pack.assets.registry.default_state(&host).map(|_| host)
}

/// The state a block is shown as: its own, or what it passes for's default (`camouflage`).
fn shown_state(game: &Game<'_>, pos: [i32; 3], block: &str) -> horizon_world::registry::BlockStateId {
    let real = game.world.block(pos[0], pos[1], pos[2]);
    if &*game.res.facts().get(real).name == block {
        real
    } else {
        game.pack.assets.registry.default_state(block).unwrap_or(real)
    }
}

/// Where a ray enters a box, within `reach`.
fn ray_box(eye: [f64; 3], dir: [f64; 3], lo: [f64; 3], hi: [f64; 3], reach: f64) -> Option<f64> {
    let (mut t0, mut t1) = (0.0f64, reach);
    for a in 0..3 {
        if dir[a].abs() < 1e-9 {
            if eye[a] < lo[a] || eye[a] > hi[a] {
                return None;
            }
            continue;
        }
        let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
    }
    (t0 <= t1).then_some(t0)
}

/// The falling block under the crosshair (its whole-block box) within reach, and its distance.
fn falling_hit(game: &Game<'_>) -> Option<(usize, f64)> {
    let (eye, dir) = game.aim()?;
    let reach = game.entity_reach + num(game, "reach_distance");
    let mut best: Option<(usize, f64)> = None;
    for (i, f) in game.falling.iter().enumerate() {
        let p = [f.position.x, f.position.y, f.position.z];
        let lo = [p[0] - 0.49, p[1], p[2] - 0.49];
        let hi = [p[0] + 0.49, p[1] + 0.98, p[2] + 0.49];
        let (mut t0, mut t1) = (0.0f64, reach);
        let mut hit = true;
        for a in 0..3 {
            if dir[a].abs() < 1e-9 {
                hit &= eye[a] >= lo[a] && eye[a] <= hi[a];
                continue;
            }
            let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
        }
        if hit && t0 <= t1 && best.is_none_or(|b| t0 < b.1) {
            best = Some((i, t0));
        }
    }
    best
}

/// The dropped item under the crosshair (ItemEntity's 0.25 box) within reach, and its distance.
fn item_hit(game: &Game<'_>) -> Option<(usize, f64)> {
    let (eye, dir) = game.aim()?;
    let reach = game.entity_reach + num(game, "reach_distance");
    let mut best: Option<(usize, f64)> = None;
    for (i, e) in game.dropped.iter().enumerate() {
        let p = [e.position.x, e.position.y, e.position.z];
        let lo = [p[0] - 0.125, p[1], p[2] - 0.125];
        let hi = [p[0] + 0.125, p[1] + 0.25, p[2] + 0.125];
        let (mut t0, mut t1) = (0.0f64, reach);
        let mut hit = true;
        for a in 0..3 {
            if dir[a].abs() < 1e-9 {
                hit &= eye[a] >= lo[a] && eye[a] <= hi[a];
                continue;
            }
            let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
        }
        if hit && t0 <= t1 && best.is_none_or(|b| t0 < b.1) {
            best = Some((i, t0));
        }
    }
    best
}

/// VanillaPlugin's harvest plugin: each ToolType's tiers, lowest first, and its skipInstaBreakingBlock.
const TOOL_TYPES: [(&[&str], bool); 6] = [
    (&["minecraft:wooden_pickaxe", "minecraft:golden_pickaxe", "minecraft:stone_pickaxe", "minecraft:copper_pickaxe", "minecraft:iron_pickaxe", "minecraft:diamond_pickaxe", "minecraft:netherite_pickaxe"], true),
    (&["minecraft:wooden_axe", "minecraft:golden_axe", "minecraft:stone_axe", "minecraft:copper_axe", "minecraft:iron_axe", "minecraft:diamond_axe", "minecraft:netherite_axe"], true),
    (&["minecraft:wooden_shovel", "minecraft:golden_shovel", "minecraft:stone_shovel", "minecraft:copper_shovel", "minecraft:iron_shovel", "minecraft:diamond_shovel", "minecraft:netherite_shovel"], true),
    (&["minecraft:wooden_hoe", "minecraft:golden_hoe", "minecraft:stone_hoe", "minecraft:copper_hoe", "minecraft:iron_hoe", "minecraft:diamond_hoe", "minecraft:netherite_hoe"], true),
    (&["minecraft:wooden_sword"], true),
    (&["minecraft:shears"], false),
];

/// ToolTier.item's predicate: the first rule that says about drops, else whether the tool mines it faster.
fn tier_correct(items: &horizon_client::items::Items, tool: &str, block: &str) -> bool {
    let Some(info) = items.get(tool) else { return false };
    for rule in &info.tool {
        if let Some(correct) = rule.correct
            && rule.blocks.iter().any(|b| b.strip_prefix('#').map_or(b == block, |tag| items.block_in_tag(tag, block)))
        {
            return correct;
        }
    }
    items.mining_speed(Some(&ItemStack::new(tool, 1)), block) > info.tool_default_speed
}

/// HarvestToolProvider.getTool, cached per block: the lowest tier of each tool type that suits it.
fn harvest_tools(game: &Game<'_>, block: &str, insta: bool) -> std::rc::Rc<[String]> {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    let hit = HARVEST_CACHE.with_borrow_mut(|(at, cache)| {
        if *at != epoch {
            *at = epoch;
            cache.clear();
        }
        cache.get(block).cloned()
    });
    if let Some(tools) = hit {
        return tools;
    }
    let items = game.res.items();
    let mut tools = Vec::new();
    for (tiers, skip_insta) in TOOL_TYPES {
        if skip_insta && insta {
            continue;
        }
        let found = tiers.iter().find(|t| tier_correct(items, t, block) || (**t == "minecraft:shears" && shear_block(game, block)));
        if let Some(t) = found {
            tools.push((*t).to_owned());
        }
    }
    let tools: std::rc::Rc<[String]> = tools.into();
    HARVEST_CACHE.with_borrow_mut(|(_, cache)| cache.insert(block.to_owned(), tools.clone()));
    tools
}

/// ToolTypeRegistryImpl.DEFAULT_SHEARS_TIER's extra blocks: tripwire, and what LootTableMineableCollector finds:
/// blocks whose loot table asks for shears.
fn shear_block(game: &Game<'_>, block: &str) -> bool {
    if block == "minecraft:tripwire" {
        return true;
    }
    let Some((ns, path)) = block.split_once(':') else { return false };
    game.native_data(&format!("data/{ns}/loot_table/blocks/{path}.json")).is_some_and(|bytes| {
        let text = String::from_utf8_lossy(&bytes);
        text.contains("minecraft:match_tool") && text.contains("minecraft:shears")
    })
}

/// HarvestToolProvider: the tools the block wants, with a tick or cross where the block needs the right one.
fn harvest_tool(game: &Game<'_>, pos: [i32; 3], block: &str) -> Option<Vec<El>> {
    if (game.creative || game.spectator()) && !on(game, "harvest_creative") {
        return None;
    }
    // Not in adventure mode, where the player can't break it (Player.blockActionRestricted).
    if game.mode == horizon_client::command::args::GameMode::Adventure {
        return None;
    }
    let facts = game.res.facts().get(shown_state(game, pos, block));
    if facts.hardness < 0.0 {
        return on(game, "harvest_unbreakable").then(|| vec![El::Text(format!("§c{}", tr("jade.harvest_tool.unbreakable", "Unbreakable")))]);
    }
    // Effective Tool: a tool that only mines faster, not one the block needs.
    if !facts.requires_tool && !on(game, "harvest_effective") {
        return None;
    }
    let tools = harvest_tools(game, block, !facts.requires_tool && facts.hardness == 0.0);
    if tools.is_empty() {
        return None;
    }
    let mut els: Vec<El> = tools.iter().map(|t| Line::small(t)).collect();
    if facts.requires_tool {
        let ok = game.res.items().correct_for_drops(game.held(), block);
        els.push(El::Mark(if ok { "§a✔".to_owned() } else { "§c✕".to_owned() }));
    }
    Some(els)
}

/// Jade's own client lines (VanillaPlugin's client components): growth, redstone, note blocks, TNT, beehives'
/// honey, enchanting power, heads (blocks); health, armour, growth, breeding, owner, horse stats, villagers,
/// item frames, armour stands, paintings, items (mobs and entities).
fn vanilla(game: &Game<'_>, t: &Target, details: bool, lines: &mut Vec<Line>) {
    match t {
        Target::Block { pos, block } => {
            let id = game.world.block(pos[0], pos[1], pos[2]);
            let Some(state) = game.pack.assets.registry.state(id) else { return };
            let prop = |k: &str| state.properties.get(k).cloned();
            let reg = &game.pack.assets.registry;
            // CropProgressProvider: crops, stems, sweet berries, nether wart, cocoa (their `age`).
            if let Some(age) = prop("age").and_then(|a| a.parse::<u32>().ok()).filter(|_| on(game, "crop_progress")) {
                let most = reg.property_values(block, "age").iter().filter_map(|v| v.parse::<u32>().ok()).max().unwrap_or(7).max(1);
                let crop = block.ends_with("_crop") || matches!(block.as_str(), "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" | "minecraft:beetroots"
                    | "minecraft:melon_stem" | "minecraft:pumpkin_stem" | "minecraft:sweet_berry_bush" | "minecraft:nether_wart" | "minecraft:cocoa"
                    | "minecraft:torchflower_crop" | "minecraft:pitcher_crop");
                if crop {
                    lines.push(Line::text(if age >= most { format!("§a{}", tr("tooltip.jade.crop_mature", "Mature")) } else { trf("tooltip.jade.crop_growth", "Growth: %s", &[&format!("§f{}%", age * 100 / most)]) }));
                }
            }
            // RedstoneProvider: a lever's state, a repeater's delay, a comparator's mode, redstone power.
            let redstone = on(game, "redstone");
            if redstone && block == "minecraft:lever" {
                let on = prop("powered").as_deref() == Some("true");
                let state = if on { format!("§a{}", tr("tooltip.jade.state_on", "On")) } else { format!("§c{}", tr("tooltip.jade.state_off", "Off")) };
                lines.push(Line::text(trf("tooltip.jade.state", "State: %s", &[&state])));
            }
            if redstone && block == "minecraft:repeater" {
                lines.push(Line::text(trf("tooltip.jade.delay", "Delay: %d", &[&format!("§f{}", prop("delay").unwrap_or_default())])));
            }
            if redstone && block == "minecraft:comparator" {
                let m = if prop("mode").as_deref() == Some("subtract") { tr("tooltip.jade.mode_subtractor", "Subtractor") } else { tr("tooltip.jade.mode_comparator", "Comparator") };
                lines.push(Line::text(trf("tooltip.jade.mode", "Mode: %s", &[&format!("§f{m}")])));
            }
            if let Some(power) = prop("power").filter(|_| redstone)
                && !matches!(block.as_str(), "minecraft:comparator" | "minecraft:lever" | "minecraft:repeater")
            {
                lines.push(Line::text(trf("tooltip.jade.power", "Power: %d", &[&format!("§f{power}")])));
            }
            // NoteBlockProvider: the instrument and the note.
            if block == "minecraft:note_block" && on(game, "note_block") {
                let inst = prop("instrument").unwrap_or_default();
                let name = match inst.as_str() {
                    "basedrum" => "Bass Drum", "snare" => "Snare Drum", "hat" => "Clicks and Sticks", "bass" => "Bass", "flute" => "Flute",
                    "bell" => "Bells", "guitar" => "Guitar", "chime" => "Chimes", "xylophone" => "Xylophone", "iron_xylophone" => "Vibraphone",
                    "cow_bell" => "Cow Bell", "didgeridoo" => "Didgeridoo", "bit" => "Square Wave", "banjo" => "Banjo", "pling" => "Electric Piano",
                    "zombie" => "Zombie", "skeleton" => "Skeleton", "creeper" => "Creeper", "dragon" => "Dragon", "wither_skeleton" => "Wither Skeleton",
                    "piglin" => "Piglin", "custom_head" => "Custom Head", _ => "Harp",
                };
                let name = tr(&format!("jade.instrument.{inst}"), name);
                let note: usize = prop("note").and_then(|n| n.parse().ok()).unwrap_or(0);
                const NOTES: [&str; 12] = ["F♯", "G", "G♯", "A", "A♯", "B", "C", "C♯", "D", "D♯", "E", "F"];
                let octave = if note < 6 { 1 } else if note < 18 { 2 } else { 3 };
                lines.push(Line::text(format!("§f{name} - {}{octave}", NOTES[note % 12])));
            }
            // TNTStabilityProvider.
            if block == "minecraft:tnt" && prop("unstable").as_deref() == Some("true") && on(game, "tnt_stability") {
                lines.push(Line::text(format!("§c{}", tr("jade.tnt.unstable", "Unstable"))));
            }
            // BeehiveProvider.Client: the honey level.
            if let Some(h) = prop("honey_level").filter(|_| on(game, "beehive")) {
                lines.push(Line::text(trf("jade.beehive.honey", "Honey: %s", &[&format!("§f{}", trf("jade.fraction", "%d / %d", &[&h.to_string(), "5"]))])));
            }
            // EnchantmentPowerProvider: a bookshelf counts one for an enchanting table.
            if matches!(block.as_str(), "minecraft:bookshelf" | "minecraft:chiseled_bookshelf") && details && on(game, "enchantment_power") {
                lines.push(Line::text(trf("jade.ench_power", "Ench Power: %s", &["§f1"])));
            }
            // TotalEnchantmentPowerProvider: the bookshelves an enchanting table reads.
            // SignProvider: the side the player faces, line by line.
            if access(game, "access_sign")
                && let Some(horizon_client::containers::BlockEntity::Sign(sign)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
            {
                let front = sign_front_faces(game, *pos);
                lines.push(Line::text(if front { tr("jade.access.sign.front", "Front:") } else { tr("jade.access.sign.back", "Back:") }));
                let text = if front { &sign.front } else { &sign.back };
                const LINES: [(&str, &str); 4] = [("jade.access.sign.line1", "First line: %s"), ("jade.access.sign.line2", "Second line: %s"), ("jade.access.sign.line3", "Third line: %s"), ("jade.access.sign.line4", "Fourth line: %s")];
                for (i, line) in text.lines.iter().enumerate() {
                    lines.push(Line::text(if details { trf(LINES[i].0, LINES[i].1, &[line]) } else { format!("§f{line}") }));
                }
            }
            if access(game, "access_block") {
                // BlockDetailsBodyProvider.
                let dir_name = |d: &str| {
                    let mut c = d.chars();
                    let english = c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str());
                    tr(&format!("jade.{d}"), &english)
                };
                if block == "minecraft:redstone_wire" {
                    let sides: Vec<String> = ["north", "south", "west", "east"].into_iter().filter(|d| prop(*d).is_some_and(|v| v != "none")).map(dir_name).collect();
                    lines.push(Line::text(if sides.is_empty() { tr("jade.access.block.redstone_wire.dot", "Shape: Dot") } else { trf("jade.access.block.redstone_wire", "Shape: %s", &[&sides.join(", ")]) }));
                } else if !(block.ends_with("campfire") || block == "minecraft:decorated_pot" || matches!(block.as_str(), "minecraft:small_dripleaf" | "minecraft:big_dripleaf" | "minecraft:big_dripleaf_stem" | "minecraft:pink_petals" | "minecraft:wildflowers")) {
                    if let Some(shape) = prop("shape").filter(|_| block.ends_with("rail")) {
                        let words = match shape.as_str() {
                            "north_south" => "North-South", "east_west" => "East-West", "ascending_east" => "Ascending East", "ascending_west" => "Ascending West",
                            "ascending_north" => "Ascending North", "ascending_south" => "Ascending South", "south_east" => "South-East", "south_west" => "South-West",
                            "north_west" => "North-West", _ => "North-East",
                        };
                        lines.push(Line::text(tr(&format!("jade.access.block.rail.{shape}"), words)));
                    }
                    if let Some(facing) = prop("facing") {
                        lines.push(Line::text(trf("jade.access.block.facing", "Facing: %s", &[&dir_name(&facing)])));
                    }
                }
                // BlockAmountProvider.
                if let Some(n) = prop("pickles").or_else(|| prop("candles")).or_else(|| prop("eggs")) {
                    lines.push(Line::text(trf("jade.access.block.amount", "Amount: {0}", &[&n.to_string()])));
                }
                if let Some(n) = prop("bites") {
                    lines.push(Line::text(trf("jade.access.block.bites", "Bites: {0}", &[&n.to_string()])));
                }
                if let Some(n) = prop("layers") {
                    lines.push(Line::text(trf("jade.access.block.layers", "Layers: {0}", &[&n.to_string()])));
                }
                if let Some(n) = prop("level").filter(|_| block.ends_with("_cauldron")) {
                    lines.push(Line::text(trf("jade.access.block.level", "Level: {0}", &[&n.to_string()])));
                }
            }
            if block == "minecraft:enchanting_table" && on(game, "total_enchantment_power") {
                let mut power = 0;
                for dx in -2..=2i32 {
                    for dz in -2..=2i32 {
                        if dx.abs() != 2 && dz.abs() != 2 {
                            continue;
                        }
                        for dy in 0..=1 {
                            let b = game.world.block(pos[0] + dx, pos[1] + dy, pos[2] + dz);
                            if game.pack.assets.registry.state(b).is_some_and(|s| s.name.as_str() == "minecraft:bookshelf") {
                                power += 1;
                            }
                        }
                    }
                }
                lines.push(Line::text(trf("jade.ench_power", "Ench Power: %s", &[&format!("§f{}", power.min(15))])));
            }
        }
        Target::Mob { index } => {
            let Some(m) = game.mobs.get(*index) else { return };
            let kind = m.kind.id.as_ref();
            // EntityHealthAndArmorProvider (not armour stands, not the creaking): the absorption and the armour come from
            // the server (`apply_stats`).
            if !matches!(kind, "minecraft:armor_stand" | "minecraft:creaking") && on(game, "entity_health") {
                // The mob's own max_health attribute (a horse rolls its own).
                let max = m.attrs.max_health.max(1.0);
                lines.push(Line::new().with(El::Hearts { health: m.health.max(0.0), max, absorb: m.absorption.max(0.0) }));
            }
            if m.kind.armor > 0.0 && on(game, "entity_armor") {
                lines.push(Line::new().with(El::Armor(m.kind.armor)));
            }
            // EntityVariantProvider: what a mob's variant is.
            if access(game, "access_variant")
                && let Some(name) = variant_name(m)
            {
                lines.push(Line::text(variant_line(m, &name)));
                // Horse.getMarkings.
                if m.kind.id == "minecraft:horse" {
                    const MARKINGS: [&str; 5] = ["none", "white", "white_field", "white_dots", "black_dots"];
                    let marking = MARKINGS[usize::from(m.extra.variant2) % MARKINGS.len()];
                    let words = tr(&format!("jade.access.entity.horse_markings.{marking}"), &marking.replace('_', " "));
                    lines.push(Line::text(tr("jade.access.entity.horse_markings", "Markings: %s").replace("%s", &words)));
                }
            }
            // HeldItemProvider: both hands.
            if access(game, "access_held") {
                if let Some(held) = m.extra.main_hand.as_deref() {
                    lines.push(Line::text(trf("jade.access.held_item.main", "Main Hand: %s", &[&item_name(game, held)])));
                }
                if let Some(held) = m.extra.off_hand.as_deref() {
                    lines.push(Line::text(trf("jade.access.held_item.off", "Off Hand: %s", &[&item_name(game, held)])));
                }
            }
            // EntityDetailsBodyProvider: pose, passengers, leads.
            if access(game, "access_entity") {
                let pose = format!("jade.access.entity.pose.{}", pose_id(m));
                if horizon_client::lang::has(&pose) {
                    lines.push(Line::text(tr("jade.access.entity.pose", "Pose: %s").replace("%s", &tr(&pose, ""))));
                }
                if m.extra.rider.is_some() {
                    lines.push(Line::text(trf("jade.access.entity.passengers", "Passengers: {0}", &["1"])));
                }
                use horizon_client::mobs::leash::Holder;
                let mob_name = |id: u32| game.mobs.iter().find(|o| o.id == id).map(|o| o.custom_name.clone().unwrap_or_else(|| {
                    let short = o.kind.id.trim_start_matches("minecraft:");
                    game.translate(&format!("entity.minecraft.{short}"), short)
                }));
                match &m.extra.leash {
                    Some(Holder::Knot(p)) => lines.push(Line::text(trf("jade.access.entity.leashed_to", "Leashed to: %s", &[&format!("{} {} {}", p[0], p[1], p[2])]))),
                    Some(Holder::Mob(id)) => {
                        if let Some(name) = mob_name(*id) {
                            lines.push(Line::text(trf("jade.access.entity.leashed_to", "Leashed to: %s", &[&name])));
                        }
                    }
                    Some(Holder::Player) => lines.push(Line::text(trf("jade.access.entity.leashed_to", "Leashed to: %s", &["Player"]))),
                    _ => {}
                }
                let leading: Vec<String> = game.mobs.iter().filter(|o| o.extra.leash == Some(Holder::Mob(m.id))).filter_map(|o| mob_name(o.id)).collect();
                if !leading.is_empty() {
                    lines.push(Line::text(trf("jade.access.entity.is_leashing", "is Leashing: %s", &[&leading.join(", ")])));
                }
            }
            // VillagerProfessionProvider: the profession, and its level for one with a job.
            if matches!(kind, "minecraft:villager" | "minecraft:zombie_villager") && on(game, "villager_profession") {
                const PROFESSIONS: [&str; 15] = ["none", "armorer", "butcher", "cartographer", "cleric", "farmer", "fisherman", "fletcher",
                    "leatherworker", "librarian", "mason", "nitwit", "shepherd", "toolsmith", "weaponsmith"];
                // 15 is AE2's Fluix Researcher (InitVillager.PROFESSION).
                let prof = if m.extra.variant2 == 15 { "fluix_researcher" } else { PROFESSIONS[m.extra.variant2 as usize % PROFESSIONS.len()] };
                let mut text = game.translate(&format!("entity.minecraft.villager.{prof}"), prof);
                let level = m.extra.variant3;
                if !matches!(prof, "none" | "nitwit") && (1..=5).contains(&level) {
                    const LEVELS: [&str; 5] = ["Novice", "Apprentice", "Journeyman", "Expert", "Master"];
                    text = format!("{text} - {}", game.translate(&format!("merchant.level.{level}"), LEVELS[level as usize - 1]));
                }
                lines.push(Line::text(format!("§f{text}")));
            }
            // PetArmorProvider: an OwnableEntity's or happy ghast's body armour ("Show Damageable" leaves out what wears no
            // durability).
            if on(game, "pet_armor")
                && matches!(kind, "minecraft:wolf" | "minecraft:cat" | "minecraft:parrot" | "minecraft:horse" | "minecraft:donkey" | "minecraft:mule"
                    | "minecraft:skeleton_horse" | "minecraft:zombie_horse" | "minecraft:llama" | "minecraft:trader_llama" | "minecraft:camel"
                    | "minecraft:camel_husk" | "minecraft:nautilus" | "minecraft:zombie_nautilus" | "minecraft:happy_ghast")
                && let Some(armor) = m.extra.body.as_deref()
                && (opt(game, "pet_armor_mode") != "damageable" || game.res.items().get(armor).is_some_and(|i| i.max_damage > 0))
            {
                lines.push(Line::new().with(Line::small(armor)).with(El::Text(format!("§f{}", item_name(game, armor)))));
            }
            // SulfurCubeProvider: what a sulfur cube swallowed.
            if kind == "minecraft:sulfur_cube"
                && on(game, "sulfur_cube")
                && let Some(block) = m.extra.head.as_deref()
            {
                lines.push(Line::new().with(Line::small(block)).with(El::Text(format!("§f{}", item_name(game, block)))));
            }
            // StatusEffectsProvider: each effect and its time, by category (good green, bad red).
            let limit = num(game, "effects_limit") as usize;
            for (id, ticks, amp) in m.extra.effects.iter().filter(|_| config(game).effects).take(limit) {
                let short = id.trim_start_matches("minecraft:");
                let mut name = game.translate(&format!("effect.minecraft.{short}"), short);
                if (1..=9).contains(amp) {
                    name = format!("{name} {}", game.translate(&format!("potion.potency.{amp}"), &roman(*amp + 1)));
                }
                let time = if *ticks < 0 { tr("effect.duration.infinite", "∞") } else { tick_duration(i64::from(*ticks)) };
                let color = match horizon_client::brewing::effect(short).map(|e| e.category) {
                    Some(horizon_client::brewing::Category::Harmful) => "§c",
                    Some(horizon_client::brewing::Category::Beneficial) => "§a",
                    _ => "§f",
                };
                lines.push(Line::text(format!("{color}{}", trf("jade.potion", "%s (%s)", &[&name, &time]))));
            }
            // AnimalOwnerProvider: a tamed animal's owner (a player in the world, or the uuid).
            if let Some(owner) = m.extra.owner.filter(|_| on(game, "animal_owner")) {
                let uuid = ((owner[0] as u32 as u128) << 96) | ((owner[1] as u32 as u128) << 64) | ((owner[2] as u32 as u128) << 32) | (owner[3] as u32 as u128);
                let name = game.native_server_players().into_iter().find(|p| p.uuid == uuid).map(|p| p.name).unwrap_or_else(|| format!("{uuid:032x}"));
                lines.push(Line::text(trf("jade.owner", "Owner: %s", &[&format!("§f{name}")])));
            }
        }
        // ItemFrameProvider: the framed item's name.
        Target::Frame { index } => {
            if let Some(stack) = game.item_frames.get(*index).and_then(|f| f.item.as_ref()).filter(|_| on(game, "item_frame")) {
                let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                lines.push(Line::text(format!("§f{name}")));
            }
        }
        // PaintingProvider: the variant's title (yellow) and author (grey).
        Target::Painting { index } => {
            if let Some(p) = game.paintings.get(*index).filter(|_| on(game, "painting")) {
                let (ns, v) = p.variant.split_once(':').unwrap_or(("minecraft", &p.variant));
                let title = game.translate(&format!("painting.{ns}.{v}.title"), "");
                let author = game.translate(&format!("painting.{ns}.{v}.author"), "");
                if !title.is_empty() {
                    lines.push(Line::text(format!("§e{title}")));
                }
                if !author.is_empty() {
                    lines.push(Line::text(format!("§7{author}")));
                }
            }
        }
        // ArmorStandProvider: each worn or held item, small, with its name.
        Target::Stand { index } => {
            if let Some(a) = game.armor_stands.get(*index).filter(|_| on(game, "armor_stand")) {
                for stack in a.equipment.iter().rev().flatten() {
                    let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                    lines.push(Line::new().with(Line::small(&stack.item)).with(El::Text(format!("§f{name}"))));
                }
            }
        }
        Target::Falling { .. } | Target::Display { .. } => {}
        // Their contents are the server's (`entity_request`): a client doesn't get a vehicle's items.
        Target::Cart { .. } | Target::Boat { .. } => {}
        // ItemTooltipProvider: the dropped stack's tooltip under its name.
        Target::Item { index } => {
            if let Some(e) = game.dropped.get(*index).filter(|_| on(game, "item_tooltip")) {
                let mut tip = game.stack_tooltip(&e.stack);
                if on(game, "item_mod_name") {
                    tip.pop();
                }
                for (t, _) in tip.into_iter().skip(1) {
                    lines.push(Line::text(format!("§7{}", t.plain())));
                }
            }
        }
    }
}

/// SignBlockEntity.isFacingFrontText: the side of the sign towards the player.
fn sign_front_faces(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let Some((eye, _)) = game.aim() else { return true };
    let id = game.world.block(pos[0], pos[1], pos[2]);
    let prop = |k: &str| game.pack.assets.registry.state(id).and_then(|s| s.properties.get(k).cloned());
    // SignBlock.getYRotationDegrees: its rotation, or its wall's facing.
    let rotation = match prop("rotation").and_then(|r| r.parse::<f32>().ok()) {
        Some(r) => r * 22.5,
        None => match prop("facing").as_deref() {
            Some("north") => 180.0,
            Some("west") => 90.0,
            Some("east") => 270.0,
            _ => 0.0,
        },
    };
    let (dx, dz) = (eye[0] - (f64::from(pos[0]) + 0.5), eye[2] - (f64::from(pos[2]) + 0.5));
    // Mth.degreesDifferenceAbs(yaw towards the player, the front's normal) <= 90.
    let toward = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    let diff = (toward - rotation + 540.0).rem_euclid(360.0) - 180.0;
    diff.abs() <= 90.0
}

/// EntityVariantHelper.getVariantName's `Left`: a dye colour by its id (`jade.access.entity.<color>`).
const DYES: [&str; 16] = ["White", "Orange", "Magenta", "Light Blue", "Yellow", "Lime", "Pink", "Gray", "Light Gray", "Cyan", "Purple", "Blue", "Brown", "Green", "Red", "Black"];

/// EntityVariantProvider: a mob's variant in words, for the kinds whose number has one meaning here.
fn variant_name(m: &horizon_client::mobs::Mob) -> Option<String> {
    let v = usize::from(m.extra.variant);
    let of = |names: &[&str]| names.get(v).map(|n| (*n).to_owned());
    match m.kind.id.as_ref() {
        "minecraft:axolotl" => of(&["lucy", "wild", "gold", "cyan", "blue"]),
        "minecraft:rabbit" if v == 99 => Some("evil".to_owned()),
        "minecraft:rabbit" => of(&["brown", "white", "black", "white_splotched", "gold", "salt"]),
        "minecraft:parrot" => of(&["red_blue", "blue", "green", "yellow_blue", "gray"]),
        "minecraft:fox" => of(&["red", "snow"]),
        // The villager's biome, `villager/variant`; zombie villagers report theirs too.
        "minecraft:villager" | "minecraft:zombie_villager" => of(&horizon_client::mobs::render::VILLAGER_TYPES),
        // wolf, cat, frog, pig, cow, chicken, mooshroom, horse, llama, zombie nautilus: `<mob>/variant`.
        _ => horizon_client::mobs::finalize::variant_component(m).map(|(_, id)| id.trim_start_matches("minecraft:").to_owned()),
    }
}

/// EntityVariantProvider: `jade.access.entity.<mob>.<variant>` when a pack names it, else the name with spaces.
fn variant_line(m: &horizon_client::mobs::Mob, name: &str) -> String {
    let mob = m.kind.id.trim_start_matches("minecraft:");
    let words = tr(&format!("jade.access.entity.{mob}.{name}"), &name.replace(['.', '_'], " "));
    tr("jade.access.entity.variant", "Variant: %s").replace("%s", &words)
}

/// EntityDetailsBodyProvider.getPoseId: the pose, or what a mob's own state says in its place.
fn pose_id(m: &horizon_client::mobs::Mob) -> u32 {
    use horizon_client::mobs::{armadillo, axolotl, fox, panda};
    match m.kind.id.as_ref() {
        "minecraft:wolf" | "minecraft:cat" | "minecraft:parrot" | "minecraft:nautilus" if m.extra.sitting => return 10,
        "minecraft:panda" if panda::is_sitting(m) => return 10,
        "minecraft:camel" | "minecraft:camel_husk" if m.extra.camel_sitting => return 10,
        "minecraft:fox" if fox::flag(m, fox::SLEEPING) => return 2,
        "minecraft:fox" if fox::flag(m, fox::SITTING) => return 10,
        "minecraft:axolotl" if axolotl::is_playing_dead(m) => return 1000,
        "minecraft:armadillo" if armadillo::state(m) == armadillo::ROLLING => return 1001,
        _ => {}
    }
    u32::from(m.extra.synced_pose)
}

/// AccessibilityPlugin.replaceTitle: the patterns of `jade.access.*` the state or the mob calls for, in Jade's order.
fn access_title(game: &Game<'_>, t: &Target, mut title: String) -> String {
    let mut wrap = |pattern: &str| title = fill(pattern, &[&title]);
    match t {
        // BlockDetailsProvider.
        Target::Block { pos, block } if access(game, "access_block") => {
            let id = shown_state(game, *pos, block);
            let Some(state) = game.pack.assets.registry.state(id) else { return title };
            let prop = |k: &str| state.properties.get(k).cloned();
            let yes = |k: &str| prop(k).as_deref() == Some("true");
            if prop("open").is_some() && block != "minecraft:barrel" {
                wrap(&if yes("open") { tr("jade.access.block.door_open", "{0}") } else { tr("jade.access.block.door_closed", "Closed {0}") });
            }
            for (key, pattern) in [("waterlogged", "Waterlogged {0}"), ("lit", "Lit {0}"), ("inverted", "Inverted {0}"), ("eye", "{0} with Eye"), ("ominous", "Ominous {0}")] {
                if yes(key) {
                    wrap(&tr(&format!("jade.access.block.{key}"), pattern));
                }
            }
            if prop("moisture").as_deref() == Some("7") {
                wrap(&tr("jade.access.block.hydrated", "Hydrated {0}"));
            }
            for (key, lang, pattern) in [("locked", "jade.access.block.locked", "Locked {0}"), ("extended", "jade.access.block.extended", "Extended {0}"), ("can_summon", "jade.access.block.summonable", "Summonable {0}")] {
                if yes(key) {
                    wrap(&tr(lang, pattern));
                }
            }
            if block.ends_with("_stairs") && prop("half").as_deref() == Some("top") {
                wrap(&tr("jade.access.block.upside_down", "Upside-down {0}"));
            }
            match prop("hatch").as_deref() {
                Some("1") => wrap(&tr("jade.access.block.hatch.1", "Slightly Cracked {0}")),
                Some("2") => wrap(&tr("jade.access.block.hatch.2", "Cracked {0}")),
                _ => {}
            }
            if yes("powered") && (block == "minecraft:repeater" || block.ends_with("rail") || block.ends_with("_shelf")) {
                wrap(&tr("jade.access.block.powered", "Powered {0}"));
            }
            let active = prop("vault_state").as_deref() == Some("active")
                || prop("trial_spawner_state").as_deref() == Some("active")
                || (block == "minecraft:creaking_heart" && prop("creaking_heart_state").as_deref() != Some("uprooted"));
            if active {
                wrap(&tr("jade.access.block.active", "Active {0}"));
            }
            // WaxedProvider.BlockComponent.
            if on(game, "waxed")
                && let Some(horizon_client::containers::BlockEntity::Sign(sign)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
                && sign.waxed
            {
                wrap(&tr("jade.access.waxed", "Waxed {0}"));
            }
        }
        // EntityDetailsProvider.
        Target::Mob { index } if access(game, "access_entity") => {
            let Some(m) = game.mobs.get(*index) else { return title };
            match m.kind.id.as_ref() {
                "minecraft:creeper" if m.extra.charged => wrap(&tr("jade.access.creeper.powered", "Charged {0}")),
                "minecraft:wither" if m.health <= m.attrs.max_health / 2.0 => wrap(&tr("jade.access.wither.powered", "Powered {0}")),
                "minecraft:zombie_villager" if horizon_client::mobs::zombie_villager::cure(m).is_some() => wrap(&tr("jade.access.zombie_villager.curing", "Curing {0}")),
                "minecraft:goat" if m.extra.lost_horns == [true, true] => wrap(&tr("jade.access.goat.hornless", "Hornless {0}")),
                "minecraft:bee" => {
                    if horizon_client::mobs::bee::has_nectar(m) {
                        wrap(&tr("jade.access.bee.nectar", "Nectarean {0}"));
                    }
                    if m.extra.anger > 0 {
                        wrap(&tr("jade.access.entity.angry", "Angry {0}"));
                    }
                }
                // jade.access.slime.size.
                "minecraft:slime" | "minecraft:magma_cube" => wrap(&trf("jade.access.slime.size", "{1,choice,1#Small|2≤Medium|4≤Large|8≤Huge} {0}", &["{0}", &m.extra.size.to_string()])),
                "minecraft:copper_golem" => match m.extra.variant {
                    1 => wrap(&tr("jade.access.entity.exposed", "Exposed {0}")),
                    2 => wrap(&tr("jade.access.entity.weathered", "Weathered {0}")),
                    3 => wrap(&tr("jade.access.entity.oxidized", "Oxidized {0}")),
                    _ => {}
                },
                _ => {}
            }
            if m.is_baby() {
                wrap(&tr("jade.access.entity.baby", "Baby {0}"));
            }
            // CommonProxy.isShearable: `readyForShearing`, and a baby sheep or mooshroom isn't.
            match m.kind.id.as_ref() {
                "minecraft:copper_golem" => {
                    let shears_take = m.saddle_item().is_some_and(|a| game.res.items().item_tags.get("minecraft:shearable_from_copper_golem").is_some_and(|t| t.contains(&*a.item)));
                    if shears_take {
                        wrap(&tr("jade.access.entity.shearable", "Shearable {0}"));
                    }
                }
                "minecraft:sheep" | "minecraft:mooshroom" if m.is_baby() || m.extra.sheared => wrap(&tr("jade.access.entity.sheared", "Sheared {0}")),
                "minecraft:snow_golem" | "minecraft:bogged" if m.extra.sheared => wrap(&tr("jade.access.entity.sheared", "Sheared {0}")),
                _ => {}
            }
            if &*m.kind.id == "minecraft:sheep"
                && let Some(color) = DYES.get(usize::from(m.extra.color))
            {
                wrap(&tr(&format!("jade.access.entity.{}", color.to_lowercase().replace(' ', "_")), &format!("{color} {{0}}")));
            }
            if m.extra.saddled {
                wrap(&tr("jade.access.entity.saddled", "Saddled {0}"));
            }
        }
        _ => {}
    }
    // NpcDescriptionProvider: a mannequin's description after its name.
    if let Target::Mob { index } = t
        && access(game, "access_npc")
        && let Some(m) = game.mobs.get(*index)
        && &*m.kind.id == "minecraft:mannequin"
    {
        let description = match m.extra.description.component() {
            Some(horizon_world::nbt::Tag::String(j)) => j.to_string().ok(),
            _ => None,
        };
        let description = description.unwrap_or_else(|| game.translate("entity.minecraft.mannequin.label", "NPC"));
        title = trf("jade.access.npc_description", "%s (%s)", &[&title, &description]);
    }
    title
}

/// The owner's name on a player head (`SkullBlockEntity`'s custom name, or its profile: a name, or a compound's).
fn head_owner(game: &Game<'_>, pos: [i32; 3]) -> Option<String> {
    use horizon_world::nbt::{JavaString, Tag};
    let Some(horizon_client::containers::BlockEntity::Skull(s)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return None };
    match s.profile.as_ref()? {
        Tag::String(j) => j.to_string().ok(),
        Tag::Compound(c) => match c.get(&JavaString::from("name")) {
            Some(Tag::String(j)) => j.to_string().ok(),
            _ => None,
        },
        _ => None,
    }
    .filter(|n| !n.is_empty())
}

/// The information options, all off by default: BlockFaceProvider, CoordinatesProvider, DistanceProvider,
/// BlockStatesProvider, BlockPropertiesProvider, RegistryNameProvider.
fn info_lines(game: &Game<'_>, t: &Target, cfg: &Config, lines: &mut Vec<Line>) {
    let (eye, dir) = game.aim().unwrap_or(([0.0; 3], [0.0; 3]));
    let block = if let Target::Block { pos, block } = t { Some((*pos, block.as_str())) } else { None };
    if cfg.block_face
        && block.is_some()
        && let Some(hit) = game.clip_ray(eye, dir, 6.0)
        && let Some(first) = lines.first_mut()
    {
        const FACES: [([i32; 3], &str); 6] = [([0, -1, 0], "down"), ([0, 1, 0], "up"), ([0, 0, -1], "north"), ([0, 0, 1], "south"), ([-1, 0, 0], "west"), ([1, 0, 0], "east")];
        if let Some((_, name)) = FACES.iter().find(|(d, _)| *d == hit.face) {
            first.els.push(El::Text(format!("§f{}", trf("jade.blockFace", " (%s)", &[&tr(&format!("jade.{name}"), name)]))));
        }
    }
    // DistanceProvider, for blocks and entities: its position in Jade's three colours, and how far the crosshair's hit is.
    if cfg.coordinates || cfg.distance {
        let (hit_d, hit_at) = CLIENT.with_borrow(|c| (c.entity_hit.0, c.entity_hit.1));
        let at = match t {
            Target::Block { pos, .. } => *pos,
            Target::Mob { index } => game.mobs.get(*index).map_or(hit_at, |m| [m.position.x, m.position.y, m.position.z]).map(|v| v.floor() as i32),
            Target::Cart { index } => game.carts.get(*index).map_or(hit_at, |c| [c.position.x, c.position.y, c.position.z]).map(|v| v.floor() as i32),
            Target::Item { index } => game.dropped.get(*index).map_or(hit_at, |e| [e.position.x, e.position.y, e.position.z]).map(|v| v.floor() as i32),
            Target::Falling { index } => game.falling.get(*index).map_or(hit_at, |f| [f.position.x, f.position.y, f.position.z]).map(|v| v.floor() as i32),
            _ => hit_at.map(|v| v.floor() as i32),
        };
        let d = if let Target::Block { .. } = t {
            let p = CLIENT.with_borrow(|c| c.hit_point);
            (0..3).map(|i| (p[i] - eye[i]).powi(2)).sum::<f64>().sqrt()
        } else {
            hit_d
        };
        let d = format!("{:.1}", d);
        let d = d.strip_suffix(".0").unwrap_or(&d).to_owned();
        if cfg.coordinates {
            // Relative only while Ctrl is held (JadeUI.hasControlDown).
            let ctrl = game.physical_keys.contains(&winit::keyboard::KeyCode::ControlLeft) || game.physical_keys.contains(&winit::keyboard::KeyCode::ControlRight);
            let at = if on(game, "coordinates_rel") && ctrl { [at[0] - eye[0].floor() as i32, at[1] - eye[1].floor() as i32, at[2] - eye[2].floor() as i32] } else { at };
            let mut l = trf("jade.blockpos", "%s %s %s", &[&format!("§#EF9A9A{}", at[0]), &format!("§#A5D6A7{}", at[1]), &format!("§#90CAF9{}", at[2])]);
            if cfg.distance {
                l.push_str(&format!("§r{}", trf("jade.distance1", " [%s]", &[&d])));
            }
            lines.push(Line::text(l));
        } else {
            lines.push(Line::text(trf("jade.distance2", "Distance: %s", &[&d])));
        }
    }
    if let Some((pos, name)) = block {
        let id = shown_state(game, pos, name);
        if cfg.block_states
            && let Some(state) = game.pack.assets.registry.state(id)
        {
            let mut props: Vec<(String, String)> = state.properties.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            props.sort();
            for (k, v) in props {
                lines.push(Line::text(format!("§7{k}: §f{v}")));
            }
        }
        if cfg.block_properties {
            let facts = game.res.facts().get(id);
            lines.push(Line::text(trf("jade.block_destroy_time", "Destroy time: %s", &[&format!("§f{}", trim2(f64::from(facts.hardness)))])));
            lines.push(Line::text(trf("jade.block_explosion_resistance", "Explosion resistance: %s", &[&format!("§f{}", trim2(f64::from(facts.explosion_resistance)))])));
            lines.push(Line::text(format!("Friction: §f{}", trim2(f64::from(facts.friction)))));
            // Block.speedFactor and jumpFactor: the blocks that differ.
            let (speed, jump) = match name {
                "minecraft:soul_sand" => (0.4, 1.0),
                "minecraft:honey_block" => (0.4, 0.5),
                _ => (1.0, 1.0),
            };
            lines.push(Line::text(trf("jade.block_speed_factor", "Speed factor: %s", &[&format!("§f{}", trim2(speed))])));
            lines.push(Line::text(trf("jade.block_jump_factor", "Jump factor: %s", &[&format!("§f{}", trim2(jump))])));
            let fire = |odds: &str| -> Option<i64> {
                BEHAVIOURS.with_borrow_mut(|b| {
                    let v = b.get_or_insert_with(|| {
                        std::fs::read_to_string(game.options.resources.join("block-behaviours.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
                    });
                    v.get("fire")?.get(odds)?.get(name)?.as_i64()
                })
            };
            if let Some(n) = fire("igniteOdds") {
                lines.push(Line::text(trf("jade.block_ignite_odds", "Ignite odds: %s", &[&format!("§f{n}")])));
            }
            if let Some(n) = fire("burnOdds") {
                lines.push(Line::text(trf("jade.block_burn_odds", "Burn odds: %s", &[&format!("§f{n}")])));
            }
        }
        // RegistryNameProvider.ForBlock, Advanced Tooltips: only with the game's F3+H on. The special ids only with it shown.
        let show = match opt(game, "registry_name").as_str() {
            "advanced" => game.settings.file.bool("advancedItemTooltips"),
            "false" => false,
            _ => true,
        };
        if show {
            lines.push(Line::text(format!("§8{name}")));
            if on(game, "registry_special") {
                let key = (pos[0], pos[1], pos[2]);
                if let Some(be) = game.world.block_entities.get(&key).filter(|_| &*game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).name == name) {
                    let tag = horizon_client::save::encode_block_entity(key, be, name);
                    if let Some(horizon_world::nbt::Tag::String(j)) = tag.get("id")
                        && let Ok(type_id) = j.to_string()
                    {
                        lines.push(Line::text(format!("§8{}", trf("config.jade.plugin_jade.registry_name.special.block_entity_type", "BlockEntity: %s", &[&type_id]))));
                    }
                }
                // FluidState: a source or a flowing fluid, or a waterlogged block's water.
                let facts = game.res.facts().get(id);
                let level = game.pack.assets.registry.state(id).and_then(|s| s.properties.get("level").cloned());
                let flowing = level.as_deref().is_some_and(|l| l != "0");
                let fluid = match name {
                    "minecraft:water" if flowing => Some("minecraft:flowing_water"),
                    "minecraft:lava" if flowing => Some("minecraft:flowing_lava"),
                    "minecraft:water" => Some("minecraft:water"),
                    "minecraft:lava" => Some("minecraft:lava"),
                    _ if facts.waterlogged => Some("minecraft:water"),
                    _ => None,
                };
                if let Some(fluid) = fluid {
                    lines.push(Line::text(format!("§8{}", trf("config.jade.plugin_jade.registry_name.special.fluid", "Fluid: %s", &[fluid]))));
                }
                // PoiTypes.forState.
                if let Some((poi, _)) = game.pack.assets.registry.state(id).and_then(horizon_client::save::poi_type) {
                    lines.push(Line::text(format!("§8{}", trf("config.jade.plugin_jade.registry_name.special.poi", "POI: %s", &[&poi.to_string()]))));
                }
            }
        }
    } else {
        // RegistryNameProvider.ForEntity.
        let show = match opt(game, "registry_name").as_str() {
            "advanced" => game.settings.file.bool("advancedItemTooltips"),
            "false" => false,
            _ => true,
        };
        if show {
            let id = match t {
                Target::Mob { index } => game.mobs.get(*index).map(|m| m.kind.id.to_string()),
                Target::Frame { index } => game.item_frames.get(*index).map(|f| f.kind().to_owned()),
                Target::Painting { .. } => Some("minecraft:painting".to_owned()),
                Target::Stand { .. } => Some("minecraft:armor_stand".to_owned()),
                Target::Item { .. } => Some("minecraft:item".to_owned()),
                Target::Falling { .. } => Some("minecraft:falling_block".to_owned()),
                Target::Cart { index } => game.carts.get(*index).map(|c| c.kind.id().to_owned()),
                Target::Boat { index } => game.boats.get(*index).map(|b| format!("minecraft:{}_{}boat", b.wood, if b.chest { "chest_" } else { "" })),
                Target::Display { id } => game.text_displays.get(id).map(|d| if d.hz_jade_block().is_some() { "minecraft:block_display".to_owned() } else { "minecraft:item_display".to_owned() }),
                Target::Block { .. } => None,
            };
            if let Some(id) = id {
                lines.push(Line::text(format!("§8{id}")));
            }
            if on(game, "registry_special")
                && let Target::Painting { index } = t
                && let Some(p) = game.paintings.get(*index)
            {
                lines.push(Line::text(format!("§8{}", trf("config.jade.plugin_jade.registry_name.special.painting", "Painting: %s", &[&p.variant.to_string()]))));
            }
        }
    }
}

/// DisplayHelper.dfCommas: at most two decimals.
pub fn trim2(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// StringUtil.formatTickDuration: m:ss (h:mm:ss past an hour).
fn tick_duration(ticks: i64) -> String {
    let s = ticks / 20;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

fn roman(n: i32) -> String {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get((n - 1).max(0) as usize).map_or_else(|| n.to_string(), |r| (*r).to_owned())
}

/// What a mod's block with no provider of its own counts as along: the ints `progress` and `max_progress` it saves.
fn saved_progress(m: &horizon_client::containers::ModBlockEntity) -> Option<f32> {
    use horizon_world::nbt::{JavaString, Tag};
    let int = |k: &str| match m.nbt.get(&JavaString::from(k)) {
        Some(Tag::Int(v)) => Some(*v),
        _ => None,
    };
    let (now, most) = (int("progress")?, int("max_progress")?);
    (most > 0 && now > 0).then(|| now as f32 / most as f32)
}

/// DisplayHelper.humanReadableNumber: "1.5kFE".
fn human(number: i64, unit: &str) -> String {
    if number == 0 {
        return format!("0{unit}");
    }
    let mut v = number.unsigned_abs() as f64;
    let mut exp = (v.log10() as usize) / 3;
    exp = exp.min(7);
    v /= 1000f64.powi(exp as i32);
    let sign = if number < 0 { "-" } else { "" };
    let prefix = if exp == 0 { String::new() } else { "kMGTPEZ"[exp - 1..exp].to_owned() };
    format!("{sign}{}{prefix}{unit}", trim2(v))
}

/// EnergyStorageProvider's line in Energy Storage's Display Style (`CommonProxy.wrapEnergyStorage` through any side).
fn energy_lines(game: &Game<'_>, pos: [i32; 3], lines: &mut Vec<String>) {
    let dim = game.dim.id();
    let found = horizon_core::geometry::Direction::ALL.into_iter().find_map(|face| game.native_energy(dim, pos, face));
    let Some(energy) = found.filter(|e| e.capacity > 0) else { return };
    let (current, max) = (human(i64::from(energy.amount), "FE"), human(i64::from(energy.capacity), "FE"));
    lines.push(match opt(game, "energy_style").as_str() {
        "plain" => trf("jade.energy.text", "Energy: %s", &[&trf("jade.fe", "%s / %s", &[&current, &max])]),
        "icon" => encode(&Line::new().with(El::Energy).with(El::Text(trf("jade.fe", "%s / %s", &[&current, &max])))),
        _ => encode(&Line::new().with(El::Bar { frac: (energy.amount as f32 / energy.capacity as f32).clamp(0.0, 1.0), color: 0xB01010, text: trf("jade.fe", "%s / %s", &[&format!("§f{current}§r"), &max]) })),
    });
}

/// The stacks of a slot that holds what the player pointed at, if a shelf (ShelfProvider): its icon, name and the book's
/// enchantments in place of the storage rows.
fn shelf_hit(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<String>) {
    let tip = game.stack_tooltip(stack);
    let name = tip.first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
    lines.push(encode(&Line::new().with(El::Icon(stack.item.to_string()))));
    lines.push(format!("§f{name}"));
    if &*stack.item == "minecraft:enchanted_book" {
        let count = tip.len().saturating_sub(usize::from(on(game, "item_mod_name")));
        for (t, _) in tip.iter().take(count).skip(1) {
            lines.push(format!("§7{}", t.plain()));
        }
    }
}

/// The entity an id of a spawner's data shows (BaseSpawner.getOrCreateDisplayEntity's type), by name.
fn spawn_data_name(game: &Game<'_>, data: &horizon_client::mobs::spawner::SpawnData) -> Option<String> {
    let id = match data.entity.get(&horizon_world::nbt::JavaString::from("id")) {
        Some(horizon_world::nbt::Tag::String(t)) => t.to_string().ok()?,
        _ => return None,
    };
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id.as_str()));
    Some(game.translate(&format!("entity.{ns}.{path}"), path))
}

/// MobSpawnerProvider.appendTooltip: the target's title as "name (mob)".
fn spawner_title(game: &Game<'_>, s: &horizon_client::mobs::spawner::Spawner) -> Option<String> {
    use horizon_client_render::chunk::block_entity::Next;
    let data = match &s.next {
        Some(Next::Own(d)) => Some(d),
        Some(Next::Potential(i)) => s.potentials.get(*i).map(|(d, _)| d),
        None => s.potentials.first().map(|(d, _)| d),
    }?;
    let name = spawn_data_name(game, data)?;
    Some(encode(&Line::new().with(El::Title(trf("jade.spawner", "%s (%s)", &["{0}", &name])))))
}

/// ItemStorageProvider's check on a container whose loot isn't rolled out to a player yet (RandomizableContainer's loot
/// table: Horizon rolls it as the chunk loads and keeps the roll, so "still rolled" stands for it), with its table.
fn unopened_loot(game: &Game<'_>, key: (i32, i32, i32), be: &horizon_client::containers::BlockEntity, block: &str) -> Option<(String, Option<i64>)> {
    let rolled = game.world.chunks.get(&(key.0 >> 4, key.2 >> 4))?.saved.as_ref()?.rolled_loot.iter().find(|r| r.pos == key)?;
    let tag = horizon_client::save::encode_block_entity(key, be, block);
    (tag.get("Items") == Some(&rolled.items)).then(|| (rolled.table.clone(), rolled.seed))
}

/// The server's lines Jade itself gives (VanillaPlugin's server data, the universal storage providers):
/// furnaces, brewing stands, containers, campfires, jukeboxes, lecterns, comparators, hoppers, beehives,
/// command blocks, shelves, spawners, cauldrons, ender chests and mod machines. `slot` is the shelf slot the player
/// points at (ShelfProvider, -1 for none).
fn vanilla_server(game: &mut Game<'_>, player: u128, pos: [i32; 3], block: &str, details: bool, slot: i32, lines: &mut Vec<String>) {
    SERVER_ITEMS.set(on(game, "item_storage"));
    let fluids = on(game, "fluid_storage");
    use horizon_client::containers::BlockEntity as Be;
    let key = (pos[0], pos[1], pos[2]);
    let asking = game.native_server_players().into_iter().find(|p| p.uuid == player);
    // Creative or spectator: not kept out of what is locked or unrolled.
    let privileged = asking.as_ref().is_some_and(|p| matches!(p.game_mode, 1 | 3));
    // BodyProviderAdapter: AE2's blocks don't show Jade's fluid and energy lines.
    let ae2 = block.starts_with("ae2:");
    // RedstoneProvider: a comparator's output, a calibrated sculk sensor's input.
    if on(game, "redstone") {
        use horizon_core::position::BlockPos;
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        if block == "minecraft:comparator" {
            let n = game.with_level(|_, _, ticks| ticks.comparator_outputs.get(&at).copied()).unwrap_or(0);
            lines.push(trf("tooltip.jade.power", "Power: %d", &[&format!("§f{n}")]));
        } else if block == "minecraft:calibrated_sculk_sensor" {
            // CalibratedSculkSensorBlockEntity's back signal: `level.getSignal` behind it, as the sensor itself reads it.
            let n = game.with_level(|b, level, ticks| b.calibrated_back_signal(level, ticks, at));
            lines.push(tr("jade.input_signal", "Input: %d").replace("%d", &format!("§f{n}")));
        }
    }
    // The ender chest's contents are the asking player's own (ItemStorageProvider.Extension.getGroups).
    if block == "minecraft:ender_chest" {
        if let Ok(inv) = game.native_inventory(player, crate::game::native_inventories::NativeInventoryKind::Ender) {
            storage(game, details, lines, inv.slots.iter().flatten());
        }
        return;
    }
    // Progress on what a mod says (`register_progress`).
    let mut progress: Vec<(f32, Option<String>)> = Vec::new();
    if let Ok(providers) = PROGRESS_PROVIDERS.lock().map(|v| v.clone()) {
        for p in providers {
            progress.extend(p(game, pos, block));
        }
    }
    let Some(be) = game.world.block_entities.get(&key) else {
        if fluids {
            cauldron(block, game, pos, details, lines);
        }
        return;
    };
    // ItemStorageProvider.putData: the block's loot not rolled for anyone, or its lock, instead of its items.
    if let Some((table, seed)) = unopened_loot(game, key, be, block) {
        // BlockLootTableProvider: for game masters.
        if on(game, "loot_table") && asking.as_ref().is_some_and(|p| p.permission >= 2) {
            lines.push(trf("jade.lootTable", "Loot Table: %s", &[&table]));
            lines.push(trf("jade.lootTableSeed", "Seed: %s", &[&seed.unwrap_or(0).to_string()]));
        }
        lines.push(tr("jade.loot_not_generated", "§oWhat could it be?"));
        return;
    }
    if !privileged && game.world.block_lock(key).is_some() {
        lines.push(tr("jade.locked", "Inventory is locked"));
        return;
    }
    let item = |s: &Option<ItemStack>| s.as_ref().map_or_else(|| Line::item("minecraft:air", 0), |s| Line::item(&s.item, s.count));
    match be {
        // FurnaceProvider: input, fuel, the progress arrow, output.
        Be::Furnace(f) => {
            if f.items.iter().any(Option::is_some) && on(game, "furnace") {
                let frac = if f.cooking_total == 0 { 0.0 } else { f.cooking_timer as f32 / f.cooking_total as f32 };
                let l = Line::new().with(item(&f.items[0])).with(item(&f.items[1])).with(El::Arrow(frac)).with(item(&f.items[2]));
                lines.push(encode(&l));
            }
        }
        // BrewingStandProvider: the blaze powder left and the time to brew.
        Be::Brewing(_) if !on(game, "brewing_stand") => {}
        Be::Brewing(b) => {
            let mut l = Line::new().with(Line::small("minecraft:blaze_powder")).with(El::Text(format!("§f{}", b.fuel)));
            if b.brew_time > 0 {
                l = l.with(El::Space(5.0)).with(Line::small("minecraft:clock")).with(El::Text(format!("§f{}", seconds(i64::from(b.brew_time)))));
            }
            lines.push(encode(&l));
            storage(game, details, lines, b.items.iter().flatten());
        }
        // ItemStorageProvider: what a container holds, in rows of nine.
        Be::Chest(items) => {
            // HopperLockProvider: a locked hopper's title.
            if block == "minecraft:hopper" && game.native_block_property(pos, "enabled").as_deref() == Some("false") && on(game, "redstone") {
                lines.push(encode(&Line::new().with(El::Title(tr("jade.access.block.locked", "Locked {0}")))));
            }
            storage(game, details, lines, items.iter().flatten());
        }
        Be::Crafter(c) => storage(game, details, lines, c.items.iter().flatten()),
        // ShelfProvider: the slot pointed at, unless Show Details lists them all.
        Be::Shelf(s) => match s.items.get(slot.max(0) as usize).and_then(Option::as_ref).filter(|_| slot >= 0 && !details && on(game, "shelf")) {
            Some(stack) => shelf_hit(game, stack, lines),
            None => storage(game, details, lines, s.items.iter().flatten()),
        },
        Be::ChiseledBookshelf(s) => match s.items.get(slot.max(0) as usize).and_then(Option::as_ref).filter(|_| slot >= 0 && !details && on(game, "shelf")) {
            Some(stack) => shelf_hit(game, stack, lines),
            None => storage(game, details, lines, s.items.iter().flatten()),
        },
        // CampfireProvider: what cooks, and how far along.
        Be::Campfire(c) => {
            let mut l = Line::new();
            for (i, s) in c.items.iter().enumerate() {
                if let Some(s) = s {
                    l = l.with(Line::item(&s.item, s.count));
                    if c.total[i] > 0 {
                        l = l.with(El::Text(format!("§f{}", seconds(i64::from((c.total[i] - c.progress[i]).max(0))))));
                    }
                }
            }
            if !l.els.is_empty() {
                lines.push(encode(&l));
            }
        }
        // JukeboxProvider: the record.
        Be::Jukebox(_) if !on(game, "jukebox") => {}
        Be::Jukebox(j) => {
            if let Some(r) = j.record() {
                let name = item_name(game, &r.item);
                lines.push(encode(&Line::new().with(Line::small(&r.item)).with(El::Text(format!("§f{name}")))));
            } else {
                lines.push(format!("§7{}", tr("tooltip.jade.empty", "Empty")));
            }
        }
        // LecternProvider: the book.
        Be::Lectern(l) if on(game, "lectern") => {
            if let Some(b) = &l.book {
                let name = item_name(game, &b.item);
                lines.push(encode(&Line::new().with(Line::small(&b.item)).with(El::Text(format!("§f{name}")))));
            }
        }
        // BeehiveProvider: the bees inside.
        Be::Beehive(h) if on(game, "beehive") => lines.push(trf("jade.beehive.bees", "Bees: %s", &[&format!("§f{}", trf("jade.fraction", "%d / %d", &[&h.bees.len().to_string(), "3"]))])),
        // CommandBlockProvider: its command.
        // An operator's: commands can hold anything, and any player in reach can ask.
        Be::CommandBlock(c) if on(game, "command_block") => {
            let allowed = game.native_server_players().into_iter().any(|p| p.uuid == player && p.permission >= 2);
            if allowed && !c.command.is_empty() {
                lines.push(format!("§f{}", c.command));
            }
        }
        // MobSpawnerProvider: the title names what it spawns.
        Be::Spawner(s) if on(game, "mob_spawner") => lines.extend(spawner_title(game, s)),
        // A mod's machine: EnergyStorageProvider, ItemStorageProvider, FluidStorageProvider, ProgressProvider.
        Be::Mod(m) => {
            if on(game, "energy_storage") && !ae2 && (details || !on(game, "energy_detailed")) {
                energy_lines(game, pos, lines);
            }
            storage(game, details, lines, m.items.iter().flatten());
            // FluidStorageProvider: a mod block's tanks, each a bar.
            if !ae2 && fluids {
                let dim = game.dim.id().to_owned();
                let mut seen = Vec::new();
                for face in horizon_core::geometry::Direction::ALL {
                    for t in game.native_fluid_tanks(&dim, pos, face).unwrap_or_default() {
                        if !seen.contains(&t.tank) && (details || !on(game, "fluid_detailed")) {
                            seen.push(t.tank);
                            let name = t.fluid.as_deref().map(|f| fluid_name(game, f));
                            lines.push(fluid_line_styled(game, name.as_deref(), t.fluid.as_deref(), t.amount, t.capacity, details));
                        }
                    }
                }
            }
            if progress.is_empty()
                && let Some(frac) = saved_progress(m)
            {
                progress.push((frac, None));
            }
        }
        // MobSpawnerCooldownProvider: a trial spawner's cooldown.
        Be::TrialSpawner(t) if on(game, "mob_spawner") => {
            // MobSpawnerProvider.ForBlock: the mob it spawns next, in the title.
            let next = t.next_spawn_data.as_ref().or_else(|| {
                let slot = if t.is_ominous { &t.ominous } else { &t.normal };
                slot.config.spawn_potentials.first().map(|(d, _)| d)
            });
            if let Some(name) = next.and_then(|d| spawn_data_name(game, d)) {
                lines.push(encode(&Line::new().with(El::Title(trf("jade.spawner", "%s (%s)", &["{0}", &name])))));
            }
            let now = game.world.game_time;
            if t.cooldown_ends_at > now {
                lines.push(trf("jade.trial_spawner_cd", "Cooldown: %s", &[&format!("§f{}", seconds(t.cooldown_ends_at - now))]));
            }
        }
        _ => {}
    }
    // ProgressProvider.Client: each bar, its text above it, small.
    for (frac, text) in progress {
        if let Some(text) = text {
            lines.push(encode(&Line::new().with(El::Small(text))));
        }
        lines.push(encode(&Line::new().with(El::Bar { frac: frac.clamp(0.0, 1.0), color: 0x4FB54F, text: String::new() })));
    }
}

/// ItemStorageProvider: the stacks, the same items together, as many rows as its amounts allow: a few types by name
/// (Show Name Threshold, or always with Always Use List Mode), the rest as icons.
fn storage<'a>(game: &Game<'_>, details: bool, lines: &mut Vec<String>, stacks: impl Iterator<Item = &'a ItemStack>) {
    if !SERVER_ITEMS.get() {
        return;
    }
    let mut merged: Vec<(String, u32)> = Vec::new();
    for s in stacks {
        match merged.iter_mut().find(|(id, _)| *id == *s.item) {
            Some(e) => e.1 += s.count,
            None => merged.push((s.item.to_string(), s.count)),
        }
    }
    if on(game, "storage_sort") {
        merged.sort_by(|a, b| b.1.cmp(&a.1));
    }
    let amount = num(game, if details { "storage_detailed" } else { "storage_normal" }) as usize;
    let extra = merged.len().saturating_sub(amount);
    merged.truncate(amount);
    let threshold = num(game, "storage_names") as usize;
    if on(game, "always_list_mode") || merged.len() < threshold {
        // ItemStorageProvider.Client.append: "<count>× <name>".
        for (id, n) in &merged {
            let count = human(i64::from(*n), "");
            lines.push(encode(&Line::new().with(Line::small(id)).with(El::Text(format!("§f{count}× {}", item_name(game, id))))));
        }
    } else {
        for row in merged.chunks((num(game, "storage_per_line") as usize).max(1)) {
            let mut l = Line::new();
            for (id, n) in row {
                l = l.with(Line::item(id, *n));
            }
            lines.push(encode(&l));
        }
    }
    if extra > 0 {
        lines.push(format!("§7and {extra} more"));
    }
}

/// FluidStorageProvider for cauldrons: water, lava or powder snow by level.
fn cauldron(block: &str, game: &mut Game<'_>, pos: [i32; 3], details: bool, lines: &mut Vec<String>) {
    let level = |game: &mut Game<'_>| -> i32 { game.native_block_property(pos, "level").and_then(|l| l.parse().ok()).unwrap_or(0) };
    let (fluid, mb) = match block {
        "minecraft:water_cauldron" => ("minecraft:water", level(game) * 1000 / 3),
        "minecraft:lava_cauldron" => ("minecraft:lava", 1000),
        "minecraft:powder_snow_cauldron" => ("minecraft:powder_snow", level(game) * 1000 / 3),
        _ => return,
    };
    if !details && on(game, "fluid_detailed") {
        return;
    }
    let name = fluid_name(game, fluid);
    lines.push(fluid_line_styled(game, Some(&name), Some(fluid), mb, 1000, details));
}

/// A fluid's name (its block's).
fn fluid_name(game: &Game<'_>, fluid: &str) -> String {
    let (ns, path) = fluid.split_once(':').unwrap_or(("minecraft", fluid));
    game.translate(&format!("block.{ns}.{path}"), path)
}

/// Fluid Storage's Display Style: the bar, or its text with or without a bucket.
fn fluid_line_styled(game: &Game<'_>, name: Option<&str>, fluid: Option<&str>, mb: i32, capacity: i32, details: bool) -> String {
    let style = opt(game, "fluid_style");
    if style == "bar" {
        return fluid_line(name, fluid, mb, capacity, details);
    }
    let text = match name {
        Some(n) if mb > 0 => trf("jade.fluid", "%s %s", &[n, &format!("§f{} mB", commas(mb.into()))]),
        _ => trf("jade.fluid", "%s %s", &[&tr("jade.fluid.empty", "Empty"), &format!("{} mB", commas(capacity.into()))]),
    };
    let mut line = Line::new();
    if style == "icon" {
        line = line.with(Line::small("minecraft:bucket"));
    }
    encode(&line.with(El::Text(text)))
}

/// FluidStorageProvider's progress bar: "Water 4,000 mB" (with Show Details, "/ 16,000 mB"; "Empty" when empty).
fn fluid_line(name: Option<&str>, fluid: Option<&str>, mb: i32, capacity: i32, details: bool) -> String {
    let color = match fluid {
        Some("minecraft:water") => 0x3F76E4,
        Some("minecraft:lava") => 0xD96415,
        Some("minecraft:powder_snow") => 0xF0F0F0,
        _ => 0x8080FF,
    };
    let text = match name {
        Some(n) if mb > 0 => {
            if details {
                let amounts = trf("jade.fluid.with_capacity", "%s / %s", &[&format!("§f{} mB§r", commas(mb.into())), &format!("{} mB", commas(capacity.into()))]);
                trf("jade.fluid", "%s %s", &[n, &amounts])
            } else {
                trf("jade.fluid", "%s %s", &[n, &format!("§f{} mB", commas(mb.into()))])
            }
        }
        _ => trf("jade.fluid", "%s %s", &[&tr("jade.fluid.empty", "Empty"), &format!("{} mB", commas(capacity.into()))]),
    };
    encode(&Line::new().with(El::Bar { frac: mb as f32 / capacity.max(1) as f32, color, text }))
}

/// A whole number with thousands commas.
pub fn commas(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// A vanilla GUI sprite's region ("hud/heart/full"), as the atlas names it.
fn sprite(game: &Game<'_>, name: &str) -> Option<horizon_client::hud::Rect> {
    let atlas = &game.gui.atlas;
    // Theme.mapSprite: the theme's stand-in, when the atlas has it.
    if let Some(mapped) = crate::game::jade_theme::current(game).mapped_sprite(&format!("minecraft:{name}"))
        && let Some(r) = atlas.sprites.get(&mapped)
    {
        return Some(*r);
    }
    atlas.sprites.get(&format!("gui/sprites/{name}")).or_else(|| atlas.sprites.get(name)).copied()
}

/// Jade's progress_base.png and progress.png as (row, from, to, grey) pixel runs.
const ARROW_BASE: &[(u8, u8, u8, u32)] = &[
    (1, 14, 15, 0x8B), (2, 14, 16, 0x8B), (3, 14, 17, 0x8B), (4, 14, 18, 0x8B), (5, 14, 19, 0x8B), (6, 14, 20, 0x8B),
    (7, 0, 21, 0x8B), (8, 0, 22, 0x8B), (9, 0, 21, 0x8B), (10, 14, 20, 0x8B), (11, 14, 19, 0x8B), (12, 14, 18, 0x8B),
    (13, 14, 17, 0x8B), (14, 14, 16, 0x8B), (15, 14, 15, 0x8B),
];
const ARROW_DONE: &[(u8, u8, u8, u32)] = &[
    (0, 14, 15, 0xFF), (1, 14, 16, 0xFF), (2, 14, 17, 0xFF), (3, 14, 18, 0xFF), (4, 14, 19, 0xFF), (5, 14, 20, 0xFF),
    (6, 0, 21, 0xFF), (7, 0, 22, 0xFF), (8, 0, 21, 0xFF), (8, 21, 22, 0x68), (9, 0, 14, 0x68), (9, 14, 20, 0xFF),
    (9, 20, 21, 0x68), (10, 14, 19, 0xFF), (10, 19, 20, 0x68), (11, 14, 18, 0xFF), (11, 18, 19, 0x68), (12, 14, 17, 0xFF),
    (12, 17, 18, 0x68), (13, 14, 16, 0xFF), (13, 16, 17, 0x68), (14, 14, 15, 0xFF), (14, 15, 16, 0x68), (15, 14, 15, 0x68),
];

/// An element's size.
fn size(game: &Game<'_>, e: &El) -> (f32, f32) {
    match e {
        El::Text(t) => (game.native_text_width(&plain(t)), 9.0),
        El::Item { small: true, .. } => (10.0, 9.0),
        El::Item { .. } => (18.0, 18.0),
        El::Arrow(_) => (22.0, 16.0),
        El::Bar { text, .. } => ((game.native_text_width(&plain(text)) + 6.0).max(100.0), 12.0),
        El::Small(t) => (game.native_text_width(&plain(t)) * 0.75, 7.0),
        El::Hearts { max, absorb, .. } => {
            let icons = ((max.ceil() + absorb.ceil()) / 2.0).ceil().max(1.0);
            let line = num(game, "health_per_line").max(1.0) as f32;
            if icons * 2.0 > num(game, "health_max") as f32 || on(game, "health_fractions") {
                (game.native_text_width(&format!("{:.0}/{:.0}", 0, max)) + 10.0, 9.0)
            } else {
                let per = icons.min(line);
                let rows = (icons / line).ceil();
                (8.0 * per + 1.0, 5.0 + 4.0 * rows)
            }
        }
        El::Armor(a) => {
            if *a > num(game, "armor_max") as f32 {
                (game.native_text_width(&format!("{a:.0}")) + 10.0, 9.0)
            } else {
                let icons = (a / 2.0).ceil();
                (8.0 * icons.min(10.0) + 1.0, 5.0 + 4.0 * (icons / 10.0).ceil())
            }
        }
        El::Space(w) => (*w, 0.0),
        El::Energy => (ENERGY_ICON + 1.0, ENERGY_ICON),
        El::Mark(_) | El::Icon(_) | El::Badge | El::Title(_) | El::Stats { .. } => (0.0, 0.0),
    }
}

fn draw_el(game: &Game<'_>, batch: &mut HudBatch, e: &El, x: f32, y: f32, h: f32, look: &crate::game::jade_theme::TextLook) {
    let atlas = &game.gui.atlas;
    let white = horizon_client::hud::rgba(255, 255, 255, 255);
    let (_, eh) = size(game, e);
    let y = y + ((h - eh) / 2.0).floor();
    match e {
        // TextElementImpl: ColorPalette.normal under what has no colour of its own, TextSetting.shadow.
        El::Text(t) => {
            batch.text_styled(atlas, &rich(&look.map(t)), x, y, look.base, look.shadow);
        }
        El::Item { id, count, small } => {
            if id == "minecraft:air" {
                return;
            }
            if *small {
                game.native_item(batch, id, [x, y], 8.0);
            } else {
                game.native_item(batch, id, [x + 1.0, y + 1.0], 16.0);
                if *count > 1 {
                    let s = if *count >= 10_000 { format!("{}k", count / 1000) } else { count.to_string() };
                    let w = game.native_text_width(&s);
                    batch.text_styled(atlas, &horizon_client::text::Text::legacy(&s), x + 18.0 - w, y + 10.0, look.amount, look.shadow);
                }
            }
        }
        // JadeUI.progressArrow: jade:progress_base (22x16), then jade:progress clipped to the done part.
        El::Arrow(f) => {
            for &(row, x0, x1, c) in ARROW_BASE {
                game.native_fill(batch, [x + x0 as f32, y + row as f32, (x1 - x0) as f32, 1.0], 0xFF00_0000 | c * 0x010101);
            }
            let done = (22.0 * f.clamp(0.0, 1.0)).floor() as u8;
            for &(row, x0, x1, c) in ARROW_DONE {
                let x1 = x1.min(done);
                if x1 > x0 {
                    game.native_fill(batch, [x + x0 as f32, y + row as f32, (x1 - x0) as f32, 1.0], 0xFF00_0000 | c * 0x010101);
                }
            }
        }
        // A fluid or energy bar with its text.
        El::Bar { frac, color, text } => {
            let (w, _) = size(game, e);
            game.native_fill(batch, [x, y, w, 12.0], 0xFF2A2A2A);
            let c = 0xFF00_0000 | color;
            game.native_fill(batch, [x + 1.0, y + 1.0, ((w - 2.0) * frac.clamp(0.0, 1.0)).floor(), 10.0], c);
            batch.text_styled(atlas, &rich(&look.map(text)), x + 3.0, y + 2.0, white, look.shadow);
        }
        // HealthElement: hearts in rows of ten (or "health/max" past 40).
        El::Hearts { health, max, absorb } => {
            let icons = ((max.ceil() + absorb.ceil()) / 2.0).ceil().max(1.0) as i32;
            let line = num(game, "health_per_line").max(1.0) as i32;
            if icons * 2 > num(game, "health_max") as i32 || on(game, "health_fractions") {
                if let Some(r) = sprite(game, "hud/heart/full") {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white);
                }
                let t = format!("§f{:.0}/{:.0}", (health + absorb).ceil(), max.ceil());
                batch.text_styled(atlas, &horizon_client::text::Text::legacy(&t), x + 10.0, y + 1.0, white, look.shadow);
                return;
            }
            let hearts = health.round() * 0.5;
            let per = icons.min(line);
            let rows = (icons + line - 1) / line;
            for i in (1..=icons).rev() {
                let col = (i - 1) % per;
                let row = (i - 1) / per;
                let px = x + 8.0 * col as f32;
                let py = y + 4.0 * (rows - 1 - row) as f32;
                if let Some(r) = sprite(game, "hud/heart/container") {
                    batch.rect_uv(px, py, 9.0, 9.0, r, white);
                }
                let absorbing = i > (max * 0.5).ceil() as i32;
                let (full, half) = if absorbing { ("hud/heart/absorbing_full", "hud/heart/absorbing_half") } else { ("hud/heart/full", "hud/heart/half") };
                let cur = if absorbing { (max.ceil() + absorb) * 0.5 } else { hearts };
                if (i as f32) <= cur.floor() {
                    if let Some(r) = sprite(game, full) {
                        batch.rect_uv(px, py, 9.0, 9.0, r, white);
                    }
                } else if (i as f32) < cur + 1.0 {
                    if let Some(r) = sprite(game, half) {
                        batch.rect_uv(px, py, 9.0, 9.0, r, white);
                    }
                }
            }
        }
        // ArmorElement: armour icons (or the number past 20).
        El::Small(t) => {
            let start = batch.vertices.len();
            batch.text_styled(atlas, &rich(&look.map(t)), x, y, look.base, look.shadow);
            batch.scale_since(start, x, y, 0.75, 0.75);
        }
        El::Armor(a) => {
            if *a > num(game, "armor_max") as f32 {
                if let Some(r) = sprite(game, "hud/armor_full") {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white);
                }
                batch.text_styled(atlas, &horizon_client::text::Text::legacy(&format!("§f{a:.0}")), x + 10.0, y + 1.0, white, look.shadow);
                return;
            }
            let half = a * 0.5;
            let icons = half.ceil() as i32;
            for i in 1..=icons {
                let px = x + 8.0 * ((i - 1) % 10) as f32;
                let py = y + 4.0 * ((i - 1) / 10) as f32;
                let name = if (i as f32) <= half.floor() { "hud/armor_full" } else { "hud/armor_half" };
                if let Some(r) = sprite(game, name) {
                    batch.rect_uv(px, py, 9.0, 9.0, r, white);
                }
            }
        }
        // HarvestToolProvider's tick or cross: three quarters, over the last tool's corner.
        El::Mark(t) => {
            let (mx, my) = (x - 3.0, y - (h / 2.0).floor() + 3.0);
            let start = batch.vertices.len();
            batch.text_styled(atlas, &rich(&look.map(t)), mx, my, look.base, look.shadow);
            batch.scale_since(start, mx, my, 0.75, 0.75);
        }
        // Each of the sprite's pixels, as the game's atlas has no room kept for a mod's without a content pack.
        El::Energy => {
            let scale = ENERGY_ICON / 16.0;
            for (px, py, argb) in ENERGY_PIXELS {
                game.native_fill(batch, [x + px as f32 * scale, y + py as f32 * scale, scale, scale], argb);
            }
        }
        El::Space(_) | El::Icon(_) | El::Badge | El::Title(_) | El::Stats { .. } => {}
    }
}

/// What a server line says about the box rather than in it: the icon, the title's pattern and the mob's stats. Takes
/// them out, and the lines they leave empty (not the title's).
fn apply_directives(game: &Game<'_>, lines: &mut Vec<Line>, icon: &mut Option<String>, icon_stack: &mut Option<ItemStack>, badge: &mut bool) {
    let mut titles: Vec<String> = Vec::new();
    let mut stats = None;
    let mut any = false;
    for l in lines.iter_mut() {
        l.els.retain(|e| match e {
            El::Icon(id) => {
                *icon = Some(id.clone());
                *icon_stack = None;
                any = true;
                false
            }
            El::Badge => {
                *badge = true;
                any = true;
                false
            }
            El::Title(t) => {
                titles.push(t.clone());
                any = true;
                false
            }
            El::Stats { absorb, armor } => {
                stats = Some((*absorb, *armor));
                any = true;
                false
            }
            _ => true,
        });
    }
    if !any {
        return;
    }
    let mut row = 0;
    lines.retain(|l| {
        row += 1;
        row == 1 || !l.els.is_empty()
    });
    // ITooltip.replace(CORE_OBJECT_NAME, ...).
    if let Some(El::Text(title)) = lines.first_mut().and_then(|l| l.els.first_mut()) {
        for pattern in &titles {
            let now = title.strip_prefix("§f").unwrap_or(title.as_str()).to_owned();
            *title = format!("§f{}", fill(pattern, &[&now]));
        }
    }
    // EntityHealthAndArmorProvider.Client: both on one line only when both are numbers.
    if let Some((absorb, armor)) = stats {
        let mut hearts_at = None;
        for (i, l) in lines.iter_mut().enumerate() {
            for e in &mut l.els {
                if let El::Hearts { absorb: a, .. } = e {
                    *a = absorb;
                    hearts_at = Some(i);
                }
            }
        }
        lines.retain(|l| !l.els.iter().any(|e| matches!(e, El::Armor(_))));
        if armor > 0.0 && on(game, "entity_armor") {
            let armor_text = armor > num(game, "armor_max") as f32;
            let hearts_text = hearts_at.is_some_and(|i| {
                lines[i].els.iter().any(|e| match e {
                    El::Hearts { max, absorb, .. } => (((max.ceil() + absorb.ceil()) / 2.0).ceil() * 2.0) > num(game, "health_max") as f32 || on(game, "health_fractions"),
                    _ => false,
                })
            });
            match hearts_at {
                Some(i) if armor_text && hearts_text => {
                    lines[i].els.push(El::Space(4.0));
                    lines[i].els.push(El::Armor(armor));
                }
                Some(i) => lines.insert(i + 1, Line::new().with(El::Armor(armor))),
                None => lines.insert(lines.len().min(1), Line::new().with(El::Armor(armor))),
            }
        }
    }
}

/// SelectableSlotContainer.getHitSlot: the shelf or chiseled bookshelf slot the crosshair is on (-1 for none).
fn shelf_slot(game: &Game<'_>, pos: [i32; 3], block: &str) -> i32 {
    let (rows, cols) = if block == "minecraft:chiseled_bookshelf" {
        (2, 3)
    } else if block.ends_with("_shelf") {
        (1, 3)
    } else {
        return -1;
    };
    let Some((eye, dir)) = game.aim() else { return -1 };
    let Some(hit) = game.clip_ray(eye, dir, 8.0).filter(|h| h.pos == pos) else { return -1 };
    let id = game.world.block(pos[0], pos[1], pos[2]);
    let facing = game.pack.assets.registry.state(id).and_then(|s| s.properties.get("facing").cloned());
    let want: [i32; 3] = match facing.as_deref() {
        Some("north") => [0, 0, -1],
        Some("south") => [0, 0, 1],
        Some("west") => [-1, 0, 0],
        Some("east") => [1, 0, 0],
        _ => return -1,
    };
    if hit.face != want {
        return -1;
    }
    let rel = |a: usize| hit.point[a] - f64::from(pos[a]);
    let (fx, fy) = match want {
        [0, 0, -1] => (1.0 - rel(0), rel(1)),
        [0, 0, 1] => (rel(0), rel(1)),
        [-1, 0, 0] => (rel(2), rel(1)),
        _ => (1.0 - rel(2), rel(1)),
    };
    let section = |c: f64, n: i32| ((c * f64::from(n)).floor() as i32).clamp(0, n - 1);
    section(fx, cols) + section(1.0 - fy, rows) * cols
}

/// Jade's keys (JadeClient: Show Overlay numpad 1, Toggle Fluid numpad 2, Show Details left shift): the toggles
/// act on a press.
fn keys(game: &Game<'_>) -> bool {
    // KeyMapping: none of them fires under the config screens, whose keys are for typing and binding.
    if crate::game::jade_config::is_open() {
        return false;
    }
    // InputConstants.Key: a mapping is a key of the keyboard or a button of the mouse.
    let down = |name: &str| {
        let default = KEYS.iter().find(|k| k[0] == name).map_or("", |k| k[2]);
        let bound = game.native_mod_key(ID, name, default);
        match horizon_client::keybinds::mouse_button(&bound) {
            Some(button) => game.physical_buttons.contains(&button),
            None => horizon_client::keybinds::key_code(&bound).is_some_and(|k| game.physical_keys.contains(&k)),
        }
    };
    static PROFILE_KEYS: [&str; 4] = ["profile.0", "profile.1", "profile.2", "profile.3"];
    let profiles = if on(game, "enable_profiles") { &PROFILE_KEYS[..] } else { &[] };
    let pressed: Vec<&'static str> = ["config", "show_overlay", "toggle_liquid", "show_recipes", "show_uses", "narrate"]
        .into_iter()
        .chain(profiles.iter().copied())
        .filter(|n| down(n))
        .collect();
    let toggle = config(game).toggle_mode;
    let tts_toggle = opt(game, "tts_mode") == "toggle";
    let hold = down("show_overlay");
    CLIENT.with_borrow_mut(|c| {
        for name in &pressed {
            if !c.keys_down.contains(name) {
                match *name {
                    "show_overlay" if toggle => {
                        c.hidden = !c.hidden;
                        c.say = Some(if c.hidden { tr("narration.jade.key.show_overlay.off", "Overlay hidden") } else { tr("narration.jade.key.show_overlay.on", "Overlay shown") });
                        if c.hidden {
                            c.hint = Some(0);
                        }
                    }
                    "toggle_liquid" => {
                        c.liquids = !c.liquids;
                        c.say = Some(if c.liquids { tr("narration.jade.key.toggle_liquid.on", "Fluid shown") } else { tr("narration.jade.key.toggle_liquid.off", "Fluid hidden") });
                    }
                    "narrate" if tts_toggle => {
                        c.tts_on = !c.tts_on;
                        c.tts_last.clear();
                        if c.tts_on {
                            c.hint = Some(1);
                        }
                    }
                    "profile.0" => c.use_profile = Some(0),
                    "profile.1" => c.use_profile = Some(1),
                    "profile.2" => c.use_profile = Some(2),
                    "profile.3" => c.use_profile = Some(3),
                    "narrate" => c.narrate = true,
                    "config" => c.open_config = true,
                    "show_recipes" => c.lookup = Some(false),
                    "show_uses" => c.lookup = Some(true),
                    _ => {}
                }
            }
        }
        // DisplayMode.HOLD_KEY: shown only while the key is held.
        if !toggle {
            c.hidden = !hold;
        }
        c.keys_down = pressed;
    });
    down("show_details")
}

/// JadeKeys: name, label and default key (Options > Mods > Jade lists them; the player rebinds them there).
pub const KEYS: [[&str; 3]; 11] = [
    ["config", "Open Config", "key.keyboard.keypad.0"],
    ["show_overlay", "Show Overlay", "key.keyboard.keypad.1"],
    ["toggle_liquid", "Toggle Fluid", "key.keyboard.keypad.2"],
    ["show_recipes", "Show Recipes", "key.keyboard.keypad.3"],
    ["show_uses", "Show Uses", "key.keyboard.keypad.4"],
    ["narrate", "Narrate Target", "key.keyboard.keypad.5"],
    ["show_details", "Show Details", "key.keyboard.left.shift"],
    ["profile.0", "Use Profile #0", "key.keyboard.unknown"],
    ["profile.1", "Use Profile #1", "key.keyboard.unknown"],
    ["profile.2", "Use Profile #2", "key.keyboard.unknown"],
    ["profile.3", "Use Profile #3", "key.keyboard.unknown"],
];

/// Jade's settings (WailaConfig's general, overlay and plugin options this port has), kept in its settings folder.
#[derive(Clone)]
struct Config {
    blocks: bool,
    entities: bool,
    /// DisplayMode: TOGGLE (the key shows and hides) or HOLD_KEY.
    toggle_mode: bool,
    mod_name: bool,
    harvest_tool: bool,
    breaking: bool,
    effects: bool,
    entity_stats: bool,
    scale: f32,
    /// Overlay anchor Y: 0 top, 1 a third down, 2 middle.
    position: u8,
    bosses: bool,
    restock: bool,
    coordinates: bool,
    distance: bool,
    block_face: bool,
    block_states: bool,
    block_properties: bool,
    registry_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { blocks: true, entities: true, toggle_mode: true, mod_name: true, harvest_tool: true, breaking: true, effects: true,
            entity_stats: true, scale: 1.0, position: 0, bosses: true, restock: false, coordinates: false, distance: false, block_face: false, block_states: false, block_properties: false, registry_name: false }
    }
}

const SCALES: [f32; 5] = [0.5, 0.75, 1.0, 1.25, 1.5];

thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

fn config(game: &Game<'_>) -> Config {
    thread_local! {
        static READ_AT: std::cell::Cell<u32> = const { std::cell::Cell::new(u32::MAX) };
    }
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    if READ_AT.replace(epoch) != epoch {
        CONFIG.with_borrow_mut(|c| *c = None);
    }
    CONFIG.with_borrow_mut(|c| {
        c.get_or_insert_with(|| {
            let flag = |k: &str, d: bool| game.native_setting(ID, k).map_or(d, |v| v == "true");
            let d = Config::default();
            Config {
                blocks: flag("display_blocks", d.blocks),
                entities: flag("display_entities", d.entities),
                toggle_mode: game.native_setting(ID, "display_mode").is_none_or(|v| v != "hold"),
                mod_name: flag("mod_name", d.mod_name),
                harvest_tool: flag("harvest_tool", d.harvest_tool),
                breaking: flag("breaking_progress", d.breaking),
                effects: flag("potion_effects", d.effects),
                entity_stats: flag("entity_stats", d.entity_stats),
                scale: game.native_setting(ID, "scale").and_then(|v| v.parse().ok()).unwrap_or(1.0),
                position: game.native_setting(ID, "position").and_then(|v| v.parse().ok()).unwrap_or(0),
                bosses: flag("display_bosses", d.bosses),
                restock: flag("villager_restock", d.restock),
                coordinates: flag("coordinates", d.coordinates),
                distance: flag("distance", d.distance),
                block_face: flag("block_face", d.block_face),
                block_states: flag("block_states", d.block_states),
                block_properties: flag("block_properties", d.block_properties),
                registry_name: game.native_setting(ID, "registry_name").is_some_and(|v| v != "false"),
            }
        })
        .clone()
    })
}

/// `native_mods`: Jade in Options > Mods, with its keys.
pub fn listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: ID.into(),
        title: "Jade 26.3.5".into(),
        description: "Shows what you are looking at.".into(),
        keys: KEYS.iter().map(|k| k.map(String::from)).collect(),
    });
}

/// An option's value as its button says it: the theme's name, `Custom` for a dragged position.
pub fn value_label(game: &Game<'_>, o: &Opt) -> String {
    match o.key {
        "overlay_theme" => return crate::game::jade_theme::main_label(game),
        "theme_style" => return crate::game::jade_theme::style_label(game),
        _ => {}
    }
    let value = opt(game, o.key);
    if value.starts_with("custom:") {
        return "Custom".to_owned();
    }
    let english = o.values.iter().find(|(v, _)| *v == value).map_or(value.as_str(), |(_, l)| *l);
    match value_lang_key(o.key, &value) {
        Some(key) => tr(&key, english),
        None => english.to_owned(),
    }
}

/// `native_settings`: Jade's options page, the store under the config screens; its first button opens those.
pub fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    ensure_lang();
    buttons.push(format!("{}...", tr("gui.jade.configuration", "Jade Configuration")));
    for o in OPTIONS {
        buttons.push(format!("{}: {}", option_label(o), value_label(game, o)));
    }
    for (key, label, _) in plugin_options() {
        buttons.push(format!("{label}: {}", if on(game, key) { tr("options.on", "On") } else { tr("options.off", "Off") }));
    }
    buttons.push(tr("config.jade.edit_ignore_list", "Edit ignore list in Files..."));
    buttons.push(tr("config.jade.reload_plugins", "Reload Plugins"));
    buttons.push(tr("config.jade.reset_settings", "Reset Settings"));
}

/// Jade.getProfilePreset: what a profile starts as besides the defaults, by its number.
fn profile_preset(profile: &str) -> &'static [(&'static str, &'static str)] {
    match profile {
        // jade.profile_preset.accessibility.
        "1" => &[("accessibility_plugin", "true"), ("always_list_mode", "true"), ("animation", "false"), ("overlay_alpha", "100"), ("block_face", "true"), ("mod_name", "false")],
        // jade.profile_preset.minimalism.
        "2" => &[("display_mode", "lite"), ("boss_bar_overlap", "hide_tip"), ("overlay_alpha", "0"), ("overlay_theme", "dark"), ("theme_style", "slim"), ("display_item", "inline"), ("breaking_progress", "false"), ("harvest_tool", "false"), ("item_tooltip", "false")],
        _ => &[],
    }
}

/// Jade.saveProfile: this game's settings, kept as profile `n`'s.
pub fn save_profile(game: &Game<'_>, n: &str) {
    let plugins = plugin_keys();
    let mut saved: Vec<String> = OPTIONS.iter().filter(|p| p.key != "profile" && p.key != "enable_profiles").map(|p| format!("{}={}", p.key, opt(game, p.key))).collect();
    saved.extend(plugins.iter().map(|k| format!("{k}={}", opt(game, *k))));
    game.native_set_setting(ID, &format!("saved_profile_{n}"), &saved.join("\n"));
}

/// Profiles: leaving one keeps its settings, entering one brings its own back; one never used starts from the defaults
/// and its preset.
fn switch_profile(game: &Game<'_>, from: &str, to: &str) {
    save_profile(game, from);
    if let Some(text) = game.native_setting(ID, &format!("saved_profile_{to}")) {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                game.native_set_setting(ID, k, v);
            }
        }
    } else {
        for o in OPTIONS.iter().filter(|p| p.key != "profile" && p.key != "enable_profiles") {
            game.native_set_setting(ID, o.key, o.default);
        }
        for (k, v) in profile_preset(to) {
            game.native_set_setting(ID, k, v);
        }
    }
}

/// `/jadec use_profile` and JadeClient.profiles' keys (Jade.useProfile).
pub fn use_profile(game: &Game<'_>, n: &str) {
    let from = opt(game, "profile");
    if on(game, "enable_profiles") {
        switch_profile(game, &from, n);
    }
    game.native_set_setting(ID, "profile", n);
    settings_changed();
}

pub fn settings_changed() {
    SETTINGS_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    CONFIG.with_borrow_mut(|c| *c = None);
}

/// `/jadec ...` (JadeClientCommand) on the player's own client, `/jade ...` to the server.
pub fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    // A guest's `/jade` that reached the server's dispatcher is theirs, not this client's. `/jadec` is always the client's own.
    if matches!(command.split_whitespace().next(), Some("jade" | "gamerule"))
        && let Some(player) = game.native_acting_player()
    {
        perform_command(game, &player, command, ran);
        return;
    }
    let mut words = command.split_whitespace();
    match words.next() {
        Some("jadec") => {
            *ran = true;
            let args: Vec<&str> = words.collect();
            match args.as_slice() {
                ["config"] => CLIENT.with_borrow_mut(|c| c.open_config = true),
                ["use_profile", n @ ("0" | "1" | "2" | "3")] => {
                    use_profile(game, n);
                    game.native_chat(&format!("Jade profile #{n}"));
                }
                ["pin"] => {
                    let was = CLIENT.with_borrow_mut(|c| c.pinned.take());
                    if was.is_none() {
                        let now = target(game);
                        CLIENT.with_borrow_mut(|c| c.pinned = now);
                    }
                }
                ["handlers"] => {
                    let text: Vec<String> = OPTIONS.iter().map(|o| format!("- `{}`: {} (default {})", o.key, o.label, o.default)).collect();
                    let done = Game::native_config_dir().ok().and_then(|d| std::fs::write(d.join("jade_handlers.md"), text.join("\n")).ok());
                    game.native_chat(if done.is_some() { "Dumped successfully" } else { "Couldn't write jade_handlers.md" });
                }
                ["generate_server_overrides", pattern @ ..] => {
                    let pattern = pattern.join(" ");
                    let wanted: serde_json::Map<String, serde_json::Value> = OPTIONS
                        .iter()
                        .filter(|o| pattern.is_empty() || o.key.contains(pattern.as_str()))
                        .map(|o| (o.key.to_owned(), serde_json::Value::String(opt(game, o.key))))
                        .collect();
                    if wanted.is_empty() {
                        game.native_chat("No config options match the pattern");
                    } else if let Some(file) = ignore_files().and_then(|f| f[0].parent().map(|d| d.join("server-plugin-overrides.json"))) {
                        let _ = std::fs::write(&file, serde_json::Value::Object(wanted).to_string());
                        settings_changed();
                        game.native_chat(&format!("Generated override file: {}", file.display()));
                    }
                }
                _ => game.native_chat("/jadec config | use_profile <0-3> | pin | handlers | generate_server_overrides [pattern]"),
            }
        }
        Some("jade") => {
            *ran = true;
            let rest: Vec<&str> = words.collect();
            let msg = format!("c {}", rest.join(" "));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_err() {
                let me = game.native_server_players().into_iter().next();
                if let Some(me) = me {
                    server_command(game, me.uuid, me.permission, &rest);
                }
            }
        }
        _ => {}
    }
}

/// `/jade show|hide <targets>` (JadeServerCommand, level 2): each player's overlay on or off.
fn server_command(game: &mut Game<'_>, from: u128, level: u8, args: &[&str]) {
    // No player (`from` 0): the server's console.
    let tell = |game: &mut Game<'_>, t: &str| {
        if from == 0 {
            game.native_feedback(horizon_client::text::Text::legacy(t));
        } else {
            game.native_tell(from, horizon_client::text::Text::legacy(t));
        }
    };
    if level < 2 {
        tell(game, "You do not have permission to use this command");
        return;
    }
    let (show, wanted) = match args {
        ["show", t] => (true, *t),
        ["hide", t] => (false, *t),
        _ => {
            tell(game, "/jade show|hide <targets>");
            return;
        }
    };
    let players = game.native_server_players();
    let mut told = 0;
    for p in players.iter().filter(|p| wanted == "@a" || (matches!(wanted, "@s" | "@p") && p.uuid == from) || p.name == wanted) {
        let line = format!("o {}", u8::from(show));
        if game.native_net_send_to_player(p.uuid, NET_DATA, line.as_bytes()).is_ok() {
            told += 1;
        }
    }
    tell(game, &format!("Overlay {} for {told} players", if show { "shown" } else { "hidden" }));
}

/// `native_perform_command`: a LAN guest's `/jade`.
pub fn perform_command(game: &mut Game<'_>, player: &crate::game::native_server::NativeServerPlayer, command: &str, ran: &mut bool) {
    let all: Vec<&str> = command.split_whitespace().collect();
    if rule_command(game, player.uuid, &all, player.permission) {
        *ran = true;
        return;
    }
    let mut words = command.split_whitespace();
    if words.next() == Some("jade") {
        *ran = true;
        let args: Vec<&str> = words.collect();
        server_command(game, player.uuid, player.permission, &args);
    }
}

/// `native_perform_console_command`: `/jade` on a server's console.
pub fn perform_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let all: Vec<&str> = command.split_whitespace().collect();
    if rule_command(game, 0, &all, 4) {
        *ran = true;
        return;
    }
    let mut words = command.split_whitespace();
    if words.next() == Some("jade") {
        *ran = true;
        let args: Vec<&str> = words.collect();
        server_command(game, 0, 4, &args);
    }
}

pub fn register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    for syntax in ["jade show <targets:player>", "jade hide <targets:player>", "gamerule jade:max_position_deviation [value:integer]"] {
        commands.push(crate::game::native_server::NativeCommand::op(syntax, 2));
    }
}

const RULE_NAME: &str = "jade:max_position_deviation";
const RULE_DEFAULT: i32 = 21;

/// One step of an option's cycle (CycleButton, Shift going back): the theme's and style's over the themes there are, a
/// value between the presets (a slider's, a dragged position) on to the next one up.
pub fn step_option(game: &Game<'_>, o: &Opt, back: bool) {
    match o.key {
        "overlay_theme" => return crate::game::jade_theme::cycle_main(game, back),
        "theme_style" => return crate::game::jade_theme::cycle_style(game, back),
        _ => {}
    }
    let value = opt(game, o.key);
    let n = o.values.len();
    let to = match o.values.iter().position(|(v, _)| *v == value) {
        Some(at) if back => (at + n - 1) % n,
        Some(at) => (at + 1) % n,
        None => {
            let now = value.parse::<f64>().unwrap_or(f64::MIN);
            o.values.iter().position(|(v, _)| v.parse::<f64>().is_ok_and(|x| x > now)).unwrap_or(0)
        }
    };
    let next = o.values[to].0;
    if o.key == "profile" && on(game, "enable_profiles") {
        switch_profile(game, &value, next);
    }
    game.native_set_setting(ID, o.key, next);
    settings_changed();
}

/// WailaConfigScreen.openFile, on the folder of the two lists.
pub fn open_ignore_lists(game: &mut Game<'_>) {
    if let Some(files) = ignore_files()
        && let Some(dir) = files[0].parent()
    {
        // JadeClient.pleaseWait.
        game.narrator_toast = Some(horizon_client::toasts::SystemToast::new("Please wait...".to_owned(), Some("This may take a few seconds.".to_owned())));
        crate::game::packs::open_folder(dir);
    }
}

/// config.jade.reload_plugins: every cached answer goes (the settings, the ignore lists, tool and mod names, the
/// themes), and what was read from the game's files.
pub fn reload_plugins() {
    BEHAVIOURS.with_borrow_mut(|b| *b = None);
    CLIENT.with_borrow_mut(|c| {
        c.data = None;
        c.mob_data = None;
        c.ber_icon = None;
    });
    crate::game::jade_theme::reloaded();
    settings_changed();
}

/// Jade.resetConfig: every option back to its default.
pub fn reset_settings(game: &Game<'_>) {
    for o in OPTIONS {
        game.native_set_setting(ID, o.key, o.default);
    }
    for (key, _, default_on) in plugin_options() {
        game.native_set_setting(ID, key, if default_on { "true" } else { "false" });
    }
    settings_changed();
}

/// `native_settings_click`: the first button opens Jade's config screens over this page; any other steps its option on.
pub fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    let Some(button) = button.checked_sub(1) else {
        crate::game::jade_config::open_over_options(game);
        return;
    };
    if let Some(o) = OPTIONS.get(button) {
        step_option(game, o, false);
    } else if let Some((key, _, _)) = PLUGIN_OPTIONS.lock().ok().and_then(|v| v.get(button - OPTIONS.len()).copied()) {
        let now = on(game, key);
        game.native_set_setting(ID, key, if now { "false" } else { "true" });
        settings_changed();
    } else {
        let plugins = plugin_options();
        match button - OPTIONS.len() - plugins.len() {
            0 => open_ignore_lists(game),
            1 => reload_plugins(),
            _ => reset_settings(game),
        }
    }
}

thread_local! {
    /// Item Storage, for the request being answered.
    static SERVER_ITEMS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// `native_hud`: the box, at the top centre as Jade's default places it.
pub fn hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    ensure_lang();
    let details = keys(game);
    if game.player.as_ref().is_some_and(|p| p.in_powder_snow) {
        CLIENT.with_borrow_mut(|c| c.powder_at = Some(std::time::Instant::now()));
    }
    if let Some(n) = CLIENT.with_borrow_mut(|c| c.use_profile.take()) {
        use_profile(game, &n.to_string());
        // narration.jade.key.profile.
        CLIENT.with_borrow_mut(|c| c.say = Some(tr("narration.jade.key.profile", "Use %s").replace("%s", &format!("#{n}"))));
    }
    // Jade.history()'s hints, once each: toast.jade.toggle_hint and toast.jade.tts_hint, in chat as Jade sends them.
    if let Some(which) = CLIENT.with_borrow_mut(|c| c.hint.take()) {
        let (flag, key_name, first, second) = if which == 0 {
            ("hint_overlay_toggle", "show_overlay", tr("toast.jade.toggle_hint.1", "Jade is now closed"), tr("toast.jade.toggle_hint.2", "Press %s again to reopen"))
        } else {
            ("hint_narrator_toggle", "narrate", tr("toast.jade.tts_hint.1", "Jade's narrator is now open"), tr("toast.jade.tts_hint.2", "Press %s again to disable"))
        };
        if game.native_setting(ID, flag).is_none() {
            game.native_set_setting(ID, flag, "shown");
            let default = KEYS.iter().find(|k| k[0] == key_name).map_or("", |k| k[2]);
            let bound = game.native_mod_key(ID, key_name, default);
            let bound = game.translate(&bound, &bound);
            game.native_chat(&first);
            game.native_chat(&second.replace("%s", &bound));
        }
    }
    if CLIENT.with_borrow_mut(|c| std::mem::take(&mut c.open_config)) && matches!(game.screen, crate::game::Screen::Playing) {
        // HomeConfigScreen over the world. Where it can't open, Options > Mods, whose Jade page has it too.
        if crate::game::jade_config::open_in_world(game) {
            return;
        }
        if game.title.is_none() {
            game.capture(false);
        }
        game.open_options(game.title.is_some());
        game.ui.options.show(horizon_client::options_ui::Page::Mods);
        return;
    }
    // JEI's recipe view opens over the inventory: once it is open, JEI hears its own key for the item.
    if !matches!(game.screen, crate::game::Screen::Playing)
        && let Some((item, uses)) = CLIENT.with_borrow_mut(|c| c.lookup_item.take())
    {
        let name = if uses { "uses" } else { "recipe" };
        let key = game.native_mod_key("jei", name, if uses { "key.keyboard.u" } else { "key.keyboard.r" });
        let stack = crate::game::native_hooks::NativeStack { item, count: 1, components: String::new() };
        game.native_panel_event("jei", &crate::game::native_hooks::NativePanelEvent::Key { key, stack });
    }
    CLIENT.with_borrow_mut(|c| c.showing = false);
    if on(game, "narrate_keys")
        && let Some(line) = CLIENT.with_borrow_mut(|c| c.say.take())
    {
        game.say_system_now(&horizon_client::text::Text::literal(&line));
    }
    // Jade hides while a screen is open (Hide from GUIs), over the player list (Hide from Player List), and with Show
    // Overlay toggled off.
    // A config screen open shows the box over itself (PreviewOptionsScreen): built here as ever, drawn there.
    let previewing = crate::game::jade_config::wants_box();
    let screen_hides = !previewing && !matches!(game.screen, crate::game::Screen::Playing) && on(game, "hide_from_guis");
    let list_hides = on(game, "hide_from_tab_list") && game.physical_keys.contains(&winit::keyboard::KeyCode::Tab);
    if !game.native_on(ID) || game.player.is_none() || screen_hides || list_hides || CLIENT.with_borrow(|c| c.hidden) {
        return;
    }
    // Boss Bar Overlap's Hide Tooltip, and how far Push Down moves it.
    let overlap = opt(game, "boss_bar_overlap");
    let bars = game.boss_overlay.bars(horizon_client::boss_overlay::now_millis()).len();
    let at_top = opt(game, "position");
    if bars > 0 && overlap == "hide_tip" && !previewing && (at_top == "0" || at_top.starts_with("custom:")) {
        return;
    }
    let pinned = CLIENT.with_borrow(|c| c.pinned.clone());
    let Some(t) = pinned.or_else(|| target(game)) else {
        CLIENT.with_borrow_mut(|c| {
            c.progress_alpha = 0.0;
            c.lookup = None;
            c.fade = 0.0;
            c.tts_last.clear();
        });
        return;
    };
    CLIENT.with_borrow_mut(|c| c.showing = true);
    // The server's lines for a block, asked for a few times a second while the player looks. Not for what passes for
    // another block without its block entity (builtInOverrides' `blockEntity(() -> null)`).
    if let Target::Block { pos, block } = &t {
        let real = game.res.facts().get(game.world.block(pos[0], pos[1], pos[2]));
        let hidden = &*real.name != block.as_str() && !real.name.contains("chest");
        let slot = shelf_slot(game, *pos, block);
        let ask = !hidden && CLIENT.with_borrow(|c| c.asked.is_none_or(|(p, at)| p != *pos || at.elapsed().as_millis() > 250) || c.asked_details != details || c.asked_slot != slot);
        if ask {
            let hit = CLIENT.with_borrow(|c| c.hit_point).map(|v| (v * 4096.0).round() as i64);
            let msg = format!("{} {} {} {} {slot} {} {} {}{}", pos[0], pos[1], pos[2], u8::from(details), hit[0], hit[1], hit[2], request_off_list(game));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_ok() {
                CLIENT.with_borrow_mut(|c| {
                    c.asked = Some((*pos, std::time::Instant::now()));
                    c.asked_details = details;
                    c.asked_slot = slot;
                });
            }
        }
    }
    // The server's lines for a mob or vehicle: only the server's copy has its timers, trades and items.
    let entity = entity_id(game, &t);
    if let Some(id) = entity {
        let ask = CLIENT.with_borrow(|c| c.mob_asked.is_none_or(|(i, at)| i != id || at.elapsed().as_millis() > 250) || c.asked_details != details);
        if ask {
            let off = request_off_list(game);
            let msg = if matches!(t, Target::Mob { .. }) {
                let cfg = config(game);
                let bit = |k: &'static str| u8::from(on(game, k));
                format!("m {id} {} {} {} {} {} {} {}{off}", u8::from(details), u8::from(cfg.restock), u8::from(cfg.entity_stats), bit("mob_growth"), bit("mob_breeding"), bit("next_entity_drop"), bit("zombie_villager"))
            } else {
                format!("k {id} {}{off}", u8::from(details))
            };
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_ok() {
                CLIENT.with_borrow_mut(|c| {
                    c.mob_asked = Some((id, std::time::Instant::now()));
                    c.asked_details = details;
                });
            }
        }
    }
    let mut icon_stack: Option<ItemStack> = None;
    let mut badge = false;
    let (icon, title, ns) = match &t {
        Target::Cart { index } => {
            let Some(c) = game.carts.get(*index) else { return };
            let id = c.kind.id();
            let short = id.trim_start_matches("minecraft:");
            // AbstractMinecart.getPickResult.
            (Some(c.kind.item().to_owned()), game.translate(&format!("entity.minecraft.{short}"), short), "minecraft".to_owned())
        }
        Target::Boat { index } => {
            let Some(b) = game.boats.get(*index) else { return };
            let id = if b.chest { format!("minecraft:{}_chest_boat", b.wood) } else { format!("minecraft:{}_boat", b.wood) };
            let short = id.trim_start_matches("minecraft:").to_owned();
            (Some(id), game.translate(&format!("entity.minecraft.{short}"), &short), "minecraft".to_owned())
        }
        // FallingBlockProvider: the block's item as its icon, its name.
        Target::Falling { index } => {
            let Some(f) = game.falling.get(*index) else { return };
            let block = game.res.facts().get(f.state).name.to_string();
            let ns = block.split_once(':').map_or("minecraft", |(n, _)| n).to_owned();
            (game.res.items().get(&block).is_some().then(|| block.clone()), item_name(game, &block), ns)
        }
        // A fluid: its name, its bucket for the icon (ClientProxy.elementFromLiquid).
        Target::Block { block, .. } if matches!(block.as_str(), "minecraft:water" | "minecraft:lava") => (Some(format!("{block}_bucket")), fluid_name(game, block), "minecraft".to_owned()),
        // PlayerHeadProvider: a head with an owner is named after them.
        Target::Block { pos, block } if (block == "minecraft:player_head" || block == "minecraft:player_wall_head") && on(game, "player_head") => {
            icon_stack = ber_icon(game, *pos);
            let name = head_owner(game, *pos);
            let title = match name {
                Some(owner) => game.translate("block.minecraft.player_head.named", "%s's Head").replace("%s", &owner),
                None => item_name(game, "minecraft:player_head"),
            };
            (Some("minecraft:player_head".to_owned()), title, "minecraft".to_owned())
        }
        Target::Block { pos, block } => {
            let hit = CLIENT.with_borrow(|c| c.hit_point);
            let named = NAME_PROVIDERS.lock().map(|v| v.clone()).unwrap_or_default().into_iter().find_map(|f| f(game, *pos, block, hit));
            match named {
                Some((icon, title, ns)) => (game.res.items().get(&icon).is_some().then_some(icon), title, ns),
                None => {
                    // ItemBERProvider: a skull's or pot's own stack as the icon.
                    if block.ends_with("_skull") || block.ends_with("_head") || block == "minecraft:decorated_pot" {
                        icon_stack = ber_icon(game, *pos);
                    }
                    // WaxedProvider.BlockComponent.getIcon.
                    if on(game, "waxed")
                        && let Some(horizon_client::containers::BlockEntity::Sign(sign)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
                    {
                        badge = sign.waxed;
                    }
                    let has_item = game.res.items().get(block).is_some();
                    (has_item.then(|| block.clone()), item_name(game, block), block.split_once(':').map_or("minecraft", |(n, _)| n).to_owned())
                }
            }
        }
        // BlockDisplayProvider and ItemDisplayProvider: what it shows, as the icon.
        Target::Display { id } => {
            let Some(d) = game.text_displays.get(id) else { return };
            if let Some(stack) = d.hz_jade_item() {
                icon_stack = Some(stack.clone());
                (Some(stack.item.to_string()), game.translate("entity.minecraft.item_display", "Item Display"), "minecraft".to_owned())
            } else {
                let block = d.hz_jade_block().map(|state| game.res.facts().get(state).name.to_string());
                (block.filter(|b| game.res.items().get(b).is_some()), game.translate("entity.minecraft.block_display", "Block Display"), "minecraft".to_owned())
            }
        }
        Target::Mob { index } => {
            let Some(m) = game.mobs.get(*index) else { return };
            let id = m.kind.id.trim_start_matches("minecraft:");
            let name = m.custom_name.clone().unwrap_or_else(|| game.translate(&format!("entity.minecraft.{id}"), &horizon_client::text::Text::legacy(id).plain()));
            let egg = format!("minecraft:{id}_spawn_egg");
            // WaxedProvider.EntityComponent.getIcon: a copper golem shows as its statue (the server says if it is waxed).
            let icon = if id == "copper_golem" && on(game, "waxed") {
                const WEATHER: [&str; 4] = ["", "exposed_", "weathered_", "oxidized_"];
                Some(format!("minecraft:{}copper_golem_statue", WEATHER[usize::from(m.extra.variant.min(3))])).filter(|s| game.res.items().get(s).is_some())
            } else {
                None
            };
            (icon.or_else(|| game.res.items().get(&egg).is_some().then_some(egg)), name, m.kind.id.split_once(':').map_or("minecraft", |(n, _)| n).to_owned())
        }
        // Entity.getPickResult as the icon: the framed item, the painting, the stand, the dropped item.
        Target::Frame { index } => {
            let Some(f) = game.item_frames.get(*index) else { return };
            let kind = f.kind();
            let name = game.translate(&format!("entity.minecraft.{}", &kind[10..]), "Item Frame");
            (Some(f.item.as_ref().map_or_else(|| kind.to_owned(), |s| s.item.to_string())), name, "minecraft".to_owned())
        }
        Target::Painting { .. } => (Some("minecraft:painting".to_owned()), game.translate("entity.minecraft.painting", "Painting"), "minecraft".to_owned()),
        Target::Stand { .. } => (Some("minecraft:armor_stand".to_owned()), game.translate("entity.minecraft.armor_stand", "Armor Stand"), "minecraft".to_owned()),
        Target::Item { index } => {
            let Some(e) = game.dropped.get(*index) else { return };
            let name = game.stack_tooltip(&e.stack).first().map_or_else(|| e.stack.item.to_string(), |(t, _)| t.plain());
            let ns = e.stack.item.split_once(':').map_or("minecraft", |(n, _)| n).to_owned();
            (Some(e.stack.item.to_string()), name, ns)
        }
    };
    if let Some(uses) = CLIENT.with_borrow_mut(|c| c.lookup.take())
        && let Some(item) = icon.clone()
    {
        CLIENT.with_borrow_mut(|c| c.lookup_item = Some((item, uses)));
        // The inventory screen, where JEI's recipes show (creative's own E opens the creative screen, which has none).
        if game.creative {
            game.screen = crate::game::Screen::Inventory;
            game.capture(false);
        } else {
            let _ = game.key_pressed(winit::keyboard::KeyCode::KeyE, winit::keyboard::KeyCode::KeyE);
        }
    }
    let title = if on(game, "accessibility_plugin") { access_title(game, &t, title) } else { title };
    let mut icon = icon;
    // The title line: the name, the harvest tool at its end (HarvestToolProvider, on the title by default).
    let mut title_line = Line::new();
    if on(game, "object_name") {
        title_line = title_line.with(El::Text(format!("§f{title}")));
    }
    let mut tool_line = None;
    if let Target::Block { pos, block } = &t
        && config(game).harvest_tool
        && let Some(tool) = harvest_tool(game, *pos, block)
    {
        if on(game, "harvest_new_line") {
            let mut l = Line::new();
            l.els.extend(tool);
            tool_line = Some(l);
        } else {
            title_line = title_line.with(El::Space(6.0));
            title_line.els.extend(tool);
        }
    }
    let mut lines = vec![title_line];
    lines.extend(tool_line);
    vanilla(game, &t, details, &mut lines);
    if let Ok(providers) = CLIENT_PROVIDERS.lock().map(|v| v.clone()) {
        for p in providers {
            p(game, &t, &mut lines);
        }
    }
    if let Target::Block { pos, .. } = &t {
        CLIENT.with_borrow(|c| {
            if let Some((p, server, at)) = &c.data
                && p == pos
                && at.elapsed().as_secs() < 2
            {
                lines.extend(server.iter().map(|s| decode(s)));
            }
        });
    }
    if let Some(id) = entity {
        CLIENT.with_borrow(|c| {
            if let Some((i, server, at)) = &c.mob_data
                && *i == id
                && at.elapsed().as_secs() < 2
            {
                lines.extend(server.iter().map(|s| decode(s)));
            }
        });
    }
    apply_directives(game, &mut lines, &mut icon, &mut icon_stack, &mut badge);
    let theme = crate::game::jade_theme::current(game);
    // IThemeHelper.title: the title in the theme's colour.
    if on(game, "object_name")
        && let Some(El::Text(text)) = lines.first_mut().and_then(|l| l.els.first_mut())
    {
        let now = text.strip_prefix("§f").map(|rest| format!("{}{rest}", theme.title_code));
        if let Some(now) = now {
            *text = now;
        }
    }
    let cfg = config(game);
    // Display Mode Lite: the name and the mod only, the rest while Show Details is held (with an arrow while there is a
    // rest: Tooltip.sneakyDetails).
    let mut sneaky = false;
    if opt(game, "display_mode") == "lite" && !details {
        sneaky = lines.len() > 1;
        lines.truncate(1);
    }
    info_lines(game, &t, &cfg, &mut lines);
    if cfg.mod_name {
        let text = format!("{}{}", theme.mod_code, mod_name(game, &ns));
        lines.push(if on(game, "mod_name_smaller") { Line::new().with(El::Small(text)) } else { Line::text(text) });
    }
    if on(game, "debug_mode") {
        lines.push(Line::text(format!("§8{t:?}")));
    }
    // Text to Speech Mode Toggle: each new target read out.
    if CLIENT.with_borrow(|c| c.tts_on) {
        let spoken: String = lines.iter().flat_map(|l| l.els.iter()).filter_map(|e| if let El::Text(t) = e { Some(plain(t)) } else { None }).collect::<Vec<_>>().join(". ");
        if CLIENT.with_borrow(|c| c.tts_last != spoken) {
            game.say_system_now(&horizon_client::text::Text::literal(&spoken));
            CLIENT.with_borrow_mut(|c| c.tts_last = spoken);
        }
    }
    // The narrate key: every text of the box, line by line, through the game's narrator.
    if CLIENT.with_borrow_mut(|c| std::mem::take(&mut c.narrate)) {
        let spoken: Vec<String> = lines
            .iter()
            .map(|l| l.els.iter().filter_map(|e| if let El::Text(t) = e { Some(plain(t)) } else { None }).collect::<Vec<_>>().join(" "))
            .filter(|t| !t.trim().is_empty())
            .collect();
        game.say_system_now(&horizon_client::text::Text::literal(&spoken.join(". ")));
    }

    // BreakingProgressProvider: a line along the box's foot while the block is mined (FAILURE's colour when it won't drop).
    let mining = if let (Target::Block { pos, block }, Some(m)) = (&t, game.mining.as_ref())
        && m.pos == *pos
        && cfg.breaking
    {
        let alpha = CLIENT.with_borrow_mut(|c| {
            c.progress_alpha = (c.progress_alpha + 0.05).min(0.6);
            c.progress_alpha
        });
        let state = game.world.block(pos[0], pos[1], pos[2]);
        let ok = !game.res.facts().get(state).requires_tool || game.res.items().correct_for_drops(game.held(), block);
        Some((m.progress, !ok, alpha))
    } else {
        CLIENT.with_borrow_mut(|c| c.progress_alpha = 0.0);
        None
    };
    let built = BuiltBox { lines, icon, icon_stack, badge, mining, sneaky };
    // A config screen draws it, over its list (PreviewOptionsScreen).
    if previewing {
        CLIENT.with_borrow_mut(|c| c.built = Some(Box::new(built)));
        return;
    }
    draw_box(game, batch, &built, false);
}

/// What the overlay shows this frame, ready to draw (BoxElementImpl's tooltip and icon).
pub struct BuiltBox {
    lines: Vec<Line>,
    icon: Option<String>,
    icon_stack: Option<ItemStack>,
    badge: bool,
    /// BreakingProgressProvider: how far, whether it fails, and its alpha.
    mining: Option<(f32, bool, f32)>,
    /// Tooltip.sneakyDetails.
    sneaky: bool,
}

/// `OverlayRenderer.renderOverlay`'s stand-in while the position is adjusted with nothing looked at: the Grass Block.
pub fn sample_box(game: &Game<'_>) -> BuiltBox {
    let theme = crate::game::jade_theme::current(game);
    let title = format!("{}{}", theme.title_code, item_name(game, "minecraft:grass_block"));
    let modname = format!("{}{}", theme.mod_code, mod_name(game, "minecraft"));
    BuiltBox { lines: vec![Line::text(title), Line::text(modname)], icon: None, icon_stack: None, badge: false, mining: None, sneaky: false }
}

/// The box `hud` built for a config screen this frame.
pub fn take_built() -> Option<Box<BuiltBox>> {
    CLIENT.with_borrow_mut(|c| c.built.take())
}

/// One axis of WailaConfig.Overlay's position when it isn't a preset, as the drag-to-place screen saves it in the same
/// setting: `custom:<overlayPos>:<anchor>`.
pub fn custom_axis(value: &str) -> Option<(f32, f32)> {
    let mut w = value.strip_prefix("custom:")?.split(':');
    Some((w.next()?.parse().ok()?, w.next()?.parse().ok()?))
}

/// Where the box goes: the content's top left and size before the scale, its rectangle with the padding, the point the
/// scale is about, and that rectangle on the screen (`TooltipAnimation.expectedRect`).
struct Placed {
    x: f32,
    y: f32,
    h: f32,
    icon_w: f32,
    title_extra: f32,
    sizes: Vec<(f32, f32)>,
    frame: [f32; 4],
    pivot: [f32; 2],
    scale: f32,
    rect: [f32; 4],
}

/// `BoxElementImpl.updateExpectedRect`, with today's presets for an axis that holds one.
fn place(game: &Game<'_>, [sw, sh]: [f32; 2], b: &BuiltBox, [pad_up, pad_down, pad_left, pad_right]: [f32; 4], adjusting: bool) -> Placed {
    let line_size = |l: &Line| -> (f32, f32) {
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        for (i, e) in l.els.iter().enumerate() {
            let (ew, eh) = size(game, e);
            w += ew + if i > 0 { 1.0 } else { 0.0 };
            h = h.max(eh);
        }
        (w, h.max(9.0))
    };
    let sizes: Vec<(f32, f32)> = b.lines.iter().map(line_size).collect();
    // Icon Position: beside the box's top, its middle, or in the title's line.
    let icon_pos = opt(game, "display_item");
    let has_icon = b.icon.is_some() && icon_pos != "hide";
    let inline = icon_pos == "inline";
    // Theme.modifyIcon: the slot's padding is part of the icon.
    let theme = crate::game::jade_theme::current(game);
    let slot = theme.icon_slot(game).filter(|_| has_icon && !inline).map_or([0.0; 4], |s| s.padding);
    let icon_w = if !has_icon || inline { 0.0 } else { 20.0 + slot[1] + slot[3] };
    let title_extra = if inline && has_icon { 11.0 } else { 0.0 };
    let w = sizes.iter().enumerate().map(|(i, s)| s.0 + if i == 0 { title_extra } else { 0.0 }).fold(0.0, f32::max) + icon_w;
    let h = sizes.iter().map(|s| s.1 + 1.0).sum::<f32>() - 1.0;
    let h = if has_icon && !inline && slot != [0.0; 4] { h.max(16.0 + slot[0] + slot[2]) } else { h };
    let scale = num(game, "scale") as f32;
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let (bw, bh) = (w + pad_left + pad_right, h + pad_up + pad_down);
    let bars = game.boss_overlay.bars(horizon_client::boss_overlay::now_millis()).len();
    let push = !adjusting && bars > 0 && opt(game, "boss_bar_overlap") == "push";
    // The overlay's anchor, Flip Position with Main Hand (accessibility.tryFlip), and its scale about it.
    let flip = on(game, "flip_main_hand") && game.settings.main_hand_left;
    let side = opt(game, "overlay_x");
    let (x, pivot_x) = match custom_axis(&side) {
        Some((px, ax)) => {
            let (px, ax) = if flip { (1.0 - px, 1.0 - ax) } else { (px, ax) };
            let left = (sw * px - bw * scale * ax).floor();
            (left + pad_left, left)
        }
        None => {
            let mut x = match side.as_str() {
                "left" => 12.0,
                "right" => (sw - w - 12.0).floor(),
                _ => ((sw - w) / 2.0).floor(),
            };
            if flip {
                x = (sw - w - x).floor();
            }
            (x, sw / 2.0)
        }
    };
    let row = opt(game, "position");
    let (y, pivot_y) = match custom_axis(&row) {
        Some((py, ay)) => {
            let mut top = (sh * (1.0 - py) - bh * scale * ay).floor();
            // A boss bar in its way pushes it down, as Push Down Tooltip says.
            let left = x - pad_left;
            let bar_bottom = 19.0 * bars as f32;
            if push && top < bar_bottom && left < sw / 2.0 + 91.0 && left + bw * scale > sw / 2.0 - 91.0 {
                top = bar_bottom;
            }
            (top + pad_up, top)
        }
        None => {
            let mut y = match row.as_str() {
                "1" => (sh / 3.0).floor(),
                "2" => ((sh - h) / 2.0).floor(),
                "3" => (sh - h - 60.0).floor(),
                _ => 8.0,
            };
            if push && !matches!(row.as_str(), "1" | "2" | "3") {
                y += 19.0 * bars as f32;
            }
            (y, y - 12.0)
        }
    };
    let (left, top) = (x - pad_left, y - pad_up);
    let rect = [pivot_x + (left - pivot_x) * scale, pivot_y + (top - pivot_y) * scale, bw * scale, bh * scale];
    Placed { x, y, h, icon_w, title_extra, sizes, frame: [left, top, bw, bh], pivot: [pivot_x, pivot_y], scale, rect }
}

/// `TooltipAnimation.expectedRect` for a box.
pub fn box_rect(game: &Game<'_>, size: [f32; 2], b: &BuiltBox, adjusting: bool) -> [f32; 4] {
    let theme = crate::game::jade_theme::current(game);
    place(game, size, b, theme.padding(), adjusting).rect
}

/// The box: the theme's frame round the icon column and the lines. Its rectangle on the screen.
pub fn draw_box(game: &Game<'_>, batch: &mut HudBatch, b: &BuiltBox, adjusting: bool) -> [f32; 4] {
    let theme = crate::game::jade_theme::current(game);
    let look = theme.text_look();
    let p = place(game, [batch.width, batch.height], b, theme.padding(), adjusting);
    let (x, y, h, icon_w, title_extra) = (p.x, p.y, p.h, p.icon_w, p.title_extra);
    let icon_pos = opt(game, "display_item");
    let inline = icon_pos == "inline";
    let icon = b.icon.as_ref().filter(|_| icon_pos != "hide");
    let start = batch.vertices.len();
    // BoxElementImpl: `backgroundOpacity` times the overlay's Background Opacity, on the frame's every pixel.
    let alpha = (num(game, "overlay_alpha") / 100.0).clamp(0.0, 1.0) as f32;
    crate::game::jade_theme::draw_background(game, batch, &theme, p.frame, icon.is_some(), alpha);
    if let Some(icon) = icon {
        let (at, size) = if inline {
            ([x, y], 9.0)
        } else {
            let iy = if icon_pos == "centered" { y + ((h - 16.0) / 2.0).max(0.0).floor() } else { y + ((h - 16.0) / 2.0).min(2.0).max(0.0) };
            match theme.icon_slot(game) {
                // The slot, with the icon inside it.
                Some(slot) => {
                    let [top, _, bottom, left] = slot.padding;
                    let iy = if icon_pos == "centered" { y + ((h - 16.0 - top - bottom) / 2.0).max(0.0).floor() + top } else { y + top };
                    let at = [x + left, iy];
                    slot.draw(game, batch, at, 16.0, alpha);
                    (at, 16.0)
                }
                None => ([x, iy], 16.0),
            }
        };
        let drawn = b.icon_stack.as_ref().is_some_and(|stack| game.native_item_stack(batch, stack, at, size));
        if !drawn {
            game.native_item(batch, icon, at, size);
        }
        // WaxedProvider.BlockComponent.waxedIcon: a honeycomb at half size on top.
        if b.badge {
            game.native_item(batch, "minecraft:honeycomb", [at[0] + size / 2.0, at[1] + size / 2.0], size / 2.0);
        }
    }
    let text_alpha = num(game, "text_background_opacity");
    let mut ly = y;
    for (row, (l, (lw, lh))) in b.lines.iter().zip(&p.sizes).enumerate() {
        if text_alpha > 0.0 {
            let a = (text_alpha / 100.0 * 255.0) as u32;
            game.native_fill(batch, [x + icon_w - 1.0, ly - 1.0, lw + 2.0, lh + 1.0], a << 24);
        }
        let mut lx = x + icon_w + if row == 0 { title_extra } else { 0.0 };
        for e in &l.els {
            draw_el(game, batch, e, lx, ly, *lh, &look);
            lx += size(game, e).0 + 1.0;
        }
        ly += lh + 1.0;
    }
    if let Some((progress, failed, progress_alpha)) = b.mining {
        crate::game::jade_theme::draw_progress(game, batch, &theme, p.frame, progress, failed, progress_alpha * alpha);
    }
    if b.sneaky {
        // OverlayRenderer.ticks: twenty a second.
        thread_local! {
            static STARTED: std::time::Instant = std::time::Instant::now();
        }
        let ticks = STARTED.with(|t| t.elapsed().as_secs_f32()) * 20.0;
        theme.draw_sneaky(game, batch, p.frame, ticks, 1.0);
    }
    if (p.scale - 1.0).abs() > 0.01 {
        batch.scale_since(start, p.pivot[0], p.pivot[1], p.scale, p.scale);
    }
    // Overlay Animation: fades in over a tenth of a second.
    if on(game, "animation") {
        let fade = CLIENT.with_borrow_mut(|c| {
            let now = std::time::Instant::now();
            let dt = c.fade_at.map_or(0.0, |t| now.duration_since(t).as_secs_f32());
            c.fade_at = Some(now);
            c.fade = (c.fade + dt / 0.1).min(1.0);
            c.fade
        });
        if fade < 1.0 {
            for v in &mut batch.vertices[start..] {
                let a = ((v.color >> 24) as f32 * fade) as u32;
                v.color = (v.color & 0x00FF_FFFF) | (a << 24);
            }
        }
    }
    p.rect
}

/// The id the server knows an entity target by (mobs, and the vehicles whose items only it has).
fn entity_id(game: &Game<'_>, t: &Target) -> Option<u32> {
    match t {
        Target::Mob { index } => game.mobs.get(*index).map(|m| m.id),
        Target::Cart { index } => game.carts.get(*index).map(|c| c.id),
        Target::Boat { index } => game.boats.get(*index).filter(|b| b.chest).map(|b| b.id),
        _ => None,
    }
}

/// ItemBERProvider: a block's picked stack with its block entity's data, kept a second (it is made anew otherwise each
/// frame).
fn ber_icon(game: &Game<'_>, pos: [i32; 3]) -> Option<ItemStack> {
    let kept = CLIENT.with_borrow(|c| c.ber_icon.as_ref().filter(|(p, _, at)| *p == pos && at.elapsed().as_secs() < 1).map(|(_, s, _)| s.clone()));
    if let Some(stack) = kept {
        return stack;
    }
    let stack = game.picked_stack(pos, true);
    CLIENT.with_borrow_mut(|c| c.ber_icon = Some((pos, stack.clone(), std::time::Instant::now())));
    stack
}


/// Boss Bar Overlap's Hide Boss Bar: no bars while the box shows.
pub fn hide_boss_bars(game: &Game<'_>) -> bool {
    CLIENT.with_borrow(|c| c.showing) && opt(game, "boss_bar_overlap") == "hide_bar"
}

/// Mod Name on Item Tooltip: the stack's mod at the end of its tooltip.
pub fn item_tooltip(game: &Game<'_>, stack: &ItemStack, mut lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
    if game.native_on(ID) && on(game, "item_mod_name") {
        let ns = stack.item.split_once(':').map_or("minecraft", |(n, _)| n);
        lines.push((horizon_client::text::Text::legacy(&format!("§9§o{}", mod_name(game, ns))), u32::MAX));
    }
    lines
}

/// The most a client may ask: an honest one asks a few times a second for what it looks at.
const REQUEST_BYTES: usize = 4096;
const REQUESTS_PER_SECOND: u32 = 60;

fn request_allowed(player: u128) -> bool {
    thread_local! {
        static RATES: RefCell<std::collections::HashMap<u128, (std::time::Instant, u32)>> = RefCell::new(std::collections::HashMap::new());
    }
    RATES.with_borrow_mut(|rates| {
        if rates.len() > 4096 {
            rates.clear();
        }
        let now = std::time::Instant::now();
        let (since, count) = rates.entry(player).or_insert((now, 0));
        if now.duration_since(*since).as_secs() >= 1 {
            *since = now;
            *count = 0;
        }
        *count += 1;
        *count <= REQUESTS_PER_SECOND
    })
}

/// `native_net_from_client` (server): a player asks for a block's lines.
pub fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != NET_REQUEST || data.len() > REQUEST_BYTES || !request_allowed(player) {
        return;
    }
    let text = String::from_utf8_lossy(data);
    if let Some(rest) = text.strip_prefix("c ") {
        let level = game.native_server_players().into_iter().find(|p| p.uuid == player).map_or(0, |p| p.permission);
        let args: Vec<&str> = rest.split_whitespace().collect();
        server_command(game, player, level, &args);
        return;
    }
    // A request's end names the toggles this player has off.
    let (text, off) = text.split_once(';').map_or((&*text, None), |(t, o)| (t, Some(o)));
    if let Some(off) = off {
        set_request_off(off);
    }
    if let Some(rest) = text.strip_prefix("m ") {
        mob_request(game, player, rest);
    } else if let Some(rest) = text.strip_prefix("k ") {
        vehicle_request(game, player, rest);
    } else {
        block_request(game, player, text);
    }
    clear_request_off();
}

/// Jade.isOutOfReach: single player's owner reaches everything; anyone else, the hand's range and the deviation.
fn out_of_reach(game: &Game<'_>, player: u128, at: [f64; 3], base: f64) -> bool {
    if game.serving.as_ref().is_some_and(|s| s.owner == player) {
        return false;
    }
    let Some(p) = game.native_server_players().into_iter().find(|p| p.uuid == player) else { return true };
    if p.dimension != game.dim.id() {
        return true;
    }
    let most = base + f64::from(RULE.get());
    (0..3).map(|i| (p.position[i].floor() - at[i].floor()).powi(2)).sum::<f64>() > most * most
}

thread_local! {
    /// Where the player answered for right now is looking on the block.
    static REQUEST_HIT: RefCell<Option<[f64; 3]>> = const { RefCell::new(None) };
}

/// While a server provider answers: the point on the block the player looks at.
pub fn request_hit() -> Option<[f64; 3]> {
    REQUEST_HIT.with_borrow(|h| *h)
}

/// A player asks for a block's lines: its position, Show Details and the slot they point at.
fn block_request(game: &mut Game<'_>, player: u128, text: &str) {
    let v: Vec<i64> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [x, y, z, rest @ ..] = v.as_slice() else { return };
    let pos = [*x as i32, *y as i32, *z as i32];
    let details = rest.first().is_some_and(|d| *d != 0);
    let slot = rest.get(1).copied().unwrap_or(-1) as i32;
    REQUEST_HIT.with_borrow_mut(|h| *h = rest.get(2..5).map(|p| [p[0] as f64 / 4096.0, p[1] as f64 / 4096.0, p[2] as f64 / 4096.0]));
    // BlockAccessorImpl: within reach of the player only (a client can ask anything).
    if out_of_reach(game, player, pos.map(f64::from), 4.5) {
        return;
    }
    let block = game.native_block(pos);
    let mut lines = Vec::new();
    vanilla_server(game, player, pos, &block, details, slot, &mut lines);
    if let Ok(providers) = SERVER_PROVIDERS.lock().map(|v| v.clone()) {
        for p in providers {
            p(game, pos, &block, &mut lines);
        }
    }
    let mut out = format!("{x} {y} {z}");
    for l in lines {
        out.push('\n');
        out.push_str(&l.replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// A player asks about a chest minecart or boat (or a spawner minecart): ContainerEntityProvider and the rest.
fn vehicle_request(game: &mut Game<'_>, player: u128, text: &str) {
    let v: Vec<u32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [id, details] = v.as_slice() else { return };
    let details = *details != 0;
    SERVER_ITEMS.set(on(game, "item_storage"));
    let mut lines = Vec::new();
    if let Some(c) = game.carts.iter().find(|c| c.id == *id) {
        let at = [c.position.x, c.position.y, c.position.z];
        if out_of_reach(game, player, at, 3.0) {
            return;
        }
        // ItemStorageProvider.putData: a loot table not rolled yet.
        if c.loot.is_some() {
            lines.push(tr("jade.loot_not_generated", "§oWhat could it be?"));
        } else if on(game, "container_entity") {
            storage(game, details, &mut lines, c.items.iter().flatten());
        }
        // MobSpawnerProvider.ForEntity.
        if on(game, "mob_spawner")
            && let Some(s) = c.spawner.as_deref()
        {
            lines.extend(spawner_title(game, s));
        }
    } else if let Some(b) = game.boats.iter().find(|b| b.id == *id) {
        let at = [b.position.x, b.position.y, b.position.z];
        if out_of_reach(game, player, at, 3.0) {
            return;
        }
        if b.chest && on(game, "container_entity") {
            storage(game, details, &mut lines, b.items.iter().flatten());
        }
    } else {
        return;
    }
    let mut out = format!("m {id}");
    for l in lines {
        out.push('\n');
        out.push_str(&l.replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// A player asks for a mob's lines: its id, then Show Details, Villager Restock and entity stats as that player has them.
fn mob_request(game: &mut Game<'_>, player: u128, text: &str) {
    let v: Vec<u32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [id, details, restock, stats, growth, breeding, drop, cure] = v.as_slice() else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == *id) else { return };
    // EntityAccessorImpl: within reach of the player only (a client can ask anything).
    if out_of_reach(game, player, [m.position.x, m.position.y, m.position.z], 3.0) {
        return;
    }
    let mut out = format!("m {id}");
    for l in mob_lines(m, game.world.game_time, *details != 0, *restock != 0, *stats != 0, [*growth != 0, *breeding != 0, *drop != 0, *cure != 0]) {
        out.push('\n');
        out.push_str(&encode(&l).replace('\n', " "));
    }
    let mut extra = Vec::new();
    mob_server_lines(game, m, player, *details != 0, &mut extra);
    for l in extra {
        out.push('\n');
        out.push_str(&l.replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// What else the server tells about a mob, needing the game: its absorption and armour, a waxed copper golem, a chested
/// horse's or llama's items and its loot table.
fn mob_server_lines(game: &Game<'_>, m: &horizon_client::mobs::Mob, player: u128, details: bool, lines: &mut Vec<String>) {
    let kind = m.kind.id.as_ref();
    // EntityHealthAndArmorProvider.
    if !matches!(kind, "minecraft:armor_stand" | "minecraft:creaking") {
        lines.push(encode(&Line::new().with(El::Stats { absorb: m.absorption.max(0.0), armor: horizon_client::mobs::armor::values(m).0 })));
    }
    // WaxedProvider.EntityData (CopperGolem.nextWeatheringTick == -2).
    if kind == "minecraft:copper_golem" && on(game, "waxed") && m.extra.vector[0] == -2.0 {
        const WEATHER: [&str; 4] = ["", "exposed_", "weathered_", "oxidized_"];
        let mut l = Line::new().with(El::Icon(format!("minecraft:waxed_{}copper_golem_statue", WEATHER[usize::from(m.extra.variant.min(3))]))).with(El::Badge);
        if access(game, "access_entity") {
            l = l.with(El::Title(tr("jade.access.waxed", "Waxed {0}")));
        }
        lines.push(encode(&l));
    }
    // ItemStorageProvider through AbstractHorseAccess.
    SERVER_ITEMS.set(on(game, "item_storage"));
    if m.extra.chested {
        storage(game, details, lines, m.extra.items.iter().flatten());
    }
    // EntityLootTableProvider: for game masters.
    if on(game, "loot_table") && game.native_server_players().into_iter().find(|p| p.uuid == player).is_some_and(|p| p.permission >= 2) {
        let table = m.extra.death_loot_table.clone().unwrap_or_else(|| {
            let (ns, path) = kind.split_once(':').unwrap_or(("minecraft", kind));
            format!("{ns}:entities/{path}")
        });
        lines.push(trf("jade.lootTable", "Loot Table: %s", &[&table]));
    }
}

/// The lines of a mob that only the server knows.
fn mob_lines(m: &horizon_client::mobs::Mob, game_time: i64, details: bool, restock: bool, entity_stats: bool, [growth, breeding, drops, cure]: [bool; 4]) -> Vec<Line> {
    let mut lines = Vec::new();
    let kind = m.kind.id.as_ref();
    // MobGrowthProvider: a baby's growing time (AgeableMob.age counts up to 0).
    if growth && m.extra.age < 0 {
        let left = if m.extra.age_locked { tr("jade.mobgrowth.paused", "Paused") } else { seconds(-i64::from(m.extra.age)) };
        lines.push(Line::text(trf("jade.mobgrowth.time", "Growing time: %s", &[&format!("§f{left}")])));
    }
    // MobBreedingProvider: an allay's duplication cooldown; in love, or the breeding cooldown (age above 0).
    if breeding && kind == "minecraft:allay" {
        if m.extra.egg_time > 0 {
            lines.push(Line::text(trf("jade.mobduplication.time", "Duplication CD: %s", &[&format!("§f{}", seconds(i64::from(m.extra.egg_time)))])));
        }
    } else if breeding && m.extra.in_love > 0 {
        // One string in the language files, its colour where English's colon is.
        lines.push(Line::text(tr("jade.mobbreeding.fed", "Breeding CD: In Love").replacen(": ", ": §f", 1)));
    } else if breeding && m.extra.age > 0 {
        lines.push(Line::text(trf("jade.mobbreeding.time", "Breeding CD: %s", &[&format!("§f{}", seconds(i64::from(m.extra.age)))])));
    }
    // HorseStatsProvider: a llama's strength; a horse's jump height and speed (out of the most with Show Details).
    let stat = |key: &str, english: &str, v: f64, most: f64| {
        let v = trim2(v);
        let shown = if details { trf("jade.fraction", "%d / %d", &[&v, &trim2(most)]) } else { v };
        trf(key, english, &[&format!("§f{shown}")])
    };
    match kind {
        _ if !entity_stats => {}
        "minecraft:llama" | "minecraft:trader_llama" => lines.push(Line::text(stat("jade.llamaStrength", "Strength: %s", f64::from(m.extra.variant2.max(1)), 5.0))),
        "minecraft:horse" | "minecraft:donkey" | "minecraft:mule" | "minecraft:skeleton_horse" | "minecraft:zombie_horse" => {
            let jump = |j: f64| 4.53680079 * j * j + 1.6143173 * j - 0.22656224;
            let speed = |v: f64| v * 43.17181546666666 - 3.39999999E-7;
            lines.push(Line::text(stat("jade.horseStat.jump", "Jump height: %s", jump(m.attrs.jump_strength), jump(1.0))));
            lines.push(Line::text(stat("jade.horseStat.speed", "Speed: %s", speed(m.attrs.speed), speed(0.3375))));
        }
        _ => {}
    }
    // ZombieVillagerProvider: the time left to cure.
    if kind == "minecraft:zombie_villager"
        && cure
        && let Some(c) = horizon_client::mobs::zombie_villager::cure(m)
        && c.time > 0
    {
        lines.push(Line::text(trf("jade.zombieConversion.time", "Conversion time: %s", &[&format!("§f{}", seconds(i64::from(c.time)))])));
    }
    // NextEntityDropProvider: a grown chicken's next egg, an armadillo's next scute.
    if drops && matches!(kind, "minecraft:chicken" | "minecraft:armadillo") && !m.is_baby() && m.extra.egg_time < 48000 {
        let (key, english) = if kind == "minecraft:chicken" { ("jade.nextEgg", "Next egg: %s") } else { ("jade.nextScute", "Next scute: %s") };
        lines.push(Line::text(trf(key, english, &[&format!("§f{}", seconds(i64::from(m.extra.egg_time)))])));
    }
    // NextEntityDropProvider: a sniffer's dig cooldown (its sniff_cooldown memory).
    if drops && kind == "minecraft:sniffer" && !m.is_baby()
        && let Some(mem) = m.extra.animal_brain.as_deref().map(|b| &b.mem)
        && mem.sniff_cooldown.present()
    {
        let left = mem.sniff_cooldown.hz_jade_ttl();
        if left > 0 && left < 48000 {
            lines.push(Line::text(trf("jade.nextSniff", "Dig CD: %s", &[&format!("§f{}", seconds(left))])));
        }
    }
    // VillagerRestockProvider (off by default): ready, or when the next restock can be (two a day, 2400 ticks
    // apart, then the next day).
    if restock
        && kind == "minecraft:villager"
        && let Some(trade) = m.extra.trade.as_ref().filter(|t| t.offers.iter().any(|o| o.uses > 0))
    {
        let limit = trade.restocks_today >= 2;
        let interval: i64 = if limit { 12000 } else { 2400 };
        let left = trade.last_restock + interval - game_time;
        lines.push(Line::text(if left <= 0 {
            format!("§a{}", tr("jade.villagerRestock.ready", "Ready to restock"))
        } else if limit {
            trf("jade.villagerRestock.limit", "Restock limit reached, next in %s", &[&format!("§f{}", seconds(left))])
        } else {
            trf("jade.villagerRestock.time", "Restock in %s", &[&format!("§f{}", seconds(left))])
        }));
    }
    lines
}

/// `native_net_from_server` (client): a block's or a mob's lines.
pub fn from_server(channel: &str, data: &[u8]) {
    if channel != NET_DATA {
        return;
    }
    let text = String::from_utf8_lossy(data);
    // ShowOverlayPacket.
    if let Some(rest) = text.strip_prefix("o ") {
        CLIENT.with_borrow_mut(|c| c.hidden = rest.trim() != "1");
        return;
    }
    if let Some(rest) = text.strip_prefix("m ") {
        let mut it = rest.split('\n');
        let Some(id) = it.next().and_then(|i| i.parse().ok()) else { return };
        let lines: Vec<String> = it.map(str::to_owned).collect();
        CLIENT.with_borrow_mut(|c| c.mob_data = Some((id, lines, std::time::Instant::now())));
        return;
    }
    let mut it = text.split('\n');
    let v: Vec<i32> = it.next().unwrap_or("").split(' ').filter_map(|s| s.parse().ok()).collect();
    let [x, y, z] = v.as_slice() else { return };
    let lines: Vec<String> = it.map(str::to_owned).collect();
    CLIENT.with_borrow_mut(|c| c.data = Some(([*x, *y, *z], lines, std::time::Instant::now())));
}

thread_local! {
    /// The world's game rules were just read, and this thread's next opened level brings ours in.
    /// Jade.MAX_POSITION_DEVIATION in the world in play.
    static RULE: std::cell::Cell<i32> = const { std::cell::Cell::new(RULE_DEFAULT) };
}

/// `native_world_lifecycle`, a world opened: Jade.MAX_POSITION_DEVIATION from data/jade/game_rules.dat (Java keeps it in
/// game_rules.dat, whose file Horizon writes for vanilla alone), else its default.
pub fn rule_world_open(game: &mut Game<'_>) {
    use horizon_world::nbt::Tag;
    let saved = horizon_client::save::read_dat(&game.world_root.join("data/jade/game_rules.dat"));
    let value = match saved.as_ref().and_then(|t| t.get("data")).and_then(|d| d.get(RULE_NAME)) {
        Some(Tag::Int(v)) => *v,
        _ => game.native_setting(ID, "max_position_deviation").and_then(|v| v.parse().ok()).unwrap_or(RULE_DEFAULT),
    };
    RULE.set(value.clamp(0, 1000));
}

/// `/gamerule jade:max_position_deviation [value]`, Jade.registerGameRules' rule (an integer, 21, 0 to 1000). True when
/// `words` is that command. No mod can add a game rule to Horizon's table: its row isn't on the Game Rules screen.
fn rule_command(game: &mut Game<'_>, from: u128, words: &[&str], permission: u8) -> bool {
    let ["gamerule", name, rest @ ..] = words else { return false };
    if name.strip_prefix("minecraft:").unwrap_or(name) != RULE_NAME {
        return false;
    }
    let tell = |game: &mut Game<'_>, t: &str| {
        if from == 0 {
            game.native_feedback(horizon_client::text::Text::legacy(t));
        } else {
            game.native_tell(from, horizon_client::text::Text::legacy(t));
        }
    };
    if permission < 2 {
        tell(game, &tr("commands.help.failed", "Unknown command or insufficient permissions"));
        return true;
    }
    match rest {
        [] => tell(game, &trf("commands.gamerule.query", "Game rule %s is currently set to: %s", &[RULE_NAME, &RULE.get().to_string()])),
        [value] => match value.parse::<i32>() {
            Ok(v) if (0..=1000).contains(&v) => {
                if v == RULE.get() {
                    tell(game, &trf("commands.gamerule.not_set", "Game rule %s was already set to: %s", &[RULE_NAME, value]));
                    return true;
                }
                RULE.set(v);
                use horizon_world::nbt::Tag;
                // Its own file: "an unknown key makes [vanilla] reject" game_rules.dat.
                let data = Tag::compound([(RULE_NAME, Tag::Int(v))]);
                let tag = Tag::compound([("data", data), ("DataVersion", Tag::Int(horizon_core::version::DATA_VERSION))]);
                let _ = horizon_client::save::write_dat(&game.world_root.join("data/jade/game_rules.dat"), tag);
                tell(game, &trf("commands.gamerule.set", "Game rule %s is now set to: %s", &[RULE_NAME, value]));
            }
            _ => tell(game, "\u{a7}cThe value must be a whole number from 0 to 1000"),
        },
        _ => tell(game, "/gamerule jade:max_position_deviation [0-1000]"),
    }
    true
}
