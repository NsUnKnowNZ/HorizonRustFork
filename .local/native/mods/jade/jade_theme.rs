// Jade's themes (ThemeHelper, Theme, BoxStyle, TextSetting, ColorPalette): `jade_themes/**.json`, the jar's set below
// and whatever the resource packs and content mods on the game's resource root add, keyed `namespace:path` as Java's
// FileToIdConverter does (`jade:dark`, `jade:dark/slim`). The `overlay_theme` setting keeps the main id (`dark`, or
// `mypack:fancy` outside Jade's namespace) and `theme_style` the part after the slash (`default` for none), so a value
// saved before themes were loaded still means the same.
//
// The jar's textures aren't on the resource root (a mod bundle carries no assets), so the atlas has no `jade:` sprites
// unless a pack brings them. A sprite in the atlas draws nine-sliced by its own `.png.mcmeta`; Jade's own sprites
// without one draw as `native_fill`s of the same shape and colours, read from the jar's PNGs (`builtin_look`).
use crate::game::Game;
use crate::game::jade;
use horizon_client::hud::{HudBatch, rgba};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

/// assets/jade/jade_themes/ of Jade-mc26.3-Fabric-26.3.5.jar, as they are there.
const JAR_THEMES: [(&str, &str); 7] = [
    ("jade:create", r#"{"version":200,"tooltipStyle":{"sprite":"jade:create","tooltip":true}}"#),
    ("jade:dark", r#"{"version":200,"tooltipStyle":{"sprite":"jade:dark","tooltip":true}}"#),
    ("jade:top", r#"{"version":200,"tooltipStyle":{"sprite":"jade:top","borderWidth":2,"padding":[5,4,5,4]}}"#),
    ("jade:waila", r#"{"version":200,"tooltipStyle":{"sprite":"jade:waila","tooltip":true}}"#),
    ("jade:create/slim", r#"{"version":200,"styleName":"jade.slim","tooltipStyle":{"sprite":"jade:create_slim"}}"#),
    ("jade:dark/slim", r#"{"version":200,"styleName":"jade.slim","tooltipStyle":{"sprite":"jade:dark_slim"}}"#),
    ("jade:waila/slim", r#"{"version":200,"styleName":"jade.slim","tooltipStyle":{"sprite":"jade:waila_slim"}}"#),
];

/// `JadeIds.DEFAULT_THEME`.
const DEFAULT_THEME: &str = "jade:dark";

/// ColorPalette: ARGB.
#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub normal: u32,
    pub info: u32,
    pub title: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
    pub failure: u32,
}

impl Palette {
    /// ColorPalette.DEFAULT.
    pub const DEFAULT: Palette = Palette { normal: 0xFFA0_A0A0, info: 0xFFFF_FFFF, title: 0xFFFF_FFFF, success: 0xFF55_FF55, warning: 0xFFFF_C107, danger: 0xFFFF_5555, failure: 0xFFAA_0000 };

    fn parse(v: Option<&serde_json::Value>) -> Palette {
        let d = Palette::DEFAULT;
        let Some(v) = v else { return d };
        let c = |name: &str, or: u32| v.get(name).and_then(|x| x.as_str()).and_then(color).unwrap_or(or);
        Palette { normal: c("normal", d.normal), info: c("info", d.info), title: c("title", d.title), success: c("success", d.success), warning: c("warning", d.warning), danger: c("danger", d.danger), failure: c("failure", d.failure) }
    }
}

/// `Color.split`: what's between the brackets, by commas or spaces (`/` is a space).
fn color_parts(s: &str) -> Option<Vec<String>> {
    let s = s.replace('/', " ");
    let inner = s.get(s.find('(')? + 1..s.find(')')?)?;
    let parts: Vec<&str> = if inner.contains(',') { inner.split(',').collect() } else { inner.split_whitespace().collect() };
    Some(parts.into_iter().map(|p| p.trim().to_owned()).collect())
}

/// `Color.parseDouble`: a number, or a percentage of `limit`.
fn color_number(p: &str, limit: f64) -> Option<f64> {
    match p.trim().strip_suffix('%') {
        Some(pct) => Some(pct.trim().parse::<f64>().ok()? * (limit / 100.0)),
        None => p.trim().parse().ok(),
    }
}

/// `Color.toDegrees`.
fn color_degrees(p: &str) -> Option<f64> {
    let p = p.trim();
    if let Some(v) = p.strip_suffix("deg") {
        v.trim().parse().ok()
    } else if let Some(v) = p.strip_suffix("grad") {
        Some(v.trim().parse::<f64>().ok()? / 400.0 * 360.0)
    } else if let Some(v) = p.strip_suffix("rad") {
        Some(v.trim().parse::<f64>().ok()? * (180.0 / std::f64::consts::PI))
    } else if let Some(v) = p.strip_suffix("turn") {
        Some(v.trim().parse::<f64>().ok()? * 360.0)
    } else {
        color_number(p, 360.0)
    }
}

/// `Color.hsl`, as ARGB.
fn hsl_color(hue: f64, saturation: f64, lightness: f64, opacity: f64) -> u32 {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h {
        h if (0.0..1.0).contains(&h) => (c, x, 0.0),
        h if (1.0..2.0).contains(&h) => (x, c, 0.0),
        h if (2.0..3.0).contains(&h) => (0.0, c, x),
        h if (3.0..4.0).contains(&h) => (0.0, x, c),
        h if (4.0..5.0).contains(&h) => (x, 0.0, c),
        h if (5.0..6.0).contains(&h) => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };
    let m = lightness - c / 2.0;
    let channel = |v: f64| (((v + m) * 255.0) as i32).clamp(0, 255) as u32;
    (((opacity.clamp(0.0, 1.0) * 255.0) as u32) << 24) | (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// `Color.valueOf`, as ARGB: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (the `#` optional), `rgb(r, g, b[, a])`,
/// `hsl(h, s, l[, a])` and `transparent`.
fn color(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase();
    if s == "transparent" {
        return Some(0);
    }
    if s.contains("rgb") {
        let parts = color_parts(&s)?;
        let channel = |p: &String| -> Option<u32> { Some(color_number(p, 255.0)?.round().clamp(0.0, 255.0) as u32) };
        let (r, g, b) = (channel(parts.first()?)?, channel(parts.get(1)?)?, channel(parts.get(2)?)?);
        let a = parts.get(3).and_then(|p| color_number(p, 1.0)).map_or(255, |a| (a.clamp(0.0, 1.0) * 255.0) as u32);
        return Some((a << 24) | (r << 16) | (g << 8) | b);
    }
    if s.contains("hsl") {
        let parts = color_parts(&s)?;
        let hue = color_degrees(parts.first()?)?;
        let saturation = color_number(parts.get(1)?, 1.0)?;
        let lightness = color_number(parts.get(2)?, 1.0)?;
        let opacity = parts.get(3).and_then(|p| color_number(p, 1.0)).unwrap_or(1.0);
        return Some(hsl_color(hue, saturation, lightness, opacity));
    }
    let hex = s.strip_prefix('#').unwrap_or(&s);
    let full: String = if hex.len() == 3 || hex.len() == 4 { hex.chars().flat_map(|c| [c, c]).collect() } else { hex.to_owned() };
    if full.len() != 6 && full.len() != 8 {
        return None;
    }
    let v = u32::from_str_radix(&full, 16).ok()?;
    Some(if full.len() == 6 { 0xFF00_0000 | v } else { (v >> 8) | (v << 24) })
}

/// A chat style's parts, as much of it as the overlay's text can show.
#[derive(Clone, Copy, Default)]
struct StylePart {
    color: Option<u32>,
    bold: Option<bool>,
    italic: Option<bool>,
    underlined: Option<bool>,
    strikethrough: Option<bool>,
    obfuscated: Option<bool>,
}

/// `Style.Serializer`'s fields.
fn style_part(v: Option<&serde_json::Value>) -> StylePart {
    let Some(v) = v else { return StylePart::default() };
    let flag = |k: &str| v.get(k).and_then(|x| x.as_bool());
    let named = |n: &str| -> Option<u32> {
        Some(match n {
            "black" => 0x000000,
            "dark_blue" => 0x0000AA,
            "dark_green" => 0x00AA00,
            "dark_aqua" => 0x00AAAA,
            "dark_red" => 0xAA0000,
            "dark_purple" => 0xAA00AA,
            "gold" => 0xFFAA00,
            "gray" => 0xAAAAAA,
            "dark_gray" => 0x555555,
            "blue" => 0x5555FF,
            "green" => 0x55FF55,
            "aqua" => 0x55FFFF,
            "red" => 0xFF5555,
            "light_purple" => 0xFF55FF,
            "yellow" => 0xFFFF55,
            "white" => 0xFFFFFF,
            other => u32::from_str_radix(other.strip_prefix('#')?, 16).ok()?,
        })
    };
    StylePart {
        color: v.get("color").and_then(|x| x.as_str()).and_then(named),
        bold: flag("bold"),
        italic: flag("italic"),
        underlined: flag("underlined"),
        strikethrough: flag("strikethrough"),
        obfuscated: flag("obfuscated"),
    }
}

/// `ThemeHelper.modName`: the theme's modNameStyle over Formatting's item mod name style (blue italic), as the codes
/// the overlay's text reads.
fn mod_name_code(theme: StylePart) -> String {
    let mut out = match theme.color {
        Some(c) => format!("§#{:06X}", c & 0xFF_FFFF),
        None => "§9".to_owned(),
    };
    if theme.italic.unwrap_or(true) {
        out.push_str("§o");
    }
    if theme.bold.unwrap_or(false) {
        out.push_str("§l");
    }
    if theme.underlined.unwrap_or(false) {
        out.push_str("§n");
    }
    if theme.strikethrough.unwrap_or(false) {
        out.push_str("§m");
    }
    if theme.obfuscated.unwrap_or(false) {
        out.push_str("§k");
    }
    out
}

/// A sprite named by a theme, as the atlas would name it, and how it scales (its `.png.mcmeta`).
struct Sprite {
    id: String,
    atlas: String,
    /// GuiSpriteScaling.NineSlice: width, height, and the border's left, top, right, bottom.
    nine: Option<[f32; 6]>,
}

impl Sprite {
    fn new(root: &std::path::Path, id: &str) -> Sprite {
        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
        let meta = root.join(format!("assets/{ns}/textures/gui/sprites/{path}.png.mcmeta"));
        Sprite { id: id.to_owned(), atlas: if ns == "minecraft" { format!("gui/sprites/{path}") } else { format!("{ns}:gui/sprites/{path}") }, nine: read_nine(&meta) }
    }

    /// `blitSprite` with a white of `alpha`, when the atlas has it. False when it hasn't.
    fn draw(&self, game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: [f32; 4], alpha: u8) -> bool {
        let Some(r) = game.gui.atlas.sprites.get(&self.atlas).copied() else {
            return false;
        };
        let white = rgba(255, 255, 255, alpha);
        let Some([sw, sh, l, t, right, b]) = self.nine else {
            batch.rect_uv(x, y, w, h, r, white);
            return true;
        };
        let (du, dv) = ((r.u1 - r.u0) / sw, (r.v1 - r.v0) / sh);
        // Each part of the sprite stretched into its part of the box (Jade's own sprites all say stretch_inner).
        let mut part = |src: [f32; 4], dst: [f32; 4]| {
            if dst[2] > 0.0 && dst[3] > 0.0 {
                let mut p = r;
                p.u0 = r.u0 + src[0] * du;
                p.v0 = r.v0 + src[1] * dv;
                p.u1 = r.u0 + (src[0] + src[2]) * du;
                p.v1 = r.v0 + (src[1] + src[3]) * dv;
                batch.rect_uv(dst[0], dst[1], dst[2], dst[3], p, white);
            }
        };
        let (l, right) = (l.min((w / 2.0).floor()), right.min((w / 2.0).floor()));
        let (t, b) = (t.min((h / 2.0).floor()), b.min((h / 2.0).floor()));
        let (iw, ih) = (w - l - right, h - t - b);
        let (siw, sih) = (sw - l - right, sh - t - b);
        part([0.0, 0.0, l, t], [x, y, l, t]);
        part([l, 0.0, siw, t], [x + l, y, iw, t]);
        part([sw - right, 0.0, right, t], [x + w - right, y, right, t]);
        part([0.0, t, l, sih], [x, y + t, l, ih]);
        part([l, t, siw, sih], [x + l, y + t, iw, ih]);
        part([sw - right, t, right, sih], [x + w - right, y + t, right, ih]);
        part([0.0, sh - b, l, b], [x, y + h - b, l, b]);
        part([l, sh - b, siw, b], [x + l, y + h - b, iw, b]);
        part([sw - right, sh - b, right, b], [x + w - right, y + h - b, right, b]);
        true
    }
}

/// A sprite's `gui.scaling` when it is a nine_slice (border a number or `{left, top, right, bottom}`).
fn read_nine(meta: &std::path::Path) -> Option<[f32; 6]> {
    let text = std::fs::read_to_string(meta).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let s = v.get("gui")?.get("scaling")?;
    if s.get("type")?.as_str()? != "nine_slice" {
        return None;
    }
    let (w, h) = (s.get("width")?.as_f64()? as f32, s.get("height")?.as_f64()? as f32);
    let border = s.get("border")?;
    let [l, t, r, b] = match border.as_f64() {
        Some(all) => [all as f32; 4],
        None => {
            let side = |k: &str| border.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            [side("left"), side("top"), side("right"), side("bottom")]
        }
    };
    Some([w, h, l, t, r, b])
}

/// BoxStyle.
struct BoxStyle {
    /// `sprite`, with `tooltip`'s `_background` and `_frame` where that is set.
    plain: Option<(Sprite, Option<Sprite>)>,
    with_icon: Option<(Sprite, Option<Sprite>)>,
    tooltip: bool,
    /// ScreenDirection's order: up, down, left, right.
    padding: [f32; 4],
    progress: Palette,
    progress_offset: [f32; 4],
}

impl BoxStyle {
    fn parse(root: &std::path::Path, v: &serde_json::Value) -> BoxStyle {
        let tooltip = v.get("tooltip").and_then(|x| x.as_bool()).unwrap_or(false);
        let sprites = |key: &str| -> Option<(Sprite, Option<Sprite>)> {
            let id = v.get(key)?.as_str()?;
            let id = if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") };
            if tooltip {
                // TooltipRenderUtil.getBackgroundSprite and getFrameSprite.
                let (ns, path) = id.split_once(':')?;
                let mut back = Sprite::new(root, &format!("{ns}:tooltip/{path}_background"));
                back.id = id.clone();
                Some((back, Some(Sprite::new(root, &format!("{ns}:tooltip/{path}_frame")))))
            } else {
                Some((Sprite::new(root, &id), None))
            }
        };
        let floats = |key: &str, or: [f32; 4]| -> [f32; 4] {
            let Some(a) = v.get(key).and_then(|x| x.as_array()) else { return or };
            let mut out = or;
            for (slot, x) in out.iter_mut().zip(a) {
                *slot = x.as_f64().unwrap_or(0.0) as f32;
            }
            out
        };
        BoxStyle {
            plain: sprites("sprite"),
            with_icon: sprites("withIconSprite"),
            tooltip,
            padding: floats("padding", [3.0; 4]),
            progress: Palette::parse(v.get("boxProgressColors")),
            progress_offset: floats("boxProgressOffset", [0.0; 4]),
        }
    }
}

/// How the overlay's text looks with this theme (TextSetting).
pub struct TextLook {
    /// ColorPalette.normal, for text with no colour of its own, in the batch's colour form.
    pub base: u32,
    /// TextSetting.shadow.
    pub shadow: bool,
    /// TextSetting.itemAmountColor, in the batch's colour form.
    pub amount: u32,
    palette: Palette,
}

impl TextLook {
    /// A line's text with the colours Jade's providers ask for by message type (IThemeHelper.info, success, ...) as
    /// the theme has them. This port writes those as the legacy codes nearest them; with the default palette they stay.
    pub fn map<'a>(&self, s: &'a str) -> Cow<'a, str> {
        let p = self.palette;
        if p == Palette::DEFAULT || !s.contains('§') {
            return Cow::Borrowed(s);
        }
        let mut out = String::with_capacity(s.len() + 16);
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c != '§' {
                out.push(c);
                continue;
            }
            let Some(code) = chars.next() else { break };
            let mapped = match code {
                '7' => Some(p.normal),
                'f' => Some(p.info),
                'a' => Some(p.success),
                '6' | 'e' => Some(p.warning),
                'c' => Some(p.danger),
                '4' => Some(p.failure),
                _ => None,
            };
            match mapped {
                Some(argb) => out.push_str(&format!("§#{:06X}", argb & 0xFF_FFFF)),
                None => {
                    out.push('§');
                    out.push(code);
                }
            }
        }
        Cow::Owned(out)
    }
}

/// SneakyDetails.Simple: the arrow under a Lite box that has more to show.
struct Sneaky {
    sprite: String,
    width: f32,
    height: f32,
    offset: [f32; 2],
    breath: bool,
    distance: f32,
    period: f32,
}

impl Sneaky {
    /// `SneakyDetails.CODEC`; `SneakyDetails.DEFAULT` without one (or for a type Java refuses too).
    fn parse(v: Option<&serde_json::Value>) -> Sneaky {
        let default = Sneaky { sprite: "jade:details_arrow".to_owned(), width: 7.0, height: 5.0, offset: [0.0; 2], breath: true, distance: 1.0, period: 12.0 };
        let Some(v) = v.filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("simple")) else { return default };
        let (Some(sprite), Some(width), Some(height)) = (v.get("sprite").and_then(|x| x.as_str()), v.get("width").and_then(|x| x.as_f64()), v.get("height").and_then(|x| x.as_f64())) else {
            return default;
        };
        let float = |k: &str, or: f64| v.get(k).and_then(|x| x.as_f64()).unwrap_or(or) as f32;
        Sneaky {
            sprite: if sprite.contains(':') { sprite.to_owned() } else { format!("minecraft:{sprite}") },
            width: width as f32,
            height: height as f32,
            offset: [float("offsetX", 0.0), float("offsetY", 0.0)],
            breath: v.get("animation").and_then(|x| x.as_str()) == Some("breath"),
            distance: float("animationDistance", 1.0),
            period: float("animationPeriod", 12.0),
        }
    }
}

/// details_arrow.png, 7 by 5, as (row, from, to, ARGB) pixel runs.
const DETAILS_ARROW: [(u8, u8, u8, u32); 13] = [
    (0, 0, 7, 0xFF66_6666),
    (1, 0, 1, 0xFF66_6666),
    (1, 1, 2, 0xFFD9_D9D9),
    (1, 2, 5, 0xFFFF_FFFF),
    (1, 5, 6, 0xFFD9_D9D9),
    (1, 6, 7, 0xFF66_6666),
    (2, 1, 2, 0xFF66_6666),
    (2, 2, 3, 0xFFD9_D9D9),
    (2, 3, 4, 0xFFFF_FFFF),
    (2, 4, 5, 0xFFD9_D9D9),
    (2, 5, 6, 0xFF66_6666),
    (3, 2, 5, 0xFF66_6666),
    (4, 3, 4, 0xFF66_6666),
];

/// `Theme.modifyIcon`'s slot: the sprite, when the atlas has it, and what stands between it and the icon (`padding`:
/// its inflation and its nine-slice border): top, right, bottom, left.
pub struct IconSlot<'a> {
    sprite: &'a Sprite,
    pub padding: [f32; 4],
}

impl IconSlot<'_> {
    /// Draws the slot round an icon of `size` at `at`.
    pub fn draw(&self, game: &Game<'_>, batch: &mut HudBatch, at: [f32; 2], size: f32, alpha: f32) {
        let [top, right, bottom, left] = self.padding;
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        self.sprite.draw(game, batch, [at[0] - left, at[1] - top, size + left + right, size + top + bottom], a);
    }
}

/// Theme and its holder's autoEnable.
pub struct Theme {
    /// `fullId`.
    pub id: String,
    /// `mainId`.
    pub main: String,
    /// `styleId`: empty for a theme's default style.
    pub style: String,
    pub style_name: String,
    tooltip_style: BoxStyle,
    pub colors: Palette,
    shadow: bool,
    /// The title's code (IThemeHelper.title).
    pub title_code: String,
    /// The mod name's codes (IThemeHelper.modName).
    pub mod_code: String,
    amount: u32,
    pub change_opacity: f32,
    auto_enable: bool,
    sneaky: Sneaky,
    icon_slot: Option<Sprite>,
    icon_slot_inflation: f32,
    /// `spriteMapping`: `namespace:path` to `namespace:path`.
    sprite_mapping: HashMap<String, String>,
}

/// A `namespace:path` sprite id as the atlas names it.
fn atlas_name(id: &str) -> String {
    match id.split_once(':') {
        Some(("minecraft", path)) => format!("gui/sprites/{path}"),
        Some((ns, path)) => format!("{ns}:gui/sprites/{path}"),
        None => format!("gui/sprites/{id}"),
    }
}

fn batch_color(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    rgba(r, g, b, a)
}

impl Theme {
    fn parse(root: &std::path::Path, id: &str, text: &str) -> Option<Theme> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        // ThemeHelper.allowedVersions.
        if !(200..=299).contains(&v.get("version")?.as_i64()?) {
            return None;
        }
        let (ns, path) = id.split_once(':')?;
        let (main, style) = match path.split_once('/') {
            Some((m, s)) => (format!("{ns}:{m}"), s.to_owned()),
            None => (id.to_owned(), String::new()),
        };
        let text_setting = v.get("text");
        let colors = Palette::parse(text_setting.and_then(|t| t.get("colors")));
        let title = colors.title & 0xFF_FFFF;
        Some(Theme {
            id: id.to_owned(),
            main,
            style,
            style_name: v.get("styleName").and_then(|x| x.as_str()).unwrap_or("jade.default").to_owned(),
            tooltip_style: BoxStyle::parse(root, v.get("tooltipStyle")?),
            colors,
            shadow: text_setting.and_then(|t| t.get("shadow")).and_then(|x| x.as_bool()).unwrap_or(true),
            title_code: if title == 0xFF_FFFF { "§f".to_owned() } else { format!("§#{title:06X}") },
            mod_code: mod_name_code(style_part(text_setting.and_then(|t| t.get("modNameStyle")))),
            amount: text_setting.and_then(|t| t.get("itemAmountColor")).and_then(|x| x.as_str()).and_then(color).unwrap_or(0xFFFF_FFFF),
            change_opacity: v.get("changeOpacity").and_then(|x| x.as_f64()).unwrap_or(0.0).clamp(0.0, 1.0) as f32,
            auto_enable: v.get("autoEnable").and_then(|x| x.as_bool()).unwrap_or(false),
            sneaky: Sneaky::parse(v.get("sneakyDetails")),
            icon_slot: v.get("iconSlotSprite").and_then(|x| x.as_str()).map(|s| Sprite::new(root, &if s.contains(':') { s.to_owned() } else { format!("minecraft:{s}") })),
            icon_slot_inflation: v.get("iconSlotInflation").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
            sprite_mapping: v
                .get("spriteMapping")
                .and_then(|x| x.as_object())
                .map(|map| {
                    let full = |s: &str| if s.contains(':') { s.to_owned() } else { format!("minecraft:{s}") };
                    map.iter().filter_map(|(from, to)| Some((full(from), full(to.as_str()?)))).collect()
                })
                .unwrap_or_default(),
        })
    }

    pub fn text_look(&self) -> TextLook {
        TextLook { base: batch_color(self.colors.normal), shadow: self.shadow, amount: batch_color(self.amount), palette: self.colors }
    }

    /// BoxStyle's padding: up, down, left, right.
    pub fn padding(&self) -> [f32; 4] {
        self.tooltip_style.padding
    }

    /// `Theme.mapSprite`, for a sprite Jade asks for by `namespace:path`: what the atlas should be asked for instead.
    /// None when the theme says nothing of it, which is nearly always (an empty map is one comparison).
    pub fn mapped_sprite(&self, id: &str) -> Option<String> {
        if self.sprite_mapping.is_empty() {
            return None;
        }
        self.sprite_mapping.get(id).map(|to| atlas_name(to))
    }

    /// `Theme.modifyIcon`: the slot behind the icon. None without one, without its sprite in the atlas (the jar's textures
    /// aren't there), and for an in-line icon.
    pub fn icon_slot(&self, game: &Game<'_>) -> Option<IconSlot<'_>> {
        let sprite = self.icon_slot.as_ref().filter(|s| game.gui.atlas.sprites.contains_key(&s.atlas))?;
        let i = self.icon_slot_inflation;
        let padding = match sprite.nine {
            Some([_, _, left, top, right, bottom]) => [i + top, i + right, i + bottom, i + left],
            None => [i; 4],
        };
        Some(IconSlot { sprite, padding })
    }

    /// `SneakyDetails.Simple.render`: the arrow at the foot of the box. Nothing for the part of its period it is away.
    pub fn draw_sneaky(&self, game: &Game<'_>, batch: &mut HudBatch, frame: [f32; 4], ticks: f32, opacity: f32) {
        let s = &self.sneaky;
        let x = frame[0] + frame[2] / 2.0 - s.width / 2.0 + s.offset[0];
        let mut y = frame[1] + frame[3] - s.height / 2.0 + s.offset[1];
        let mut alpha = 1.0;
        if s.breath {
            let breath = (ticks / 5.0) % s.period - 2.0;
            if breath > 4.0 {
                return;
            }
            alpha = 1.0 - breath.abs() / 2.0;
            y += s.distance * breath;
            if alpha < 0.016 {
                return;
            }
        }
        let alpha = (alpha * opacity).clamp(0.0, 1.0);
        let name = self.mapped_sprite(&s.sprite).unwrap_or_else(|| atlas_name(&s.sprite));
        if let Some(r) = game.gui.atlas.sprites.get(&name).copied() {
            batch.rect_uv(x, y, s.width, s.height, r, rgba(255, 255, 255, (alpha * 255.0) as u8));
        } else if s.sprite == "jade:details_arrow" {
            // Jade's own sprite isn't in the atlas: its pixels, at its size.
            for (row, from, to, argb) in DETAILS_ARROW {
                let (from, to, row) = (f32::from(from), f32::from(to), f32::from(row));
                batch.fill(&game.gui.atlas, x + from, y + row, to - from, 1.0, dim(argb, alpha));
            }
        }
    }
}

/// The colours of Jade's own sprites, from the jar's PNGs: the fill, the border's top and bottom (the sides go from
/// one to the other), its width, and whether a one pixel ring of the fill goes round it (the `tooltip/*_background`
/// sprites, drawn with the corners cut; they stand one pixel outside the box).
struct Look {
    fill: u32,
    top: u32,
    bottom: u32,
    border: f32,
    ring: bool,
}

fn builtin_look(id: &str) -> Option<Look> {
    let (fill, top, bottom, border, ring) = match id {
        "jade:dark" => (0xFF13_1313, 0xFF38_3838, 0xFF24_2424, 1.0, true),
        "jade:waila" => (0xFF10_0010, 0xFF50_00FF, 0xFF29_0080, 1.0, true),
        "jade:create" => (0xFF00_0000, 0xFF40_3C37, 0xFF20_1E1C, 1.0, true),
        "jade:dark_slim" => (0xFF13_1313, 0xFF38_3838, 0xFF24_2424, 1.0, false),
        "jade:waila_slim" => (0xFF10_0010, 0xFF50_00FF, 0xFF29_0080, 1.0, false),
        "jade:create_slim" => (0xFF00_0000, 0xFF40_3C37, 0xFF20_1E1C, 1.0, false),
        "jade:top" => (0x8000_6699, 0xFF99_99AD, 0xFF99_99AD, 2.0, false),
        _ => return None,
    };
    Some(Look { fill, top, bottom, border, ring })
}

/// A colour of a sprite under `ARGB.white(alpha)`.
fn dim(argb: u32, alpha: f32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    rgba(r, g, b, (f32::from(a) * alpha) as u8)
}

/// Draws a `Look` over the box (the ring outside it where it has one), each pixel once, so a translucent box looks as
/// the sprite does.
fn draw_look(game: &Game<'_>, batch: &mut HudBatch, look: &Look, [x, y, w, h]: [f32; 4], alpha: f32) {
    let atlas = &game.gui.atlas;
    let (mut x, mut y, mut w, mut h) = (x, y, w, h);
    let fill = dim(look.fill, alpha);
    if look.ring {
        (x, y, w, h) = (x - 1.0, y - 1.0, w + 2.0, h + 2.0);
        batch.fill(atlas, x + 1.0, y, w - 2.0, 1.0, fill);
        batch.fill(atlas, x + 1.0, y + h - 1.0, w - 2.0, 1.0, fill);
        batch.fill(atlas, x, y + 1.0, 1.0, h - 2.0, fill);
        batch.fill(atlas, x + w - 1.0, y + 1.0, 1.0, h - 2.0, fill);
        (x, y, w, h) = (x + 1.0, y + 1.0, w - 2.0, h - 2.0);
    }
    let b = look.border;
    let (top, bottom) = (dim(look.top, alpha), dim(look.bottom, alpha));
    batch.fill(atlas, x, y, w, b, top);
    batch.fill(atlas, x, y + h - b, w, b, bottom);
    batch.fill_gradient(atlas, (x, y + b, b, h - 2.0 * b), top, bottom);
    batch.fill_gradient(atlas, (x + w - b, y + b, b, h - 2.0 * b), top, bottom);
    batch.fill(atlas, x + b, y + b, w - 2.0 * b, h - 2.0 * b, fill);
}

/// `BoxStyle.render`: the theme's box behind `rect` (`BoxElementImpl`'s own rectangle, with its padding), `alpha` being
/// `IDisplayHelper.backgroundOpacity` times the overlay's Background Opacity. Draws nothing at none.
pub fn draw_background(game: &Game<'_>, batch: &mut HudBatch, theme: &Theme, rect: [f32; 4], has_icon: bool, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let style = &theme.tooltip_style;
    let chosen = if has_icon { style.with_icon.as_ref().or(style.plain.as_ref()) } else { style.plain.as_ref() };
    let Some((back, frame)) = chosen else { return };
    let a = (alpha.min(1.0) * 255.0) as u8;
    let [x, y, w, h] = rect.map(f32::round);
    let outer = if style.tooltip { [x - 9.0, y - 9.0, w + 18.0, h + 18.0] } else { [x, y, w, h] };
    if back.draw(game, batch, outer, a) {
        if let Some(frame) = frame {
            frame.draw(game, batch, outer, a);
        }
    } else if let Some(look) = builtin_look(&back.id) {
        draw_look(game, batch, &look, [x, y, w, h], alpha.min(1.0));
    } else if let Some(look) = builtin_look("jade:dark") {
        // A sprite neither the atlas nor Jade's own: the box stays readable.
        draw_look(game, batch, &look, [x, y, w, h], alpha.min(1.0));
    }
}

/// `BoxElementImpl.drawBoxProgressBar`: along the box's foot, in the style's colour for TITLE or FAILURE.
pub fn draw_progress(game: &Game<'_>, batch: &mut HudBatch, theme: &Theme, [x, y, w, h]: [f32; 4], progress: f32, failed: bool, alpha: f32) {
    let style = &theme.tooltip_style;
    let base = if failed { style.progress.failure } else { style.progress.title };
    let [up, down, left, right] = style.progress_offset;
    let bottom = y + h;
    let width = (w + right - left) * progress.clamp(0.0, 1.0);
    batch.fill(&game.gui.atlas, x + left, bottom - 1.0 + up, width, 1.0 + down - up, dim(base, alpha.clamp(0.0, 1.0)));
}

/// The themes, sorted as Java's `TreeMap<Identifier, Theme>` is (by path, then namespace).
struct Loaded {
    themes: Vec<Rc<Theme>>,
}

/// Bumped when the resources change (`native_client_resources_reloaded`).
static RELOADS: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static LOADED: RefCell<Option<(u32, Rc<Loaded>)>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<(u32, u32, Rc<Theme>)>> = const { RefCell::new(None) };
}

/// `native_client_resources_reloaded`, and Reload Plugins: the themes are read again when next wanted.
pub fn reloaded() {
    RELOADS.fetch_add(1, Ordering::Relaxed);
}

/// Every `assets/<namespace>/jade_themes/**.json` under the game's resource root, the packs' and content mods' included.
fn pack_theme_files(root: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    let Ok(namespaces) = std::fs::read_dir(root.join("assets")) else {
        return out;
    };
    for ns in namespaces.flatten() {
        let name = ns.file_name().to_string_lossy().into_owned();
        let base = ns.path().join("jade_themes");
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "json")
                    && let Ok(rel) = p.strip_prefix(&base)
                {
                    let rel = rel.with_extension("").to_string_lossy().replace('\\', "/");
                    out.push((format!("{name}:{rel}"), p));
                }
            }
        }
    }
    out
}

