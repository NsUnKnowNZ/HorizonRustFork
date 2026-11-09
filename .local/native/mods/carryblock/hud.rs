// native_hooks::Game::native_hud (a hook point): the box saying what the
// window's player carries.
fn carryblock_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    game.carryblock_hud(batch, corners);
}
