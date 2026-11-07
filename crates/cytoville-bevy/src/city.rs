// SPDX-License-Identifier: AGPL-3.0-or-later
//! Native biofilm illustration, cached separately from commands and UI text.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use cytoville_core::Content;

const W: usize = 1024;
const H: usize = 900;
type Rgba = [u8; 4];

pub fn position(id: &str, content: &Content) -> Vec2 {
    let (x, y) = match id {
        "home" => (38., 40.),
        "university" => (3., 28.),
        "solar" => (26., 12.),
        "checkpoint" => (49., 28.),
        "security" => (73., 12.),
        "detritus" => (3., 53.),
        "boutique" => (73., 53.),
        "rift" => (38., 62.),
        "gym" => (6., 75.),
        "diner" => (70., 75.),
        _ => content
            .locations
            .get(id)
            .map(|l| (l.x as f32, l.y as f32))
            .unwrap_or((38., 40.)),
    };
    Vec2::new(x, y)
}

pub fn destination(id: &str, content: &Content) -> Vec2 {
    // Keep the central cell illustration clear of the registry's two-line card.
    if id == "home" {
        return Vec2::new(44., 34.5);
    }
    position(id, content) + Vec2::new(11.5, -5.5)
}

fn pigment(hex: &str) -> Rgba {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x73d5b4);
    [(value >> 16) as u8, (value >> 8) as u8, value as u8, 255]
}

