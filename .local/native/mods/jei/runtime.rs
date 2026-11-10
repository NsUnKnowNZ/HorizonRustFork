// JEI for Horizon: an item and recipe viewer built on Horizon's native hook
// points. This file is both the mod's Cargo library (its tests run with
// `cargo test`) and the module the bundle adds to the game as crate::game::jei.
//
// Phase 0: the bundle's skeleton. It registers its row in Options > Mods, a
// settings page with one toggle, and the /jei command, to prove the wiring
// before the viewer itself (tasks 1.x and 2.x in tasks/todo.md).

/// The mod's id: its row, its settings folder and its keys.
pub const ID: &str = "jei";
pub const NAME: &str = "JEI";
pub const VERSION: &str = "0.2.1";
pub const DESCRIPTION: &str = "An item and recipe viewer: every item beside container screens, with search, recipes, uses and bookmarks.";

/// The hook points' version this build needs: the newest point it hooks
/// (`native_item_stack`, API 313). It builds on any Horizon at least that new.
pub const HOOKS: u32 = 313;

/// Its keys: name, label and default (the Java mod's R and U, and A for a
/// bookmark).
pub const BOOKMARK_KEY: &str = "key.keyboard.a";
pub const KEYS: [[&str; 3]; 3] = [
    ["recipe", "Show recipes", "key.keyboard.r"],
    ["uses", "Show uses", "key.keyboard.u"],
    ["bookmark", "Bookmark item", BOOKMARK_KEY],
];

/// `JEI 0.1.0`, as Options > Mods lists it.
pub fn title() -> String {
    format!("{NAME} {VERSION}")
}

/// The line `/jei` says in chat.
pub fn about() -> String {
    format!("{NAME} {VERSION} (alpha) on hook points version {HOOKS}")
}

/// The line `/jei` says in a world: what the viewer holds.
pub fn about_counts(items: usize, recipes: usize) -> String {
    format!("{}: {items} items, {recipes} recipes", about())
}

/// The UI's language: the game's language is not exposed to mods, so the
/// player picks one here (default English).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

pub fn lang(saved: Option<&str>) -> Lang {
    if saved == Some("es") {
        Lang::Es
    } else {
        Lang::En
    }
}

impl Lang {
    pub fn next(self) -> Lang {
        match self {
            Lang::En => Lang::Es,
            Lang::Es => Lang::En,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    pub fn language_button(self) -> String {
        format!(
            "Language: {}",
            match self {
                Lang::En => "English",
                Lang::Es => "Español",
            }
        )
    }

    pub fn give_button(self, on: bool) -> String {
        let state = if on { "ON" } else { "OFF" };
        match self {
            Lang::En => format!("Click gives items: {state}"),
            Lang::Es => format!("Clic da objetos: {state}"),
        }
    }

    fn recipes(self) -> &'static str {
        match self {
            Lang::En => "Recipes",
            Lang::Es => "Recetas",
        }
    }

    fn uses(self) -> &'static str {
        match self {
            Lang::En => "Uses",
            Lang::Es => "Usos",
        }
    }

    fn shapeless(self) -> &'static str {
        match self {
            Lang::En => "Shapeless",
            Lang::Es => "Sin forma",
        }
    }

    /// A workbench's note: any one of the slot's `n` blocks fits.
    fn any_of(self, n: usize) -> String {
        match self {
            Lang::En => format!("Any of these {n}"),
            Lang::Es => format!("Cualquiera de estos {n}"),
        }
    }

    fn no_recipes(self) -> &'static str {
        match self {
            Lang::En => "No recipes for",
            Lang::Es => "Sin recetas para",
        }
    }

    fn no_uses(self) -> &'static str {
        match self {
            Lang::En => "No uses for",
            Lang::Es => "Sin usos para",
        }
    }

    /// A tooltip's hint: what R opens and how much of it there is.
    pub fn recipe_hint(self, n: usize) -> String {
        match self {
            Lang::En => format!("R: Recipes ({n})"),
            Lang::Es => format!("R: Recetas ({n})"),
        }
    }

    pub fn uses_hint(self, n: usize) -> String {
        match self {
            Lang::En => format!("U: Uses ({n})"),
            Lang::Es => format!("U: Usos ({n})"),
        }
    }

    pub fn bookmarked(self) -> &'static str {
        match self {
            Lang::En => "★ Bookmarked",
            Lang::Es => "★ Marcado",
        }
    }

    /// A bookmark slot's tooltip: its actions, as JEI's bar has them.
    pub fn bookmark_hint(self) -> &'static str {
        match self {
            Lang::En => "Left: recipes   Right: uses   Shift: remove",
            Lang::Es => "Izquierda: recetas   Derecha: usos   Mayús: quitar",
        }
    }

    pub fn bookmark_added(self) -> &'static str {
        match self {
            Lang::En => "Bookmarked",
            Lang::Es => "Marcado",
        }
    }

    pub fn bookmark_removed(self) -> &'static str {
        match self {
            Lang::En => "Bookmark removed",
            Lang::Es => "Marca quitada",
        }
    }

    pub fn bookmarks_full(self) -> &'static str {
        match self {
            Lang::En => "The bookmark bar is full",
            Lang::Es => "La barra de marcas está llena",
        }
    }

    pub fn bookmarks_cleared(self) -> &'static str {
        match self {
            Lang::En => "Bookmarks cleared",
            Lang::Es => "Marcas borradas",
        }
    }

    pub fn sort_button(self, sort: Sort) -> String {
        match self {
            Lang::En => format!("Sort: {}", sort.label(self)),
            Lang::Es => format!("Orden: {}", sort.label(self)),
        }
    }

    pub fn hints_button(self, on: bool) -> String {
        let state = if on { "ON" } else { "OFF" };
        match self {
            Lang::En => format!("Tooltip hints: {state}"),
            Lang::Es => format!("Consejos en tooltips: {state}"),
        }
    }

    pub fn bookmarks_button(self, on: bool) -> String {
        let state = if on { "ON" } else { "OFF" };
        match self {
            Lang::En => format!("Bookmark bar: {state}"),
            Lang::Es => format!("Barra de marcas: {state}"),
        }
    }
}

/// Whether "Click gives items" is on: `true` only when the player turned it on.
pub fn give(saved: Option<&str>) -> bool {
    saved == Some("true")
}

/// The settings page's button for "Click gives items", in English.
pub fn give_button(on: bool) -> String {
    Lang::En.give_button(on)
}

/// `/jei` and its options, without the slash. `None` for an unknown option.
pub fn command(args: &str) -> Option<Command> {
    let text = args.trim();
    if let Some(sort) = text.strip_prefix("sort") {
        let sort = sort.trim();
        if sort.is_empty() {
            return Some(Command::SortNone);
        }
        return Sort::from_code(sort).map(Command::Sort);
    }
    match text {
        "" | "about" => Some(Command::About),
        "give" => Some(Command::ToggleGive),
        "bookmarks clear" | "clear" => Some(Command::BookmarksClear),
        "config" => Some(Command::Config),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    About,
    ToggleGive,
    /// `sort name` or `sort id`.
    Sort(Sort),
    /// `sort`: say the order of the list now.
    SortNone,
    BookmarksClear,
    /// `config`: where the config and the bookmarks are kept.
    Config,
}

/// The help `/jei unknown` prints.
pub const COMMAND_USAGE: &str =
    "Use /jei, /jei give, /jei sort [name|id], /jei bookmarks clear or /jei config";

/// One word of a search, as JEI's `SearchTokenizer` builds it (MIT, © mezz:
/// `Gui/src/main/java/mezz/jei/gui/search/SearchTokenizer.java`). `start` and
/// `end` are character offsets, for highlighting; `operator` is a `|` between
/// alternatives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub exclusion: bool,
    pub operator: bool,
    pub start: usize,
    pub end: usize,
}

impl Token {
    fn operator(at: usize) -> Self {
        Self {
            text: "|".to_owned(),
            exclusion: false,
            operator: true,
            start: at,
            end: at + 1,
        }
    }
}

/// Splits a search into tokens: words, quoted phrases, `-exclusions` and `|`
/// alternatives. Escapes (`\"`) keep a quote in a word or phrase. Unicode
/// offsets count characters, not UTF-16 units.
pub fn tokenize(filter: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    if filter.is_empty() {
        return tokens;
    }
    let mut current = String::new();
    let mut inside_quotes = false;
    let mut exclusion = false;
    let mut escaped = false;
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for (at, c) in filter.char_indices() {
        if escaped {
            current.push(c);
            end = Some(at + c.len_utf8());
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            start.get_or_insert(at);
            end = Some(at + 1);
            continue;
        }
        if c == '"' {
            if inside_quotes {
                add_token(&mut tokens, &current, exclusion, start, Some(at + 1));
                current.clear();
                inside_quotes = false;
                exclusion = false;
                start = None;
                end = None;
            } else if !current.is_empty() && !is_prefix(&current) {
                add_token(&mut tokens, &current, exclusion, start, end);
                current.clear();
                exclusion = false;
                start = None;
                end = None;
            } else {
                inside_quotes = true;
                start.get_or_insert(at);
                end = Some(at + 1);
            }
            continue;
        }
        if !inside_quotes && c.is_whitespace() {
            if !current.is_empty() {
                add_token(&mut tokens, &current, exclusion, start, end);
                current.clear();
            }
            exclusion = false;
            start = None;
            end = None;
            continue;
        }
        if !inside_quotes && c == '|' {
            if !current.is_empty() {
                add_token(&mut tokens, &current, exclusion, start, end);
                current.clear();
            }
            tokens.push(Token::operator(at));
            exclusion = false;
            start = None;
            end = None;
            continue;
        }
        if !inside_quotes && current.is_empty() && c == '-' {
            exclusion = true;
            start = Some(at);
            end = Some(at + 1);
            continue;
        }
        start.get_or_insert(at);
        end = Some(at + c.len_utf8());
        current.push(c);
    }

    if !current.is_empty() || inside_quotes {
        add_token(&mut tokens, &current, exclusion, start, end);
    }
    tokens
}

fn add_token(
    tokens: &mut Vec<Token>,
    content: &str,
    exclusion: bool,
    start: Option<usize>,
    end: Option<usize>,
) {
    let text = content.trim();
    if !text.is_empty() {
        tokens.push(Token {
            text: text.to_owned(),
            exclusion,
            operator: false,
            start: start.unwrap_or_default(),
            end: end.unwrap_or_default(),
        });
    }
}

/// A single character that is not a word, `-` or `|`: the `@`, `#`, `$`, `%`
/// prefixes, which mark a quoted phrase as prefixed (`@"two words"`).
fn is_prefix(text: &str) -> bool {
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return false;
    };
    !c.is_alphanumeric() && c != '-' && c != '|'
}

/// The tokens grouped by `|`: each group is one alternative of the search; a
/// group is a conjunction of its tokens.
pub fn split_by_operator(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for token in tokens {
        if token.operator {
            groups.push(std::mem::take(&mut current));
        } else {
            current.push(token.clone());
        }
    }
    groups.push(current);
    groups
}

/// One item as the search sees it.
pub struct SearchItem<'a> {
    pub name: &'a str,
    pub id: &'a str,
    /// Tags without the `#` (`minecraft:logs`).
    pub tags: &'a [String],
}

/// A filter, read once: its tokens grouped by `|`, lowercased, ready to test
/// every item of a list against.
pub struct Search {
    groups: Vec<Vec<Query>>,
}

/// One token of a filter, lowercased once instead of once per item.
struct Query {
    text: String,
    exclusion: bool,
}

impl Search {
    pub fn new(filter: &str) -> Self {
        let groups = (split_by_operator(&tokenize(filter)).into_iter())
            .map(|group| {
                (group.into_iter())
                    .map(|token| Query {
                        text: token.text.to_lowercase(),
                        exclusion: token.exclusion,
                    })
                    .collect()
            })
            .collect();
        Self { groups }
    }

    /// Whether `item` passes: any `|` alternative matches, and every token of
    /// it matches. Prefixes: `@` namespace, `#` tag; `$` (tooltip) and `%`
    /// (creative tab) have no data yet and match nothing.
    pub fn keeps(&self, item: &SearchItem<'_>) -> bool {
        (self.groups.iter()).any(|group| group.iter().all(|token| token_hits(item, token)))
    }
}