/// `ThemeHelper.apply`: the jar's themes, those on the resource root over them, and a new theme that asks for it turned
/// on (autoEnable, with its changeOpacity).
fn load(game: &Game<'_>) -> Loaded {
    let root = game.options.resources.clone();
    let mut themes: Vec<Rc<Theme>> = JAR_THEMES.iter().filter_map(|(id, text)| Theme::parse(&root, id, text).map(Rc::new)).collect();
    for (id, path) in pack_theme_files(&root) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Some(theme) = Theme::parse(&root, &id, &text) else { continue };
        themes.retain(|t| t.id != id);
        themes.push(Rc::new(theme));
    }
    let key = |t: &Theme| {
        let (ns, path) = t.id.split_once(':').unwrap_or(("", t.id.as_str()));
        (path.to_owned(), ns.to_owned())
    };
    themes.sort_by_key(|t| key(&**t));
    // Jade.history().themesHash: which themes have been seen, so that one turns on only when it is new.
    let seen = game.native_setting(jade::ID, "themes_seen").unwrap_or_default();
    let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
    let joined = ids.join("\n");
    if seen != joined {
        let known: Vec<&str> = seen.lines().collect();
        if let Some(new) = themes.iter().find(|t| t.auto_enable && !known.contains(&t.id.as_str())) {
            game.native_set_setting(jade::ID, "overlay_theme", &stored_main(&new.main));
            game.native_set_setting(jade::ID, "theme_style", if new.style.is_empty() { "default" } else { &new.style });
            if new.change_opacity != 0.0 {
                game.native_set_setting(jade::ID, "overlay_alpha", &format!("{}", (new.change_opacity * 100.0).round() as u32));
            }
            jade::settings_changed();
        }
        game.native_set_setting(jade::ID, "themes_seen", &joined);
    }
    Loaded { themes }
}

