fn simple_death_drops(
    game: &mut crate::game::Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    if context.table.is_empty()
        && context.entity.is_some_and(|(_, kind)| kind == "minecraft:player")
        && let Some(uuid) = context.player
    {
        crate::game::simple_death_game::player_drops(game, uuid, drops);
    }
}
