//! Validate the node-local bounds of signerless anonymous ballots: the queue partition
//! (`[queue.anonymous_ballots]`) and Torii's isolated ingress (`[torii.anonymous_ballot_ingress]`).

use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use std::path::PathBuf;
fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}
fn parse_actual_config(inline_toml: &str) -> Result<ActualConfig, String> {
    let table: toml::Table = inline_toml.parse().expect("inline TOML should parse");
    let user = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .map_err(|error| format!("{error:?}"))?;
    user.parse().map_err(|error| format!("{error:?}"))
}
#[test]
fn anonymous_ballot_bounds_have_working_defaults_separate_from_the_ordinary_queue() {
    let config = parse_actual_config("").expect("the default configuration parses");
    let partition = config.queue.anonymous_ballots;
    assert_eq!(
        partition.pending_limit,
        defaults::queue::ANONYMOUS_BALLOT_PENDING_LIMIT
    );
    assert_eq!(partition.pending_limit.get(), 1_024);
    assert_eq!(partition.max_retained_bytes.get(), 320 * 1024 * 1024);
    assert_eq!(partition.verifier_concurrency.get(), 2);
    assert_eq!(partition.verification_rate_per_sec.get(), 8);
    assert_eq!(partition.verification_burst.get(), 16);
    // The partition's budget is its own: the ordinary queue keeps its defaults.
    assert_eq!(config.queue.capacity, defaults::queue::CAPACITY);
    assert_eq!(
        config.queue.max_retained_bytes,
        defaults::queue::MAX_RETAINED_BYTES
    );
    let ingress = config.torii.anonymous_ballot_ingress;
    assert_eq!(ingress.max_concurrent_compute_jobs.get(), 2);
    // One canonical envelope frame: the default is the data model's envelope cap itself.
    assert_eq!(
        ingress.max_body_bytes.get(),
        iroha_data_model::governance::anonymous_ballot::ANONYMOUS_BALLOT_ENVELOPE_MAX_BYTES_V1
    );
    assert_eq!(
        ingress.max_body_bytes.get(),
        defaults::torii::ANONYMOUS_BALLOT_INGRESS_MAX_BODY_BYTES_V1
    );
    assert_eq!(ingress.submissions_per_sec.get(), 16);
    assert_eq!(ingress.submission_burst.get(), 32);
    // The ordinary transaction ingress is a different corridor with its own permits.
    assert_eq!(
        config.torii.transaction_ingress.max_concurrent_compute_jobs,
        defaults::torii::TRANSACTION_INGRESS_MAX_CONCURRENT_COMPUTE_JOBS
    );
}
#[test]
fn anonymous_ballot_bound_overrides_reach_actual_config() {
    let config = parse_actual_config(
        r"
[queue.anonymous_ballots]
pending_limit = 7
max_retained_bytes = 2_000_000
verifier_concurrency = 1
verification_rate_per_sec = 3
verification_burst = 5

[torii.anonymous_ballot_ingress]
max_concurrent_compute_jobs = 4
max_body_bytes = 200_000
submissions_per_sec = 9
submission_burst = 11
",
    )
    .expect("nonzero bounds within their ranges parse");
    let partition = config.queue.anonymous_ballots;
    assert_eq!(partition.pending_limit.get(), 7);
    assert_eq!(partition.max_retained_bytes.get(), 2_000_000);
    assert_eq!(partition.verifier_concurrency.get(), 1);
    assert_eq!(partition.verification_rate_per_sec.get(), 3);
    assert_eq!(partition.verification_burst.get(), 5);
    let ingress = config.torii.anonymous_ballot_ingress;
    assert_eq!(ingress.max_concurrent_compute_jobs.get(), 4);
    assert_eq!(ingress.max_body_bytes.get(), 200_000);
    assert_eq!(ingress.submissions_per_sec.get(), 9);
    assert_eq!(ingress.submission_burst.get(), 11);
}
#[test]
fn anonymous_ballot_bounds_reject_zero_and_a_body_above_one_envelope() {
    for (section, key) in [
        ("queue.anonymous_ballots", "pending_limit"),
        ("queue.anonymous_ballots", "max_retained_bytes"),
        ("queue.anonymous_ballots", "verifier_concurrency"),
        ("queue.anonymous_ballots", "verification_rate_per_sec"),
        ("queue.anonymous_ballots", "verification_burst"),
        (
            "torii.anonymous_ballot_ingress",
            "max_concurrent_compute_jobs",
        ),
        ("torii.anonymous_ballot_ingress", "max_body_bytes"),
        ("torii.anonymous_ballot_ingress", "submissions_per_sec"),
        ("torii.anonymous_ballot_ingress", "submission_burst"),
    ] {
        let error = parse_actual_config(&format!("[{section}]\n{key} = 0\n"))
            .expect_err("a zero bound is refused");
        assert!(error.contains(key), "{section}.{key}: {error}");
    }
    // At the cap the body limit parses; one byte more is refused with the key named.
    parse_actual_config("[torii.anonymous_ballot_ingress]\nmax_body_bytes = 262_144\n")
        .expect("the envelope cap itself");
    let error = parse_actual_config("[torii.anonymous_ballot_ingress]\nmax_body_bytes = 262_145\n")
        .expect_err("a body limit above one envelope frame");
    assert!(
        error.contains("torii.anonymous_ballot_ingress.max_body_bytes"),
        "{error}"
    );
}
