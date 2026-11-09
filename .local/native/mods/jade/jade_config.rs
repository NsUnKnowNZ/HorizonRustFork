// Jade's config screens: HomeConfigScreen, WailaConfigScreen, PluginsConfigScreen and ProfileConfigScreen, with the
// OptionsList, OptionsNav, BaseOptionsScreen and PreviewOptionsScreen they stand on, as one mod-drawn screen. The
// settings API stays the store: a button writes the same setting `native_settings_click` does, at once (so `Done`, not
// Java's Save & Quit and Cancel), and a slider or a dragged overlay when it is let go.
//
// It opens as the mod's own screen over the world (`native_mod_screen_open`), or drawn over the Options screen from Jade's
// page in Options > Mods: a mod's own screen can't open there, nor over the title, and that page can be reached from
// both. Options draws no hud, so with a world behind it `frame` builds the preview's box; with none there is no preview
// (as Java has none without a level).
//
// The screen hooks have only `&Game`: settings can be written from them, what needs more waits for `frame`.
//
// Jade's GUI sprites aren't in the atlas (jade_theme.rs), so its navigation bar, search box, cards, popup and toggles are
// drawn with fills of their PNGs' colours, the jar's `high_contrast/` versions while the high_contrast pack is on. Those
// three sprites are all that folder replaces. The buttons, sliders and scrollbar are vanilla's own sprites, as Java's are.
//
// Its words are the jar's language files (`jade::ensure_lang`), keys named beside each use.
use crate::game::Game;
use crate::game::jade::{self, ID};
use crate::game::jade_theme;
use crate::game::native_hooks::NativeScreenView;
use crate::game::jade::tr;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::ui;
use std::cell::RefCell;

/// BaseOptionsScreen: the navigation column's width, and what's left under the list.
const NAV_W: f32 = 120.0;
const FOOTER: f32 = 32.0;
/// BaseOptionsScreen's search box and OptionsNav's rows.
const SEARCH_H: f32 = 18.0;
const NAV_ROW: f32 = 18.0;
/// OptionsList's rows.
const ROW: f32 = 26.0;
/// OptionsList.OPTION_ON and OPTION_OFF.
const ON_COLOR: u32 = 0xB9_F6CA;
const OFF_COLOR: u32 = 0xFF_8A80;
/// ProfileEntry's name while it is the active profile, and otherwise.
const ACTIVE_COLOR: u32 = 0xFF_FF55;
const NAME_COLOR: u32 = 0xE0_E0E0;
/// OptionsList's card mode.
const CARD_GAP: f32 = 8.0;
const CARD_HEIGHT: f32 = 42.0;
const CARD_HEIGHT_TALL: f32 = 46.0;
const CARD_BLEED: f32 = 3.0;
const MIN_CARD_WIDTH: f32 = 168.0;
const MAX_CARDS_PER_ROW: f32 = 3.0;
/// The toggle a card has for a Boolean option (ToggleButton).
const TOGGLE_W: f32 = 29.0;
const TOGGLE_H: f32 = 17.0;

/// What a page is made of, in WailaConfigScreen's and PluginsConfigScreen's order.
#[derive(Clone, Copy)]
enum Item {
    /// A section, by its title's language key.
    Head(&'static str),
    Danger(&'static str),
    /// An option, how deep under the one above it (OptionValue.parent).
    Opt(&'static str, u8),
    /// A numeric option: its depth, least, most, step, and whether it reads in percent.
    Slide(&'static str, u8, f32, f32, f32, bool),
    /// One of JadeKeys, by name.
    Key(&'static str),
    Adjust,
    Reload,
    Reset,
}
use Item::{Adjust, Danger, Head, Key, Opt, Reload, Reset, Slide};

/// WailaConfigScreen.createOptions. The options this port keeps that Java's screen doesn't show are in their sections:
/// no setting is left without a row.
const MOD_PAGE: &[Item] = &[
    Head("config.jade.general"),
    Opt("debug_mode", 0),
    Opt("display_tooltip", 0),
    Opt("display_entities", 0),
    Opt("display_bosses", 1),
    Opt("display_blocks", 0),
    Opt("display_fluids", 1),
    Opt("builtin_camouflage", 1),
    Opt("display_mode", 0),
    Opt("item_mod_name", 0),
    Opt("hide_from_guis", 0),
    Opt("hide_from_tab_list", 0),
    Opt("boss_bar_overlap", 0),
    Slide("reach_distance", 0, 0.0, 20.0, 0.5, false),
    Opt("perspective_mode", 0),
    Head("config.jade.overlay"),
    Adjust,
    Opt("overlay_x", 1),
    Opt("position", 1),
    Opt("overlay_theme", 0),
    Opt("theme_style", 1),
    Slide("overlay_alpha", 0, 0.0, 100.0, 1.0, true),
    Slide("scale", 0, 0.2, 2.0, 0.01, false),
    Opt("display_item", 0),
    Opt("animation", 0),
    Head("config.jade.key_binds"),
    Key("config"),
    Key("show_overlay"),
    Key("toggle_liquid"),
    Key("show_recipes"),
    Key("show_uses"),
    Key("narrate"),
    Key("show_details"),
    Head("config.jade.accessibility"),
    Opt("accessibility_plugin", 0),
    Opt("tts_mode", 0),
    Opt("narrate_keys", 0),
    Opt("always_list_mode", 0),
    Slide("text_background_opacity", 0, 0.0, 100.0, 1.0, true),
    Opt("flip_main_hand", 0),
    Danger("config.jade.danger_zone"),
    Reload,
    Reset,
];

/// The plugin options of Jade's own plugins, by WailaClientRegistration's categories. The ranges are VanillaPlugin's
/// `addConfig`s.
const PLUGIN_PAGE: &[Item] = &[
    Head("config.jade.plugin_jade"),
    Opt("object_name", 0),
    Opt("mod_name", 0),
    Opt("mod_name_smaller", 1),
    Opt("mod_name_translated", 1),
    Opt("distance", 0),
    Opt("coordinates", 0),
    Opt("coordinates_rel", 1),
    Opt("block_face", 0),
    Opt("registry_name", 0),
    Opt("registry_special", 1),
    Opt("block_states", 0),
    Opt("block_properties", 0),
    Opt("loot_table", 0),
    Head("config.jade.plugin_minecraft.entity"),
    Opt("entity_health", 0),
    Slide("health_max", 1, 0.0, 200.0, 1.0, false),
    Slide("health_per_line", 1, 5.0, 40.0, 1.0, false),
    Opt("health_fractions", 1),
    Opt("entity_armor", 0),
    Slide("armor_max", 1, 0.0, 200.0, 1.0, false),
    Opt("entity_stats", 0),
    Opt("potion_effects", 0),
    Slide("effects_limit", 1, 1.0, 99.0, 1.0, false),
    Opt("item_frame", 0),
    Opt("container_entity", 0),
    Opt("mob_growth", 0),
    Opt("mob_breeding", 0),
    Opt("armor_stand", 0),
    Opt("painting", 0),
    Opt("next_entity_drop", 0),
    Opt("animal_owner", 0),
    Opt("sulfur_cube", 0),
    Opt("villager_profession", 0),
    Opt("villager_restock", 0),
    Opt("zombie_villager", 0),
    Opt("pet_armor", 0),
    Opt("pet_armor_mode", 1),
    Opt("item_tooltip", 0),
    Head("config.jade.plugin_minecraft.block"),
    Opt("breaking_progress", 0),
    Opt("harvest_tool", 0),
    Opt("harvest_new_line", 1),
    Opt("harvest_effective", 1),
    Opt("harvest_unbreakable", 1),
    Opt("harvest_creative", 1),
    Opt("brewing_stand", 0),
    Opt("tnt_stability", 0),
    Opt("beehive", 0),
    Opt("note_block", 0),
    Opt("command_block", 0),
    Opt("enchantment_power", 0),
    Opt("total_enchantment_power", 0),
    Opt("player_head", 0),
    Opt("item_storage", 0),
    Slide("storage_detailed", 1, 0.0, 54.0, 1.0, false),
    Slide("storage_normal", 1, 0.0, 54.0, 1.0, false),
    Slide("storage_names", 1, 0.0, 9.0, 1.0, false),
    Slide("storage_per_line", 1, 3.0, 27.0, 1.0, false),
    Opt("storage_sort", 1),
    Opt("energy_storage", 0),
    Opt("energy_detailed", 1),
    Opt("energy_style", 1),
    Opt("fluid_storage", 0),
    Opt("fluid_detailed", 1),
    Opt("fluid_style", 1),
    Opt("lectern", 0),
    Opt("furnace", 0),
    Opt("mob_spawner", 0),
    Opt("crop_progress", 0),
    Opt("redstone", 0),
    Opt("jukebox", 0),
    Opt("shelf", 0),
    Opt("waxed", 0),
];

/// `getConfigListView`: the access providers only with the Accessibility Plugin on.
const ACCESS_PAGE: &[Item] = &[
    Head("config.jade.plugin_jade_access"),
    Opt("access_sign", 0),
    Opt("access_block", 0),
    Opt("access_entity", 0),
    Opt("access_variant", 0),
    Opt("access_held", 0),
    Opt("access_npc", 0),
];

/// What a plugin option's card shows: the provider's `getConfigIcon`, or the ConfigIcon of the plugin's `addConfig` (distance,
/// coordinates, mod name, registry name, breaking progress, health, armour, pet armour). Only a primary key can have one
/// (ConfigEntry.setIcon), and all of Jade's are items: nothing it has calls `ConfigIcon.sprite`.
fn config_icon(key: &str) -> Option<&'static str> {
    Some(match key {
        "object_name" | "mod_name" | "registry_name" | "access_npc" => "minecraft:name_tag",
        "distance" => "minecraft:spyglass",
        "coordinates" | "block_face" => "minecraft:compass",
        "block_states" => "minecraft:debug_stick",
        "block_properties" => "minecraft:comparator",
        "loot_table" | "item_storage" => "minecraft:chest",
        "entity_health" => "minecraft:golden_apple",
        "entity_armor" => "minecraft:iron_chestplate",
        "entity_stats" => "minecraft:saddle",
        // StatusEffectsProvider.Client: a Potion of Healing (`draw_card`).
        "potion_effects" => "minecraft:potion",
        "item_frame" => "minecraft:item_frame",
        "mob_growth" | "mob_breeding" | "crop_progress" => "minecraft:wheat",
        "armor_stand" => "minecraft:armor_stand",
        "painting" => "minecraft:painting",
        "next_entity_drop" => "minecraft:egg",
        "animal_owner" => "minecraft:lead",
        "sulfur_cube" => "minecraft:sulfur",
        "villager_profession" | "villager_restock" => "minecraft:emerald",
        "zombie_villager" => "minecraft:rotten_flesh",
        "pet_armor" => "minecraft:wolf_armor",
        "item_tooltip" => "minecraft:paper",
        "breaking_progress" => "minecraft:iron_pickaxe",
        "harvest_tool" => "minecraft:diamond_pickaxe",
        "brewing_stand" => "minecraft:brewing_stand",
        "tnt_stability" => "minecraft:tnt",
        "beehive" => "minecraft:beehive",
        "note_block" => "minecraft:note_block",
        "command_block" => "minecraft:command_block",
        "enchantment_power" | "total_enchantment_power" => "minecraft:enchanting_table",
        "player_head" => "minecraft:player_head",
        "energy_storage" => "minecraft:redstone_block",
        "fluid_storage" => "minecraft:water_bucket",
        "lectern" => "minecraft:lectern",
        "furnace" => "minecraft:furnace",
        "mob_spawner" => "minecraft:spawner",
        "redstone" => "minecraft:redstone",
        "jukebox" => "minecraft:jukebox",
        "shelf" => "minecraft:chiseled_bookshelf",
        "waxed" => "minecraft:honeycomb",
        "access_sign" => "minecraft:oak_sign",
        "access_block" | "access_entity" => "minecraft:book",
        "access_variant" => "minecraft:blue_egg",
        "access_held" => "minecraft:diamond_sword",
        _ => return None,
    })
}

/// The option's `_desc` and `_extra_msg` (`gamerule.*.description` for the one that is a game rule in Java).
fn help(key: &str) -> (String, String) {
    let Some(lang) = jade::option_lang_key(key) else { return Default::default() };
    let desc = if lang.starts_with("gamerule.") { format!("{lang}.description") } else { format!("{lang}_desc") };
    (tr(&desc, ""), tr(&format!("{lang}_extra_msg"), ""))
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Mod,
    Plugins,
    Profiles,
}

#[derive(Clone, Copy, PartialEq)]
enum Host {
    /// The mod's own screen over the world.
    Mod,
    /// Drawn over the Options screen, its input taken.
    Options,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    None,
    Search,
    Profile(usize),
    /// A row of the list or a card, by its index in `rows` (the arrows and Tab move it; Enter uses it).
    Row(usize),
    /// A section of the navigation column, by its place there.
    Nav(usize),
    /// A button of HomeConfigScreen.
    Home(usize),
}

/// What ConfirmScreen asks.
#[derive(Clone, Copy, PartialEq)]
enum Confirm {
    Reset,
    SaveProfile(usize),
}

#[derive(Clone, Copy)]
enum Drag {
    None,
    /// A slider, by its index in `rows`, and the track's left edge and width.
    Slider(usize, [f32; 2]),
    /// The scrollbar, and where on its handle it was taken.
    Bar(f32),
    /// The overlay, and the pointer's offset from its centre (PreviewOptionsScreen.dragOffset).
    Overlay([f32; 2]),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Title,
    Toggle,
    Cycle,
    Theme,
    Style,
    Slider { min: f32, max: f32, step: f32, percent: bool },
    /// JadeKeys, by index in `jade::KEYS`.
    Keybind(usize),
    /// `overlay_pos`: Adjust...
    Adjust,
    Reload,
    Reset,
    /// ProfileEntry.
    Profile(usize),
}

struct Row {
    kind: Kind,
    label: String,
    key: &'static str,
    desc: String,
    /// OptionsList.Entry.getMessages, lower case: what the search looks in.
    words: String,
    parent: Option<usize>,
    /// `secondaryOptions()`: the rows under it.
    children: Vec<usize>,
    /// `isSecondary()`: in card mode it lives in its card's popup.
    secondary: bool,
    depth: u8,
    /// `editIgnoreList`'s button on the row.
    ignore: bool,
    danger: bool,
    /// `OptionsList.Entry.icon`, an item: drawn on its card, the title moved over for it.
    icon: Option<&'static str>,
}

/// An EditBox's text and caret (in characters).
#[derive(Default)]
struct Field {
    text: String,
    caret: usize,
}

impl Field {
    fn set(&mut self, text: String) {
        self.caret = text.chars().count();
        self.text = text;
    }
    fn byte_at(&self, chars: usize) -> usize {
        self.text.char_indices().nth(chars).map_or(self.text.len(), |(i, _)| i)
    }
    fn insert(&mut self, c: char, most: usize) {
        if self.text.chars().count() < most {
            let at = self.byte_at(self.caret);
            self.text.insert(at, c);
            self.caret += 1;
        }
    }
    /// One of the keys an EditBox moves or deletes with. Whether it changed the text.
    fn key(&mut self, key: &str) -> bool {
        match key {
            "key.keyboard.backspace" if self.caret > 0 => {
                self.caret -= 1;
                let at = self.byte_at(self.caret);
                self.text.remove(at);
                true
            }
            "key.keyboard.delete" if self.caret < self.text.chars().count() => {
                let at = self.byte_at(self.caret);
                self.text.remove(at);
                true
            }
            "key.keyboard.left" => {
                self.caret = self.caret.saturating_sub(1);
                false
            }
            "key.keyboard.right" => {
                self.caret = (self.caret + 1).min(self.text.chars().count());
                false
            }
            "key.keyboard.home" => {
                self.caret = 0;
                false
            }
            "key.keyboard.end" => {
                self.caret = self.text.chars().count();
                false
            }
            _ => false,
        }
    }
}

/// What waits for `frame`, which has the game's `&mut`.
enum Action {
    Close,
    Sound,
    SetKey(&'static str, String),
    ResetKeys,
    OpenIgnoreLists,
}

struct Ui {
    host: Host,
    page: Page,
    rows: Vec<Row>,
    /// `children()`: the rows the search leaves, as indexes in `rows`.
    shown: Vec<usize>,
    scroll: f32,
    nav_scroll: f32,
    search: Field,
    names: [Field; 4],
    focus: Focus,
    drag: Drag,
    /// `OptionsList.selectedKey`: the key waiting for a key, by index in `jade::KEYS`.
    binding: Option<usize>,
    /// PreviewOptionsScreen.adjustingPosition.
    adjusting: bool,
    /// `OverlayRenderer.animation.expectedRect`.
    rect: [f32; 4],
    /// History.previewOverlay.
    preview: bool,
    confirm: Option<Confirm>,
    sample: Option<jade::BuiltBox>,
    /// The window in GUI pixels, as last drawn.
    size: [f32; 2],
    /// A mouse button went down on this screen and hasn't come up.
    took_press: bool,
    opened: std::time::Instant,
    /// OptionsList.DisplayMode.CARD: the plugin page, wide enough, without Always Use List Mode.
    cards: bool,
    /// `layoutCards`: each of `shown`'s rectangle, its y from the content's top; no height for what isn't laid out.
    card_rects: Vec<[f32; 4]>,
    card_height: f32,
    /// SecondaryPopup.
    expanded: Option<usize>,
    fading: Option<usize>,
    sticky: bool,
    pending: Option<(usize, [f32; 2], std::time::Instant)>,
    popup_alpha: f32,
    popup_slide: f32,
    popup_at: std::time::Instant,
    /// `Minecraft.getLastInputType().isKeyboard()`: what's focused shows as hovered, not what's under the pointer.
    keyboard: bool,
    last_mouse: [f32; 2],
    /// AbstractSliderButton.canChangeValue: Enter on a focused slider lets the arrows move it.
    slider_keys: bool,
    /// ToggleButton.pressed: the row whose toggle is held.
    pressed: Option<usize>,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static PENDING: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
}

fn queue(action: Action) {
    PENDING.with_borrow_mut(|p| p.push(action));
}

fn click_sound() {
    queue(Action::Sound);
}

fn hit([x, y, w, h]: [f32; 4], [mx, my]: [f32; 2]) -> bool {
    mx >= x && mx < x + w && my >= y && my < y + h
}

/// A config screen is up.
pub fn is_open() -> bool {
    UI.with_borrow(|u| u.is_some())
}

/// A page with the preview is up, where `hud` can build the box for it (PreviewOptionsScreen).
pub fn wants_box() -> bool {
    UI.with_borrow(|u| u.as_ref().is_some_and(|ui| matches!(ui.page, Page::Mod | Page::Plugins)))
}

fn new_ui(host: Host) -> Ui {
    Ui {
        host,
        page: Page::Home,
        rows: Vec::new(),
        shown: Vec::new(),
        scroll: 0.0,
        nav_scroll: 0.0,
        search: Field::default(),
        names: Default::default(),
        focus: Focus::None,
        drag: Drag::None,
        binding: None,
        adjusting: false,
        rect: [0.0; 4],
        preview: true,
        confirm: None,
        sample: None,
        size: [0.0; 2],
        took_press: false,
        opened: std::time::Instant::now(),
        cards: false,
        card_rects: Vec::new(),
        card_height: 0.0,
        expanded: None,
        fading: None,
        sticky: false,
        pending: None,
        popup_alpha: 0.0,
        popup_slide: 0.0,
        popup_at: std::time::Instant::now(),
        keyboard: false,
        last_mouse: [0.0; 2],
        slider_keys: false,
        pressed: None,
    }
}

/// `/jadec config` and Jade's menu key: HomeConfigScreen over the world. False where the game won't have it.
pub fn open_in_world(game: &mut Game<'_>) -> bool {
    if !game.native_mod_screen_open(ID) {
        return false;
    }
    jade::ensure_lang();
    let mut ui = new_ui(Host::Mod);
    ui.preview = game.native_setting(ID, "preview_overlay").is_none_or(|v| v != "false");
    UI.with_borrow_mut(|u| *u = Some(ui));
    true
}

/// The Jade page's button in Options > Mods: HomeConfigScreen over that screen.
pub fn open_over_options(game: &Game<'_>) {
    jade::ensure_lang();
    let mut ui = new_ui(Host::Options);
    ui.preview = game.native_setting(ID, "preview_overlay").is_none_or(|v| v != "false");
    UI.with_borrow_mut(|u| *u = Some(ui));
}

/// `native_frame`: what the hooks couldn't do, and the screen's end when something else took its place. One borrow a
/// frame while closed.
pub fn frame(game: &mut Game<'_>) {
    let Some(host) = UI.with_borrow(|u| u.as_ref().map(|ui| ui.host)) else {
        return;
    };
    let up = match host {
        Host::Mod => game.native_mod_screen() == Some(ID),
        Host::Options => game.screen == crate::game::Screen::Options,
    };
    if !up {
        finish(game);
        return;
    }
    let actions = PENDING.with_borrow_mut(std::mem::take);
    for action in actions {
        match action {
            Action::Close => {
                finish(game);
                match host {
                    Host::Mod => game.native_mod_screen_close(ID),
                    // The page under it says what the options are now.
                    Host::Options => {
                        if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == ID) {
                            game.native_settings_refresh(i);
                        }
                    }
                }
                return;
            }
            Action::Sound => game.audio.play_button_click(),
            Action::SetKey(name, key) => {
                game.settings.mods.keys.entry(ID.to_owned()).or_default().insert(name.to_owned(), key);
                game.settings.save(&Game::options_path());
            }
            Action::ResetKeys => {
                game.settings.mods.keys.remove(ID);
                game.settings.save(&Game::options_path());
            }
            Action::OpenIgnoreLists => jade::open_ignore_lists(game),
        }
    }
    // Options draws no hud: with a world behind it, the preview's box is built here (the batch goes unused: `hud` keeps
    // the box for `draw_preview` while a page wants it).
    if host == Host::Options && game.player.is_some() && wants_box() {
        let mut unused = HudBatch::new([1, 1]);
        jade::hud(game, &mut unused);
    }
}

/// The screen goes: what was still moving is kept.
fn finish(game: &Game<'_>) {
    let ui = UI.with_borrow_mut(|u| u.take());
    PENDING.with_borrow_mut(|p| p.clear());
    if let Some(mut ui) = ui {
        ui.let_go(game);
        if ui.adjusting {
            ui.exit_adjusting(game);
        }
    }
}

/// `f` on the screen when it is the one up.
fn with_ui<R>(game: &Game<'_>, screen_id: &str, f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    if !is_open() {
        return None;
    }
    UI.with_borrow_mut(|u| {
        let ui = u.as_mut()?;
        let up = match ui.host {
            Host::Mod => screen_id == "mod" && game.native_mod_screen() == Some(ID),
            Host::Options => screen_id == "options",
        };
        up.then(|| f(ui))
    })
}

/// `native_screen_render_post`.
pub fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    with_ui(game, screen.id, |ui| ui.draw(game, batch, screen.mouse));
}

/// `native_screen_mouse_button_pressed_pre`: every press on the screen is its own.
pub fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    let taken = with_ui(game, screen.id, |ui| {
        ui.took_press = true;
        ui.press(game, screen.mouse, button, modifiers);
    });
    if taken.is_some() {
        *canceled = true;
    }
}

/// `native_screen_mouse_button_released_pre`: a release is taken only for a press that was (the click on Options that
/// opened the screen ends on that screen's own terms).
pub fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, canceled: &mut bool) {
    let took = with_ui(game, screen.id, |ui| {
        ui.let_go(game);
        std::mem::take(&mut ui.took_press)
    });
    if took == Some(true) {
        *canceled = true;
    }
}

