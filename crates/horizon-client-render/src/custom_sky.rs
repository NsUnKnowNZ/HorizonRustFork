//! OptiFine custom skies (`horizon_optifine::sky`) as the game draws them: a pack's layers with
//! their decoded textures, read once with the pack, and each frame's choice of layers, their
//! brightness and rotation ([`CustomSky::draws`]), which `gfx::custom_sky` draws over the
//! vanilla sky. A pack without `optifine/sky` gives an empty [`CustomSky`] and the sky stays
//! vanilla.

use std::path::Path;

use glam::{Mat4, Vec3};
use horizon_optifine::sky::{Blend, Conditions, Skies, SkyLayer, Transition};

/// A layer's texture, RGBA8.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The pack's layers, by world, with one texture per layer (`images[i]` for the `i`th layer
/// over all worlds, in `worlds` order).
#[derive(Default)]
pub struct CustomSky {
    worlds: Vec<(i32, Vec<SkyLayer>)>,
    pub images: Vec<Image>,
}

/// One layer of a frame: which texture, with which blend, how bright and turned how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    pub image: u16,
    pub blend: Blend,
    pub brightness: f32,
    /// Camera-relative world from the skybox's space.
    pub rotation: Mat4,
}

/// The eased brightness of each layer, kept by the game between frames.
#[derive(Default)]
pub struct State {
    world: Option<i32>,
    eased: Vec<Transition>,
}

/// Textures past this many texels in all are left out (a skybox is a few megapixels at most).
const TEXEL_BUDGET: u64 = 64 << 20;

