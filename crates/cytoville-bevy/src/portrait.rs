// SPDX-License-Identifier: AGPL-3.0-or-later
//! Native cell rendering: geometry, expressed organelles and membrane phenotype.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use cytoville_core::{Actor, Look};
#[derive(PartialEq)]
struct Appearance {
    specimen: String,
    phase: String,
    organelles: Vec<String>,
    membrane: String,
    accent: String,
    pattern: String,
}
impl Appearance {
    fn of(a: &Actor, look: &Look) -> Self {
        Self {
            specimen: a.specimen.clone(),
            phase: a.phase.clone(),
            organelles: a.organelles.clone(),
            membrane: look.membrane.clone(),
            accent: look.accent.clone(),
            pattern: look.pattern.clone(),
        }
    }
}
/// Keep only the two current appearances. Travel, names and resource changes do
/// not rerasterize or upload an unchanged 256×256 texture.
#[derive(Resource, Default)]
pub struct Cache([Option<(Appearance, Handle<Image>)>; 2]);
impl Cache {
    pub fn get(
        &mut self,
        seat: usize,
        a: &Actor,
        look: &Look,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let key = Appearance::of(a, look);
        let slot = &mut self.0[seat];
        if let Some((previous, handle)) = slot
            && *previous == key
        {
            return handle.clone();
        }
        let handle = images.add(image(a, look));
        *slot = Some((key, handle.clone()));
        handle
    }
}
fn rgb(s: &str, fallback: [u8; 3]) -> [u8; 3] {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return fallback;
    }
    let mut c = [0; 3];
    for i in 0..3 {
        let Ok(v) = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16) else {
            return fallback;
        };
        c[i] = v;
    }
    c
}
pub fn image(a: &Actor, look: &Look) -> Image {
    const N: usize = 256;
    let mut data = vec![0; N * N * 4];
    let body = rgb(&look.membrane, [91, 219, 170]);
    let accent = rgb(&look.accent, [250, 180, 80]);
    let inside = |x: f32, y: f32| -> bool {
        match a.specimen.as_str() {
            "amoeba" => {
                let t = y.atan2(x);
                let r = 0.65 + 0.13 * (t * 5.0).sin() + 0.06 * (t * 3.0).cos();
                x * x + y * y < r * r
            }
            "stentor" => y.abs() < 0.8 && x.abs() < (0.22 + (0.8 - y) * 0.3),
            "vorticella" => {
                y < 0.37 && ((x / 0.68).powi(2) + ((y + 0.2) / 0.48).powi(2) < 1.0)
                    || (0.37..0.85).contains(&y) && (x - 0.07 * (y * 18.0).sin()).abs() < 0.035
            }
            "diatom" => x.abs() < 0.73 && y.abs() < 0.40 && (x.abs() < 0.58 || y.abs() < 0.30),
            "didinium" => (x / 0.58).powi(2) + (y / 0.66).powi(2) < 1.0,
            "paramecium" => {
                ((x + 0.08 * (y * 4.0).sin()) / 0.45).powi(2) + (y / 0.83).powi(2) < 1.0
            }
            _ => (x / 0.82).powi(2) + (y / 0.39).powi(2) < 1.0,
        }
    };
    for py in 0..N {
        for px in 0..N {
            let x = (px as f32 - 128.0) / 112.0;
            let y = (py as f32 - 128.0) / 112.0;
            let index = (py * N + px) * 4;
            let cell = inside(x, y);
            let edge = cell
                && (!inside(x + 0.025, y)
                    || !inside(x - 0.025, y)
                    || !inside(x, y + 0.025)
                    || !inside(x, y - 0.025));
            let mut color = if edge {
                [body[0], body[1], body[2], 255]
            } else if cell {
                [body[0] / 2, body[1] / 2, body[2] / 2, 210]
            } else {
                [0; 4]
            };
            if cell {
                if look.pattern == "spots" && ((x * 13.0).sin() * (y * 13.0).sin()) > 0.72
                    || look.pattern == "stripes" && (x * 32.0 + y * 12.0).sin() > 0.82
                {
                    color = [body[0], body[1], body[2], 150];
                }
                if a.specimen == "diatom" && (x * 55.0).sin() > 0.85 {
                    color = [accent[0], accent[1], accent[2], 230];
                }
                for (i, id) in a.organelles.iter().enumerate() {
                    let t = i as f32 * 2.399;
                    let ox = 0.48 * t.cos();
                    let oy = 0.27 * t.sin();
                    let r = if id == "vacuole" { 0.11 } else { 0.065 };
                    if (x - ox).powi(2) + (y - oy).powi(2) < r * r {
                        color = match id.as_str() {
                            "chloroplast" => [45, 210, 95, 255],
                            "vacuole" => [125, 218, 250, 255],
                            "lysosome" => [199, 125, 244, 255],
                            "storage" => [245, 240, 205, 255],
                            _ => [accent[0], accent[1], accent[2], 240],
                        };
                    }
                }
                let nuclei = if a.phase == "G1" { 1 } else { 2 };
                for i in 0..nuclei {
                    let nx = if nuclei == 1 {
                        0.0
                    } else {
                        i as f32 * 0.28 - 0.14
                    };
                    let d = (x - nx).powi(2) + y * y;
                    if d < 0.15f32.powi(2) {
                        color = if d < 0.055f32.powi(2) {
                            [29, 94, 150, 255]
                        } else {
                            [181, 226, 251, 255]
                        };
                    }
                }
            }
            if !cell
                && (a.has("cilia") || a.has("basal") || a.specimen == "didinium")
                && ((x * 0.93, y * 0.93).0 != 0.0)
                && inside(x * 0.9, y * 0.9)
                && (y.atan2(x) * 40.0).sin() > 0.8
            {
                color = [body[0], body[1], body[2], 210];
            }
            if a.has("flagellum")
                && x > 0.7
                && x < 1.08
                && (y - 0.16 * ((x - 0.7) * 14.0).sin()).abs() < 0.015
            {
                color = [body[0], body[1], body[2], 240];
            }
            data[index..index + 4].copy_from_slice(&color);
        }
    }
    Image::new(
        Extent3d {
            width: N as u32,
            height: N as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_ignores_gameplay_fields_but_tracks_visible_appearance() {
        let rt = cytoville_runtime::Runtime::new(
            cytoville_core::Content::builtin(),
            "euglena",
            "fresh-daughter",
            "standard",
            7,
            false,
        )
        .unwrap();
        let mut actor = rt.game.actors[0].clone();
        let mut look = rt.game.looks[0].clone();
        let mut cache = Cache::default();
        let mut images = Assets::default();
        let original = cache.get(0, &actor, &look, &mut images);
        actor.location = "university".into();
        actor.atp -= 1;
        actor.name = "Renamed".into();
        look.name = "Renamed".into();
        assert_eq!(original, cache.get(0, &actor, &look, &mut images));
        look.pattern = "spots".into();
        let patterned = cache.get(0, &actor, &look, &mut images);
        assert_ne!(original, patterned);
        actor.phase = "G2".into();
        let divided = cache.get(0, &actor, &look, &mut images);
        assert_ne!(patterned, divided);
        actor.organelles.push("vacuole".into());
        assert_ne!(divided, cache.get(0, &actor, &look, &mut images));
    }
}
