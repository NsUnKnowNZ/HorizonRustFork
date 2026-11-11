fn mob_ai_tweaks_died(game: &mut crate::game::Game<'_>, entity: u32, kind: &str, damage_type: &str) {
    crate::game::mob_ai_tweaks::died(game, entity, kind, damage_type);
}
