//! Configuration admission does not create directories or qualify a proof installation.

use super::*;
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

fn table() -> toml::Table {
    let mut table = toml::Table::new();
    for (name, value) in [
        ("scheme_id_hex", "01".repeat(32)),
        ("manifest_digest_hex", "02".repeat(32)),
        ("verifier_pack", "/var/lib/iroha/finality/pack".into()),
        (
            "producer_inventory",
            "/var/lib/iroha/finality/inventory".into(),
        ),
        (
            "server_originals",
            "/var/lib/iroha/finality/originals".into(),
        ),
        ("journal_dir", "/var/lib/iroha/finality/journal".into()),
    ] {
        table.insert(name.into(), toml::Value::String(value));
    }
    table
}

fn read(table: toml::Table) -> KagemushaLoadFinality {
    ConfigReader::new()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<KagemushaLoadFinality>()
        .unwrap()
}

fn root(selected: Option<toml::Table>) -> std::result::Result<actual::Root, String> {
    let mut override_table = toml::Table::new();
    if let Some(selected) = selected {
        let mut torii = toml::Table::new();
        torii.insert(
            "kagemusha_load_finality".into(),
            toml::Value::Table(selected),
        );
        override_table.insert("torii".into(), toml::Value::Table(torii));
    }
    ConfigReader::new()
        .read_toml_with_extends(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),
        )
        .unwrap()
        .with_toml_source(TomlSource::inline(override_table))
        .read_and_complete::<super::super::Root>()
        .map_err(|error| format!("{error:?}"))?
        .parse()
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn optional_service_stays_disabled_until_exact_installation_is_selected() {
    assert!(root(None).unwrap().torii.kagemusha_load_finality.is_none());
    let actual = root(Some(table()))
        .unwrap()
        .torii
        .kagemusha_load_finality
        .unwrap();
    // Torii's optional section uses JSON decoding instead of the direct ReadConfig path.
    assert_eq!(actual, read(table()).checked().unwrap());
    assert_eq!(actual.scheme_id, [1; 32]);
    assert_eq!(actual.manifest_digest, [2; 32]);
    assert_eq!(
        actual.verifier_pack,
        Path::new("/var/lib/iroha/finality/pack")
    );
    assert_eq!(
        actual.producer_inventory,
        Path::new("/var/lib/iroha/finality/inventory")
    );
    assert_eq!(
        actual.server_originals,
        Path::new("/var/lib/iroha/finality/originals")
    );
    assert_eq!(
        actual.journal_dir,
        Path::new("/var/lib/iroha/finality/journal")
    );
    assert_eq!(
        actual.max_pending_requests,
        defaults::torii::kagemusha_load_finality::MAX_PENDING_REQUESTS
    );
    assert_eq!(actual.native_step_timeout, Duration::from_millis(5_000));
}

#[test]
fn partial_installations_and_unknown_signing_authority_are_rejected() {
    for key in table().keys() {
        let mut partial = table();
        partial.remove(key);
        assert!(root(Some(partial)).is_err(), "missing {key}");
    }
    let mut extra = table();
    extra.insert(
        "signer_private_key".into(),
        toml::Value::String("/private/key".into()),
    );
    assert!(root(Some(extra)).is_err());
}

#[test]
fn identities_require_exact_nonzero_lowercase_hex() {
    for bad in [
        "".into(),
        "00".repeat(32),
        "FF".repeat(32),
        "01".repeat(31),
        format!(" {}", "01".repeat(32)),
        "gg".repeat(32),
    ] {
        for key in ["scheme_id_hex", "manifest_digest_hex"] {
            let mut input = table();
            input.insert(key.into(), toml::Value::String(bad.clone()));
            assert!(read(input).checked().is_err(), "{key}: {bad}");
        }
    }
}

