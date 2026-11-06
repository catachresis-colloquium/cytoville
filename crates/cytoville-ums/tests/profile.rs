// SPDX-License-Identifier: AGPL-3.0-or-later
use cytoville_ums::{CytovilleProfile, descriptor};
use ums_profile_sdk::{
    ProfileExporter, ProfileGenerator, ProfileImporter, ProfileValidator, SimulationAdapter,
};
#[test]
fn non_platformer_profile_roundtrips_and_previews_real_rust_rules() {
    descriptor().validate().unwrap();
    let p = CytovilleProfile;
    let model = p
        .generate(&serde_json::json!({"generator":"builtin"}))
        .unwrap();
    assert!(p.validate(&model).is_empty());
    assert_eq!(p.import(&p.export(&model).unwrap()).unwrap(), model);
    let preview = p.preview(&model, 7).unwrap();
    assert_eq!(preview["actors"][0]["specimen"], "euglena");
    assert_eq!(preview["turn"], 1);
    assert!(
        !descriptor()
            .capabilities
            .iter()
            .any(|x| x.contains("platform"))
    );
}