impl CustomSky {
    /// The custom skies of the overlay at `root`. A layer whose texture does not decode is
    /// dropped with a warning.
    pub fn load(root: &Path) -> Self {
        let mut skies = Skies::scan(root);
        // FabricSkyboxes / Nuit skies (`assets/<ns>/sky/*.json`) join the OptiFine ones, after
        // them, in their priority order.
        for (world, layer) in fabric_skies(root) {
            skies.worlds.entry(world).or_default().layers.push(layer);
        }
        let mut out = Self::default();
        let mut texels = 0u64;
        for (world, sky) in skies.worlds {
            let mut layers = Vec::new();
            for layer in sky.layers {
                match crate::hud::decode(&root.join(&layer.source)) {
                    Ok((width, height, rgba))
                        if texels + u64::from(width) * u64::from(height) <= TEXEL_BUDGET =>
                    {
                        texels += u64::from(width) * u64::from(height);
                        out.images.push(Image {
                            width,
                            height,
                            rgba,
                        });
                        layers.push(layer);
                    }
                    Ok(_) => log::warn!("{}: past the custom sky's texture budget", layer.source),
                    Err(e) => log::warn!("{}: {e:#}", layer.source),
                }
            }
            if !layers.is_empty() {
                out.worlds.push((world, layers));
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }

    /// Whether `world` has layers (`0` the Overworld, `1` the End).
    pub fn has(&self, world: i32) -> bool {
        self.worlds.iter().any(|(w, _)| *w == world)
    }

    /// This frame's layers of `world` into `out` (cleared first), in drawing order: those whose
    /// conditions hold, eased over their `transition` by `seconds` since the last frame, at the
    /// brightness of their fade times. `sun_turns` is the sun's angle over 360° (`0` at noon).
    pub fn draws(
        &self,
        world: i32,
        conditions: &Conditions,
        sun_turns: f32,
        seconds: f32,
        state: &mut State,
        out: &mut Vec<Draw>,
    ) {
        out.clear();
        let mut first = 0;
        let mut found = None;
        for (w, layers) in &self.worlds {
            if *w == world {
                found = Some(layers);
                break;
            }
            first += layers.len();
        }
        let Some(layers) = found else {
            state.world = None;
            return;
        };
        // A new world (or pack) starts each layer where its conditions put it: no fade in.
        let snap = state.world != Some(world) || state.eased.len() != layers.len();
        if snap {
            state.world = Some(world);
            state.eased.clear();
            state.eased.resize(layers.len(), Transition::default());
        }
        for (i, (layer, eased)) in layers.iter().zip(&mut state.eased).enumerate() {
            let target = if layer.shows(conditions) { 1.0 } else { 0.0 };
            let shown = if snap {
                eased.value = target;
                target
            } else {
                eased.step(target, seconds, layer.transition)
            };
            let brightness = shown * layer.brightness(conditions.day_time);
            if brightness <= 0.0 {
                continue;
            }
            let turns = layer.turns(conditions.day_time, sun_turns);
            let rotation = if turns == 0.0 {
                Mat4::IDENTITY
            } else {
                Mat4::from_axis_angle(Vec3::from_array(layer.axis), turns * std::f32::consts::TAU)
            };
            out.push(Draw {
                image: (first + i) as u16,
                blend: layer.blend,
                brightness,
                rotation,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_optifine::sky::Now;

    fn pack(files: &[(&str, &str)]) -> (std::path::PathBuf, CustomSky) {
        let root = std::env::temp_dir().join(format!(
            "horizon-custom-sky-{}-{}",
            std::process::id(),
            files.len()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("assets/minecraft/optifine/sky/world0");
        std::fs::create_dir_all(&dir).unwrap();
        let png = {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 6, 4);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&[255, 0, 0, 255].repeat(24))
                .unwrap();
            writer.finish().unwrap();
            bytes
        };
        for (name, text) in files {
            std::fs::write(dir.join(format!("{name}.properties")), text).unwrap();
            std::fs::write(dir.join(format!("{name}.png")), &png).unwrap();
        }
        let sky = CustomSky::load(&root);
        (root, sky)
    }

    fn noon(biome: &str) -> Conditions<'_> {
        Conditions {
            day_time: 6000,
            weather: Now::Clear,
            biome,
            y: 64,
        }
    }

    #[test]
    fn layers_ease_in_at_a_biome_border_but_not_on_arrival() {
        let (root, sky) = pack(&[
            ("sky1", "biomes=desert\ntransition=2"),
            ("sky2", "blend=alpha\nrotate=false"),
        ]);
        let _ = std::fs::remove_dir_all(root);
        assert!(sky.has(0) && !sky.has(1));
        assert_eq!(sky.images[0].width, 6);
        let (mut state, mut out) = (State::default(), Vec::new());
        sky.draws(
            0,
            &noon("minecraft:desert"),
            0.0,
            0.05,
            &mut state,
            &mut out,
        );
        assert_eq!(
            out.iter().map(|d| d.brightness).collect::<Vec<_>>(),
            [1.0, 1.0]
        );
        // Out of the desert: the first layer fades over its two seconds.
        sky.draws(0, &noon("minecraft:plains"), 0.0, 0.5, &mut state, &mut out);
        assert_eq!(out[0].brightness, 0.75);
        assert_eq!(out[1].blend, Blend::Alpha);
        sky.draws(0, &noon("minecraft:plains"), 0.0, 2.0, &mut state, &mut out);
        assert_eq!(out.len(), 1, "a dark layer is not drawn");
        assert_eq!(out[0].image, 1);
        // Into the End, which has no layers, and back: no fade.
        sky.draws(1, &noon("minecraft:desert"), 0.0, 0.0, &mut state, &mut out);
        assert!(out.is_empty());
        sky.draws(0, &noon("minecraft:desert"), 0.0, 0.0, &mut state, &mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn a_layer_turns_west_like_the_sun() {
        let (root, sky) = pack(&[("sky1", "")]);
        let _ = std::fs::remove_dir_all(root);
        let (mut state, mut out) = (State::default(), Vec::new());
        // A quarter turn after noon (sunset): the skybox's zenith has gone down to the west.
        let c = Conditions {
            day_time: 12000,
            ..noon("minecraft:plains")
        };
        sky.draws(0, &c, 0.25, 0.0, &mut state, &mut out);
        let up = out[0].rotation.transform_vector3(Vec3::Y);
        assert!((up - Vec3::NEG_X).length() < 1e-5, "{up}");
    }
}

/// The skies of FabricSkyboxes and its successor Nuit (`assets/fabricskyboxes/sky/*.json`,
/// `assets/nuit/sky/*.json`), as OptiFine layers: the textured kinds whose six faces are one
/// 3x2 image (`single-sprite-square-textured`, OptiFine's own layout) or six images
/// (`square-textured`, joined here into that layout is not done: those are skipped), with their
/// fade times (ticks), blend, rotation, weathers, worlds, biomes and heights. `monocolor`
/// layers and the decorations switches (hide the sun, moon or stars) are not read. Written from
/// the format's public documentation (the FabricSkyboxes wiki, schema version 2).
pub fn fabric_skies(root: &Path) -> Vec<(i32, SkyLayer)> {
    use horizon_optifine::matchers::{IdList, RangeList};
    use horizon_optifine::sky::{Fade, Weather};
    use serde_json::Value;
    let mut found: Vec<(i64, String, i32, SkyLayer)> = Vec::new();
    for ns in ["fabricskyboxes", "nuit"] {
        let dir = root.join("assets").join(ns).join("sky");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".json") {
                continue;
            }
            let file = format!("assets/{ns}/sky/{name}");
            let Some(v) = std::fs::read(entry.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                log::warn!("{file}: not JSON; skipped");
                continue;
            };
            let kind = v["type"].as_str().unwrap_or("");
            if kind != "single-sprite-square-textured" {
                if kind != "monocolor" {
                    log::info!("{file}: sky type `{kind}` is not read yet; skipped");
                }
                continue;
            }
            // `namespace:path` of a texture: `assets/<namespace>/<path>`.
            let Some(texture) = v["texture"].as_str().or_else(|| v["textures"]["texture"].as_str())
            else {
                continue;
            };
            let (tns, tpath) = texture.split_once(':').unwrap_or(("minecraft", texture));
            let source = format!("assets/{tns}/{tpath}");
            if !root.join(&source).is_file() {
                log::warn!("{file}: no texture {source}");
                continue;
            }
            let props = &v["properties"];
            let fade = &props["fade"];
            let tick = |k: &str| fade[k].as_i64().map(|t| t.rem_euclid(24_000));
            let fade = match (tick("startFadeIn"), tick("endFadeIn"), tick("startFadeOut"), tick("endFadeOut")) {
                (Some(start_in), Some(end_in), Some(start_out), Some(end_out)) => Some(Fade {
                    start_in,
                    end_in,
                    start_out,
                    end_out,
                }),
                _ if fade["alwaysOn"].as_bool() == Some(true) => None,
                _ => None,
            };
            let blend = match v["blend"]["type"].as_str().unwrap_or("add") {
                "alpha" | "normal" | "decorations" => Blend::Alpha,
                "add" => Blend::Add,
                "subtract" => Blend::Subtract,
                "multiply" => Blend::Multiply,
                "screen" | "lighten" => Blend::Screen,
                "replace" => Blend::Replace,
                "dodge" => Blend::Dodge,
                "burn" | "darken" => Blend::Burn,
                "overlay" => Blend::Overlay,
                other => {
                    log::info!("{file}: blend `{other}` drawn as alpha");
                    Blend::Alpha
                }
            };
            // Turning: with the sun (`skyboxRotation`), and/or at `rotationSpeed{X,Y,Z}` turns a
            // day around those axes (only the largest is kept).
            let rot = &props["rotation"];
            let speeds = ["rotationSpeedX", "rotationSpeedY", "rotationSpeedZ"]
                .map(|k| rot[k].as_f64().unwrap_or(0.0) as f32);
            let follows_sun = rot["skyboxRotation"].as_bool().unwrap_or(true);
            let (axis_i, speed) = speeds
                .iter()
                .copied()
                .enumerate()
                .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
                .unwrap_or((2, 0.0));
            let (rotate, speed, axis) = if speed != 0.0 {
                let mut axis = [0.0; 3];
                axis[axis_i] = 1.0;
                (true, speed, axis)
            } else {
                (follows_sun, 1.0, [0.0, 0.0, 1.0])
            };
            let cond = &v["conditions"];
            let strings = |key: &str| -> Vec<String> {
                cond[key]
                    .as_array()
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default()
            };
            let weathers = strings("weather");
            let weather = if weathers.is_empty() {
                Weather { clear: true, rain: true, thunder: true }
            } else {
                Weather {
                    clear: weathers.iter().any(|w| w == "clear"),
                    rain: weathers.iter().any(|w| w == "rain" || w == "snow"),
                    thunder: weathers.iter().any(|w| w == "thunder"),
                }
            };
            let mut worlds: Vec<i32> = strings("worlds")
                .iter()
                .chain(strings("dimensions").iter())
                .filter_map(|w| match w.as_str() {
                    "minecraft:overworld" => Some(0),
                    "minecraft:the_end" => Some(1),
                    _ => None,
                })
                .collect();
            if worlds.is_empty() {
                worlds.push(0);
            }
            let biomes = strings("biomes");
            let heights: Vec<String> = cond["yRanges"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|r| Some(format!("{}-{}", r["min"].as_f64()? as i32, r["max"].as_f64()? as i32)))
                        .collect()
                })
                .unwrap_or_default();
            let transition = v["transitionInDuration"]
                .as_f64()
                .map_or(1.0, |ticks| (ticks / 20.0) as f32);
            let layer = SkyLayer {
                file: file.clone(),
                source,
                fade,
                blend,
                rotate,
                speed,
                axis,
                days: None,
                days_loop: 8,
                weather,
                biomes: (!biomes.is_empty()).then(|| IdList::parse(&biomes.join(" "))),
                heights: (!heights.is_empty()).then(|| RangeList::parse(&heights.join(" "))).flatten(),
                transition,
            };
            let priority = props["priority"].as_i64().unwrap_or(0);
            for world in worlds {
                found.push((priority, file.clone(), world, layer.clone()));
            }
        }
    }
    found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    if !found.is_empty() {
        log::info!("Horizon: {} FabricSkyboxes/Nuit sky layers", found.len());
    }
    found.into_iter().map(|(_, _, w, l)| (w, l)).collect()
}
