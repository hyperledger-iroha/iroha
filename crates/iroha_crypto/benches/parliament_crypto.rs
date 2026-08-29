//! Criterion benchmarks for Parliament's adaptive threshold-BLS and timed-OVN primitives.
//!
//! Fixture construction is deterministic and happens outside the timed loops. In
//! particular, the 1,000-seat timed-OVN fixture contains genuine proof-verified
//! registrations rather than a benchmark-only shortcut.
//!
//! Run from the workspace root with
//! `cargo bench -p iroha_crypto --bench parliament_crypto --no-default-features --features bls`.
//! Append `-- parliament/threshold_bls` or `-- parliament/timed_ovn` to select
//! one family and avoid constructing the unrelated fixture.
//! Set `IROHA_PARLIAMENT_CRYPTO_ALLOCATION_EVIDENCE_V1` to a fresh output path
//! to skip Criterion sampling and record the fixed workload matrix's scoped
//! logical heap requests for the allocation-budget gate. The counter covers
//! only requests made on the measured thread; any future primitive that moves
//! work to another thread needs a separate process-level memory gate.

#![allow(unsafe_code)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    env,
    fs::OpenOptions,
    hint::black_box,
    io::{BufWriter, Write as _},
    path::Path,
    time::Duration,
};

use blstrs::{G2Affine, G2Projective, Scalar};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use group::{Curve as _, Group as _, prime::PrimeCurveAffine as _};
use iroha_crypto::{
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript,
        AdaptiveThresholdBlsSecretShare, BeaconPurpose, DasRenDealerSecret, DasRenPartialSignature,
        THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1, ThresholdBlsError, ThresholdBlsSession,
        ThresholdBlsSignature, TleReleasePurpose,
    },
    timed_ovn::{
        TIMED_OVN_CHOICE_COUNT_V1, TIMED_OVN_G2_BYTES_V1, TIMED_OVN_GT_BYTES_V1,
        TIMED_OVN_MAX_PARTICIPANTS_V1, TIMED_OVN_SCALAR_BYTES_V1, TimedOvnChoiceV1,
        TimedOvnMaskedBallotV1, TimedOvnRegistrationSecretV1, TimedOvnRegistrationV1,
        TimedOvnRosterV1, TimedOvnSessionV1, TimedOvnSurvivorRosterV1,
        aggregate_timed_ovn_ballots_v1, timed_ovn_parameter_hash_v1,
    },
    tle::{TleMasterPublicKey, TleReleaseIdentityV1},
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng as _;

const THRESHOLD_COMMITTEE_SIZES: [u16; 3] = [4, 16, 31];
const TIMED_OVN_ROSTER_SIZES: [usize; 3] = [3, 32, TIMED_OVN_MAX_PARTICIPANTS_V1];
const COMBINE_PAYLOAD_BYTES: usize = 128;
const ALLOCATION_EVIDENCE_PATH_ENV: &str = "IROHA_PARLIAMENT_CRYPTO_ALLOCATION_EVIDENCE_V1";
const ALLOCATION_EVIDENCE_SCHEMA_V1: &str = "iroha.sora_parliament.crypto_allocations.v1";

struct TrackingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_CALLS: Cell<u64> = const { Cell::new(0) };
    static REALLOCATION_CALLS: Cell<u64> = const { Cell::new(0) };
    static ALLOCATED_BYTES: Cell<u64> = const { Cell::new(0) };
    static REALLOCATED_BYTES: Cell<u64> = const { Cell::new(0) };
    static ALLOCATION_COUNTER_OVERFLOWED: Cell<bool> = const { Cell::new(false) };
}

#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn add_allocation_metric(cell: &'static std::thread::LocalKey<Cell<u64>>, value: u64) {
    cell.with(|metric| match metric.get().checked_add(value) {
        Some(total) => metric.set(total),
        None => ALLOCATION_COUNTER_OVERFLOWED.with(|overflowed| overflowed.set(true)),
    });
}

