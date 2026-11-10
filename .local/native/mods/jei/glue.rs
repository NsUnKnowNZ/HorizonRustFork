// The edges between Horizon and the viewer (crate::game::jei): the game's
// item and recipe tables in, the panel, give and place out, and the overlay
// (tooltips, the bookmark bar and its clicks) wired to the viewer.
use crate::game::Game;
use crate::game::jei::{
    self, Bookmarks, Config, Context, Counts, Custom, Event, Ingredient, Item, Kind, Layout,
    Lookup, Out, Recipe, Session, Sort, Source, Stack, Toggle, View, Viewer,
};
use crate::game::native_hooks::{
    NativePanelEvent, NativeRecipeKind, NativeRecipeLayout, NativeRecipeStack, NativeRecipeView,
    NativeRecipes, NativeScreenView, NativeStack,
};
use horizon_client::items::ItemStack;
use std::cell::RefCell;
use std::path::PathBuf;

/// Variants one item adds to the list (potions, tipped arrows).
const VARIANTS_PER_ITEM: usize = 8;

/// Shift's GLFW bit.
const SHIFT: u32 = 1;

/// The viewer while a world is open, and what the overlay keeps.
struct State {
    session: Session,
    bookmarks: Bookmarks,
    config: Config,
    /// The config and the bookmarks have been read from disk.
    loaded: bool,
    /// Actions a hook that only borrows the game leaves to the frame.
    pending: Vec<Event>,
    /// The bookmark under the left button, while it is down.
    drag: Option<Drag>,
}

/// A bookmark being dragged to another slot.
#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
    /// The pointer moved to another slot: a drop reorders instead of opening.
    moved: bool,
}

impl State {
    fn new() -> Self {
        Self {
            session: Session::new(),
            bookmarks: Bookmarks::default(),
            config: Config::default(),
            loaded: false,
            pending: Vec::new(),
            drag: None,
        }
    }

    fn viewer(&self) -> Option<&Viewer> {
        self.session.viewer()
    }

    fn viewer_mut(&mut self) -> Option<&mut Viewer> {
        self.session.viewer_mut()
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

/// The folder the mod keeps its files in, `config/jei/`, inside the game's.
fn dir() -> Option<PathBuf> {
    let mut path = Game::native_config_dir().ok()?;
    path.push(jei::ID);
    Some(path)
}

fn config_path() -> Option<PathBuf> {
    Some(dir()?.join("jei.cfg"))
}

fn bookmarks_path() -> Option<PathBuf> {
    Some(dir()?.join("bookmarks.txt"))
}

fn read(path: Option<PathBuf>) -> Option<String> {
    std::fs::read_to_string(path?).ok()
}

fn write(path: Option<PathBuf>, text: &str) {
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

/// Reads the config and the bookmarks once: `config/jei/jei.cfg` and
/// `config/jei/bookmarks.txt`.
fn load() {
    if STATE.with(|s| s.borrow().loaded) {
        return;
    }
    let config = read(config_path()).map_or_else(Config::default, |text| Config::parse(&text));
    let bookmarks =
        read(bookmarks_path()).map_or_else(Bookmarks::default, |text| jei::bookmarks_parse(&text));
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.config = config;
        state.bookmarks = bookmarks;
        state.loaded = true;
    });
}

/// The config now, read from disk the first time.
pub(super) fn config() -> Config {
    load();
    STATE.with(|s| s.borrow().config)
}

fn store_config(config: Config) {
    STATE.with(|s| s.borrow_mut().config = config);
    write(config_path(), &config.to_text());
}

fn save_bookmarks(bookmarks: &Bookmarks) {
    write(bookmarks_path(), &jei::bookmarks_text(bookmarks));
}

/// Where the config and the bookmarks live, for `/jei config`.
pub(super) fn files() -> String {
    match (config_path(), bookmarks_path()) {
        (Some(config), Some(bookmarks)) => {
            format!(
                "JEI files: {} and {}",
                config.display(),
                bookmarks.display()
            )
        }
        _ => "JEI: the game's config folder is unknown".to_owned(),
    }
}

/// The player's language.
fn lang(game: &Game<'_>) -> jei::Lang {
    jei::lang(game.native_setting(jei::ID, "language").as_deref())
}

/// Builds the viewer once per world and shows the item list in the panel.
pub(super) fn sync(game: &mut Game<'_>) {
    let has_player = game.native_player().is_some();
    let enabled = game.native_on(jei::ID);
    let needs_build = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let needs_build = state.session.update(has_player, enabled);
        if !has_player {
            state.drag = None;
            state.pending.clear();
        }
        needs_build
    });
    if !needs_build {
        return;
    }
    let Some(catalog) = game.native_recipes() else {
        return;
    };
    load();
    let sort = STATE.with(|s| s.borrow().config.sort);
    let items = items(&catalog);
    let mut recipes = recipes(&catalog);
    let custom = custom(game, &items);
    recipes.extend(custom.recipes);
    let found = jei::harvest_variants(&recipes, VARIANTS_PER_ITEM, jei::ITEMS_MAX);
    let (mut viewer, mut out) = Viewer::with_groups(
        items,
        recipes,
        found,
        custom.benches,
        custom.groups,
        lang(game),
    );
    if sort != viewer.sort() {
        out = viewer.set_sort(sort);
    }
    STATE.with(|s| s.borrow_mut().session.set_viewer(viewer));
    if let Err(error) = apply(game, out) {
        game.native_chat(&format!("JEI: {error}"));
    }
}

