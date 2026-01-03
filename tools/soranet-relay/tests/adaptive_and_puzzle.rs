use std::{
    net::SocketAddr,
    num::NonZeroU32,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use iroha_crypto::soranet::{
    handshake::DEFAULT_DESCRIPTOR_COMMIT,
    pow::Parameters as PowParameters,
    puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
};
use rand::{SeedableRng, rngs::StdRng};
use soranet_relay::{
    config::{
        AdaptiveDifficultyConfig, PowConfig, PuzzleConfig, QuotaConfig, RelayMode, SlowlorisConfig,
    },
    dos::{DoSControls, ThrottleReason},
    metrics::Metrics,
};

#[test]
fn adaptive_difficulty_escalates_after_repeated_failures() {
    let metrics = Arc::new(Metrics::new());
    let mut pow_cfg = PowConfig {
        required: true,
        difficulty: 4,
        max_future_skew_secs: 120,
        min_ticket_ttl_secs: 30,
        adaptive: AdaptiveDifficultyConfig {
            enabled: true,
            min_difficulty: 4,
            max_difficulty: 10,
            pow_failure_threshold: 2,
            success_threshold: 1,
            window_secs: 1,
            increase_step: 2,
            decrease_step: 1,
        },
        quotas: QuotaConfig {
            per_remote_burst: 16,
            ..QuotaConfig::default()
        },
        ..PowConfig::default()
    };
    pow_cfg.apply_defaults().expect("pow defaults");
    assert_eq!(pow_cfg.adaptive.pow_failure_threshold, 2);

    let base = PowParameters::new(
        pow_cfg.difficulty.min(u8::MAX as u32) as u8,
        Duration::from_secs(pow_cfg.max_future_skew_secs),
        Duration::from_secs(pow_cfg.min_ticket_ttl_secs),
    );
    let controls = DoSControls::new(
        base,
        &pow_cfg,
        None,
        None,
        Arc::clone(&metrics),
        RelayMode::Entry,
    );

    let remote: SocketAddr = "192.0.2.10:7447".parse().expect("valid socket");

    // Cross the failure threshold twice with a small delay so the adaptive window closes.
    let iterations =
        usize::try_from(pow_cfg.adaptive.pow_failure_threshold + 1).expect("fits into usize");
    for _ in 0..iterations {
        let attempt = controls
            .begin(remote, None)
            .expect("attempt should be accepted");
        controls.record_pow_failure(&attempt, Duration::from_millis(5));
        thread::sleep(Duration::from_millis(10));
    }

    // Allow the adaptive window to roll over so adjustments are applied, then trigger an update.
    thread::sleep(Duration::from_secs(2));
    let attempt = controls
        .begin(remote, None)
        .expect("attempt should be accepted after cooldown");
    controls.record_pow_failure(&attempt, Duration::from_millis(5));

    let new_params = controls.current_pow_parameters();
    assert!(
        new_params.difficulty() > base.difficulty(),
        "expected difficulty to increase above {}, got {}",
        base.difficulty(),
        new_params.difficulty()
    );
}

#[test]
fn puzzle_failures_surface_and_sync_policies() {
    let metrics = Arc::new(Metrics::new());
    let puzzle_params = PuzzleParameters::new(
        NonZeroU32::new(8_192).expect("non-zero memory"),
        NonZeroU32::new(2).expect("non-zero iterations"),
        NonZeroU32::new(1).expect("non-zero lanes"),
        6,
        Duration::from_secs(120),
        Duration::from_secs(30),
    );
    let mut pow_cfg = PowConfig {
        required: true,
        difficulty: 6,
        max_future_skew_secs: 120,
        min_ticket_ttl_secs: 30,
        adaptive: AdaptiveDifficultyConfig {
            enabled: true,
            min_difficulty: 6,
            max_difficulty: 12,
            pow_failure_threshold: 3,
            success_threshold: 3,
            window_secs: 1,
            increase_step: 1,
            decrease_step: 1,
        },
        quotas: QuotaConfig {
            per_remote_burst: 8,
            per_descriptor_burst: 4,
            ..QuotaConfig::default()
        },
        puzzle: Some(PuzzleConfig {
            enabled: true,
            memory_kib: puzzle_params.memory_kib().get(),
            time_cost: puzzle_params.time_cost().get(),
            lanes: puzzle_params.lanes().get(),
        }),
        ..PowConfig::default()
    };
    pow_cfg.apply_defaults().expect("pow defaults");

    let base = PowParameters::new(
        pow_cfg.difficulty.min(u8::MAX as u32) as u8,
        Duration::from_secs(pow_cfg.max_future_skew_secs),
        Duration::from_secs(pow_cfg.min_ticket_ttl_secs),
    );
    let controls = DoSControls::new(
        base,
        &pow_cfg,
        Some(puzzle_params),
        None,
        Arc::clone(&metrics),
        RelayMode::Entry,
    );

    let remote: SocketAddr = "192.0.2.44:7555".parse().expect("valid socket");
    let descriptor = DEFAULT_DESCRIPTOR_COMMIT;
    let relay_id = [0xCC; 32];
    let mut rng = StdRng::from_seed([0xAA; 32]);

    let attempt = controls
        .begin(remote, Some(&descriptor))
        .expect("attempt should be accepted");
    let params = controls
        .current_puzzle_parameters()
        .expect("puzzle policy active");
    let ttl = Duration::from_secs(45);
    let binding = PuzzleBinding::new(&descriptor, &relay_id, None);
    let mut ticket = puzzle::mint_ticket(&params, &binding, ttl, &mut rng).expect("mint ticket");
    ticket.difficulty = ticket.difficulty.saturating_sub(1);
    let err = puzzle::verify(&ticket, &binding, &params).expect_err("tampered must fail");
    match err {
        puzzle::Error::DifficultyMismatch { .. } => {}
        other => panic!("unexpected puzzle error variant: {other:?}"),
    };
    controls.record_pow_failure(&attempt, Duration::from_millis(5));

    let pow_state = controls.current_pow_parameters();
    let puzzle_state = controls
        .current_puzzle_parameters()
        .expect("puzzle policy active");
    assert_eq!(
        pow_state.difficulty(),
        puzzle_state.difficulty(),
        "puzzle policy must track adaptive difficulty"
    );
    assert_eq!(
        pow_state.max_future_skew(),
        puzzle_state.max_future_skew(),
        "puzzle policy must share timing bounds with pow"
    );
}

#[test]
fn adaptive_difficulty_replay_is_deterministic() {
    #[derive(Clone, Copy)]
    enum Outcome {
        Success,
        PowFailure,
    }

    #[derive(Clone, Copy)]
    struct Event {
        offset: Duration,
        outcome: Outcome,
    }

    fn run_sequence(
        controls: &DoSControls,
        base: Instant,
        remote: SocketAddr,
        descriptor: &[u8],
        events: &[Event],
    ) -> (Vec<u8>, Vec<u8>) {
        let mut pow_diffs = Vec::new();
        let mut puzzle_diffs = Vec::new();
        for event in events {
            let now = base + event.offset;
            let attempt = controls
                .begin_at(remote, Some(descriptor), now)
                .expect("attempt should be accepted");
            match event.outcome {
                Outcome::Success => {
                    controls.record_success_at(&attempt, Duration::from_millis(30), now);
                }
                Outcome::PowFailure => {
                    controls.record_pow_failure_at(&attempt, Duration::from_millis(30), now);
                }
            }
            pow_diffs.push(controls.current_pow_parameters().difficulty());
            let puzzle = controls
                .current_puzzle_parameters()
                .expect("puzzle parameters available");
            puzzle_diffs.push(puzzle.difficulty());
        }
        (pow_diffs, puzzle_diffs)
    }

    let mut pow_cfg = PowConfig {
        required: true,
        difficulty: 5,
        max_future_skew_secs: 300,
        min_ticket_ttl_secs: 60,
        adaptive: AdaptiveDifficultyConfig {
            enabled: true,
            min_difficulty: 5,
            max_difficulty: 8,
            pow_failure_threshold: 2,
            success_threshold: 2,
            window_secs: 2,
            increase_step: 2,
            decrease_step: 1,
        },
        quotas: QuotaConfig {
            per_remote_burst: 16,
            ..QuotaConfig::default()
        },
        puzzle: Some(PuzzleConfig {
            enabled: true,
            memory_kib: 32 * 1024,
            time_cost: 2,
            lanes: 1,
        }),
        ..PowConfig::default()
    };
    pow_cfg.apply_defaults().expect("pow defaults");

    let base_params = PowParameters::new(
        pow_cfg.difficulty.min(u8::MAX as u32) as u8,
        Duration::from_secs(pow_cfg.max_future_skew_secs),
        Duration::from_secs(pow_cfg.min_ticket_ttl_secs),
    );
    let puzzle_params = pow_cfg
        .puzzle_parameters(&base_params)
        .expect("puzzle defaults valid")
        .expect("puzzle parameters enabled");

    let metrics_primary = Arc::new(Metrics::new());
    let metrics_replay = Arc::new(Metrics::new());
    let controls_primary = DoSControls::new(
        base_params,
        &pow_cfg,
        Some(puzzle_params),
        None,
        metrics_primary,
        RelayMode::Entry,
    );
    let controls_replay = DoSControls::new(
        base_params,
        &pow_cfg,
        Some(puzzle_params),
        None,
        metrics_replay,
        RelayMode::Entry,
    );

    let remote: SocketAddr = "192.0.2.200:7555".parse().expect("valid socket addr");
    let descriptor = DEFAULT_DESCRIPTOR_COMMIT;
    let events = [
        Event {
            offset: Duration::from_secs(0),
            outcome: Outcome::Success,
        },
        Event {
            offset: Duration::from_secs(1),
            outcome: Outcome::PowFailure,
        },
        Event {
            offset: Duration::from_secs(3),
            outcome: Outcome::PowFailure,
        },
        Event {
            offset: Duration::from_secs(5),
            outcome: Outcome::Success,
        },
        Event {
            offset: Duration::from_secs(7),
            outcome: Outcome::Success,
        },
        Event {
            offset: Duration::from_secs(9),
            outcome: Outcome::PowFailure,
        },
        Event {
            offset: Duration::from_secs(11),
            outcome: Outcome::PowFailure,
        },
    ];

    let base_primary = Instant::now();
    let (pow_primary, puzzle_primary) = run_sequence(
        &controls_primary,
        base_primary,
        remote,
        &descriptor,
        &events,
    );

    let base_replay = base_primary + Duration::from_secs(30);
    let (pow_replay, puzzle_replay) =
        run_sequence(&controls_replay, base_replay, remote, &descriptor, &events);

    assert_eq!(
        pow_primary, pow_replay,
        "pow difficulty sequence should be deterministic"
    );
    assert_eq!(
        puzzle_primary, puzzle_replay,
        "puzzle difficulty sequence should be deterministic"
    );
    for (pow, puzzle) in pow_primary.iter().zip(puzzle_primary.iter()) {
        assert_eq!(pow, puzzle, "puzzle difficulty must track pow");
    }
}

#[test]
fn volumetric_dos_soak_preserves_puzzle_and_latency_slo() {
    const SLO_MS: u64 = 300;
    let metrics = Arc::new(Metrics::new());
    let mut pow_cfg = PowConfig {
        required: true,
        difficulty: 6,
        max_future_skew_secs: 90,
        min_ticket_ttl_secs: 20,
        adaptive: AdaptiveDifficultyConfig {
            enabled: false,
            min_difficulty: 6,
            max_difficulty: 6,
            pow_failure_threshold: 4,
            success_threshold: 4,
            window_secs: 4,
            increase_step: 1,
            decrease_step: 1,
        },
        quotas: QuotaConfig {
            per_remote_burst: 6,
            per_remote_window_secs: 3,
            per_descriptor_burst: 24,
            per_descriptor_window_secs: 3,
            cooldown_secs: 4,
            max_entries: 128,
        },
        slowloris: SlowlorisConfig {
            enabled: true,
            max_handshake_millis: SLO_MS,
            timeout_threshold: 3,
            window_secs: 6,
            penalty_secs: 5,
        },
        puzzle: Some(PuzzleConfig {
            enabled: true,
            memory_kib: 4_096,
            time_cost: 1,
            lanes: 1,
        }),
        ..PowConfig::default()
    };
    pow_cfg
        .apply_defaults()
        .expect("pow configuration should be valid");

    let quotas_entry = pow_cfg.quotas_for_mode(RelayMode::Entry);
    let burst_limit = quotas_entry.per_remote_burst;
    let cooldown = Duration::from_secs(quotas_entry.cooldown_secs);
    let slowloris_threshold =
        usize::try_from(pow_cfg.slowloris.timeout_threshold).expect("threshold fits into usize");
    let slowloris_penalty = Duration::from_secs(pow_cfg.slowloris.penalty_secs);

    let base_params = PowParameters::new(
        pow_cfg.difficulty.min(u8::MAX as u32) as u8,
        Duration::from_secs(pow_cfg.max_future_skew_secs),
        Duration::from_secs(pow_cfg.min_ticket_ttl_secs),
    );
    let puzzle_params = pow_cfg
        .puzzle_parameters(&base_params)
        .expect("puzzle parameters")
        .expect("puzzle enabled");
    let controls = DoSControls::new(
        base_params,
        &pow_cfg,
        Some(puzzle_params),
        None,
        Arc::clone(&metrics),
        RelayMode::Entry,
    );
    let descriptor = DEFAULT_DESCRIPTOR_COMMIT;
    let relay_id = [0xBA; 32];
    let binding = PuzzleBinding::new(&descriptor, &relay_id, None);
    let remote1: SocketAddr = "198.51.100.10:7000".parse().expect("valid socket addr");
    let remote2: SocketAddr = "198.51.100.11:7001".parse().expect("valid socket addr");
    let mut rng = StdRng::from_seed([0x42; 32]);
    let mut now = Instant::now();

    let baseline_difficulty = controls.current_pow_parameters().difficulty();
    let mut observed_difficulties = Vec::new();
    let mut latencies = Vec::new();

    for _ in 0..burst_limit {
        let attempt = controls
            .begin_at(remote1, Some(&descriptor), now)
            .expect("attempt within burst limit");
        let params = controls
            .current_puzzle_parameters()
            .expect("puzzle policy active");
        observed_difficulties.push(params.difficulty());
        assert_eq!(
            params.difficulty(),
            controls.current_pow_parameters().difficulty(),
            "puzzle difficulty should track pow difficulty"
        );

        let ticket = puzzle::mint_ticket(
            &params,
            &binding,
            Duration::from_secs(pow_cfg.min_ticket_ttl_secs + 30),
            &mut rng,
        )
        .expect("ticket minted");
        puzzle::verify(&ticket, &binding, &params).expect("minted ticket verifies");

        let elapsed = Duration::from_millis(190);
        latencies.push(elapsed);
        controls.record_success_at(&attempt, elapsed, now);
        now += Duration::from_millis(75);
    }

    assert!(
        latencies
            .iter()
            .all(|&lat| lat <= Duration::from_millis(SLO_MS)),
        "latencies {latencies:?} must stay within the {SLO_MS} ms SLO"
    );

    let throttle = controls
        .begin_at(remote1, Some(&descriptor), now)
        .expect_err("burst limit should throttle the remote");
    assert_eq!(throttle.reason, ThrottleReason::RemoteQuota);
    assert!(
        throttle.cooldown >= cooldown,
        "expected cooldown {:?} to be at least {:?}",
        throttle.cooldown,
        cooldown
    );

    now += throttle.cooldown + Duration::from_millis(10);

    for _ in 0..slowloris_threshold {
        let attempt = controls
            .begin_at(remote2, Some(&descriptor), now)
            .expect("attempt within slowloris window");
        let params = controls
            .current_puzzle_parameters()
            .expect("puzzle policy active");
        observed_difficulties.push(params.difficulty());

        let ticket = puzzle::mint_ticket(
            &params,
            &binding,
            Duration::from_secs(pow_cfg.min_ticket_ttl_secs + 30),
            &mut rng,
        )
        .expect("ticket minted");
        puzzle::verify(&ticket, &binding, &params).expect("minted ticket verifies");

        let elapsed = Duration::from_millis(SLO_MS + 40);
        controls.record_success_at(&attempt, elapsed, now);
        now += Duration::from_millis(10);
    }

    let penalty = controls
        .begin_at(remote2, Some(&descriptor), now)
        .expect_err("slow handshake series should trigger cooldown");
    assert_eq!(penalty.reason, ThrottleReason::RemoteQuota);
    let penalty_tolerance = Duration::from_millis(50);
    let min_expected_cooldown = slowloris_penalty.saturating_sub(penalty_tolerance);
    assert!(
        penalty.cooldown >= min_expected_cooldown,
        "expected cooldown {:?} to cover slowloris penalty {:?} (allowing {:?} tolerance)",
        penalty.cooldown,
        slowloris_penalty,
        penalty_tolerance
    );

    let snapshot = metrics.snapshot();
    assert!(
        snapshot.active_remote_cooldowns > 0,
        "slowloris penalty should register active cooldowns"
    );

    for difficulty in observed_difficulties {
        assert_eq!(
            difficulty, baseline_difficulty,
            "puzzle difficulty should stay stable during soak"
        );
    }
    assert_eq!(
        controls.current_pow_parameters().difficulty(),
        baseline_difficulty,
        "pow difficulty should remain stable because adaptive mode is disabled"
    );
}
