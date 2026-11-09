// Game::use_item HEAD (pinned): a sneaking right click with an empty hand
// picks up the block or mob aimed at; while carrying, a right click on a
// block puts it down and any other right click does nothing (full hands).
// Everything else is used as usual.
fn carryblock_use(
    game: &mut crate::game::Game<'_>,
) -> core::ops::ControlFlow<Result<Option<Hand>>> {
    match game.carryblock_use() {
        Some(hand) => core::ops::ControlFlow::Break(Ok(hand)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
