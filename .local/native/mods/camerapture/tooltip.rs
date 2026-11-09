fn camerapture_tooltip(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    crate::game::camerapture::tooltip(game, stack, lines);
}