fn record_allocation(bytes: usize, reallocation: bool) {
    TRACK_ALLOCATIONS.with(|tracking| {
        if !tracking.get() {
            return;
        }
        let bytes = u64::try_from(bytes).unwrap_or_else(|_| {
            ALLOCATION_COUNTER_OVERFLOWED.with(|overflowed| overflowed.set(true));
            u64::MAX
        });
        if reallocation {
            add_allocation_metric(&REALLOCATION_CALLS, 1);
            add_allocation_metric(&REALLOCATED_BYTES, bytes);
        } else {
            add_allocation_metric(&ALLOCATION_CALLS, 1);
            add_allocation_metric(&ALLOCATED_BYTES, bytes);
        }
    });
}

// SAFETY: every allocation operation delegates to `System` with the exact
// pointer/layout contract it received. The additional thread-local counters do
// not affect allocation ownership or layout.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size(), false);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size(), false);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout are forwarded unchanged to their allocator.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the pointer, old layout, and requested size are forwarded unchanged.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            record_allocation(new_size, true);
        }
        replacement
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationMetrics {
    allocation_calls: u64,
    reallocation_calls: u64,
    allocated_bytes: u64,
    reallocated_bytes: u64,
}

struct AllocationTrackingGuard;

impl Drop for AllocationTrackingGuard {
    fn drop(&mut self) {
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
    }
}

fn allocations_during<T>(operation: impl FnOnce() -> T) -> (T, AllocationMetrics) {
    TRACK_ALLOCATIONS.with(|tracking| {
        assert!(!tracking.replace(false), "allocation measurement nested");
    });
    ALLOCATION_CALLS.with(|count| count.set(0));
    REALLOCATION_CALLS.with(|count| count.set(0));
    ALLOCATED_BYTES.with(|bytes| bytes.set(0));
    REALLOCATED_BYTES.with(|bytes| bytes.set(0));
    ALLOCATION_COUNTER_OVERFLOWED.with(|overflowed| overflowed.set(false));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let guard = AllocationTrackingGuard;
    let output = operation();
    drop(guard);
    assert!(
        !ALLOCATION_COUNTER_OVERFLOWED.with(Cell::get),
        "allocation evidence counter overflowed"
    );
    let metrics = AllocationMetrics {
        allocation_calls: ALLOCATION_CALLS.with(Cell::get),
        reallocation_calls: REALLOCATION_CALLS.with(Cell::get),
        allocated_bytes: ALLOCATED_BYTES.with(Cell::get),
        reallocated_bytes: REALLOCATED_BYTES.with(Cell::get),
    };
    (output, metrics)
}

struct AllocationEvidenceRecord {
    benchmark_id: String,
    metrics: AllocationMetrics,
}

fn measure_allocation_case<Setup, Input, Routine, Output>(
    records: &mut Vec<AllocationEvidenceRecord>,
    benchmark_id: String,
    mut setup: Setup,
    mut routine: Routine,
) where
    Setup: FnMut() -> Input,
    Routine: FnMut(Input) -> Output,
{
    assert!(
        !benchmark_id
            .bytes()
            .any(|byte| matches!(byte, b'\t' | b'\n' | b'\r')),
        "benchmark identifier must be one TSV field"
    );
    let warm_input = setup();
    let _ = black_box(routine(warm_input));
    let measured_input = setup();
    let (output, metrics) = allocations_during(|| routine(black_box(measured_input)));
    let _ = black_box(output);
    records.push(AllocationEvidenceRecord {
        benchmark_id,
        metrics,
    });
}

// These expressions intentionally mirror the fixed-width public wire contract.
// The production constants are private implementation details, so the benchmark
// pins both the derived shape and the current byte totals at fixture creation.
const EXPECTED_REGISTRATION_WIRE_BYTES_V1: usize = 8
    + 32 * 2
    + TIMED_OVN_CHOICE_COUNT_V1 * (TIMED_OVN_GT_BYTES_V1 * 2 + TIMED_OVN_SCALAR_BYTES_V1);
