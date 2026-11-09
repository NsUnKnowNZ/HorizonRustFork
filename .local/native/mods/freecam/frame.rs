// Once a frame: the camera flies while the world is played, and holds when
// a screen opens (its key clearings are honored through the runtime).
fn freecam_frame(game: &crate::game::Game<'_>, elapsed: std::time::Duration) {
    use crate::game::freecam as freecam;
    freecam::frame(elapsed, game.native_playing());
}