fn token_hits(item: &SearchItem<'_>, token: &Query) -> bool {
    let text = token.text.as_str();
    let hit = if let Some(namespace) = text.strip_prefix('@') {
        item.id
            .split(':')
            .next()
            .is_some_and(|ns| ns.starts_with(namespace))
    } else if let Some(tag) = text.strip_prefix('#') {
        item.tags.iter().any(|t| t.to_lowercase().contains(tag))
    } else if text.starts_with('$') || text.starts_with('%') {
        false
    } else {
        item.name.to_lowercase().contains(text) || item.id.to_lowercase().contains(text)
    };
    hit != token.exclusion
}

/// Items the panel takes, and the most recipes a page shows.
pub const ITEMS_MAX: usize = 8192;
pub const RECIPES_MAX: usize = 256;
/// Stacks a slot cycles through.
pub const STACKS_MAX: usize = 64;

/// One item of the game, as the viewer lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
}

impl Item {
    fn search(&self) -> SearchItem<'_> {
        SearchItem {
            name: &self.name,
            id: &self.id,
            tags: &self.tags,
        }
    }
}

/// A stack the panel shows: an item id, a count and components in vanilla's
/// item syntax, or "".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    pub components: String,
}

impl Stack {
    pub fn item(id: &str) -> Self {
        Self {
            item: id.to_owned(),
            count: 1,
            components: String::new(),
        }
    }
}

/// One alternative of a recipe slot: an index into the viewer's items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ingredient {
    pub item: u32,
    pub count: u32,
    pub components: String,
}

/// The recipe kinds: vanilla's, which the game gives a viewer, and the types
/// of mods the viewer reads from their recipe files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CraftingShaped,
    CraftingShapeless,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
    /// Charm's woodcutter (`charm:woodcutting`): one input, as stonecutting.
    Woodcutting,
    /// Charm's kiln (`charm:firing`): one input cooked, as smelting.
    Firing,
    /// A workbench of Chipped's (`chipped:workbench`), by its place in the
    /// viewer's benches: a block of a group for another of the same group.
    Workbench(u16),
}

/// The station a kind is made at, in the order of the tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Station {
    Crafting,
    Furnace,
    BlastFurnace,
    Smoker,
    Campfire,
    Stonecutter,
    Smithing,
    Brewing,
    Woodcutter,
    Kiln,
    /// A workbench, by its place in the viewer's benches.
    Workbench(u16),
}

impl Station {
    /// The stations that are always the same; the workbenches follow them.
    pub const ALL: [Station; 10] = [
        Station::Crafting,
        Station::Furnace,
        Station::BlastFurnace,
        Station::Smoker,
        Station::Campfire,
        Station::Stonecutter,
        Station::Smithing,
        Station::Brewing,
        Station::Woodcutter,
        Station::Kiln,
    ];

    /// The item whose icon stands for the station in the tabs. A workbench's
    /// is the viewer's to say (`Viewer::station_icon`).
    pub fn icon(self) -> &'static str {
        match self {
            Station::Crafting => "minecraft:crafting_table",
            Station::Furnace => "minecraft:furnace",
            Station::BlastFurnace => "minecraft:blast_furnace",
            Station::Smoker => "minecraft:smoker",
            Station::Campfire => "minecraft:campfire",
            Station::Stonecutter => "minecraft:stonecutter",
            Station::Smithing => "minecraft:smithing_table",
            Station::Brewing => "minecraft:brewing_stand",
            Station::Woodcutter => "charm:woodcutter",
            Station::Kiln => "charm:kiln",
            Station::Workbench(_) => "minecraft:crafting_table",
        }
    }

    /// The vanilla station nearest a mod's, for when its own item is not in
    /// the game.
    fn stand_in(self) -> &'static str {
        match self {
            Station::Woodcutter => "minecraft:stonecutter",
            Station::Kiln => "minecraft:furnace",
            Station::Workbench(_) => "minecraft:stonecutter",
            _ => "minecraft:crafting_table",
        }
    }

    fn order(self) -> usize {
        if let Station::Workbench(bench) = self {
            return Station::ALL.len() + bench as usize;
        }
        Station::ALL
            .iter()
            .position(|s| *s == self)
            .unwrap_or(Station::ALL.len())
    }
}

impl Kind {
    pub fn station(self) -> Station {
        match self {
            Kind::CraftingShaped | Kind::CraftingShapeless => Station::Crafting,
            Kind::Smelting => Station::Furnace,
            Kind::Blasting => Station::BlastFurnace,
            Kind::Smoking => Station::Smoker,
            Kind::Campfire => Station::Campfire,
            Kind::Stonecutting => Station::Stonecutter,
            Kind::Smithing => Station::Smithing,
            Kind::Brewing => Station::Brewing,
            Kind::Woodcutting => Station::Woodcutter,
            Kind::Firing => Station::Kiln,
            Kind::Workbench(bench) => Station::Workbench(bench),
        }
    }
}

/// A recipe as the viewer knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub kind: Kind,
    /// The grid's size for a shaped recipe; 0 for the other kinds.
    pub width: u32,
    pub height: u32,
    /// Each slot's alternatives; the input(s) for the non-grid kinds.
    pub slots: Vec<Vec<Ingredient>>,
    pub results: Vec<Ingredient>,
    /// Game ticks and experience, for cooking; 0 otherwise.
    pub cooking_time: u32,
    pub experience: f32,
    /// The recipe book's id, or -1.
    pub book_id: i32,
}

/// Blocks a workbench turns into one another, one for one (Chipped: the
/// items of one tag). A recipe for every pair would be hundreds of thousands,
/// so the viewer keeps the group and makes a page's recipes from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The workbench: its place in the viewer's benches.
    pub bench: u16,
    /// Its items, as indexes into the viewer's items, in their order.
    pub members: Vec<u32>,
}

/// What a recipe file names as an ingredient: an item, or a tag without the
/// `#`. A bare name is `minecraft:`'s, as vanilla reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Item(String),
    Tag(String),
}

/// `minecraft:stone` for `stone`.
fn qualified(name: &str) -> std::borrow::Cow<'_, str> {
    if name.contains(':') {
        std::borrow::Cow::Borrowed(name)
    } else {
        std::borrow::Cow::Owned(format!("minecraft:{name}"))
    }
}

/// The items by id and by tag, to read the recipe files of mods' own types.
pub struct Lookup<'a> {
    ids: std::collections::HashMap<&'a str, u32>,
    tags: std::collections::HashMap<&'a str, Vec<u32>>,
}

impl<'a> Lookup<'a> {
    pub fn new(items: &'a [Item]) -> Self {
        let mut ids = std::collections::HashMap::with_capacity(items.len());
        let mut tags: std::collections::HashMap<&'a str, Vec<u32>> =
            std::collections::HashMap::new();
        for (index, item) in items.iter().enumerate() {
            ids.insert(item.id.as_str(), index as u32);
            for tag in &item.tags {
                tags.entry(tag.as_str()).or_default().push(index as u32);
            }
        }
        Self { ids, tags }
    }

    /// An item's index, by its id.
    pub fn item(&self, id: &str) -> Option<u32> {
        self.ids.get(qualified(id).as_ref()).copied()
    }

    /// The items the sources name, each once, in the items' order; the ones
    /// the game doesn't know are left out.
    pub fn items(&self, sources: &[Source]) -> Vec<u32> {
        let mut found: Vec<u32> = Vec::new();
        for source in sources {
            match source {
                Source::Item(id) => found.extend(self.item(id)),
                Source::Tag(tag) => {
                    let members = self.tags.get(qualified(tag).as_ref());
                    found.extend(members.into_iter().flatten());
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// What the recipe types of mods add to the game's tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Custom {
    pub recipes: Vec<Recipe>,
    /// The workbenches: the id of the item that stands for each.
    pub benches: Vec<String>,
    pub groups: Vec<Group>,
}

impl Custom {
    /// A recipe of `kind`: its slots' sources and what it makes. Nothing is
    /// added, and false comes back, when a slot or the result names no item
    /// the game knows.
    #[allow(clippy::too_many_arguments)]
    pub fn recipe(
        &mut self,
        lookup: &Lookup<'_>,
        kind: Kind,
        slots: &[Vec<Source>],
        result: &str,
        count: u32,
        cooking_time: u32,
        experience: f32,
    ) -> bool {
        let Some(result) = lookup.item(result) else {
            return false;
        };
        let mut filled: Vec<Vec<Ingredient>> = Vec::with_capacity(slots.len());
        for slot in slots {
            let items = lookup.items(slot);
            if items.is_empty() {
                return false;
            }
            filled.push(items.into_iter().map(plain).collect());
        }
        if filled.is_empty() {
            return false;
        }
        self.recipes.push(Recipe {
            kind,
            width: 0,
            height: 0,
            slots: filled,
            results: vec![Ingredient {
                count: count.clamp(1, 99),
                ..plain(result)
            }],
            cooking_time,
            experience,
            book_id: -1,
        });
        true
    }

    /// A workbench and its groups: `icon` is the item that stands for it, and
    /// each group's sources name the blocks it turns into one another. A
    /// group of fewer than two known items says nothing and is left out, as
    /// is a bench without groups. Returns the groups it added.
    pub fn bench(&mut self, lookup: &Lookup<'_>, icon: &str, groups: &[Vec<Source>]) -> usize {
        let Ok(bench) = u16::try_from(self.benches.len()) else {
            return 0;
        };
        let before = self.groups.len();
        for sources in groups {
            let members = lookup.items(sources);
            if members.len() >= 2 {
                self.groups.push(Group { bench, members });
            }
        }
        let added = self.groups.len() - before;
        if added > 0 {
            self.benches.push(qualified(icon).into_owned());
        }
        added
    }
}

/// One of an item, without components.
fn plain(item: u32) -> Ingredient {
    Ingredient {
        item,
        count: 1,
        components: String::new(),
    }
}

/// A workbench's recipe: any one of `inputs` for `result`.
fn workbench_recipe(bench: u16, inputs: &[u32], result: u32) -> Recipe {
    Recipe {
        kind: Kind::Workbench(bench),
        width: 0,
        height: 0,
        slots: vec![inputs.iter().copied().map(plain).collect()],
        results: vec![plain(result)],
        cooking_time: 0,
        experience: 0.0,
        book_id: -1,
    }
}

/// How the panel lays a recipe out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Grid,
    Cooking,
    Single,
    Smithing,
    Brewing,
}

/// A recipe as the panel draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub layout: Layout,
    pub width: u32,
    pub height: u32,
    pub slots: Vec<Vec<Stack>>,
    pub results: Vec<Stack>,
    pub note: String,
    /// The action button: none, or whether it is enabled.
    pub action: Option<bool>,
}

/// What the player did, in the viewer's words.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Search(String),
    Clicked {
        index: usize,
        right: bool,
    },
    Key {
        key: String,
        stack: Stack,
    },
    /// A bookmark or a panel row asked for a stack's recipes or uses.
    Open {
        stack: Stack,
        uses: bool,
    },
    Tab(usize),
    Back,
    SlotClicked {
        stack: Stack,
        right: bool,
    },
    Action(usize),
}

/// What the game is to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// Show these items in the panel; empty clears it.
    Items(Vec<Stack>),
    /// Show this recipe page; no views goes back to the items.
    Recipes {
        title: String,
        tabs: Vec<Stack>,
        tab: u32,
        views: Vec<View>,
    },
    /// Creative: give the player this item.
    Give { item: String, count: u32 },
    /// Place this recipe-book id in the open grid.
    Place { book_id: i32, all: bool },
    /// A line only this player sees.
    Say(String),
}

/// The player's settings and the book's state, for one event.
pub struct Context<'a> {
    pub recipe_key: &'a str,
    pub uses_key: &'a str,
    /// "Click gives items" is on.
    pub give: bool,
    /// The game would give items (creative).
    pub creative: bool,
    /// The recipe book's ids now.
    pub known: &'a [i32],
}

/// How the item list is ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// By name, then id, then variant.
    #[default]
    Name,
    /// By id, then variant.
    Id,
}

impl Sort {
    pub fn code(self) -> &'static str {
        match self {
            Sort::Name => "name",
            Sort::Id => "id",
        }
    }

    pub fn from_code(text: &str) -> Option<Sort> {
        match text.trim().to_lowercase().as_str() {
            "name" | "nombre" => Some(Sort::Name),
            "id" => Some(Sort::Id),
            _ => None,
        }
    }

    pub fn next(self) -> Sort {
        match self {
            Sort::Name => Sort::Id,
            Sort::Id => Sort::Name,
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Sort::Name, Lang::En) => "Name",
            (Sort::Name, Lang::Es) => "Nombre",
            (Sort::Id, _) => "ID",
        }
    }
}

