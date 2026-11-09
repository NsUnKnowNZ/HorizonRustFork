// Custom Window Title for Horizon. Config: horizon-mods/.horizon-mod-data/custom_window_title/config.json
use std::sync::atomic::{AtomicBool, Ordering};

const DIR: &str = "horizon-mods/.horizon-mod-data/custom_window_title";
static DONE: AtomicBool = AtomicBool::new(false);

fn icon(path: &std::path::Path) -> Option<winit::window::Icon> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        _ => return None,
    };
    winit::window::Icon::from_rgba(rgba, info.width, info.height).ok()
}

pub(super) fn frame(game: &mut super::Game<'_>) {
    if DONE.load(Ordering::Relaxed) {
        return;
    }
    let Some(window) = game.window.clone() else { return };
    DONE.store(true, Ordering::Relaxed);
    let text = std::fs::read_to_string(std::path::Path::new(DIR).join("config.json")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if let Some(title) = v["title"].as_str() {
        window.set_title(&title.replace("{mcversion}", "26.3"));
    }
    if let Some(name) = v["icon"].as_str() {
        if let Some(icon) = icon(&std::path::Path::new(DIR).join(name)) {
            window.set_window_icon(Some(icon));
        }
    }
}
