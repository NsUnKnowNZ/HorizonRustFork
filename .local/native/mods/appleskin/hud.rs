fn appleskin_hud(game: &crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    if layer == "minecraft:player_health" {
        game.appleskin_bar(batch);
    }
}