const EXPECTED_BALLOT_WIRE_BYTES_V1: usize = 8
    + 32 * 5
    + 2
    + TIMED_OVN_CHOICE_COUNT_V1 * TIMED_OVN_G2_BYTES_V1
    + TIMED_OVN_CHOICE_COUNT_V1 * TIMED_OVN_GT_BYTES_V1
    + TIMED_OVN_CHOICE_COUNT_V1 * TIMED_OVN_SCALAR_BYTES_V1
    + 2 * TIMED_OVN_CHOICE_COUNT_V1 * TIMED_OVN_CHOICE_COUNT_V1 * TIMED_OVN_SCALAR_BYTES_V1;

fn binding(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn indexed_binding(tag: u8, index: usize) -> [u8; 32] {
    let mut value = [tag; 32];
    value[24..].copy_from_slice(
        &u64::try_from(index)
            .expect("benchmark index fits u64")
            .to_be_bytes(),
    );
    value
}

struct ThresholdFixture {
    committee_size: u16,
    threshold: usize,
    payload: Vec<u8>,
    transcript: AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
    partials: Vec<DasRenPartialSignature<BeaconPurpose>>,
    verify_cases: Vec<(Vec<u8>, ThresholdBlsSignature<BeaconPurpose>)>,
}

fn threshold_verify_cases(
    committee_size: u16,
    signers: &[AdaptiveThresholdBlsSecretShare<BeaconPurpose>],
    threshold: usize,
    transcript: &AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
    rng: &mut ChaCha20Rng,
) -> Vec<(Vec<u8>, ThresholdBlsSignature<BeaconPurpose>)> {
    if committee_size != THRESHOLD_COMMITTEE_SIZES[2] {
        return Vec::new();
    }
    [
        0,
        COMBINE_PAYLOAD_BYTES,
        THRESHOLD_BLS_MAX_MESSAGE_PAYLOAD_BYTES_V1,
    ]
    .into_iter()
    .map(|payload_len| {
        let payload = vec![0x5A; payload_len];
        let partials = signers[..threshold]
            .iter()
            .map(|signer| {
                signer
                    .sign_payload_with_rng(transcript, &payload, rng)
                    .expect("sign verification-size case")
            })
            .collect::<Vec<_>>();
        let signature = transcript
            .combine_partial_signatures(&payload, &partials)
            .expect("combine verification-size case");
        (payload, signature)
    })
    .collect()
}

fn threshold_fixture(committee_size: u16) -> ThresholdFixture {
    let fault_tolerance = (committee_size - 1) / 3;
    let threshold = fault_tolerance + 1;
    let session = ThresholdBlsSession::<BeaconPurpose>::new(
        binding(1),
        indexed_binding(2, usize::from(committee_size)),
        binding(3),
        committee_size,
        threshold,
    )
    .expect("valid benchmark threshold session");
    let parameters =
        AdaptiveThresholdBlsParameters::derive(&session).expect("derive adaptive parameters");
    let minimum_qualified = committee_size - fault_tolerance;
    let seed = u8::try_from(committee_size).expect("supported committee size fits u8");
    let mut rng = ChaCha20Rng::from_seed([seed; 32]);
    let mut dealer_secrets = Vec::with_capacity(usize::from(minimum_qualified));
    let mut dealers = Vec::with_capacity(usize::from(minimum_qualified));
    for dealer_index in 1..=minimum_qualified {
        let (secret, dealer) =
            DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                .expect("generate deterministic adaptive dealer");
        dealer_secrets.push(secret);
        dealers.push(dealer);
    }
    let qualified_indices = (1..=minimum_qualified).collect::<Vec<_>>();
    let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
        &parameters,
        &dealers,
        &qualified_indices,
        indexed_binding(4, usize::from(committee_size)),
    )
    .expect("finalize adaptive transcript");
    let signers = (1..=committee_size)
        .map(|recipient_index| {
            let private_shares = dealer_secrets
                .iter()
                .zip(&dealers)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&parameters, dealer, recipient_index)
                        .expect("derive deterministic private contribution")
                })
                .collect::<Vec<_>>();
            AdaptiveThresholdBlsSecretShare::from_dealer_shares(&transcript, &private_shares)
                .expect("combine deterministic signer share")
        })
        .collect::<Vec<_>>();

    let payload = vec![0xA5; COMBINE_PAYLOAD_BYTES];
    let partials = signers
        .iter()
        .map(|signer| {
            signer
                .sign_payload_with_rng(&transcript, &payload, &mut rng)
                .expect("sign benchmark payload")
        })
        .collect::<Vec<_>>();
    let threshold_len = usize::from(threshold);
    let threshold_signature = transcript
        .combine_partial_signatures(&payload, &partials[..threshold_len])
        .expect("combine threshold subset");
    let full_signature = transcript
        .combine_partial_signatures(&payload, &partials)
        .expect("combine full committee");
    assert_eq!(
        threshold_signature, full_signature,
        "threshold and full subsets must reconstruct the same signature"
    );

    let verify_cases = threshold_verify_cases(
        committee_size,
        &signers,
        threshold_len,
        &transcript,
        &mut rng,
    );

    ThresholdFixture {
        committee_size,
        threshold: threshold_len,
        payload,
        transcript,
        partials,
        verify_cases,
    }
}