/// Runs what the overlay's hooks left for it (a bookmark click opens the
/// panel), once a frame, where the game can still be changed.
pub(super) fn flush(game: &mut Game<'_>) {
    if !game.native_on(jei::ID) {
        return;
    }
    let pending: Vec<Event> = STATE.with(|s| std::mem::take(&mut s.borrow_mut().pending));
    for event in pending {
        dispatch(game, event);
    }
}

/// The panel's events: search, clicks, keys, tabs, pages and the action button.
pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) {
    if !game.native_on(jei::ID) {
        return;
    }
    let Some(event) = convert_event(event) else {
        return;
    };
    // The bookmark key bookmarks the stack under the pointer, wherever it is:
    // the game says which one it is.
    if let Event::Key { key, stack } = &event {
        let bookmark_key = game.native_mod_key(jei::ID, "bookmark", jei::BOOKMARK_KEY);
        if *key == bookmark_key {
            bookmark(game, stack.clone());
            return;
        }
    }
    dispatch(game, event);
}

/// Bookmarks a stack, or takes its bookmark away, and says so.
fn bookmark(game: &mut Game<'_>, stack: Stack) {
    let lang = lang(game);
    let result = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let result = state.bookmarks.toggle(&stack);
        if result != Toggle::Full {
            save_bookmarks(&state.bookmarks);
        }
        result
    });
    let name = STATE
        .with(|s| s.borrow().viewer().map(|v| v.item_name(&stack.item)))
        .unwrap_or_else(|| stack.item.clone());
    let text = match result {
        Toggle::Added(_) => format!("{}: {name}", lang.bookmark_added()),
        Toggle::Removed(_) => format!("{}: {name}", lang.bookmark_removed()),
        Toggle::Full => lang.bookmarks_full().to_owned(),
    };
    game.native_chat(&text);
}

/// Takes a bookmark away by its slot, keeping the file.
fn remove_bookmark(index: usize) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if state.bookmarks.remove(index).is_some() {
            save_bookmarks(&state.bookmarks);
        }
        state.drag = None;
    });
}

/// The bookmark bar over a container screen: its slots, the item in each and
/// the name of the one under the pointer, as the game finished drawing.
pub(super) fn screen_render_post(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &NativeScreenView<'_>,
) {
    if !game.native_on(jei::ID) || !jei::overlay_screen(screen.id) {
        return;
    }
    load();
    let (config, bookmarks, drag) = STATE.with(|s| {
        let state = s.borrow();
        (state.config, state.bookmarks.clone(), state.drag)
    });
    if !config.bookmarks || bookmarks.is_empty() {
        return;
    }
    let bar = jei::bookmark_bar(batch.height);
    let count = bookmarks.len().min(bar.capacity());
    let hovered = bar.hit(screen.mouse, count);
    // The bar's box first, so the screen shows through the slots.
    game.native_fill(batch, bar.rect(), 0x5000_0000);
    for index in 0..count {
        let rect = bar.slot(index);
        if hovered == Some(index) || drag.is_some_and(|drag| drag.from == index) {
            game.native_fill(batch, rect, 0x50FF_FFFF);
        }
        if let Some(item) = item_stack(&bookmarks.slots()[index]) {
            game.native_item_stack(batch, &item, [rect[0] + 1.0, rect[1] + 1.0], 16.0);
        }
    }
    let Some(index) = hovered else {
        return;
    };
    let slot = &bookmarks.slots()[index];
    let name = STATE
        .with(|s| s.borrow().viewer().map(|v| v.item_name(&slot.item)))
        .unwrap_or_else(|| slot.item.clone());
    draw_tooltip(
        game,
        batch,
        screen.mouse,
        &jei::bookmark_tooltip(&name, lang(game)),
    );
}

