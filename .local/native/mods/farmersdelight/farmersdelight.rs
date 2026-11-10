// Farmer's Delight's kitchen blocks, written from the mod's documented behaviour, its own data
// files (recipes, tags, sounds, GUI texture) and what its blocks are observed to do:
//  - The cutting board holds one item. A right click with the tool its `farmersdelight:cutting`
//    recipe names cuts it: the results pop out, the tool wears by one and the recipe's sound plays.
//    An empty hand takes the item back.
//  - The stove is lit when placed (flint and steel or a fire charge light it again, a shovel puts it
//    out), takes up to six campfire-cookable items on its top and pops each out cooked.
//  - The cooking pot cooks its six ingredients into a `farmersdelight:cooking` meal when a heat
//    source is below it (a lit stove, magma, lava, a lit campfire, fire). The meal waits in the pot
//    until its container (a bowl, a bottle...) is in the container slot; then it moves to the output.
//    A right click with that container serves one at once.
//  - Nourishment stops hunger from going down; Comfort heals a point every four seconds while the
//    player has no saturation left.
use super::native_block_entity_draw::BlockEntityDraw;
use super::{Game, Mapping};
use horizon_assets::item_model::DisplayContext;
use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer};
use horizon_client::crafting::Ingredient;
use horizon_client::first_person::Hand;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeScreen;
use horizon_world::nbt::{Compound, JavaString, Tag};
use horizon_world::registry::BlockStateId;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

pub(super) const BOARD: &str = "farmersdelight:cutting_board";
pub(super) const STOVE: &str = "farmersdelight:stove";
pub(super) const POT: &str = "farmersdelight:cooking_pot";
pub(super) const SKILLET: &str = "farmersdelight:skillet";
/// How many of one food a skillet fries at once.
pub(super) const SKILLET_MAX: u32 = 8;
const POT_MENU: &str = "farmersdelight:cooking_pot";
const POT_GUI: &str = "farmersdelight:gui/cooking_pot";
const CAMPFIRE: &str = "minecraft:campfire_cooking";
pub(super) const INPUTS: usize = 6;
pub(super) const MEAL: usize = 6;
const CONTAINER: usize = 7;
const OUTPUT: usize = 8;
const STOVE_SLOTS: usize = 6;
/// The board's item, then the layers stacked on it (Hands-On Delight's sandwiches).
pub(super) const BOARD_LAYERS: usize = 6;
/// The stove's six spots on its top, in the block's own frame (north is -z).
const STOVE_SPOTS: [[f32; 2]; STOVE_SLOTS] =
    [[0.25, 0.32], [0.5, 0.32], [0.75, 0.32], [0.25, 0.68], [0.5, 0.68], [0.75, 0.68]];
const BLOCKS: usize = 4; // SoundSource.BLOCKS

fn ours(block: &str) -> bool {
    matches!(block, BOARD | STOVE | POT | SKILLET)
}

