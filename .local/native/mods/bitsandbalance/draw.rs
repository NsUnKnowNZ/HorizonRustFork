fn bitsandbalance_draw(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    if layer == "minecraft:hotbar" {
        crate::game::bitsandbalance::draw(game, batch);
    }
}
