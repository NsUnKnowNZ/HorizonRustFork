fn reacharound_use_block(world: &mut RemoteUseWorld<'_, '_>) -> core::ops::ControlFlow<bool> {
    let game = &mut *world.0;
    if game.pick().is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    let Some(t) = super::reacharound::target(game) else {
        return core::ops::ControlFlow::Continue(());
    };
    // PlacementFeature.executeReacharound: useItemOn at the empty cell, from its corner.
    let hit = super::Hit {
        pos: t.pos,
        face: t.face,
        point: [f64::from(t.pos[0]), f64::from(t.pos[1]), f64::from(t.pos[2])],
    };
    let before = game.forced_hit.replace(hit);
    let done = game.remote_use_block(t.hand);
    game.forced_hit = before;
    if done {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