pub(super) fn key(s: &str) -> JavaString {
    s.into()
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && ours(block) {
        *entity = Some((block.to_owned(), Compound::default()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == STOVE || block == POT || block == SKILLET {
        *ticks = true;
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let closed = |slots: usize| ModContainer {
        slots,
        faces: Default::default(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        menu: None,
    };
    *container = match block {
        BOARD => Some(closed(BOARD_LAYERS)),
        STOVE => Some(closed(STOVE_SLOTS)),
        SKILLET => Some(closed(1)),
        POT => {
            // Hoppers fill the ingredients from above and the container slot from the sides, and
            // take the served meals from below.
            let ingredients: Vec<usize> = (0..INPUTS).collect();
            let side = vec![CONTAINER];
            let mut inputs = ingredients.clone();
            inputs.push(CONTAINER);
            Some(ModContainer {
                slots: 9,
                faces: [
                    vec![OUTPUT],
                    ingredients,
                    side.clone(),
                    side.clone(),
                    side.clone(),
                    side,
                ],
                inputs,
                outputs: vec![OUTPUT],
                menu: Some(POT_MENU.to_owned()),
            })
        }
        _ => None,
    };
}

// ---- recipes ----

/// The item and tag names an ingredient of a mod's recipe file lists: `{"item"}`, `{"tag"}`, a list,
/// Fabric's `fabric:any` and Farmer's Delight's tool abilities (as the tools that have them).
fn names(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(list) => list.iter().for_each(|v| names(v, out)),
        Value::Object(o) => {
            if let Some(item) = o.get("item").and_then(Value::as_str) {
                out.push(item.to_owned());
            } else if let Some(tag) = o.get("tag").and_then(Value::as_str) {
                out.push(format!("#{tag}"));
            } else if let Some(list) = o.get("ingredients").or_else(|| o.get("children")) {
                names(list, out);
            } else if let Some(action) = o.get("action").and_then(Value::as_str) {
                out.extend(ability(action).iter().map(|s| (*s).to_owned()));
            }
        }
        _ => {}
    }
}

fn ability(action: &str) -> &'static [&'static str] {
    match action {
        "knife_dig" => &["#c:tools/knife"],
        "axe_dig" | "axe_strip" | "axe_scrape" | "axe_wax_off" => &["#minecraft:axes"],
        "shovel_dig" | "shovel_flatten" => &["#minecraft:shovels"],
        "pickaxe_dig" => &["#minecraft:pickaxes"],
        "hoe_dig" | "hoe_till" => &["#minecraft:hoes"],
        "shears_dig" | "shears_carve" | "shears_harvest" => &["minecraft:shears", "#c:tools/shear"],
        "sword_dig" | "sword_sweep" => &["#minecraft:swords"],
        _ => &[],
    }
}

fn ingredient(game: &Game<'_>, v: &Value) -> Option<Ingredient> {
    let mut list = Vec::new();
    names(v, &mut list);
    if list.is_empty() {
        return None;
    }
    game.native_recipe_ingredient(&Value::Array(list.into_iter().map(Value::String).collect()))
}

pub(super) struct Cut {
    pub(super) input: Ingredient,
    pub(super) tool: Ingredient,
    pub(super) results: Vec<(ItemStack, f32)>,
    pub(super) sound: Option<String>,
}

pub(super) struct Meal {
    pub(super) inputs: Vec<Ingredient>,
    pub(super) result: ItemStack,
    pub(super) container: Option<String>,
    pub(super) time: i32,
}

thread_local! {
    static CUTS: RefCell<(usize, usize, Rc<Vec<Cut>>)> = RefCell::new((0, 0, Rc::default()));
    static MEALS: RefCell<(usize, usize, Rc<Vec<Meal>>)> = RefCell::new((0, 0, Rc::default()));
}

/// The table's recipes of `kind`, parsed once for each table (made again when the table is).
fn parsed<T>(
    game: &Game<'_>,
    cache: &'static std::thread::LocalKey<RefCell<(usize, usize, Rc<Vec<T>>)>>,
    kind: &str,
    parse: impl Fn(&Game<'_>, &Value) -> Option<T>,
) -> Rc<Vec<T>> {
    let recipes = game.native_recipes_of_type(kind);
    let stamp = (recipes.as_ptr() as usize, recipes.len());
    cache.with_borrow_mut(|(at, len, list)| {
        if (*at, *len) != stamp || list.is_empty() && !recipes.is_empty() {
            *list = Rc::new(recipes.iter().filter_map(|r| parse(game, &r.json)).collect());
            (*at, *len) = stamp;
        }
        Rc::clone(list)
    })
}

pub(super) fn cuts(game: &Game<'_>) -> Rc<Vec<Cut>> {
    parsed(game, &CUTS, "farmersdelight:cutting", |game, d| {
        let input = ingredient(game, &d["ingredients"])?;
        let tool = ingredient(game, &d["tool"])?;
        let results: Vec<(ItemStack, f32)> = d["result"]
            .as_array()?
            .iter()
            .filter_map(|r| {
                let stack = game.native_recipe_result(&r["item"])?;
                Some((stack, r["chance"].as_f64().unwrap_or(1.0) as f32))
            })
            .collect();
        let sound = d["sound"]["sound_id"].as_str().or(d["sound"].as_str()).map(str::to_owned);
        Some(Cut { input, tool, results, sound })
    })
}

pub(super) fn meals(game: &Game<'_>) -> Rc<Vec<Meal>> {
    parsed(game, &MEALS, "farmersdelight:cooking", |game, d| {
        let inputs: Vec<Ingredient> = d["ingredients"]
            .as_array()?
            .iter()
            .map(|i| ingredient(game, i))
            .collect::<Option<_>>()?;
        if inputs.is_empty() || inputs.len() > INPUTS {
            return None;
        }
        let result = game.native_recipe_result(&d["result"])?;
        // The recipe's own container, else what the meal leaves when eaten.
        let container = d["container"]["id"]
            .as_str()
            .map(|id| if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") })
            .or_else(|| {
                let info = game.items().get(&result.item)?;
                info.crafting_remainder.clone().or_else(|| info.use_remainder.clone())
            });
        let time = d["cookingtime"].as_i64().unwrap_or(200).max(1) as i32;
        Some(Meal { inputs, result, container, time })
    })
}

/// A sandwich built on the board (Hands-On Delight): bread first, then the fillings in any order;
/// the last one turns the stack into `result`. From the shapeless crafting recipes of mods' foods
/// with bread among their ingredients (Farmer's Delight's hamburger, sandwiches and wrap).
pub(super) struct Sandwich {
    pub(super) base: Ingredient,
    pub(super) fillings: Vec<Ingredient>,
    pub(super) result: ItemStack,
}

thread_local! {
    static SANDWICHES: RefCell<(usize, usize, Rc<Vec<Sandwich>>)> = RefCell::new((0, 0, Rc::default()));
}

pub(super) fn sandwiches(game: &Game<'_>) -> Rc<Vec<Sandwich>> {
    let recipes = &game.recipes().crafting;
    let stamp = (recipes.as_ptr() as usize, recipes.len());
    SANDWICHES.with_borrow_mut(|(at, len, list)| {
        if (*at, *len) != stamp {
            let bread = ItemStack::new("minecraft:bread", 1);
            *list = Rc::new(
                recipes
                    .iter()
                    .filter_map(|(_, recipe)| {
                        let horizon_client::crafting::Recipe::Shapeless { ingredients, result } = recipe else {
                            return None;
                        };
                        let item = result.item.as_deref()?;
                        if item.starts_with("minecraft:") || ingredients.len() < 2 || ingredients.len() > BOARD_LAYERS {
                            return None;
                        }
                        let b = ingredients.iter().position(|i| i.test(&bread))?;
                        let fillings = ingredients.iter().enumerate().filter(|(i, _)| *i != b).map(|(_, g)| g.clone()).collect();
                        Some(Sandwich {
                            base: ingredients[b].clone(),
                            fillings,
                            result: ItemStack::new(item, result.count.max(1)),
                        })
                    })
                    .collect(),
            );
            (*at, *len) = stamp;
        }
        Rc::clone(list)
    })
}

/// Whether the stacks can each take a different one of the ingredients (some may be left over).
pub(super) fn partial(ingredients: &[Ingredient], stacks: &[&ItemStack]) -> bool {
    fn assign(i: usize, ingredients: &[Ingredient], stacks: &[&ItemStack], used: &mut [bool]) -> bool {
        if i == stacks.len() {
            return true;
        }
        for (j, ingredient) in ingredients.iter().enumerate() {
            if !used[j] && ingredient.test(stacks[i]) {
                used[j] = true;
                if assign(i + 1, ingredients, stacks, used) {
                    return true;
                }
                used[j] = false;
            }
        }
        false
    }
    stacks.len() <= ingredients.len() && assign(0, ingredients, stacks, &mut vec![false; ingredients.len()])
}

/// Whether the ingredients take the stacks one each, in any order (a shapeless match).
pub(super) fn matches(ingredients: &[Ingredient], stacks: &[&ItemStack]) -> bool {
    fn assign(i: usize, ingredients: &[Ingredient], stacks: &[&ItemStack], used: &mut [bool]) -> bool {
        if i == stacks.len() {
            return true;
        }
        for (j, ingredient) in ingredients.iter().enumerate() {
            if !used[j] && ingredient.test(stacks[i]) {
                used[j] = true;
                if assign(i + 1, ingredients, stacks, used) {
                    return true;
                }
                used[j] = false;
            }
        }
        false
    }
    ingredients.len() == stacks.len() && assign(0, ingredients, stacks, &mut vec![false; ingredients.len()])
}

/// The container a meal is served in, when it has one.
pub(super) fn meal_container(game: &Game<'_>, meal: &ItemStack) -> Option<String> {
    meals(game).iter().find(|m| m.result.item == meal.item).and_then(|m| m.container.clone())
}

// ---- small helpers ----

pub(super) fn items_at(game: &Game<'_>, pos: [i32; 3]) -> Vec<Option<ItemStack>> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) => m.items.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn set_slot(game: &mut Game<'_>, pos: [i32; 3], slot: usize, stack: Option<ItemStack>) {
    game.native_with_mod_block_entity(pos, |m| {
        if m.items.len() <= slot {
            m.items.resize(slot + 1, None);
        }
        m.items[slot] = stack;
    });
    game.native_block_entity_sync(pos);
}

pub(super) fn one(stack: &ItemStack) -> ItemStack {
    ItemStack { count: 1, ..stack.clone() }
}

/// Takes one from the acting hand (none in creative).
fn use_one_held(game: &mut Game<'_>) {
    if game.creative {
        return;
    }
    let cell = game.held_mut();
    if let Some(s) = cell {
        s.count = s.count.saturating_sub(1);
        if s.count == 0 {
            *cell = None;
        }
    }
    game.guest_inventory_resend = true;
}

pub(super) fn center(pos: [i32; 3], dy: f64) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + dy, f64::from(pos[2]) + 0.5]
}

pub(super) fn shrink(cell: &mut Option<ItemStack>, by: u32) {
    if let Some(s) = cell {
        s.count = s.count.saturating_sub(by);
        if s.count == 0 {
            *cell = None;
        }
    }
}

/// A roll in 0..1 from the game time, the place and a salt.
pub(super) fn roll(game: &Game<'_>, pos: [i32; 3], salt: u64) -> f32 {
    let mut x = (game.world.game_time as u64)
        ^ (pos[0] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (pos[1] as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (pos[2] as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ salt.wrapping_mul(0x27D4_EB2F_1656_67C5);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn int_array(nbt: &Compound, name: &str, len: usize) -> Vec<i32> {
    let mut out = match nbt.get(&key(name)) {
        Some(Tag::IntArray(a)) => a.clone(),
        _ => Vec::new(),
    };
    out.resize(len, 0);
    out
}

fn lit(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_block_property(pos, "lit").as_deref() == Some("true")
}

// ---- right clicks ----

/// A right click on a board, a stove or a pot, by the world's own game (a client of a server
/// leaves it to the server). Sneaking with something in hand places it as usual.
pub(super) fn click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || game.remote.is_some() || !game.world.loaded(pos[0], pos[2]) {
        return;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let Some(name) = game.native_state_block(state).map(str::to_owned) else { return };
    if !ours(&name) {
        return;
    }
    if game.keys.held(Mapping::Sneak) && game.held().is_some() {
        return;
    }
    let key = (pos[0], pos[1], pos[2]);
    if game.world.block_entities.get(&key).is_none()
        && let Some(entity) = game.new_block_entity(&name)
    {
        game.world.block_entities.insert(key, entity);
    }
    match name.as_str() {
        BOARD => board_click(game, pos, canceled),
        STOVE => stove_click(game, pos, canceled),
        SKILLET => skillet_click(game, pos, canceled),
        _ => pot_click(game, pos, canceled),
    }
}

fn board_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let items = items_at(game, pos);
    let top = items.iter().rposition(Option::is_some);
    if let Some(top) = top.filter(|&t| t > 0) {
        // A stack of layers: an empty hand takes the top one back.
        if game.held().is_none() && game.hand == Hand::Main {
            if let Some(Some(item)) = items.get(top).cloned() {
                set_slot(game, pos, top, None);
                game.give_or_drop(item);
                game.guest_inventory_resend = true;
                game.native_play_sound_in_place("farmersdelight:block.cutting_board.remove_item", center(pos, 0.1), BLOCKS, 1.0, 1.0);
                game.swing_arm();
                *canceled = true;
            }
        }
        return;
    }
    let board = items.into_iter().next().flatten();
    let held = game.held().cloned();
    let at = center(pos, 0.1);
    match (board, held) {
        (None, Some(held)) => {
            let tool = game.items().get(&held.item).is_some_and(|i| i.max_damage > 0);
            set_slot(game, pos, 0, Some(one(&held)));
            use_one_held(game);
            let sound = if tool {
                "farmersdelight:block.cutting_board.carve_tool"
            } else {
                "farmersdelight:block.cutting_board.place_item"
            };
            game.native_play_sound_in_place(sound, at, BLOCKS, 1.0, 0.8);
            game.swing_arm();
            *canceled = true;
        }
        (Some(item), Some(tool)) => {
            let cuts = cuts(game);
            let Some(cut) = cuts.iter().find(|c| c.input.test(&item) && c.tool.test(&tool)) else {
                return;
            };
            for (i, (result, chance)) in cut.results.iter().enumerate() {
                if *chance < 1.0 && roll(game, pos, i as u64) >= *chance {
                    continue;
                }
                game.spawn_item(result.clone(), center(pos, 0.25), [0.0, 0.2, 0.0], 10);
            }
            set_slot(game, pos, 0, None);
            if game.items().get(&tool.item).is_some_and(|i| i.max_damage > 0) {
                game.damage_held(1);
                game.guest_inventory_resend = true;
            }
            let sound = cut.sound.as_deref().unwrap_or("farmersdelight:block.cutting_board.knife_cut");
            game.native_play_sound_in_place(sound, at, BLOCKS, 0.8, 1.0);
            game.swing_arm();
            *canceled = true;
        }
        (Some(item), None) if game.hand == Hand::Main => {
            set_slot(game, pos, 0, None);
            game.give_or_drop(item);
            game.guest_inventory_resend = true;
            game.native_play_sound_in_place("farmersdelight:block.cutting_board.remove_item", at, BLOCKS, 1.0, 1.0);
            game.swing_arm();
            *canceled = true;
        }
        _ => {}
    }
}

fn stove_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    // The stove has no menu: whatever happens, its furnace template opens nothing.
    *canceled = true;
    let Some(held) = game.held().cloned() else { return };
    let item = &*held.item;
    let is_lit = lit(game, pos);
    if !is_lit && (item == "minecraft:flint_and_steel" || item == "minecraft:fire_charge") {
        game.native_set_block_property(pos, "lit", "true");
        if item == "minecraft:flint_and_steel" {
            game.native_play_sound_in_place("minecraft:item.flintandsteel.use", center(pos, 0.5), BLOCKS, 1.0, 1.0);
            game.damage_held(1);
        } else {
            game.native_play_sound_in_place("minecraft:item.firecharge.use", center(pos, 0.5), BLOCKS, 1.0, 1.0);
            use_one_held(game);
        }
        game.guest_inventory_resend = true;
        game.swing_arm();
        return;
    }
    if is_lit && game.items().item_in_tag("minecraft:shovels", item) {
        game.native_set_block_property(pos, "lit", "false");
        game.native_play_sound_in_place("minecraft:block.fire.extinguish", center(pos, 0.5), BLOCKS, 0.5, 1.0);
        game.damage_held(1);
        game.guest_inventory_resend = true;
        game.swing_arm();
        return;
    }
    let Some(time) = game.recipes().cook(CAMPFIRE, &held).map(|r| r.time as i32) else { return };
    // Something standing on the stove (a pot, a skillet) covers its top.
    let above = game.world.block(pos[0], pos[1] + 1, pos[2]);
    if !game.res.facts().get(above).name.ends_with(":air") {
        return;
    }
    let items = items_at(game, pos);
    let Some(slot) = (0..STOVE_SLOTS).find(|&i| items.get(i).is_none_or(Option::is_none)) else { return };
    game.native_with_mod_block_entity(pos, |m| {
        if m.items.len() < STOVE_SLOTS {
            m.items.resize(STOVE_SLOTS, None);
        }
        m.items[slot] = Some(one(&held));
        let mut times = int_array(&m.nbt, "CookingTimes", STOVE_SLOTS);
        let mut totals = int_array(&m.nbt, "CookingTotalTimes", STOVE_SLOTS);
        times[slot] = 0;
        totals[slot] = time;
        m.nbt.insert(key("CookingTimes"), Tag::IntArray(times));
        m.nbt.insert(key("CookingTotalTimes"), Tag::IntArray(totals));
    });
    game.native_block_entity_sync(pos);
    use_one_held(game);
    game.native_play_sound_in_place("minecraft:block.wood.place", center(pos, 1.0), BLOCKS, 0.7, 1.4);
    game.swing_arm();
}

fn pot_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    *canceled = true;
    // Serving straight from the pot with the meal's container in hand.
    let items = items_at(game, pos);
    if let (Some(held), Some(Some(meal))) = (game.held().cloned(), items.get(MEAL).cloned())
        && meal_container(game, &meal).is_some_and(|c| *c == *held.item)
    {
        let mut rest = meal.clone();
        rest.count -= 1;
        set_slot(game, pos, MEAL, (rest.count > 0).then_some(rest));
        use_one_held(game);
        game.give_or_drop(one(&meal));
        game.guest_inventory_resend = true;
        game.native_play_sound_in_place("minecraft:item.armor.equip_generic", center(pos, 0.5), BLOCKS, 1.0, 1.0);
        game.swing_arm();
        return;
    }
    // The menu opens on the main hand's turn; an off hand that serves nothing leaves the click.
    if game.hand != Hand::Main {
        *canceled = false;
        return;
    }
    if game.mod_menu_at((pos[0], pos[1], pos[2])).is_some() {
        game.open_menu(pos);
    }
}

/// A right click on a skillet: food that a campfire cooks goes in (up to eight of one kind, while
/// none has started frying); an empty hand takes it all back.
fn skillet_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let held = game.held().cloned();
    match held {
        Some(_) => {
            if skillet_put_from(game, pos, None) > 0 {
                game.swing_arm();
                *canceled = true;
            }
        }
        None if game.hand == Hand::Main => {
            if let Some(Some(food)) = items_at(game, pos).into_iter().next() {
                set_slot(game, pos, 0, None);
                skillet_reset(game, pos);
                game.give_or_drop(food);
                game.guest_inventory_resend = true;
                game.swing_arm();
                *canceled = true;
            }
        }
        None => {}
    }
}

/// Puts what it can of `held` (the acting hand's stack) in the skillet; how many went in.
pub(super) fn skillet_put_from(game: &mut Game<'_>, pos: [i32; 3], slot: Option<usize>) -> u32 {
    let stack = match slot {
        Some(slot) => game.inventory.get(slot).cloned().flatten(),
        None => game.held().cloned(),
    };
    let Some(stack) = stack else { return 0 };
    if game.recipes().cook(CAMPFIRE, &stack).is_none() {
        return 0;
    }
    let items = items_at(game, pos);
    let pan = items.first().cloned().flatten();
    let started = skillet_sides(game, pos).is_some_and(|(a, b, _)| a > 0 || b > 0);
    let room = match &pan {
        None => SKILLET_MAX,
        Some(p) if p.same_item_same_components(&stack) && !started => SKILLET_MAX.saturating_sub(p.count),
        Some(_) => 0,
    };
    let n = room.min(stack.count);
    if n == 0 {
        return 0;
    }
    let mut cell = pan;
    add_to(&mut cell, ItemStack { count: n, ..stack.clone() });
    set_slot(game, pos, 0, cell);
    if !game.creative {
        let source = match slot {
            Some(slot) => game.inventory.get_mut(slot),
            None => Some(game.held_mut()),
        };
        if let Some(source) = source {
            shrink(source, n);
        }
    }
    game.guest_inventory_resend = true;
    game.native_play_sound_in_place("farmersdelight:block.skillet.add_food", center(pos, 0.1), BLOCKS, 0.8, 1.0);
    n
}

fn skillet_sides(game: &Game<'_>, pos: [i32; 3]) -> Option<(i32, i32, bool)> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(m)) => {
            let get = |k: &str| match m.nbt.get(&key(k)) {
                Some(Tag::Int(v)) => *v,
                _ => 0,
            };
            Some((get("SideA"), get("SideB"), get("Flipped") != 0))
        }
        _ => None,
    }
}

pub(super) fn skillet_reset(game: &mut Game<'_>, pos: [i32; 3]) {
    game.native_with_mod_block_entity(pos, |m| {
        for k in ["SideA", "SideB", "Flipped"] {
            m.nbt.insert(key(k), Tag::Int(0));
        }
    });
    game.native_block_entity_sync(pos);
}

/// The skillet tossed (Hands-On Delight): its food turns over in the air, the other side down.
pub(super) fn tossed(game: &mut Game<'_>, pos: [i32; 3]) {
    let now = game.world.game_time as i32;
    let has_food = matches!(items_at(game, pos).first(), Some(Some(_)));
    if !has_food {
        return;
    }
    game.native_with_mod_block_entity(pos, |m| {
        let flipped = matches!(m.nbt.get(&key("Flipped")), Some(Tag::Int(v)) if *v != 0);
        m.nbt.insert(key("Flipped"), Tag::Int(i32::from(!flipped)));
        m.nbt.insert(key("FlipAt"), Tag::Int(now));
    });
    game.native_block_entity_sync(pos);
    game.native_play_sound_in_place("farmersdelight:block.skillet.add_food", center(pos, 0.3), BLOCKS, 0.6, 1.4);
}

fn skillet_tick(game: &mut Game<'_>, pos: [i32; 3], e: &mut ModBlockEntity) {
    if e.items.is_empty() {
        e.items.resize(1, None);
    }
    let time = game.world.game_time;
    if time % 20 == 0 {
        let below = game.world.block(pos[0], pos[1] - 1, pos[2]);
        let name = game.res.facts().get(below).name.clone();
        let tray = game.items().block_in_tag("farmersdelight:tray_heat_sources", &name);
        let want = if tray { "true" } else { "false" };
        if game.native_block_property(pos, "support").as_deref().is_some_and(|v| v != want) {
            game.native_set_block_property(pos, "support", want);
        }
    }
    let get = |e: &ModBlockEntity, k: &str| match e.nbt.get(&key(k)) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let hot = heated(game, pos);
    let Some(food) = e.items[0].clone() else {
        e.nbt.insert(key("Hot"), Tag::Int(i32::from(hot)));
        return;
    };
    // Food a campfire cooks fries a side at a time, each for half the campfire's time.
    let Some((total, result)) = game.recipes().cook(CAMPFIRE, &food).map(|r| (r.time as i32, r.result.clone())) else {
        e.nbt.insert(key("Hot"), Tag::Int(i32::from(hot)));
        return;
    };
    let half = (total / 2).max(1);
    let flipped = get(e, "Flipped") != 0;
    let (mut a, mut b) = (get(e, "SideA"), get(e, "SideB"));
    if hot {
        if flipped {
            b += 1;
        } else {
            a += 1;
        }
        if time % 50 == 0 {
            game.native_play_sound_in_place("farmersdelight:block.skillet.sizzle", center(pos, 0.1), BLOCKS, 0.4, 1.0);
        }
    }
    let (down, up) = if flipped { (b, a) } else { (a, b) };
    let mut flip = flipped;
    // Left alone, the food turns itself two seconds after its side is done.
    if down >= half + 40 && up < half {
        flip = !flipped;
        e.nbt.insert(key("FlipAt"), Tag::Int(time as i32));
    }
    if a >= half && b >= half {
        e.items[0] = Some(ItemStack { count: (food.count * result.count.max(1)).min(64), ..result });
        a = 0;
        b = 0;
        flip = false;
        game.native_play_sound_in_place("farmersdelight:block.skillet.add_food", center(pos, 0.1), BLOCKS, 0.7, 0.8);
    }
    e.nbt.insert(key("SideA"), Tag::Int(a));
    e.nbt.insert(key("SideB"), Tag::Int(b));
    e.nbt.insert(key("Flipped"), Tag::Int(i32::from(flip)));
    e.nbt.insert(key("Total"), Tag::Int(total));
    e.nbt.insert(key("Hot"), Tag::Int(i32::from(hot)));
    // Clients see the food, the sides in quarters, and the turns.
    let quarter = |v: i32| (v * 4 / half).min(4);
    let sig = (quarter(a) << 8) ^ (quarter(b) << 12) ^ (i32::from(flip) << 16) ^ (i32::from(hot) << 17) ^ (food.count as i32) ^ ((food.item.len() as i32) << 20);
    if get(e, "Sig") != sig || e.items[0].as_ref().is_some_and(|i| i.item != food.item) {
        e.nbt.insert(key("Sig"), Tag::Int(sig));
        game.native_block_entity_sync(pos);
    }
}

/// A placed stove starts lit; a pot sets its tray over a fire.
pub(super) fn placed(game: &mut Game<'_>, pos: [i32; 3], state: BlockStateId) {
    if game.remote.is_some() {
        return;
    }
    match game.native_state_block(state).map(str::to_owned).as_deref() {
        Some(STOVE) => {
            game.native_set_block_property(pos, "lit", "true");
        }
        Some(POT) => update_support(game, pos),
        _ => {}
    }
}

fn update_support(game: &mut Game<'_>, pos: [i32; 3]) {
    let below = game.world.block(pos[0], pos[1] - 1, pos[2]);
    let name = game.res.facts().get(below).name.clone();
    let want = if game.items().block_in_tag("farmersdelight:tray_heat_sources", &name) { "tray" } else { "none" };
    if game.native_block_property(pos, "support").as_deref().is_some_and(|v| v != want) {
        game.native_set_block_property(pos, "support", want);
    }
}

// ---- ticks ----

pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if game.remote.is_some() {
        return;
    }
    match entity.id.as_str() {
        STOVE => stove_tick(game, pos, entity),
        POT => pot_tick(game, pos, entity),
        SKILLET => skillet_tick(game, pos, entity),
        _ => {}
    }
}

