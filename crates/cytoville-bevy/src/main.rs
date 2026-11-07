// SPDX-License-Identifier: AGPL-3.0-or-later
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};
use cytoville_bevy::network;
use cytoville_core::{Command as GameCommand, Content, actions, rules, turn};
use cytoville_runtime::Runtime;
use enaction_time::FixedStep;
use std::{path::PathBuf, time::Instant};
mod portrait;
mod soundtrack;
const BG: Color = Color::srgb(0.025, 0.055, 0.075);
const PANEL: Color = Color::srgb(0.055, 0.11, 0.14);
const INK: Color = Color::srgb(0.85, 0.94, 0.90);
const MINT: Color = Color::srgb(0.36, 0.9, 0.68);
static FONT: std::sync::OnceLock<Handle<Font>> = std::sync::OnceLock::new();
#[derive(Resource)]
struct Session {
    runtime: Runtime,
    network: Option<network::Network>,
    menu: bool,
    dirty: bool,
    specimen: usize,
    scenario: usize,
    difficulty: usize,
    seed: u32,
    page: usize,
    message: String,
    save: PathBuf,
    clock: FixedStep,
    visual_phase: f64,
    smoke_until: Option<Instant>,
    name_edit: Option<String>,
    inspect: bool,
}
#[derive(Component)]
struct Screen;
#[derive(Component)]
struct CellPulse;
#[derive(Component)]
struct Enabled(bool);
#[derive(Resource)]
struct Capture {
    path: Option<PathBuf>,
    frames: u32,
}
#[derive(Component, Clone)]
enum Click {
    Specimen(usize),
    Scenario(usize),
    Difficulty(usize),
    Start,
    Continue,
    Save,
    Menu,
    Page(usize),
    Pattern,
    Pigment,
    Rename,
    Inspect,
    Game(GameCommand),
}
fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: bevy::text::FontSource::Handle(FONT.get().cloned().unwrap_or_default()),
            font_size: bevy::text::FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}
