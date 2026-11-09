// Storage for carried furniture and containers: a block entity of the block's own with item
// slots and a vanilla slot-grid menu.
use super::{Game, Mapping};
use horizon_client::containers::ModContainer;

/// The slots and the menu of a storage block, if it is one.
fn storage(block: &str) -> Option<(usize, &'static str)> {
    let (ns, path) = block.split_once(':')?;
    let ends = |list: &[&str]| list.iter().any(|s| path == s.trim_start_matches('_') || path.ends_with(s));
    match ns {
        "handcrafted"
            if ends(&["_counter", "_cupboard", "_drawer", "_desk", "_nightstand", "_side_table", "_shelf"]) =>
        {
            Some((27, "minecraft:generic_9x3"))
        }
        "charm" if ends(&["_chest", "_barrel"]) => Some((27, "minecraft:generic_9x3")),
        "supplementaries" if path == "safe" => Some((27, "minecraft:generic_9x3")),
        "supplementaries" if path == "sack" => Some((9, "minecraft:generic_3x3")),
        "suppsquared" if path.starts_with("sack_") => Some((9, "minecraft:generic_3x3")),
        _ => None,
    }
}

/// Blocks whose decorative template does not open a menu on use: the right click opens it.
fn opens_by_click(block: &str) -> bool {
    block == "supplementaries:safe" || block == "supplementaries:sack" || block.starts_with("suppsquared:sack_")
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    if entity.is_none() && storage(block).is_some() {
        *entity = Some((block.to_owned(), horizon_world::nbt::Compound::default()));
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let Some((slots, menu)) = storage(block) else { return };
    let all: Vec<usize> = (0..slots).collect();
    *container = Some(ModContainer {
        slots,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: all.clone(),
        outputs: all,
        menu: Some(menu.to_owned()),
    });
}

/// A safe or a sack opens on a right click, unless sneaking with something to place. The world's
/// own game does it (single player's server thread, a LAN or dedicated host, with the acting
/// player's state in place); a client of a server leaves the click to the server, which sends the
/// gate back as it was and opens the menu.
pub(super) fn click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || game.remote.is_some() || !game.world.loaded(pos[0], pos[2]) {
        return;
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let Some(name) = game.native_state_block(state).map(str::to_owned) else { return };
    if game.keys.held(Mapping::Sneak) && game.stack_in(game.selected).is_some() {
        return;
    }
    if !opens_by_click(&name) || game.mod_menu_at((pos[0], pos[1], pos[2])).is_none() {
        return;
    }
    let key = (pos[0], pos[1], pos[2]);
    if game.world.block_entities.get(&key).is_none()
        && let Some(entity) = game.new_block_entity(&name)
    {
        game.world.block_entities.insert(key, entity);
    }
    *canceled = true;
    game.open_menu(pos);
}
