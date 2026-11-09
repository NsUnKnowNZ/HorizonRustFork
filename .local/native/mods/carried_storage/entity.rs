fn carried_storage_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::carried_storage::entity(block, entity);
}
