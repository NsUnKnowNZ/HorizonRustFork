//! Seamless Loading Screen (MIT): a picture of the world, taken while it was played, is the
//! loading screen's background when it is opened again, in place of the panorama. The picture is
//! the world-only grab the world icon uses (after the level and the hand, before the effects and
//! the HUD), every ten seconds of play with no screen open, saved as the mod saves it:
//! `screenshots/worlds/singleplayer/<save folder>.png`, at most 1024 pixels wide. Single player
//! only: a server's connection keeps no name for its address here.
use super::*;

/// How often a picture is taken while playing.
const EVERY: Duration = Duration::from_secs(10);
/// The widest the picture is: the HUD atlas's mod image slot.
const WIDTH: u32 = 1024;

#[derive(Default)]
pub(super) struct SeamlessLoading {
    last_capture: Option<Instant>,
    /// The picture in the mod image slot: its file and size.
    shown: Option<(PathBuf, [u32; 2])>,
    /// A file that would not read, not tried again.
    failed: Option<PathBuf>,
}

fn picture_path(folder: &str) -> PathBuf {
    horizon_client::logging::player_dir()
        .join("screenshots")
        .join("worlds")
        .join("singleplayer")
        .join(format!("{folder}.png"))
}

impl Game<'_> {
    fn seamless_folder(&self) -> Option<&str> {
        self.remote.as_ref()?.save_folder.as_deref()
    }

    /// Once a frame: a picture of the world for its next loading screen, every ten seconds
    /// while it is played with no screen open and every loaded section drawn.
    pub(super) fn tick_seamless_capture(&mut self) {
        if self.screenshot_sequence.is_some()
            || self.player.is_none()
            || self.screen != Screen::Playing
            || self.remote_loading()
            || self
                .seamless
                .last_capture
                .is_some_and(|at| at.elapsed() < EVERY)
        {
            return;
        }
        let Some(folder) = self.seamless_folder().map(str::to_owned) else {
            return;
        };
        if self.streaming.pending() != 0 || !self.pending_uploads.is_empty() {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if renderer.drawn_sections <= 10 || renderer.screenshot.is_some() {
            return;
        }
        let (w, h) = (renderer.config.width.max(1), renderer.config.height.max(1));
        let width = w.min(WIDTH);
        let height = (u64::from(width) * u64::from(h) / u64::from(w)).clamp(1, u64::from(WIDTH)) as u32;
        let path = picture_path(&folder);
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_err()
        {
            return;
        }
        self.seamless.last_capture = Some(Instant::now());
        renderer.screenshot = Some(horizon_client::gfx::ScreenshotRequest {
            path,
            size: Some([width, height]),
            scaled: false,
        });
    }

    /// The level loading screen's background (`Reason::Other`): the world's picture, when it
    /// has one, covering the screen. True when it drew.
    pub(super) fn draw_seamless_background(&mut self, batch: &mut HudBatch) -> bool {
        let Some(folder) = self.seamless_folder().map(str::to_owned) else {
            return false;
        };
        let path = picture_path(&folder);
        if self.seamless.shown.as_ref().is_none_or(|s| s.0 != path) {
            if self.seamless.failed.as_ref() == Some(&path) || !path.is_file() {
                return false;
            }
            let Some(rect) = self.gui.atlas.sprites.get(crate::hud::MOD_IMAGE_SPRITE).copied() else {
                return false;
            };
            let read = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| horizon_client::hud::decode_bytes(&bytes).map_err(|e| e.to_string()));
            let (width, height, rgba) = match read {
                Ok(image) if image.0 <= rect.w && image.1 <= rect.h => image,
                Ok(_) | Err(_) => {
                    log::warn!("Horizon: the loading screen picture {} doesn't read", path.display());
                    self.seamless.failed = Some(path);
                    return false;
                }
            };
            let (aw, ah) = (self.gui.atlas.width as f32, self.gui.atlas.height as f32);
            self.map_patches.push(horizon_client::font::Patch {
                x: (rect.u0 * aw).round() as u32,
                y: (rect.v0 * ah).round() as u32,
                width,
                height,
                rgba,
            });
            self.seamless.shown = Some((path, [width, height]));
        }
        let Some((_, [width, height])) = self.seamless.shown else {
            return false;
        };
        let Some(mut r) = self.gui.atlas.sprites.get(crate::hud::MOD_IMAGE_SPRITE).copied() else {
            return false;
        };
        // Covering the screen: the picture's middle, cut to the screen's shape.
        let (sw, sh) = (batch.width, batch.height);
        let (pw, ph) = (width as f32, height as f32);
        let (mut cw, mut ch) = (pw, pw * sh / sw);
        if ch > ph {
            (cw, ch) = (ph * sw / sh, ph);
        }
        let (cx, cy) = ((pw - cw) / 2.0, (ph - ch) / 2.0);
        let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
        r.u0 += cx * du;
        r.v0 += cy * dv;
        r.u1 = r.u0 + cw * du;
        r.v1 = r.v0 + ch * dv;
        batch.rect_uv(0.0, 0.0, sw, sh, r, u32::MAX);
        // Dimmed a little, so the progress and its map read over it.
        batch.fill(&self.gui.atlas, 0.0, 0.0, sw, sh, rgba(0, 0, 0, 0x50));
        true
    }
}
