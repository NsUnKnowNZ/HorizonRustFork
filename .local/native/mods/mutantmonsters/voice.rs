fn mutantmonsters_voice(m: &crate::mobs::Mob, sounds: &mut [&'static str; 3]) {
    crate::mutantmonsters_ai::voice(m, sounds);
}