fn bench_threshold_combine(c: &mut Criterion, fixtures: &[ThresholdFixture]) {
    let mut combine = c.benchmark_group("parliament/threshold_bls/combine");
    combine
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for fixture in fixtures {
        combine.throughput(Throughput::Elements(
            u64::try_from(fixture.threshold).expect("threshold fits u64"),
        ));
        combine.bench_with_input(
            BenchmarkId::new("threshold", fixture.committee_size),
            fixture,
            |b, fixture| {
                b.iter(|| {
                    black_box(
                        fixture
                            .transcript
                            .combine_partial_signatures(
                                black_box(&fixture.payload),
                                black_box(&fixture.partials[..fixture.threshold]),
                            )
                            .expect("threshold subset remains valid"),
                    )
                });
            },
        );
        combine.throughput(Throughput::Elements(u64::from(fixture.committee_size)));
        combine.bench_with_input(
            BenchmarkId::new("full", fixture.committee_size),
            fixture,
            |b, fixture| {
                b.iter(|| {
                    black_box(
                        fixture
                            .transcript
                            .combine_partial_signatures(
                                black_box(&fixture.payload),
                                black_box(&fixture.partials),
                            )
                            .expect("full committee remains valid"),
                    )
                });
            },
        );
    }
    combine.finish();
}

fn bench_threshold_invalid(c: &mut Criterion, fixtures: &[ThresholdFixture]) {
    let mut invalid = c.benchmark_group("parliament/threshold_bls/invalid_fast_fail");
    invalid
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for fixture in fixtures {
        let mut reordered = fixture.partials[..fixture.threshold].to_vec();
        reordered.swap(0, 1);
        assert_eq!(
            fixture
                .transcript
                .combine_partial_signatures(&fixture.payload, &reordered),
            Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
        );
        invalid.bench_with_input(
            BenchmarkId::new("reordered", fixture.committee_size),
            &(fixture, reordered),
            |b, (fixture, reordered)| {
                b.iter(|| {
                    let result = fixture.transcript.combine_partial_signatures(
                        black_box(&fixture.payload),
                        black_box(reordered),
                    );
                    assert_eq!(
                        result,
                        Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
                    );
                    black_box(result)
                });
            },
        );
    }
    invalid.finish();
}

fn bench_threshold_verify(c: &mut Criterion, fixtures: &[ThresholdFixture]) {
    let largest = fixtures
        .last()
        .expect("threshold fixture matrix is nonempty");
    let mut verify = c.benchmark_group("parliament/threshold_bls/final_verify");
    verify
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for (payload, signature) in &largest.verify_cases {
        verify.throughput(if payload.is_empty() {
            Throughput::Elements(1)
        } else {
            Throughput::Bytes(u64::try_from(payload.len()).expect("payload length fits u64"))
        });
        verify.bench_with_input(
            BenchmarkId::from_parameter(payload.len()),
            &(payload, signature),
            |b, (payload, signature)| {
                b.iter(|| {
                    largest
                        .transcript
                        .verify_final_signature(black_box(payload), black_box(signature))
                        .expect("final signature remains valid");
                });
            },
        );
    }
    verify.finish();
}