fn column() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    }
}
fn button(action: Click, enabled: bool) -> impl Bundle {
    (
        Button,
        action,
        Enabled(enabled),
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(if enabled {
            Color::srgb(0.09, 0.21, 0.23)
        } else {
            Color::srgb(0.08, 0.10, 0.12)
        }),
        BorderColor::all(if enabled {
            Color::srgb(0.21, 0.46, 0.42)
        } else {
            PANEL
        }),
    )
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |key: &str| args.windows(2).find(|a| a[0] == key).map(|a| a[1].clone());
    let seed = arg("--seed").and_then(|s| s.parse().ok()).unwrap_or(7);
    let mut content = Content::builtin();
    if let Some(path) = arg("--package") {
        let v: cytoville_core::package::Package =
            serde_json::from_slice(&std::fs::read(path).expect("read package"))
                .expect("parse package");
        content = v.load().expect("valid content package");
    }
    let mut runtime = Runtime::new(
        content,
        "euglena",
        "fresh-daughter",
        "standard",
        seed,
        false,
    )
    .expect("new game");
    let mut network = None;
    let mut message = String::new();
    if args
        .iter()
        .any(|s| ["--host", "--join", "--resume", "--resume-file"].contains(&s.as_str()))
    {
        let address = arg("--server").unwrap_or("127.0.0.1:4280".into());
        let session = arg("--session").unwrap_or("cytoville".into());
        let request = if args.contains(&"--host".into()) {
            serde_json::json!({"op":"create","session":session,"specimen":arg("--specimen").unwrap_or("euglena".into()),"scenario":arg("--scenario").unwrap_or("fresh-daughter".into()),"difficulty":"standard","seed":seed,"multiplayer":!args.contains(&"--native-rival".into())})
        } else {
            serde_json::json!({"op":if args.contains(&"--resume".into()){"resume"}else{"join"},"session":session,"key":arg("--key").unwrap_or_default()})
        };
        let (address, request) = if let Some(file) = arg("--resume-file") {
            let ticket: serde_json::Value =
                serde_json::from_slice(&std::fs::read(file).expect("read resume file"))
                    .expect("resume JSON");
            (
                ticket["server"].as_str().expect("server").to_string(),
                ticket["request"].clone(),
            )
        } else {
            (address, request)
        };
        match network::Network::start(address, request) {
            Ok((net, remote)) => {
                message = if net.invitation.is_empty() {
                    format!("Connected as cell {}", net.seat + 1)
                } else {
                    format!("Invitation key: {}", net.invitation)
                };
                runtime = remote;
                let path = arg("--connection-file").unwrap_or("cytoville-connection.json".into());
                if let Err(e) = save_ticket(&path, &net.resume) {
                    message.push_str(&format!(" · Cannot save resume ticket: {e}"));
                }
                network = Some(net);
            }
            Err(e) => {
                eprintln!("Cannot join Cytoville: {e}");
                std::process::exit(1);
            }
        }
    }
    let menu = network.is_none() && !args.contains(&"--play".into());
    let smoke_until = arg("--smoke-seconds")
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| Instant::now() + std::time::Duration::from_secs(n));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "CYTOVILLE · Biofilm Metropolis".into(),
            resolution: (1440, 960).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(BG))
    .insert_resource(Session {
        runtime,
        network,
        menu,
        dirty: true,
        specimen: 0,
        scenario: 0,
        difficulty: 1,
        seed,
        page: 0,
        message,
        save: arg("--save")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("cytoville-save.json")),
        clock: FixedStep::from_hz(30.0),
        visual_phase: 0.0,
        name_edit: None,
        inspect: false,
        smoke_until,
    })
    .add_systems(
        Startup,
        |mut commands: Commands, mut fonts: ResMut<Assets<Font>>| {
            let font =
                Font::from_bytes(include_bytes!("../../../assets/fonts/DejaVuSans.ttf").to_vec());
            let _ = FONT.set(fonts.add(font));
            commands.spawn(Camera2d);
        },
    )
    .insert_resource(Capture {
        path: arg("--screenshot").map(PathBuf::from),
        frames: 0,
    })
    .add_observer(interact)
    .add_systems(
        Update,
        (keyboard, network_updates, render, animate, capture).chain(),
    );
    soundtrack::install(&mut app);
    app.run();
}
fn save_ticket(path: &str, ticket: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec(ticket)?)?;
    Ok(())
}
fn save(s: &Session) -> Result<(), String> {
    if s.network.is_some() {
        return Err("The multiplayer server owns this session".into());
    }
    let bytes = serde_json::to_vec(&s.runtime).map_err(|e| e.to_string())?;
    let temp = s.save.with_extension("tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &s.save).map_err(|e| e.to_string())
}
fn load(s: &mut Session) -> Result<(), String> {
    let bytes = std::fs::read(&s.save).map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Save too large".into());
    }
    let rt: Runtime = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    rt.validate()?;
    s.runtime = rt;
    s.menu = false;
    s.page = 0;
    Ok(())
}
fn command(s: &mut Session, cmd: GameCommand) {
    let result = if let Some(net) = &s.network {
        net.command(s.runtime.game.revision, &cmd)
    } else {
        s.runtime.dispatch(0, &cmd)
    };
    s.message = match result {
        Ok(()) => {
            if s.network.is_none() {
                save(s).err().unwrap_or_default()
            } else {
                String::new()
            }
        }
        Err(e) => e,
    };
    s.dirty = true;
}
fn interact(event: On<Activate>, mut s: ResMut<Session>, q: Query<(&Click, &Enabled)>) {
    let Ok((click, enabled)) = q.get(event.entity) else {
        return;
    };
    if !enabled.0 {
        return;
    }
    match click.clone() {
        Click::Specimen(i) => s.specimen = i,
        Click::Scenario(i) => s.scenario = i,
        Click::Difficulty(i) => s.difficulty = i,
        Click::Start => {
            let c = s.runtime.content.clone();
            let sp = c.specimen_order[s.specimen].clone();
            let sc = c.scenario_order[s.scenario].clone();
            match Runtime::new(
                c,
                &sp,
                &sc,
                ["gentle", "standard", "fierce"][s.difficulty],
                s.seed,
                false,
            ) {
                Ok(rt) => {
                    s.runtime = rt;
                    s.menu = false;
                    s.page = 0;
                    s.message.clear();
                }
                Err(e) => s.message = e,
            }
        }
        Click::Continue => {
            if let Err(e) = load(&mut s) {
                s.message = e;
            }
        }
        Click::Save => s.message = save(&s).map(|_| "Saved".into()).unwrap_or_else(|e| e),
        Click::Menu => {
            if s.network.is_none() {
                s.menu = true;
            }
        }
        Click::Page(p) => s.page = p,
        Click::Pattern => {
            let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
            let a = &s.runtime.game.looks[seat];
            let patterns = ["plain", "spots", "stripes", "glow"];
            let i = patterns.iter().position(|p| *p == a.pattern).unwrap_or(0);
            let cmd = GameCommand::Customize {
                name: a.name.clone(),
                membrane: a.membrane.clone(),
                accent: a.accent.clone(),
                pattern: patterns[(i + 1) % 4].into(),
            };
            command(&mut s, cmd);
        }
        Click::Pigment => {
            let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
            let a = &s.runtime.game.looks[seat];
            let palette = [
                "#4ade80", "#38bdf8", "#c084fc", "#fb7185", "#fbbf24", "#f8fafc",
            ];
            let i = palette.iter().position(|p| *p == a.membrane).unwrap_or(0);
            let cmd = GameCommand::Customize {
                name: a.name.clone(),
                membrane: palette[(i + 1) % palette.len()].into(),
                accent: palette[(i + 3) % palette.len()].into(),
                pattern: a.pattern.clone(),
            };
            command(&mut s, cmd);
        }
        Click::Rename => {
            let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
            s.name_edit = Some(s.runtime.game.looks[seat].name.clone());
        }
        Click::Inspect => {
            s.inspect = !s.inspect;
            s.page = 0;
        }
        Click::Game(cmd) => {
            s.page = 0;
            command(&mut s, cmd);
        }
    }
    s.dirty = true;
}
fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut s: ResMut<Session>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
) {
    if let Some(mut name) = s.name_edit.take() {
        for event in typed.read() {
            if event.state == bevy::input::ButtonState::Pressed {
                if event.key_code == KeyCode::Backspace {
                    name.pop();
                } else if let Some(t) = &event.text {
                    for ch in t.chars().filter(|c| !c.is_control()) {
                        if name.chars().count() < 40 {
                            name.push(ch);
                        }
                    }
                }
            }
        }
        if keys.just_pressed(KeyCode::Enter) {
            let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
            let a = &s.runtime.game.looks[seat];
            let cmd = GameCommand::Customize {
                name,
                membrane: a.membrane.clone(),
                accent: a.accent.clone(),
                pattern: a.pattern.clone(),
            };
            command(&mut s, cmd);
        } else if !keys.just_pressed(KeyCode::Escape) {
            s.name_edit = Some(name);
        }
        s.dirty = true;
        return;
    }
    typed.clear();
    if keys.just_pressed(KeyCode::F5) {
        s.message = save(&s).map(|_| "Saved".into()).unwrap_or_else(|e| e);
        s.dirty = true;
    }
    if keys.just_pressed(KeyCode::F9) && s.network.is_none() {
        if let Err(e) = load(&mut s) {
            s.message = e;
        }
        s.dirty = true;
    }
    if keys.just_pressed(KeyCode::Space) && !s.menu {
        command(&mut s, GameCommand::EndTurn);
    }
    if keys.just_pressed(KeyCode::Escape) && s.network.is_none() {
        s.menu = !s.menu;
        s.dirty = true;
    }
}
fn network_updates(mut s: ResMut<Session>) {
    let updates = if let Some(net) = &s.network {
        net.receive.lock().unwrap().try_iter().collect::<Vec<_>>()
    } else {
        vec![]
    };
    for update in updates {
        match update {
            Ok(runtime) => {
                if runtime.game != s.runtime.game {
                    s.runtime = runtime;
                    s.dirty = true;
                }
            }
            Err(error) => {
                s.message = error;
                s.dirty = true;
            }
        }
    }
}
fn render(
    mut commands: Commands,
    mut s: ResMut<Session>,
    screens: Query<Entity, With<Screen>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !s.dirty {
        return;
    }
    s.dirty = false;
    for entity in &screens {
        commands.entity(entity).despawn();
    }
    let c = &s.runtime.content;
    let g = &s.runtime.game;
    let seat = s.network.as_ref().map(|n| n.seat).unwrap_or(0);
    let a = &g.actors[seat];
    let portrait = images.add(portrait::image(a, &g.looks[seat]));
    commands.spawn((Screen,Node{width:Val::Percent(100.0),height:Val::Percent(100.0),padding:UiRect::all(Val::Px(24.0)),..column()},BackgroundColor(BG))).with_children(|root|{
        root.spawn(Node{align_items:AlignItems::Center,justify_content:JustifyContent::SpaceBetween,..default()}).with_children(|head|{head.spawn(text("CYTOVILLE",32.0,MINT));head.spawn(text("ONE CELL. ONE LIFETIME. A WHOLE CITY.",15.0,INK));if !s.menu{head.spawn(button(Click::Menu,true)).with_child(text("Menu",15.0,INK));head.spawn(button(Click::Save,s.network.is_none())).with_child(text("Save · F5",15.0,INK));}});
        if !s.message.is_empty(){root.spawn(text(s.message.clone(),15.0,Color::srgb(1.0,0.75,0.38)));}
        if let Some(name)=&s.name_edit{root.spawn(text(format!("Name: {name}_   Enter to save · Escape to cancel"),20.0,MINT));}
        if s.menu{
            root.spawn(text("A protist life simulation and time-management strategy",23.0,INK));root.spawn(text("Develop one cell through its ontogeny. Earn biomass, express organelles and finish your checklist before your rival.",17.0,INK));root.spawn(text("CHOOSE YOUR SPECIMEN",16.0,MINT));
            root.spawn(Node{flex_wrap:FlexWrap::Wrap,column_gap:Val::Px(8.0),row_gap:Val::Px(8.0),..default()}).with_children(|row|{for(i,id)in c.specimen_order.iter().enumerate(){row.spawn(button(Click::Specimen(i),true)).with_child(text(format!("{}{}",if s.specimen==i{"● "}else{""},c.specimens[id].name),17.0,INK));}});
            let sp=&c.specimens[&c.specimen_order[s.specimen]];root.spawn(text(format!("{} · {}",sp.latin,sp.title),20.0,MINT));root.spawn(text(sp.blurb.clone(),17.0,INK));root.spawn(text("CHOOSE YOUR CELL CYCLE",16.0,MINT));
            for(i,id)in c.scenario_order.iter().enumerate(){root.spawn(button(Click::Scenario(i),true)).with_child(text(format!("{}{} · {} cycles",if s.scenario==i{"● "}else{""},c.scenarios[id].name,c.scenarios[id].turn_limit),18.0,INK));}
            root.spawn(Node{column_gap:Val::Px(10.0),..default()}).with_children(|row|{for(i,name)in["Gentle","Standard","Fierce"].iter().enumerate(){row.spawn(button(Click::Difficulty(i),true)).with_child(text(format!("{}{}",if s.difficulty==i{"● "}else{""},name),16.0,INK));}});
            root.spawn(Node{column_gap:Val::Px(12.0),margin:UiRect::top(Val::Px(12.0)),..default()}).with_children(|row|{row.spawn(button(Click::Start,true)).with_child(text("Begin life →",22.0,MINT));row.spawn(button(Click::Continue,true)).with_child(text("Continue · F9",20.0,INK));});
            root.spawn(text("Travel spends ATP. Eat every cycle. Rest to control stress. End a cycle with Space. Progress autosaves after every accepted command.",16.0,INK));return;
        }
        root.spawn(text(format!("CYCLE {} / {} · {} → {} | {} · {}",g.turn,c.scenarios[&g.scenario_id].turn_limit,c.weather[&g.weather].name,c.weather[&g.forecast].name,a.name,a.phase),18.0,INK));root.spawn(text(format!("ATP {} / {}     Biomass {} μg C     Nutrition {:.0}%     Stress {:.0}%     Size {:.1}%     Genes T{}",a.atp,a.atp_max,a.biomass,a.nutrition,a.stress,a.size,a.transcription),20.0,MINT));
        if g.status!="playing"{root.spawn(text(format!("{} · {}",g.status.to_uppercase(),g.winner.map(|i|format!("{} completed the checklist",g.actors[i].name)).unwrap_or("The biofilm exhausted its nutrients".into())),25.0,MINT));}else if g.active!=seat{root.spawn(text("Waiting for the other cell's turn…",18.0,INK));}
        root.spawn(Node{width:Val::Percent(100.0),flex_grow:1.0,min_height:Val::Px(360.0),column_gap:Val::Px(18.0),..default()}).with_children(|body|{
            body.spawn((Node{width:Val::Percent(56.0),height:Val::Percent(100.0),position_type:PositionType::Relative,..default()},BackgroundColor(PANEL))).with_children(|map|{
                map.spawn(Node{position_type:PositionType::Absolute,left:Val::Px(12.0),top:Val::Px(10.0),..default()}).with_child(text("THE BIOFILM METROPOLIS",18.0,MINT));
                for id in &c.location_order{let l=&c.locations[id];let(x,y)=match id.as_str(){"home"=>(38.,40.),"university"=>(3.,28.),"solar"=>(26.,12.),"checkpoint"=>(49.,28.),"security"=>(73.,12.),"detritus"=>(3.,53.),"boutique"=>(73.,53.),"rift"=>(38.,62.),"gym"=>(6.,75.),"diner"=>(70.,75.),_=>(l.x as f32,l.y as f32)};let cost=rules::travel_cost(c,a,id).unwrap_or(0);map.spawn(button(Click::Game(GameCommand::Travel{location:id.clone()}),cost<=a.atp&&g.active==seat)).insert(Node{position_type:PositionType::Absolute,left:Val::Percent(x),top:Val::Percent(y),padding:UiRect::all(Val::Px(8.0)),width:Val::Percent(23.0),border_radius:BorderRadius::all(Val::Px(8.0)),..default()}).with_child(text(format!("{}{}\n{cost} ATP",if a.location==*id{"◉ "}else{""},l.short),14.0,if a.location==*id{MINT}else{INK}));}
                map.spawn((CellPulse,ImageNode::new(portrait.clone()),Node{position_type:PositionType::Absolute,left:Val::Px(8.0),bottom:Val::Px(4.0),width:Val::Px(130.0),height:Val::Px(130.0),..default()}));
                map.spawn(Node{position_type:PositionType::Absolute,left:Val::Px(145.0),bottom:Val::Px(18.0),..column()}).with_children(|info|{info.spawn(text(format!("{} · {}",c.specimens[&a.specimen].name,g.looks[seat].pattern),15.0,INK));info.spawn(text(format!("{} organelles · {} stored meals · {}",a.organelles.len(),a.storage_stock,a.phase),14.0,INK));});
            });
            body.spawn((Node{width:Val::Percent(44.0),padding:UiRect::all(Val::Px(14.0)),..column()},BackgroundColor(PANEL))).with_children(|panel|{
                if s.inspect{panel.spawn(text("EXPRESSED ORGANELLES",22.0,MINT));let page=s.page.min(a.organelles.len().div_ceil(4).saturating_sub(1));for id in a.organelles.iter().skip(page*4).take(4){let o=&c.organelles[id];panel.spawn(text(o.name.clone(),18.0,MINT));panel.spawn(text(format!("{} {}",o.effect,o.science),14.0,INK));}panel.spawn(button(Click::Page((page+1)%a.organelles.len().div_ceil(4).max(1)),true)).with_child(text("Next organelles",15.0,INK));panel.spawn(button(Click::Inspect,true)).with_child(text("Back to district",15.0,INK));
                }else if let Some(event)=g.pending_event.as_ref().and_then(|id|c.events.iter().find(|e|e.id==*id)){
                    panel.spawn(text(event.title.clone(),22.0,MINT));panel.spawn(text(event.extra.get("prompt").and_then(|v|v.as_str()).unwrap_or("Encounter"),16.0,INK));for choice in &event.choices{let lock=turn::choice_lock(a,choice);panel.spawn(button(Click::Game(GameCommand::Choose{id:choice.id.clone()}),lock.is_none())).with_child(text(format!("{}{}",choice.label,lock.map(|l|format!(" · {l}")).unwrap_or_default()),15.0,INK));}
                }else{
                    panel.spawn(text(c.locations[&a.location].name.clone(),22.0,MINT));panel.spawn(text(c.locations[&a.location].blurb.clone(),15.0,INK));let available=actions::list(c,g,seat,&a.location);let pages=available.len().div_ceil(5).max(1);let page=s.page.min(pages-1);
                    for action in available.iter().skip(page*5).take(5){panel.spawn(button(Click::Game(GameCommand::Act{id:action.id.clone()}),action.enabled()&&g.active==seat)).with_child(text(format!("{}\n{} ATP · {} μg C{}",action.label,action.atp,action.biomass,action.reason.as_ref().map(|r|format!(" · {r}")).unwrap_or_default()),15.0,if action.enabled(){INK}else{Color::srgb(0.5,0.6,0.6)}));}
                    if pages>1{panel.spawn(Node{column_gap:Val::Px(8.0),..default()}).with_children(|row|{row.spawn(button(Click::Page(page.saturating_sub(1)),true)).with_child(text("←",18.0,INK));row.spawn(text(format!("{} / {pages}",page+1),16.0,INK));row.spawn(button(Click::Page((page+1).min(pages-1)),true)).with_child(text("→",18.0,INK));});}
                    if a.location=="home"{panel.spawn(Node{flex_wrap:FlexWrap::Wrap,column_gap:Val::Px(6.0),row_gap:Val::Px(6.0),..default()}).with_children(|row|{for(click,label)in[(Click::Pattern,"Pattern"),(Click::Pigment,"Pigment"),(Click::Rename,"Rename"),(Click::Inspect,"Organelles")]{row.spawn(button(click,g.active==seat)).with_child(text(label,14.0,INK));}});}
                }
                panel.spawn(button(Click::Game(GameCommand::EndTurn),g.pending_event.is_none()&&g.active==seat)).with_child(text("End cycle · Space →",20.0,MINT));
            });
        });
        root.spawn(Node{column_gap:Val::Px(24.0),..default()}).with_children(|bottom|{
            bottom.spawn(Node{width:Val::Percent(56.0),..column()}).with_children(|goals|{goals.spawn(text("ONTOGENETIC QUEST CHECKLIST",15.0,MINT));for q in &c.scenarios[&g.scenario_id].quests{goals.spawn(text(format!("{} {} [{:.0} / {:.0}]",if a.done.contains(&q.id){"✓"}else{"○"},q.label,rules::quest_value(c,a,q),q.target),14.0,INK));}});
            bottom.spawn(Node{width:Val::Percent(44.0),..column()}).with_children(|log|{log.spawn(text(format!("RIVAL: {} · {} goals · {}",g.actors[1-seat].name,g.actors[1-seat].done.len(),g.actors[1-seat].location),15.0,MINT));for line in g.log.iter().rev().take(4){log.spawn(text(line,13.0,INK));}});
        });
    });
}
fn animate(
    time: Res<Time>,
    mut s: ResMut<Session>,
    mut cells: Query<&mut Node, With<CellPulse>>,
    mut exit: MessageWriter<AppExit>,
) {
    let steps = s.clock.advance(time.delta_secs_f64());
    s.visual_phase += steps as f64 / 30.0;
    for mut cell in &mut cells {
        cell.width = Val::Px(130.0 + 3.0 * (s.visual_phase * 2.0).sin() as f32);
    }
    if s.smoke_until.is_some_and(|d| Instant::now() >= d) {
        exit.write(AppExit::Success);
    }
}
fn capture(mut commands: Commands, mut c: ResMut<Capture>) {
    c.frames += 1;
    if c.frames == 10
        && let Some(path) = c.path.take()
    {
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
}