fn stove_tick(game: &mut Game<'_>, pos: [i32; 3], e: &mut ModBlockEntity) {
    if e.items.iter().all(Option::is_none) {
        return;
    }
    if e.items.len() < STOVE_SLOTS {
        e.items.resize(STOVE_SLOTS, None);
    }
    let is_lit = lit(game, pos);
    let mut times = int_array(&e.nbt, "CookingTimes", STOVE_SLOTS);
    let totals = int_array(&e.nbt, "CookingTotalTimes", STOVE_SLOTS);
    let mut done = false;
    for i in 0..STOVE_SLOTS {
        let Some(item) = e.items[i].clone() else { continue };
        if !is_lit {
            // CampfireBlockEntity.cooldownTick: an unlit stove's food cools back.
            times[i] = (times[i] - 2).max(0);
            continue;
        }
        times[i] += 1;
        if times[i] >= totals[i].max(1) {
            let result = game
                .recipes()
                .cook(CAMPFIRE, &item)
                .map_or_else(|| item.clone(), |r| r.result.clone());
            game.spawn_item(result, center(pos, 1.05), [0.0, 0.25, 0.0], 10);
            e.items[i] = None;
            times[i] = 0;
            done = true;
        }
    }
    e.nbt.insert(key("CookingTimes"), Tag::IntArray(times));
    if done {
        game.native_block_entity_sync(pos);
    }
}

