//! `[sumeragi]` keys of the Sumeragi core: local-parameter overrides, safety-record paths and
//! retired keys (`specs/sumeragi.md` §7.4, §12.4).

use std::{path::PathBuf, time::Duration};

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};

/// The node key of `tests/fixtures/base.toml`.
const NODE_KEY: &str = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2";
const OTHER_KEY_A: &str = "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B";
const OTHER_KEY_B: &str = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn base_reader() -> ConfigReader {
    ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base config should load")
}

fn parse_inline(toml: &str) -> Result<ActualConfig, String> {
    let table = toml.parse().expect("inline TOML should parse lexically");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .map_err(|error| format!("{error:?}"))?
        .parse()
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn defaults_keep_every_local_parameter_and_place_records_next_to_kura() {
    let config = parse_inline("").expect("defaults parse");
    let sumeragi = &config.sumeragi;
    assert!(sumeragi.local.is_empty());
    assert!(sumeragi.retired_keys.is_empty());
    assert_eq!(
        sumeragi.records_dir,
        PathBuf::from("./storage-sumeragi-records")
    );
    assert_eq!(
        sumeragi.installation_log,
        PathBuf::from("./storage-sumeragi-installation.log")
    );
    assert_eq!(
        config.kura.store_dir.resolve_relative_path(),
        PathBuf::from("./storage")
    );
}

#[test]
fn default_record_paths_follow_a_custom_kura_store_dir() {
    let config = parse_inline("[kura]\nstore_dir = \"/var/lib/iroha/peer0\"\n")
        .expect("custom store dir parses");
    assert_eq!(
        config.sumeragi.records_dir,
        PathBuf::from("/var/lib/iroha/peer0-sumeragi-records")
    );
    assert_eq!(
        config.sumeragi.installation_log,
        PathBuf::from("/var/lib/iroha/peer0-sumeragi-installation.log")
    );
}

#[test]
fn every_override_reaches_the_actual_config() {
    let toml = format!(
        r#"
[sumeragi]
view_timeout_base_ms = 2500
view_timeout_max_ms = 40000
start_level_cap = 3
start_level_decay_after = 6
rebroadcast_interval_ms = 700
status_keepalive_ms = 4000
build_timeout_ms = 150
fetch_retry_ms = 300
sync_batch = 32
sync_retry_ms = 1500
sync_max_bytes = 8388608
max_observers = 16
records_dir = "/srv/sumeragi/records"
installation_log = "/srv/keys/installation.log"
retired_keys = ["{OTHER_KEY_A}", "{OTHER_KEY_B}"]
"#
    );
    let config = parse_inline(&toml).expect("overrides parse");
    let local = config.sumeragi.local;
    assert_eq!(local.t_base, Some(Duration::from_millis(2_500)));
    assert_eq!(local.t_max, Some(Duration::from_millis(40_000)));
    assert_eq!(local.start_cap, Some(3));
    assert_eq!(local.decay_after, Some(6));
    assert_eq!(local.rebroadcast_interval, Some(Duration::from_millis(700)));
    assert_eq!(local.status_keepalive, Some(Duration::from_millis(4_000)));
    assert_eq!(local.build_timeout, Some(Duration::from_millis(150)));
    assert_eq!(local.fetch_retry, Some(Duration::from_millis(300)));
    assert_eq!(local.sync_batch, Some(32));
    assert_eq!(local.sync_retry, Some(Duration::from_millis(1_500)));
    assert_eq!(local.sync_max_bytes, Some(8 * 1024 * 1024));
    assert_eq!(local.max_observers, Some(16));
    assert!(!local.is_empty());
    assert_eq!(
        config.sumeragi.records_dir,
        PathBuf::from("/srv/sumeragi/records")
    );
    assert_eq!(
        config.sumeragi.installation_log,
        PathBuf::from("/srv/keys/installation.log")
    );
    let retired: Vec<String> = config
        .sumeragi
        .retired_keys
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        retired,
        vec![OTHER_KEY_A.to_owned(), OTHER_KEY_B.to_owned()]
    );
}

#[test]
fn a_partial_override_leaves_the_other_parameters_unset() {
    let config = parse_inline("[sumeragi]\nsync_batch = 8\n").expect("partial override parses");
    let local = config.sumeragi.local;
    assert_eq!(local.sync_batch, Some(8));
    assert_eq!(local.t_base, None);
    assert_eq!(local.max_observers, None);
}

#[test]
fn relative_record_paths_resolve_against_their_config_file() {
    let config: ActualConfig = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("sumeragi_record_paths.toml"))
        .expect("config loads")
        .read_and_complete::<UserConfig>()
        .expect("user config")
        .parse()
        .expect("actual config");
    assert_eq!(
        config.sumeragi.records_dir,
        fixtures_dir().join("sumeragi/records")
    );
    assert_eq!(
        config.sumeragi.installation_log,
        fixtures_dir().join("keys/installation.log")
    );
}

