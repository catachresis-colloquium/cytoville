// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::Content;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub format: String,
    pub profile: String,
    pub version: u32,
    pub content_sha256: String,
    pub content: Value,
}
fn digest(content: &Value) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(content).map_err(|e| e.to_string())?)
    ))
}
impl Package {
    pub fn compile(content: Value) -> Result<Self, String> {
        let typed: Content = serde_json::from_value(content.clone()).map_err(|e| e.to_string())?;
        typed.validate()?;
        Ok(Self {
            format: "cytoville-package/1".into(),
            profile: "cytoville".into(),
            version: 1,
            content_sha256: digest(&content)?,
            content,
        })
    }
    pub fn load(&self) -> Result<Content, String> {
        if self.format != "cytoville-package/1"
            || self.profile != "cytoville"
            || self.version != 1
            || self.content_sha256 != digest(&self.content)?
        {
            return Err("Package version, profile or content digest mismatch".into());
        }
        let content: Content =
            serde_json::from_value(self.content.clone()).map_err(|e| e.to_string())?;
        content.validate()?;
        Ok(content)
    }
}
