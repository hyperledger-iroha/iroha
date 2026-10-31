//! Cross-platform receipt verification preserves the remote Linux custody contract.
use super::*;

fn fixture(changed: bool) -> (json::Value, json::Value, json::Value) {
    let operation = format!("update-{}", "a".repeat(32));
    let plan = norito::json!({"operation":operation,"commit":("b".repeat(40)),"network_id":"network", "deployment":{"config_root":"/srv/taira","config_release":("c".repeat(40)),"config_filename":"beacon.toml"}});
    let mut prepared_rows = Vec::new();
    let mut installed_rows = Vec::new();
    for (index, role) in VALIDATOR_SLUGS.iter().enumerate() {
        let (source, staged, original) = config_paths(&plan, role).unwrap();
        let before = norito::json!([1, (10 + index), 33152, 0, 0, 1, 100, 1000, 1000]);
        let next = norito::json!([1, (20 + index), 33152, 0, 0, 1, 90, 2000, 2000]);
        let row = norito::json!({"role":(*role),"source_path":(source),"staged_path":(staged),"original_path":(original),"changed":changed,"source_sha256":("d".repeat(64)),"output_sha256":((if changed {"e"}else{"d"}).repeat(64)),"before_stamp":before,"staged_stamp":next});
        let mut installed = row.clone();
        let mut stamp = field(
            &row,
            if changed {
                "staged_stamp"
            } else {
                "before_stamp"
            },
        )
        .unwrap()
        .clone();
        if changed {
            *stamp.get_mut(8usize).unwrap() = norito::json!(3000);
        }
        installed
            .as_object_mut()
            .unwrap()
            .insert("installed_stamp".into(), stamp);
        prepared_rows.push(row);
        installed_rows.push(installed);
    }
    let receipt = |kind: &str, rows: Vec<json::Value>| norito::json!({"schema":(format!("{PREFIX}.{kind}.v1")),"operation":(text(&plan,"operation").unwrap()),"source_commit":(text(&plan,"commit").unwrap()),"network_id":"network","rows":rows});
    let prepared = receipt("prepared", prepared_rows);
    let installed = receipt("installed", installed_rows);
    (plan, prepared, installed)
}

#[test]
fn retirement_receipt_accepts_only_bound_inode_transition_and_true_noop() {
    for changed in [false, true] {
        let (plan, prepared, installed) = fixture(changed);
        validate_retirement_records(&prepared, &installed, &plan).unwrap();
        for key in ["operation", "source_commit", "network_id"] {
            let mut foreign = installed.clone();
            *foreign.get_mut(key).unwrap() = norito::json!("foreign");
            assert!(validate_retirement_records(&prepared, &foreign, &plan).is_err());
        }
        let mut foreign = installed.clone();
        *foreign
            .get_mut("rows")
            .unwrap()
            .get_mut(0usize)
            .unwrap()
            .get_mut("installed_stamp")
            .unwrap()
            .get_mut(1usize)
            .unwrap() = norito::json!(999);
        assert!(validate_retirement_records(&prepared, &foreign, &plan).is_err());
        let mut foreign = installed.clone();
        foreign
            .get_mut("rows")
            .unwrap()
            .get_mut(0usize)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), norito::json!(true));
        assert!(validate_retirement_records(&prepared, &foreign, &plan).is_err());
    }
}

#[test]
fn retirement_receipt_rejects_substituted_roles_paths_stamps_and_digests() {
    let (plan, prepared, installed) = fixture(true);
    for key in [
        "role",
        "source_path",
        "staged_path",
        "original_path",
        "source_sha256",
        "output_sha256",
    ] {
        let mut foreign = installed.clone();
        *foreign
            .get_mut("rows")
            .unwrap()
            .get_mut(0usize)
            .unwrap()
            .get_mut(key)
            .unwrap() = norito::json!("foreign");
        assert!(
            validate_retirement_records(&prepared, &foreign, &plan).is_err(),
            "{key}"
        );
    }
    for index in [0usize, 1, 2, 3, 4, 5, 6, 7] {
        let mut foreign = installed.clone();
        *foreign
            .get_mut("rows")
            .unwrap()
            .get_mut(0usize)
            .unwrap()
            .get_mut("installed_stamp")
            .unwrap()
            .get_mut(index)
            .unwrap() = norito::json!(999);
        assert!(
            validate_retirement_records(&prepared, &foreign, &plan).is_err(),
            "{index}"
        );
    }
    let mut foreign = installed.clone();
    foreign
        .get_mut("rows")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(validate_retirement_records(&prepared, &foreign, &plan).is_err());
}

#[test]
fn retirement_rename_allows_only_ctime_change() {
    let a = norito::json!([1, 2, 33152, 0, 0, 1, 100, 1000, 2000]);
    let b = norito::json!([1, 2, 33152, 0, 0, 1, 100, 1000, 3000]);
    assert!(same_renamed_file(&a, &b).unwrap());
    let mut b = b;
    *b.get_mut(7usize).unwrap() = norito::json!(999);
    assert!(!same_renamed_file(&a, &b).unwrap());
}

#[test]
fn retirement_paths_keep_linux_spelling_on_every_client_host() {
    let (plan, prepared, installed) = fixture(true);
    let (source, staged, original) = config_paths(&plan, VALIDATOR_SLUGS[0]).unwrap();
    let parent = format!(
        "/srv/taira/taira-validator-1/releases/{}/config",
        "c".repeat(40)
    );
    let operation = format!("update-{}", "a".repeat(32));
    assert_eq!(source, format!("{parent}/beacon.toml"));
    assert_eq!(
        staged,
        format!("{parent}/.beacon.toml.{operation}.retirement-next")
    );
    assert_eq!(
        original,
        format!("{parent}/.beacon.toml.{operation}.retirement-original")
    );
    validate_retirement_records(&prepared, &installed, &plan).unwrap();
    for key in ["source_path", "staged_path", "original_path"] {
        let mut foreign = installed.clone();
        let row = foreign.get_mut("rows").unwrap().get_mut(0usize).unwrap();
        let native_spelling = text(row, key).unwrap().replace('/', "\\");
        *row.get_mut(key).unwrap() = norito::json!(native_spelling);
        assert!(
            validate_retirement_records(&prepared, &foreign, &plan).is_err(),
            "{key}"
        );
    }
}