/// What the mod keeps between sessions, in `config/jei/jei.cfg`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub sort: Sort,
    /// The "R: Recipes" lines in item tooltips.
    pub hints: bool,
    /// The bookmark bar on container screens.
    pub bookmarks: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sort: Sort::Name,
            hints: true,
            bookmarks: true,
        }
    }
}

impl Config {
    /// Reads the file: `key=value` lines, unknown keys and values ignored.
    pub fn parse(text: &str) -> Config {
        let mut config = Config::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "sort" => {
                    if let Some(sort) = Sort::from_code(value) {
                        config.sort = sort;
                    }
                }
                "hints" => config.hints = truthy(value, config.hints),
                "bookmarks" => config.bookmarks = truthy(value, config.bookmarks),
                _ => {}
            }
        }
        config
    }

    pub fn to_text(self) -> String {
        format!(
            "sort={}\nhints={}\nbookmarks={}\n",
            self.sort.code(),
            self.hints,
            self.bookmarks
        )
    }
}

fn truthy(value: &str, default: bool) -> bool {
    match value.to_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => true,
        "false" | "off" | "no" | "0" => false,
        _ => default,
    }
}

/// What bookmarking a stack did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Added(usize),
    Removed(usize),
    /// Twenty-four are the bar's, as JEI's.
    Full,
}

/// One slot of the bar, in GUI pixels.
pub const BOOKMARK_SLOT: f32 = 18.0;
/// The bar's columns and rows, and its gap from the screen's corner.
pub const BOOKMARK_COLUMNS: usize = 2;
pub const BOOKMARK_ROWS: usize = 12;
pub const BOOKMARK_MARGIN: f32 = 4.0;
/// The bookmarks the bar holds.
pub const BOOKMARKS_MAX: usize = BOOKMARK_COLUMNS * BOOKMARK_ROWS;

/// The bookmarks, in the order the bar shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bookmarks {
    slots: Vec<Stack>,
}

impl Bookmarks {
    pub fn new(slots: Vec<Stack>) -> Self {
        let slots = (slots.into_iter())
            .filter(|s| !s.item.is_empty() && s.item != "minecraft:air")
            .take(BOOKMARKS_MAX)
            .map(|s| Stack {
                count: s.count.clamp(1, 99),
                ..s
            })
            .collect();
        Self { slots }
    }

    pub fn slots(&self) -> &[Stack] {
        &self.slots
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Stack> {
        self.slots.get(index)
    }

    /// Where a stack is bookmarked: the same item and the same components.
    pub fn position(&self, stack: &Stack) -> Option<usize> {
        (self.slots.iter()).position(|s| s.item == stack.item && s.components == stack.components)
    }

    pub fn toggle(&mut self, stack: &Stack) -> Toggle {
        if let Some(index) = self.position(stack) {
            self.slots.remove(index);
            return Toggle::Removed(index);
        }
        if self.slots.len() >= BOOKMARKS_MAX {
            return Toggle::Full;
        }
        self.slots.push(Stack {
            item: stack.item.clone(),
            count: 1,
            components: stack.components.clone(),
        });
        Toggle::Added(self.slots.len() - 1)
    }

    pub fn remove(&mut self, index: usize) -> Option<Stack> {
        (index < self.slots.len()).then(|| self.slots.remove(index))
    }

    pub fn move_slot(&mut self, from: usize, to: usize) -> bool {
        if from == to || from >= self.slots.len() || to >= self.slots.len() {
            return false;
        }
        let stack = self.slots.remove(from);
        self.slots.insert(to, stack);
        true
    }

    pub fn clear(&mut self) {
        self.slots.clear();
    }
}

/// The bookmarks as a file: `#` comments, then one stack a line,
/// `item<TAB>count<TAB>components`.
pub fn bookmarks_text(bookmarks: &Bookmarks) -> String {
    let mut text = String::from("# JEI bookmarks: item<TAB>count<TAB>components\n");
    for stack in bookmarks.slots() {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            stack.item, stack.count, stack.components
        ));
    }
    text
}

pub fn bookmarks_parse(text: &str) -> Bookmarks {
    let mut stacks = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let item = parts.next().unwrap_or_default().trim();
        if item.is_empty() {
            continue;
        }
        let count = parts
            .next()
            .and_then(|count| count.trim().parse().ok())
            .unwrap_or(1u32);
        stacks.push(Stack {
            item: item.to_owned(),
            count,
            components: parts.next().unwrap_or_default().to_owned(),
        });
    }
    Bookmarks::new(stacks)
}

/// An item as one of its variants: the same item with components (a potion of
/// strength, a tipped arrow).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    /// The item's index in the viewer's items.
    pub item: u32,
    pub components: String,
}

/// The variants the recipe tables mention: every distinct components string a
/// result or an ingredient carries, in the order the tables name them. That is
/// what makes potions and tipped arrows items of their own in the list.
pub fn harvest_variants(recipes: &[Recipe], per_item: usize, total: usize) -> Vec<Variant> {
    let mut out: Vec<Variant> = Vec::new();
    let mut seen: std::collections::HashSet<(u32, String)> = std::collections::HashSet::new();
    let mut counted: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for recipe in recipes {
        let stacks = (recipe.results.iter()).chain(recipe.slots.iter().flatten());
        for stack in stacks {
            if stack.components.is_empty() {
                continue;
            }
            let count = counted.entry(stack.item).or_default();
            if *count >= per_item {
                continue;
            }
            if !seen.insert((stack.item, stack.components.clone())) {
                continue;
            }
            *count += 1;
            out.push(Variant {
                item: stack.item,
                components: stack.components.clone(),
            });
            if out.len() >= total {
                return out;
            }
        }
    }
    out
}

/// How many recipes make an item, and how many use it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub recipes: u32,
    pub uses: u32,
}

/// What a tooltip says of an item: its recipes and uses, and whether it is
/// bookmarked. Empty when there is nothing to say.
pub const HINT_COLOR: [u8; 4] = [170, 170, 170, 255];
pub const MARK_COLOR: [u8; 4] = [255, 255, 85, 255];
pub const NAME_COLOR: [u8; 4] = [255, 255, 255, 255];

pub fn tooltip_lines(counts: Counts, bookmarked: bool, lang: Lang) -> Vec<(String, [u8; 4])> {
    let mut lines = Vec::new();
    if counts.recipes > 0 {
        lines.push((lang.recipe_hint(counts.recipes as usize), HINT_COLOR));
    }
    if counts.uses > 0 {
        lines.push((lang.uses_hint(counts.uses as usize), HINT_COLOR));
    }
    if bookmarked {
        lines.push((lang.bookmarked().to_owned(), MARK_COLOR));
    }
    lines
}

/// A bookmark slot's tooltip: the item's name and what a click does.
pub fn bookmark_tooltip(name: &str, lang: Lang) -> Vec<(String, [u8; 4])> {
    vec![
        (name.to_owned(), NAME_COLOR),
        (lang.bookmark_hint().to_owned(), HINT_COLOR),
    ]
}

/// Every recipe's results and inputs, counted once per recipe for each stack
/// they name: the item with its components, and the item alone.
fn count_recipes(recipes: &[Recipe]) -> std::collections::HashMap<(u32, String), Counts> {
    let mut counts: std::collections::HashMap<(u32, String), Counts> =
        std::collections::HashMap::new();
    for recipe in recipes {
        let mut made: Vec<(u32, &str)> = (recipe.results.iter())
            .map(|s| (s.item, s.components.as_str()))
            .collect();
        made.sort_unstable();
        made.dedup();
        for (item, components) in made {
            bump(&mut counts, item, components, true);
        }
        let mut used: Vec<(u32, &str)> = (recipe.slots.iter().flatten())
            .map(|s| (s.item, s.components.as_str()))
            .collect();
        used.sort_unstable();
        used.dedup();
        for (item, components) in used {
            bump(&mut counts, item, components, false);
        }
    }
    counts
}

fn bump(
    counts: &mut std::collections::HashMap<(u32, String), Counts>,
    item: u32,
    components: &str,
    made: bool,
) {
    raise(counts, (item, components.to_owned()), made);
    if !components.is_empty() {
        raise(counts, (item, String::new()), made);
    }
}

fn raise(
    counts: &mut std::collections::HashMap<(u32, String), Counts>,
    key: (u32, String),
    made: bool,
) {
    let counts = counts.entry(key).or_default();
    if made {
        counts.recipes += 1;
    } else {
        counts.uses += 1;
    }
}

/// The bookmark bar's places: two columns at the screen's top left corner,
/// twelve rows or as many as fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub origin: [f32; 2],
    pub columns: usize,
    pub rows: usize,
}

pub fn bookmark_bar(height: f32) -> Bar {
    let fits = (((height - 2.0 * BOOKMARK_MARGIN) / BOOKMARK_SLOT).floor()).max(0.0) as usize;
    Bar {
        origin: [BOOKMARK_MARGIN, BOOKMARK_MARGIN],
        columns: BOOKMARK_COLUMNS,
        rows: fits.clamp(1, BOOKMARK_ROWS),
    }
}

/// The bar's places for a hit test: always the twelve rows, since the
/// screen's height only decides how many of them it draws.
pub fn bookmark_slots() -> Bar {
    Bar {
        origin: [BOOKMARK_MARGIN, BOOKMARK_MARGIN],
        columns: BOOKMARK_COLUMNS,
        rows: BOOKMARK_ROWS,
    }
}

impl Bar {
    pub fn capacity(&self) -> usize {
        self.columns * self.rows
    }

    /// The whole bar.
    pub fn rect(&self) -> [f32; 4] {
        [
            self.origin[0],
            self.origin[1],
            self.columns as f32 * BOOKMARK_SLOT,
            self.rows as f32 * BOOKMARK_SLOT,
        ]
    }

    /// Slot `index`'s place: column by column, from the top.
    pub fn slot(&self, index: usize) -> [f32; 4] {
        let column = index % self.columns;
        let row = index / self.columns;
        [
            self.origin[0] + column as f32 * BOOKMARK_SLOT,
            self.origin[1] + row as f32 * BOOKMARK_SLOT,
            BOOKMARK_SLOT,
            BOOKMARK_SLOT,
        ]
    }

    /// The slot under a point, among `count` slots.
    pub fn hit(&self, point: [f32; 2], count: usize) -> Option<usize> {
        (0..count.min(self.capacity())).find(|&index| inside(point, self.slot(index)))
    }
}

/// Whether the overlay belongs on the screen with this id: the container
/// screens (a station's or a chest's, all of them `inventory`) and the
/// creative one, where the panel is. Not the pause and options screens over
/// a world, which still have the player's menu open.
pub fn overlay_screen(id: &str) -> bool {
    id == "inventory" || id == "creative"
}

/// Whether a point is inside a rectangle in GUI pixels.
pub fn inside(point: [f32; 2], rect: [f32; 4]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] < rect[0] + rect[2]
        && point[1] < rect[1] + rect[3]
}

/// One page of the history: an item's recipes or uses, on one station's tab.
#[derive(Clone, Debug, PartialEq)]
struct Page {
    item: Stack,
    uses: bool,
    tab: usize,
}

/// One row of the item list: an item, and the components that make it one of
/// its variants ("" for the item itself). `key` is the row's place in the sort.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    item: u32,
    components: String,
    key: String,
}

/// The viewer: the list, the search and the history of recipe pages.
pub struct Viewer {
    items: Vec<Item>,
    /// Each item's index in `items`, by id.
    ids: std::collections::HashMap<String, u32>,
    recipes: Vec<Recipe>,
    /// The stacks the tables name with components, listed after their item.
    variants: std::collections::HashMap<u32, Vec<Variant>>,
    /// Recipes made and uses, by item index and components ("" for any).
    counts: std::collections::HashMap<(u32, String), Counts>,
    /// The workbenches: the id of the item that stands for each.
    benches: Vec<String>,
    /// The workbenches' groups, and each item's (item, group), in order.
    groups: Vec<Group>,
    grouped: Vec<(u32, u32)>,
    lang: Lang,
    sort: Sort,
    /// The panel's rows now, after the search and the sort.
    listed: Vec<Entry>,
    search: String,
    /// The page shown, then the ones Back returns to.
    history: Vec<Page>,
    /// The book id behind each recipe the page shows (-1: none).
    shown: Vec<i32>,
}