/// A small tooltip at the pointer: the overlay's slots are not the game's, so
/// it draws its own.
fn draw_tooltip(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    mouse: [f32; 2],
    lines: &[(String, [u8; 4])],
) {
    if lines.is_empty() {
        return;
    }
    let mut width: f32 = 0.0;
    for (text, _) in lines {
        width = width.max(game.native_text_width(text));
    }
    let height = lines.len() as f32 * 10.0;
    let x = (mouse[0] + 10.0).min(batch.width - width - 6.0).max(2.0);
    let y = (mouse[1] - 12.0)
        .max(2.0)
        .min((batch.height - height - 6.0).max(2.0));
    game.native_fill(
        batch,
        [x - 3.0, y - 3.0, width + 6.0, height + 6.0],
        0xF0_10_00_10,
    );
    for (n, (text, [r, g, b, _])) in lines.iter().enumerate() {
        let rgb = u32::from_be_bytes([0, *r, *g, *b]);
        game.native_text(batch, text, [x, y + n as f32 * 10.0], rgb, false);
    }
}

/// A press over the bar: the left button takes the slot (opening its recipes
/// when the release comes without a move), the right one opens its uses,
/// shift and the middle button take the bookmark away.
pub(super) fn screen_mouse_pressed(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(jei::ID) || !jei::overlay_screen(screen.id) {
        return;
    }
    load();
    let (config, bookmarks) = STATE.with(|s| {
        let state = s.borrow();
        (state.config, state.bookmarks.clone())
    });
    if !config.bookmarks || bookmarks.is_empty() {
        return;
    }
    // The corner's places do not depend on the screen's height: a hit test
    // uses the whole bar, of which the screen draws as many rows as fit.
    let Some(index) = jei::bookmark_slots().hit(screen.mouse, bookmarks.len()) else {
        return;
    };
    *canceled = true;
    let stack = bookmarks.slots()[index].clone();
    match button {
        // Left: a click opens the recipes, a drag reorders.
        0 if modifiers & SHIFT == 0 => {
            STATE.with(|s| {
                s.borrow_mut().drag = Some(Drag {
                    from: index,
                    moved: false,
                })
            });
        }
        0 => remove_bookmark(index),
        // Right: the uses.
        1 => pending(Event::Open { stack, uses: true }),
        // Middle: take it away.
        2 => remove_bookmark(index),
        _ => {}
    }
}

/// A release over the bar: a left click that never left its slot opens the
/// stack's recipes.
pub(super) fn screen_mouse_released(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    _modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(jei::ID) || !jei::overlay_screen(screen.id) || button != 0 {
        return;
    }
    let Some(drag) = STATE.with(|s| s.borrow_mut().drag.take()) else {
        return;
    };
    *canceled = true;
    if drag.moved {
        return;
    }
    let stack = STATE.with(|s| s.borrow().bookmarks.get(drag.from).cloned());
    if let Some(stack) = stack {
        pending(Event::Open { stack, uses: false });
    }
}

/// Dragging a bookmark over another slot moves it there.
pub(super) fn screen_mouse_dragged(
    game: &Game<'_>,
    screen: &NativeScreenView<'_>,
    button: i32,
    _delta: [f32; 2],
    _modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(jei::ID) || !jei::overlay_screen(screen.id) || button != 0 {
        return;
    }
    let Some(mut drag) = STATE.with(|s| s.borrow().drag) else {
        return;
    };
    let count = STATE.with(|s| s.borrow().bookmarks.len());
    let Some(to) = jei::bookmark_slots().hit(screen.mouse, count) else {
        return;
    };
    *canceled = true;
    if to == drag.from {
        return;
    }
    let moved = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let moved = state.bookmarks.move_slot(drag.from, to);
        if moved {
            save_bookmarks(&state.bookmarks);
        }
        moved
    });
    if moved {
        drag.from = to;
        drag.moved = true;
        STATE.with(|s| s.borrow_mut().drag = Some(drag));
    }
}

fn pending(event: Event) {
    STATE.with(|s| s.borrow_mut().pending.push(event));
}

