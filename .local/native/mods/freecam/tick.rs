// Once a tick: exits on damage or a closed world (the player's view kind
// comes back), turns off cleanly when the mod is disabled mid-flight, and
// drives the freeze gate the motion tick reads.
fn freecam_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::freecam as freecam;
    // Turned off in Options > Mods: the camera returns, silently, and the
    // freeze gate clears (this runs even while the world is gone).
    if !game.native_on(freecam::ID) {
        if let Some(freecam::Command::Exit) = freecam::disable() {
            game.camera_type = match freecam::camera_kind() {
                1 => crate::game::perspective::CameraType::ThirdPersonBack,
                2 => crate::game::perspective::CameraType::ThirdPersonFront,
                _ => crate::game::perspective::CameraType::FirstPerson,
            };
        }
        horizon_client::motion::freecam_gate::set_freeze(false);
        return;
    }
    let world_open = game.player.is_some();
    let health = game.player.as_ref().map(|p| p.health.health());
    if let Some(freecam::Command::Exit) = freecam::tick(world_open, health) {
        game.camera_type = match freecam::camera_kind() {
            1 => crate::game::perspective::CameraType::ThirdPersonBack,
            2 => crate::game::perspective::CameraType::ThirdPersonFront,
            _ => crate::game::perspective::CameraType::FirstPerson,
        };
    }
    // The player freezes inside the motion tick, which runs after this.
    horizon_client::motion::freecam_gate::set_freeze(freecam::freezes_player());
}
