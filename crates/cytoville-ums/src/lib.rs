// SPDX-License-Identifier: AGPL-3.0-or-later
//! Executable UMS services for a city-map life simulation, using the real SDK.
use cytoville_core::{Command, Content, apply, package::Package, turn};
use serde_json::{Value, json};
use std::{path::Path, sync::OnceLock};
use ums_profile_sdk::{
    ProfileCompiler, ProfileDescriptor, ProfileError, ProfileExporter, ProfileGenerator,
    ProfileImporter, ProfileValidator, SimulationAdapter,
};
pub struct CytovilleProfile;
pub fn descriptor() -> &'static ProfileDescriptor {
    static PROFILE: OnceLock<ProfileDescriptor> = OnceLock::new();
    PROFILE.get_or_init(|| {
        let p: ProfileDescriptor = serde_json::from_str(include_str!(
            "../../../profiles/cytoville/v1/descriptor.json"
        ))
        .expect("profile JSON");
        p.validate().expect("valid profile");
        p
    })
}
fn content(model: &Value) -> Result<Value, String> {
    if model["profile"]["id"] != "cytoville" || model["profile"]["version"] != "1.0.0" {
        return Err("Expected Cytoville profile 1.0.0".into());
    }
    let value = model["content"].clone();
    let c: Content = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    c.validate()?;
    Ok(value)
}
impl ProfileValidator for CytovilleProfile {
    fn validate(&self, model: &Value) -> Vec<String> {
        content(model).err().into_iter().collect()
    }
}
impl ProfileImporter for CytovilleProfile {
    fn import(&self, source: &[u8]) -> Result<Value, String> {
        let model: Value = serde_json::from_slice(source).map_err(|e| e.to_string())?;
        content(&model)?;
        Ok(model)
    }
}
impl ProfileExporter for CytovilleProfile {
    fn export(&self, model: &Value) -> Result<Vec<u8>, String> {
        content(model)?;
        serde_json::to_vec_pretty(model).map_err(|e| e.to_string())
    }
}
impl ProfileGenerator for CytovilleProfile {
    fn generate(&self, request: &Value) -> Result<Value, String> {
        if request["generator"] != "builtin" {
            return Err("Unknown generator".into());
        }
        let c: Value = serde_json::from_str(include_str!("../../../assets/content/cytoville.json"))
            .map_err(|e| e.to_string())?;
        Ok(json!({"profile":{"id":"cytoville","version":"1.0.0"},"content":c}))
    }
}
impl ProfileCompiler for CytovilleProfile {
    fn compile(&self, source: &Path, root: &Path) -> Result<Value, ProfileError> {
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(root.join("contracts/cytoville/v1/manifest.json"))
                .map_err(|e| ProfileError(e.to_string()))?,
        )
        .map_err(|e| ProfileError(e.to_string()))?;
        if manifest["profile"] != "cytoville" || manifest["packageFormat"] != "cytoville-package/1"
        {
            return Err(ProfileError("Game contract mismatch".into()));
        }
        let model = self
            .import(&std::fs::read(source).map_err(|e| ProfileError(e.to_string()))?)
            .map_err(ProfileError)?;
        let package =
            Package::compile(content(&model).map_err(ProfileError)?).map_err(ProfileError)?;
        serde_json::to_value(package).map_err(|e| ProfileError(e.to_string()))
    }
}
impl SimulationAdapter for CytovilleProfile {
    fn preview(&self, model: &Value, seed: u64) -> Result<Value, String> {
        let c: Content = serde_json::from_value(content(model)?).map_err(|e| e.to_string())?;
        let mut g = turn::new_game(
            &c,
            &c.specimen_order[0],
            &c.scenario_order[0],
            "standard",
            seed as u32,
            true,
        )?;
        for value in model["commands"].as_array().into_iter().flatten() {
            let command: Command =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            g = apply(&c, &g, g.active, &command)?;
        }
        serde_json::to_value(g).map_err(|e| e.to_string())
    }
}
