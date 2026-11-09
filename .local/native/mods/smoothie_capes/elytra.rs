fn smoothie_player_elytra_geometry(
    game: &crate::game::Game<'_>,
    m: &Mob,
    chest: Option<&str>,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> std::ops::ControlFlow<()> {
    if chest.is_none() || !crate::game::smoothie_elytra_config::enabled(game) {
        return std::ops::ControlFlow::Continue(());
    }
    let stack = m.extra.armor[1]
        .as_ref()
        .filter(|s| chest == Some(&*s.item));
    let mut angles = m.extra.elytra.angles(partial);
    let flight_ticks = m.extra.fall_fly_ticks as f32 + partial;
    let flight = (flight_ticks / 10.0).clamp(0.0, 1.0);
    let time = m.age as f32 + partial;
    let (flex, twist) = (time * 0.34).sin_cos();
    let amplitude = if m.extra.crouching {
        0.012
    } else {
        0.025 + 0.040 * flight
    };
    angles[0] += flex * amplitude;
    angles[1] += twist * amplitude * 0.45;
    angles[2] += (flex * 0.65 + twist * 0.35) * amplitude;
    game.wings_geometry(
        chest,
        stack,
        m.cape.as_deref(),
        false,
        horizon_client::mobs::render::base_transform(m, partial, camera),
        angles,
        m.extra.crouching,
        light,
        game.wings_overlay(stack),
        out,
    );
    std::ops::ControlFlow::Break(())
}
