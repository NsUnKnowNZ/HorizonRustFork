fn mutantmonsters_step(m: &crate::mobs::Mob, sound: &mut Option<(&'static str, f32, f32)>) {
    crate::mutantmonsters_ai::step_sound(m, sound);
}