/// Adds the viewer's lines to any item's tooltip: what makes it, what uses it
/// and whether it is bookmarked.
pub(super) fn tooltip(
    game: &Game<'_>,
    stack: &ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    if !game.native_on(jei::ID) {
        return;
    }
    load();
    let components = crate::game::native_recipes::components(stack);
    let config = STATE.with(|s| s.borrow().config);
    let counts = STATE.with(|s| {
        let state = s.borrow();
        state.viewer().map_or(Counts::default(), |viewer| {
            viewer.counts(&stack.item, &components)
        })
    });
    let marked = STATE.with(|s| {
        s.borrow()
            .bookmarks
            .position(&Stack {
                item: stack.item.to_string(),
                count: 1,
                components: components.clone(),
            })
            .is_some()
    });
    let counts = if config.hints {
        counts
    } else {
        Counts::default()
    };
    let added = jei::tooltip_lines(counts, marked, lang(game));
    if added.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push((horizon_client::text::Text::literal(""), u32::MAX));
    }
    for (text, [r, g, b, a]) in added {
        let color = u32::from_le_bytes([r, g, b, a]);
        lines.push((horizon_client::text::Text::literal(&text), color));
    }
}

/// The settings page's config, as its buttons show it.
pub(super) fn settings() -> Config {
    config()
}

/// The next order, shown in the list at once.
pub(super) fn cycle_sort(game: &mut Game<'_>) {
    set_sort(game, config().sort.next());
}

pub(super) fn set_sort(game: &mut Game<'_>, sort: Sort) {
    let mut config = config();
    config.sort = sort;
    store_config(config);
    let out = STATE.with(|s| s.borrow_mut().viewer_mut().map(|v| v.set_sort(sort)));
    if let Some(out) = out {
        if let Err(error) = apply(game, out) {
            game.native_chat(&format!("JEI: {error}"));
        }
    }
}

pub(super) fn toggle_hints() -> Config {
    let mut config = config();
    config.hints = !config.hints;
    store_config(config);
    config
}

pub(super) fn toggle_bar() -> Config {
    let mut config = config();
    config.bookmarks = !config.bookmarks;
    store_config(config);
    config
}

/// Empties the bar, keeping the file.
pub(super) fn clear_bookmarks() {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.bookmarks.clear();
        state.drag = None;
        save_bookmarks(&state.bookmarks);
    });
}

fn dispatch(game: &mut Game<'_>, event: Event) {
    let known = game.native_known_recipes();
    let recipe_key = game.native_mod_key(jei::ID, "recipe", "key.keyboard.r");
    let uses_key = game.native_mod_key(jei::ID, "uses", "key.keyboard.u");
    let give = jei::give(game.native_setting(jei::ID, "give").as_deref());
    let lang = lang(game);
    let ctx = Context {
        recipe_key: &recipe_key,
        uses_key: &uses_key,
        give,
        // `native_give` succeeds in creative only: try, and fall back below.
        creative: give,
        known: &known,
    };

    let mut fallback = false;
    for out in handle(&event, &ctx, lang) {
        if let Out::Give { item, count } = out {
            if game.native_give(&item, count).is_err() {
                fallback = true;
            }
            continue;
        }
        if let Err(error) = apply(game, out) {
            game.native_chat(&format!("JEI: {error}"));
        }
    }
    if fallback {
        // Survival: the same click opens the recipes, as JEI without cheat mode.
        let ctx = Context {
            give: false,
            creative: false,
            ..ctx
        };
        for out in handle(&event, &ctx, lang) {
            if let Err(error) = apply(game, out) {
                game.native_chat(&format!("JEI: {error}"));
            }
        }
    }
}

fn handle(event: &Event, ctx: &Context<'_>, lang: jei::Lang) -> Vec<Out> {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let Some(viewer) = state.viewer_mut() else {
            return Vec::new();
        };
        viewer.set_lang(lang);
        viewer.handle(event.clone(), ctx)
    })
}

/// What `/jei` adds about the recipe types of mods; none without them.
pub(super) fn custom_line() -> Option<String> {
    STATE.with(|s| s.borrow().viewer().and_then(Viewer::about_custom))
}

/// The viewer's size, for `/jei`; none outside a world.
pub(super) fn counts() -> Option<(usize, usize)> {
    STATE.with(|s| {
        s.borrow()
            .viewer()
            .map(|viewer| (viewer.item_count(), viewer.recipe_count()))
    })
}

