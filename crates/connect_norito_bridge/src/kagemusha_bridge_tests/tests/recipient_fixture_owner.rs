use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use super::*;

const FIXTURE_STAGE_ENV: &str = "IROHA_CONNECT_RECIPIENT_FIXTURE_STAGE";

fn fixture_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2 + bytes.len() / 32 + 1);
    for chunk in bytes.chunks(32) {
        for byte in chunk {
            write!(encoded, "{byte:02x}").expect("write fixture hex");
        }
        encoded.push('\n');
    }
    encoded
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("connect crate is nested below the repository root")
        .to_path_buf()
}

fn fixture_output_root() -> (PathBuf, bool) {
    let checked = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let Some(raw_stage) = env::var_os("IROHA_CONNECT_RECIPIENT_FIXTURE_STAGE") else {
        return (checked, false);
    };
    assert!(
        !raw_stage.is_empty(),
        "{FIXTURE_STAGE_ENV} must not be empty"
    );
    let stage = PathBuf::from(raw_stage);
    assert!(
        stage.is_absolute(),
        "{FIXTURE_STAGE_ENV} must be an absolute path"
    );
    let metadata = fs::symlink_metadata(&stage).expect("inspect recipient fixture stage");
    assert!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "recipient fixture stage must be a regular directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o700,
            "recipient fixture stage must have mode 0700"
        );
    }
    let canonical_stage = fs::canonicalize(&stage).expect("canonicalize recipient stage");
    let canonical_repository =
        fs::canonicalize(repository_root()).expect("canonicalize repository root");
    assert!(
        !canonical_stage.starts_with(&canonical_repository),
        "recipient fixture stage must be outside the repository"
    );
    assert!(
        fs::read_dir(&stage)
            .expect("read recipient fixture stage")
            .next()
            .is_none(),
        "recipient fixture stage must be empty"
    );
    (stage, true)
}

fn publish_or_check(fixtures: &[(&str, Vec<u8>)]) {
    let (root, write) = fixture_output_root();
    for (name, bytes) in fixtures {
        let expected = fixture_hex(bytes);
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
                .write_all(expected.as_bytes())
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
                actual == expected.as_bytes(),
                "recipient fixture is stale: {} (expected {} bytes, found {})",
                path.display(),
                expected.len(),
                actual.len()
            );
        }
    }
    if write {
        let mut actual = fs::read_dir(&root)
            .expect("read completed recipient fixture stage")
            .map(|entry| {
                entry
                    .expect("read recipient fixture stage entry")
                    .file_name()
            })
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = fixtures
            .iter()
            .map(|(name, _)| (*name).into())
            .collect::<Vec<std::ffi::OsString>>();
        expected.sort();
        assert_eq!(actual, expected, "recipient fixture stage is not exact");
        #[cfg(unix)]
        fs::File::open(&root)
            .expect("open recipient fixture stage")
            .sync_all()
            .expect("sync recipient fixture stage");
    }
}

#[test]
#[ignore = "registered fixture owner; checks in place unless an external stage is supplied"]
fn recipient_receive_offer_v2_fixture_owner() {
    let offer = realistic_recipient_receive_offer_v2(1);
    let fresh_amount_request = recipient_offer_fresh_amount_request_v2(&offer);
    let publisher_key_pair = receiver_offer_publisher_key_pair_v1();
    let publisher_public_key = publisher_key_pair.public_key().to_bytes().1.to_vec();
    let fixtures = [
        (
            "offline_recipient_receive_offer_v2.hex",
            norito::to_bytes(&offer).expect("encode receiver offer"),
        ),
        (
            "offline_recipient_payment_request_v2.hex",
            norito::to_bytes(&offer.request).expect("encode recipient request"),
        ),
        (
            "offline_recipient_payment_request_v2_fresh_amount.hex",
            norito::to_bytes(&fresh_amount_request).expect("encode fresh-amount recipient request"),
        ),
        (
            "offline_recipient_registration_lineage_v2.hex",
            norito::to_bytes(&offer.lineage).expect("encode recipient lineage"),
        ),
        (
            "offline_recipient_checkpoint_envelope.hex",
            offer
                .publisher_checkpoint_envelope
                .clone()
                .expect("receiver offer publisher envelope"),
        ),
        (
            "offline_recipient_checkpoint_publisher_public_key.hex",
            publisher_public_key,
        ),
        (
            "offline_recipient_trusted_checkpoint_v2.hex",
            recipient_offer_trusted_checkpoint_v2(&offer).to_vec(),
        ),
    ];
    publish_or_check(&fixtures);
}
