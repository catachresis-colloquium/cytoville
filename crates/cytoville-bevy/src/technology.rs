// SPDX-License-Identifier: AGPL-3.0-or-later
//! Player-facing research catalogue. Entries do not advertise executable backends.
use super::{Click, INK, MINT, button, text};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;

use super::settings::Settings;

pub struct Technology {
    pub name: &'static str,
    pub opportunity: &'static str,
    pub limitation: &'static str,
    pub contribution: &'static str,
    pub source: &'static str,
}

pub const TECHNOLOGIES: &[Technology] = &[
    Technology {
        name: "CPU · specialist instructions",
        opportunity: "Make the work smaller and use supported vector instructions for measured simulation workloads. Xeon and i9 are product families, not guarantees of a particular instruction set.",
        limitation: "The ordinary CPU game already runs. Optional workload-specific acceleration and capability checks are not implemented in this client.",
        contribution: "Contribute repeatable frame/input measurements and scalar-versus-vector kernel tests. Ask vendors for portable, documented instruction and performance interfaces.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "GPU · simulation compute",
        opportunity: "Explore large batches of cell or field calculations on a graphics processor. The game already uses Bevy rendering; this entry concerns additional compute workloads.",
        limitation: "No Cytoville GPU simulation backend is shipped. A graphics-capable device alone does not establish faster or deterministic simulation.",
        contribution: "Find a measured bottleneck, implement a bounded kernel and compare complete transfer-plus-compute time against the CPU reference. Contribute cross-vendor tests.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "NPU / TPU · neural inference",
        opportunity: "Investigate optional learned advisers or content tools. These processors accelerate supported neural models; they do not automatically accelerate the whole game.",
        limitation: "No compatible game model, device probe or inference backend is shipped. Your hardware has not been assessed for this option.",
        contribution: "Build small reproducible model benchmarks, document supported operators and test accessible explanations. Ask vendors for open drivers and reproducible energy measurements.",
        source: "https://docs.openvino.ai/2026/about-openvino/release-notes-openvino/system-requirements.html",
    },
    Technology {
        name: "FPGA · programmable hardware",
        opportunity: "Prototype specialised simulation kernels on reconfigurable logic when a real workload justifies the hardware and transfer cost.",
        limitation: "No board support, driver or validated bitstream is shipped. Hardware compatibility is unknown.",
        contribution: "Contribute an open toolchain, board-specific kernel, CPU comparison and resource/energy measurements. Ask manufacturers for documented, maintainable interfaces.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "Physics / mathematics kernels",
        opportunity: "Offer specialised operations with CPU or device implementations, selected by the operation's actual requirements.",
        limitation: "No optional Cytoville physics or mathematics accelerator backend is shipped. A device name is not evidence of a working kernel.",
        contribution: "Specify bounded inputs, numerical error and deterministic replay requirements; contribute reference results and adversarial tests.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "Audio / DSP / media",
        opportunity: "Explore low-latency synthesis and media processing on supported audio or signal processors. Ordinary game audio already works.",
        limitation: "No optional DSP or media accelerator is shipped. APU can mean different hardware; support must name the actual device and operation.",
        contribution: "Measure audible glitches, buffer deadlines and end-to-end latency; contribute accessible audio cues and documented device integrations.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "Crypto / I/O / DPU",
        opportunity: "Investigate established cryptographic providers and server networking offload where measured load warrants it. Local TLS already uses a maintained TLS library.",
        limitation: "No optional game-managed crypto, IOPU or DPU backend is shipped. This is not a promise to fix menu latency through network hardware.",
        contribution: "Contribute secure provider integration and realistic server-load benchmarks. Ask vendors for auditable interfaces; keep keys and player data out of public diagnostics.",
        source: "https://github.com/catachresis-colloquium/enaction-engine/blob/main/docs/decisions/0020-operation-first-coprocessor-runtime.adoc",
    },
    Technology {
        name: "QPU · quantum annealing",
        opportunity: "Research bounded optimisation jobs such as offline content balancing or server-placement advice, with a classical reference and independently checked results.",
        limitation: "Research only: no solver or quantum hardware connection is shipped. Queuing, programming and readout matter; this does not belong in the frame loop.",
        contribution: "Contribute small QUBO/Ising examples, feasibility checks and fair classical baselines. Request reproducible results and accessible research access, not unmeasured speed claims.",
        source: "https://docs.dwavequantum.com/en/latest/quantum_research/operation_timing.html",
    },
    Technology {
        name: "Reversible / adiabatic computing",
        opportunity: "Explore reversible kernels and future energy-recovering classical hardware. Track inverse operations, retained information and temporary storage explicitly.",
        limitation: "Research only: no physical energy-recovery backend is shipped. Reversible software on an ordinary CPU does not make its circuits adiabatic.",
        contribution: "Reproduce a small reversible circuit or kernel, check its inverse and account for garbage/ancillas. Support open research hardware and measured energy results.",
        source: "https://www.sandia.gov/news/publications/hpc-annual-reports/article/stretching-the-thermodynamic-limits-of-hpc-efficiency/",
    },
];

pub fn draw(root: &mut ChildSpawnerCommands, settings: &Settings) {
    let selected = settings.selected.clamp(7, 8);
    root.spawn(Node {
        column_gap: Val::Px(8.),
        ..default()
    })
    .with_children(|row| {
        for i in [7, 8] {
            row.spawn(button(Click::Technology(i), true))
                .with_child(text(
                    format!(
                        "{}{}",
                        if selected == i { "● " } else { "" },
                        TECHNOLOGIES[i].name
                    ),
                    16.,
                    MINT,
                ));
        }
    });
    let tech = &TECHNOLOGIES[selected];
    root.spawn(text(tech.opportunity, 16., INK));
    root.spawn((
        button(Click::UnavailableTechnology, false),
        InteractionDisabled,
    ))
    .with_child(text(
        "Experimental backend · not available in this build",
        16.,
        INK,
    ));
    root.spawn(text(tech.limitation, 15., INK));
    root.spawn(button(
        Click::ResearchHarness,
        settings.research_pending.is_none(),
    ))
    .with_child(text("Run local research harness · H", 16., MINT));
    if !settings.research_summary.is_empty() {
        root.spawn(text(&settings.research_summary, 15., INK));
    }
    root.spawn(button(Click::TechnologyContribute, true))
        .with_child(text(
            if settings.contribute {
                "Hide research and contribution guide"
            } else {
                "Learn, contribute and support the research"
            },
            16.,
            MINT,
        ));
    if settings.contribute {
        root.spawn(text(tech.contribution, 15., INK));
        root.spawn(text("AI can help explain a paper or draft an experiment. Check original sources and reproduce results; include tool versions and limitations.",14.,INK));
        root.spawn(Node {
            column_gap: Val::Px(8.),
            ..default()
        })
        .with_children(|row| {
            row.spawn(button(Click::TechnologySource, true))
                .with_child(text("Research source · L ↗", 15., MINT));
            row.spawn(button(Click::TechnologyProject, true))
                .with_child(text("Contribute · C ↗", 15., MINT));
        });
    }
}
/// Only callers selecting a built-in catalogue link may launch the browser.
pub fn open_source(selected: usize, project: bool) -> std::io::Result<()> {
    let url = if project {
        "https://github.com/catachresis-colloquium/cytoville/blob/main/docs/contributing.adoc"
    } else {
        TECHNOLOGIES[selected].source
    };
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(url).spawn().map(|_| ())
}