fn bench_threshold_bls(c: &mut Criterion) {
    let fixtures = THRESHOLD_COMMITTEE_SIZES
        .into_iter()
        .map(threshold_fixture)
        .collect::<Vec<_>>();
    bench_threshold_combine(c, &fixtures);
    bench_threshold_invalid(c, &fixtures);
    bench_threshold_verify(c, &fixtures);
}

struct TimedSurvivorCase {
    participant_ids: Vec<[u8; 32]>,
    release_identity: TleReleaseIdentityV1,
}

struct TimedOvnFixture {
    roster: TimedOvnRosterV1,
    survivor_cases: Vec<TimedSurvivorCase>,
    small_survivors: TimedOvnSurvivorRosterV1,
    small_ballots: Vec<TimedOvnMaskedBallotV1>,
    registration_wire: Vec<u8>,
    ballot_wire: Vec<u8>,
}

fn cast_small_timed_ovn_ballots(
    secrets: &[TimedOvnRegistrationSecretV1],
    survivors: &TimedOvnSurvivorRosterV1,
    rng: &mut ChaCha20Rng,
) -> Vec<TimedOvnMaskedBallotV1> {
    let choices = [
        TimedOvnChoiceV1::Aye,
        TimedOvnChoiceV1::Nay,
        TimedOvnChoiceV1::Abstain,
    ];
    secrets
        .iter()
        .zip(choices)
        .map(|(secret, choice)| {
            secret
                .cast_ballot_with_rng(survivors, choice, rng)
                .expect("cast deterministic timed-OVN ballot")
        })
        .collect()
}

fn timed_ovn_fixture() -> TimedOvnFixture {
    let network_id = binding(20);
    let threshold_session =
        ThresholdBlsSession::<TleReleasePurpose>::new(network_id, binding(21), binding(22), 4, 2)
            .expect("valid TLE threshold session");
    let master_point = (G2Projective::generator() * Scalar::from(43_u64)).to_affine();
    assert_ne!(master_point, G2Affine::identity());
    let master = TleMasterPublicKey::from_bytes(
        *threshold_session.session_id(),
        &master_point.to_compressed(),
    )
    .expect("valid deterministic TLE master key");
    let governance_attempt_id = binding(23);
    let body_instance_id = binding(24);
    let ballot_attempt_id = binding(25);
    let session = TimedOvnSessionV1::new(
        network_id,
        binding(26),
        governance_attempt_id,
        body_instance_id,
        ballot_attempt_id,
        timed_ovn_parameter_hash_v1(),
        master,
    )
    .expect("valid timed-OVN session");

    let participant_ids = (0..TIMED_OVN_MAX_PARTICIPANTS_V1)
        .map(|index| indexed_binding(40, index + 1))
        .collect::<Vec<_>>();
    let mut rng = ChaCha20Rng::from_seed([0xC7; 32]);
    let mut small_secrets = Vec::with_capacity(TIMED_OVN_ROSTER_SIZES[0]);
    let mut registrations = Vec::with_capacity(TIMED_OVN_MAX_PARTICIPANTS_V1);
    for (index, participant_id) in participant_ids.iter().copied().enumerate() {
        let (secret, registration) =
            TimedOvnRegistrationSecretV1::generate_with_rng(&session, participant_id, &mut rng)
                .expect("generate deterministic timed-OVN registration");
        if index < TIMED_OVN_ROSTER_SIZES[0] {
            small_secrets.push(secret);
        }
        registrations.push(registration);
    }
    let roster = TimedOvnRosterV1::new(&session, registrations)
        .expect("freeze proof-verified timed-OVN roster");
    let registration_wire = roster.registrations()[0].to_bytes();
    assert_eq!(EXPECTED_REGISTRATION_WIRE_BYTES_V1, 3_624);
    assert_eq!(registration_wire.len(), EXPECTED_REGISTRATION_WIRE_BYTES_V1);

    let survivor_cases = TIMED_OVN_ROSTER_SIZES
        .into_iter()
        .enumerate()
        .map(|(case_index, survivor_count)| {
            let case_ids = participant_ids[..survivor_count].to_vec();
            let survivor_root = roster
                .prospective_survivor_root(&case_ids)
                .expect("derive prospective survivor root");
            let release_identity = TleReleaseIdentityV1::new(
                threshold_session,
                governance_attempt_id,
                body_instance_id,
                ballot_attempt_id,
                survivor_root,
                indexed_binding(27, case_index + 1),
                10_000,
                timed_ovn_parameter_hash_v1(),
            )
            .expect("construct timed-OVN release identity");
            TimedOvnSurvivorRosterV1::new(&roster, &case_ids, &release_identity)
                .expect("preflight survivor fixture");
            TimedSurvivorCase {
                participant_ids: case_ids,
                release_identity,
            }
        })
        .collect::<Vec<_>>();
    let small_case = &survivor_cases[0];
    let small_survivors = TimedOvnSurvivorRosterV1::new(
        &roster,
        &small_case.participant_ids,
        &small_case.release_identity,
    )
    .expect("freeze small survivor fixture");
    let small_ballots = cast_small_timed_ovn_ballots(&small_secrets, &small_survivors, &mut rng);
    let ballot_wire = small_ballots[0].to_bytes();
    assert_eq!(EXPECTED_BALLOT_WIRE_BYTES_V1, 2_858);
    assert_eq!(ballot_wire.len(), EXPECTED_BALLOT_WIRE_BYTES_V1);
    aggregate_timed_ovn_ballots_v1(&small_survivors, &small_ballots)
        .expect("preflight small ballot aggregate");

    TimedOvnFixture {
        roster,
        survivor_cases,
        small_survivors,
        small_ballots,
        registration_wire,
        ballot_wire,
    }
}

