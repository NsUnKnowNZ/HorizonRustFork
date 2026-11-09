fn fancy_health_bar_draw(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    if layer == "minecraft:player_health" {
        crate::game::fancy_health_bar::draw(game, batch);
    }
}