#[test]
fn zero_overrides_are_rejected() {
    for key in [
        "view_timeout_base_ms",
        "view_timeout_max_ms",
        "rebroadcast_interval_ms",
        "status_keepalive_ms",
        "build_timeout_ms",
        "fetch_retry_ms",
        "sync_batch",
        "sync_retry_ms",
    ] {
        let error = parse_inline(&format!("[sumeragi]\n{key} = 0\n"))
            .expect_err("a zero override must fail");
        assert!(
            error.contains("sumeragi local-parameter overrides must be non-zero")
                && error.contains(key),
            "{key}: {error}"
        );
    }
    // Counts that may legitimately be zero are accepted.
    let config = parse_inline("[sumeragi]\nstart_level_cap = 0\nmax_observers = 0\n")
        .expect("zero start cap and observer budget parse");
    assert_eq!(config.sumeragi.local.start_cap, Some(0));
    assert_eq!(config.sumeragi.local.max_observers, Some(0));
}

#[test]
fn out_of_range_overrides_are_rejected_by_the_reader() {
    assert!(parse_inline("[sumeragi]\nsync_batch = 65536\n").is_err());
    assert!(parse_inline("[sumeragi]\nsync_max_bytes = 4294967296\n").is_err());
}

#[test]
fn records_inside_the_kura_store_are_rejected() {
    for records in ["./storage/records", "./storage", "storage/../storage/x"] {
        let error = parse_inline(&format!(
            "[kura]\nstore_dir = \"./storage\"\n[sumeragi]\nrecords_dir = \"{records}\"\ninstallation_log = \"/keys/install.log\"\n"
        ))
        .expect_err("records inside the Kura store must fail");
        assert!(
            error.contains("must not be inside kura.store_dir"),
            "{records}: {error}"
        );
    }
}

#[test]
fn an_installation_log_inside_the_records_is_rejected() {
    let error = parse_inline(
        "[sumeragi]\nrecords_dir = \"/srv/records\"\ninstallation_log = \"/srv/records/install.log\"\n",
    )
    .expect_err("a log inside the records must fail");
    assert!(
        error.contains("must be outside sumeragi.records_dir"),
        "{error}"
    );
}

#[test]
fn duplicate_retired_keys_are_rejected() {
    let error = parse_inline(&format!(
        "[sumeragi]\nretired_keys = [\"{OTHER_KEY_A}\", \"{OTHER_KEY_A}\"]\n"
    ))
    .expect_err("duplicate retired keys must fail");
    assert!(error.contains("more than once"), "{error}");
}

#[test]
fn the_node_key_cannot_be_retired() {
    let error = parse_inline(&format!("[sumeragi]\nretired_keys = [\"{NODE_KEY}\"]\n"))
        .expect_err("retiring the configured key must fail");
    assert!(
        error.contains("must not contain the node's configured key"),
        "{error}"
    );
}

#[test]
fn record_keys_are_not_mistaken_for_unknown_fields() {
    // Deny-unknown still applies inside `[sumeragi]`.
    let error = parse_inline("[sumeragi]\nrecord_dir = \"/srv/records\"\n")
        .expect_err("a misspelled key must fail");
    assert!(error.contains("record_dir"), "{error}");
}

#[test]
fn retired_block_queue_limit_and_storage_tables_are_rejected() {
    // Block limits are chain parameters; the retired ingress, limit and storage tables have no
    // node-local replacement and must fail instead of being ignored.
    for (table, field) in [
        ("block", "max_transactions"),
        ("queues", "body_bytes"),
        ("limits", "max_lanes"),
        ("storage", "body_store_max_bytes_per_height"),
    ] {
        let error = parse_inline(&format!("[sumeragi.{table}]\n{field} = 1\n"))
            .expect_err("a retired Sumeragi table must fail");
        assert!(
            error.contains("unknown parameter") && error.contains(&format!("sumeragi.{table}")),
            "retired `sumeragi.{table}` must be named as unknown: {error}"
        );
    }
}

#[test]
fn retired_body_ingress_environment_name_is_not_an_input() {
    // `SUMERAGI_QUEUES_BODY_BYTES` bound the retired `[sumeragi.queues].body_bytes`; generators
    // no longer emit it and the reader must leave it unvisited.
    let env = iroha_config_base::env::MockEnv::new().set("SUMERAGI_QUEUES_BODY_BYTES", "213909504");
    let _actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("a retired environment name is not a schema input")
        .parse()
        .expect("a retired environment name cannot alter the configuration");
    assert!(env.unvisited().contains("SUMERAGI_QUEUES_BODY_BYTES"));
}
