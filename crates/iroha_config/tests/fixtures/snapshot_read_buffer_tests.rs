//! Configured snapshot backing-buffer limits survive parsing into actual config.

use super::*;

fn parse_snapshot_read_budget(extra: &str) -> std::result::Result<Config, String> {
    ConfigReader::new()
        .without_env()
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .map_err(|error| format!("{error:?}"))?
        .with_toml_source(TomlSource::inline(extra.parse().unwrap()))
        .read_and_complete::<UserConfig>()
        .map_err(|error| format!("{error:?}"))?
        .parse()
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn snapshot_read_buffer_default_and_explicit_budget_reach_actual_config() {
    let defaults = parse_snapshot_read_budget("").unwrap();
    assert_eq!(
        defaults.snapshot.max_read_buffer_bytes,
        defaults::snapshot::MAX_READ_BUFFER_BYTES
    );
    assert_eq!(
        defaults.snapshot.max_read_buffer_bytes,
        defaults.snapshot.max_payload_bytes
    );
    let explicit =
        parse_snapshot_read_budget("[snapshot]\nmax_read_buffer_bytes = 1610612736\n").unwrap();
    assert_eq!(explicit.snapshot.max_read_buffer_bytes.get(), 1_610_612_736);
}

#[test]
fn snapshot_read_buffer_budget_rejects_zero_and_inconsistent_resource_limits() {
    for (input, expected) in [
        (
            "[snapshot]\nmax_read_buffer_bytes = 0\n",
            "max_read_buffer_bytes",
        ),
        (
            "[snapshot]\nmax_read_buffer_bytes = 1024\n",
            "must admit snapshot.max_payload_bytes",
        ),
        (
            "[snapshot]\nmax_read_buffer_bytes = 2147483649\n",
            "must not exceed snapshot.resources.max_transient_bytes",
        ),
    ] {
        let error = parse_snapshot_read_budget(input).unwrap_err();
        assert!(error.contains(expected), "unexpected diagnostic: {error}");
    }
}