fn heated(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let state = game.world.block(below[0], below[1], below[2]);
    let name = &game.res.facts().get(state).name;
    game.items().block_in_tag("farmersdelight:heat_sources", name)
        && game.native_block_property(below, "lit").is_none_or(|v| v == "true")
}

pub(super) fn room_for(game: &Game<'_>, cell: &Option<ItemStack>, stack: &ItemStack) -> bool {
    match cell {
        None => true,
        Some(s) => {
            let max = game.items().get(&s.item).map_or(64, |i| i.max_stack);
            s.same_item_same_components(stack) && s.count + stack.count <= max
        }
    }
}

pub(super) fn add_to(cell: &mut Option<ItemStack>, stack: ItemStack) {
    match cell {
        Some(s) => s.count += stack.count,
        None => *cell = Some(stack),
    }
}

fn pot_tick(game: &mut Game<'_>, pos: [i32; 3], e: &mut ModBlockEntity) {
    if e.items.len() < 9 {
        e.items.resize(9, None);
    }
    if game.world.game_time % 20 == 0 {
        update_support(game, pos);
    }
    let hot = heated(game, pos);
    let mut cook = match e.nbt.get(&key("CookTime")) {
        Some(Tag::Int(t)) => *t,
        _ => 0,
    };
    let mut total = 0;
    let stacks: Vec<&ItemStack> = e.items[..INPUTS].iter().flatten().collect();
    let meals = meals(game);
    let found = if stacks.is_empty() { None } else { meals.iter().find(|m| matches(&m.inputs, &stacks)) };
    match found {
        Some(meal) if hot && room_for(game, &e.items[MEAL], &meal.result) => {
            total = meal.time;
            // Hands-On Delight's stirring hurries the pot along.
            let boost = match e.nbt.get(&key("Boost")) {
                Some(Tag::Int(b)) => *b,
                _ => 0,
            };
            let hurry = boost.min(3);
            e.nbt.insert(key("Boost"), Tag::Int(boost - hurry));
            cook += 1 + hurry;
            if cook % 60 == 1 {
                game.native_play_sound_in_place("farmersdelight:block.cooking_pot.boil_soup", center(pos, 0.5), BLOCKS, 0.5, 1.0);
            }
            if cook >= total {
                cook = 0;
                for slot in 0..INPUTS {
                    let Some(item) = e.items[slot].clone() else { continue };
                    let left = game
                        .items()
                        .get(&item.item)
                        .and_then(|i| i.remainder.clone().or_else(|| i.crafting_remainder.clone()));
                    shrink(&mut e.items[slot], 1);
                    if let Some(left) = left {
                        game.spawn_item(ItemStack::new(&left, 1), center(pos, 0.7), [0.0, 0.2, 0.0], 10);
                    }
                }
                // Each star of steady stirring adds a quarter more servings.
                let stars = match e.nbt.get(&key("Stars")) {
                    Some(Tag::Int(s)) => (*s).clamp(0, 3),
                    _ => 0,
                };
                let mut result = meal.result.clone();
                let extra = result.count as f32 * 0.25 * stars as f32;
                let whole = extra.floor();
                let max = game.items().get(&result.item).map_or(64, |i| i.max_stack);
                result.count += whole as u32 + u32::from(roll(game, pos, 99) < extra - whole);
                result.count = result.count.min(max);
                if room_for(game, &e.items[MEAL], &result) {
                    add_to(&mut e.items[MEAL], result);
                } else {
                    add_to(&mut e.items[MEAL], meal.result.clone());
                }
                e.nbt.insert(key("Stars"), Tag::Int(0));
                e.nbt.insert(key("Steady"), Tag::Int(0));
            }
        }
        Some(meal) => {
            total = meal.time;
            // Off the heat, the cooking stops where it got to.
        }
        None => cook = 0,
    }
    // Serving: the meal moves to the output once its container is in place (or at once, for a meal
    // with none), taking one container each.
    if let Some(meal) = e.items[MEAL].clone() {
        let served = one(&meal);
        let container = meal_container(game, &meal);
        let has = match &container {
            None => true,
            Some(c) => e.items[CONTAINER].as_ref().is_some_and(|s| *s.item == **c),
        };
        if has && room_for(game, &e.items[OUTPUT], &served) {
            shrink(&mut e.items[MEAL], 1);
            if container.is_some() {
                shrink(&mut e.items[CONTAINER], 1);
            }
            add_to(&mut e.items[OUTPUT], served);
        }
    }
    e.nbt.insert(key("CookTime"), Tag::Int(cook));
    e.data = vec![cook, total, i32::from(hot)];
    // How far the meal has come, in fifths, and whether the pot is over heat: what clients draw.
    let stage = if total > 0 { (cook * 5 / total).clamp(0, 4) } else { -1 };
    e.nbt.insert(key("Stage"), Tag::Int(stage));
    e.nbt.insert(key("Hot"), Tag::Int(i32::from(hot)));
    // Clients see the pot's contents (Hands-On Delight draws them): sent again when they change.
    let sig = pot_signature(e);
    if !matches!(e.nbt.get(&key("Sig")), Some(Tag::Int(s)) if *s == sig) {
        e.nbt.insert(key("Sig"), Tag::Int(sig));
        game.native_block_entity_sync(pos);
    }
}