fn bench_timed_ovn(c: &mut Criterion) {
    let fixture = timed_ovn_fixture();
    bench_timed_ovn_roster_freeze(c, &fixture);
    bench_timed_ovn_survivor_freeze(c, &fixture);
    bench_timed_ovn_wire(c, &fixture);
    bench_timed_ovn_aggregate(c, &fixture);
}

fn bench_timed_ovn_roster_freeze(c: &mut Criterion, fixture: &TimedOvnFixture) {
    let mut roster = c.benchmark_group("parliament/timed_ovn/registration_roster_freeze");
    roster
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for survivor_case in &fixture.survivor_cases {
        let count = survivor_case.participant_ids.len();
        roster.throughput(Throughput::Elements(
            u64::try_from(count).expect("roster size fits u64"),
        ));
        roster.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || fixture.roster.registrations()[..count].to_vec(),
                |registrations| {
                    black_box(
                        TimedOvnRosterV1::new(fixture.roster.session(), registrations)
                            .expect("proof-verified roster remains valid"),
                    )
                },
                BatchSize::LargeInput,
            );
        });
    }
    roster.finish();
}

fn bench_timed_ovn_survivor_freeze(c: &mut Criterion, fixture: &TimedOvnFixture) {
    let mut survivors = c.benchmark_group("parliament/timed_ovn/survivor_freeze");
    survivors
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for survivor_case in &fixture.survivor_cases {
        let count = survivor_case.participant_ids.len();
        survivors.throughput(Throughput::Elements(
            u64::try_from(count).expect("survivor count fits u64"),
        ));
        survivors.bench_with_input(
            BenchmarkId::from_parameter(count),
            survivor_case,
            |b, survivor_case| {
                b.iter_batched(
                    || (),
                    |()| {
                        TimedOvnSurvivorRosterV1::new(
                            black_box(&fixture.roster),
                            black_box(&survivor_case.participant_ids),
                            &survivor_case.release_identity,
                        )
                        .expect("survivor roster remains valid")
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    survivors.finish();
}

fn bench_timed_ovn_wire(c: &mut Criterion, fixture: &TimedOvnFixture) {
    let mut wire = c.benchmark_group("parliament/timed_ovn/wire");
    wire.sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    wire.throughput(Throughput::Bytes(
        u64::try_from(fixture.registration_wire.len()).expect("wire size fits u64"),
    ));
    wire.bench_function("registration_encode_3624_bytes", |b| {
        b.iter(|| black_box(black_box(&fixture.roster.registrations()[0]).to_bytes()));
    });
    wire.bench_function("registration_decode_verify_3624_bytes", |b| {
        b.iter(|| {
            black_box(
                TimedOvnRegistrationV1::from_bytes(
                    black_box(fixture.roster.session()),
                    black_box(&fixture.registration_wire),
                )
                .expect("registration wire remains valid"),
            )
        });
    });
    wire.throughput(Throughput::Bytes(
        u64::try_from(fixture.ballot_wire.len()).expect("wire size fits u64"),
    ));
    wire.bench_function("ballot_encode_2858_bytes", |b| {
        b.iter(|| black_box(black_box(&fixture.small_ballots[0]).to_bytes()));
    });
    wire.bench_function("ballot_decode_verify_2858_bytes", |b| {
        b.iter(|| {
            black_box(
                TimedOvnMaskedBallotV1::from_bytes(
                    black_box(&fixture.small_survivors),
                    black_box(&fixture.ballot_wire),
                )
                .expect("ballot wire remains valid"),
            )
        });
    });
    wire.finish();
}

fn bench_timed_ovn_aggregate(c: &mut Criterion, fixture: &TimedOvnFixture) {
    let mut aggregate = c.benchmark_group("parliament/timed_ovn/aggregate");
    aggregate
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .throughput(Throughput::Elements(
            u64::try_from(fixture.small_ballots.len()).expect("ballot count fits u64"),
        ));
    aggregate.bench_function("proof_verified_3", |b| {
        b.iter(|| {
            black_box(
                aggregate_timed_ovn_ballots_v1(
                    black_box(&fixture.small_survivors),
                    black_box(&fixture.small_ballots),
                )
                .expect("small ballot corpus remains valid"),
            )
        });
    });
    aggregate.finish();
}

fn threshold_allocation_evidence(records: &mut Vec<AllocationEvidenceRecord>) {
    let fixtures = THRESHOLD_COMMITTEE_SIZES
        .into_iter()
        .map(threshold_fixture)
        .collect::<Vec<_>>();
    for fixture in &fixtures {
        measure_allocation_case(
            records,
            format!(
                "parliament/threshold_bls/combine/threshold/{}",
                fixture.committee_size
            ),
            || (),
            |()| {
                fixture
                    .transcript
                    .combine_partial_signatures(
                        black_box(&fixture.payload),
                        black_box(&fixture.partials[..fixture.threshold]),
                    )
                    .expect("threshold subset remains valid")
            },
        );
        measure_allocation_case(
            records,
            format!(
                "parliament/threshold_bls/combine/full/{}",
                fixture.committee_size
            ),
            || (),
            |()| {
                fixture
                    .transcript
                    .combine_partial_signatures(
                        black_box(&fixture.payload),
                        black_box(&fixture.partials),
                    )
                    .expect("full committee remains valid")
            },
        );

        let mut reordered = fixture.partials[..fixture.threshold].to_vec();
        reordered.swap(0, 1);
        measure_allocation_case(
            records,
            format!(
                "parliament/threshold_bls/invalid_fast_fail/reordered/{}",
                fixture.committee_size
            ),
            || (),
            |()| {
                let result = fixture
                    .transcript
                    .combine_partial_signatures(black_box(&fixture.payload), black_box(&reordered));
                assert_eq!(
                    result,
                    Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
                );
                result
            },
        );
    }

    let largest = fixtures
        .last()
        .expect("threshold fixture matrix is nonempty");
    for (payload, signature) in &largest.verify_cases {
        measure_allocation_case(
            records,
            format!("parliament/threshold_bls/final_verify/{}", payload.len()),
            || (),
            |()| {
                largest
                    .transcript
                    .verify_final_signature(black_box(payload), black_box(signature))
                    .expect("final signature remains valid");
            },
        );
    }
}

fn timed_ovn_allocation_evidence(records: &mut Vec<AllocationEvidenceRecord>) {
    let fixture = timed_ovn_fixture();
    for survivor_case in &fixture.survivor_cases {
        let count = survivor_case.participant_ids.len();
        measure_allocation_case(
            records,
            format!("parliament/timed_ovn/registration_roster_freeze/{count}"),
            || fixture.roster.registrations()[..count].to_vec(),
            |registrations| {
                TimedOvnRosterV1::new(fixture.roster.session(), registrations)
                    .expect("proof-verified roster remains valid")
            },
        );
        measure_allocation_case(
            records,
            format!("parliament/timed_ovn/survivor_freeze/{count}"),
            || (),
            |()| {
                TimedOvnSurvivorRosterV1::new(
                    black_box(&fixture.roster),
                    black_box(&survivor_case.participant_ids),
                    &survivor_case.release_identity,
                )
                .expect("survivor roster remains valid")
            },
        );
    }

    measure_allocation_case(
        records,
        "parliament/timed_ovn/wire/registration_encode_3624_bytes".to_owned(),
        || (),
        |()| black_box(&fixture.roster.registrations()[0]).to_bytes(),
    );
    measure_allocation_case(
        records,
        "parliament/timed_ovn/wire/registration_decode_verify_3624_bytes".to_owned(),
        || (),
        |()| {
            TimedOvnRegistrationV1::from_bytes(
                black_box(fixture.roster.session()),
                black_box(&fixture.registration_wire),
            )
            .expect("registration wire remains valid")
        },
    );
    measure_allocation_case(
        records,
        "parliament/timed_ovn/wire/ballot_encode_2858_bytes".to_owned(),
        || (),
        |()| black_box(&fixture.small_ballots[0]).to_bytes(),
    );
    measure_allocation_case(
        records,
        "parliament/timed_ovn/wire/ballot_decode_verify_2858_bytes".to_owned(),
        || (),
        |()| {
            TimedOvnMaskedBallotV1::from_bytes(
                black_box(&fixture.small_survivors),
                black_box(&fixture.ballot_wire),
            )
            .expect("ballot wire remains valid")
        },
    );
    measure_allocation_case(
        records,
        "parliament/timed_ovn/aggregate/proof_verified_3".to_owned(),
        || (),
        |()| {
            aggregate_timed_ovn_ballots_v1(
                black_box(&fixture.small_survivors),
                black_box(&fixture.small_ballots),
            )
            .expect("small ballot corpus remains valid")
        },
    );
}

fn write_allocation_evidence(path: &Path) -> std::io::Result<()> {
    let mut records = Vec::with_capacity(23);
    threshold_allocation_evidence(&mut records);
    timed_ovn_allocation_evidence(&mut records);
    assert_eq!(records.len(), 23, "allocation evidence matrix changed");

    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut output = BufWriter::new(file);
    writeln!(output, "schema\t{ALLOCATION_EVIDENCE_SCHEMA_V1}")?;
    writeln!(output, "scope\tmeasured-thread-logical-heap-requests-only")?;
    writeln!(
        output,
        "benchmark_id\tallocation_calls\treallocation_calls\tallocated_bytes\treallocated_bytes"
    )?;
    for record in records {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}",
            record.benchmark_id,
            record.metrics.allocation_calls,
            record.metrics.reallocation_calls,
            record.metrics.allocated_bytes,
            record.metrics.reallocated_bytes,
        )?;
    }
    output.flush()?;
    output.get_ref().sync_all()
}

fn main() {
    if let Some(path) = env::var_os(ALLOCATION_EVIDENCE_PATH_ENV) {
        let path = Path::new(&path);
        write_allocation_evidence(path).unwrap_or_else(|error| {
            panic!(
                "failed to write Parliament allocation evidence to {}: {error}",
                path.display()
            )
        });
        return;
    }

    // Criterion's positional filter is also used to avoid constructing the
    // unrelated heavyweight fixture when a complete family name is selected.
    let arguments = env::args().collect::<Vec<_>>();
    let threshold_selected = arguments
        .iter()
        .any(|argument| argument.contains("threshold_bls"));
    let timed_ovn_selected = arguments
        .iter()
        .any(|argument| argument.contains("timed_ovn"));
    let mut criterion = Criterion::default().configure_from_args();
    if threshold_selected || !timed_ovn_selected {
        bench_threshold_bls(&mut criterion);
    }
    if timed_ovn_selected || !threshold_selected {
        bench_timed_ovn(&mut criterion);
    }
    criterion.final_summary();
}
