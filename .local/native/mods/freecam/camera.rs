// The view the frame renders from: the freecam's while it holds the camera,
// the game's own otherwise.
fn freecam_camera(
    game: &crate::game::Game<'_>,
) -> core::ops::ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    use crate::game::freecam as freecam;
    if !game.native_on(freecam::ID) {
        return core::ops::ControlFlow::Continue(());
    }
    match freecam::view() {
        Some(([x, y, z], yaw, pitch)) => {
            core::ops::ControlFlow::Break(Some((glam::DVec3::new(x, y, z), yaw, pitch)))
        }
        None => core::ops::ControlFlow::Continue(()),
    }
}