struct Canvas(Vec<u8>);
impl Canvas {
    fn pixel(&mut self, x: i32, y: i32, c: Rgba, coverage: f32) {
        if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
            return;
        }
        let k = (y as usize * W + x as usize) * 4;
        let a = coverage.clamp(0., 1.) * c[3] as f32 / 255.;
        for (i, channel) in c.iter().take(3).enumerate() {
            self.0[k + i] = (self.0[k + i] as f32 * (1. - a) + *channel as f32 * a) as u8;
        }
        self.0[k + 3] = 255;
    }
    fn ellipse(&mut self, at: Vec2, radius: Vec2, color: Rgba, ring: bool) {
        for y in (at.y - radius.y - 2.) as i32..=(at.y + radius.y + 2.) as i32 {
            for x in (at.x - radius.x - 2.) as i32..=(at.x + radius.x + 2.) as i32 {
                let distance = ((Vec2::new(x as f32, y as f32) - at) / radius).length();
                let edge = radius.min_element();
                let alpha = if ring {
                    (2. - (distance - 1.).abs() * edge).clamp(0., 1.)
                } else {
                    ((1. - distance) * edge + 0.5).clamp(0., 1.)
                };
                if alpha > 0. {
                    self.pixel(x, y, color, alpha);
                }
            }
        }
    }
    fn line(&mut self, from: Vec2, to: Vec2, width: f32, color: Rgba) {
        let length = from.distance(to).ceil().max(1.) as usize;
        for i in 0..=length {
            self.ellipse(
                from.lerp(to, i as f32 / length as f32),
                Vec2::splat(width),
                color,
                false,
            );
        }
    }
    fn path(&mut self, from: Vec2, to: Vec2, bend: f32, width: f32, color: Rgba) {
        let control = (from + to) * 0.5 + Vec2::new(bend, bend * 0.4);
        let mut last = from;
        for i in 1..=80 {
            let t = i as f32 / 80.;
            let point = from * (1. - t).powi(2) + control * 2. * t * (1. - t) + to * t * t;
            self.line(last, point, width, color);
            last = point;
        }
    }
    fn icon(&mut self, id: &str, at: Vec2, color: Rgba) {
        let dark = [color[0] / 4, color[1] / 4, color[2] / 4, 255];
        let bright = [
            color[0].saturating_add(30),
            color[1].saturating_add(30),
            color[2].saturating_add(30),
            255,
        ];
        let p = |x, y| at + Vec2::new(x, y);
        match id {
            "university" | "checkpoint" => {
                self.path(p(-35., -5.), p(35., -5.), -30., 2., color);
                for x in [-25., -12., 0., 12., 25.] {
                    self.line(p(x, -4.), p(x, 25.), 3., color);
                }
                self.line(p(-37., 27.), p(37., 27.), 3., bright);
                self.ellipse(p(0., -16.), Vec2::splat(5.), bright, false);
            }
            "solar" => {
                self.ellipse(p(0., -14.), Vec2::splat(13.), bright, false);
                for i in 0..8 {
                    let v = Vec2::from_angle(i as f32 * std::f32::consts::TAU / 8.);
                    self.line(p(0., -14.) + v * 18., p(0., -14.) + v * 25., 1.5, color);
                }
                for x in [-24., 0., 24.] {
                    self.line(p(x - 6., 15.), p(x + 6., 30.), 7., color);
                    self.line(p(x - 4., 15.), p(x + 7., 26.), 1., dark);
                }
            }
            "detritus" => {
                for (x, y) in [(-24., 8.), (0., -3.), (24., 13.)] {
                    self.line(p(x, y), p(x, 28.), 9., color);
                    self.ellipse(p(x, y), Vec2::new(9., 4.), bright, false);
                }
                for (x, y) in [(-16., -14.), (7., -25.), (24., -8.)] {
                    self.ellipse(p(x, y), Vec2::splat(3.), bright, true);
                }
            }
            "security" => {
                self.ellipse(at, Vec2::new(29., 33.), dark, false);
                self.path(p(-27., -24.), p(0., 33.), -18., 2., color);
                self.path(p(27., -24.), p(0., 33.), 18., 2., color);
                self.line(p(-27., -24.), p(0., -32.), 2., color);
                self.line(p(0., -32.), p(27., -24.), 2., color);
                self.line(p(-13., 0.), p(-2., 11.), 3., bright);
                self.line(p(-2., 11.), p(17., -13.), 3., bright);
            }
            "boutique" => {
                for i in 0..5 {
                    self.path(
                        p(-30., -20. + i as f32 * 10.),
                        p(30., -20. + i as f32 * 10.),
                        -13.,
                        3.,
                        color,
                    );
                }
                self.ellipse(p(30., -30.), Vec2::splat(4.), bright, false);
                self.ellipse(p(-29., -28.), Vec2::splat(3.), bright, false);
            }
            "diner" => {
                self.ellipse(p(0., 10.), Vec2::new(34., 19.), color, false);
                self.ellipse(p(0., 0.), Vec2::new(34., 9.), dark, false);
                for (x, y) in [(-16., -6.), (2., -11.), (17., -5.)] {
                    self.ellipse(p(x, y), Vec2::splat(6.), bright, false);
                }
                self.line(p(-24., 31.), p(24., 31.), 2., bright);
            }
            "gym" => {
                self.ellipse(at, Vec2::splat(29.), color, true);
                self.ellipse(at, Vec2::splat(21.), dark, true);
                self.line(p(-26., 0.), p(26., 0.), 3., bright);
                for x in [-24., 24.] {
                    self.line(p(x, -10.), p(x, 10.), 5., color);
                }
            }
            "rift" => {
                for i in 0..3 {
                    self.ellipse(
                        at,
                        Vec2::new(12. + i as f32 * 10., 29. - i as f32 * 4.),
                        [color[0], color[1], color[2], 170],
                        true,
                    );
                }
                self.line(p(0., -35.), p(0., 35.), 1., bright);
            }
            _ => {
                self.ellipse(at, Vec2::new(36., 28.), dark, false);
                self.ellipse(at, Vec2::new(36., 28.), color, true);
                self.ellipse(p(-4., 0.), Vec2::splat(12.), bright, false);
                self.ellipse(p(-4., 0.), Vec2::splat(6.), dark, false);
                for (x, y) in [(-23., -7.), (18., 11.), (17., -13.)] {
                    self.ellipse(p(x, y), Vec2::new(6., 3.), color, false);
                }
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct Artwork {
    key: String,
    handle: Option<Handle<Image>>,
}
impl Artwork {
    pub fn get(&mut self, c: &Content, images: &mut Assets<Image>) -> Handle<Image> {
        // Includes custom UMS geometry and colours; save/load may replace content.
        let key = c
            .location_order
            .iter()
            .map(|id| {
                let l = &c.locations[id];
                format!("{id}:{}:{}:{};", l.x, l.y, l.color)
            })
            .collect::<String>();
        if self.key == key
            && let Some(handle) = &self.handle
        {
            return handle.clone();
        }
        let mut canvas = Canvas(vec![0; W * H * 4]);
        for y in 0..H {
            for x in 0..W {
                let wave = ((x as f32 * 0.013).sin() * (y as f32 * 0.008).cos() + 1.) * 2.;
                let glow = (1.
                    - ((x as f32 / W as f32 - 0.48).powi(2) + (y as f32 / H as f32 - 0.4).powi(2))
                        * 2.)
                    .max(0.);
                let k = (y * W + x) * 4;
                canvas.0[k..k + 4].copy_from_slice(&[
                    (7. + glow * 5.) as u8,
                    (20. + glow * 18. + wave) as u8,
                    (28. + glow * 18. + wave) as u8,
                    255,
                ]);
            }
        }
        // Fixed-seed particulate field; no random work per frame.
        for i in 0..130 {
            let p = Vec2::new(((i * 317 + 71) % W) as f32, ((i * 197 + 43) % H) as f32);
            canvas.ellipse(
                p,
                Vec2::splat(1. + (i % 4) as f32),
                [83, 173, 155, 35],
                i % 3 == 0,
            );
        }
        let point = |id: &str| destination(id, c) * Vec2::new(W as f32, H as f32) / 100.;
        let roads = [
            ("university", "home", 40.),
            ("home", "boutique", -40.),
            ("boutique", "diner", 50.),
            ("diner", "gym", -30.),
            ("gym", "detritus", -40.),
            ("detritus", "university", 30.),
            ("university", "solar", -25.),
            ("solar", "checkpoint", -20.),
            ("checkpoint", "security", -30.),
            ("security", "boutique", 20.),
            ("detritus", "home", -20.),
            ("checkpoint", "home", 10.),
            ("rift", "home", 0.),
            ("rift", "gym", 20.),
            ("rift", "diner", -20.),
            ("security", "solar", 30.),
        ];
        for (a, b, bend) in roads {
            if c.locations.contains_key(a) && c.locations.contains_key(b) {
                canvas.path(point(a), point(b), bend, 5., [28, 68, 70, 255]);
                canvas.path(point(a), point(b), bend, 1.1, [57, 111, 106, 255]);
            }
        }
        for id in &c.location_order {
            let p = point(id);
            let color = pigment(&c.locations[id].color);
            canvas.ellipse(
                p,
                Vec2::new(68., 48.),
                [color[0], color[1], color[2], 15],
                false,
            );
            canvas.ellipse(p, Vec2::new(50., 37.), [12, 32, 39, 255], false);
            canvas.ellipse(
                p,
                Vec2::new(51., 38.),
                [color[0], color[1], color[2], 80],
                true,
            );
            canvas.icon(id, p, color);
        }
        let handle = images.add(Image::new(
            Extent3d {
                width: W as u32,
                height: H as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            canvas.0,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ));
        self.key = key;
        self.handle = Some(handle.clone());
        handle
    }
}

#[derive(Component)]
pub struct CellMarker(pub usize);
#[derive(Component)]
pub struct Bubble(pub f32);
#[derive(Component)]
pub struct Landmark;

#[derive(Clone, Copy)]
struct Journey {
    current: Vec2,
    from: Vec2,
    to: Vec2,
    elapsed: f32,
    duration: f32,
}

impl Journey {
    fn new(at: Vec2) -> Self {
        Self {
            current: at,
            from: at,
            to: at,
            elapsed: 1.,
            duration: 1.,
        }
    }

    fn advance(&mut self, target: Vec2, delta: f32) -> Vec2 {
        if self.to != target {
            self.from = self.current;
            self.to = target;
            self.elapsed = 0.;
            self.duration = (self.from.distance(target) / 45.).clamp(0.65, 1.5);
        }
        // Presentation only. A stalled frame must not skip the whole journey;
        // authoritative travel and its ATP charge remain instantaneous.
        self.elapsed = (self.elapsed + delta.clamp(0., 0.1)).min(self.duration);
        let t = self.elapsed / self.duration;
        self.current = self.from.lerp(self.to, t * t * (3. - 2. * t));
        self.current
    }
}

#[derive(Resource, Default)]
pub struct Motion([Option<Journey>; 2]);

pub fn animate(
    time: Res<Time>,
    s: Res<super::Session>,
    settings: Res<super::settings::Settings>,
    mut motion: ResMut<Motion>,
    mut markers: Query<(&CellMarker, &mut UiTransform), Without<Bubble>>,
    mut bubbles: Query<(&Bubble, &mut UiTransform), Without<CellMarker>>,
) {
    if s.menu {
        return;
    }
    for (marker, mut transform) in &mut markers {
        let mut target = destination(
            &s.runtime.game.actors[marker.0].location,
            &s.runtime.content,
        ) + Vec2::new(if marker.0 == 0 { -3.0 } else { 3.0 }, 0.);
        target.x = target.x.clamp(1.0, 99.0);
        let journey = motion.0[marker.0].get_or_insert_with(|| Journey::new(target));
        if settings.preferences.reduced_motion {
            *journey = Journey::new(target);
        }
        let current = journey.advance(target, time.delta_secs());
        transform.translation = bevy::ui::Val2::percent(current.x, current.y);
    }
    let t = if settings.preferences.reduced_motion {
        0.
    } else {
        time.elapsed_secs_wrapped()
    };
    for (bubble, mut transform) in &mut bubbles {
        transform.translation = bevy::ui::Val2::px(
            (t * 0.7 + bubble.0).sin() * 5.,
            -(t * 5. + bubble.0 * 13.).rem_euclid(95.),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_survives_a_hitch_and_finishes_without_overshoot() {
        let start = Vec2::new(44., 34.5);
        let end = Vec2::new(14.5, 22.5);
        let mut journey = Journey::new(start);
        let first = journey.advance(end, 10.);
        assert!(first.distance(start) > 0.);
        assert!(
            first.distance(end) > 1.,
            "a stalled frame teleported the cell"
        );
        for _ in 0..120 {
            let at = journey.advance(end, 1. / 60.);
            assert!(at.x >= end.x && at.x <= start.x);
            assert!(at.y >= end.y && at.y <= start.y);
        }
        assert_eq!(journey.current, end);
    }

    #[test]
    fn new_destination_starts_at_current_visual_position() {
        let mut journey = Journey::new(Vec2::ZERO);
        let halfway = journey.advance(Vec2::splat(50.), 0.1);
        assert_eq!(journey.advance(Vec2::new(20., 80.), 0.), halfway);
        assert_eq!(journey.from, halfway);
    }
}
