fn reacharound_hud(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    if layer == "minecraft:crosshair" {
        crate::game::reacharound::hud(game, batch);
    }
}
