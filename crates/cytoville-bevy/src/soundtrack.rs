// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scene-adaptive procedural audio; separate from simulation randomness.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
#[derive(Resource, Clone)]
pub struct Sound {
    pub scene: Arc<AtomicU32>,
    pub enabled: Arc<AtomicBool>,
}
impl Default for Sound {
    fn default() -> Self {
        Self {
            scene: Arc::new(AtomicU32::new(0)),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }
}
#[derive(Asset, TypePath)]
struct Score {
    control: Sound,
}
struct Decoder {
    control: Sound,
    index: u64,
    phase: f32,
    pad: f32,
    level: f32,
}
const ROOT: [f32; 15] = [
    220., 196., 174., 261., 110., 146., 233., 164., 130., 293., 196., 155., 138., 261., 103.,
];
const BPM: [f32; 15] = [
    62., 58., 66., 84., 76., 104., 80., 92., 122., 52., 70., 70., 60., 96., 48.,
];
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let scene = self.control.scene.load(Ordering::Relaxed).min(14) as usize;
        let t = self.index as f32 / 44100.;
        let beat = t * BPM[scene] / 60.;
        let step = beat as usize;
        let scale = if [1, 3, 7, 9, 10, 13].contains(&scene) {
            [0., 2., 4., 7., 9.]
        } else {
            [0., 3., 5., 7., 10.]
        };
        let hz = ROOT[scene] * 2f32.powf(scale[(step * 7 + scene) % 5] / 12.);
        self.phase = (self.phase + hz / 44100.) % 1.;
        self.pad = (self.pad + ROOT[scene] * 0.5 / 44100.) % 1.;
        let envelope = (1. - beat.fract()).powi(3);
        let target = if self.control.enabled.load(Ordering::Relaxed) {
            0.14
        } else {
            0.
        };
        self.level += (target - self.level) * 0.0004;
        self.index = (self.index + 1) % (44100 * 3600);
        Some(
            self.level
                * ((self.phase * std::f32::consts::TAU).sin() * envelope * 0.55
                    + (self.pad * std::f32::consts::TAU).sin() * 0.25),
        )
    }
}
impl Source for Decoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(44100).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
impl Decodable for Score {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            control: self.control.clone(),
            index: 0,
            phase: 0.,
            pad: 0.,
            level: 0.,
        }
    }
}
pub fn install(app: &mut App) {
    app.init_resource::<Sound>()
        .add_audio_source::<Score>()
        .add_systems(Startup, start)
        .add_systems(Update, adapt);
}
fn start(mut commands: Commands, mut assets: ResMut<Assets<Score>>, sound: Res<Sound>) {
    commands.spawn(AudioPlayer(assets.add(Score {
        control: sound.clone(),
    })));
}
fn adapt(s: Res<crate::Session>, sound: Res<Sound>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::KeyM) {
        sound.enabled.fetch_xor(true, Ordering::Relaxed);
    }
    let g = &s.runtime.game;
    let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
    let id = if s.menu {
        0
    } else if g.status == "won" {
        13
    } else if g.status != "playing" {
        14
    } else if g.pending_event.is_some() {
        12
    } else {
        match g.actors[seat].location.as_str() {
            "home" => 1,
            "university" => 2,
            "solar" => 3,
            "detritus" => 4,
            "security" => 5,
            "boutique" => 6,
            "diner" => 7,
            "gym" => 8,
            "checkpoint" => 9,
            "trophic" => 10,
            "rift" => 11,
            _ => 0,
        }
    };
    sound.scene.store(id, Ordering::Relaxed);
}
