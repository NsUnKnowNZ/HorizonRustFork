// A key went down or up anywhere: the mod's two keys and the fly keys, as
// the WebAssembly Freecam kept them.
fn freecam_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::freecam as freecam;
    // Turned off in Options > Mods: nothing until the next tick clears up.
    if !game.native_on(freecam::ID) {
        return;
    }
    let toggle_key = game.native_mod_key(freecam::ID, freecam::TOGGLE, freecam::DEFAULT_TOGGLE_KEY);
    let reset_key =
        game.native_mod_key(freecam::ID, freecam::TRIPOD_RESET, freecam::DEFAULT_RESET_KEY);
    let playing = game.native_playing();
    let player = game.player.as_ref().map(|p| freecam::Player {
        pos: [p.position.x, p.position.y, p.position.z],
        eye_height: p.dimensions.eye_height,
        yaw: game.yaw,
        pitch: game.pitch,
    });
    let command = {
        let game = &*game;
        freecam::key(playing, key, pressed, &toggle_key, &reset_key, player, |name| {
            game.native_setting(freecam::ID, name)
        })
    };
    match command {
        // The camera detaches: remember the player's view kind, show the
        // body from behind (the hand hides with it), and drop the fly keys
        // so the player stands still instead of walking on.
        Some(freecam::Command::Enter) => {
            freecam::remember_camera(match game.camera_type {
                crate::game::perspective::CameraType::ThirdPersonBack => 1,
                crate::game::perspective::CameraType::ThirdPersonFront => 2,
                crate::game::perspective::CameraType::FirstPerson => 0,
            });
            game.camera_type = crate::game::perspective::CameraType::ThirdPersonBack;
            for k in [
                winit::keyboard::KeyCode::KeyW,
                winit::keyboard::KeyCode::KeyS,
                winit::keyboard::KeyCode::KeyA,
                winit::keyboard::KeyCode::KeyD,
                winit::keyboard::KeyCode::Space,
            ] {
                game.keys.remove(&k);
            }
        }
        // The camera returns: the player's own view kind comes back.
        Some(freecam::Command::Exit) => {
            game.camera_type = match freecam::camera_kind() {
                1 => crate::game::perspective::CameraType::ThirdPersonBack,
                2 => crate::game::perspective::CameraType::ThirdPersonFront,
                _ => crate::game::perspective::CameraType::FirstPerson,
            };
        }
        None => {}
    }
}
