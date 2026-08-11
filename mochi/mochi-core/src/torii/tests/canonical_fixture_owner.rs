use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use super::*;

const STAGE_ENV: &str = "IROHA_MOCHI_CANONICAL_FIXTURE_STAGE";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mochi-core is nested below the repository root")
        .to_path_buf()
}

fn fixture_output_root() -> (PathBuf, bool) {
    let checked = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let Some(raw_stage) = env::var_os("IROHA_MOCHI_CANONICAL_FIXTURE_STAGE") else {
        return (checked, false);
    };
    assert!(!raw_stage.is_empty(), "{STAGE_ENV} must not be empty");
    let stage = PathBuf::from(raw_stage);
    assert!(stage.is_absolute(), "{STAGE_ENV} must be an absolute path");
    let metadata = fs::symlink_metadata(&stage).expect("inspect Mochi canonical fixture stage");
    assert!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Mochi canonical fixture stage must be a regular directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o700,
            "Mochi canonical fixture stage must have mode 0700"
        );
    }
    let canonical_stage = fs::canonicalize(&stage).expect("canonicalize Mochi fixture stage");
    let canonical_repository =
        fs::canonicalize(repository_root()).expect("canonicalize repository root");
    assert!(
        !canonical_stage.starts_with(&canonical_repository),
        "Mochi canonical fixture stage must be outside the repository"
    );
    assert!(
        fs::read_dir(&stage)
            .expect("read Mochi canonical fixture stage")
            .next()
            .is_none(),
        "Mochi canonical fixture stage must be empty"
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
                "Mochi canonical fixture is stale: {} (expected {} bytes, found {})",
                path.display(),
                expected.len(),
                actual.len()
            );
        }
    }
    if write {
        let mut actual = fs::read_dir(&root)
            .expect("read completed Mochi canonical fixture stage")
            .map(|entry| {
                entry
                    .expect("read Mochi canonical fixture stage entry")
                    .file_name()
            })
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = fixtures
            .iter()
            .map(|(name, _)| (*name).into())
            .collect::<Vec<std::ffi::OsString>>();
        expected.sort();
        assert_eq!(
            actual, expected,
            "Mochi canonical fixture stage is not exact"
        );
        #[cfg(unix)]
        fs::File::open(&root)
            .expect("open Mochi canonical fixture stage")
            .sync_all()
            .expect("sync Mochi canonical fixture stage");
    }
}

fn canonical_event_fixture(message: &EventMessage, error: &'static str) -> Vec<u8> {
    let (payload, flags) = norito::codec::encode_with_header_flags(message);
    norito::core::frame_bare_with_header_flags::<EventMessage>(&payload, flags).expect(error)
}

#[test]
#[ignore = "registered fixture owner; checks in place unless an external stage is supplied"]
fn canonical_torii_binary_fixture_owner() {
    let fixtures = [
        (
            "canonical_block_wire.bin",
            sample_block()
                .canonical_wire()
                .expect("canonical wire")
                .into_vec(),
        ),
        (
            "canonical_event_message.bin",
            canonical_event_fixture(&sample_time_event_message(), "frame event message"),
        ),
        (
            "canonical_pipeline_event_message.bin",
            canonical_event_fixture(
                &sample_pipeline_event_message(),
                "frame pipeline event message",
            ),
        ),
        (
            "canonical_data_event_message.bin",
            canonical_event_fixture(&sample_data_event_message(), "frame data event message"),
        ),
    ];
    publish_or_check(&fixtures);
}
