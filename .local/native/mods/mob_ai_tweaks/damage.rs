fn mob_ai_tweaks_damage(entity: i32, kind: &str, damage_type: &str, causing: i32, amount: &mut f32, canceled: &mut bool) {
    crate::game::mob_ai_tweaks::damage(entity, kind, damage_type, causing, amount, canceled);
}