impl Viewer {
    /// A viewer with the world's tables and the variants they name (potions,
    /// tipped arrows); the output lists every item.
    pub fn with_variants(
        items: Vec<Item>,
        recipes: Vec<Recipe>,
        variants: Vec<Variant>,
        lang: Lang,
    ) -> (Self, Out) {
        Self::with_groups(items, recipes, variants, Vec::new(), Vec::new(), lang)
    }

    /// The same, with the workbenches of mods and their groups (`Custom`).
    pub fn with_groups(
        items: Vec<Item>,
        recipes: Vec<Recipe>,
        variants: Vec<Variant>,
        benches: Vec<String>,
        groups: Vec<Group>,
        lang: Lang,
    ) -> (Self, Out) {
        let mut grouped: Vec<(u32, u32)> = (groups.iter().enumerate())
            .flat_map(|(index, group)| group.members.iter().map(move |&item| (item, index as u32)))
            .collect();
        grouped.sort_unstable();
        grouped.dedup();
        let ids = (items.iter().enumerate())
            .map(|(index, item)| (item.id.clone(), index as u32))
            .collect();
        let counts = count_recipes(&recipes);
        let mut variants_by_item: std::collections::HashMap<u32, Vec<Variant>> =
            std::collections::HashMap::new();
        for variant in variants {
            (variants_by_item.entry(variant.item).or_default()).push(variant);
        }
        let mut me = Self {
            items,
            ids,
            recipes,
            variants: variants_by_item,
            counts,
            benches,
            groups,
            grouped,
            lang,
            sort: Sort::Name,
            listed: Vec::new(),
            search: String::new(),
            history: Vec::new(),
            shown: Vec::new(),
        };
        let out = me.list();
        (me, out)
    }

    /// The player changed the language: rewrite the strings of the next page.
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn recipe_count(&self) -> usize {
        self.recipes.len()
    }

    /// What `/jei` adds about the recipe types of mods: how many there are,
    /// and how many items have a recipe with them and without. None when the
    /// tables have none of them.
    pub fn about_custom(&self) -> Option<String> {
        let of = |kind: Kind| self.recipes.iter().filter(|r| r.kind == kind).count();
        let (woodcutting, firing) = (of(Kind::Woodcutting), of(Kind::Firing));
        if woodcutting + firing + self.groups.len() == 0 {
            return None;
        }
        // Each item: 1 when a vanilla kind makes it, 2 when a mod's does.
        let mut made = vec![0u8; self.items.len()];
        for recipe in &self.recipes {
            let own = matches!(recipe.kind, Kind::Woodcutting | Kind::Firing);
            for result in &recipe.results {
                if let Some(mark) = made.get_mut(result.item as usize) {
                    *mark |= if own { 2 } else { 1 };
                }
            }
        }
        let mut blocks = 0;
        for &(item, _) in &self.grouped {
            if let Some(mark) = made.get_mut(item as usize) {
                blocks += usize::from(*mark & 4 == 0);
                *mark |= 2 | 4;
            }
        }
        let with = made.iter().filter(|mark| **mark != 0).count();
        let without = made.iter().filter(|mark| **mark & 1 != 0).count();
        let stations = format!(
            "{woodcutting} woodcutting, {firing} kiln, {} workbenches with {} groups of {blocks} blocks",
            self.benches.len(),
            self.groups.len()
        );
        Some(format!(
            "{NAME}: {stations}; {with} of {} items have a recipe, {without} without these",
            self.items.len()
        ))
    }

    /// The groups an item is in.
    fn groups_of(&self, item: u32) -> impl Iterator<Item = &Group> + '_ {
        let start = self.grouped.partition_point(|&(member, _)| member < item);
        (self.grouped[start..].iter())
            .take_while(move |&&(member, _)| member == item)
            .filter_map(|&(_, group)| self.groups.get(group as usize))
    }

    /// What an item's groups add to its counts: a recipe for as many of the
    /// group's other blocks as a slot cycles through, a use for each.
    fn group_counts(&self, item: u32) -> Counts {
        let mut counts = Counts::default();
        for group in self.groups_of(item) {
            let others = group.members.len().saturating_sub(1);
            counts.recipes += others.div_ceil(STACKS_MAX) as u32;
            counts.uses += others as u32;
        }
        counts
    }

    /// The recipes an item's groups stand for. Making it: one recipe whose
    /// input lists the rest of the group, as many as a slot cycles through.
    /// Using it: one recipe for each block it turns into, as a stonecutter's.
    fn group_recipes(&self, item: u32, uses: bool) -> Vec<Recipe> {
        let mut out = Vec::new();
        for group in self.groups_of(item) {
            let others: Vec<u32> = (group.members.iter().copied())
                .filter(|&member| member != item)
                .collect();
            if uses {
                let made = |&other: &u32| workbench_recipe(group.bench, &[item], other);
                out.extend(others.iter().map(made));
            } else {
                let made = |inputs: &[u32]| workbench_recipe(group.bench, inputs, item);
                out.extend(others.chunks(STACKS_MAX).map(made));
            }
        }
        out
    }

    /// The id of the item that stands for a station: a workbench's is its own.
    fn station_icon(&self, station: Station) -> &str {
        match station {
            Station::Workbench(bench) => {
                (self.benches.get(bench as usize)).map_or(station.icon(), String::as_str)
            }
            _ => station.icon(),
        }
    }

    /// The name of a mod's station whose item is not in the game (the game
    /// has its recipes and not the block): its id in words, `Mason Table` for
    /// `chipped:mason_table`. None for a station that has its item.
    fn station_label(&self, station: Station) -> Option<String> {
        let own = matches!(
            station,
            Station::Woodcutter | Station::Kiln | Station::Workbench(_)
        );
        let icon = self.station_icon(station);
        if !own || self.ids.contains_key(icon) {
            return None;
        }
        let path = icon.rsplit([':', '/']).next().unwrap_or(icon);
        let words: Vec<String> = (path.split(|c: char| !c.is_alphanumeric()))
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut chars = word.chars();
                let first = chars.next().map(|c| c.to_ascii_uppercase());
                first.into_iter().chain(chars).collect()
            })
            .collect();
        Some(words.join(" "))
    }

    /// A station's tab: its item, or, for a station without one, the vanilla
    /// station nearest it under the station's name.
    fn station_tab(&self, station: Station) -> Stack {
        match self.station_label(station) {
            Some(label) => Stack {
                components: format!("[minecraft:custom_name=\"{label}\"]"),
                ..Stack::item(station.stand_in())
            },
            None => Stack::item(self.station_icon(station)),
        }
    }

    /// The order of the list now.
    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// Another order: the output lists the items again.
    pub fn set_sort(&mut self, sort: Sort) -> Out {
        self.sort = sort;
        self.list()
    }

    fn id(&self, item: u32) -> &str {
        self.items
            .get(item as usize)
            .map_or("minecraft:air", |i| &i.id)
    }

    /// The game's name for an item id.
    pub fn item_name(&self, id: &str) -> String {
        self.name(id)
    }

    fn name(&self, item: &str) -> String {
        self.ids
            .get(item)
            .and_then(|&index| self.items.get(index as usize))
            .map_or_else(|| item.to_owned(), |i| i.name.clone())
    }

    /// How many recipes make a stack and how many use it. A stack with
    /// components counts those recipes when the tables name them, else every
    /// recipe of its item.
    pub fn counts(&self, item: &str, components: &str) -> Counts {
        let Some(&index) = self.ids.get(item) else {
            return Counts::default();
        };
        let named = (!components.is_empty())
            .then(|| self.counts.get(&(index, components.to_owned())))
            .flatten();
        let listed = named
            .or_else(|| self.counts.get(&(index, String::new())))
            .copied()
            .unwrap_or_default();
        let grouped = self.group_counts(index);
        Counts {
            recipes: listed.recipes + grouped.recipes,
            uses: listed.uses + grouped.uses,
        }
    }

    fn convert(&self, ingredient: &Ingredient) -> Stack {
        Stack {
            item: self.id(ingredient.item).to_owned(),
            count: ingredient.count,
            components: ingredient.components.clone(),
        }
    }

    /// The items the search keeps, each followed by its variants, in the
    /// configured order: JEI's words, phrases, exclusions and prefixes.
    fn list(&mut self) -> Out {
        let mut listed: Vec<Entry> = Vec::new();
        let filter = Search::new(&self.search);
        for (index, item) in self.items.iter().enumerate() {
            if item.id == "minecraft:air" || !filter.keeps(&item.search()) {
                continue;
            }
            if listed.len() >= ITEMS_MAX {
                break;
            }
            listed.push(self.entry(index as u32, String::new()));
            let variants = self.variants.get(&(index as u32));
            for variant in variants.into_iter().flatten() {
                if listed.len() >= ITEMS_MAX {
                    break;
                }
                listed.push(self.entry(index as u32, variant.components.clone()));
            }
        }
        listed.sort_by(|a, b| a.key.cmp(&b.key));
        self.listed = listed;
        let stacks = (self.listed.iter())
            .map(|entry| self.stack(entry))
            .collect();
        Out::Items(stacks)
    }

    /// A row, with its place in the sort.
    fn entry(&self, item: u32, components: String) -> Entry {
        let key = self.entry_key(item, &components);
        Entry {
            item,
            components,
            key,
        }
    }

    fn entry_key(&self, item: u32, components: &str) -> String {
        let Some(item) = self.items.get(item as usize) else {
            return components.to_owned();
        };
        match self.sort {
            Sort::Name => format!(
                "{}\u{0}{}\u{0}{components}",
                item.name.to_lowercase(),
                item.id
            ),
            Sort::Id => format!("{}\u{0}{components}", item.id),
        }
    }

    fn stack(&self, entry: &Entry) -> Stack {
        Stack {
            item: self.id(entry.item).to_owned(),
            count: 1,
            components: entry.components.clone(),
        }
    }

    /// The recipes the current page lists, across every station: the
    /// tables', then the ones the item's workbench groups stand for.
    fn found(&self, page: &Page) -> Vec<std::borrow::Cow<'_, Recipe>> {
        let Some(&item) = self.ids.get(&page.item.item) else {
            return Vec::new();
        };
        // The same item, and the same components when the page names some.
        let components = page.item.components.as_str();
        let hit = |s: &Ingredient| {
            s.item == item && (components.is_empty() || s.components == components)
        };
        let mut found: Vec<std::borrow::Cow<'_, Recipe>> = (self.recipes.iter())
            .filter(|r| {
                if page.uses {
                    r.slots.iter().flatten().any(hit)
                } else {
                    r.results.iter().any(hit)
                }
            })
            .map(std::borrow::Cow::Borrowed)
            .collect();
        let grouped = self.group_recipes(item, page.uses);
        found.extend(grouped.into_iter().map(std::borrow::Cow::Owned));
        found
    }

    /// Opens a page, pushing it on the history; says why when it is empty.
    fn open(&mut self, item: &Stack, uses: bool, known: &[i32]) -> Vec<Out> {
        let item = Stack {
            count: 1,
            ..item.clone()
        };
        let mut page = Page { item, uses, tab: 0 };
        if !page.item.components.is_empty() && self.found(&page).is_empty() {
            // A stack the tables don't name with its components (a worn tool,
            // a named block) has its item's recipes, as its tooltip counts.
            page.item.components.clear();
        }
        if self.found(&page).is_empty() {
            let name = self.name(&page.item.item).to_owned();
            return vec![Out::Say(format!(
                "{}: {name}",
                if uses {
                    self.lang.no_uses()
                } else {
                    self.lang.no_recipes()
                }
            ))];
        }
        self.history.push(page);
        vec![self.show(known)]
    }

    /// The page on top of the history, or the items when there is none.
    fn show(&mut self, known: &[i32]) -> Out {
        let Some(mut page) = self.history.last().cloned() else {
            self.shown.clear();
            return Out::Recipes {
                title: String::new(),
                tabs: Vec::new(),
                tab: 0,
                views: Vec::new(),
            };
        };
        let found = self.found(&page);
        let mut stations: Vec<Station> = found
            .iter()
            .map(|r| r.kind.station())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        stations.sort_by_key(|s| s.order());
        page.tab = page.tab.min(stations.len().saturating_sub(1));
        let selected = stations.get(page.tab).copied();
        let on_station: Vec<&Recipe> = found
            .iter()
            .map(|r| r.as_ref())
            .filter(|r| Some(r.kind.station()) == selected)
            .collect();
        let total = on_station.len();
        let mut views: Vec<View> = on_station
            .iter()
            .take(RECIPES_MAX)
            .map(|&r| self.view(r, known))
            .collect();
        // A station without an item of its own is named on its first recipe.
        let label = selected.and_then(|station| self.station_label(station));
        if let (Some(label), Some(first)) = (label, views.first_mut()) {
            first.note = if first.note.is_empty() {
                label
            } else {
                format!("{label}: {}", first.note)
            };
        }
        self.shown = on_station
            .iter()
            .take(RECIPES_MAX)
            .map(|r| r.book_id)
            .collect();
        let mut title = format!(
            "{}: {}",
            if page.uses {
                self.lang.uses()
            } else {
                self.lang.recipes()
            },
            self.name(&page.item.item)
        );
        if total > views.len() {
            title.push_str(&format!(" ({} of {total})", views.len()));
        }
        let tabs = (stations.iter()).map(|s| self.station_tab(*s)).collect();
        if let Some(top) = self.history.last_mut() {
            top.tab = page.tab;
        }
        Out::Recipes {
            title,
            tabs,
            tab: page.tab as u32,
            views,
        }
    }

    /// A recipe as the panel draws it.
    fn view(&self, recipe: &Recipe, known: &[i32]) -> View {
        let slots = (recipe.slots.iter())
            .map(|slot| {
                slot.iter()
                    .take(STACKS_MAX)
                    .map(|s| self.convert(s))
                    .collect()
            })
            .collect();
        let results = (recipe.results.iter())
            .take(STACKS_MAX)
            .map(|s| self.convert(s))
            .collect();
        let (layout, width, height, note) = match recipe.kind {
            Kind::CraftingShaped => (Layout::Grid, recipe.width, recipe.height, String::new()),
            Kind::CraftingShapeless => {
                let n = recipe.slots.len() as u32;
                let width = match n {
                    0 | 1 => 1,
                    2..=4 => 2,
                    _ => 3,
                };
                (
                    Layout::Grid,
                    width,
                    n.div_ceil(width).max(1),
                    self.lang.shapeless().into(),
                )
            }
            Kind::Smelting | Kind::Blasting | Kind::Smoking | Kind::Campfire | Kind::Firing => {
                let seconds = recipe.cooking_time as f32 / 20.0;
                (
                    Layout::Cooking,
                    1,
                    1,
                    format!(
                        "{seconds} s, {} XP",
                        (recipe.experience * 100.0).round() / 100.0
                    ),
                )
            }
            Kind::Stonecutting | Kind::Woodcutting => (Layout::Single, 1, 1, String::new()),
            Kind::Workbench(_) => {
                // One input: the note says when any block of several fits.
                let fit = recipe.slots.first().map_or(0, Vec::len);
                let note = if fit > 1 {
                    self.lang.any_of(fit)
                } else {
                    String::new()
                };
                (Layout::Single, 1, 1, note)
            }
            Kind::Smithing => (Layout::Smithing, 3, 1, String::new()),
            Kind::Brewing => (Layout::Brewing, 2, 1, String::new()),
        };
        View {
            layout,
            width,
            height,
            slots,
            results,
            note,
            action: (recipe.book_id >= 0).then(|| known.contains(&recipe.book_id)),
        }
    }

    /// The player's event.
    pub fn handle(&mut self, event: Event, ctx: &Context<'_>) -> Vec<Out> {
        match event {
            Event::Search(text) => {
                self.search = text;
                vec![self.list()]
            }
            Event::Clicked { index, right } => {
                let Some(entry) = self.listed.get(index) else {
                    return Vec::new();
                };
                let stack = self.stack(entry);
                if ctx.give && ctx.creative {
                    return vec![Out::Give {
                        item: stack.item,
                        count: if right { 99 } else { 1 },
                    }];
                }
                self.open(&stack, right, ctx.known)
            }
            Event::Open { stack, uses } => self.open(&stack, uses, ctx.known),
            Event::Key { key, stack } => {
                if key == ctx.recipe_key {
                    self.open(&stack, false, ctx.known)
                } else if key == ctx.uses_key {
                    self.open(&stack, true, ctx.known)
                } else {
                    Vec::new()
                }
            }
            Event::Tab(tab) => {
                if let Some(page) = self.history.last_mut() {
                    page.tab = tab;
                }
                vec![self.show(ctx.known)]
            }
            Event::Back => {
                self.history.pop();
                vec![self.show(ctx.known)]
            }
            Event::SlotClicked { stack, right } => self.open(&stack, right, ctx.known),
            Event::Action(i) => match self.shown.get(i) {
                Some(&book) if book >= 0 => vec![Out::Place {
                    book_id: book,
                    all: false,
                }],
                _ => Vec::new(),
            },
        }
    }
}