#[test]
fn custody_paths_reject_aliases_traversal_and_unbounded_text_without_io() {
    for bad in [
        "".into(),
        "/".into(),
        "relative".into(),
        "/var/../private".into(),
        "/var/./private".into(),
        "/var//private".into(),
        "/var/private/".into(),
        "/var/\0private".into(),
        format!("/{}", "x".repeat(4096)),
    ] {
        for key in [
            "verifier_pack",
            "producer_inventory",
            "server_originals",
            "journal_dir",
        ] {
            let mut input = table();
            input.insert(key.into(), toml::Value::String(bad.clone()));
            assert!(read(input).checked().is_err(), "{key}: {bad:?}");
        }
    }
}

#[test]
fn all_resource_limits_refuse_zero_infinity_and_out_of_range_values() {
    let edits: [fn(&mut KagemushaLoadFinality); 21] = [
        |c| c.max_pending_requests = 0,
        |c| c.max_pending_requests = 65,
        |c| c.maximum_receipt_height = 1,
        |c| c.maximum_receipt_height = u64::MAX,
        |c| c.maximum_key_bytes = 0,
        |c| c.maximum_key_bytes = (1 << 30) + 1,
        |c| c.maximum_original_bytes = 0,
        |c| c.maximum_original_bytes = usize::MAX,
        |c| c.maximum_artifacts = 0,
        |c| c.maximum_artifacts = 65_537,
        |c| c.msm_bytes = (1 << 20) - 1,
        |c| c.msm_bytes = (1 << 30) + 1,
        |c| c.maximum_journal_entries = 2,
        |c| c.maximum_journal_entries = 1_000_001,
        |c| c.maximum_journal_bytes = 0,
        |c| c.maximum_journal_bytes = u64::MAX,
        |c| c.native_working_set_bytes = 0,
        |c| c.native_working_set_bytes = (1 << 30) + 1,
        |c| c.native_step_timeout_ms = 0,
        |c| c.native_step_timeout_ms = 60_001,
        |c| c.native_step_timeout_ms = u64::MAX,
    ];
    for edit in edits {
        let mut input = read(table());
        edit(&mut input);
        let mut emitter = Emitter::new();
        assert!(input.parse(&mut emitter).is_none());
        assert!(
            format!("{:?}", emitter.into_result().unwrap_err())
                .contains("torii.kagemusha_load_finality")
        );
    }
}

#[test]
fn explicit_resource_selection_reaches_actual_without_replacement() {
    let mut selected = table();
    for (key, value) in [
        ("max_pending_requests", 64),
        ("maximum_receipt_height", 2),
        ("maximum_key_bytes", 1 << 30),
        ("maximum_original_bytes", 1 << 30),
        ("maximum_artifacts", 65_536),
        ("msm_bytes", 1 << 20),
        ("maximum_journal_entries", 3),
        ("maximum_journal_bytes", 1024),
        ("native_working_set_bytes", 1024),
        ("native_step_timeout_ms", 60_000),
    ] {
        selected.insert(key.into(), toml::Value::Integer(value));
    }
    let actual = root(Some(selected))
        .unwrap()
        .torii
        .kagemusha_load_finality
        .unwrap();
    assert_eq!(actual.max_pending_requests, 64);
    assert_eq!(actual.maximum_receipt_height, 2);
    assert_eq!(actual.maximum_key_bytes, 1 << 30);
    assert_eq!(actual.maximum_original_bytes, 1 << 30);
    assert_eq!(actual.maximum_artifacts, 65_536);
    assert_eq!(actual.msm_bytes, 1 << 20);
    assert_eq!(actual.maximum_journal_entries, 3);
    assert_eq!(actual.maximum_journal_bytes, 1024);
    assert_eq!(actual.native_working_set_bytes, 1024);
    assert_eq!(actual.native_step_timeout, Duration::from_secs(60));
}

#[test]
fn debug_does_not_disclose_custody_paths() {
    let input = read(table());
    assert!(!format!("{input:?}").contains("/var/lib"));
    assert!(!format!("{:?}", input.checked().unwrap()).contains("/var/lib"));
}
