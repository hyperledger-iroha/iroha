use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};

#[test]
fn sorafs_compat_command_emits_matrix() {
    let output = cargo_bin_cmd!("xtask")
        .args(["sorafs-compat", "--out", "-"])
        .output()
        .expect("run sorafs-compat command");

    assert!(
        output.status.success(),
        "command exit status: {:?}",
        output.status
    );

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let value: Value = json::from_str(&stdout).expect("parse json");

    let profiles = value["profiles"]
        .as_array()
        .expect("profiles array must exist");
    assert!(
        !profiles.is_empty(),
        "compatibility matrix should list at least one profile"
    );

    let sf1 = &profiles[0];
    assert_eq!(
        sf1["canonical_handle"],
        Value::String("sorafs.sf1@1.0.0".to_owned()),
        "first profile should be the canonical sf1 entry"
    );

    let components = sf1["components"]
        .as_array()
        .expect("components array must be present");
    assert!(
        components
            .iter()
            .any(|entry| entry["name"] == Value::String("carv1_bridge_sha2_256".to_owned())),
        "component list should mention the CARv1 bridge lane"
    );

    let legacy_formats = value["legacy_formats"]
        .as_array()
        .expect("legacy formats array must exist");
    assert!(
        legacy_formats
            .iter()
            .any(|entry| entry["format"] == Value::String("carv1".to_owned())),
        "legacy formats should enumerate the CARv1 bridge"
    );
}
