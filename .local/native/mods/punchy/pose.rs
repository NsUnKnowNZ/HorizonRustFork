fn punchy_pose(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
    swing: f32,
    x: &mut f32,
    y: &mut f32,
    z: &mut f32,
    pitch: &mut f32,
    roll: &mut f32,
) {
    crate::game::punchy::pose(game, hand, swing, x, y, z, pitch, roll);
}