/// `native_screen_mouse_scrolled_pre`.
pub fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2], modifiers: u32, canceled: &mut bool) {
    if with_ui(game, screen.id, |ui| ui.wheel(screen.mouse, scroll[1] as f32, modifiers)).is_some() {
        *canceled = true;
    }
}

/// `native_screen_key_pressed_pre`.
pub fn key(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, repeat: bool, modifiers: u32, canceled: &mut bool) {
    if with_ui(game, screen.id, |ui| ui.key(game, key, repeat, modifiers)).is_some() {
        *canceled = true;
    }
}

/// `native_screen_char_typed_pre`.
pub fn typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if with_ui(game, screen.id, |ui| ui.typed(game, c)).is_some() {
        *canceled = true;
    }
}

/// Builds a page's rows: OptionsList.add, with `parent` as `setDefaultParent` and `OptionValue.parent` leave it.
#[derive(Default)]
struct Builder {
    rows: Vec<Row>,
    head: Option<usize>,
    primary: Option<usize>,
}

impl Builder {
    fn push(&mut self, kind: Kind, label: String, key: &'static str, depth: u8) -> &mut Row {
        let (desc, extra) = help(key);
        let secondary = kind != Kind::Title && depth > 0 && self.primary.is_some();
        let parent = if kind == Kind::Title {
            None
        } else if depth == 0 {
            self.head
        } else {
            self.primary.or(self.head)
        };
        let index = self.rows.len();
        match kind {
            Kind::Title => {
                self.head = Some(index);
                self.primary = None;
            }
            _ if depth == 0 => self.primary = Some(index),
            _ => {}
        }
        let words = format!("{label} {key} {extra} {desc}").to_lowercase();
        if let Some(p) = parent {
            self.rows[p].children.push(index);
        }
        self.rows.push(Row { kind, label, key, desc, words, parent, children: Vec::new(), secondary, depth, ignore: false, danger: false, icon: config_icon(key) });
        &mut self.rows[index]
    }

    fn items(&mut self, items: &[Item]) {
        for item in items {
            match *item {
                Head(lang) => {
                    self.push(Kind::Title, tr(lang, lang), "", 0);
                }
                Danger(lang) => {
                    self.push(Kind::Title, tr(lang, lang), "", 0).danger = true;
                }
                Opt(key, depth) => {
                    let Some(o) = jade::option(key) else { continue };
                    let kind = match key {
                        "overlay_theme" => Kind::Theme,
                        "theme_style" => Kind::Style,
                        _ if o.values.len() == 2 && o.values[0].0 == "true" && o.values[1].0 == "false" => Kind::Toggle,
                        _ => Kind::Cycle,
                    };
                    self.push(kind, jade::option_label(o), o.key, depth).ignore = matches!(key, "display_entities" | "display_blocks");
                }
                Slide(key, depth, min, max, step, percent) => {
                    let Some(o) = jade::option(key) else { continue };
                    self.push(Kind::Slider { min, max, step, percent }, jade::option_label(o), o.key, depth);
                }
                Key(name) => {
                    let Some(i) = jade::KEYS.iter().position(|k| k[0] == name) else { continue };
                    let label = tr(&format!("key.jade.{name}"), jade::KEYS[i][1]);
                    // WailaConfigScreen: the two recipe keys say what they do.
                    let row = self.push(Kind::Keybind(i), label, "", 0);
                    row.desc = tr(&format!("gui.jade.{name}.desc"), "");
                }
                Adjust => {
                    self.push(Kind::Adjust, tr("config.jade.overlay_pos", "Overlay Position"), "", 0);
                }
                Reload => {
                    self.push(Kind::Reload, tr("config.jade.reload_plugins", "Reload Plugins"), "", 0);
                }
                Reset => {
                    self.push(Kind::Reset, tr("config.jade.reset_settings", "Reset Settings"), "", 0).danger = true;
                }
            }
        }
    }
}

/// `plugin_ae2.power_storage` or `ae2_power_storage`: the mod a plugin's option belongs to.
fn option_namespace(key: &str) -> &str {
    let key = key.strip_prefix("plugin_").unwrap_or(key);
    key.split(['.', '_']).next().unwrap_or(key)
}

fn build(game: &Game<'_>, page: Page) -> Vec<Row> {
    let mut b = Builder::default();
    match page {
        Page::Home => {}
        Page::Mod => b.items(MOD_PAGE),
        Page::Plugins => {
            b.items(PLUGIN_PAGE);
            if jade::on(game, "accessibility_plugin") {
                b.items(ACCESS_PAGE);
            }
            // PluginsConfigScreen: each other mod's options under its name (IWailaClientRegistration.addConfig).
            let mut plugins = jade::plugin_options();
            plugins.sort_by_key(|(key, _, _)| option_namespace(key).to_owned());
            let mut last = None;
            for (key, label, _) in plugins {
                let ns = option_namespace(key);
                if last != Some(ns) {
                    b.push(Kind::Title, jade::mod_name(game, ns), "", 0);
                    last = Some(ns);
                }
                b.push(Kind::Toggle, label.to_owned(), key, 0);
            }
        }
        // ProfileConfigScreen.createOptions.
        Page::Profiles => {
            b.push(Kind::Title, tr("config.jade.profiles", "Profiles"), "", 0);
            b.push(Kind::Toggle, tr("config.jade.enable_profiles", "Enabled"), "enable_profiles", 0);
            for i in 0..4 {
                b.push(Kind::Profile(i), tr(&format!("config.jade.profile.{i}"), &format!("#{i}")), "", 0);
            }
            b.push(Kind::Title, tr("config.jade.key_binds", "Key Binds"), "", 0);
            for name in ["profile.0", "profile.1", "profile.2", "profile.3"] {
                b.items(&[Key(name)]);
            }
        }
    }
    b.rows
}

