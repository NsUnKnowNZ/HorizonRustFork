//! Textures native mods register at run time: vanilla's `TextureManager.register(id, texture)`
//! of a `DynamicTexture` (a Fabric mod's `NativeImageBackedTexture`) and `TextureManager.release`.
//! A picture a mod downloaded or a canvas a player painted is pixels the mod holds under a key
//! of its own (`camerapture:pictures/<uuid>`); drawing it asks for its rectangle in the shared
//! atlas, the one the HUD, the world's quads and items all sample. The atlas has
//! [`MOD_TEXTURE_SLOTS`] slots for them ([`hud::mod_texture_sprite`](horizon_client::hud)):
//! an image takes one the first time it is drawn and gives it up when it is the least recently
//! drawn and another needs one, as map textures do (`AtlasSlots`). Vanilla keeps a texture
//! per image instead; more images in view at once than slots draw what fits and leave the rest
//! out for that frame (`native_mod_texture` is None), so a mod draws its own stand-in for them.
use super::{Game, atlas_slots::AtlasSlots};
use horizon_client::hud::{
    HudBatch, MOD_TEXTURE_SIZE, MOD_TEXTURE_SLOTS, Rect, mod_texture_sprite,
};
use std::{collections::HashMap, sync::Arc};

/// One registered image: its pixels and their revision, which an upload compares.
struct Image {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
    revision: u64,
}

/// The images mods registered and the atlas slots showing them.
pub(super) struct ModTextures {
    images: HashMap<String, Image>,
    slots: AtlasSlots<String>,
    /// Each slot's uploaded image revision (none: blank, or another atlas's).
    uploaded: Vec<Option<u64>>,
    /// Slots to upload with this frame's atlas patches.
    pending: Vec<usize>,
    /// What each slot holds, for the patch.
    holder: Vec<Option<String>>,
    next_revision: u64,
}

impl Default for ModTextures {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            slots: AtlasSlots::new(MOD_TEXTURE_SLOTS),
            uploaded: vec![None; MOD_TEXTURE_SLOTS],
            pending: Vec::new(),
            holder: vec![None; MOD_TEXTURE_SLOTS],
            next_revision: 1,
        }
    }
}

impl ModTextures {
    /// The atlas was replaced (a resource reload): every slot is blank.
    pub(super) fn forget(&mut self) {
        self.slots.forget();
        self.uploaded.fill(None);
        self.holder.fill(None);
        self.pending.clear();
    }
    /// The slot showing `key` this frame, an upload queued when it does not hold its pixels.
    fn show(&mut self, key: &str) -> Option<(usize, u32, u32)> {
        let image = self.images.get(key)?;
        let (width, height, revision) = (image.width, image.height, image.revision);
        let slot = match self.slots.show(key) {
            Some(slot) => slot,
            None => self.slots.claim(key.to_owned())?.0,
        };
        if self.uploaded[slot] != Some(revision) {
            self.uploaded[slot] = Some(revision);
            self.holder[slot] = Some(key.to_owned());
            if !self.pending.contains(&slot) {
                self.pending.push(slot);
            }
        }
        Some((slot, width, height))
    }
}

/// `rect` cut down to its top-left `width` x `height` pixels.
fn crop(mut rect: Rect, width: u32, height: u32) -> Rect {
    let (du, dv) = (
        (rect.u1 - rect.u0) / rect.w.max(1) as f32,
        (rect.v1 - rect.v0) / rect.h.max(1) as f32,
    );
    rect.u1 = rect.u0 + du * width as f32;
    rect.v1 = rect.v0 + dv * height as f32;
    rect.w = width;
    rect.h = height;
    rect
}

/// Whether `rgba` is a `width` x `height` image a slot holds.
fn fits(width: u32, height: u32, len: usize) -> bool {
    (1..=MOD_TEXTURE_SIZE).contains(&width)
        && (1..=MOD_TEXTURE_SIZE).contains(&height)
        && len == width as usize * height as usize * 4
}