fn loaded(game: &Game<'_>) -> Rc<Loaded> {
    let at = RELOADS.load(Ordering::Relaxed);
    if let Some(l) = LOADED.with_borrow(|l| l.as_ref().filter(|(r, _)| *r == at).map(|(_, l)| l.clone())) {
        return l;
    }
    let fresh = Rc::new(load(game));
    LOADED.with_borrow_mut(|l| *l = Some((at, fresh.clone())));
    fresh
}

/// A main id as the setting keeps it.
fn stored_main(main: &str) -> String {
    main.strip_prefix("jade:").unwrap_or(main).to_owned()
}

/// `overlay.getTheme()`: the theme the settings name, or the same theme's default style, or Jade's default. Costs two
/// counters a frame once found.
pub fn current(game: &Game<'_>) -> Rc<Theme> {
    let epoch = jade::settings_epoch();
    let reloads = RELOADS.load(Ordering::Relaxed);
    if let Some(t) = ACTIVE.with_borrow(|a| a.as_ref().filter(|(e, r, _)| *e == epoch && *r == reloads).map(|(_, _, t)| t.clone())) {
        return t;
    }
    let themes = loaded(game);
    // Loading can turn a new theme on, which moves the epoch.
    let epoch = jade::settings_epoch();
    let main = jade::opt(game, "overlay_theme");
    let main = if main.contains(':') { main } else { format!("jade:{main}") };
    let style = jade::opt(game, "theme_style");
    let full = if style == "default" || style.is_empty() { main.clone() } else { format!("{main}/{style}") };
    let find = |id: &str| themes.themes.iter().find(|t| t.id == id).cloned();
    // A theme is always found: the jar's own are parsed from the constants above.
    let theme = find(&full).or_else(|| find(&main)).or_else(|| find(DEFAULT_THEME)).or_else(|| themes.themes.first().cloned());
    let theme = theme.unwrap_or_else(|| Rc::new(fallback(game)));
    ACTIVE.with_borrow_mut(|a| *a = Some((epoch, reloads, theme.clone())));
    theme
}

