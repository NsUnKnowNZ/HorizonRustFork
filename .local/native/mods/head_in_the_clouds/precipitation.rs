fn head_in_the_clouds_precipitation(pos: BlockPos) -> core::ops::ControlFlow<BiomePrecipitation> {
    if head_in_the_clouds::above_clouds(pos.y) {
        core::ops::ControlFlow::Break(BiomePrecipitation::None)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
