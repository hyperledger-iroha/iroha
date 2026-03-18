use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    fs,
    path::{Path, PathBuf},
};

use norito::json::{self, Value};

const LANGUAGES: &[&str] = &["rust", "go", "cpp"];

#[test]
fn interop_vectors_stay_in_sync_across_languages() {
    let root = workspace_root();

    let mut expected_ids: Option<BTreeSet<String>> = None;
    let mut canonical_payloads: BTreeMap<String, Value> = BTreeMap::new();

    for &language in LANGUAGES {
        let tests_dir = interop_dir(&root, "tests/interop/soranet/interop", language);
        let fixtures_dir = interop_dir(&root, "fixtures/soranet_handshake/interop", language);

        assert!(
            tests_dir.exists(),
            "missing test fixtures directory for language {language}: {}",
            tests_dir.display()
        );
        assert!(
            fixtures_dir.exists(),
            "missing published fixtures directory for language {language}: {}",
            fixtures_dir.display()
        );

        let test_ids = collect_fixture_ids(&tests_dir);
        let fixture_ids = collect_fixture_ids(&fixtures_dir);

        if let Some(expected) = &expected_ids {
            assert_eq!(
                &test_ids, expected,
                "fixture set mismatch for language {language}"
            );
        } else {
            expected_ids = Some(test_ids.clone());
        }

        assert_eq!(
            test_ids, fixture_ids,
            "published fixtures diverge from test bundle for language {language}"
        );

        for id in &test_ids {
            let test_value = load_fixture(&tests_dir, id);
            let fixture_value = load_fixture(&fixtures_dir, id);

            assert_eq!(
                test_value, fixture_value,
                "published fixture differs from test fixture for {language}/{id}"
            );

            let normalized = normalize_fixture(&test_value, language, id);
            match canonical_payloads.entry(id.clone()) {
                Entry::Vacant(slot) => {
                    slot.insert(normalized);
                }
                Entry::Occupied(existing) => {
                    assert_eq!(
                        normalized,
                        *existing.get(),
                        "fixture {id} for language {language} diverges from canonical payload"
                    );
                }
            }
        }
    }
}

fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .expect("crate is nested under tools")
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}

fn interop_dir(root: &Path, prefix: &str, language: &str) -> PathBuf {
    root.join(prefix).join(language)
}

fn collect_fixture_ids(dir: &Path) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for entry in fs::read_dir(dir).expect("read interop directory") {
        let entry = entry.expect("read interop entry");
        if !entry.file_type().expect("interop entry type").is_file() {
            continue;
        }
        let name = entry.file_name();
        let utf = name.to_string_lossy();
        if let Some(stripped) = utf.strip_suffix(".json") {
            ids.insert(stripped.to_string());
        }
    }
    ids
}

fn load_fixture(dir: &Path, id: &str) -> Value {
    let path = dir.join(format!("{id}.json"));
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read fixture {path}: {err}",
            path = path.display()
        )
    });
    json::from_str(&contents).unwrap_or_else(|err| {
        panic!(
            "failed to parse fixture {path}: {err}",
            path = path.display()
        )
    })
}

fn normalize_fixture(value: &Value, expected_language: &str, expected_id: &str) -> Value {
    let map = match value {
        Value::Object(map) => map,
        other => panic!("expected fixture to be a JSON object, got {other:?}"),
    };

    let id_value = map
        .get("id")
        .unwrap_or_else(|| panic!("fixture missing id field"));
    let id = match id_value {
        Value::String(s) => s.as_str(),
        other => panic!("fixture id must be string, found {other:?}"),
    };
    assert_eq!(
        id, expected_id,
        "fixture id mismatch (expected {expected_id}, found {id})"
    );

    let language_value = map
        .get("language")
        .unwrap_or_else(|| panic!("fixture missing language field"));
    let language = match language_value {
        Value::String(s) => s.as_str(),
        other => panic!("fixture language must be string, found {other:?}"),
    };
    assert_eq!(
        language, expected_language,
        "fixture language mismatch (expected {expected_language}, found {language})"
    );

    let mut normalized = map.clone();
    normalized
        .remove("language")
        .expect("language field should exist in clone");

    Value::Object(normalized)
}