fn pot_signature(e: &ModBlockEntity) -> i32 {
    let mut h: u32 = 2166136261;
    let mut mix = |b: &[u8]| {
        for x in b {
            h = (h ^ u32::from(*x)).wrapping_mul(16777619);
        }
    };
    for s in e.items.iter().take(MEAL + 1) {
        match s {
            Some(s) => {
                mix(s.item.as_bytes());
                mix(&s.count.to_le_bytes());
            }
            None => mix(&[0]),
        }
    }
    for k in ["Stars", "Stage", "Hot", "StirAt"] {
        if let Some(Tag::Int(v)) = e.nbt.get(&key(k)) {
            mix(&v.to_le_bytes());
        }
    }
    h as i32
}

/// A round of stirring (Hands-On Delight), `quality` 0 to 100: the pot cooks faster, and steady
/// wide rounds fill its stars (one for every three, up to three).
pub(super) fn stirred(game: &mut Game<'_>, pos: [i32; 3], quality: u8) {
    // When it was last stirred, in seconds of game time: other players see the spoon go round.
    let now = (game.world.game_time / 20) as i32;
    game.native_with_mod_block_entity(pos, |m| {
        m.nbt.insert(key("StirAt"), Tag::Int(now));
        let get = |m: &ModBlockEntity, k: &str| match m.nbt.get(&key(k)) {
            Some(Tag::Int(v)) => *v,
            _ => 0,
        };
        let boost = (get(m, "Boost") + 12).min(200);
        let steady = get(m, "Steady") + i32::from(quality >= 60);
        m.nbt.insert(key("Boost"), Tag::Int(boost));
        m.nbt.insert(key("Steady"), Tag::Int(steady));
        m.nbt.insert(key("Stars"), Tag::Int((steady / 3).min(3)));
    });
}

