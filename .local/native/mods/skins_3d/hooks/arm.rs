fn skins3d_arm(game: &crate::game::Game<'_>, left: bool, matrix: glam::Mat4, light: u32, out: &mut Vec<horizon_client::mesher::Vertex>) -> std::ops::ControlFlow<()> {
    crate::game::umbx_skins3d_glue::arm(game, left, matrix, light, out)
}
