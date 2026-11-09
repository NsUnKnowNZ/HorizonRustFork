fn fog_render_fog(
    game: &crate::game::Game<'_>,
    camera: [f64; 3],
    fog_type: &horizon_client::fog::FogType,
    fog: &mut horizon_client::fog::FogData,
) {
    crate::game::fog_looks::render(game, camera, fog_type, fog);
}