// ---- clients ----

/// What a client draws: the board's and the stove's items.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if tag.is_some() || !(entity.id == BOARD || entity.id == STOVE || entity.id == POT || entity.id == SKILLET) {
        return;
    }
    let mut c = Compound::default();
    let shown = if entity.id == POT { &entity.items[..entity.items.len().min(MEAL + 1)] } else { &entity.items[..] };
    c.insert(key("Items"), horizon_items::item_nbt::encode_items(shown));
    if entity.id == SKILLET {
        for k in ["SideA", "SideB", "Flipped", "FlipAt", "Total", "Hot"] {
            let v = match entity.nbt.get(&key(k)) {
                Some(Tag::Int(v)) => *v,
                _ => 0,
            };
            c.insert(key(k), Tag::Int(v));
        }
    }
    if entity.id == POT {
        for k in ["Stars", "Stage", "Hot", "StirAt"] {
            let v = match entity.nbt.get(&key(k)) {
                Some(Tag::Int(v)) => *v,
                _ => 0,
            };
            c.insert(key(k), Tag::Int(v));
        }
    }
    *tag = Some(c);
}

/// The pot's spoon, a model of its own so that it can move (its block model is an open pot).
pub(super) const SPOON_MODEL: &str = "farmersdelight:block/cooking_pot_spoon";