/// The viewer across world changes and the enabled switch in Options > Mods.
#[derive(Default)]
pub struct Session {
    viewer: Option<Viewer>,
}

impl Session {
    pub fn new() -> Self {
        Self { viewer: None }
    }

    pub fn viewer(&self) -> Option<&Viewer> {
        self.viewer.as_ref()
    }

    pub fn viewer_mut(&mut self) -> Option<&mut Viewer> {
        self.viewer.as_mut()
    }

    pub fn set_viewer(&mut self, viewer: Viewer) {
        self.viewer = Some(viewer);
    }

    pub fn clear(&mut self) {
        self.viewer = None;
    }

    /// Synchronizes the session on a frame:
    /// - Outside a world (`has_player` is false), the cached viewer is cleared
    ///   even while the mod is disabled in Options > Mods.
    /// - When the mod is disabled, returns false without building.
    /// - When enabled in a world, returns true if a viewer needs to be built.
    pub fn update(&mut self, has_player: bool, enabled: bool) -> bool {
        if !has_player {
            self.clear();
            return false;
        }
        if !enabled {
            return false;
        }
        self.viewer.is_none()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn title_has_name_and_version() {
        assert_eq!(super::title(), "JEI 0.2.1");
    }

    #[test]
    fn about_names_the_hook_points() {
        assert!(super::about().contains("hook points version 313"));
    }

    #[test]
    fn about_counts_lists_the_tables() {
        let text = super::about_counts(1658, 1234);
        assert!(text.contains("1658 items, 1234 recipes"), "{text}");
    }

    #[test]
    fn give_is_off_until_turned_on() {
        assert!(!super::give(None));
        assert!(!super::give(Some("false")));
        assert!(super::give(Some("true")));
    }

    #[test]
    fn commands() {
        assert_eq!(super::command(""), Some(super::Command::About));
        assert_eq!(super::command("  about "), Some(super::Command::About));
        assert_eq!(super::command("give"), Some(super::Command::ToggleGive));
        assert_eq!(
            super::command("sort id"),
            Some(super::Command::Sort(super::Sort::Id))
        );
        assert_eq!(
            super::command("sort  name "),
            Some(super::Command::Sort(super::Sort::Name))
        );
        assert_eq!(super::command("sort"), Some(super::Command::SortNone));
        assert_eq!(
            super::command("bookmarks clear"),
            Some(super::Command::BookmarksClear)
        );
        assert_eq!(super::command("config"), Some(super::Command::Config));
        assert_eq!(super::command("sort by size"), None);
        assert_eq!(super::command("nope"), None);
    }

    // SearchTokenizerTest (JEI, MIT): the same cases, with character offsets.
    fn token(text: &str, exclusion: bool, start: usize, end: usize) -> super::Token {
        super::Token {
            text: text.to_owned(),
            exclusion,
            operator: false,
            start,
            end,
        }
    }

    #[test]
    fn search_simple_words() {
        assert_eq!(
            super::tokenize("iron sword"),
            vec![token("iron", false, 0, 4), token("sword", false, 5, 10),]
        );
    }

    #[test]
    fn search_quoted_phrase() {
        assert_eq!(
            super::tokenize("\"iron sword\""),
            vec![token("iron sword", false, 0, 12)]
        );
        assert_eq!(
            super::tokenize("\"iron sword"),
            vec![token("iron sword", false, 0, 11)]
        );
    }

    #[test]
    fn search_exclusion() {
        assert_eq!(
            super::tokenize("-diamond"),
            vec![token("diamond", true, 0, 8)]
        );
        assert_eq!(
            super::tokenize("-\"iron sword\""),
            vec![token("iron sword", true, 0, 13)]
        );
    }

    #[test]
    fn search_dash_inside_quotes_is_text() {
        assert_eq!(
            super::tokenize("\"-iron sword\""),
            vec![token("-iron sword", false, 0, 13)]
        );
        assert_eq!(
            super::tokenize("-\"-iron sword\""),
            vec![token("-iron sword", true, 0, 14)]
        );
    }

    #[test]
    fn search_mixed() {
        assert_eq!(
            super::tokenize("sword \"iron pick\" -wood"),
            vec![
                token("sword", false, 0, 5),
                token("iron pick", false, 6, 17),
                token("wood", true, 18, 23),
            ]
        );
    }

    #[test]
    fn search_empty_and_whitespace() {
        assert!(super::tokenize("").is_empty());
        assert!(super::tokenize("   ").is_empty());
        assert!(super::tokenize("-   ").is_empty());
        assert!(super::tokenize(" \"  ").is_empty());
        assert!(super::tokenize("\"\"").is_empty());
    }

    #[test]
    fn search_escaped_quotes() {
        assert_eq!(
            super::tokenize("iron\\\"sword \\\"quoted\\\""),
            vec![
                token("iron\"sword", false, 0, 11),
                token("\"quoted\"", false, 12, 22),
            ]
        );
        assert_eq!(
            super::tokenize("\"the \\\"best\\\" sword\""),
            vec![token("the \"best\" sword", false, 0, 20)]
        );
    }

    #[test]
    fn search_prefixed_quoted_phrases() {
        assert_eq!(
            super::tokenize("%\"redstone blocks\""),
            vec![token("%redstone blocks", false, 0, 18)]
        );
        assert_eq!(
            super::tokenize("-%\"redstone blocks\""),
            vec![token("%redstone blocks", true, 0, 19)]
        );
        assert_eq!(
            super::tokenize("%\"redstone blocks"),
            vec![token("%redstone blocks", false, 0, 17)]
        );
    }

    #[test]
    fn search_trims_inside_quotes() {
        assert_eq!(
            super::tokenize("\"  iron sword  \" -\"  gold ingot  \""),
            vec![
                token("iron sword", false, 0, 16),
                token("gold ingot", true, 17, 34),
            ]
        );
    }

    #[test]
    fn search_whitespace_separates() {
        assert_eq!(
            super::tokenize("iron\t\"gold ingot\"\n-wood\r\n\"red\tstone\""),
            vec![
                token("iron", false, 0, 4),
                token("gold ingot", false, 5, 17),
                token("wood", true, 18, 23),
                token("red\tstone", false, 25, 36),
            ]
        );
    }

    #[test]
    fn search_bare_dash_does_not_exclude() {
        assert_eq!(
            super::tokenize("- diamond - \"iron sword\""),
            vec![
                token("diamond", false, 2, 9),
                token("iron sword", false, 12, 24),
            ]
        );
        assert_eq!(
            super::tokenize("-\"\" diamond -\"  \" gold"),
            vec![token("diamond", false, 4, 11), token("gold", false, 18, 22),]
        );
    }

    #[test]
    fn search_dash_in_a_word_is_not_exclusion() {
        assert_eq!(
            super::tokenize("iron-sword @mod-name %-redstone"),
            vec![
                token("iron-sword", false, 0, 10),
                token("@mod-name", false, 11, 20),
                token("%-redstone", false, 21, 31),
            ]
        );
    }

    #[test]
    fn search_or_operator() {
        assert_eq!(
            super::tokenize("iron|sword"),
            vec![
                token("iron", false, 0, 4),
                super::Token::operator(4),
                token("sword", false, 5, 10),
            ]
        );
        assert_eq!(
            super::tokenize("iron | sword"),
            vec![
                token("iron", false, 0, 4),
                super::Token::operator(5),
                token("sword", false, 7, 12),
            ]
        );
        assert_eq!(
            super::tokenize("\"iron|sword\""),
            vec![token("iron|sword", false, 0, 12)]
        );
    }

    #[test]
    fn search_groups_by_or() {
        let groups = super::split_by_operator(&super::tokenize("iron|gold sword"));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[0][0].text, "iron");
        assert_eq!(groups[1].len(), 2);
        assert_eq!(groups[1][0].text, "gold");
        assert_eq!(groups[1][1].text, "sword");
    }

