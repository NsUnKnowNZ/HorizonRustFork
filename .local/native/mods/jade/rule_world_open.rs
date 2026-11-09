fn jade_rule_world_open(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::WorldPhase) {
    if phase == horizon_client::mod_api::WorldPhase::WorldOpenReady {
        crate::game::jade::rule_world_open(game);
    }
}
