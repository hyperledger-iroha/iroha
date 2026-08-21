//! Unit coverage for Taira localnet configuration and simulation helpers.

use super::*;

#[test]
fn simulation_config_defaults_are_valid() {
    let cfg = SimulationConfig::quick(90, 30);
    assert!(cfg.duration >= Duration::from_secs(1));
    assert!(cfg.tps >= 1);
    assert!(cfg.packet_loss_percent <= 100);
    assert!(cfg.churn_interval >= Duration::from_secs(1));
    assert!(cfg.max_height_skew_grace >= Duration::from_secs(1));
    assert!(cfg.max_transient_height_skew >= cfg.max_height_skew);
    assert!(cfg.stall_timeout >= Duration::from_secs(10));
    assert!((0.0..=1.0).contains(&cfg.max_lagged_cycle_ratio));
    assert!((0.0..=1.0).contains(&cfg.min_committed_tps_ratio));
}
#[test]
fn env_u64_respects_minimum() {
    assert_eq!(env_u64("IROHA_TAIRA_NO_SUCH_VAR", 10, 2), 10);
}
#[test]
fn env_u8_respects_closed_range() {
    assert_eq!(env_u8("IROHA_TAIRA_NO_SUCH_U8_VAR", 10, 0, 100), 10);
}
#[test]
fn env_f64_respects_minimum() {
    assert_eq!(env_f64("IROHA_TAIRA_NO_SUCH_VAR_FLOAT", 0.25, 0.0), 0.25);
}
#[test]
fn skew_breach_window_tracks_first_exceedance_and_recovers() {
    let base = Instant::now();
    let start = update_skew_breach_started(None, 3, 2, base).expect("breach should start");
    assert_eq!(start, base);
    let sustained = update_skew_breach_started(Some(start), 4, 2, base + Duration::from_secs(2))
        .expect("breach should stay active");
    assert_eq!(sustained, start);
    let recovered =
        update_skew_breach_started(Some(sustained), 2, 2, base + Duration::from_secs(3));
    assert!(recovered.is_none());
}
#[test]
fn skew_breach_is_not_unrecovering_when_min_height_progresses_recently() {
    assert!(!is_skew_breach_unrecovering(
        Duration::from_secs(20),
        Duration::from_secs(5),
        Duration::from_secs(15),
        Duration::from_secs(60),
    ));
}
#[test]
fn skew_breach_is_unrecovering_when_duration_and_min_age_exceed_thresholds() {
    assert!(is_skew_breach_unrecovering(
        Duration::from_secs(40),
        Duration::from_secs(61),
        Duration::from_secs(15),
        Duration::from_secs(60),
    ));
}
#[test]
fn queue_timeout_error_classifier_matches_expected_message() {
    let err = eyre!("transaction queued for too long");
    assert!(is_queue_timeout_error(&err));
}
#[test]
fn http_timeout_error_classifier_matches_expected_message() {
    let err = eyre!("operation timed out");
    assert!(is_http_timeout_error(&err));
}
#[test]
fn process_churn_index_rotates_across_all_validators() {
    assert_eq!(first_process_churn_index(7), 1);
    assert_eq!(next_process_churn_index(1, 7), 2);
    assert_eq!(next_process_churn_index(5, 7), 6);
    assert_eq!(next_process_churn_index(6, 7), 0);
    assert_eq!(next_process_churn_index(0, 7), 1);
}
#[test]
fn process_churn_index_handles_single_validator() {
    assert_eq!(first_process_churn_index(1), 0);
    assert_eq!(next_process_churn_index(0, 1), 0);
}
#[test]
fn select_process_churn_index_prioritizes_unresponsive_validator() {
    let observed = [Some(100), Some(101), None, Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 0, 6), 2);
}
#[test]
fn select_process_churn_index_prioritizes_lagger_when_skew_is_large() {
    let observed = [Some(100), Some(84), Some(99), Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 0, 6), 1);
}
#[test]
fn select_process_churn_index_uses_round_robin_fallback_when_balanced() {
    let observed = [Some(100), Some(98), Some(99), Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 2, 6), 2);
}
#[test]
fn select_process_churn_index_clamps_out_of_bounds_fallback() {
    let observed = [Some(10), Some(10), Some(10)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 7, 6), 2);
}
#[test]
fn next_process_churn_deadline_uses_interval_without_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_process_churn_deadline(now, interval, false);
    assert_eq!(deadline.duration_since(now), interval);
}
#[test]
fn next_process_churn_deadline_adds_backoff_without_schedule_drift() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_process_churn_deadline(now, interval, true);
    assert_eq!(
        deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn next_membership_churn_deadline_uses_interval_without_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_membership_churn_deadline(now, interval, false);
    assert_eq!(deadline.duration_since(now), interval);
}
#[test]
fn next_membership_churn_deadline_adds_backoff_when_lagged() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_membership_churn_deadline(now, interval, true);
    assert_eq!(
        deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn membership_backoff_triggers_only_on_hard_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let warning_only = MembershipCycleOutcome {
        hard_lagged: false,
        warning_lagged: true,
    };
    let warning_deadline = next_membership_churn_deadline(
        now,
        interval,
        membership_backoff_requires_hard_lag(warning_only),
    );
    assert_eq!(warning_deadline.duration_since(now), interval);
    let hard_lagged = MembershipCycleOutcome {
        hard_lagged: true,
        warning_lagged: false,
    };
    let hard_deadline = next_membership_churn_deadline(
        now,
        interval,
        membership_backoff_requires_hard_lag(hard_lagged),
    );
    assert_eq!(
        hard_deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn stalled_joiner_catchup_marks_warning_without_hard_lag() {
    let mut outcome = MembershipCycleOutcome::default();
    record_joiner_stall_warning(&mut outcome, JOINER_STALL_WARNING_THRESHOLD);
    assert!(outcome.warning_lagged);
    assert!(!outcome.hard_lagged);
}
#[test]
fn propagation_and_quorum_failures_mark_hard_lag() {
    let mut propagation_timeout = MembershipCycleOutcome::default();
    propagation_timeout.mark_hard_lag();
    assert!(propagation_timeout.hard_lagged);
    assert!(!propagation_timeout.warning_lagged);
    let mut quorum_timeout = MembershipCycleOutcome::default();
    quorum_timeout.mark_hard_lag();
    assert!(quorum_timeout.hard_lagged);
    assert!(!quorum_timeout.warning_lagged);
}
#[test]
fn effective_final_settle_window_scales_with_duration() {
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(3_600)),
        FINAL_SETTLE_WINDOW
    );
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(90)),
        Duration::from_secs(30)
    );
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(30)),
        Duration::from_secs(10)
    );
}
#[test]
fn initial_churn_delay_stays_inside_churn_window() {
    assert_eq!(
        initial_churn_delay(Duration::from_secs(30), Duration::from_secs(20)),
        Duration::from_secs(19)
    );
    assert_eq!(
        initial_churn_delay(Duration::from_secs(30), Duration::from_secs(60)),
        Duration::from_secs(30)
    );
}
#[test]
fn scheduled_churn_floor_requires_sustained_cycles() {
    assert_eq!(
        scheduled_churn_cycles(
            Duration::from_secs(60),
            Duration::from_secs(30),
            Duration::from_secs(30)
        ),
        1
    );
    assert_eq!(
        scheduled_churn_cycles(
            Duration::from_secs(60),
            Duration::from_secs(15),
            Duration::from_secs(30)
        ),
        2
    );
    assert_eq!(minimum_required_churn_cycles(1), 1);
    assert_eq!(minimum_required_churn_cycles(10), 9);
    assert_eq!(minimum_required_churn_cycles(287), 259);
    let duration = Duration::from_secs(DEFAULT_SIM_DURATION_SECS);
    let churn_window = duration.saturating_sub(effective_final_settle_window(duration));
    assert_eq!(
        scheduled_churn_cycles(
            churn_window,
            initial_churn_delay(
                Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
                churn_window
            ),
            Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
        ),
        287
    );
    assert_eq!(
        scheduled_churn_cycles(
            churn_window,
            initial_churn_delay(
                Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS / 2),
                churn_window,
            ),
            Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
        ),
        288
    );
    assert_eq!(minimum_required_churn_cycles(288), 260);
}
#[test]
fn lagged_cycle_ratio_rejects_one_bad_cycle() {
    assert_eq!(lagged_cycle_ratio(0, 0), 0.0);
    assert_eq!(lagged_cycle_ratio(1, 1), 1.0);
    assert!((lagged_cycle_ratio(1, 3) - (1.0 / 3.0)).abs() < f64::EPSILON);
    assert!(lagged_cycle_ratio(1, 1) > DEFAULT_MAX_LAGGED_CYCLE_RATIO);
}
#[test]
fn view_change_rate_uses_final_counter_and_full_soak_time() {
    assert_eq!(view_change_rate(10, 22, Duration::from_secs(60)), 0.2);
    assert_eq!(view_change_rate(22, 10, Duration::from_secs(60)), 0.0);
    assert_eq!(view_change_rate(0, 1, Duration::ZERO), 1.0);
}
fn status_with_view_changes(view_changes: u32) -> iroha::client::Status {
    iroha::client::Status {
        view_changes,
        ..iroha::client::Status::default()
    }
}
#[test]
fn view_change_tracker_accumulates_each_validator_across_restart_resets() {
    let mut first = iroha::client::Status::default();
    first.view_changes = 10;
    let mut second = iroha::client::Status::default();
    second.view_changes = 5;
    let baseline = vec![(0, first.clone()), (1, second.clone())];
    let mut tracker = ViewChangeTracker::new(3);
    tracker.establish_baseline(&baseline);
    assert_eq!(total_indexed_view_changes(&baseline), 15);
    first.view_changes = 13;
    second.view_changes = 7;
    let mut newly_observed = iroha::client::Status::default();
    newly_observed.view_changes = 4;
    tracker.observe(&[(0, first.clone()), (1, second.clone()), (2, newly_observed)]);
    assert_eq!(tracker.total_since_baseline(), 9);
    first.view_changes = 2;
    second.view_changes = 9;
    tracker.observe(&[(0, first), (1, second)]);
    assert_eq!(tracker.total_since_baseline(), 13);
}
#[test]
fn view_change_tracker_conservatively_counts_a_late_first_observation() {
    let mut tracker = ViewChangeTracker::new(2);
    tracker.establish_baseline(&[(0, status_with_view_changes(5))]);
    tracker.observe(&[
        (0, status_with_view_changes(7)),
        (1, status_with_view_changes(3)),
    ]);
    assert_eq!(tracker.total_since_baseline(), 5);
}
#[test]
fn min_txs_approved_returns_lowest_counter() {
    let mut first = iroha::client::Status::default();
    first.txs_approved = 42;
    let mut second = iroha::client::Status::default();
    second.txs_approved = 17;
    let mut third = iroha::client::Status::default();
    third.txs_approved = 99;
    assert_eq!(min_txs_approved(&[first, second, third]), 17);
}
#[test]
fn joiner_mutable_paths_are_rewritten_below_one_disjoint_root() {
    fn path_table(field: &str) -> Table {
        Table::from_iter([(
            field.to_owned(),
            TomlValue::String("/incumbent/shared-state".to_owned()),
        )])
    }
    let mut streaming = path_table("session_store_dir");
    streaming.insert(
        "soranet".into(),
        TomlValue::Table(path_table("provision_spool_dir")),
    );
    streaming.insert(
        "soravpn".into(),
        TomlValue::Table(path_table("provision_spool_dir")),
    );
    let pow = path_table("revocation_store_path");
    let mut handshake = Table::new();
    handshake.insert("pow".into(), TomlValue::Table(pow));
    let mut network = Table::new();
    network.insert("soranet_handshake".into(), TomlValue::Table(handshake));
    let mut sorafs = Table::new();
    sorafs.insert("storage".into(), TomlValue::Table(path_table("data_dir")));
    sorafs.insert("por".into(), TomlValue::Table(path_table("state_dir")));
    let mut torii = path_table("data_dir");
    torii.insert(
        "da_ingest".into(),
        TomlValue::Table(Table::from_iter([
            (
                "replay_cache_store_dir".to_owned(),
                TomlValue::String("/incumbent/shared-state".to_owned()),
            ),
            (
                "manifest_store_dir".to_owned(),
                TomlValue::String("/incumbent/shared-state".to_owned()),
            ),
        ])),
    );
    let mut root = Table::from_iter([
        ("kura".to_owned(), TomlValue::Table(path_table("store_dir"))),
        (
            "soracloud_runtime".to_owned(),
            TomlValue::Table(path_table("state_dir")),
        ),
        (
            "tiered_state".to_owned(),
            TomlValue::Table(Table::from_iter([
                (
                    "cold_store_root".to_owned(),
                    TomlValue::String("/incumbent/shared-state".to_owned()),
                ),
                (
                    "da_store_root".to_owned(),
                    TomlValue::String("/incumbent/shared-state".to_owned()),
                ),
            ])),
        ),
        ("streaming".to_owned(), TomlValue::Table(streaming)),
        ("torii".to_owned(), TomlValue::Table(torii)),
        ("network".to_owned(), TomlValue::Table(network)),
        ("sorafs".to_owned(), TomlValue::Table(sorafs)),
    ]);
    let temp_dir = tempfile::tempdir().expect("create path-isolation fixture directory");
    let joiner_root = temp_dir.path().join("storage/joiner");
    rewrite_joiner_mutable_paths(&mut root, &joiner_root)
        .expect("rewrite every joiner-owned mutable store");

    let lookup = |path: &[&str]| {
        let mut table = &root;
        for (index, key) in path.iter().enumerate() {
            let value = table.get(*key).expect("configured joiner path component");
            if index + 1 == path.len() {
                return value.as_str().expect("configured joiner path string");
            }
            table = value.as_table().expect("configured joiner path table");
        }
        unreachable!("path fixture is non-empty")
    };
    let paths = [
        lookup(&["kura", "store_dir"]),
        lookup(&["soracloud_runtime", "state_dir"]),
        lookup(&["tiered_state", "cold_store_root"]),
        lookup(&["tiered_state", "da_store_root"]),
        lookup(&["streaming", "session_store_dir"]),
        lookup(&["streaming", "soranet", "provision_spool_dir"]),
        lookup(&["streaming", "soravpn", "provision_spool_dir"]),
        lookup(&["torii", "data_dir"]),
        lookup(&["torii", "da_ingest", "replay_cache_store_dir"]),
        lookup(&["torii", "da_ingest", "manifest_store_dir"]),
        lookup(&[
            "network",
            "soranet_handshake",
            "pow",
            "revocation_store_path",
        ]),
        lookup(&["sorafs", "storage", "data_dir"]),
        lookup(&["sorafs", "por", "state_dir"]),
    ];
    assert!(
        paths
            .iter()
            .all(|path| Path::new(path).starts_with(&joiner_root)),
        "every mutable joiner store must live below the disjoint joiner root: {paths:?}"
    );
    assert_eq!(
        paths.iter().copied().collect::<BTreeSet<_>>().len(),
        paths.len(),
        "mutable joiner stores must not alias one another"
    );
}
#[test]
fn five_validator_joiner_config_scales_body_ingress_and_passes_actual_admission() {
    let validators = (0..5)
        .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let mut trusted_peers = Vec::new();
    let mut trusted_peers_pop = Vec::new();
    for (index, validator) in validators.iter().take(4).enumerate() {
        let address = canonical_loopback_addr(
            13_337_u16 + u16::try_from(index).expect("validator index fits u16"),
        );
        trusted_peers.push(TomlValue::String(format!(
            "{}@{address}",
            validator.public_key()
        )));
        let pop = bls_normal_pop_prove(validator.private_key()).expect("generate validator PoP");
        let mut entry = Table::new();
        entry.insert(
            "public_key".into(),
            TomlValue::String(validator.public_key().to_string()),
        );
        entry.insert("pop_hex".into(), TomlValue::String(hex::encode(pop)));
        trusted_peers_pop.push(TomlValue::Table(entry));
    }
    let node = &validators[0];
    let transport = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let streaming = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let genesis = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let body_source_bytes = defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let authenticated_non_validator_sources =
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get();
    let four_validator_body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        4,
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .expect("four-validator body budget");
    let four_validator_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        4,
        authenticated_non_validator_sources,
    )
    .expect("four-validator message budget");
    let mut queues = Table::new();
    queues.insert(
        "authenticated_non_validator_sources".into(),
        TomlValue::Integer(
            i64::try_from(authenticated_non_validator_sources).expect("fixture fits TOML"),
        ),
    );
    queues.insert(
        "body_source_bytes".into(),
        TomlValue::Integer(i64::try_from(body_source_bytes).expect("fixture fits TOML")),
    );
    queues.insert(
        "bodies".into(),
        TomlValue::Integer(i64::try_from(four_validator_bodies).expect("fixture fits TOML")),
    );
    queues.insert(
        "body_bytes".into(),
        TomlValue::Integer(i64::try_from(four_validator_body_bytes).expect("fixture fits TOML")),
    );
    let mut sumeragi = Table::new();
    sumeragi.insert("role".into(), TomlValue::String("validator".into()));
    sumeragi.insert("queues".into(), TomlValue::Table(queues));
    let mut network = Table::new();
    network.insert(
        "address".into(),
        TomlValue::String(canonical_loopback_addr(13_337)),
    );
    network.insert(
        "public_address".into(),
        TomlValue::String(canonical_loopback_addr(13_337)),
    );
    network.insert("max_total_connections".into(), TomlValue::Integer(32));
    let mut torii = Table::new();
    torii.insert(
        "address".into(),
        TomlValue::String(canonical_loopback_addr(18_080)),
    );
    let mut genesis_config = Table::new();
    genesis_config.insert(
        "public_key".into(),
        TomlValue::String(genesis.public_key().to_string()),
    );
    genesis_config.insert(
        "expected_hash".into(),
        TomlValue::String(
            "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E".to_owned(),
        ),
    );
    let mut streaming_config = Table::new();
    streaming_config.insert(
        "identity_public_key".into(),
        TomlValue::String(streaming.public_key().to_string()),
    );
    streaming_config.insert(
        "identity_private_key".into(),
        TomlValue::String(ExposedPrivateKey(streaming.private_key().clone()).to_string()),
    );
    let mut root = Table::new();
    root.insert(
        "chain".into(),
        TomlValue::String("00000000-0000-0000-0000-000000000000".into()),
    );
    root.insert(
        "public_key".into(),
        TomlValue::String(node.public_key().to_string()),
    );
    root.insert(
        "private_key".into(),
        TomlValue::String(ExposedPrivateKey(node.private_key().clone()).to_string()),
    );
    root.insert(
        "soranet_transport_public_key".into(),
        TomlValue::String(transport.public_key().to_string()),
    );
    root.insert(
        "soranet_transport_private_key".into(),
        TomlValue::String(ExposedPrivateKey(transport.private_key().clone()).to_string()),
    );
    root.insert("trusted_peers".into(), TomlValue::Array(trusted_peers));
    root.insert(
        "trusted_peers_pop".into(),
        TomlValue::Array(trusted_peers_pop),
    );
    root.insert("sumeragi".into(), TomlValue::Table(sumeragi));
    root.insert("network".into(), TomlValue::Table(network));
    root.insert("torii".into(), TomlValue::Table(torii));
    root.insert("genesis".into(), TomlValue::Table(genesis_config));
    root.insert("streaming".into(), TomlValue::Table(streaming_config));

    let required_body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        5,
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .expect("five-validator body budget");
    let required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        5,
        authenticated_non_validator_sources,
    )
    .expect("five-validator message budget");
    ensure_sumeragi_body_ingress(&mut root, 5)
        .expect("reserve the fifth validator partition before bootstrap");
    assert_eq!(
        required_body_bytes,
        7 * body_source_bytes,
        "five validators and two authenticated non-validator sources require seven isolated byte partitions"
    );
    let startup_emitted = toml::to_string(&root).expect("serialize incumbent validator config");
    let startup_emitted = startup_emitted
        .parse::<Table>()
        .expect("incumbent validator config is TOML");
    let startup_admitted = actual::Root::from_toml_source(TomlSource::inline(startup_emitted))
        .expect("four-validator bootstrap config with reserved joiner capacity passes admission");
    assert_eq!(
        startup_admitted
            .common
            .trusted_peers
            .value()
            .validator_roster_len(),
        4
    );
    assert_eq!(
        startup_admitted.sumeragi.queues.bodies.get(),
        required_bodies,
        "incumbent validators must reserve the post-registration protected message slots"
    );
    assert_eq!(
        startup_admitted.sumeragi.queues.body_bytes.get(),
        required_body_bytes,
        "incumbent validators must already cover the post-registration runtime roster"
    );

    let joiner = &validators[4];
    let joiner_pop =
        bls_normal_pop_prove(joiner.private_key()).expect("generate joiner validator PoP");
    append_joiner_validator_and_scale_body_ingress(
        &mut root,
        joiner.public_key(),
        &joiner_pop,
        &canonical_loopback_addr(13_341),
    )
    .expect("append joiner and scale its ingress budget");
    assert_eq!(
        root.get("sumeragi")
            .and_then(TomlValue::as_table)
            .and_then(|sumeragi| sumeragi.get("queues"))
            .and_then(TomlValue::as_table)
            .and_then(|queues| queues.get("bodies"))
            .and_then(TomlValue::as_integer),
        Some(i64::try_from(required_bodies).expect("fixture fits TOML"))
    );
    assert_eq!(
        root.get("sumeragi")
            .and_then(TomlValue::as_table)
            .and_then(|sumeragi| sumeragi.get("queues"))
            .and_then(TomlValue::as_table)
            .and_then(|queues| queues.get("body_bytes"))
            .and_then(TomlValue::as_integer),
        Some(i64::try_from(required_body_bytes).expect("fixture fits TOML"))
    );
    let emitted = toml::to_string(&root).expect("serialize emitted joiner config");
    let emitted = emitted
        .parse::<Table>()
        .expect("emitted joiner config is TOML");
    let admitted = actual::Root::from_toml_source(TomlSource::inline(emitted))
        .expect("emitted five-validator joiner config passes canonical admission");
    assert_eq!(
        admitted.common.trusted_peers.value().validator_roster_len(),
        5
    );
    assert_eq!(admitted.sumeragi.queues.bodies.get(), required_bodies);
    assert_eq!(
        admitted.sumeragi.queues.body_bytes.get(),
        required_body_bytes
    );
}
#[test]
fn joiner_body_ingress_scaling_preserves_larger_authored_capacity() {
    let body_source_bytes = defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let authenticated_non_validator_sources =
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get();
    let authored_body_bytes = 9 * body_source_bytes;
    let required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        5,
        authenticated_non_validator_sources,
    )
    .expect("five-validator message budget");
    let authored_bodies = required_bodies + 7;
    let mut queues = Table::new();
    queues.insert(
        "authenticated_non_validator_sources".into(),
        TomlValue::Integer(
            i64::try_from(authenticated_non_validator_sources).expect("fixture fits TOML"),
        ),
    );
    queues.insert(
        "body_source_bytes".into(),
        TomlValue::Integer(i64::try_from(body_source_bytes).expect("fixture fits TOML")),
    );
    queues.insert(
        "bodies".into(),
        TomlValue::Integer(i64::try_from(authored_bodies).expect("fixture fits TOML")),
    );
    queues.insert(
        "body_bytes".into(),
        TomlValue::Integer(i64::try_from(authored_body_bytes).expect("fixture fits TOML")),
    );
    let mut sumeragi = Table::new();
    sumeragi.insert("queues".into(), TomlValue::Table(queues));
    let mut root = Table::new();
    root.insert("sumeragi".into(), TomlValue::Table(sumeragi));

    ensure_sumeragi_body_ingress(&mut root, 5).expect("larger authored budget is valid");
    assert_eq!(
        root.get("sumeragi")
            .and_then(TomlValue::as_table)
            .and_then(|sumeragi| sumeragi.get("queues"))
            .and_then(TomlValue::as_table)
            .and_then(|queues| queues.get("bodies"))
            .and_then(TomlValue::as_integer),
        Some(i64::try_from(authored_bodies).expect("fixture fits TOML"))
    );
    assert_eq!(
        root.get("sumeragi")
            .and_then(TomlValue::as_table)
            .and_then(|sumeragi| sumeragi.get("queues"))
            .and_then(TomlValue::as_table)
            .and_then(|queues| queues.get("body_bytes"))
            .and_then(TomlValue::as_integer),
        Some(i64::try_from(authored_body_bytes).expect("fixture fits TOML"))
    );
}
#[test]
fn planned_validator_capacity_is_reserved_for_every_incumbent_config() {
    let temp_dir = tempfile::tempdir().expect("create localnet fixture directory");
    let body_source_bytes = defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
    let authenticated_non_validator_sources =
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get();
    let bootstrap_body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        usize::from(TAIRA_VALIDATORS),
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .expect("bootstrap body budget");
    let bootstrap_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        usize::from(TAIRA_VALIDATORS),
        authenticated_non_validator_sources,
    )
    .expect("bootstrap message budget");
    for idx in 0..TAIRA_VALIDATORS {
        fs::write(
            temp_dir.path().join(format!("peer{idx}.toml")),
            format!(
                "[sumeragi.queues]\nauthenticated_non_validator_sources = {authenticated_non_validator_sources}\nbodies = {bootstrap_bodies}\nbody_source_bytes = {body_source_bytes}\nbody_bytes = {bootstrap_body_bytes}\n"
            ),
        )
        .expect("write incumbent config fixture");
    }

    reserve_localnet_validator_body_ingress(
        temp_dir.path(),
        TAIRA_VALIDATORS,
        usize::from(TAIRA_TOTAL_PORT_SLOTS),
    )
    .expect("reserve planned joiner capacity");

    let required_body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        usize::from(TAIRA_TOTAL_PORT_SLOTS),
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .expect("post-registration body budget");
    let required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        usize::from(TAIRA_TOTAL_PORT_SLOTS),
        authenticated_non_validator_sources,
    )
    .expect("post-registration message budget");
    for idx in 0..TAIRA_VALIDATORS {
        let config_path = temp_dir.path().join(format!("peer{idx}.toml"));
        let config: TomlValue = toml::from_str(
            &fs::read_to_string(&config_path).expect("read rewritten incumbent config"),
        )
        .expect("rewritten incumbent config is TOML");
        assert_eq!(
            config
                .get("sumeragi")
                .and_then(TomlValue::as_table)
                .and_then(|sumeragi| sumeragi.get("queues"))
                .and_then(TomlValue::as_table)
                .and_then(|queues| queues.get("bodies"))
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(required_bodies).expect("fixture fits TOML")),
            "peer{idx} must reserve the joiner's protected message slots"
        );
        assert_eq!(
            config
                .get("sumeragi")
                .and_then(TomlValue::as_table)
                .and_then(|sumeragi| sumeragi.get("queues"))
                .and_then(TomlValue::as_table)
                .and_then(|queues| queues.get("body_bytes"))
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(required_body_bytes).expect("fixture fits TOML")),
            "peer{idx} must reserve the joiner's validator-source partition"
        );
    }
}
#[test]
fn apply_queue_transaction_ttl_updates_queue_section() {
    let mut root = Table::new();
    root.insert("queue".into(), TomlValue::Table(Table::new()));
    apply_queue_transaction_ttl(&mut root, 7_200_000).expect("queue ttl should apply");
    let applied = root
        .get("queue")
        .and_then(TomlValue::as_table)
        .and_then(|queue| {
            queue
                .get("transaction_time_to_live_ms")
                .and_then(TomlValue::as_integer)
        });
    assert_eq!(applied, Some(7_200_000));
}
#[test]
fn apply_client_transaction_ttl_caps_status_timeout() {
    let mut transaction = Table::new();
    transaction.insert("time_to_live_ms".into(), TomlValue::Integer(600_000));
    transaction.insert("status_timeout_ms".into(), TomlValue::Integer(900_000));
    let mut root = Table::new();
    root.insert("transaction".into(), TomlValue::Table(transaction));
    apply_client_transaction_ttl(&mut root, 300_000).expect("client ttl should apply");
    let tx = root
        .get("transaction")
        .and_then(TomlValue::as_table)
        .expect("transaction section should exist");
    assert_eq!(
        tx.get("time_to_live_ms").and_then(TomlValue::as_integer),
        Some(300_000)
    );
    assert_eq!(
        tx.get("status_timeout_ms").and_then(TomlValue::as_integer),
        Some(300_000)
    );
}
#[test]
fn apply_packet_impairment_sets_both_directions() {
    let mut root = Table::new();
    root.insert("network".into(), TomlValue::Table(Table::new()));
    apply_packet_impairment(&mut root, 10).expect("packet impairment should apply");
    let network = root
        .get("network")
        .and_then(TomlValue::as_table)
        .expect("network section should exist");
    assert_eq!(
        network
            .get("debug_packet_loss_inbound_percent")
            .and_then(TomlValue::as_integer),
        Some(10)
    );
    assert_eq!(
        network
            .get("debug_packet_loss_outbound_percent")
            .and_then(TomlValue::as_integer),
        Some(10)
    );
    assert!(apply_packet_impairment(&mut root, 101).is_err());
}
#[test]
fn joiner_stall_warning_threshold_matches_policy() {
    assert!(!should_count_joiner_stall_as_warning(0));
    assert!(!should_count_joiner_stall_as_warning(1));
    assert!(!should_count_joiner_stall_as_warning(2));
    assert!(should_count_joiner_stall_as_warning(3));
}
#[test]
fn release_execution_profile_accepts_only_the_exact_positive_profile() {
    let profile = validate_release_execution_profile("release", "release", "true")
        .expect("exact release/offline profile");
    assert_eq!(profile.build_profile, "release");
    assert!(profile.cargo_net_offline);
}
#[test]
fn release_execution_profile_rejects_wrong_or_blank_build_profiles() {
    for build_profile in ["", "debug", " release", "release "] {
        assert!(
            validate_release_execution_profile(build_profile, build_profile, "true").is_err(),
            "unexpectedly accepted build profile {build_profile:?}"
        );
    }
}
#[test]
fn release_execution_profile_rejects_cargo_profile_mismatch() {
    for cargo_profile in ["", "debug", "release ", "Release"] {
        assert!(
            validate_release_execution_profile("release", cargo_profile, "true").is_err(),
            "unexpectedly accepted Cargo profile {cargo_profile:?}"
        );
    }
}
#[test]
fn release_execution_profile_rejects_non_exact_offline_values() {
    for cargo_net_offline in ["", "1", "TRUE", " true", "true ", "false"] {
        assert!(
            validate_release_execution_profile("release", "release", cargo_net_offline).is_err(),
            "unexpectedly accepted CARGO_NET_OFFLINE={cargo_net_offline:?}"
        );
    }
}
fn sample_simulation_summary() -> SimulationSummary {
    SimulationSummary {
        git_revision: "1".repeat(40),
        workspace_source_manifest_sha256: "a".repeat(64),
        build_profile: "release".to_owned(),
        cargo_net_offline: true,
        localnet_artifact_path: "/tmp/taira-localnet".to_owned(),
        daemon_binary_path: "/tmp/iroha3d".to_owned(),
        daemon_binary_blake2b_256: "b".repeat(64),
        kagami_binary_path: "/tmp/kagami".to_owned(),
        kagami_binary_blake2b_256: "c".repeat(64),
        test_binary_path: "/tmp/taira-test".to_owned(),
        test_binary_blake2b_256: "d".repeat(64),
        generated_config_blake2b_256: "e".repeat(64),
        seed: "taira-public-sim".to_owned(),
        duration_secs: 60,
        target_tps: 5,
        packet_loss_percent: 10,
        churn_interval_secs: 300,
        max_height_skew: 2,
        max_height_skew_grace_secs: 30,
        max_transient_height_skew: 32,
        stall_timeout_secs: 300,
        max_view_change_rate: 0.2,
        max_lagged_cycle_ratio: 0.35,
        min_committed_tps_ratio: 0.6,
        process_downtime_secs: 5,
        tx_attempted: 300,
        tx_sent: 295,
        tx_submit_errors: 0,
        process_churn_cycles: 4,
        expected_process_churn_cycles: 4,
        process_churn_lagged_cycles: 0,
        membership_join_cycles: 3,
        membership_leave_cycles: 3,
        expected_membership_churn_cycles: 6,
        membership_cleanup_leave: false,
        membership_churn_lagged_cycles: 1,
        membership_churn_warning_cycles: 2,
        churn_paused_secs: 5.0,
        churn_paused_ratio: 1.0 / 12.0,
        soak_overrun_secs: 0.0,
        max_height_skew_observed: 1,
        view_changes_start: 0,
        view_changes_end: 0,
        view_change_rate_per_sec: 0.0,
        scheduled_tps: 5.0,
        submitted_tps: 4.9,
        committed_tps: 4.8,
        committed_txs_min_delta: 288,
        saturated_samples: 0,
        total_samples: 60,
        initial_status_snapshots: vec![norito::json!({"height": 1_u64})],
        final_status_snapshots: vec![norito::json!({"height": 61_u64})],
        no_progress_intervals: vec![NoProgressInterval {
            start_elapsed_ms: 1_000,
            end_elapsed_ms: 2_000,
            classifications: vec!["commit_quorum_missing".to_owned()],
            classified: true,
            status_snapshots: Vec::new(),
        }],
        unclassified_no_progress_intervals: 0,
    }
}
#[test]
fn simulation_summary_json_records_release_profile_and_status_evidence() {
    let summary = sample_simulation_summary();
    let value = summary.to_json_value();
    let object = value
        .as_object()
        .expect("summary must render to JSON object");
    assert_eq!(
        object.get("seed").and_then(norito::json::Value::as_str),
        Some("taira-public-sim")
    );
    assert_eq!(
        object
            .get("workspace_source_manifest_sha256")
            .and_then(norito::json::Value::as_str),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        object
            .get("build_profile")
            .and_then(norito::json::Value::as_str),
        Some("release")
    );
    assert_eq!(
        object
            .get("cargo_net_offline")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );
    for name in [
        "daemon_binary_blake2b_256",
        "kagami_binary_blake2b_256",
        "test_binary_blake2b_256",
        "generated_config_blake2b_256",
    ] {
        assert_eq!(
            object
                .get(name)
                .and_then(norito::json::Value::as_str)
                .map(str::len),
            Some(64),
            "evidence digest {name}"
        );
    }
    assert_eq!(
        object
            .get("membership_churn_warning_cycles")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    for (name, expected) in [
        ("duration_secs", 60),
        ("target_tps", 5),
        ("packet_loss_percent", 10),
        ("churn_interval_secs", 300),
        ("max_height_skew", 2),
        ("max_height_skew_grace_secs", 30),
        ("max_transient_height_skew", 32),
        ("stall_timeout_secs", 300),
        ("process_downtime_secs", 5),
        ("expected_process_churn_cycles", 4),
        ("expected_membership_churn_cycles", 6),
    ] {
        assert_eq!(
            object.get(name).and_then(norito::json::Value::as_u64),
            Some(expected),
            "profile field {name}"
        );
    }
    for (name, expected) in [
        ("max_view_change_rate", 0.2),
        ("max_lagged_cycle_ratio", 0.35),
        ("min_committed_tps_ratio", 0.6),
    ] {
        assert_eq!(
            object.get(name).and_then(norito::json::Value::as_f64),
            Some(expected),
            "profile field {name}"
        );
    }
    assert_eq!(
        object
            .get("unclassified_no_progress_intervals")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        object
            .get("initial_status_snapshots")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        object
            .get("final_status_snapshots")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::ApplicationPending),
        "application_pending"
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::SuccessorActivationPending),
        "successor_activation_pending"
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::LocalControlPending),
        "local_control_pending"
    );
}
#[test]
fn write_summary_persists_local_and_durable_evidence() {
    let temp = tempfile::tempdir().expect("temporary evidence directory");
    let local = temp.path().join("local/summary.json");
    let durable = temp.path().join("durable/taira-summary.json");
    fs::create_dir_all(local.parent().expect("local parent")).expect("create local parent");
    write_summary(&local, &durable, &sample_simulation_summary())
        .expect("write both summary copies");
    let local_bytes = fs::read(&local).expect("read local summary");
    let durable_bytes = fs::read(&durable).expect("read durable summary");
    assert_eq!(local_bytes, durable_bytes);
    assert!(
        String::from_utf8(local_bytes)
            .expect("summary UTF-8")
            .contains("workspace_source_manifest_sha256")
    );
}
include!("taira_public_localnet_config_digest_test.rs");
