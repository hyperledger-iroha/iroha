use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self as serde_json, Value};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask workspace root")
        .to_path_buf()
}

#[test]
fn ministry_agenda_validate_example_passes() {
    let root = workspace_root();
    let proposal = root.join("docs/examples/ministry/agenda_proposal_example.json");
    let registry = root.join("docs/examples/ministry/agenda_duplicate_registry.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(&root);
    cmd.args([
        "ministry-agenda",
        "validate",
        "--proposal",
        proposal.to_str().unwrap(),
        "--registry",
        registry.to_str().unwrap(),
    ]);
    cmd.assert().success();
}

#[test]
fn ministry_agenda_duplicate_conflict_is_reported() {
    let root = workspace_root();
    let base_proposal = root.join("docs/examples/ministry/agenda_proposal_example.json");
    let registry = root.join("docs/examples/ministry/agenda_duplicate_registry.json");
    let mut payload: Value =
        serde_json::from_str(&fs::read_to_string(&base_proposal).expect("read proposal"))
            .expect("parse proposal");

    // Rewrite the first target to match the registry entry so the validator detects a conflict.
    if let Some(target) = payload
        .get_mut("targets")
        .and_then(Value::as_array_mut)
        .and_then(|array| array.get_mut(0))
        .and_then(Value::as_object_mut)
    {
        target.insert(
            "hash_hex".to_string(),
            Value::String(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
            ),
        );
    } else {
        panic!("proposal fixture is missing targets[0]");
    }

    let temp = TempDir::new().expect("temp dir");
    let modified = temp.path().join("proposal.json");
    fs::write(&modified, serde_json::to_string_pretty(&payload).unwrap())
        .expect("write modified proposal");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(&root);
    cmd.args([
        "ministry-agenda",
        "validate",
        "--proposal",
        modified.to_str().unwrap(),
        "--registry",
        registry.to_str().unwrap(),
    ]);
    let output = cmd.assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(
        stderr.contains("proposal conflicts with existing registry entries"),
        "expected duplicate conflict message, stderr={stderr}"
    );
}
