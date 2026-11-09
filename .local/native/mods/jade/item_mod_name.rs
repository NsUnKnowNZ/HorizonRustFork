fn jade_item_mod_name(game: &Game<'_>, stack: &ItemStack, lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
    crate::game::jade::item_tooltip(game, stack, lines)
}