// Native mods use these helpers.
#[allow(dead_code, reason = "native mod API (docs/native-mods.md)")]
impl Game<'_> {
    /// Registers (or replaces) the RGBA image `key` (`mod_id:path`), `width` x `height`
    /// pixels of four bytes, each side at most [`MOD_TEXTURE_SIZE`]: vanilla's
    /// `TextureManager.register(key, new DynamicTexture(image))`. Scale a larger image down
    /// first. False, with nothing changed, when it is not that size. The pixels are kept here
    /// and go to the atlas when the image is drawn, again after each replacement.
    pub(super) fn native_mod_texture_set(
        &self,
        key: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        if !fits(width, height, rgba.len()) {
            return false;
        }
        let mut textures = self.mod_textures.borrow_mut();
        let revision = textures.next_revision;
        textures.next_revision += 1;
        textures.images.insert(
            key.to_owned(),
            Image {
                width,
                height,
                rgba: rgba.into(),
                revision,
            },
        );
        true
    }
    /// Forgets image `key` (`TextureManager.release`); its slot goes to the next image.
    pub(super) fn native_mod_texture_remove(&self, key: &str) {
        self.mod_textures.borrow_mut().images.remove(key);
    }
    /// The size image `key` was registered with, if it is.
    pub(super) fn native_mod_texture_size(&self, key: &str) -> Option<[u32; 2]> {
        let textures = self.mod_textures.borrow();
        textures.images.get(key).map(|i| [i.width, i.height])
    }
    /// Where image `key` is in the shared atlas this frame, cut to its own size: draw it with
    /// `HudBatch::rect_uv` (or [`Self::native_mod_texture_draw`]), as a world quad
    /// (`native_world_mod_texture_quad`) or as an item's quads (a special item model).
    /// Asking marks it drawn this frame, and the first ask after it was registered or
    /// replaced uploads it with this frame's atlas patches. None when it isn't registered,
    /// when every slot holds an image drawn this frame, or without the slots (a game built
    /// without native mods).
    pub(super) fn native_mod_texture(&self, key: &str) -> Option<Rect> {
        let (slot, width, height) = self.mod_textures.borrow_mut().show(key)?;
        let rect = self
            .gui
            .atlas
            .sprites
            .get(mod_texture_sprite(slot)?)
            .copied()?;
        Some(crop(rect, width, height))
    }
    /// Image `key` stretched into `dst` (`[x, y, w, h]` in GUI pixels), tinted by `argb`
    /// (`0xFFFFFFFF` draws it as it is): `GuiGraphics.blit` of a registered texture. False
    /// when it can't be drawn this frame (`native_mod_texture`).
    pub(super) fn native_mod_texture_draw(
        &self,
        batch: &mut HudBatch,
        key: &str,
        dst: [f32; 4],
        argb: u32,
    ) -> bool {
        let Some(rect) = self.native_mod_texture(key) else {
            return false;
        };
        let [a, r, g, b] = argb.to_be_bytes();
        batch.rect_uv(
            dst[0],
            dst[1],
            dst[2],
            dst[3],
            rect,
            horizon_client::hud::rgba(r, g, b, a),
        );
        true
    }
    /// Queues the uploads the frame's drawing asked for into this frame's atlas patches, then
    /// starts the next frame of the slots' use. Once a frame, just before the patches go.
    pub(super) fn upload_mod_textures(&mut self) {
        let atlas_size = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
        let textures = self.mod_textures.get_mut();
        for slot in std::mem::take(&mut textures.pending) {
            let image = textures.holder[slot]
                .as_ref()
                .and_then(|key| textures.images.get(key))
                .map(|i| (i.width, i.height, i.rgba.clone()));
            let Some((width, height, rgba)) = image else {
                textures.uploaded[slot] = None;
                continue;
            };
            let Some(rect) = mod_texture_sprite(slot)
                .and_then(|name| self.gui.atlas.sprites.get(name))
                .copied()
            else {
                continue;
            };
            self.map_patches.push(horizon_client::font::Patch {
                x: (rect.u0 * atlas_size.0).round() as u32,
                y: (rect.v0 * atlas_size.1).round() as u32,
                width,
                height,
                rgba: rgba.to_vec(),
            });
        }
        textures.slots.next_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_image_uploads_again_into_the_slot_it_holds() {
        let mut textures = ModTextures::default();
        textures.images.insert(
            "m:a".into(),
            Image {
                width: 2,
                height: 1,
                rgba: vec![0; 8].into(),
                revision: 1,
            },
        );
        assert_eq!(textures.show("m:a"), Some((0, 2, 1)));
        assert_eq!(std::mem::take(&mut textures.pending), [0]);
        // Drawn again unchanged: nothing to upload.
        assert_eq!(textures.show("m:a"), Some((0, 2, 1)));
        assert!(textures.pending.is_empty());
        textures.images.get_mut("m:a").unwrap().revision = 2;
        assert_eq!(textures.show("m:a"), Some((0, 2, 1)));
        assert_eq!(textures.pending, [0]);
    }

    #[test]
    fn crop_keeps_the_images_own_pixels() {
        let rect = Rect {
            w: 512,
            h: 512,
            u0: 0.5,
            v0: 0.25,
            u1: 0.75,
            v1: 0.5,
        };
        let cut = crop(rect, 256, 128);
        assert_eq!((cut.w, cut.h), (256, 128));
        assert!((cut.u1 - 0.625).abs() < 1e-6 && (cut.v1 - 0.3125).abs() < 1e-6);
    }

    #[test]
    fn only_images_a_slot_holds_fit() {
        assert!(fits(MOD_TEXTURE_SIZE, 1, MOD_TEXTURE_SIZE as usize * 4));
        assert!(!fits(
            MOD_TEXTURE_SIZE + 1,
            1,
            (MOD_TEXTURE_SIZE as usize + 1) * 4
        ));
        assert!(!fits(2, 2, 15));
    }
}
