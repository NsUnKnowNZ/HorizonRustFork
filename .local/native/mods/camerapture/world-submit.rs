fn camerapture_world_submit(game: &crate::game::Game<'_>, draw: &mut crate::game::native_world_draw::NativeWorldDraw) {
    crate::game::camerapture_frames::submit(game, draw);
}