fn apply(game: &mut Game<'_>, out: Out) -> Result<(), String> {
    match out {
        Out::Items(stacks) => {
            game.native_panel_items(jei::ID, panel_stacks(stacks));
            Ok(())
        }
        Out::Recipes {
            title,
            tabs,
            tab,
            views,
        } => {
            game.native_panel_recipes(
                jei::ID,
                &title,
                to_native(tabs),
                tab,
                views.into_iter().map(native_view).collect(),
            );
            Ok(())
        }
        Out::Give { item, count } => game.native_give(&item, count),
        Out::Place { book_id, all } => game.native_place_recipe(book_id, all),
        Out::Say(text) => {
            game.native_chat(&text);
            Ok(())
        }
    }
}

/// The game's stacks. An empty list keeps one invisible slot: the panel (and
/// its search box and events) goes away when the list is empty.
fn panel_stacks(stacks: Vec<Stack>) -> Vec<NativeStack> {
    if stacks.is_empty() {
        return vec![NativeStack {
            item: "minecraft:air".to_owned(),
            count: 0,
            components: String::new(),
        }];
    }
    to_native(stacks)
}

fn to_native(stacks: Vec<Stack>) -> Vec<NativeStack> {
    stacks
        .into_iter()
        .map(|s| NativeStack {
            item: s.item,
            count: s.count,
            components: s.components,
        })
        .collect()
}

/// A bookmark as the game's stack, with the components `/give` reads.
fn item_stack(stack: &Stack) -> Option<ItemStack> {
    let (damage, extra) = if stack.components.is_empty() {
        (0, None)
    } else {
        let tag = horizon_client::item_data::command_components(&stack.components).ok()?;
        horizon_client::item_data::decode_components(Some(&tag))
    };
    Some(ItemStack {
        item: stack.item.as_str().into(),
        count: stack.count.clamp(1, 99),
        damage,
        extra,
    })
}

fn native_view(view: View) -> NativeRecipeView {
    NativeRecipeView {
        layout: match view.layout {
            Layout::Grid => NativeRecipeLayout::Grid,
            Layout::Cooking => NativeRecipeLayout::Cooking,
            Layout::Single => NativeRecipeLayout::Single,
            Layout::Smithing => NativeRecipeLayout::Smithing,
            Layout::Brewing => NativeRecipeLayout::Brewing,
        },
        width: view.width,
        height: view.height,
        slots: view.slots.into_iter().map(to_native).collect(),
        results: to_native(view.results),
        note: view.note,
        action: view.action,
    }
}

fn convert_event(event: &NativePanelEvent) -> Option<Event> {
    Some(match event {
        NativePanelEvent::Search(text) => Event::Search(text.clone()),
        NativePanelEvent::Clicked { index, right } => Event::Clicked {
            index: *index as usize,
            right: *right,
        },
        NativePanelEvent::Key { key, stack } => Event::Key {
            key: key.clone(),
            stack: stack_of(stack),
        },
        NativePanelEvent::Tab(tab) => Event::Tab(*tab as usize),
        NativePanelEvent::Back => Event::Back,
        NativePanelEvent::SlotClicked { stack, right, .. } => Event::SlotClicked {
            stack: stack_of(stack),
            right: *right,
        },
        NativePanelEvent::Action(index) => Event::Action(*index as usize),
        _ => return None,
    })
}

fn stack_of(stack: &NativeStack) -> Stack {
    Stack {
        item: stack.item.clone(),
        count: stack.count,
        components: stack.components.clone(),
    }
}

fn items(catalog: &NativeRecipes) -> Vec<Item> {
    catalog
        .items
        .iter()
        .map(|item| Item {
            id: item.id.clone(),
            name: item.name.clone(),
            tags: item.tags.clone(),
        })
        .collect()
}

fn recipes(catalog: &NativeRecipes) -> Vec<Recipe> {
    catalog
        .recipes
        .iter()
        // A recipe with no ingredients says nothing in a list of recipes.
        .filter(|recipe| !recipe.slots.is_empty())
        .map(|recipe| Recipe {
            kind: kind(recipe.kind),
            width: recipe.width,
            height: recipe.height,
            slots: ingredient_slots(&recipe.slots),
            results: ingredient_list(&recipe.results),
            cooking_time: recipe.cooking_time,
            experience: recipe.experience,
            book_id: recipe.book_id,
        })
        .collect()
}