    fn stone<'a>(tags: &'a [String]) -> super::SearchItem<'a> {
        super::SearchItem {
            name: "Stone",
            id: "minecraft:stone",
            tags,
        }
    }

    /// The filter, read and applied to one item.
    fn keeps(item: &super::SearchItem<'_>, search: &str) -> bool {
        super::Search::new(search).keeps(item)
    }

    #[test]
    fn search_keeps_by_name_and_id() {
        let tags = ["minecraft:logs".to_owned()];
        let item = stone(&tags);
        assert!(keeps(&item, ""));
        assert!(keeps(&item, "stone"));
        assert!(keeps(&item, "STONE"));
        assert!(keeps(&item, "minecraft:sto"));
        assert!(!keeps(&item, "diamond"));
        assert!(!keeps(&item, "-stone"));
        assert!(keeps(&item, "-diamond"));
    }

    #[test]
    fn search_keeps_by_prefix() {
        let tags = ["minecraft:logs".to_owned()];
        let item = stone(&tags);
        assert!(keeps(&item, "@minecraft"));
        assert!(!keeps(&item, "@other"));
        assert!(keeps(&item, "#logs"));
        assert!(!keeps(&item, "#stone"));
        // No tooltip or creative-tab data yet: those prefixes match nothing.
        assert!(!keeps(&item, "$anything"));
        assert!(!keeps(&item, "%anything"));
    }

    #[test]
    fn search_keeps_or_and_phrases() {
        let tags: [String; 0] = [];
        let item = stone(&tags);
        assert!(keeps(&item, "iron|stone"));
        assert!(!keeps(&item, "iron|gold"));
        assert!(keeps(&item, "\"one stone\"|stone"));
        assert!(!keeps(&item, "\"one stone\""));
    }

    #[test]
    fn a_search_reads_the_filter_once_and_tests_many_items() {
        let tags = ["minecraft:logs".to_owned()];
        let stone = stone(&tags);
        let dirt = super::SearchItem {
            name: "Dirt",
            id: "minecraft:dirt",
            tags: &tags,
        };
        // Uppercase, to prove the tokens are lowercased once, not per item.
        let search = super::Search::new("STONE|DIRT -#LOGS");
        assert!(search.keeps(&stone));
        assert!(!search.keeps(&dirt));
        let other = super::Search::new("stone ");
        assert!(other.keeps(&stone));
        assert!(!other.keeps(&dirt));
    }

    // The viewer: the list, station tabs, history, give and transfer.
    fn item(id: &str, name: &str) -> super::Item {
        super::Item {
            id: id.to_owned(),
            name: name.to_owned(),
            tags: Vec::new(),
        }
    }

    fn ingredient(item: u32) -> super::Ingredient {
        super::Ingredient {
            item,
            count: 1,
            components: String::new(),
        }
    }

    fn stack(id: &str) -> super::Stack {
        super::Stack::item(id)
    }

    /// Stone, Diamond, Diamond Block, Diamond Ore, Crafting Table, Furnace.
    fn tables() -> (Vec<super::Item>, Vec<super::Recipe>) {
        let items = vec![
            item("minecraft:stone", "Stone"),
            item("minecraft:diamond", "Diamond"),
            item("minecraft:diamond_block", "Block of Diamond"),
            item("minecraft:diamond_ore", "Diamond Ore"),
            item("minecraft:crafting_table", "Crafting Table"),
            item("minecraft:furnace", "Furnace"),
        ];
        let recipes = vec![
            // Block of Diamond -> Diamond (shapeless), book 7.
            super::Recipe {
                kind: super::Kind::CraftingShapeless,
                width: 0,
                height: 0,
                slots: vec![vec![ingredient(2)]],
                results: vec![ingredient(1)],
                cooking_time: 0,
                experience: 0.0,
                book_id: 7,
            },
            // Diamond Ore -> Diamond (smelting), book 8.
            super::Recipe {
                kind: super::Kind::Smelting,
                width: 0,
                height: 0,
                slots: vec![vec![ingredient(3)]],
                results: vec![ingredient(1)],
                cooking_time: 200,
                experience: 1.0,
                book_id: 8,
            },
            // Diamond -> Block of Diamond (shaped 1x1), book 9.
            super::Recipe {
                kind: super::Kind::CraftingShaped,
                width: 1,
                height: 1,
                slots: vec![vec![ingredient(1)]],
                results: vec![ingredient(2)],
                cooking_time: 0,
                experience: 0.0,
                book_id: 9,
            },
            // A smithing recipe the book doesn't show.
            super::Recipe {
                kind: super::Kind::Smithing,
                width: 0,
                height: 0,
                slots: vec![
                    vec![ingredient(0)],
                    vec![ingredient(1)],
                    vec![ingredient(2)],
                ],
                results: vec![ingredient(1)],
                cooking_time: 0,
                experience: 0.0,
                book_id: -1,
            },
        ];
        (items, recipes)
    }

    fn context(known: &[i32]) -> super::Context<'_> {
        super::Context {
            recipe_key: "key.keyboard.r",
            uses_key: "key.keyboard.u",
            give: false,
            creative: false,
            known,
        }
    }

    /// A viewer of the tables, without variants.
    fn viewer(
        items: Vec<super::Item>,
        recipes: Vec<super::Recipe>,
        lang: super::Lang,
    ) -> (super::Viewer, super::Out) {
        super::Viewer::with_variants(items, recipes, Vec::new(), lang)
    }

    #[test]
    fn viewer_lists_items_and_filters() {
        let (items, recipes) = tables();
        let (mut viewer, out) = viewer(items, recipes, super::Lang::En);
        assert!(matches!(&out, super::Out::Items(list) if list.len() == 6));
        let out = viewer.handle(super::Event::Search("diamond".into()), &context(&[]));
        assert!(matches!(&out[0], super::Out::Items(list) if list.len() == 3));
        let out = viewer.handle(super::Event::Search(String::new()), &context(&[]));
        assert!(matches!(&out[0], super::Out::Items(list) if list.len() == 6));
    }

    #[test]
    fn viewer_opens_recipes_on_station_tabs() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        let out = viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[7]),
        );
        let super::Out::Recipes {
            title,
            tabs,
            tab,
            views,
        } = &out[0]
        else {
            panic!("not a page: {out:?}");
        };
        assert_eq!(title, "Recipes: Diamond");
        assert_eq!(*tab, 0);
        assert_eq!(tabs[0].item, "minecraft:crafting_table");
        assert_eq!(tabs[1].item, "minecraft:furnace");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].layout, super::Layout::Grid);
        assert_eq!(views[0].note, "Shapeless");
        // The book holds recipe 7: the "+" is enabled.
        assert_eq!(views[0].action, Some(true));

        let out = viewer.handle(super::Event::Tab(1), &context(&[7]));
        let super::Out::Recipes { tab, views, .. } = &out[0] else {
            panic!("not a page: {out:?}");
        };
        assert_eq!(*tab, 1);
        assert_eq!(views[0].layout, super::Layout::Cooking);
        assert_eq!(views[0].note, "10 s, 1 XP");
    }

    #[test]
    fn viewer_uses_find_every_input() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        let out = viewer.handle(
            super::Event::Key {
                key: "key.keyboard.u".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[]),
        );
        let super::Out::Recipes { title, tabs, .. } = &out[0] else {
            panic!("not a page: {out:?}");
        };
        assert_eq!(title, "Uses: Diamond");
        // Crafting (diamond -> block) and smithing (diamond is its base).
        assert_eq!(tabs.len(), 2, "one tab per station: {tabs:?}");
    }

    #[test]
    fn viewer_back_walks_the_history() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[]),
        );
        viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:diamond_block"),
            },
            &context(&[]),
        );
        let out = viewer.handle(super::Event::Back, &context(&[]));
        assert!(
            matches!(&out[0], super::Out::Recipes { title, .. } if title == "Recipes: Diamond")
        );
        let out = viewer.handle(super::Event::Back, &context(&[]));
        assert!(matches!(&out[0], super::Out::Recipes { views, .. } if views.is_empty()));
    }

    #[test]
    fn viewer_gives_only_in_creative() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        let mut ctx = context(&[]);
        ctx.give = true;
        // The first item the search shows.
        viewer.handle(super::Event::Search("diamond".into()), &ctx);
        // Not creative: the click opens the item's recipes instead.
        let out = viewer.handle(
            super::Event::Clicked {
                index: 0,
                right: false,
            },
            &ctx,
        );
        assert!(matches!(&out[0], super::Out::Recipes { .. }), "{out:?}");
        ctx.creative = true;
        let out = viewer.handle(
            super::Event::Clicked {
                index: 0,
                right: false,
            },
            &ctx,
        );
        assert!(matches!(&out[0], super::Out::Give { count: 1, .. }));
        let out = viewer.handle(
            super::Event::Clicked {
                index: 0,
                right: true,
            },
            &ctx,
        );
        assert!(matches!(&out[0], super::Out::Give { count: 99, .. }));
    }

    #[test]
    fn viewer_places_a_known_book_recipe() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[7]),
        );
        let out = viewer.handle(super::Event::Action(0), &context(&[7]));
        assert_eq!(
            out,
            vec![super::Out::Place {
                book_id: 7,
                all: false
            }]
        );
        // The smithing tab's recipe has no book id: its button does nothing.
        viewer.handle(
            super::Event::Key {
                key: "key.keyboard.u".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[]),
        );
        viewer.handle(super::Event::Tab(1), &context(&[]));
        let out = viewer.handle(super::Event::Action(0), &context(&[]));
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn language_follows_the_setting() {
        assert_eq!(super::lang(None), super::Lang::En);
        assert_eq!(super::lang(Some("es")), super::Lang::Es);
        assert_eq!(super::lang(Some("fr")), super::Lang::En);
        assert_eq!(super::Lang::En.language_button(), "Language: English");
        assert_eq!(super::Lang::Es.language_button(), "Language: Español");
        assert_eq!(super::Lang::En.give_button(true), "Click gives items: ON");
        assert_eq!(super::Lang::Es.give_button(false), "Clic da objetos: OFF");
    }

    #[test]
    fn viewer_speaks_spanish() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::Es);
        let out = viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:diamond"),
            },
            &context(&[]),
        );
        let super::Out::Recipes { title, views, .. } = &out[0] else {
            panic!("not a page: {out:?}");
        };
        assert_eq!(title, "Recetas: Diamond");
        assert_eq!(views[0].note, "Sin forma");
        let out = viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:stone"),
            },
            &context(&[]),
        );
        assert_eq!(out, vec![super::Out::Say("Sin recetas para: Stone".into())]);
    }

    #[test]
    fn viewer_says_when_there_is_nothing() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        let out = viewer.handle(
            super::Event::Key {
                key: "key.keyboard.r".into(),
                stack: stack("minecraft:stone"),
            },
            &context(&[]),
        );
        assert_eq!(out, vec![super::Out::Say("No recipes for: Stone".into())]);
    }

    // The bookmarks: the stack under a key, the bar and its file.
    fn potion(components: &str) -> super::Stack {
        super::Stack {
            item: "minecraft:potion".to_owned(),
            count: 1,
            components: components.to_owned(),
        }
    }

    #[test]
    fn bookmarks_toggle_and_keep_their_order() {
        let mut bookmarks = super::Bookmarks::default();
        assert!(bookmarks.is_empty());
        assert_eq!(
            bookmarks.toggle(&stack("minecraft:stone")),
            super::Toggle::Added(0)
        );
        assert_eq!(
            bookmarks.toggle(&potion("[potion=\"minecraft:strength\"]")),
            super::Toggle::Added(1)
        );
        // The same item with other components is another bookmark.
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(
            bookmarks.toggle(&stack("minecraft:stone")),
            super::Toggle::Removed(0)
        );
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks.slots()[0].item, "minecraft:potion");
        // A stack is bookmarked with a count of one.
        assert_eq!(bookmarks.slots()[0].count, 1);
    }

    #[test]
    fn bookmarks_hold_twenty_four() {
        let mut bookmarks = super::Bookmarks::default();
        for n in 0..super::BOOKMARKS_MAX {
            let item = super::Stack::item(&format!("minecraft:item_{n}"));
            assert_eq!(bookmarks.toggle(&item), super::Toggle::Added(n));
        }
        assert_eq!(
            bookmarks.toggle(&super::Stack::item("minecraft:one_too_many")),
            super::Toggle::Full
        );
        bookmarks.clear();
        assert!(bookmarks.is_empty());
    }

    #[test]
    fn bookmarks_move_and_remove() {
        let mut bookmarks =
            super::bookmarks_parse("minecraft:a\t1\t\nminecraft:b\t1\t\nminecraft:c\t1\t");
        assert_eq!(bookmarks.len(), 3);
        assert!(bookmarks.move_slot(0, 2));
        let order: Vec<&str> = (bookmarks.slots().iter())
            .map(|s| s.item.as_str())
            .collect();
        assert_eq!(order, ["minecraft:b", "minecraft:c", "minecraft:a"]);
        assert!(!bookmarks.move_slot(2, 2));
        assert!(!bookmarks.move_slot(0, 3));
        assert_eq!(
            bookmarks.remove(1).map(|s| s.item).as_deref(),
            Some("minecraft:c")
        );
        assert!(bookmarks.remove(9).is_none());
        assert_eq!(bookmarks.len(), 2);
    }

    #[test]
    fn bookmarks_round_trip_through_their_file() {
        let mut bookmarks = super::Bookmarks::default();
        bookmarks.toggle(&stack("minecraft:diamond"));
        bookmarks.toggle(&potion("[potion=\"minecraft:swiftness\"]"));
        let text = super::bookmarks_text(&bookmarks);
        let back = super::bookmarks_parse(&text);
        assert_eq!(back, bookmarks);
        // Comments, blank lines and junk are skipped; a missing count is one.
        let back = super::bookmarks_parse("# mine\n\nminecraft:stone\n\t\n");
        assert_eq!(back.len(), 1);
        assert_eq!(back.slots()[0].count, 1);
        // Air is not a bookmark.
        assert!(super::bookmarks_parse("minecraft:air\t1\t\n").is_empty());
    }

    #[test]
    fn config_is_read_and_written() {
        let default = super::Config::default();
        assert_eq!(default.sort, super::Sort::Name);
        assert!(default.hints && default.bookmarks);
        let back = super::Config::parse(&default.to_text());
        assert_eq!(back, default);
        let back = super::Config::parse("sort=id\nhints=off\nbookmarks=no\n");
        assert_eq!(back.sort, super::Sort::Id);
        assert!(!back.hints && !back.bookmarks);
        // Unknown keys and values keep what was there.
        let back = super::Config::parse("sort=nope\nsize=3\n");
        assert_eq!(back, default);
        assert_eq!(super::Sort::Name.next(), super::Sort::Id);
        assert_eq!(super::Sort::Name.label(super::Lang::Es), "Nombre");
    }

    #[test]
    fn the_overlay_is_only_on_container_screens() {
        assert!(super::overlay_screen("inventory"));
        assert!(super::overlay_screen("creative"));
        // The pause and options screens over a world still have the player's
        // menu open, so a slot count is not enough to tell them apart.
        assert!(!super::overlay_screen("paused"));
        assert!(!super::overlay_screen("options"));
        assert!(!super::overlay_screen("chat"));
        assert!(!super::overlay_screen("title"));
    }

    #[test]
    fn the_bar_holds_two_columns_of_slots() {
        let bar = super::bookmark_bar(240.0);
        assert_eq!(bar.columns, 2);
        assert_eq!(bar.rows, 12);
        assert_eq!(bar.capacity(), super::BOOKMARKS_MAX);
        assert_eq!(bar.slot(0), [4.0, 4.0, 18.0, 18.0]);
        assert_eq!(bar.slot(1), [22.0, 4.0, 18.0, 18.0]);
        assert_eq!(bar.slot(2), [4.0, 22.0, 18.0, 18.0]);
        // A short screen shows fewer rows, never none.
        assert_eq!(super::bookmark_bar(100.0).rows, 5);
        assert_eq!(super::bookmark_bar(10.0).rows, 1);
        // The slots under a point, only as many as there are bookmarks.
        let bar = super::bookmark_bar(240.0);
        assert_eq!(bar.hit([5.0, 5.0], 3), Some(0));
        assert_eq!(bar.hit([23.0, 5.0], 3), Some(1));
        assert_eq!(bar.hit([23.0, 5.0], 1), None);
        assert_eq!(bar.hit([3.0, 5.0], 3), None);
        assert_eq!(bar.hit([5.0, 23.0], 0), None);
        assert_eq!(bar.rect(), [4.0, 4.0, 36.0, 216.0]);
        assert_eq!(super::bookmark_bar(240.0).capacity(), 24);
    }

    #[test]
    fn tooltip_hints_say_what_the_item_has() {
        let counts = super::Counts {
            recipes: 12,
            uses: 3,
        };
        let lines = super::tooltip_lines(counts, true, super::Lang::En);
        assert_eq!(lines[0].0, "R: Recipes (12)");
        assert_eq!(lines[1].0, "U: Uses (3)");
        assert_eq!(lines[2], ("★ Bookmarked".to_owned(), super::MARK_COLOR));
        let lines = super::tooltip_lines(super::Counts::default(), false, super::Lang::Es);
        assert!(lines.is_empty());
        let es = super::tooltip_lines(counts, false, super::Lang::Es);
        assert_eq!(es[0].0, "R: Recetas (12)");
        let bar = super::bookmark_tooltip("Diamond", super::Lang::En);
        assert_eq!(bar[0].0, "Diamond");
        assert!(bar[1].0.contains("Left: recipes"));
    }

    #[test]
    fn viewer_lists_variants_after_their_item() {
        let items = vec![
            item("minecraft:stone", "Stone"),
            item("minecraft:potion", "Potion"),
        ];
        let recipes = vec![super::Recipe {
            kind: super::Kind::Brewing,
            width: 0,
            height: 0,
            slots: vec![vec![ingredient(0)]],
            results: vec![
                super::Ingredient {
                    item: 1,
                    count: 1,
                    components: "[potion=\"minecraft:strength\"]".to_owned(),
                },
                super::Ingredient {
                    item: 1,
                    count: 1,
                    components: "[potion=\"minecraft:swiftness\"]".to_owned(),
                },
            ],
            cooking_time: 0,
            experience: 0.0,
            book_id: -1,
        }];
        let variants = super::harvest_variants(&recipes, 8, 100);
        assert_eq!(variants.len(), 2);
        let (mut viewer, out) =
            super::Viewer::with_variants(items, recipes.clone(), variants, super::Lang::En);
        let super::Out::Items(list) = &out else {
            panic!("not a list: {out:?}");
        };
        // By name: Potion, its two variants, then Stone.
        assert_eq!(list.len(), 4);
        assert_eq!(list[0].item, "minecraft:potion");
        assert!(list[0].components.is_empty());
        assert_eq!(list[1].components, "[potion=\"minecraft:strength\"]");
        assert_eq!(list[2].components, "[potion=\"minecraft:swiftness\"]");
        assert_eq!(list[3].item, "minecraft:stone");
        // By id, stone moves first.
        let out = viewer.set_sort(super::Sort::Id);
        let super::Out::Items(list) = &out else {
            panic!("not a list: {out:?}");
        };
        assert_eq!(list[0].item, "minecraft:potion");
        assert_eq!(list[3].item, "minecraft:stone");
        // A variant opens the recipes the tables name for its components.
        let out = viewer.handle(
            super::Event::Open {
                stack: super::Stack {
                    item: "minecraft:potion".to_owned(),
                    count: 1,
                    components: "[potion=\"minecraft:strength\"]".to_owned(),
                },
                uses: false,
            },
            &context(&[]),
        );
        assert!(matches!(&out[0], super::Out::Recipes { .. }), "{out:?}");
    }

    #[test]
    fn viewers_count_what_makes_and_uses_an_item() {
        let (items, recipes) = tables();
        let (viewer, _) = viewer(items, recipes, super::Lang::En);
        // Diamond is made by three recipes (shapeless, smelting, smithing) and
        // used by two (the block, the smithing base).
        assert_eq!(
            viewer.counts("minecraft:diamond", ""),
            super::Counts {
                recipes: 3,
                uses: 2
            }
        );
        assert_eq!(viewer.counts("minecraft:stone", "").recipes, 0);
        assert_eq!(
            viewer.counts("minecraft:nope", ""),
            super::Counts::default()
        );
        assert_eq!(
            viewer.item_name("minecraft:diamond_block"),
            "Block of Diamond"
        );
        assert_eq!(viewer.item_name("minecraft:nope"), "minecraft:nope");
        assert_eq!(viewer.sort(), super::Sort::Name);
    }

    // The recipe types of mods: a woodcutter, a kiln and a workbench's groups.
    fn tagged(id: &str, name: &str, tags: &[&str]) -> super::Item {
        super::Item {
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            ..item(id, name)
        }
    }

    fn tag(name: &str) -> Vec<super::Source> {
        vec![super::Source::Tag(name.to_owned())]
    }

    fn one(id: &str) -> Vec<super::Source> {
        vec![super::Source::Item(id.to_owned())]
    }

    /// Oak Planks, Oak Stairs, Clay, Terracotta, Amethyst and three variants
    /// of it, a pillar in a second group, the three stations and a lone block.
    fn mod_items() -> Vec<super::Item> {
        vec![
            tagged("minecraft:oak_planks", "Oak Planks", &["minecraft:planks"]),
            item("minecraft:oak_stairs", "Oak Stairs"),
            item("minecraft:clay", "Clay"),
            item("minecraft:terracotta", "Terracotta"),
            tagged(
                "minecraft:amethyst_block",
                "Block of Amethyst",
                &["chipped:amethyst_block"],
            ),
            tagged(
                "chipped:amethyst_bricks",
                "Amethyst Bricks",
                &["chipped:amethyst_block"],
            ),
            tagged(
                "chipped:amethyst_pillar",
                "Amethyst Pillar",
                &["chipped:amethyst_block", "chipped:pillars"],
            ),
            tagged(
                "chipped:amethyst_tiles",
                "Amethyst Tiles",
                &["chipped:amethyst_block"],
            ),
            tagged("chipped:stone_pillar", "Stone Pillar", &["chipped:pillars"]),
            item("charm:woodcutter", "Woodcutter"),
            item("charm:kiln", "Kiln"),
            item("chipped:mason_table", "Mason Table"),
            tagged("chipped:lonely", "Lonely", &["chipped:lonely"]),
        ]
    }

    fn mod_viewer() -> super::Viewer {
        let items = mod_items();
        let lookup = super::Lookup::new(&items);
        let mut custom = super::Custom::default();
        assert!(custom.recipe(
            &lookup,
            super::Kind::Woodcutting,
            &[tag("minecraft:planks")],
            "minecraft:oak_stairs",
            1,
            0,
            0.0
        ));
        assert!(custom.recipe(
            &lookup,
            super::Kind::Firing,
            &[one("clay")],
            "terracotta",
            1,
            100,
            0.1
        ));
        // Unknown items add nothing.
        assert!(!custom.recipe(
            &lookup,
            super::Kind::Firing,
            &[one("nope:clay")],
            "terracotta",
            1,
            100,
            0.1
        ));
        assert!(!custom.recipe(
            &lookup,
            super::Kind::Firing,
            &[one("clay")],
            "nope:pot",
            1,
            100,
            0.1
        ));
        // A group of one block and an unknown tag are left out.
        let groups = [
            tag("chipped:amethyst_block"),
            tag("chipped:pillars"),
            tag("chipped:lonely"),
            tag("chipped:nothing"),
        ];
        assert_eq!(custom.bench(&lookup, "chipped:mason_table", &groups), 2);
        assert_eq!(custom.bench(&lookup, "chipped:empty_table", &[]), 0);
        assert_eq!(custom.benches, vec!["chipped:mason_table".to_owned()]);
        drop(lookup);
        let (viewer, _) = super::Viewer::with_groups(
            items,
            custom.recipes,
            Vec::new(),
            custom.benches,
            custom.groups,
            super::Lang::En,
        );
        viewer
    }

    fn page(out: &[super::Out]) -> (&str, Vec<&str>, &[super::View]) {
        let super::Out::Recipes {
            title, tabs, views, ..
        } = &out[0]
        else {
            panic!("not a page: {out:?}");
        };
        let tabs = tabs.iter().map(|t| t.item.as_str()).collect();
        (title.as_str(), tabs, views.as_slice())
    }

    fn ids(stacks: &[super::Stack]) -> Vec<&str> {
        stacks.iter().map(|s| s.item.as_str()).collect()
    }

    fn open(viewer: &mut super::Viewer, id: &str, uses: bool) -> Vec<super::Out> {
        viewer.handle(
            super::Event::Open {
                stack: stack(id),
                uses,
            },
            &context(&[]),
        )
    }

    #[test]
    fn lookups_read_items_and_tags() {
        let items = mod_items();
        let lookup = super::Lookup::new(&items);
        assert_eq!(lookup.item("oak_planks"), Some(0));
        assert_eq!(lookup.item("minecraft:oak_planks"), Some(0));
        assert_eq!(lookup.item("nope"), None);
        assert_eq!(
            lookup.items(&tag("chipped:amethyst_block")),
            vec![4, 5, 6, 7]
        );
        // A bare tag is vanilla's; an item named twice is listed once.
        let mut sources = tag("planks");
        sources.extend(one("oak_planks"));
        sources.extend(one("minecraft:clay"));
        assert_eq!(lookup.items(&sources), vec![0, 2]);
        assert!(lookup.items(&tag("chipped:nothing")).is_empty());
    }

    #[test]
    fn woodcutting_and_firing_have_their_stations() {
        let mut viewer = mod_viewer();
        let out = open(&mut viewer, "minecraft:oak_stairs", false);
        let (title, tabs, views) = page(&out);
        assert_eq!(title, "Recipes: Oak Stairs");
        assert_eq!(tabs, vec!["charm:woodcutter"]);
        assert_eq!(views[0].layout, super::Layout::Single);
        assert_eq!(ids(&views[0].slots[0]), vec!["minecraft:oak_planks"]);
        assert_eq!(views[0].action, None);

        let out = open(&mut viewer, "minecraft:clay", true);
        let (title, tabs, views) = page(&out);
        assert_eq!(title, "Uses: Clay");
        assert_eq!(tabs, vec!["charm:kiln"]);
        assert_eq!(views[0].layout, super::Layout::Cooking);
        assert_eq!(views[0].note, "5 s, 0.1 XP");
        assert_eq!(ids(&views[0].results), vec!["minecraft:terracotta"]);
        let about = viewer.about_custom().unwrap_or_default();
        let stations = "JEI: 1 woodcutting, 1 kiln, 1 workbenches with 2 groups of 5 blocks;";
        assert!(about.starts_with(stations), "{about}");
        let items = "7 of 13 items have a recipe, 0 without these";
        assert!(about.ends_with(items), "{about}");
    }

    #[test]
    fn a_workbench_makes_a_block_from_the_rest_of_its_group() {
        let mut viewer = mod_viewer();
        let out = open(&mut viewer, "chipped:amethyst_bricks", false);
        let (title, tabs, views) = page(&out);
        assert_eq!(title, "Recipes: Amethyst Bricks");
        assert_eq!(tabs, vec!["chipped:mason_table"]);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].layout, super::Layout::Single);
        assert_eq!(views[0].note, "Any of these 3");
        assert_eq!(
            ids(&views[0].slots[0]),
            vec![
                "minecraft:amethyst_block",
                "chipped:amethyst_pillar",
                "chipped:amethyst_tiles"
            ]
        );
        assert_eq!(ids(&views[0].results), vec!["chipped:amethyst_bricks"]);
    }

    #[test]
    fn a_workbench_turns_a_block_into_each_other_of_its_groups() {
        let mut viewer = mod_viewer();
        let out = open(&mut viewer, "minecraft:amethyst_block", true);
        let (title, tabs, views) = page(&out);
        assert_eq!(title, "Uses: Block of Amethyst");
        assert_eq!(tabs, vec!["chipped:mason_table"]);
        let made: Vec<&str> = views.iter().flat_map(|v| ids(&v.results)).collect();
        assert_eq!(
            made,
            vec![
                "chipped:amethyst_bricks",
                "chipped:amethyst_pillar",
                "chipped:amethyst_tiles"
            ]
        );
        assert!(views.iter().all(|v| v.note.is_empty()));
        assert!(
            views
                .iter()
                .all(|v| ids(&v.slots[0]) == vec!["minecraft:amethyst_block"])
        );

        // The pillar is in two groups: both make it, and it makes both's.
        let out = open(&mut viewer, "chipped:amethyst_pillar", false);
        let (_, _, views) = page(&out);
        assert_eq!(views.len(), 2);
        assert_eq!(ids(&views[1].slots[0]), vec!["chipped:stone_pillar"]);
        assert!(views[1].note.is_empty());
        let out = open(&mut viewer, "chipped:amethyst_pillar", true);
        assert_eq!(page(&out).2.len(), 4);
        assert_eq!(
            viewer.counts("chipped:amethyst_pillar", ""),
            super::Counts {
                recipes: 2,
                uses: 4
            }
        );
        assert_eq!(
            viewer.counts("minecraft:amethyst_block", ""),
            super::Counts {
                recipes: 1,
                uses: 3
            }
        );
        // A block alone in its tag, and one outside every group.
        assert_eq!(
            viewer.counts("chipped:lonely", ""),
            super::Counts::default()
        );
        let out = open(&mut viewer, "chipped:lonely", false);
        assert_eq!(out, vec![super::Out::Say("No recipes for: Lonely".into())]);
    }

    #[test]
    fn a_large_group_is_made_in_slots_of_sixty_four() {
        let mut items = vec![item("chipped:mason_table", "Mason Table")];
        for n in 0..70 {
            items.push(tagged(
                &format!("chipped:brick_{n:02}"),
                &format!("Brick {n:02}"),
                &["chipped:bricks"],
            ));
        }
        let lookup = super::Lookup::new(&items);
        let mut custom = super::Custom::default();
        assert_eq!(
            custom.bench(&lookup, "chipped:mason_table", &[tag("chipped:bricks")]),
            1
        );
        drop(lookup);
        let (mut viewer, _) = super::Viewer::with_groups(
            items,
            Vec::new(),
            Vec::new(),
            custom.benches,
            custom.groups,
            super::Lang::En,
        );
        let out = open(&mut viewer, "chipped:brick_00", false);
        let (_, _, views) = page(&out);
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].slots[0].len(), 64);
        assert_eq!(views[0].note, "Any of these 64");
        assert_eq!(views[1].slots[0].len(), 5);
        let out = open(&mut viewer, "chipped:brick_00", true);
        assert_eq!(page(&out).2.len(), 69);
        assert_eq!(
            viewer.counts("chipped:brick_00", ""),
            super::Counts {
                recipes: 2,
                uses: 69
            }
        );
        assert_eq!(viewer.recipe_count(), 0);
    }

    #[test]
    fn a_missing_station_item_has_a_vanilla_icon() {
        let items = vec![
            tagged("chipped:a", "A", &["chipped:g"]),
            tagged("chipped:b", "B", &["chipped:g"]),
            item("minecraft:stick", "Stick"),
        ];
        let lookup = super::Lookup::new(&items);
        let mut custom = super::Custom::default();
        custom.bench(&lookup, "chipped:mason_table", &[tag("chipped:g")]);
        assert!(custom.recipe(
            &lookup,
            super::Kind::Woodcutting,
            &[one("chipped:a")],
            "stick",
            4,
            0,
            0.0
        ));
        drop(lookup);
        let (mut viewer, _) = super::Viewer::with_groups(
            items,
            custom.recipes,
            Vec::new(),
            custom.benches,
            custom.groups,
            super::Lang::Es,
        );
        let out = open(&mut viewer, "chipped:a", true);
        let (title, tabs, views) = page(&out);
        assert_eq!(title, "Usos: A");
        assert_eq!(tabs, vec!["minecraft:stonecutter", "minecraft:stonecutter"]);
        assert_eq!(views[0].results[0].count, 4);
        // The stations are not items here: each tab and first recipe names its own.
        assert_eq!(views[0].note, "Woodcutter");
        let out = viewer.handle(super::Event::Tab(1), &context(&[]));
        let super::Out::Recipes { tabs, views, .. } = &out[0] else {
            panic!("not a page: {out:?}");
        };
        assert_eq!(tabs[0].components, "[minecraft:custom_name=\"Woodcutter\"]");
        assert_eq!(
            tabs[1].components,
            "[minecraft:custom_name=\"Mason Table\"]"
        );
        assert_eq!(views[0].note, "Mason Table");
        let out = open(&mut viewer, "chipped:b", false);
        assert_eq!(page(&out).2[0].note, "Mason Table");
    }

    #[test]
    fn a_stack_with_components_falls_back_to_its_item() {
        let (items, recipes) = tables();
        let (mut viewer, _) = viewer(items, recipes, super::Lang::En);
        let worn = super::Stack {
            item: "minecraft:diamond".into(),
            count: 1,
            components: "[minecraft:custom_name=\"Shiny\"]".into(),
        };
        let out = viewer.handle(
            super::Event::Open {
                stack: worn,
                uses: false,
            },
            &context(&[]),
        );
        let (title, tabs, _) = page(&out);
        assert_eq!(title, "Recipes: Diamond");
        assert_eq!(tabs.len(), 3);
    }

    #[test]
    fn variants_stop_at_their_limits() {
        let mut results = Vec::new();
        for n in 0..10 {
            results.push(super::Ingredient {
                item: 0,
                count: 1,
                components: format!("[food=\"v{n}\"]"),
            });
        }
        let recipes = vec![super::Recipe {
            kind: super::Kind::Smelting,
            width: 0,
            height: 0,
            slots: vec![vec![ingredient(0)]],
            results,
            cooking_time: 0,
            experience: 0.0,
            book_id: -1,
        }];
        assert_eq!(super::harvest_variants(&recipes, 3, 100).len(), 3);
        assert_eq!(super::harvest_variants(&recipes, 10, 4).len(), 4);
        // No components, no variants.
        assert!(super::harvest_variants(&[], 8, 100).is_empty());
    }

    #[test]
    fn session_clears_viewer_across_world_switch_while_disabled() {
        let mut session = super::Session::new();
        let (items_a, recipes_a) = tables();
        let (viewer_a, _) = viewer(items_a, recipes_a, super::Lang::En);

        // 1. Open world A with JEI enabled: builds and caches viewer A.
        assert!(session.update(true, true));
        session.set_viewer(viewer_a);
        assert!(session.viewer().is_some());
        assert_eq!(
            session
                .viewer()
                .unwrap()
                .counts("minecraft:diamond", "")
                .recipes,
            3
        );

        // 2. Disable JEI while in world A: stays cached for world A.
        assert!(!session.update(true, false));
        assert!(session.viewer().is_some());

        // Re-enabling in the same world A without leaving does not rebuild.
        assert!(!session.update(true, true));
        assert!(session.viewer().is_some());

        // Disable again before leaving.
        assert!(!session.update(true, false));

        // 3. Leave world A (unload world) while JEI is disabled: MUST clear viewer.
        assert!(!session.update(false, false));
        assert!(
            session.viewer().is_none(),
            "viewer must be cleared on world unload even while disabled"
        );

        // 4. Join world B while JEI is still disabled: remains cleared.
        assert!(!session.update(true, false));
        assert!(session.viewer().is_none());

        // 5. Re-enable JEI in world B: must signal rebuild for world B.
        assert!(
            session.update(true, true),
            "re-enabling in world B must request a new viewer build"
        );

        // World B's catalog has emerald instead of diamond.
        let items_b = vec![item("minecraft:emerald", "Emerald")];
        let (viewer_b, _) =
            super::Viewer::with_variants(items_b, Vec::new(), Vec::new(), super::Lang::En);
        session.set_viewer(viewer_b);

        // Viewer now holds World B's items and recipes, not World A's.
        assert_eq!(session.viewer().unwrap().item_count(), 1);
        assert_eq!(
            session.viewer().unwrap().item_name("minecraft:emerald"),
            "Emerald"
        );
        assert_eq!(
            session
                .viewer()
                .unwrap()
                .counts("minecraft:diamond", "")
                .recipes,
            0
        );
    }
}
