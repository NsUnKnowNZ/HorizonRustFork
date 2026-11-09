fn camerapture_component_types(types: &mut Vec<String>) {
    crate::game::camerapture::component_types(types);
    horizon_client::mod_api::set_native_listening();
}
