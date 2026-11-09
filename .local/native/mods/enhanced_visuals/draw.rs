fn enhanced_visuals_draw(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    if layer == "minecraft:hotbar" {
        crate::game::enhanced_visuals::draw(game, batch);
    }
}