/// A pot whose spoon another mod draws this frame (Hands-On Delight's stirring): set in its own
/// `native_block_entity_submit`, which runs before this one.
pub(super) static SPOON_AWAY: std::sync::Mutex<Option<[i32; 3]>> = std::sync::Mutex::new(None);

pub(super) fn standalone_models(models: &mut Vec<String>) {
    models.push(SPOON_MODEL.to_owned());
}

/// `facing`'s turn of the block model (north unturned), about +y.
pub(super) fn facing_turn(game: &Game<'_>, state: BlockStateId) -> f32 {
    match game.native_block_state_property(state, "facing") {
        Some("east") => -90.0,
        Some("south") => 180.0,
        Some("west") => 90.0,
        _ => 0.0,
    }
}

pub(super) fn submit(game: &Game<'_>, pos: [i32; 3], block: &str, state: BlockStateId, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    if block == POT {
        let away = SPOON_AWAY.lock().map(|mut a| a.take() == Some(pos)).unwrap_or(false);
        if !away {
            // At rest, leaning against the rim as the closed pot's model has it.
            draw.push();
            draw.rotate_about([0.0, 1.0, 0.0], facing_turn(game, state), [0.5, 0.5, 0.5]);
            draw.rotate_about([0.0, 0.0, 1.0], -22.5, [0.5, 0.1875, 0.5]);
            game.native_draw_model(draw, SPOON_MODEL, 0xFF_FFFF);
            draw.pop();
        }
        return;
    }
    if block == SKILLET {
        skillet_draw(game, entity, draw);
        return;
    }
    let count = match block {
        BOARD => 1,
        STOVE => STOVE_SLOTS,
        _ => return,
    };
    let items = horizon_items::item_nbt::decode_items(entity.nbt.get(&key("Items")), count);
    if items.iter().all(Option::is_none) {
        return;
    }
    // The block model turns with `facing` (north unturned), and so do its items.
    let turn = facing_turn(game, state);
    let seed = pos[0].wrapping_mul(31).wrapping_add(pos[1].wrapping_mul(17)).wrapping_add(pos[2]);
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], turn, [0.5, 0.5, 0.5]);
    for (i, stack) in items.iter().enumerate() {
        let Some(stack) = stack else { continue };
        draw.push();
        if block == BOARD {
            let is_block = game.items().get(&stack.item).is_some_and(|i| i.block.is_some());
            if is_block && !game.items().item_in_tag("farmersdelight:flat_on_cutting_board", &stack.item) {
                // A block stands on the board, half size.
                draw.translate([0.5, 0.0625 + 0.25, 0.5]);
                draw.scale([0.5, 0.5, 0.5]);
            } else {
                draw.translate([0.5, 0.0625 + 0.016, 0.5]);
                draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
                draw.scale([0.6, 0.6, 0.6]);
            }
        } else {
            let [x, z] = STOVE_SPOTS[i];
            draw.translate([x, 1.016, z]);
            draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
            draw.scale([0.3, 0.3, 0.3]);
        }
        game.native_draw_item(draw, stack, DisplayContext::Fixed, None, seed.wrapping_add(i as i32));
        draw.pop();
    }
    draw.pop();
}