/// The recipe types of mods the viewer shows beside the game's own, read from
/// their files (`native_recipes_of_type`): Charm's woodcutter and kiln, one
/// input and one result each, and Chipped's workbenches, whose file lists
/// tags, each a group of blocks the bench turns into one another.
fn custom(game: &Game<'_>, items: &[Item]) -> Custom {
    let lookup = Lookup::new(items);
    let mut custom = Custom::default();
    let single = [
        ("charm:woodcutting", Kind::Woodcutting),
        ("charm:firing", Kind::Firing),
    ];
    for (name, kind) in single {
        for recipe in game.native_recipes_of_type(name) {
            let json: &serde_json::Value = &recipe.json;
            let Some((result, count)) = result_of(&json["result"]) else {
                continue;
            };
            let time = json["cookingtime"].as_u64().unwrap_or(0) as u32;
            let experience = json["experience"].as_f64().unwrap_or(0.0) as f32;
            let slots = [sources(&json["ingredient"])];
            custom.recipe(&lookup, kind, &slots, &result, count, time, experience);
        }
    }
    for recipe in game.native_recipes_of_type("chipped:workbench") {
        let json: &serde_json::Value = &recipe.json;
        let groups: Vec<Vec<Source>> = (json["ingredients"].as_array())
            .map(|list| list.iter().map(group_sources).collect())
            .unwrap_or_default();
        // The file is named after its bench: `chipped:mason_table`.
        custom.bench(&lookup, &recipe.id, &groups);
    }
    custom
}

/// A group's sources: its tag, and the vanilla block the tag is named after
/// (`minecraft:amethyst_block` for `chipped:amethyst_block`), the one a
/// player starts from. Chipped's tag files name that block bare
/// (`amethyst_block`), and the game's tag loader keeps a bare id as it is
/// written instead of reading it as `minecraft:`'s, so the tag alone leaves
/// it out. A name that is no item adds nothing.
fn group_sources(json: &serde_json::Value) -> Vec<Source> {
    let mut found = sources(json);
    let named: Vec<Source> = (found.iter())
        .filter_map(|source| match source {
            Source::Tag(tag) => tag.rsplit(':').next(),
            Source::Item(_) => None,
        })
        .map(|path| Source::Item(path.to_owned()))
        .collect();
    found.extend(named);
    found
}

/// What an ingredient's JSON names: an id or a `#tag`, an `{"item": ..}` or
/// a `{"tag": ..}`, or a list of them.
fn sources(json: &serde_json::Value) -> Vec<Source> {
    use serde_json::Value;
    let text = |value: &Value| value.as_str().map(str::to_owned);
    match json {
        Value::String(name) => match name.strip_prefix('#') {
            Some(tag) => vec![Source::Tag(tag.to_owned())],
            None => vec![Source::Item(name.clone())],
        },
        Value::Object(fields) => {
            let item = fields.get("item").and_then(text).map(Source::Item);
            let tag = fields.get("tag").and_then(text).map(Source::Tag);
            item.or(tag).into_iter().collect()
        }
        Value::Array(list) => list.iter().flat_map(sources).collect(),
        _ => Vec::new(),
    }
}

/// A result's id and count: `{"id": .., "count": ..}` or a bare id.
fn result_of(json: &serde_json::Value) -> Option<(String, u32)> {
    if let Some(id) = json.as_str() {
        return Some((id.to_owned(), 1));
    }
    let id = json["id"].as_str()?;
    let count = json["count"].as_u64().unwrap_or(1).clamp(1, 99) as u32;
    Some((id.to_owned(), count))
}

fn ingredient_slots(slots: &[Vec<NativeRecipeStack>]) -> Vec<Vec<Ingredient>> {
    slots.iter().map(|slot| ingredient_list(slot)).collect()
}

fn ingredient_list(stacks: &[NativeRecipeStack]) -> Vec<Ingredient> {
    stacks
        .iter()
        .map(|stack| Ingredient {
            item: stack.item,
            count: stack.count,
            components: stack.components.clone(),
        })
        .collect()
}

fn kind(kind: NativeRecipeKind) -> Kind {
    match kind {
        NativeRecipeKind::CraftingShaped => Kind::CraftingShaped,
        NativeRecipeKind::CraftingShapeless => Kind::CraftingShapeless,
        NativeRecipeKind::Smelting => Kind::Smelting,
        NativeRecipeKind::Blasting => Kind::Blasting,
        NativeRecipeKind::Smoking => Kind::Smoking,
        NativeRecipeKind::Campfire => Kind::Campfire,
        NativeRecipeKind::Stonecutting => Kind::Stonecutting,
        NativeRecipeKind::Smithing => Kind::Smithing,
        NativeRecipeKind::Brewing => Kind::Brewing,
    }
}