/// JadeClient.registerBuiltInVariables: a description's `${...}` as the keys they name.
fn fill_variables(game: &Game<'_>, text: &str) -> String {
    let mut out = text.to_owned();
    for (variable, name) in [("${SHOW_DETAILS}", "show_details"), ("${SHOW_OVERLAY}", "show_overlay")] {
        if out.contains(variable) {
            out = out.replace(variable, &key_label(game, name));
        }
    }
    out
}

fn key_label(game: &Game<'_>, name: &str) -> String {
    let default = jade::KEYS.iter().find(|k| k[0] == name).map_or("key.keyboard.unknown", |k| k[2]);
    horizon_client::keybinds::display_name(&game.native_mod_key(ID, name, default)).plain()
}

/// Font.split: a line's words broken to `most` pixels.
fn wrap(game: &Game<'_>, text: &str, most: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let longer = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if game.native_text_width(&longer) > most && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = longer;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `calculateAnchor`.
fn calculate_anchor(center: f32, size: f32, rect_size: f32) -> f32 {
    let anchor = center / size;
    if anchor < 0.25 {
        return 0.0;
    }
    if anchor > 0.75 {
        return 1.0;
    }
    let half = rect_size / 2.0;
    let tolerance = (half / 2.0 - 3.0).min(15.0);
    if (center + half - size / 2.0).abs() < tolerance {
        1.0
    } else if (center - half - size / 2.0).abs() < tolerance {
        0.0
    } else {
        0.5
    }
}

/// `maybeSnap`.
fn maybe_snap(value: f32, snap: bool) -> f32 {
    if snap && value > 0.475 && value < 0.525 { 0.5 } else { value }
}

/// `accessibility.tryFlip`.
fn try_flip(game: &Game<'_>, value: f32) -> f32 {
    if jade::on(game, "flip_main_hand") && game.settings.main_hand_left { 1.0 - value } else { value }
}

fn modifier_down(game: &Game<'_>, left: winit::keyboard::KeyCode, right: winit::keyboard::KeyCode) -> bool {
    game.physical_keys.contains(&left) || game.physical_keys.contains(&right)
}

/// The list's rectangle, and the rows' left edge and width (OptionsList.getRowLeft and getRowWidth).
fn list_rect([w, h]: [f32; 2]) -> [f32; 4] {
    [NAV_W, 0.0, (w - NAV_W).max(0.0), (h - FOOTER).max(0.0)]
}

fn row_span(list: [f32; 4]) -> (f32, f32) {
    let width = (list[2] - 12.0).clamp(0.0, 320.0);
    (list[0] + ((list[2] - width) / 2.0).floor(), width)
}

/// Where a row's widgets are at height `y`: the main one (`getWidgetZone`), `editIgnoreList`'s and the reset button.
struct RowRects {
    value: [f32; 4],
    ignore: [f32; 4],
    reset: [f32; 4],
    name: [f32; 4],
    use_it: [f32; 4],
    save: [f32; 4],
}

fn row_rects(left: f32, width: f32, y: f32, ignore: bool) -> RowRects {
    let right = left + width - 4.0;
    let value_x = right - 22.0 - 100.0;
    RowRects {
        value: [value_x, y + 3.0, if ignore { 79.0 } else { 100.0 }, 20.0],
        ignore: [value_x + 80.0, y + 3.0, 20.0, 20.0],
        reset: [right - 20.0, y + 3.0, 20.0, 20.0],
        name: [left + 6.0, y + 3.0, (width - 118.0).clamp(0.0, 150.0), 20.0],
        use_it: [left + width - 110.0, y + 3.0, 48.0, 20.0],
        save: [left + width - 58.0, y + 3.0, 48.0, 20.0],
    }
}

/// HomeConfigScreen.init's buttons: mod settings, plugin settings, profiles, done.
fn home_rects([w, h]: [f32; 2]) -> [[f32; 4]; 4] {
    let wide = 110.0f32.min((w / 2.0 - 40.0).max(60.0));
    [
        [(w / 2.0).floor() - 5.0 - wide, (h / 2.0).floor() - 10.0, wide, 20.0],
        [(w / 2.0).floor() + 5.0, (h / 2.0).floor() - 10.0, wide, 20.0],
        [(w / 2.0).floor() + 10.0 + wide, (h / 2.0).floor() - 10.0, 20.0, 20.0],
        [(w / 2.0).floor() - 50.0, (h / 2.0).floor() + 20.0, 100.0, 20.0],
    ]
}

/// BaseOptionsScreen's button and PreviewOptionsScreen's.
fn done_rect([w, h]: [f32; 2]) -> [f32; 4] {
    [w - 100.0, h - 25.0, 90.0, 20.0]
}

fn preview_rect([_, h]: [f32; 2]) -> [f32; 4] {
    [10.0, h - 25.0, 85.0, 20.0]
}

/// ConfirmScreen's yes and no.
fn confirm_rects([w, h]: [f32; 2]) -> ([f32; 4], [f32; 4]) {
    let (cx, cy) = ((w / 2.0).floor(), (h / 2.0).floor());
    ([cx - 105.0, cy + 24.0, 100.0, 20.0], [cx + 5.0, cy + 24.0, 100.0, 20.0])
}

impl Ui {
    /// Another screen of the set (`setScreen`): its rows, its own search.
    fn enter(&mut self, game: &Game<'_>, page: Page) {
        self.page = page;
        self.rows = build(game, page);
        self.scroll = 0.0;
        self.nav_scroll = 0.0;
        self.search = Field::default();
        self.focus = Focus::None;
        self.binding = None;
        self.drag = Drag::None;
        self.confirm = None;
        self.cards = self.wants_cards(game);
        self.reset_popup();
        self.slider_keys = false;
        self.filter();
        if page == Page::Profiles {
            for (i, field) in self.names.iter_mut().enumerate() {
                // ProfileEntry: the name kept, or Jade.getProfilePreset's.
                let preset = jade::option("profile").and_then(|o| o.values.get(i)).map_or("", |(_, l)| *l);
                field.set(game.native_setting(ID, &format!("profile_name_{i}")).unwrap_or_else(|| preset.to_owned()));
            }
        }
    }

    /// OptionsList.updateSearch, in list mode: what matches every word, with what is under it and over it.
    fn filter(&mut self) {
        let text = self.search.text.trim().to_lowercase();
        let n = self.rows.len();
        self.shown.clear();
        self.reset_popup();
        if text.is_empty() {
            self.shown.extend(0..n);
        } else if self.cards {
            // collectSearch in card mode: the cards with a match of their own or among theirs, each under its section,
            // and a lone card with options opens at once.
            let words: Vec<&str> = text.split_whitespace().collect();
            let matches = |row: &Row| words.iter().all(|w| row.words.contains(*w));
            let results: Vec<usize> = (0..n)
                .filter(|i| {
                    let row = &self.rows[*i];
                    !row.secondary && row.kind != Kind::Title && (matches(row) || row.children.iter().any(|c| matches(&self.rows[*c])))
                })
                .collect();
            let mut last_title = None;
            let mut shown = Vec::new();
            for &i in &results {
                if let Some(title) = self.rows[i].parent.filter(|p| self.rows[*p].kind == Kind::Title)
                    && last_title != Some(title)
                {
                    shown.push(title);
                    last_title = Some(title);
                }
                shown.push(i);
                shown.extend(self.rows[i].children.iter().copied());
            }
            let lone = results.first().copied().filter(|i| results.len() == 1 && !self.rows[*i].children.is_empty());
            self.shown = shown;
            if let Some(i) = lone {
                self.expand(i, true);
            }
        } else {
            let words: Vec<&str> = text.split_whitespace().collect();
            let mut matched = vec![false; n];
            for i in 0..n {
                if !words.iter().all(|w| self.rows[i].words.contains(*w)) {
                    continue;
                }
                matched[i] = true;
                let mut up = self.rows[i].parent;
                while let Some(a) = up {
                    matched[a] = true;
                    up = self.rows[a].parent;
                }
                for (j, under) in matched.iter_mut().enumerate() {
                    let mut up = self.rows[j].parent;
                    while let Some(a) = up {
                        if a == i {
                            *under = true;
                            break;
                        }
                        up = self.rows[a].parent;
                    }
                }
            }
            self.shown.extend((0..n).filter(|i| matched[*i]));
        }
        self.scroll = 0.0;
        self.layout_cards();
    }

    fn max_scroll(&self) -> f32 {
        let list = list_rect(self.size);
        if self.cards {
            return (self.card_height + 4.0 - list[3]).max(0.0);
        }
        (self.shown.len() as f32 * ROW + 4.0 - list[3]).max(0.0)
    }

    /// PluginsConfigScreen.createOptions with OptionsList.setDisplayMode: list mode when asked for, or with room for one
    /// card a row.
    fn wants_cards(&self, game: &Game<'_>) -> bool {
        self.page == Page::Plugins && !jade::on(game, "always_list_mode") && cards_per_row(self.size) > 1.0
    }

    /// `layoutCards`: the rows of cards, a row as tall as its tallest, sections between them. The popup's rows aren't
    /// here: `popup_rect` places them.
    fn layout_cards(&mut self) {
        self.card_rects.clear();
        self.card_height = 0.0;
        if !self.cards {
            return;
        }
        let (left, row_w) = card_span(self.size);
        let per = cards_per_row(self.size);
        let card_w = ((row_w - CARD_GAP * (per - 1.0)) / per).floor();
        let mut y = 0.0;
        let mut row: Vec<usize> = Vec::new();
        self.card_rects.resize(self.shown.len(), [0.0; 4]);
        for i in 0..self.shown.len() {
            let entry = &self.rows[self.shown[i]];
            if entry.secondary {
                continue;
            }
            if entry.kind == Kind::Title {
                if !row.is_empty() {
                    y = self.place_card_row(&row, left, card_w, y);
                    row.clear();
                }
                self.card_rects[i] = [left, y, row_w, ROW];
                y += ROW + CARD_GAP;
            } else {
                row.push(i);
                if row.len() as f32 >= per {
                    y = self.place_card_row(&row, left, card_w, y);
                    row.clear();
                }
            }
        }
        if !row.is_empty() {
            y = self.place_card_row(&row, left, card_w, y);
        }
        self.card_height = y + 4.0;
    }

    /// `placeCardRow`.
    fn place_card_row(&mut self, row: &[usize], left: f32, card_w: f32, y: f32) -> f32 {
        let height = row.iter().map(|i| if self.rows[self.shown[*i]].kind == Kind::Toggle { CARD_HEIGHT } else { CARD_HEIGHT_TALL }).fold(0.0, f32::max);
        for (at, i) in row.iter().enumerate() {
            self.card_rects[*i] = [left + at as f32 * (card_w + CARD_GAP), y, card_w, height];
        }
        y + height + CARD_GAP
    }

    /// A laid out card's rectangle on the screen, by its place in `shown`. None for what has no place of its own.
    fn card_rect(&self, at: usize) -> Option<[f32; 4]> {
        let [x, y, w, h] = *self.card_rects.get(at)?;
        (h > 0.0).then(|| [x, list_rect(self.size)[1] + 2.0 + CARD_BLEED + y - self.scroll, w, h])
    }

    /// The card of a row in `rows`.
    fn card_of(&self, row: usize) -> Option<[f32; 4]> {
        self.card_rect(self.shown.iter().position(|r| *r == row)?)
    }

    /// SecondaryPopup.reset.
    fn reset_popup(&mut self) {
        self.expanded = None;
        self.fading = None;
        self.sticky = false;
        self.pending = None;
        self.popup_alpha = 0.0;
        self.popup_slide = 0.0;
    }

    /// SecondaryPopup.expand.
    fn expand(&mut self, row: usize, sticky: bool) {
        self.expanded = Some(row);
        self.fading = None;
        self.popup_alpha = 0.0;
        self.popup_slide = 0.0;
        self.sticky = sticky;
    }

    /// SecondaryPopup.collapse.
    fn collapse(&mut self) {
        if self.expanded.is_some() {
            self.fading = self.expanded.take();
            self.sticky = false;
            self.popup_slide = 1.0;
        }
    }

    /// SecondaryPopup.effectiveEntry.
    fn popup_entry(&self) -> Option<usize> {
        self.expanded.or(self.fading).filter(|r| self.shown.contains(r))
    }

    /// `layoutSecondaryPopup`: the panel of a card's options, under the card or moved up to fit. Its rows are a row high
    /// from three pixels in.
    fn popup_rect(&self, entry: usize) -> Option<[f32; 4]> {
        let [cx, cy, cw, ch] = self.card_of(entry)?;
        let count = self.rows[entry].children.len();
        if count == 0 {
            return None;
        }
        let (left, row_w) = card_span(self.size);
        let list = list_rect(self.size);
        let w = row_w.min(cw.max(220.0));
        let x = cx.min(left + row_w - w).max(left);
        let h = 6.0 + count as f32 * ROW;
        let mut y = cy + ch + 6.0;
        let bottom = list[1] + list[3];
        if y + h > bottom {
            y = list[1].max(bottom - h);
        }
        Some([x, y, w, h])
    }

    /// `slideOffset`.
    fn popup_slide_offset(&self) -> f32 {
        ((self.popup_slide - 1.0) * 8.0).round()
    }

    /// A popup row's value widget (`getWidgetZone` in popup mode: `applyPopupWidgets` took ten off its width).
    fn popup_value(panel: [f32; 4], y: f32) -> [f32; 4] {
        [panel[0] + panel[2] - 2.0 - 100.0, y + 3.0, 90.0, 20.0]
    }

    /// SecondaryPopup.isMouseOver: on the card, on the panel, or crossing the gap between them.
    fn over_popup(&self, entry: usize, m: [f32; 2]) -> bool {
        let Some([x, y, w, h]) = self.card_of(entry) else { return false };
        let card = [x - CARD_BLEED, y - CARD_BLEED, w + 2.0 * CARD_BLEED, h + 2.0 * CARD_BLEED];
        if hit(card, m) {
            return true;
        }
        let Some(panel) = self.popup_rect(entry) else { return false };
        if hit(panel, m) {
            return true;
        }
        if m[0] < card[0].min(panel[0]) || m[0] >= (card[0] + card[2]).max(panel[0] + panel[2]) {
            return false;
        }
        (m[1] >= card[1] + card[3] && m[1] < panel[1]) || (m[1] >= panel[1] + panel[3] && m[1] < card[1])
    }

    /// SecondaryPopup.commitPending, tick and OptionsList's collapse when the pointer has left.
    fn tick_popup(&mut self, game: &Game<'_>, m: [f32; 2]) {
        if let Some((row, at, when)) = self.pending {
            if (m[0] - at[0]).abs() > 2.0 || (m[1] - at[1]).abs() > 2.0 {
                self.pending = None;
            } else if when.elapsed().as_millis() >= 120 {
                self.pending = None;
                click_sound();
                if self.expanded == Some(row) {
                    self.collapse();
                } else {
                    self.expand(row, false);
                }
            }
        }
        let fade_ms = if jade::on(game, "debug_mode") { 500.0 } else { 100.0 };
        let step = self.popup_at.elapsed().as_secs_f32() * 1000.0 / fade_ms;
        self.popup_at = std::time::Instant::now();
        if self.expanded.is_some() {
            self.popup_alpha = (self.popup_alpha + step).clamp(0.0, 1.0);
            self.popup_slide = (self.popup_slide + step).clamp(0.0, 1.0);
            self.fading = None;
        } else if self.fading.is_some() {
            self.popup_alpha = (self.popup_alpha - step).clamp(0.0, 1.0);
            if self.popup_alpha <= 0.0 {
                self.fading = None;
            }
        }
        if let Some(entry) = self.expanded
            && !self.sticky
        {
            let inside = self.keyboard && matches!(self.focus, Focus::Row(r) if r == entry || self.rows[r].parent == Some(entry));
            if !inside && !self.over_popup(entry, m) {
                self.collapse();
            }
        }
    }

    /// `showOnTop`.
    fn show_on_top(&mut self, row: usize) {
        let Some(at) = self.shown.iter().position(|r| *r == row) else { return };
        let top = if self.cards { self.card_rects.get(at).map_or(0.0, |r| r[1]) + CARD_BLEED } else { ROW * at as f32 + 1.0 };
        self.scroll = top.clamp(0.0, self.max_scroll());
    }

    /// `scrollToEntry`: the row into view (a popup's: its card).
    fn scroll_to(&mut self, row: usize) {
        let list = list_rect(self.size);
        let (top, height) = if self.cards {
            let anchor = if self.rows[row].secondary { self.rows[row].parent.unwrap_or(row) } else { row };
            let Some(at) = self.shown.iter().position(|r| *r == anchor) else { return };
            let Some(r) = self.card_rects.get(at) else { return };
            (r[1], r[3] + 2.0 * (2.0 + CARD_BLEED))
        } else {
            let Some(at) = self.shown.iter().position(|r| *r == row) else { return };
            (at as f32 * ROW, ROW + 4.0)
        };
        if top < self.scroll {
            self.scroll = top;
        } else if top + height > self.scroll + list[3] {
            self.scroll = top + height - list[3];
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// `changeFocus` with the keyboard: on a row, its popup opened or another closed (OptionsList.setSelected), in view.
    fn focus_row(&mut self, row: usize) {
        self.keyboard = true;
        self.slider_keys = false;
        self.focus = match self.rows[row].kind {
            Kind::Profile(i) => {
                self.names[i].caret = self.names[i].text.chars().count();
                Focus::Profile(i)
            }
            _ => Focus::Row(row),
        };
        if self.cards {
            let entry = &self.rows[row];
            let target = if entry.secondary { entry.parent } else if !entry.children.is_empty() { Some(row) } else { None };
            match target {
                Some(t) if self.expanded != Some(t) => self.expand(t, false),
                None => self.collapse(),
                _ => {}
            }
        }
        self.scroll_to(row);
    }

    /// The row the focus is on (a profile's name is on its row).
    fn focused_row(&self) -> Option<usize> {
        match self.focus {
            Focus::Row(r) => Some(r),
            Focus::Profile(i) => self.rows.iter().position(|r| r.kind == Kind::Profile(i)),
            _ => None,
        }
    }

    /// What the keyboard can land on: `focusableWidgets`, in the order shown.
    fn focusable(&self, game: &Game<'_>, row: usize) -> bool {
        let entry = &self.rows[row];
        entry.kind != Kind::Title && self.row_enabled(game, entry)
    }

    /// `nextTabFocusPath`: every widget in order, a popup's among them.
    fn tab(&mut self, game: &Game<'_>, forward: bool) {
        let order: Vec<usize> = self.shown.iter().copied().filter(|r| self.focusable(game, *r)).collect();
        if order.is_empty() {
            return;
        }
        let at = self.focused_row().and_then(|r| order.iter().position(|o| *o == r));
        let next = match (at, forward) {
            (Some(i), true) => i + 1,
            (Some(0), false) => return self.focus = Focus::Search,
            (Some(i), false) => i - 1,
            (None, true) => 0,
            (None, false) => order.len() - 1,
        };
        match order.get(next) {
            Some(row) => self.focus_row(*row),
            None => self.focus = Focus::Search,
        }
    }

    /// The arrows on the list: up and down the rows; left to the navigation.
    fn arrow_list(&mut self, game: &Game<'_>, key: &str) {
        let order: Vec<usize> = self.shown.iter().copied().filter(|r| self.focusable(game, *r)).collect();
        let at = self.focused_row().and_then(|r| order.iter().position(|o| *o == r));
        match key {
            "key.keyboard.down" => {
                let next = at.map_or(0, |i| i + 1);
                if let Some(row) = order.get(next) {
                    self.focus_row(*row);
                }
            }
            "key.keyboard.up" => match at {
                Some(0) | None => self.focus = Focus::Search,
                Some(i) => self.focus_row(order[i - 1]),
            },
            "key.keyboard.left" => self.to_nav(),
            _ => {}
        }
    }

    /// `nextCardFocusPath`: left and right through the cards, up and down the column; off the left to the navigation.
    fn arrow_cards(&mut self, game: &Game<'_>, key: &str) {
        let cards: Vec<usize> = (0..self.shown.len())
            .filter(|i| {
                let row = self.shown[*i];
                !self.rows[row].secondary && self.focusable(game, row) && self.card_rects.get(*i).is_some_and(|r| r[3] > 0.0)
            })
            .collect();
        let anchor = self.focused_row().map(|r| if self.rows[r].secondary { self.rows[r].parent.unwrap_or(r) } else { r });
        let at = anchor.and_then(|a| cards.iter().position(|i| self.shown[*i] == a));
        let Some(at) = at else {
            if let Some(first) = cards.first() {
                self.focus_row(self.shown[*first]);
            }
            return;
        };
        let target = match key {
            "key.keyboard.right" => cards.get(at + 1).copied(),
            "key.keyboard.left" => match at.checked_sub(1) {
                Some(i) => cards.get(i).copied(),
                None => return self.to_nav(),
            },
            _ => {
                // The rows of cards, and the same column of the next that has one.
                let mut rows: Vec<Vec<usize>> = Vec::new();
                let mut last_y = f32::MIN;
                for i in &cards {
                    let y = self.card_rects[*i][1];
                    if rows.is_empty() || y != last_y {
                        rows.push(Vec::new());
                        last_y = y;
                    }
                    if let Some(row) = rows.last_mut() {
                        row.push(*i);
                    }
                }
                let here = cards[at];
                let Some((row, column)) = rows.iter().enumerate().find_map(|(r, cols)| cols.iter().position(|c| *c == here).map(|c| (r, c))) else { return };
                let up = key == "key.keyboard.up";
                let mut r = row;
                loop {
                    r = if up {
                        match r.checked_sub(1) {
                            Some(r) => r,
                            None => break None,
                        }
                    } else {
                        r + 1
                    };
                    match rows.get(r) {
                        None => break None,
                        Some(cols) => {
                            if let Some(c) = cols.get(column) {
                                break Some(*c);
                            }
                        }
                    }
                }
            }
        };
        if let Some(i) = target {
            self.focus_row(self.shown[i]);
        }
    }

    /// `OptionsNav.getCurrentEntry`: the focus on the section it is in.
    fn to_nav(&mut self) {
        let titles = self.titles();
        if titles.is_empty() {
            return;
        }
        let current = self.focused_row().map(|r| self.root_of(r));
        self.focus = Focus::Nav(current.and_then(|c| titles.iter().position(|t| *t == c)).unwrap_or(0));
        self.keyboard = true;
    }

    /// Enter or Space on a row: what a click on its widget does.
    fn use_row(&mut self, game: &Game<'_>, row_index: usize, shift: bool) {
        let kind = self.rows[row_index].kind;
        let key = self.rows[row_index].key;
        match kind {
            Kind::Toggle => {
                click_sound();
                if let Some(o) = jade::option(key) {
                    jade::step_option(game, o, false);
                } else {
                    // A plugin's option (register_option).
                    let now = jade::on(game, key);
                    game.native_set_setting(ID, key, if now { "false" } else { "true" });
                    jade::settings_changed();
                }
            }
            Kind::Cycle | Kind::Theme | Kind::Style => {
                click_sound();
                if let Some(o) = jade::option(key) {
                    jade::step_option(game, o, shift);
                }
            }
            Kind::Keybind(i) => {
                click_sound();
                self.binding = Some(i);
            }
            // WailaConfigScreen's `overlay_pos.adjust`: PreviewOptionsScreen.startAdjustingPosition.
            Kind::Adjust => {
                click_sound();
                self.adjusting = true;
            }
            Kind::Reload => {
                click_sound();
                jade::reload_plugins();
                self.rows = build(game, self.page);
                self.filter();
            }
            Kind::Reset => {
                click_sound();
                self.confirm = Some(Confirm::Reset);
            }
            Kind::Slider { .. } | Kind::Title | Kind::Profile(_) => {}
        }
    }

    /// The arrows on a focused slider that Enter let go (AbstractSliderButton.keyPressed): a step.
    fn step_slider(&self, game: &Game<'_>, row_index: usize, by: f32) {
        let row = &self.rows[row_index];
        let Kind::Slider { min, max, step, .. } = row.kind else { return };
        let v = (jade::num(game, row.key) as f32 + by * step).clamp(min, max);
        let text = if step >= 1.0 { format!("{}", v.round() as i64) } else { jade::trim2(f64::from(v)) };
        jade::set_live(game, row.key, text);
        jade::commit_live(game, row.key);
        jade::settings_changed();
    }

    /// The section a shown row is in (`Entry.root`), as an index in `rows`.
    fn root_of(&self, mut row: usize) -> usize {
        while let Some(p) = self.rows[row].parent {
            row = p;
        }
        row
    }

    /// `currentTitle`: the section of the row under the pointer, or else of the list's top.
    fn current_title(&self, mouse: [f32; 2]) -> Option<usize> {
        let list = list_rect(self.size);
        if self.cards {
            let top = hit(list, mouse).then(|| (0..self.shown.len()).find(|i| self.card_rect(*i).is_some_and(|r| hit([r[0] - CARD_BLEED, r[1] - CARD_BLEED, r[2] + 2.0 * CARD_BLEED, r[3] + 2.0 * CARD_BLEED], mouse)))).flatten();
            let top = top.or_else(|| (0..self.shown.len()).find(|i| self.card_rect(*i).is_some_and(|r| r[1] + r[3] > list[1])));
            return top.map(|i| self.root_of(self.shown[i]));
        }
        let at = if hit(list, mouse) { mouse[1] } else { list[1] };
        let index = ((at - list[1] - 2.0 + self.scroll) / ROW).max(0.0) as usize;
        self.shown.get(index.min(self.shown.len().saturating_sub(1))).map(|r| self.root_of(*r))
    }

    /// OptionsNav.refresh: the sections shown. None for a list of one.
    fn titles(&self) -> Vec<usize> {
        if self.shown.len() <= 1 {
            return Vec::new();
        }
        self.shown.iter().copied().filter(|r| self.rows[*r].kind == Kind::Title).collect()
    }

    fn row_enabled(&self, game: &Game<'_>, row: &Row) -> bool {
        match row.kind {
            Kind::Style => jade_theme::style_count(game) > 1,
            Kind::Profile(_) => jade::on(game, "enable_profiles"),
            Kind::Keybind(_) if self.page == Page::Profiles => jade::on(game, "enable_profiles"),
            _ => true,
        }
    }

    /// Whether the row's value isn't what it started as (which lights its reset button).
    fn row_changed(&self, game: &Game<'_>, row: &Row) -> bool {
        match row.kind {
            Kind::Keybind(i) => {
                let [name, _, default] = jade::KEYS[i];
                game.native_mod_key(ID, name, default) != default
            }
            Kind::Adjust => jade::opt(game, "overlay_x").starts_with("custom:") || jade::opt(game, "position").starts_with("custom:"),
            Kind::Toggle | Kind::Cycle | Kind::Slider { .. } | Kind::Style | Kind::Theme => jade::opt(game, row.key) != default_of(row.key),
            _ => false,
        }
    }

    fn reset_row(&self, game: &Game<'_>, row: usize) {
        let row = &self.rows[row];
        match row.kind {
            Kind::Keybind(i) => {
                let [name, _, default] = jade::KEYS[i];
                queue(Action::SetKey(name, default.to_owned()));
            }
            Kind::Adjust => {
                for key in ["overlay_x", "position"] {
                    game.native_set_setting(ID, key, &default_of(key));
                }
                jade::settings_changed();
            }
            Kind::Theme => {
                game.native_set_setting(ID, "overlay_theme", &default_of("overlay_theme"));
                game.native_set_setting(ID, "theme_style", &default_of("theme_style"));
                jade::settings_changed();
            }
            Kind::Toggle | Kind::Cycle | Kind::Slider { .. } | Kind::Style => {
                game.native_set_setting(ID, row.key, &default_of(row.key));
                jade::settings_changed();
            }
            _ => {}
        }
    }

    /// PreviewOptionsScreen.moveOverlay: as the same settings as the presets, kept when it is let go.
    fn move_overlay(&self, game: &Game<'_>, center_x: f32, center_y: f32, snap: bool) {
        let [w, h] = self.size;
        let [_, _, rw, rh] = self.rect;
        let (ax, ay) = (calculate_anchor(center_x, w, rw), calculate_anchor(center_y, h, rh));
        let px = (center_x + rw * (ax - 0.5)) / w;
        let py = 1.0 - (center_y + rh * (ay - 0.5)) / h;
        let px = try_flip(game, maybe_snap(px, snap)).clamp(0.0, 1.0);
        let py = maybe_snap(py, snap).clamp(0.0, 1.0);
        jade::set_live(game, "overlay_x", format!("custom:{px}:{}", try_flip(game, ax)));
        jade::set_live(game, "position", format!("custom:{py}:{ay}"));
    }

    fn keep_position(&self, game: &Game<'_>) {
        jade::commit_live(game, "overlay_x");
        jade::commit_live(game, "position");
        jade::settings_changed();
    }

    fn exit_adjusting(&mut self, game: &Game<'_>) {
        self.keep_position(game);
        self.adjusting = false;
        self.drag = Drag::None;
        self.sample = None;
    }

    /// A slider or a dragged overlay let go (mouseReleased).
    fn let_go(&mut self, game: &Game<'_>) {
        match self.drag {
            Drag::Slider(row, _) => {
                jade::commit_live(game, self.rows[row].key);
                jade::settings_changed();
            }
            Drag::Overlay(_) => self.keep_position(game),
            Drag::None | Drag::Bar(_) => {}
        }
        self.drag = Drag::None;
        self.pressed = None;
    }

    /// SliderOptionValue.Slider.applyValue.
    fn slide_to(&self, game: &Game<'_>, row: usize, [track_x, track_w]: [f32; 2], x: f32) {
        let row = &self.rows[row];
        let Kind::Slider { min, max, step, .. } = row.kind else { return };
        let t = ui::slider_value(track_x, track_w, x);
        let v = min + (max - min) * t;
        let v = (v / step).round() * step;
        let text = if step >= 1.0 { format!("{}", v.round() as i64) } else { jade::trim2(f64::from(v)) };
        jade::set_live(game, row.key, text);
    }

    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch, mouse: [f32; 2]) {
        self.size = [batch.width, batch.height];
        // The window's size, or Always Use List Mode, can change the mode.
        let cards = self.wants_cards(game);
        if cards != self.cards {
            self.cards = cards;
            self.filter();
        }
        self.layout_cards();
        if mouse != self.last_mouse {
            self.last_mouse = mouse;
            self.keyboard = false;
        }
        if self.cards {
            self.tick_popup(game, mouse);
        }
        // Slider, scrollbar and overlay drags, each frame: the mouse hooks say press and release.
        match self.drag {
            Drag::Slider(row, track) => self.slide_to(game, row, track, mouse[0]),
            Drag::Bar(grab) => {
                let list = list_rect(self.size);
                let most = self.max_scroll();
                let handle = self.bar_height(list, most);
                if list[3] > handle {
                    self.scroll = ((mouse[1] - grab - list[1]) / (list[3] - handle) * most).clamp(0.0, most);
                }
            }
            Drag::Overlay(off) => {
                let ctrl = modifier_down(game, winit::keyboard::KeyCode::ControlLeft, winit::keyboard::KeyCode::ControlRight);
                self.move_overlay(game, mouse[0] - off[0], mouse[1] - off[1], !ctrl);
            }
            Drag::None => {}
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        let atlas = &game.gui.atlas;
        // Screen.renderBackground: the world dimmed; over Options, that screen hidden.
        let [w, h] = self.size;
        if self.host == Host::Mod {
            batch.fill_gradient(atlas, (0.0, 0.0, w, h), rgba(16, 16, 16, 0xC0), rgba(16, 16, 16, 0xD0));
        } else {
            batch.fill(atlas, 0.0, 0.0, w, h, rgba(16, 16, 16, 0xF8));
        }
        // What's under a modal layer doesn't hover (`extractRenderState` with the mouse at MAX_VALUE).
        let m = if self.adjusting || self.confirm.is_some() { [-1.0e6, -1.0e6] } else { mouse };
        if self.page == Page::Home {
            self.draw_home(game, batch, m);
        } else {
            self.draw_list(game, batch, m);
        }
        if self.adjusting {
            // PreviewOptionsScreen.extractRenderState.
            game.native_fill(batch, [0.0, 0.0, w, h], 0x8080_8080);
            let text = tr("config.jade.overlay_pos.exit", "Press ESC or click the center to exit");
            let tw = game.native_text_width(&text);
            let (tx, ty) = (((w - tw) / 2.0).floor(), (h / 2.0).floor() - 7.0);
            game.native_fill(batch, [tx - 4.0, ty - 4.0, tw + 8.0, 17.0], 0x8800_0000);
            game.native_text(batch, &text, [tx, ty], 0xFFFFFF, false);
        }
        self.draw_preview(game, batch, mouse);
        if self.adjusting {
            let [rx, ry, rw, rh] = self.rect;
            // The guides at the middle a position snaps to.
            if jade::custom_axis(&jade::opt(game, "overlay_x")).is_some_and(|(p, _)| p == 0.5) {
                game.native_fill(batch, [(w / 2.0).floor(), (ry - 5.0).floor(), 1.0, rh + 9.0], 0xFF00_00FF);
            }
            if jade::custom_axis(&jade::opt(game, "position")).is_some_and(|(p, _)| p == 0.5) {
                game.native_fill(batch, [(rx - 5.0).floor(), (h / 2.0).floor(), rw + 9.0, 1.0], 0xFF00_00FF);
            }
        }
        if let Some(confirm) = self.confirm {
            self.draw_confirm(game, batch, confirm, mouse);
        }
    }

    /// OverlayRenderer under PreviewOptionsScreen: over the screen, gone under the pointer unless it is what's being
    /// changed (`forcePreviewOverlay`), and the Grass Block while adjusting with nothing looked at.
    fn draw_preview(&mut self, game: &Game<'_>, batch: &mut HudBatch, mouse: [f32; 2]) {
        let built = jade::take_built();
        if !matches!(self.page, Page::Mod | Page::Plugins) {
            return;
        }
        if self.adjusting && built.is_none() && self.sample.is_none() {
            self.sample = Some(jade::sample_box(game));
        }
        let shown: Option<&jade::BuiltBox> = match &built {
            Some(b) => Some(&**b),
            None if self.adjusting => self.sample.as_ref(),
            None => None,
        };
        let Some(shown) = shown else { return };
        let rect = jade::box_rect(game, [batch.width, batch.height], shown, self.adjusting);
        let forced = self.adjusting || matches!(self.drag, Drag::Slider(row, _) if self.rows[row].key == "scale");
        let draw = forced || (self.preview && self.confirm.is_none() && !hit(rect, mouse));
        if draw {
            jade::draw_box(game, batch, shown, self.adjusting);
        }
        self.rect = rect;
    }

    fn draw_confirm(&self, game: &Game<'_>, batch: &mut HudBatch, confirm: Confirm, mouse: [f32; 2]) {
        let [w, h] = self.size;
        game.native_fill(batch, [0.0, 0.0, w, h], 0xD000_0000);
        let (title, message, yes) = match confirm {
            Confirm::Reset => (tr("config.jade.reset_settings", "Reset Settings"), tr("config.jade.reset_settings.confirm", "Are you sure you wish to reset settings?"), tr("controls.reset", "Reset")),
            Confirm::SaveProfile(i) => (
                tr("gui.jade.save_profile.title", "Save as Profile"),
                tr("gui.jade.save_profile.message", "By continuing, you will save the current settings as profile %s. This will overwrite the existing profile. (Key binds will not be saved)").replace("%s", &self.profile_title(i)),
                tr("gui.continue", "Continue"),
            ),
        };
        let cy = (h / 2.0).floor();
        let tw = game.native_text_width(&title);
        game.native_text(batch, &title, [((w - tw) / 2.0).floor(), cy - 44.0], 0xFFFFFF, true);
        for (i, line) in wrap(game, &message, w.min(320.0) - 20.0).iter().enumerate() {
            let lw = game.native_text_width(line);
            game.native_text(batch, line, [((w - lw) / 2.0).floor(), cy - 24.0 + 10.0 * i as f32], 0xFFFFFF, true);
        }
        let (yes_rect, no_rect) = confirm_rects(self.size);
        let atlas = &game.gui.atlas;
        ui::button(batch, atlas, yes_rect[0], yes_rect[1], yes_rect[2], yes_rect[3], &Text::literal(yes), true, hit(yes_rect, mouse));
        ui::button(batch, atlas, no_rect[0], no_rect[1], no_rect[2], no_rect[3], &Text::literal(tr("gui.cancel", "Cancel")), true, hit(no_rect, mouse));
    }

    /// HomeConfigScreen.
    fn draw_home(&self, game: &Game<'_>, batch: &mut HudBatch, m: [f32; 2]) {
        let [w, h] = self.size;
        let atlas = &game.gui.atlas;
        let center = |game: &Game<'_>, batch: &mut HudBatch, text: &str, y: f32| {
            let tw = game.native_text_width(&jade::plain(text));
            batch.text_styled(&game.gui.atlas, &jade::rich(text), ((w - tw) / 2.0).floor(), y, rgba(0xE0, 0xE0, 0xE0, 255), true);
        };
        let mid = (h / 2.0).floor();
        center(game, &mut *batch, &format!("§f{}", tr("gui.jade.configuration", "Jade Configuration")), mid - 62.0);
        center(game, &mut *batch, &tr("gui.jade.configuration.desc1", "a §fUI enhancement§r mod that"), mid - 44.0);
        center(game, &mut *batch, &tr("gui.jade.configuration.desc2", "§fdisplays information§r about what §fyou are looking at§r."), mid - 32.0);
        let [mod_settings, plugin_settings, profiles, done] = home_rects(self.size);
        let button = |batch: &mut HudBatch, r: [f32; 4], label: &str, focus: usize| {
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &Text::literal(label), true, hit(r, m) || self.focus == Focus::Home(focus));
        };
        button(&mut *batch, mod_settings, &tr("gui.jade.jade_settings", "Mod Settings"), 0);
        button(&mut *batch, plugin_settings, &tr("gui.jade.plugin_settings", "Plugin Settings"), 1);
        button(&mut *batch, done, &tr("gui.done", "Done"), 3);
        // The profile button: paper on a button (HomeConfigScreen.profileIcon), its name a tooltip.
        button(&mut *batch, profiles, "", 2);
        game.native_item(batch, "minecraft:paper", [profiles[0] + 2.0, profiles[1] + 2.0], 16.0);
        center(game, &mut *batch, &tr("gui.jade.by", "Made with %s by Snownee").replace("%s", "§c\u{2764}§r"), h - 18.0);
        if hit(profiles, m) {
            let lines = vec![(Text::literal(tr("gui.jade.profile_settings", "Configuration Profile Settings")), u32::MAX)];
            game.draw_tooltip_lines(batch, &lines, m[0], m[1]);
        }
    }

    /// BaseOptionsScreen: the search box and navigation on the left, the list, the buttons under it.
    fn draw_list(&self, game: &Game<'_>, batch: &mut HudBatch, m: [f32; 2]) {
        let [w, h] = self.size;
        let atlas = &game.gui.atlas;
        let high_contrast = game.settings.resource_packs.iter().any(|p| p == "high_contrast");
        self.draw_nav(game, batch, m, high_contrast);
        self.draw_search(game, batch, high_contrast);
        let list = list_rect(self.size);
        let (left, width) = row_span(list);
        let start = batch.vertices.len();
        let mut tooltip: Option<(&Row, [f32; 2])> = None;
        if self.cards {
            tooltip = self.draw_cards(game, batch, m);
        } else {
            for (i, row_index) in self.shown.iter().enumerate() {
                let y = list[1] + 2.0 - self.scroll + i as f32 * ROW;
                if y + ROW < list[1] || y > list[1] + list[3] {
                    continue;
                }
                let row = &self.rows[*row_index];
                let hover = if self.keyboard { self.focused_row() == Some(*row_index) } else { hit(list, m) && m[1] >= y && m[1] < y + ROW };
                // extractSelection.
                if hover && row.kind != Kind::Title {
                    game.native_fill(batch, [list[0], y, list[2], ROW], 0x33FF_FFFF);
                }
                self.draw_row(game, batch, row, *row_index, left, width, y, m);
                if hover && !self.keyboard && m[0] < row_rects(left, width, y, false).value[0] {
                    tooltip = Some((row, m));
                }
            }
        }
        if self.shown.is_empty() {
            let text = tr("gui.jade.no_results", "No results :(");
            let tw = game.native_text_width(&text);
            game.native_text(batch, &text, [list[0] + ((list[2] - tw) / 2.0).floor(), list[1] + 10.0], 0xAAAAAA, true);
        }
        batch.clip_since(start, list[0], list[1], list[0] + list[2], list[1] + list[3]);
        // extractListSeparators.
        game.native_fill(batch, [0.0, h - FOOTER, w, 1.0], 0x33FF_FFFF);
        game.native_fill(batch, [0.0, h - FOOTER + 1.0, w, 1.0], 0xBF00_0000);
        // extractScrollbar.
        let most = self.max_scroll();
        if most > 0.0 {
            let handle = self.bar_height(list, most);
            let bar_x = w - 6.0;
            ui::nine_slice(batch, atlas, "gui/sprites/widget/scroller_background", bar_x, list[1], 6.0, list[3], u32::MAX);
            let y = list[1] + self.scroll / most * (list[3] - handle);
            ui::nine_slice(batch, atlas, "gui/sprites/widget/scroller", bar_x, y.floor(), 6.0, handle, u32::MAX);
        }
        let done = done_rect(self.size);
        ui::button(batch, atlas, done[0], done[1], done[2], done[3], &Text::literal(tr("gui.done", "Done")), true, hit(done, m));
        if self.has_preview_button(game) {
            let r = preview_rect(self.size);
            let label = Text::literal(format!("{}: ", tr("gui.jade.preview", "Preview"))).append(on_off(self.preview));
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &label, true, hit(r, m));
        }
        // BaseOptionsScreen.extractRenderState: a row's description by its title, with its key under Shift.
        if let Some((row, at)) = tooltip {
            let mut lines: Vec<(Text, u32)> = Vec::new();
            if !row.desc.is_empty() {
                for line in wrap(game, &fill_variables(game, &row.desc), 255.0) {
                    lines.push((Text::literal(line), u32::MAX));
                }
            }
            let shift = modifier_down(game, winit::keyboard::KeyCode::ShiftLeft, winit::keyboard::KeyCode::ShiftRight);
            if shift && !row.key.is_empty() {
                lines.push((Text::colored(row.key, 0xAAAAAA), u32::MAX));
            }
            if !lines.is_empty() {
                game.draw_tooltip_lines(batch, &lines, at[0], at[1]);
            }
        }
    }

    /// The Preview button: only over a level, with a page that has the preview.
    fn has_preview_button(&self, game: &Game<'_>) -> bool {
        game.player.is_some() && matches!(self.page, Page::Mod | Page::Plugins)
    }

    /// OptionsList.extractListItems in card mode: the sections and cards, then the open popup over them. What the
    /// tooltip is of, when the pointer is on a title (BaseOptionsScreen.extractRenderState).
    fn draw_cards(&self, game: &Game<'_>, batch: &mut HudBatch, m: [f32; 2]) -> Option<(&Row, [f32; 2])> {
        let list = list_rect(self.size);
        let atlas = &game.gui.atlas;
        let entry = self.popup_entry();
        let panel = entry.and_then(|e| self.popup_rect(e));
        let on_panel = !self.keyboard && panel.is_some_and(|p| hit(p, m));
        let focused = self.focused_row().map(|r| if self.rows[r].secondary { self.rows[r].parent.unwrap_or(r) } else { r });
        let mut tooltip = None;
        for (at, row_index) in self.shown.iter().enumerate() {
            let Some(rect) = self.card_rect(at) else { continue };
            let [x, y, w, h] = rect;
            if y + h + CARD_BLEED < list[1] || y - CARD_BLEED > list[1] + list[3] {
                continue;
            }
            let row = &self.rows[*row_index];
            if row.kind == Kind::Title {
                let label = if row.danger { Text::colored(row.label.as_str(), 0xFF5555) } else { Text::literal(row.label.as_str()) };
                let tw = game.native_text_width(&row.label);
                batch.text_styled(atlas, &label, x + ((w - tw) / 2.0).floor(), y + 9.0, rgba(255, 255, 255, 255), true);
                continue;
            }
            let over = hit(list, m) && !on_panel && hit([x - CARD_BLEED, y - CARD_BLEED, w + 2.0 * CARD_BLEED, h + 2.0 * CARD_BLEED], m);
            let hovered = if self.keyboard { focused == Some(*row_index) } else { over };
            self.draw_card(game, batch, row, *row_index, rect, hovered, m);
            let title_x = x + 2.0 + if row.icon.is_some() { 26.0 } else { 6.0 };
            if over && !self.keyboard && m[0] >= title_x && m[0] < title_x + game.native_text_width(&row.label) && m[1] < y + h - 10.0 {
                tooltip = Some((row, m));
            }
        }
        if let (Some(entry), Some(panel)) = (entry, panel) {
            let on_row = self.draw_popup(game, batch, entry, panel, m);
            if on_row.is_some() {
                tooltip = on_row;
            }
        }
        tooltip
    }

    /// `extractCardContent`.
    #[allow(clippy::too_many_arguments)]
    fn draw_card(&self, game: &Game<'_>, batch: &mut HudBatch, row: &Row, row_index: usize, [x, y, w, h]: [f32; 4], hovered: bool, m: [f32; 2]) {
        let atlas = &game.gui.atlas;
        let white = rgba(255, 255, 255, 255);
        let expandable = !row.children.is_empty();
        draw_card_background(game, batch, [x, y, w, h], expandable, hovered);
        // The entry's content: two pixels in (`CONTENT_PADDING`).
        let (cx, cy, cw, ch) = (x + 2.0, y + 2.0, w - 4.0, h - 4.0);
        // `getTextX`.
        let text_x = cx + if row.icon.is_some() { 26.0 } else { 6.0 };
        if row.kind == Kind::Toggle {
            // The title on up to two lines, the toggle at the right, both about the middle.
            let mid = cy + (ch / 2.0).floor();
            let toggle = [cx + cw - TOGGLE_W - 4.0, mid - (TOGGLE_H / 2.0).floor()];
            let most = toggle[0] - 4.0 - text_x;
            let lines = wrap(game, &row.label, most);
            if lines.len() <= 1 {
                game.native_text(batch, &row.label, [text_x, mid - 4.0], 0xFFFFFF, true);
            } else {
                game.native_text(batch, &lines[0], [text_x, mid - 9.0], 0xFFFFFF, true);
                let rest = Text::literal(lines[1..].join(" "));
                ui::scrolling_text(batch, atlas, &rest, text_x, text_x + most, mid, mid + 9.0, mid + 1.0, white);
            }
            let state = if self.pressed == Some(row_index) {
                ToggleState::Pressed
            } else if !self.keyboard && hit([toggle[0], toggle[1], TOGGLE_W, TOGGLE_H], m) {
                ToggleState::Hovered
            } else if self.keyboard && self.focused_row() == Some(row_index) {
                ToggleState::Focused
            } else {
                ToggleState::Normal
            };
            draw_toggle(game, batch, toggle, jade::on(game, row.key), state);
        } else {
            let label = Text::literal(row.label.as_str());
            ui::scrolling_text(batch, atlas, &label, text_x, cx + cw - 8.0, cy + 2.0, cy + 15.0, cy + 4.0, white);
            let value = [cx + 4.0, cy + ch - 2.0 - 20.0, cw - 8.0, 20.0];
            let hover = if self.keyboard { self.focused_row() == Some(row_index) } else { hit(value, m) };
            self.draw_value(game, batch, row, row_index, value, hover, self.row_enabled(game, row));
        }
        // JadeIcons.render: 16 pixels, four in; about the middle beside a toggle, else level with the title.
        if let Some(icon) = row.icon {
            let at = [cx + 4.0, if row.kind == Kind::Toggle { cy + (ch / 2.0).floor() - 8.0 } else { cy + 1.0 }];
            if row.key == "potion_effects" {
                game.native_item_stack(batch, &horizon_client::items::ItemStack::new(icon, 1).with_potion("minecraft:healing"), at, 16.0);
            } else {
                game.native_item(batch, icon, at, 16.0);
            }
        }
        if expandable {
            // card_arrow.png, a third of whose rows have anything on them.
            let (ax, ay) = (x + ((w - 15.0) / 2.0).floor(), y + h + CARD_BLEED - 15.0 - 2.0);
            game.native_fill(batch, [ax + 5.0, ay + 12.0, 5.0, 1.0], 0xC0FF_FFFF);
            game.native_fill(batch, [ax + 6.0, ay + 13.0, 3.0, 1.0], 0xC0FF_FFFF);
            game.native_fill(batch, [ax + 7.0, ay + 14.0, 1.0, 1.0], 0xC0FF_FFFF);
        }
    }

    /// `popup_background`, and the card's options on it as rows, fading in and sliding down together. The row whose title
    /// the pointer is on.
    fn draw_popup(&self, game: &Game<'_>, batch: &mut HudBatch, entry: usize, [px, py, pw, ph]: [f32; 4], m: [f32; 2]) -> Option<(&Row, [f32; 2])> {
        let atlas = &game.gui.atlas;
        let slide = self.popup_slide_offset();
        let start = batch.vertices.len();
        let (bx, by, bw, bh) = (px - CARD_BLEED, py - CARD_BLEED + slide, pw + 2.0 * CARD_BLEED, ph + 2.0 * CARD_BLEED);
        game.native_fill(batch, [bx, by, bw, 1.0], 0xA59F_C8F0);
        game.native_fill(batch, [bx, by + bh - 1.0, bw, 1.0], 0xA59F_C8F0);
        game.native_fill(batch, [bx, by + 1.0, 1.0, bh - 2.0], 0xA59F_C8F0);
        game.native_fill(batch, [bx + bw - 1.0, by + 1.0, 1.0, bh - 2.0], 0xA59F_C8F0);
        game.native_fill(batch, [bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0], 0xD71C_222E);
        let mut tooltip = None;
        for (k, row_index) in self.rows[entry].children.iter().enumerate() {
            let row = &self.rows[*row_index];
            let y = py + 3.0 + slide + k as f32 * ROW;
            let value = Self::popup_value([px, py, pw, ph], y);
            let label_x = px + 2.0 + 10.0;
            let enabled = self.row_enabled(game, row);
            let label = if enabled { Text::literal(row.label.as_str()) } else { Text::colored(row.label.as_str(), 0xAAAAAA) };
            ui::scrolling_text(batch, atlas, &label, label_x, value[0] - 4.0, y, y + ROW, y + 9.0, rgba(255, 255, 255, 255));
            let hover = if self.keyboard { self.focused_row() == Some(*row_index) } else { hit(value, m) };
            self.draw_value(game, batch, row, *row_index, value, hover, enabled);
            if !self.keyboard && hit([px, y, pw, ROW], m) && m[0] < value[0] {
                tooltip = Some((row, m));
            }
        }
        // SecondaryPopup.applyAlpha.
        if self.popup_alpha < 1.0 {
            for v in &mut batch.vertices[start..] {
                let a = ((v.color >> 24) as f32 * self.popup_alpha) as u32;
                v.color = (v.color & 0x00FF_FFFF) | (a << 24);
            }
        }
        tooltip
    }

    /// A row's widget in its rectangle (`mainWidget`).
    #[allow(clippy::too_many_arguments)]
    fn draw_value(&self, game: &Game<'_>, batch: &mut HudBatch, row: &Row, row_index: usize, [vx, vy, vw, vh]: [f32; 4], hover: bool, enabled: bool) {
        let atlas = &game.gui.atlas;
        match row.kind {
            Kind::Slider { min, max, percent, .. } => {
                let value = jade::num(game, row.key) as f32;
                let t = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
                let text = if percent { format!("{}%", value.round() as i64) } else { jade::trim2(f64::from(value)) };
                let dragging = matches!(self.drag, Drag::Slider(r, _) if r == row_index) || (self.slider_keys && self.focused_row() == Some(row_index));
                ui::slider(batch, atlas, vx, vy, vw, &Text::literal(text), t, hover, true, dragging);
            }
            Kind::Toggle => ui::button(batch, atlas, vx, vy, vw, vh, &on_off(jade::on(game, row.key)), true, hover),
            Kind::Cycle | Kind::Theme | Kind::Style => {
                let text = jade::option(row.key).map_or_else(String::new, |o| jade::value_label(game, o));
                ui::button(batch, atlas, vx, vy, vw, vh, &Text::literal(text), enabled, enabled && hover);
            }
            Kind::Keybind(i) => {
                let name = key_label(game, jade::KEYS[i][0]);
                let text = if self.binding == Some(i) { Text::colored(format!("> {name} <"), 0xFFFF55) } else { Text::literal(name) };
                ui::button(batch, atlas, vx, vy, vw, vh, &text, enabled, enabled && hover);
            }
            Kind::Adjust => ui::button(batch, atlas, vx, vy, vw, vh, &Text::literal(tr("config.jade.overlay_pos.adjust", "Adjust...")), true, hover),
            Kind::Reload => ui::button(batch, atlas, vx, vy, vw, vh, &Text::literal(tr("config.jade.reload_plugins.button", "Reload")), true, hover),
            Kind::Reset => ui::button(batch, atlas, vx, vy, vw, vh, &Text::colored(tr("controls.reset", "Reset"), 0xFF5555), true, hover),
            Kind::Title | Kind::Profile(_) => {}
        }
    }

    fn bar_height(&self, list: [f32; 4], most: f32) -> f32 {
        let content = list[3] + most;
        (list[3] * list[3] / content).clamp(32.0, (list[3] - 8.0).max(32.0))
    }

    /// `jade:navbar_background` and `inworld_navbar_background` (the same pixels), or their high_contrast versions.
    fn draw_nav(&self, game: &Game<'_>, batch: &mut HudBatch, m: [f32; 2], high_contrast: bool) {
        let nav = [0.0, SEARCH_H, NAV_W, (self.size[1] - FOOTER - SEARCH_H).max(0.0)];
        if high_contrast {
            game.native_fill(batch, [nav[0], nav[1], nav[2] - 2.0, nav[3]], 0x7000_0000);
            game.native_fill(batch, [nav[2] - 2.0, nav[1], 1.0, nav[3]], 0xFFFF_FF55);
            game.native_fill(batch, [nav[2] - 1.0, nav[1], 1.0, nav[3]], 0xFF12_0D09);
        } else {
            game.native_fill(batch, nav, 0x7000_0000);
        }
        let titles = self.titles();
        let current = self.current_title(m);
        let start = batch.vertices.len();
        for (i, row) in titles.iter().enumerate() {
            let y = nav[1] + i as f32 * NAV_ROW - self.nav_scroll;
            let label = &self.rows[*row].label;
            // OptionsNav.Entry.extractContent.
            if self.focus == Focus::Nav(i) {
                game.native_fill(batch, [0.0, y, NAV_W - 2.0, NAV_ROW], 0x33FF_FFFF);
            }
            game.native_text(batch, label, [10.0, y + 5.0], 0xFFFFFF, true);
            if current == Some(*row) {
                game.native_fill(batch, [2.0, y + 2.0, 2.0, NAV_ROW - 4.0], 0xFFFF_FFFF);
            }
        }
        batch.clip_since(start, nav[0], nav[1], nav[0] + nav[2] - 2.0, nav[1] + nav[3]);
    }

    /// The search box: `jade:search_box_background`, or its high_contrast version with a line under it.
    fn draw_search(&self, game: &Game<'_>, batch: &mut HudBatch, high_contrast: bool) {
        let [x, y, w, h] = [0.0, 0.0, NAV_W, SEARCH_H];
        if high_contrast {
            game.native_fill(batch, [x, y, w - 2.0, h - 1.0], 0x7000_0000);
            game.native_fill(batch, [x, y + h - 1.0, w - 2.0, 1.0], 0xFFFF_FF55);
            game.native_fill(batch, [x + w - 2.0, y, 1.0, h], 0xFFFF_FF55);
            game.native_fill(batch, [x + w - 1.0, y, 1.0, h], 0xFF12_0D09);
        } else {
            game.native_fill(batch, [x, y, w, h], 0xA600_0000);
        }
        let focused = self.focus == Focus::Search;
        self.draw_field(game, batch, &self.search, [x, y, w - 18.0, h], 12.0, 6.0, focused, &tr("gui.jade.search.hint", "Search..."), 0xE0E0E0);
        // NotUglyEditBox.alwaysRenderCross.
        if !self.search.text.is_empty() {
            game.native_text(batch, "x", [w - 12.0, y + 5.0], 0xAAAAAA, true);
        }
    }

    /// An EditBox's text, scrolled to its caret, with its hint while it is empty.
    #[allow(clippy::too_many_arguments)]
    fn draw_field(&self, game: &Game<'_>, batch: &mut HudBatch, field: &Field, [x, y, w, h]: [f32; 4], text_x: f32, text_y: f32, focused: bool, hint: &str, color: u32) {
        let start = batch.vertices.len();
        if field.text.is_empty() && !focused {
            game.native_text(batch, hint, [x + text_x, y + text_y], 0x808080, true);
        } else {
            let before: String = field.text.chars().take(field.caret).collect();
            let caret = game.native_text_width(&before);
            let shift = (caret - (w - text_x - 2.0)).max(0.0);
            game.native_text(batch, &field.text, [x + text_x - shift, y + text_y], color, true);
            if focused && self.opened.elapsed().as_millis() / 300 % 2 == 0 {
                game.native_fill(batch, [x + text_x - shift + caret, y + text_y - 1.0, 1.0, 10.0], 0xFFE0_E0E0);
            }
        }
        batch.clip_since(start, x + text_x - 1.0, y, x + w, y + h);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_row(&self, game: &Game<'_>, batch: &mut HudBatch, row: &Row, row_index: usize, left: f32, width: f32, y: f32, m: [f32; 2]) {
        let atlas = &game.gui.atlas;
        let white = rgba(255, 255, 255, 255);
        let rects = row_rects(left, width, y, row.ignore);
        let enabled = self.row_enabled(game, row);
        match row.kind {
            // OptionsList.Title: centred.
            Kind::Title => {
                let label = if row.danger { Text::colored(row.label.as_str(), 0xFF5555) } else { Text::literal(row.label.as_str()) };
                let tw = game.native_text_width(&row.label);
                batch.text_styled(atlas, &label, left + ((width - tw) / 2.0).floor(), y + 9.0, white, true);
                return;
            }
            // ProfileEntry: its name in an edit box, Use and Save.
            Kind::Profile(i) => {
                let focused = self.focus == Focus::Profile(i);
                if focused || (enabled && hit(rects.name, m)) {
                    let sprite = if focused { "gui/sprites/widget/text_field_highlighted" } else { "gui/sprites/widget/text_field" };
                    ui::nine_slice(batch, atlas, sprite, rects.name[0], rects.name[1], rects.name[2], rects.name[3], u32::MAX);
                }
                let active = enabled && jade::opt(game, "profile") == i.to_string();
                let color = if active { ACTIVE_COLOR } else if enabled { NAME_COLOR } else { 0x808080 };
                self.draw_field(game, batch, &self.names[i], rects.name, 4.0, 7.0, focused, &row.label, color);
                let use_it = enabled && !active;
                ui::button(batch, atlas, rects.use_it[0], rects.use_it[1], rects.use_it[2], rects.use_it[3], &Text::literal(tr("gui.jade.profile.use", "Use")), use_it, use_it && hit(rects.use_it, m));
                ui::button(batch, atlas, rects.save[0], rects.save[1], rects.save[2], rects.save[3], &Text::literal(tr("selectWorld.edit.save", "Save")), use_it, use_it && hit(rects.save, m));
                return;
            }
            _ => {}
        }
        // The title, scrolling when it doesn't fit (JadeMultiLineTextWidget's TextOverflow.SCROLLING).
        let label_x = left + 10.0 + 12.0 * f32::from(row.depth);
        let label = if row.danger {
            Text::colored(row.label.as_str(), 0xFF5555)
        } else if enabled {
            Text::literal(row.label.as_str())
        } else {
            Text::colored(row.label.as_str(), 0xAAAAAA)
        };
        ui::scrolling_text(batch, atlas, &label, label_x, rects.value[0] - 4.0, y, y + ROW, y + 9.0, white);
        let hover = hit(rects.value, m) || (self.keyboard && self.focused_row() == Some(row_index));
        self.draw_value(game, batch, row, row_index, rects.value, hover, enabled);
        if row.ignore {
            // `editIgnoreList`: its ☰ as the three bars it is.
            let r = rects.ignore;
            let over = hit(r, m);
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &Text::literal(""), true, over);
            for bar in 0..3 {
                game.native_fill(batch, [r[0] + 5.0, r[1] + 6.0 + 3.0 * bar as f32, 10.0, 2.0], 0xFFFF_FFFF);
            }
        }
        if !matches!(row.kind, Kind::Reload | Kind::Reset) {
            let changed = self.row_changed(game, row);
            let r = rects.reset;
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &Text::literal("R"), changed, changed && hit(r, m));
        }
    }

    fn profile_title(&self, i: usize) -> String {
        let name = self.names[i].text.trim();
        if name.is_empty() { tr(&format!("config.jade.profile.{i}"), &format!("#{i}")) } else { name.to_owned() }
    }

    fn press(&mut self, game: &Game<'_>, m: [f32; 2], button: i32, modifiers: u32) {
        self.keyboard = false;
        // OptionsList.mouseClicked: the button becomes the key (InputConstants.Type.MOUSE).
        if let Some(i) = self.binding.take() {
            let bound = match button {
                0 => "key.mouse.left".to_owned(),
                1 => "key.mouse.right".to_owned(),
                2 => "key.mouse.middle".to_owned(),
                n => format!("key.mouse.{}", n + 1),
            };
            queue(Action::SetKey(jade::KEYS[i][0], bound));
            return;
        }
        if button != 0 {
            return;
        }
        if let Some(confirm) = self.confirm {
            let (yes, no) = confirm_rects(self.size);
            if hit(yes, m) {
                click_sound();
                self.confirm = None;
                self.do_confirm(game, confirm);
            } else if hit(no, m) {
                click_sound();
                self.confirm = None;
            }
            return;
        }
        if self.adjusting {
            self.press_adjusting(game, m);
            return;
        }
        if self.page == Page::Home {
            self.focus = Focus::None;
            if let Some(i) = home_rects(self.size).iter().position(|r| hit(*r, m)) {
                self.home_button(game, i);
            }
            return;
        }
        let was = std::mem::replace(&mut self.focus, Focus::None);
        if hit(done_rect(self.size), m) {
            click_sound();
            self.enter(game, Page::Home);
            return;
        }
        if self.has_preview_button(game) && hit(preview_rect(self.size), m) {
            click_sound();
            self.preview = !self.preview;
            game.native_set_setting(ID, "preview_overlay", if self.preview { "true" } else { "false" });
            return;
        }
        // The search box and its cross.
        if hit([0.0, 0.0, NAV_W, SEARCH_H], m) {
            if m[0] >= NAV_W - 16.0 && !self.search.text.is_empty() {
                click_sound();
                self.search = Field::default();
                self.filter();
            }
            self.focus = Focus::Search;
            return;
        }
        // OptionsNav.Entry.onPress: showOnTop.
        if hit([0.0, SEARCH_H, NAV_W, self.size[1] - FOOTER - SEARCH_H], m) {
            let index = ((m[1] - SEARCH_H + self.nav_scroll) / NAV_ROW) as usize;
            if let Some(row) = self.titles().get(index).copied() {
                click_sound();
                self.show_on_top(row);
            }
            return;
        }
        // The scrollbar.
        let list = list_rect(self.size);
        let most = self.max_scroll();
        if most > 0.0 && m[0] >= self.size[0] - 6.0 && hit(list, m) {
            let handle = self.bar_height(list, most);
            let top = list[1] + self.scroll / most * (list[3] - handle);
            let grab = if m[1] >= top && m[1] < top + handle { m[1] - top } else { handle / 2.0 };
            self.drag = Drag::Bar(grab);
            return;
        }
        if !hit(list, m) {
            return;
        }
        if self.cards {
            self.press_cards(game, m, modifiers & 1 != 0);
            return;
        }
        let index = ((m[1] - list[1] - 2.0 + self.scroll) / ROW).floor();
        if index < 0.0 {
            return;
        }
        let Some(row) = self.shown.get(index as usize).copied() else { return };
        let y = list[1] + 2.0 - self.scroll + index * ROW;
        let shift = modifiers & 1 != 0;
        let ctrl = modifiers & 2 != 0;
        self.press_row(game, row, y, m, shift, ctrl, was);
    }

    #[allow(clippy::too_many_arguments)]
    fn press_row(&mut self, game: &Game<'_>, row_index: usize, y: f32, m: [f32; 2], shift: bool, ctrl: bool, was: Focus) {
        let list = list_rect(self.size);
        let (left, width) = row_span(list);
        let row = &self.rows[row_index];
        let rects = row_rects(left, width, y, row.ignore);
        let enabled = self.row_enabled(game, row);
        let kind = row.kind;
        if let Kind::Profile(i) = kind {
            if !enabled {
                return;
            }
            let active = jade::opt(game, "profile") == i.to_string();
            if hit(rects.name, m) {
                self.focus = Focus::Profile(i);
                if was != Focus::Profile(i) {
                    self.names[i].caret = self.names[i].text.chars().count();
                }
            } else if !active && hit(rects.use_it, m) {
                click_sound();
                jade::use_profile(game, &i.to_string());
            } else if !active && hit(rects.save, m) {
                click_sound();
                // ProfileEntry: Ctrl saves without asking.
                if ctrl {
                    jade::save_profile(game, &i.to_string());
                } else {
                    self.confirm = Some(Confirm::SaveProfile(i));
                }
            }
            return;
        }
        if kind == Kind::Title {
            return;
        }
        if !matches!(kind, Kind::Reload | Kind::Reset) && hit(rects.reset, m) {
            if self.row_changed(game, &self.rows[row_index]) {
                click_sound();
                self.reset_row(game, row_index);
            }
            return;
        }
        if self.rows[row_index].ignore && hit(rects.ignore, m) {
            click_sound();
            queue(Action::OpenIgnoreLists);
            return;
        }
        if !hit(rects.value, m) || !enabled {
            return;
        }
        if matches!(kind, Kind::Slider { .. }) {
            self.press_slider(game, row_index, rects.value, m);
        } else {
            self.use_row(game, row_index, shift);
        }
    }

    fn press_slider(&mut self, game: &Game<'_>, row_index: usize, value: [f32; 4], m: [f32; 2]) {
        click_sound();
        let track = [value[0], value[2]];
        self.drag = Drag::Slider(row_index, track);
        self.slide_to(game, row_index, track, m[0]);
    }

    /// HomeConfigScreen's buttons: mod settings, plugin settings, profiles, done.
    fn home_button(&mut self, game: &Game<'_>, i: usize) {
        click_sound();
        match i {
            0 => self.enter(game, Page::Mod),
            1 => self.enter(game, Page::Plugins),
            2 => self.enter(game, Page::Profiles),
            _ => queue(Action::Close),
        }
    }

    /// OptionsList.mouseClicked and Entry.mouseClicked in card mode: the popup's rows first (nothing goes through the
    /// panel), then a card: its widget, or anywhere else on one with options to open its popup.
    fn press_cards(&mut self, game: &Game<'_>, m: [f32; 2], shift: bool) {
        if let Some(entry) = self.expanded
            && let Some(panel) = self.popup_rect(entry)
            && hit(panel, m)
        {
            let slide = self.popup_slide_offset();
            let k = ((m[1] - panel[1] - 3.0 - slide) / ROW).floor();
            if k >= 0.0
                && let Some(row) = self.rows[entry].children.get(k as usize).copied()
            {
                let y = panel[1] + 3.0 + slide + k * ROW;
                let value = Self::popup_value(panel, y);
                if hit(value, m) && self.row_enabled(game, &self.rows[row]) {
                    if matches!(self.rows[row].kind, Kind::Slider { .. }) {
                        self.press_slider(game, row, value, m);
                    } else {
                        self.use_row(game, row, shift);
                    }
                }
            }
            return;
        }
        for at in 0..self.shown.len() {
            let Some([x, y, w, h]) = self.card_rect(at) else { continue };
            let row = self.shown[at];
            if self.rows[row].kind == Kind::Title || !hit([x - CARD_BLEED, y - CARD_BLEED, w + 2.0 * CARD_BLEED, h + 2.0 * CARD_BLEED], m) {
                continue;
            }
            let (cx, cy, cw, ch) = (x + 2.0, y + 2.0, w - 4.0, h - 4.0);
            if self.rows[row].kind == Kind::Toggle {
                let mid = cy + (ch / 2.0).floor();
                if hit([cx + cw - TOGGLE_W - 4.0, mid - (TOGGLE_H / 2.0).floor(), TOGGLE_W, TOGGLE_H], m) {
                    self.pressed = Some(row);
                    self.use_row(game, row, shift);
                    return;
                }
            } else {
                let value = [cx + 4.0, cy + ch - 2.0 - 20.0, cw - 8.0, 20.0];
                if hit(value, m) {
                    if self.row_enabled(game, &self.rows[row]) {
                        if matches!(self.rows[row].kind, Kind::Slider { .. }) {
                            self.press_slider(game, row, value, m);
                        } else {
                            self.use_row(game, row, shift);
                        }
                    }
                    return;
                }
            }
            if !self.rows[row].children.is_empty() {
                self.pending = Some((row, m, std::time::Instant::now()));
            }
            return;
        }
    }

    fn do_confirm(&mut self, game: &Game<'_>, confirm: Confirm) {
        match confirm {
            // WailaConfigScreen's reset: Jade.resetConfig, and its key mappings' defaults.
            Confirm::Reset => {
                jade::reset_settings(game);
                queue(Action::ResetKeys);
                self.rows = build(game, self.page);
                self.filter();
                self.scroll = self.max_scroll();
            }
            Confirm::SaveProfile(i) => jade::save_profile(game, &i.to_string()),
        }
    }

    /// PreviewOptionsScreen.mouseClicked: the overlay to drag, the screen's edges to send it to, the centre to leave.
    fn press_adjusting(&mut self, game: &Game<'_>, m: [f32; 2]) {
        let r = self.rect;
        if hit(r, m) {
            self.drag = Drag::Overlay([m[0] - (r[0] + r[2] / 2.0), m[1] - (r[1] + r[3] / 2.0)]);
            return;
        }
        click_sound();
        let [w, h] = self.size;
        let x_index = ((m[0] / (w / 3.0)) as i32).clamp(0, 2);
        let y_index = ((m[1] / (h / 3.0)) as i32).clamp(0, 2);
        if x_index == 1 && y_index == 1 {
            self.exit_adjusting(game);
        } else {
            let (fx, fy) = (x_index as f32 / 2.0, y_index as f32 / 2.0);
            let fx = try_flip(game, fx);
            jade::set_live(game, "overlay_x", format!("custom:{fx}:{fx}"));
            jade::set_live(game, "position", format!("custom:{}:{fy}", 1.0 - fy));
            self.keep_position(game);
        }
    }

    fn wheel(&mut self, m: [f32; 2], amount: f32, modifiers: u32) {
        if self.adjusting || self.confirm.is_some() || self.page == Page::Home {
            return;
        }
        if hit([0.0, SEARCH_H, NAV_W, self.size[1] - FOOTER - SEARCH_H], m) {
            let most = (self.titles().len() as f32 * NAV_ROW - (self.size[1] - FOOTER - SEARCH_H)).max(0.0);
            self.nav_scroll = (self.nav_scroll - amount * NAV_ROW * 3.0).clamp(0.0, most);
        } else {
            // SmoothScrollableList.scrollRate: three rows, nine with Ctrl.
            let rate = ROW * if modifiers & 2 != 0 { 9.0 } else { 3.0 };
            self.scroll = (self.scroll - amount * rate).clamp(0.0, self.max_scroll());
        }
    }

    fn key(&mut self, game: &Game<'_>, key: &str, repeat: bool, modifiers: u32) {
        let (shift, ctrl) = (modifiers & 1 != 0, modifiers & 2 != 0);
        const ESCAPE: &str = "key.keyboard.escape";
        if let Some(i) = self.binding.take() {
            // OptionsList.keyPressed: Escape unbinds it.
            let bound = if key == ESCAPE { "key.keyboard.unknown" } else { key };
            queue(Action::SetKey(jade::KEYS[i][0], bound.to_owned()));
            return;
        }
        if repeat && key == ESCAPE {
            return;
        }
        if let Some(confirm) = self.confirm {
            match key {
                ESCAPE => self.confirm = None,
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                    self.confirm = None;
                    self.do_confirm(game, confirm);
                }
                _ => {}
            }
            return;
        }
        if self.adjusting {
            // PreviewOptionsScreen.moveOverlayRelatively.
            let step = if shift { 20.0 } else { 2.0 };
            let [rx, ry, rw, rh] = self.rect;
            let (cx, cy) = (rx + rw / 2.0, ry + rh / 2.0);
            match key {
                "key.keyboard.up" => self.move_overlay(game, cx, cy - step, false),
                "key.keyboard.down" => self.move_overlay(game, cx, cy + step, false),
                "key.keyboard.left" => self.move_overlay(game, cx - step, cy, false),
                "key.keyboard.right" => self.move_overlay(game, cx + step, cy, false),
                ESCAPE => {
                    click_sound();
                    self.exit_adjusting(game);
                }
                _ => {}
            }
            return;
        }
        if key == ESCAPE {
            if self.page == Page::Home {
                queue(Action::Close);
            } else {
                self.enter(game, Page::Home);
            }
            return;
        }
        const SELECT: [&str; 3] = ["key.keyboard.enter", "key.keyboard.keypad.enter", "key.keyboard.space"];
        if self.page == Page::Home {
            // The Screen's own focus: the four buttons in order.
            let at = if let Focus::Home(i) = self.focus { Some(i) } else { None };
            match key {
                "key.keyboard.tab" if shift => self.focus = Focus::Home(at.map_or(3, |i| (i + 3) % 4)),
                "key.keyboard.left" | "key.keyboard.up" => self.focus = Focus::Home(at.map_or(3, |i| (i + 3) % 4)),
                "key.keyboard.tab" | "key.keyboard.right" | "key.keyboard.down" => self.focus = Focus::Home(at.map_or(0, |i| (i + 1) % 4)),
                _ if SELECT.contains(&key) => {
                    if let Some(i) = at {
                        self.home_button(game, i);
                    }
                }
                _ => {}
            }
            return;
        }
        match self.focus {
            Focus::Search => {
                if self.search.key(key) {
                    self.filter();
                }
            }
            Focus::Profile(i) => {
                if self.names[i].key(key) {
                    game.native_set_setting(ID, &format!("profile_name_{i}"), &self.names[i].text);
                }
            }
            _ => {}
        }
        // AbstractSliderButton: Enter lets a focused slider's arrows move it; they do until Enter again or the focus goes.
        if let Focus::Row(row) = self.focus
            && matches!(self.rows[row].kind, Kind::Slider { .. })
        {
            if SELECT.contains(&key) {
                self.slider_keys = !self.slider_keys;
                return;
            }
            if self.slider_keys && matches!(key, "key.keyboard.left" | "key.keyboard.right") {
                self.step_slider(game, row, if key == "key.keyboard.left" { -1.0 } else { 1.0 });
                return;
            }
        }
        match key {
            // BaseOptionsScreen.keyPressed.
            "key.keyboard.tab" if ctrl && !shift => self.focus = Focus::Search,
            "key.keyboard.tab" => {
                self.tab(game, !shift);
                return;
            }
            "key.keyboard.page.up" => self.scroll = (self.scroll - ROW * 9.0).max(0.0),
            "key.keyboard.page.down" => self.scroll = (self.scroll + ROW * 9.0).min(self.max_scroll()),
            _ => {}
        }
        let arrow = matches!(key, "key.keyboard.up" | "key.keyboard.down" | "key.keyboard.left" | "key.keyboard.right");
        match self.focus {
            // The search box keeps left and right for its caret; down leaves it for the list.
            Focus::Search => {
                if key == "key.keyboard.down" {
                    self.focus = Focus::None;
                    if self.cards {
                        self.arrow_cards(game, key);
                    } else {
                        self.arrow_list(game, key);
                    }
                }
            }
            Focus::Nav(i) => {
                let titles = self.titles();
                match key {
                    "key.keyboard.up" => match i.checked_sub(1) {
                        Some(i) => self.focus = Focus::Nav(i),
                        None => self.focus = Focus::Search,
                    },
                    "key.keyboard.down" if i + 1 < titles.len() => self.focus = Focus::Nav(i + 1),
                    "key.keyboard.right" => {
                        // Back to the first row of that section.
                        if let Some(row) = titles.get(i).and_then(|t| self.shown.iter().copied().find(|r| self.rows[*r].parent == Some(*t) && self.focusable(game, *r))) {
                            self.focus_row(row);
                        }
                    }
                    _ if SELECT.contains(&key) => {
                        if let Some(row) = titles.get(i).copied() {
                            click_sound();
                            self.show_on_top(row);
                        }
                    }
                    _ => {}
                }
            }
            // A name's box keeps left and right too.
            Focus::Profile(i) => {
                if matches!(key, "key.keyboard.up" | "key.keyboard.down") {
                    self.arrow_list(game, key);
                } else if SELECT.contains(&key) && key != "key.keyboard.space" && jade::on(game, "enable_profiles") && jade::opt(game, "profile") != i.to_string() {
                    click_sound();
                    jade::use_profile(game, &i.to_string());
                }
            }
            Focus::Row(row) if SELECT.contains(&key) => self.use_row(game, row, shift),
            Focus::Row(_) | Focus::None | Focus::Home(_) if arrow => {
                if self.cards {
                    self.arrow_cards(game, key);
                } else {
                    self.arrow_list(game, key);
                }
            }
            _ => {}
        }
    }

    fn typed(&mut self, game: &Game<'_>, c: char) {
        if c.is_control() {
            return;
        }
        match self.focus {
            Focus::Search => {
                self.search.insert(c, 64);
                self.filter();
            }
            Focus::Profile(i) => {
                // ProfileEntry: setMaxLength(100).
                self.names[i].insert(c, 100);
                game.native_set_setting(ID, &format!("profile_name_{i}"), &self.names[i].text);
            }
            _ => {}
        }
    }
}