fn fallback(game: &Game<'_>) -> Theme {
    Theme {
        id: DEFAULT_THEME.to_owned(),
        main: DEFAULT_THEME.to_owned(),
        style: String::new(),
        style_name: "jade.default".to_owned(),
        tooltip_style: BoxStyle::parse(&game.options.resources, &serde_json::json!({"sprite": "jade:dark", "tooltip": true})),
        colors: Palette::DEFAULT,
        shadow: true,
        title_code: "§f".to_owned(),
        mod_code: "§9§o".to_owned(),
        amount: 0xFFFF_FFFF,
        change_opacity: 0.0,
        auto_enable: false,
        sneaky: Sneaky::parse(None),
        icon_slot: None,
        icon_slot_inflation: 0.0,
        sprite_mapping: HashMap::new(),
    }
}

/// `jade.theme.<namespace>.<path>`: Jade's own names are in the jar's language files (`jade::ensure_lang`); these are for
/// the moment before that, and a theme of a pack's with no name.
fn theme_name(game: &Game<'_>, main: &str) -> String {
    let (ns, path) = main.split_once(':').unwrap_or(("jade", main));
    let fallback = match main {
        "jade:waila" => "Classic".to_owned(),
        "jade:dark" => "Dark".to_owned(),
        "jade:top" => "One Probe".to_owned(),
        "jade:create" => "Create".to_owned(),
        _ => {
            let mut c = path.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        }
    };
    game.translate(&format!("jade.theme.{ns}.{path}"), &fallback)
}

