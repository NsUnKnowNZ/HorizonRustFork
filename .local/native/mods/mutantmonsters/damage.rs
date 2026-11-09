fn mutantmonsters_damage(victim: u32, kind: &str, attacker: Option<u32>, damage_type: &str, amount: &mut f32, cancel: &mut bool) {
    crate::game::mutantmonsters::incoming_damage(victim, kind, attacker, damage_type, amount, cancel);
}
