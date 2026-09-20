//! Fixed original client descriptors, source custody and absolute deadline regressions.
use super::*;

#[test]
fn private_table_erasure_reaches_every_nested_string() {
    let mut private = PrivateTable(
        r#"
root = "root-secret"
array = ["array-secret", { password = "nested-secret", nested = ["nested-array-secret"] }]
count = 7
active = true
[account]
private_key = "account-secret"
[basic_auth]
password = "password-secret"
web_login = "login-secret"
"#
        .parse()
        .unwrap(),
    );
    let expected: toml::Table = r#"
root = ""
array = ["", { password = "", nested = [""] }]
count = 7
active = true
[account]
private_key = ""
[basic_auth]
password = ""
web_login = ""
"#
    .parse()
    .unwrap();
    zeroize::Zeroize::zeroize(&mut private);
    assert_eq!(private.0, expected);
    zeroize::Zeroize::zeroize(&mut private);
    assert_eq!(private.0, expected, "erasure remains safe on the Drop path");
}

#[test]
fn deadline_conversion_never_restarts_or_expands_the_original_remaining_budget() {
    let start = Instant::now();
    let deadline = Deadline::at(1_100, 100, start).unwrap();
    assert_eq!(deadline.end_ns, 1_100);
    assert_eq!(deadline.instant, start + Duration::from_nanos(1_000));
    for (end, now) in [
        (100, 100),
        (99, 100),
        (MAX_TRIAL_NS + 101, 100),
        (u64::MAX, 0),
    ] {
        assert!(Deadline::at(end, now, start).is_err());
    }
    assert!(Deadline::at(MAX_TRIAL_NS + 100, 100, start).is_ok());
    assert!(Deadline::at(1, 0, start).unwrap().check().is_err());
}

fn table() -> toml::Table {
    let mut table: toml::Table = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../defaults/client.toml"
    ))
    .parse()
    .unwrap();
    table
        .get_mut("account")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert(
            "chain_discriminant".to_owned(),
            toml::Value::Integer(i64::from(iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT)),
        );
    let transaction = toml::Table::from_iter([
        ("time_to_live_ms".to_owned(), toml::Value::Integer(5000)),
        ("status_timeout_ms".to_owned(), toml::Value::Integer(2000)),
        ("nonce".to_owned(), toml::Value::Boolean(false)),
    ]);
    table.insert("transaction".to_owned(), toml::Value::Table(transaction));
    table
}

#[test]
fn fixed_profile_rejects_every_alternate_source_and_unknown_nested_field() {
    fixed_config(&table()).unwrap();
    for key in [
        "extends",
        "network_id_file",
        "connect",
        "soracloud",
        "musubi",
        "unknown",
    ] {
        let mut changed = table();
        changed.insert(
            key.to_owned(),
            toml::Value::String("unopened-private-path".to_owned()),
        );
        assert!(fixed_config(&changed).is_err(), "{key}");
    }
    for key in ["transaction", "account", "basic_auth"] {
        let mut changed = table();
        changed
            .get_mut(key)
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "unknown".to_owned(),
                toml::Value::String("private".to_owned()),
            );
        assert!(fixed_config(&changed).is_err(), "{key}");
        changed.remove(key);
        assert!(fixed_config(&changed).is_err(), "missing {key}");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod original {
    use super::*;
    use std::{
        fs::{self, File},
        os::{fd::AsRawFd as _, unix::fs::OpenOptionsExt as _},
    };

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, File, Args) {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .canonicalize()
            .unwrap()
            .join("peer0-client.toml");
        let table = table();
        let bytes = zeroize::Zeroizing::new(toml::to_string(&table).unwrap());
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap()
            .write_all(bytes.as_bytes())
            .unwrap();
        let fd = File::open(&path).unwrap();
        let mut args = super::super::super::tests::arguments();
        args.network_id = table["network_id"].as_str().unwrap().parse().unwrap();
        args.client_config_sha256 = hex::encode(iroha_crypto::sha256(bytes.as_bytes()));
        args.client_config_max_bytes = MAX_CONFIG_BYTES;
        args.total_max_bytes = 32 * MIB as u64;
        args.deadline_monotonic_ns = monotonic_ns().unwrap() + 60_000_000_000;
        (root, path, fd, args)
    }

    #[test]
    fn exact_original_descriptor_loads_without_reopening_a_different_client() {
        let (_root, path, fd, args) = fixture();
        let (owner, config) = OriginalClient::admit(&args, fd.as_raw_fd() as u32, &path).unwrap();
        assert_eq!(config.network_id, args.network_id);
        owner.check().unwrap();
        owner.client(&config).unwrap();
    }

    #[test]
    fn equal_byte_foreign_descriptor_and_original_source_replacement_are_rejected() {
        let (_root, path, fd, args) = fixture();
        let foreign = path.with_extension("foreign");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&foreign)
            .unwrap()
            .write_all(&fs::read(&path).unwrap())
            .unwrap();
        let other = File::open(&foreign).unwrap();
        assert!(OriginalClient::admit(&args, other.as_raw_fd() as u32, &path).is_err());
        let (owner, _) = OriginalClient::admit(&args, fd.as_raw_fd() as u32, &path).unwrap();
        fs::rename(foreign, &path).unwrap();
        assert!(owner.check().is_err());
        assert!(owner.check().is_err());
    }

    #[test]
    fn changed_original_bytes_and_independent_network_or_digest_fail_closed() {
        for mutation in 0..3 {
            let (_root, path, fd, mut args) = fixture();
            if mutation == 0 {
                args.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign"),
                ));
                assert!(OriginalClient::admit(&args, fd.as_raw_fd() as u32, &path).is_err());
            } else if mutation == 1 {
                args.client_config_sha256 = "00".repeat(32);
                assert!(OriginalClient::admit(&args, fd.as_raw_fd() as u32, &path).is_err());
            } else {
                let (owner, _) =
                    OriginalClient::admit(&args, fd.as_raw_fd() as u32, &path).unwrap();
                fs::write(path, b"changed private bytes").unwrap();
                assert!(owner.check().is_err());
            }
        }
    }

    #[test]
    fn original_expired_deadline_refuses_config_and_never_creates_outputs() {
        let (_root, path, fd, mut args) = fixture();
        args.deadline_monotonic_ns = 1;
        args.finality_out = path.with_extension("finality");
        args.queries_out = path.with_extension("queries");
        let mut stdout = Vec::new();
        assert!(
            args.clone()
                .run_with_inherited(fd.as_raw_fd() as u32, &path, &mut stdout)
                .is_err()
        );
        assert!(stdout.is_empty());
        assert!(!args.finality_out.exists() && !args.queries_out.exists());
    }
}
