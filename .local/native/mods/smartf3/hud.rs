// Game::hud HEAD (pinned): while F3 is open, SmartF3 builds the HUD with its
// own debug screen in place of the vanilla one; otherwise (F3 closed, the mod
// off in Options > Mods, set to vanilla F3, or its file broken) the game's own
// `hud` runs. `HudBatch` is spelled as the target spells its result.
fn smartf3_hud(
    game: &mut crate::game::Game<'_>,
    size: [u32; 2],
) -> core::ops::ControlFlow<HudBatch> {
    match game.smartf3_hud(size) {
        Some(batch) => core::ops::ControlFlow::Break(batch),
        None => core::ops::ControlFlow::Continue(()),
    }
}
