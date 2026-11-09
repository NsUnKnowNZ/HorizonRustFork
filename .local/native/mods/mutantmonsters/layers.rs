fn mutantmonsters_layers(layers: &mut Vec<(String, horizon_client::mobs::model_layers::LayerDefinition)>) {
    horizon_client::mod_api::set_native_listening();
    crate::game::mutantmonsters_models::layers(layers);
}