/// The Theme button's label.
pub fn main_label(game: &Game<'_>) -> String {
    theme_name(game, &current(game).main)
}

/// How many styles the current theme has (one: the Theme Style button says Unavailable).
pub fn style_count(game: &Game<'_>) -> usize {
    let main = current(game).main.clone();
    loaded(game).themes.iter().filter(|t| t.main == main).count()
}

/// The Theme Style button's label.
pub fn style_label(game: &Game<'_>) -> String {
    if style_count(game) <= 1 {
        return game.translate("jade.unavailable", "Unavailable");
    }
    let key = current(game).style_name.clone();
    let fallback = match key.as_str() {
        "jade.default" => "Default",
        "jade.slim" => "Slim",
        other => other,
    };
    game.translate(&key, fallback)
}

/// The Theme button (WailaConfigScreen's `overlay_theme` cycle): the next theme's default style, with its changeOpacity.
pub fn cycle_main(game: &Game<'_>, back: bool) {
    let themes = loaded(game);
    let mains: Vec<&Rc<Theme>> = themes.themes.iter().filter(|t| t.style.is_empty()).collect();
    if mains.is_empty() {
        return;
    }
    let now = current(game).main.clone();
    let at = mains.iter().position(|t| t.id == now).unwrap_or(0);
    let next = if back { mains[(at + mains.len() - 1) % mains.len()] } else { mains[(at + 1) % mains.len()] };
    game.native_set_setting(jade::ID, "overlay_theme", &stored_main(&next.main));
    game.native_set_setting(jade::ID, "theme_style", "default");
    if next.change_opacity != 0.0 {
        game.native_set_setting(jade::ID, "overlay_alpha", &format!("{}", (next.change_opacity * 100.0).round() as u32));
    }
    jade::settings_changed();
}

/// The Theme Style button: the current theme's next style (`valuesSupplier`).
pub fn cycle_style(game: &Game<'_>, back: bool) {
    let themes = loaded(game);
    let theme = current(game);
    let styles: Vec<&Rc<Theme>> = themes.themes.iter().filter(|t| t.main == theme.main).collect();
    if styles.len() <= 1 {
        return;
    }
    let at = styles.iter().position(|t| t.id == theme.id).unwrap_or(0);
    let next = if back { styles[(at + styles.len() - 1) % styles.len()] } else { styles[(at + 1) % styles.len()] };
    game.native_set_setting(jade::ID, "theme_style", if next.style.is_empty() { "default" } else { &next.style });
    jade::settings_changed();
}
