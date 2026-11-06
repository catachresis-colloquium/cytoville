// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_ums::{CytovilleProfile, descriptor};
use serde_json::json;
use std::path::Path;
use ums_profile_sdk::{ProfileCompiler, ProfileGenerator, ProfileImporter, SimulationAdapter};
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let p = CytovilleProfile;
    let get = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or("Missing argument".to_string())
    };
    match get(1)?{
        "describe"=>println!("{}",serde_json::to_string_pretty(descriptor()).map_err(|e|e.to_string())?),
        "init"=>{std::fs::write(get(2)?,serde_json::to_vec_pretty(&p.generate(&json!({"generator":"builtin"}))?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;}
        "validate"=>{p.import(&std::fs::read(get(2)?).map_err(|e|e.to_string())?)?;println!("valid");}
        "compile"=>{let package=p.compile(Path::new(get(2)?),Path::new(get(3)?)).map_err(|e|e.to_string())?;std::fs::write(get(4)?,serde_json::to_vec_pretty(&package).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;}
        "preview"=>{let model=p.import(&std::fs::read(get(2)?).map_err(|e|e.to_string())?)?;let seed=args.get(3).map(|s|s.parse::<u64>()).transpose().map_err(|e|e.to_string())?.unwrap_or(7);println!("{}",p.preview(&model,seed)?);}
        _=>return Err("Usage: cytoville-ums describe | init FILE | validate FILE | compile SOURCE GAME_ROOT OUTPUT | preview FILE".into()),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
