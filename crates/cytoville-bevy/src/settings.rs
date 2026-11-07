// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local presentation preferences; authoritative game configuration is separate.
use super::{BG, Click, INK, MINT, PANEL, Session, button, column, text};
use bevy::{
    prelude::*,
    render::renderer::RenderAdapterInfo,
    window::{MonitorSelection, PresentMode, WindowMode},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
type ResearchResult = Result<cytoville_runtime::research::HarnessReceipt, String>;
type ResearchInbox = std::sync::Mutex<std::sync::mpsc::Receiver<ResearchResult>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Performance,
    Accessibility,
    Video,
    Game,
    Players,
    Server,
}
impl Tab {
    pub const ALL: [Self; 6] = [
        Self::Performance,
        Self::Accessibility,
        Self::Video,
        Self::Game,
        Self::Players,
        Self::Server,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Performance => "Performance",
            Self::Accessibility => "Accessibility",
            Self::Video => "Video",
            Self::Game => "Game",
            Self::Players => "Players",
            Self::Server => "Server",
        }
    }
    pub fn next(self, offset: usize) -> Self {
        Self::ALL
            [(Self::ALL.iter().position(|t| *t == self).unwrap_or(0) + offset) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    FrameQueue,
    Contrast,
    Motion,
    Music,
    Vsync,
    Fullscreen,
    Border,
    Resolution,
    Adiabatic,
    Weevil,
    AiCapability,
    AiName,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub difficulty: usize,
    pub rival: cytoville_runtime::RivalSettings,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub music: bool,
    pub vsync: bool,
    pub fullscreen: bool,
    pub decorated: bool,
    pub resolution: usize,
    pub frame_queue: u32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            difficulty: 1,
            rival: Default::default(),
            high_contrast: false,
            reduced_motion: false,
            music: true,
            vsync: true,
            fullscreen: false,
            decorated: true,
            resolution: 0,
            frame_queue: 0,
        }
    }
}

#[derive(Resource, Default)]
pub struct Settings {
    pub open: bool,
    pub tab: Tab,
    pub selected: usize,
    pub contribute: bool,
    pub notice: String,
    pub research_open: bool,
    pub preferences: Preferences,
    pub adapter: String,
    pub focus: usize,
    pub edit_ai_name: Option<String>,
    pub research_summary: String,
    pub research_pending: Option<ResearchInbox>,
    pub(crate) path: Option<PathBuf>,
    pub(crate) apply_pending: bool,
    pub(crate) last_video: Option<(bool, bool, bool, usize, u32)>,
}

impl Settings {
    pub fn start_research(&mut self, runtime: &cytoville_runtime::Runtime, seat: usize) {
        if self.research_pending.is_some() {
            return;
        }
        let districts = ["university", "solar", "detritus", "diner"];
        let Some(actor) = runtime.game.actors.get(seat) else {
            return;
        };
        let mut costs = [0; 4];
        for (i, district) in districts.iter().enumerate() {
            let Some(cost) = cytoville_core::rules::travel_cost(&runtime.content, actor, district)
            else {
                self.research_summary = "This content package lacks a harness district.".into();
                return;
            };
            costs[i] = cost;
        }
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("cytoville-research".into())
            .spawn(move || {
                let _ = send.send(cytoville_runtime::research::run_local_harness(costs));
            }) {
            Ok(_) => {
                self.research_pending = Some(std::sync::Mutex::new(receive));
                self.research_summary = "Running bounded CPU reference checks…".into();
            }
            Err(error) => self.research_summary = format!("Cannot start local harness: {error}"),
        }
    }
    pub fn load(path: PathBuf) -> Self {
        let mut result = Self {
            path: Some(path.clone()),
            apply_pending: true,
            ..default()
        };
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= 16 * 1024 => {
                match serde_json::from_slice::<Preferences>(&bytes) {
                    Ok(prefs)
                        if prefs.resolution < 3
                            && prefs.frame_queue <= 3
                            && prefs.difficulty < 3
                            && prefs.rival.validate().is_ok() =>
                    {
                        result.preferences = prefs
                    }
                    _ => result.notice = "Saved settings were invalid; using defaults.".into(),
                }
            }
            Ok(_) => result.notice = "Saved settings were too large; using defaults.".into(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => result.notice = format!("Cannot read settings: {error}"),
        }
        result
    }

