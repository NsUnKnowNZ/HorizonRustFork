// What a display entity shows, for Jade's BlockDisplayProvider and ItemDisplayProvider: `RemoteTextDisplay` keeps its
// fields to this file.
use super::{RemoteTextDisplay, Shows};

impl RemoteTextDisplay {
    /// `Display.BlockDisplay.getBlockState`; None for another kind of display.
    pub(in crate::game) fn hz_jade_block(&self) -> Option<horizon_world::registry::BlockStateId> {
        if self.shows == Shows::Block { self.block } else { None }
    }
    /// `Display.ItemDisplay.getItemStack`; None for another kind of display, or an empty one.
    pub(in crate::game) fn hz_jade_item(&self) -> Option<&horizon_client::items::ItemStack> {
        if self.shows == Shows::Item { self.item.as_ref() } else { None }
    }
}
