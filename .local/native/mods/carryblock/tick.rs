// native_hooks::Game::native_tick (a hook point): carriers' slowness, HUD and
// action bar, and a dead carrier's load left where they died; on the window,
// Options > Mods' on/off for single player's server thread.
fn carryblock_tick(game: &mut crate::game::Game<'_>) {
    game.carryblock_tick();
}
