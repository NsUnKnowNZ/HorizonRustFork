fn camerapture_hud(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::camerapture_client::hud(game, batch);
}