    pub fn options(&self) -> &'static [Preference] {
        use Preference::*;
        match self.tab {
            Tab::Performance => &[FrameQueue, Adiabatic],
            Tab::Accessibility => &[Contrast, Motion, Music],
            Tab::Video => &[Vsync, Fullscreen, Border, Resolution],
            Tab::Game => &[Weevil],
            Tab::Players => &[AiCapability, AiName],
            _ => &[],
        }
    }

    pub fn change(&mut self, pref: Preference) {
        use Preference::*;
        let p = &mut self.preferences;
        match pref {
            FrameQueue => p.frame_queue = (p.frame_queue + 1) % 4,
            Contrast => p.high_contrast = !p.high_contrast,
            Motion => p.reduced_motion = !p.reduced_motion,
            Music => p.music = !p.music,
            Vsync => p.vsync = !p.vsync,
            Fullscreen => p.fullscreen = !p.fullscreen,
            Border => p.decorated = !p.decorated,
            Resolution if p.fullscreen => {
                self.notice = "Leave fullscreen to change the window resolution.".into();
                return;
            }
            Resolution => p.resolution = (p.resolution + 1) % 3,
            Weevil => p.rival.inject_weevil = !p.rival.inject_weevil,
            AiCapability => p.rival.capability = (p.rival.capability + 1) % 3,
            AiName => {
                self.edit_ai_name = Some(p.rival.name.clone().unwrap_or_else(|| "Rival".into()));
                return;
            }
            Adiabatic => {
                self.research_open = !self.research_open;
                self.selected = 7;
                self.contribute = false;
                return;
            }
        }
        self.apply_pending = true;
        self.notice.clear();
        self.persist();
    }
    pub fn set_ai_name(&mut self, name: String) {
        let mut candidate = self.preferences.rival.clone();
        candidate.name = Some(name.trim().to_string());
        if let Err(error) = candidate.validate() {
            self.notice = error;
            return;
        }
        self.preferences.rival = candidate;
        self.notice.clear();
        self.persist();
    }
    pub fn set_difficulty(&mut self, difficulty: usize) {
        if difficulty < 3 {
            self.preferences.difficulty = difficulty;
            self.persist();
        }
    }
    fn persist(&mut self) {
        if let Some(path) = &self.path {
            let result = serde_json::to_vec_pretty(&self.preferences)
                .map_err(std::io::Error::other)
                .and_then(|bytes| {
                    let temp = path.with_extension("tmp");
                    std::fs::write(&temp, bytes)?;
                    std::fs::rename(temp, path)
                });
            if let Err(error) = result {
                self.notice =
                    format!("Settings apply for this session but could not be saved: {error}");
            }
        }
    }
}

pub fn poll_research(mut settings: ResMut<Settings>, mut session: ResMut<Session>) {
    let Some(inbox) = &settings.research_pending else {
        return;
    };
    let result = match inbox.lock() {
        Ok(receiver) => receiver.try_recv(),
        Err(_) => Err(std::sync::mpsc::TryRecvError::Disconnected),
    };
    let message = match result {
        Ok(Ok(receipt)) => format!(
            "Local checks passed: {} QUBO candidates; {} reversible input cases; {} ancilla wires restored. CPU reference only; physical hardware and energy savings untested.",
            receipt.enumerated_states, receipt.reversible_input_cases, receipt.clean_ancilla_wires
        ),
        Ok(Err(error)) => format!("Local harness failed: {error}"),
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            "Local harness stopped without a result.".into()
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => return,
    };
    settings.research_pending = None;
    settings.research_summary = message;
    session.dirty = true;
}

pub fn detect_adapter(info: Res<RenderAdapterInfo>, mut settings: ResMut<Settings>) {
    settings.adapter = format!("{} · {:?}", info.name, info.backend);
}

