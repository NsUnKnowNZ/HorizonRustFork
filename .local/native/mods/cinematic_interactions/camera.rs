// The station's camera while a scene is on.
fn cinematic_interactions_camera(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    let _ = game;
    match crate::game::cinematic_interactions::camera() {
        Some(([x, y, z], yaw, pitch)) => core::ops::ControlFlow::Break(Some((glam::DVec3::new(x, y, z), yaw, pitch))),
        None => core::ops::ControlFlow::Continue(()),
    }
}