/// The skillet's food lying in the pan, a few of the stack shown; tossed, it turns over in the air.
fn skillet_draw(game: &Game<'_>, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let items = horizon_items::item_nbt::decode_items(entity.nbt.get(&key("Items")), 1);
    let Some(Some(food)) = items.first() else { return };
    let flip_at = match entity.nbt.get(&key("FlipAt")) {
        Some(Tag::Int(v)) => *v,
        _ => i32::MIN,
    };
    let since = (draw.game_time as i32).wrapping_sub(flip_at) as f32 + draw.partial;
    let toss = (0.0..12.0).contains(&since).then(|| since / 12.0);
    const SPOTS: [[f32; 2]; 4] = [[0.36, 0.38], [0.64, 0.42], [0.4, 0.64], [0.62, 0.66]];
    let shown = (food.count as usize).clamp(1, 4);
    for (i, [x, z]) in SPOTS.iter().take(shown).enumerate() {
        draw.push();
        let (lift, turn) = match toss {
            Some(t) => (0.6 * 4.0 * t * (1.0 - t), 180.0 * t),
            None => (0.0, 0.0),
        };
        // A little higher each, so pieces that overlap don't flicker through each other.
        draw.translate([*x, 0.0625 + 0.012 + i as f32 * 0.008 + lift, *z]);
        draw.rotate_about([0.0, 1.0, 0.0], 25.0 + i as f32 * 70.0, [0.0, 0.0, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], 90.0 + turn, [0.0, 0.0, 0.0]);
        draw.scale([0.42, 0.42, 0.42]);
        game.native_draw_item(draw, food, DisplayContext::Fixed, None, 40 + i as i32);
        draw.pop();
    }
}

pub(super) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    if block != POT || screen.is_some() {
        return;
    }
    let at = |x: f32, y: f32| Some([x, y]);
    *screen = Some(NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![
            at(30.0, 17.0),
            at(48.0, 17.0),
            at(66.0, 17.0),
            at(30.0, 35.0),
            at(48.0, 35.0),
            at(66.0, 35.0),
            at(124.0, 26.0),
            at(92.0, 55.0),
            at(124.0, 55.0),
        ],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: true,
    });
}

pub(super) fn screen_draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block != POT {
        return;
    }
    let [x, y] = origin;
    game.native_sprite(batch, POT_GUI, [0.0, 0.0, 176.0, 166.0], [x, y, 176.0, 166.0]);
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    if get(2) != 0 {
        game.native_sprite(batch, POT_GUI, [176.0, 0.0, 17.0, 15.0], [x + 47.0, y + 55.0, 17.0, 15.0]);
    }
    let (cook, total) = (get(0), get(1));
    if cook > 0 && total > 0 {
        let w = (24.0 * cook as f32 / total as f32).clamp(0.0, 24.0).floor();
        game.native_sprite(batch, POT_GUI, [176.0, 15.0, w, 17.0], [x + 89.0, y + 25.0, w, 17.0]);
    }
}

// ---- effects ----

/// Nourishment and Comfort, on each player's server tick.
pub(super) fn player_tick(game: &mut Game<'_>) {
    if game.remote.is_some() {
        return;
    }
    let nourished = game.potions.effects.get("farmersdelight:nourishment").is_some();
    let comforted = game.potions.effects.get("farmersdelight:comfort").is_some();
    if !nourished && !comforted {
        return;
    }
    let time = game.world.game_time;
    let Some(p) = game.player.as_mut() else { return };
    if nourished {
        p.health.food.exhaustion = 0.0;
    }
    if comforted && time % 80 == 0 && p.health.food.saturation <= 0.0 {
        let health = p.health.health();
        if let Some(uuid) = game.native_acting_player().map(|a| a.uuid) {
            game.native_set_player_status(uuid, Some(health + 1.0), None, None);
        }
    }
}
