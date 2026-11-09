// Every item tooltip the game shows, the panel's and the slots' included:
// what makes the item, what uses it, and whether it is bookmarked.
fn jei_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::jei_glue::tooltip(game, stack, lines);
}