/// `OPTION_ON` and `OPTION_OFF`.
fn on_off(on: bool) -> Text {
    if on { Text::colored(tr("options.on", "On"), ON_COLOR) } else { Text::colored(tr("options.off", "Off"), OFF_COLOR) }
}

/// `cardsPerRow`.
fn cards_per_row(size: [f32; 2]) -> f32 {
    (card_span(size).1 / MIN_CARD_WIDTH).floor().clamp(1.0, MAX_CARDS_PER_ROW)
}

/// `getRowLeft` and `getRowWidth` in card mode (`cardRowWidth`).
fn card_span(size: [f32; 2]) -> (f32, f32) {
    let list = list_rect(size);
    let width = (list[2] - 16.0).clamp(0.0, 900.0);
    (list[0] + ((list[2] - width) / 2.0).floor(), width)
}

/// `card_background`, `_hover`, `_expandable` and `_expandable_hover`, from their PNGs: the frame and fill stand a bleed
/// outside the card, the frame's corners cut, and one with options lightens over its last seven rows.
fn draw_card_background(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: [f32; 4], expandable: bool, hovered: bool) {
    const SHADE: [u32; 7] = [0xA116_1A24, 0xA219_1E28, 0xA31C_222D, 0xA420_2733, 0xA525_2D3A, 0xA62A_3442, 0xA72F_3A4A];
    const SHADE_HOVER: [u32; 7] = [0xB81F_2634, 0xB922_2A39, 0xBA26_2F3F, 0xBB2B_3546, 0xBC30_3B4D, 0xBE36_4356, 0xBF3C_4B60];
    let (frame, fill) = match (expandable, hovered) {
        (false, false) => (0x33FF_FFFF, 0x9912_1216),
        (false, true) => (0x66FF_FFFF, 0xB01C_1C24),
        (true, false) => (0x557F_A8D0, 0x9912_151E),
        (true, true) => (0x779F_C8F0, 0xB01C_2230),
    };
    let (ox, oy, ow, oh) = (x - CARD_BLEED, y - CARD_BLEED, w + 2.0 * CARD_BLEED, h + 2.0 * CARD_BLEED);
    game.native_fill(batch, [ox + 1.0, oy, ow - 2.0, 1.0], frame);
    game.native_fill(batch, [ox + 1.0, oy + oh - 1.0, ow - 2.0, 1.0], frame);
    game.native_fill(batch, [ox, oy + 1.0, 1.0, oh - 2.0], frame);
    game.native_fill(batch, [ox + ow - 1.0, oy + 1.0, 1.0, oh - 2.0], frame);
    let (ix, iy, iw, ih) = (ox + 1.0, oy + 1.0, ow - 2.0, oh - 2.0);
    if expandable {
        game.native_fill(batch, [ix, iy, iw, ih - 7.0], fill);
        for (i, shade) in (if hovered { SHADE_HOVER } else { SHADE }).into_iter().enumerate() {
            game.native_fill(batch, [ix, iy + ih - 7.0 + i as f32, iw, 1.0], shade);
        }
    } else {
        game.native_fill(batch, [ix, iy, iw, ih], fill);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ToggleState {
    Normal,
    Hovered,
    Focused,
    Pressed,
}

/// ToggleButton's `toggle/<on|off>_<state>` sprite, from its PNG, as runs of fills: `(x0, y0, x1, y1, ARGB)`, inclusive.
fn draw_toggle(game: &Game<'_>, batch: &mut HudBatch, [x, y]: [f32; 2], on: bool, state: ToggleState) {
    const INK: u32 = 0xFF1E_1E1F;
    const SHADOW: u32 = 0xFF58_585A;
    // The knob's lights: its left and top edge, its right and lower edge, its face.
    let (edge, under, face) = match state {
        ToggleState::Hovered => (0xFFFB_FBFD, 0xFFF8_FAFB, 0xFFF4_F6F9),
        ToggleState::Pressed => (0xFFE0_E0E1, 0xFFD0_D1D3, 0xFFB1_B2B5),
        ToggleState::Normal | ToggleState::Focused => (0xFFEC_EDEE, 0xFFE3_E3E5, 0xFFD0_D1D4),
    };
    let mut r = |x0: u8, y0: u8, x1: u8, y1: u8, argb: u32| {
        game.native_fill(batch, [x + f32::from(x0), y + f32::from(y0), f32::from(x1 - x0 + 1), f32::from(y1 - y0 + 1)], argb);
    };
    // Where the knob's left edge is: on the right side when on.
    let k: u8 = if on { 14 } else { 1 };
    // The outline: the track's and the knob's together.
    r(1, 15, 27, 15, INK);
    r(k, 1, k + 13, 1, INK);
    r(k, 2, k, 14, INK);
    r(k + 13, 2, k + 13, 14, INK);
    // The knob.
    r(k + 1, 2, k + 11, 2, edge);
    r(k + 2, 3, k + 11, 11, face);
    r(k + 12, 2, k + 12, 12, under);
    r(k + 2, 12, k + 11, 12, under);
    r(k + 2, 13, k + 12, 14, SHADOW);
    // The left edge runs to the foot on the on sprite; the off sprite's stops above its shadow.
    if on {
        r(k + 1, 3, k + 1, 14, edge);
    } else {
        r(k + 1, 3, k + 1, 11, edge);
        r(k + 1, 12, k + 1, 12, under);
        r(k + 1, 13, k + 1, 14, SHADOW);
    }
    if on {
        // The track, green, with its I.
        r(1, 3, 13, 3, INK);
        r(1, 4, 1, 14, INK);
        r(2, 4, 13, 4, 0xFF63_9D52);
        r(2, 5, 2, 13, 0xFF63_9D52);
        r(3, 5, 13, 13, 0xFF3C_8527);
        r(2, 14, 13, 14, 0xFF4F_913C);
        r(8, 7, 8, 11, 0xFFFF_FFFF);
    } else {
        // The track, grey, with its O.
        r(15, 3, 27, 3, INK);
        r(27, 4, 27, 14, INK);
        r(15, 4, 25, 4, 0xFFA3_A4A6);
        r(26, 4, 26, 13, 0xFF97_989B);
        r(15, 5, 25, 13, 0xFF8C_8D90);
        r(15, 14, 26, 14, 0xFF97_989B);
        r(18, 7, 22, 7, 0xFF24_2425);
        r(18, 8, 18, 10, 0xFF24_2425);
        r(22, 8, 22, 10, 0xFF24_2425);
        r(18, 11, 22, 11, 0xFF24_2425);
    }
    if state == ToggleState::Focused {
        // The white ring round the knob.
        let (a, b): (u8, u8) = if on { (13, 28) } else { (0, 15) };
        r(a, 0, b, 0, 0xFFFF_FFFF);
        r(a, 16, b, 16, 0xFFFF_FFFF);
        r(a, 1, a, 15, 0xFFFF_FFFF);
        r(b, 1, b, 15, 0xFFFF_FFFF);
    }
}

/// An option's first value, Jade's or a plugin's.
fn default_of(key: &str) -> String {
    if let Some(o) = jade::option(key) {
        return o.default.to_owned();
    }
    jade::plugin_options().into_iter().find(|(k, _, _)| *k == key).map_or_else(|| "true".to_owned(), |(_, _, on)| on.to_string())
}
