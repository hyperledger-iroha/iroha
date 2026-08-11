use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use super::*;

const STAGE_ENV: &str = "IROHA_MOCHI_REPLAY_FIXTURE_STAGE";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mochi-integration is nested below the repository root")
        .to_path_buf()
}

fn fixture_output_root() -> (PathBuf, bool) {
    let checked = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("torii_replay");
    let Some(raw_stage) = env::var_os("IROHA_MOCHI_REPLAY_FIXTURE_STAGE") else {
        return (checked, false);
    };
    assert!(!raw_stage.is_empty(), "{STAGE_ENV} must not be empty");
    let stage = PathBuf::from(raw_stage);
    assert!(stage.is_absolute(), "{STAGE_ENV} must be an absolute path");
    let metadata = fs::symlink_metadata(&stage).expect("inspect Mochi replay fixture stage");
    assert!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Mochi replay fixture stage must be a regular directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o700,
            "Mochi replay fixture stage must have mode 0700"
        );
    }
    let canonical_stage = fs::canonicalize(&stage).expect("canonicalize Mochi replay stage");
    let canonical_repository =
        fs::canonicalize(repository_root()).expect("canonicalize repository root");
    assert!(
        !canonical_stage.starts_with(&canonical_repository),
        "Mochi replay fixture stage must be outside the repository"
    );
    assert!(
        fs::read_dir(&stage)
            .expect("read Mochi replay fixture stage")
            .next()
            .is_none(),
        "Mochi replay fixture stage must be empty"
    );
    (stage, true)
}

fn publish_or_check(fixtures: &[(&str, Vec<u8>)]) {
    let (root, write) = fixture_output_root();
    for (name, expected) in fixtures {
        let path = root.join(name);
        if write {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;

                options.mode(0o600);
            }
            let mut output = options.open(&path).unwrap_or_else(|error| {
                panic!("create staged fixture {}: {error}", path.display())
            });
            output
                .write_all(expected)
                .unwrap_or_else(|error| panic!("write staged fixture {}: {error}", path.display()));
            output
                .sync_all()
                .unwrap_or_else(|error| panic!("sync staged fixture {}: {error}", path.display()));
        } else {
            let metadata = fs::symlink_metadata(&path)
                .unwrap_or_else(|error| panic!("inspect fixture {}: {error}", path.display()));
            assert!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "checked fixture must be a regular file: {}",
                path.display()
            );
            let actual = fs::read(&path)
                .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
            assert!(
                actual.as_slice() == expected.as_slice(),
                "Mochi replay fixture is stale: {} (expected {} bytes, found {})",
                path.display(),
                expected.len(),
                actual.len()
            );
        }
    }
    if write {
        let mut actual = fs::read_dir(&root)
            .expect("read completed Mochi replay fixture stage")
            .map(|entry| {
                entry
                    .expect("read Mochi replay fixture stage entry")
                    .file_name()
            })
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = fixtures
            .iter()
            .map(|(name, _)| (*name).into())
            .collect::<Vec<std::ffi::OsString>>();
        expected.sort();
        assert_eq!(actual, expected, "Mochi replay fixture stage is not exact");
        #[cfg(unix)]
        fs::File::open(&root)
            .expect("open Mochi replay fixture stage")
            .sync_all()
            .expect("sync Mochi replay fixture stage");
    }
}

fn json_fixture<T: json::JsonSerialize + ?Sized>(value: &T, error: &'static str) -> Vec<u8> {
    let mut bytes = json::to_vec_pretty(value).expect(error);
    bytes.push(b'\n');
    bytes
}

fn fixture_bytes(data: &MockToriiData) -> [(&'static str, Vec<u8>); 6] {
    [
        (
            "status.json",
            json_fixture(&data.status, "serialize status fixture"),
        ),
        (
            "sumeragi.json",
            json_fixture(&data.sumeragi, "serialize Sumeragi status fixture"),
        ),
        (
            "sumeragi_diagnostics.json",
            json_fixture(
                &data.sumeragi_diagnostics,
                "serialize Sumeragi diagnostics fixture",
            ),
        ),
        (
            "configuration.json",
            json_fixture(&data.configuration, "serialize configuration fixture"),
        ),
        ("metrics.prom", data.metrics.as_bytes().to_vec()),
        ("query.bin", data.query_response.clone()),
    ]
}

#[test]
#[ignore = "registered fixture owner; checks in place unless an external stage is supplied"]
fn torii_replay_fixture_owner() {
    let data = MockToriiData::default();
    publish_or_check(&fixture_bytes(&data));
}
