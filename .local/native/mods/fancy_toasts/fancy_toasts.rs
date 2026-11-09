// Fancy Toasts for Horizon: advancements as a plaque dropping in at the top centre.
use super::Game;
use horizon_client::advancements::Frame;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

struct Shown {
    id: String,
    start: Instant,
    sounded: bool,
}

struct State {
    queue: VecDeque<String>,
    shown: Option<Shown>,
}

static STATE: Mutex<State> = Mutex::new(State { queue: VecDeque::new(), shown: None });

/// Seconds to drop in and to leave; the time between is 5 s times Notification Time.
const IN: f32 = 0.5;
const OUT: f32 = 0.35;
const TOP: f32 = 6.0;

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// An advancement's toast is about to be queued: it is shown here instead.
pub(super) fn toast_add(kind: &str, id: &str, cancel: &mut bool) {
    if kind != "advancement" {
        return;
    }
    *cancel = true;
    let mut s = lock();
    if s.queue.len() < 32 && !s.queue.iter().any(|q| q == id) {
        s.queue.push_back(id.to_owned());
    }
}

fn ease_out_back(t: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn with_alpha(color: u32, alpha: f32) -> u32 {
    let a = ((color >> 24) as f32 * alpha.clamp(0.0, 1.0)) as u32;
    a << 24 | color & 0x00FF_FFFF
}

/// The frame's label, its colour and the plaque's border.
fn style(frame: Frame) -> (&'static str, u32, u32) {
    match frame {
        Frame::Task => ("advancements.toast.task", 0xFFFF_FF55, 0xFF8C_B43C),
        Frame::Goal => ("advancements.toast.goal", 0xFF55_FFFF, 0xFF3C_8CC8),
        Frame::Challenge => ("advancements.toast.challenge", 0xFFFF_88FF, 0xFFA0_46C8),
    }
}

pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    let hold = 5.0 * game.settings.notification_display_time.max(0.1);
    let mut s = lock();
    if s.shown.is_none()
        && let Some(id) = s.queue.pop_front()
    {
        s.shown = Some(Shown { id, start: Instant::now(), sounded: false });
    }
    let Some(shown) = s.shown.as_mut() else {
        return;
    };
    let t = shown.start.elapsed().as_secs_f32();
    let tree = game.adv.tree.clone();
    let display = tree.by_id.get(&shown.id).and_then(|&i| tree.list.get(i)).and_then(|a| a.display.as_ref());
    let Some(d) = display.filter(|_| t < IN + hold + OUT) else {
        s.shown = None;
        return;
    };
    let (label_key, label_color, border) = style(d.frame);
    if !std::mem::replace(&mut shown.sounded, true) {
        match d.frame {
            Frame::Challenge => game.audio.play("minecraft:ui.toast.challenge_complete", None, 1.0, 1.0),
            Frame::Goal => {
                game.audio.play("minecraft:ui.toast.in", None, 1.0, 1.0);
                game.audio.play("minecraft:entity.player.levelup", None, 0.35, 1.3);
            }
            Frame::Task => game.audio.play("minecraft:ui.toast.in", None, 1.0, 1.0),
        }
    }
    drop(s);

    let title = super::advancements::text_of(&d.title);
    let description = super::advancements::text_of(&d.description);
    let label = Text::translate(label_key, &[]);
    let atlas = &game.gui.atlas;
    let font = &atlas.font;
    let mut lines = font.split(&description, 230.0);
    lines.truncate(2);
    let text_w = lines
        .iter()
        .map(|l| font.width_text(l))
        .chain([font.width_text(&title), font.width_text(&label)])
        .fold(0.0f32, f32::max);
    let w = (text_w + 52.0).clamp(170.0, 290.0).round();
    let h = (32.0 + 10.0 * lines.len() as f32).round();
    let x = ((batch.width - w) / 2.0).round();
    // Drop in with a little overshoot, wait, then rise away.
    let y = if t < IN {
        let p = ease_out_back(t / IN);
        -h - 4.0 + (TOP + h + 4.0) * p
    } else if t < IN + hold {
        TOP
    } else {
        let p = ((t - IN - hold) / OUT).clamp(0.0, 1.0);
        TOP - (TOP + h + 6.0) * p * p
    };
    let fade = if t > IN + hold { 1.0 - ((t - IN - hold) / OUT).clamp(0.0, 1.0) } else { (t / 0.15).min(1.0) };

    let start = batch.vertices.len();
    // The plaque: a dark body, a border in the frame's colour, a lighter band at the top.
    batch.fill(atlas, x - 1.0, y - 1.0, w + 2.0, h + 2.0, with_alpha(border, fade));
    batch.fill_gradient(atlas, (x, y, w, h), with_alpha(0xF020_2028, fade), with_alpha(0xF00E_0E12, fade));
    batch.fill(atlas, x, y, w, 1.0, with_alpha(0x50FF_FFFF, fade));
    // The icon's well.
    batch.fill(atlas, x + 5.0, y + (h - 28.0) / 2.0, 28.0, 28.0, with_alpha(0x60000000, fade));
    batch.fill(atlas, x + 5.0, y + (h - 28.0) / 2.0, 28.0, 1.0, with_alpha(border, fade * 0.8));
    // A shine sweeping across once it has landed.
    let sweep = (t - IN * 0.6) / 0.9;
    if (0.0..1.0).contains(&sweep) {
        let sx = x - 30.0 + (w + 60.0) * sweep;
        let band = 14.0;
        batch.fill_quad(
            atlas,
            [[sx, y], [sx + band, y], [sx + band - 10.0, y + h], [sx - 10.0, y + h]],
            with_alpha(0x38FF_FFFF, fade),
        );
        batch.clip_since(start, x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0);
    }
    let tx = x + 40.0;
    batch.text_styled(atlas, &label, tx, y + 6.0, with_alpha(label_color, fade), true);
    batch.text_styled(atlas, &title, tx, y + 17.0, with_alpha(0xFFFF_FFFF, fade), true);
    for (i, line) in lines.iter().enumerate() {
        batch.text_styled(atlas, line, tx, y + 29.0 + 10.0 * i as f32, with_alpha(0xFFA8_A8B0, fade), false);
    }
    let icon = ItemStack::new(&d.icon, 1);
    let (ix, iy) = (x + 7.0, y + (h - 24.0) / 2.0);
    let item_start = batch.vertices.len();
    game.draw_item_model(batch, &icon, ix, iy);
    batch.scale_since(item_start, ix, iy, 1.5, 1.5);
}