pub fn apply(
    mut settings: ResMut<Settings>,
    mut windows: Query<&mut Window>,
    sound: Option<Res<super::soundtrack::Sound>>,
) {
    if !settings.apply_pending {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let p = &settings.preferences;
    let video = (
        p.vsync,
        p.fullscreen,
        p.decorated,
        p.resolution,
        p.frame_queue,
    );
    if settings.last_video.is_none_or(|old| old.4 != p.frame_queue) {
        window.desired_maximum_frame_latency = std::num::NonZero::new(p.frame_queue);
    }
    if settings.last_video.is_none_or(|old| old.0 != p.vsync) {
        window.present_mode = if p.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
    }
    if settings.last_video.is_none_or(|old| old.2 != p.decorated) {
        window.decorations = p.decorated;
    }
    if settings.last_video.is_none_or(|old| old.1 != p.fullscreen) {
        window.mode = if p.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
    }
    if !p.fullscreen
        && settings
            .last_video
            .is_none_or(|old| old.1 != p.fullscreen || old.3 != p.resolution)
    {
        if p.resolution == 0 {
            window.set_maximized(true);
        } else {
            window.set_maximized(false);
            let (w, h) = if p.resolution == 1 {
                (1280., 720.)
            } else {
                (1440., 900.)
            };
            window.resolution.set(w, h);
        }
    }
    if let Some(sound) = sound {
        sound
            .enabled
            .store(p.music, std::sync::atomic::Ordering::Relaxed);
    }
    settings.apply_pending = false;
    settings.last_video = Some(video);
}

fn toggle(
    root: &mut ChildSpawnerCommands,
    settings: &Settings,
    pref: Preference,
    label: &str,
    on: bool,
) {
    let focused = settings.options().get(settings.focus) == Some(&pref);
    root.spawn(button(Click::Preference(pref), true))
        .with_child(text(
            format!(
                "{}{}: {}",
                if focused { "› " } else { "" },
                label,
                if on { "On" } else { "Off" }
            ),
            18.,
            if settings.preferences.high_contrast {
                Color::WHITE
            } else {
                INK
            },
        ));
}

pub fn draw(root: &mut ChildSpawnerCommands, settings: &Settings, session: &Session) {
    let p = &settings.preferences;
    let ink = if p.high_contrast { Color::WHITE } else { INK };
    let mint = if p.high_contrast {
        Color::srgb(1., 1., 0.)
    } else {
        MINT
    };
    root.spawn(text("SETTINGS", 28., mint));
    root.spawn(Node {
        column_gap: Val::Px(8.),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    })
    .with_children(|tabs| {
        for tab in Tab::ALL {
            tabs.spawn(button(Click::SettingsTab(tab), true))
                .with_child(text(
                    format!(
                        "{}{}",
                        if tab == settings.tab { "● " } else { "" },
                        tab.name()
                    ),
                    18.,
                    ink,
                ));
        }
    });
    root.spawn((Node { width: Val::Percent(100.), padding: UiRect::all(Val::Px(20.)), ..column() }, BackgroundColor(if p.high_contrast { BG } else { PANEL }))).with_children(|body| {
        match settings.tab {
            Tab::Performance => {
                body.spawn(text("Rendering and computation",22.,mint));
                body.spawn(text(format!("Active renderer: {}",settings.adapter),17.,ink));
                body.spawn(button(Click::Preference(Preference::FrameQueue),true)).with_child(text(format!("Presentation queue: {}",["Automatic", "1 frame · lower latency", "2 frames", "3 frames"][p.frame_queue as usize]),18.,ink));
                body.spawn(text("A driver hint: fewer queued frames can reduce input delay, with possible throughput trade-offs. V-sync is in Video.",16.,ink));
                body.spawn(text("The CPU runs the game rules; Bevy renders the city. Specialist compute backends are being integrated and tested separately from rendering.",17.,ink));
                body.spawn(text("CPU / GPU / NPU / TPU / FPGA / DSP / physics / mathematics / crypto / I/O / QPU",15.,ink));
                body.spawn(text("Backend controls will name the actual operation, implementation and detected device. Current availability is not inferred from a processor's product name.",16.,ink));
                body.spawn(Node { height: Val::Px(18.), ..default() });
                body.spawn(button(Click::Preference(Preference::Adiabatic),true)).with_child(text(if settings.research_open { "− Experimental adiabatic computing" } else { "+ Experimental adiabatic computing · research and contribution" },16.,mint));
                if settings.research_open { super::technology::draw(body,settings); }
            },
            Tab::Accessibility => {
                body.spawn(text("Presentation and sound",22.,mint));
                toggle(body,settings,Preference::Contrast,"High contrast",p.high_contrast);
                toggle(body,settings,Preference::Motion,"Reduce animation",p.reduced_motion);
                toggle(body,settings,Preference::Music,"Soundtrack",p.music);
                body.spawn(text("The current soundtrack is mono. Stereo-to-mono controls and voiceover are still being implemented.",17.,ink));
                body.spawn(text("Keyboard: Left/Right tabs · Up/Down controls · Enter change · Escape return.",16.,ink));
            },
            Tab::Video => {
                body.spawn(text("Display",22.,mint));
                toggle(body,settings,Preference::Vsync,"V-sync",p.vsync);
                toggle(body,settings,Preference::Fullscreen,"Borderless fullscreen",p.fullscreen);
                toggle(body,settings,Preference::Border,"Window borders",p.decorated);
                let resolution = ["Maximised desktop", "1280 × 720", "1440 × 900"][p.resolution];
                body.spawn(button(Click::Preference(Preference::Resolution),!p.fullscreen)).with_child(text(format!("Window resolution: {resolution}"),18.,ink));
                body.spawn(text("Fullscreen uses the desktop resolution. Window sizes follow your display scaling.",16.,ink));
            },
            Tab::Game => {
                body.spawn(text("Next game",22.,mint));
                body.spawn(Node { column_gap: Val::Px(8.), ..default() }).with_children(|row| { for (i,label) in ["Easy","Normal","Hard"].iter().enumerate() { row.spawn(button(Click::Difficulty(i),true)).with_child(text(format!("{}{}",if session.difficulty==i {"● "}else{""},label),18.,ink)); } });
                body.spawn(text("Difficulty applies when you begin a new game. The current game keeps its saved rules.",17.,ink));
                toggle(body,settings,Preference::Weevil,"Inject Evil Weevil",p.rival.inject_weevil);
                body.spawn(text("On: native Evil Weevil drives rival intent. Off: a deterministic rule-based rival. Applies to your next single-player game.",17.,ink));
            },
            Tab::Players => {
                body.spawn(text(if session.network.is_some() { "Multiplayer session" } else { "Single player" },22.,mint));
                body.spawn(text("AI rivals: 1 · Additional rival counts are not yet available.",17.,ink));
                body.spawn(button(Click::Preference(Preference::AiCapability),true)).with_child(text(format!("AI capability: {}",["Casual · 3 decisions/cycle","Balanced · 12 decisions/cycle","Thorough · 80 decisions/cycle"][p.rival.capability as usize]),18.,ink));
                body.spawn(button(Click::Preference(Preference::AiName),true)).with_child(text(format!("AI name: {}",p.rival.name.as_deref().unwrap_or("Specimen name")),18.,ink));
                if let Some(name) = &settings.edit_ai_name { body.spawn(text(format!("Name: {name}_ · Enter saves · Escape cancels"),18.,mint)); }
                body.spawn(text("AI changes apply to the next single-player game; saved and server-owned games retain their acknowledged settings.",16.,ink));
                body.spawn(text("Local encrypted multiplayer remains available through the documented host/join/resume launch options.",17.,ink));
            },
            Tab::Server => {
                body.spawn(text("Game Server Admin",22.,mint));
                body.spawn(text("Not connected · Game Server Admin integration is not yet available in this build.",17.,ink));
                body.spawn(text(if session.network.is_some() { "This game is connected to its multiplayer host. GSA administration is a separate connection." } else { "This is a local single-player session. No public server is started by opening Settings." },17.,ink));
            },
        }
    });
    if !settings.notice.is_empty() {
        root.spawn(text(&settings.notice, 16., ink));
    }
    root.spawn(button(Click::Settings, true))
        .with_child(text("← Return · Escape", 18., mint));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_ai_name_does_not_replace_saved_choice() {
        let mut settings = Settings::default();
        settings.set_ai_name("Ada".into());
        settings.set_ai_name(" \n ".into());
        assert_eq!(settings.preferences.rival.name.as_deref(), Some("Ada"));
        assert!(!settings.notice.is_empty());
    }
    #[test]
    fn changing_accessibility_does_not_reset_the_window_mode() {
        let mut app = App::new();
        let mut settings = Settings {
            last_video: Some((true, false, true, 0, 0)),
            ..default()
        };
        settings.change(Preference::Contrast);
        app.insert_resource(settings).add_systems(Update, apply);
        let mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
        let id = app.world_mut().spawn(Window { mode, ..default() }).id();
        app.update();
        assert_eq!(app.world().entity(id).get::<Window>().unwrap().mode, mode);
    }
    #[test]
    fn preference_roundtrip_preserves_real_video_and_ai_controls() {
        let mut settings = Settings::default();
        settings.change(Preference::Motion);
        settings.change(Preference::Vsync);
        settings.change(Preference::Weevil);
        settings.change(Preference::AiCapability);
        settings.set_ai_name("Ada".into());
        let bytes = serde_json::to_vec(&settings.preferences).unwrap();
        let loaded: Preferences = serde_json::from_slice(&bytes).unwrap();
        assert!(loaded.reduced_motion);
        assert!(!loaded.vsync);
        assert!(!loaded.rival.inject_weevil);
        assert_eq!(loaded.rival.capability, 0);
        assert_eq!(loaded.rival.name.as_deref(), Some("Ada"));
    }
}
