fn xaeroworldmap_disconnecting(game: &crate::game::Game<'_>) {
    let _ = game;
    crate::game::xaeroworldmap::save();
}
