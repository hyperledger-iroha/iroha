//! Real signed requests in the collector's original physical event format; no proof receipts.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::transaction::{FeePaymentIntent, TransactionBuilder};
use iroha_model_base::metadata::Metadata;

const SEED: &str = "000000000000000000000000000000000000000000000000000000000000001f";
const LOGICAL: [&str; 8] = [
    "ef59140916eea123bbe781098fd64cb19e6812af306e57d92b8718db8e24246b",
    "dcfd03f458309f428c7a6c65fa65b99caf41bf99e7d14211fa6cdae3d151c6d1",
    "c998d50dc32275ee4c02e5c040e06e540a376abb600ff9192be7a1ee0f344bbe",
    "5c0f0618fbb4b0b87c2908760b025cdc94c418b32d6d33ed0b517ed92b063e0e",
    "21d40caff6c4da16298a6d682db300ed6faee7a23a8c0ddd80f22cd6a23db6b0",
    "bc6631134f2c87e517946a970ea0814be52ab658470eb0fbd3d2cc395f14bd3d",
    "958a2ab04ced5dac21e713d66a70ad13124148507cdc263b591419a99b2f2c4d",
    "8a8348219ae572cec0c6ef1343c30a51b4e51eca9e7740a003eab3313c8de87d",
];
fn keys() -> Vec<KeyPair> {
    (0..4)
        .map(|i| KeyPair::try_from_seed(vec![80 + i; 32], Algorithm::Ed25519).unwrap())
        .collect()
}
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"journal fixture independent network",
    )))
}
fn expected(lanes: usize) -> JournalExpectations {
    JournalExpectations {
        network_id: network(),
        seed: SEED.to_owned(),
        pair_index: 1,
        variant: if lanes == 1 {
            JournalVariant::OneLane
        } else {
            JournalVariant::FourLane
        },
        accounts: keys()
            .iter()
            .enumerate()
            .map(|(i, key)| JournalAccount {
                authority: AccountId::new(key.public_key().clone()),
                route: RoutingDecision::new(
                    LaneId::new((i % lanes + 1) as u32),
                    DataSpaceId::new((i % lanes + 1) as u64),
                ),
            })
            .collect(),
        timing: JournalTiming {
            rate_numerator: 100,
            rate_denominator: 1,
            warmup_ns: 40_000_000,
            measurement_ns: 40_000_000,
            drain_ns: 2_000_000,
            submission_lag_bound_ns: 1_000_000,
        },
        bounds: JournalBounds {
            preparation_lookahead: 256,
            preparation_concurrency: 8,
            preparation_ahead_ns: 1_000_000,
            max_submissions: 256,
            max_in_flight: 4096,
            max_status_requests: 64,
            poll_interval_ns: 1_000_000,
            max_requests: 8,
        },
        sampling: JournalSampling {
            interval_ns: 2_000_000,
            response_deadline_ns: 1_000_000,
            max_start_lag_ns: 0,
        },
    }
}
fn scheduled(index: usize) -> i64 {
    let start = if index < 4 { -42_000_000 } else { 0 };
    start + (index % 4) as i64 * 10_000_000
}
fn plan(index: usize) -> Value {
    // Independent fixed vectors: rotation is two, each cohort resets at sequence one.
    norito::json!({"cohort": (if index < 4 { "warmup" } else { "measurement" }),
        "sequence": (index % 4 + 1), "logical_id": (LOGICAL[index]),
        "scheduled_offset_ns": (scheduled(index)), "account_index": ((index % 4 + 2) % 4)})
}
fn sampling() -> Value {
    norito::json!({"interval_ns": 2_000_000, "response_deadline_ns": 1_000_000,
        "max_start_lag_ns": 0, "first_offset_ns": 0, "final_offset_ns": 42_000_000, "sample_count": 22})
}
fn manifest(kind: &str, sequence: usize) -> Value {
    // This reference is deliberately not a capture-authentication fixture. The reader must not
    // open this name or assert its fake digest describes any process/resource observation.
    norito::json!({"name": (format!("{kind}-{sequence:010}.json")), "sha256": ("ab".repeat(32)), "bytes": 100})
}
fn sign(
    index: usize,
    padding: usize,
    owner: usize,
    wrong_effect: bool,
    wrong_network: bool,
) -> Vec<u8> {
    let keys = keys();
    let authority = AccountId::new(keys[owner].public_key().clone());
    let mut metadata = Metadata::default();
    metadata.insert("gscale_logical_id".parse().unwrap(), LOGICAL[index]);
    if padding > 0 {
        metadata.insert(
            "fixture_padding".parse().unwrap(),
            "a".repeat(padding).as_str(),
        );
    }
    let logical = if wrong_effect {
        LOGICAL[(index + 1) % 8]
    } else {
        LOGICAL[index]
    };
    let network = if wrong_network {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"other network")))
    } else {
        network()
    };
    let mut builder = TransactionBuilder::new(
        network,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(expected_executable(&authority, logical).unwrap())
    .with_metadata(metadata);
    builder.set_creation_time(std::time::Duration::from_millis(1));
    norito::encode_canonical(&builder.sign(keys[owner].private_key())).unwrap()
}
fn signed_rows(index: usize, bytes: &[u8]) -> Vec<Value> {
    let tx: SignedTransaction = canonical(bytes).unwrap();
    raw_signed_rows(index, bytes, &tx.hash().to_string())
}
fn raw_signed_rows(index: usize, bytes: &[u8], hash: &str) -> Vec<Value> {
    let digest = hex::encode(iroha_crypto::sha256(bytes));
    let chunks = bytes.len().div_ceil(4096);
    let mut rows = vec![
        norito::json!({"event": "signed_request_begin", "index": index, "plan": (plan(index)),
        "hash": hash, "encoding": "norito.canonical.signed_transaction.v1", "byte_length": (bytes.len()),
        "canonical_sha256": digest, "chunk_count": chunks}),
    ];
    for (chunk_index, chunk) in bytes.chunks(4096).enumerate() {
        rows.push(norito::json!({"event": "signed_request_chunk", "index": index, "chunk_index": chunk_index,
            "offset": (chunk_index * 4096), "bytes_hex": (hex::encode(chunk))}));
    }
    rows.push(
        norito::json!({"event": "signed_request_retained", "index": index, "hash": hash,
        "byte_length": (bytes.len()), "canonical_sha256": digest, "chunk_count": chunks}),
    );
    rows
}
struct Fixture {
    rows: Vec<Value>,
    signed: Vec<Vec<u8>>,
}
impl Fixture {
    fn new(lanes: usize, padding: usize) -> Self {
        let policy = expected(lanes);
        let accounts: Vec<_> = policy
            .accounts
            .iter()
            .map(|a| norito::json!({"authority": (a.authority.to_string())}))
            .collect();
        let mut rows = vec![
            norito::json!({"event": "plan", "schema": SCHEMA, "pair_index": 1,
            "variant": (policy.variant.text()), "seed": SEED, "accounts": accounts,
            "account_selection": SELECTION, "workload": WORKLOAD, "max_effects_per_account": 1024,
            "local_applied_required": true,
            "scheduled_requests": 8, "warmup_ns": 40_000_000, "measurement_ns": 40_000_000, "drain_ns": 2_000_000,
            "submission_lag_bound_ns": 1_000_000, "preparation_lookahead": 256, "preparation_concurrency": 8,
            "preparation_ahead_ns": 1_000_000, "max_submissions": 256, "max_in_flight": 4096,
            "max_status_requests": 64, "poll_interval_ns": 1_000_000}),
            norito::json!({"event": "resource_preflight", "sequence": 0, "outcome": "complete",
                "manifest": (manifest("preflight", 0)), "sampling": (sampling())}),
        ];
        for index in 0..8 {
            rows.push(norito::json!({"event": "scheduled", "index": index, "plan": (plan(index))}));
        }
        for (index, account) in policy.accounts.iter().enumerate() {
            rows.push(norito::json!({"event": "workload_account_preflight", "authority": (account.authority.to_string()),
                "account_index": index, "expected_effects": 2, "expected_account_sha256": (format!("{index:064x}")),
                "expected_account_frame_bytes": 1000}));
        }
        rows.push(norito::json!({"event": "clock_started", "initial_offset_ns": (-43_000_000)}));
        let timed_start = rows.len();
        let signed: Vec<_> = (0..8)
            .map(|i| sign(i, padding, (i % 4 + 2) % 4, false, false))
            .collect();
        for (index, bytes) in signed.iter().enumerate() {
            rows.extend(signed_rows(index, bytes));
            let hash = canonical::<SignedTransaction>(bytes)
                .unwrap()
                .hash()
                .to_string();
            let offer = scheduled(index) + 10;
            rows.push(norito::json!({"event": "prepared", "index": index, "hash": hash, "offset_ns": (offer - 10)}));
            rows.push(
                norito::json!({"event": "offer", "index": index, "hash": hash, "offset_ns": offer}),
            );
            // A poll may finish before the submission acknowledgment. Only the real state
            // Applied row is final; cache Applied and missing results still consume attempts.
            rows.push(norito::json!({"event": "status_missing", "index": index, "hash": hash, "offset_ns": (offer + 10)}));
            rows.push(norito::json!({"event": "accepted", "index": index, "hash": hash, "offset_ns": (offer + 20)}));
            rows.push(norito::json!({"event": "status", "index": index, "expected_hash": hash,
                "offset_ns": (offer + 1_000_010), "hash_matches": true, "global_scope_matches": true,
                "resolved_from": "cache", "status": "Applied", "block_height": 2}));
            rows.push(norito::json!({"event": "status", "index": index, "expected_hash": hash,
                "offset_ns": (offer + 2_000_010), "hash_matches": true, "global_scope_matches": true,
                "resolved_from": "state", "status": "Applied", "block_height": 2}));
        }
        // Mirror the independently completed local observer, preserving separate attempts.
        let local_rows = rows
            .iter()
            .filter(|row| matches!(event(row), "status" | "status_missing"))
            .cloned()
            .map(|mut row| {
                let present = event(&row) == "status";
                set(
                    &mut row,
                    "event",
                    norito::json!(if present {
                        "local_status"
                    } else {
                        "local_status_missing"
                    }),
                );
                if present {
                    let object = row.as_object_mut().unwrap();
                    let matches = object.remove("global_scope_matches").unwrap();
                    object.insert("local_scope_matches".to_owned(), matches);
                }
                row
            })
            .collect::<Vec<_>>();
        rows.extend(local_rows);
        // Merge the two actual producer event streams by emission offset below; signed
        // retention remains one contiguous command immediately before preparation completes.
        for sequence in 1..=22 {
            let start = (sequence - 1) * 2_000_000;
            rows.push(
                norito::json!({"event": "resource_request", "kind": "sample", "sequence": sequence,
                "scheduled_offset_ns": start, "start_offset_ns": start}),
            );
            rows.push(norito::json!({"event": "resource_observation", "sequence": sequence,
                "scheduled_offset_ns": start, "start_offset_ns": start, "end_offset_ns": (start + 1),
                "outcome": "complete", "manifest": (manifest("sample", sequence as usize))}));
        }
        rows[timed_start..].sort_by_key(|row| match event(row) {
            "signed_request_begin" | "signed_request_chunk" | "signed_request_retained" => {
                scheduled(row.get("index").unwrap().as_u64().unwrap() as usize) - 1
            }
            "resource_request" => row.get("start_offset_ns").unwrap().as_i64().unwrap(),
            "resource_observation" => row.get("end_offset_ns").unwrap().as_i64().unwrap(),
            _ => row.get("offset_ns").unwrap().as_i64().unwrap(),
        });
        rows.push(norito::json!({"event": "resource_request", "kind": "finish", "sequence": 23, "start_offset_ns": 42_000_002}));
        rows.push(norito::json!({"event": "resource_collection_finished", "sequence": 23, "start_offset_ns": 42_000_002,
            "end_offset_ns": 42_000_003, "sampling": (sampling())}));
        rows.push(norito::json!({"event": "workload_postconditions_started"}));
        for (index, account) in policy.accounts.iter().enumerate() {
            rows.push(norito::json!({"event": "workload_account_postcondition", "authority": (account.authority.to_string()),
                "account_index": index, "verified_effects": 2, "account_sha256": (format!("{index:064x}")),
                "read_source": "signed_find_account_by_id_after_complete_drain"}));
        }
        for (index, bytes) in signed.iter().enumerate() {
            let hash = canonical::<SignedTransaction>(bytes)
                .unwrap()
                .hash()
                .to_string();
            let offer = scheduled(index) + 10;
            rows.push(norito::json!({"event": "request_final", "plan": (plan(index)), "hash": hash,
                "offer_offset_ns": offer, "acknowledgment_offset_ns": (offer + 20), "applied_offset_ns": (offer + 2_000_010),
                "block_height": 2, "status_attempts": 3,
                "local_applied_offset_ns": (offer + 2_000_010), "local_block_height": 2, "local_status_attempts": 3,
                "submission_finished": true, "failure": null}));
        }
        rows.push(norito::json!({"event": "collection_finished", "passed": true, "failure": null}));
        Self { rows, signed }
    }
    fn raw(&self) -> Vec<u8> {
        encode_rows(&self.rows)
    }
    fn read(&self, lanes: usize) -> Result<CompleteJournal> {
        read_raw(&self.raw(), expected(lanes))
    }
}
fn encode_rows(rows: &[Value]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in rows {
        raw.extend(json::to_vec(row).unwrap());
        raw.push(b'\n');
    }
    raw
}
fn read_raw(raw: &[u8], expected: JournalExpectations) -> Result<CompleteJournal> {
    read_original_journal(
        raw,
        iroha_crypto::sha256(raw),
        u64::try_from(raw.len()).unwrap(),
        expected,
    )
}
fn event(row: &Value) -> &str {
    row.get("event").unwrap().as_str().unwrap()
}
fn at(rows: &[Value], name: &str, ordinal: usize) -> usize {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| event(row) == name)
        .nth(ordinal)
        .unwrap()
        .0
}
fn set(row: &mut Value, name: &str, value: Value) {
    *row.get_mut(name).unwrap() = value;
}
fn rejects(rows: &[Value], lanes: usize, reason: &str) {
    let error = read_raw(&encode_rows(rows), expected(lanes))
        .err()
        .expect("mutant must fail");
    assert!(
        error.to_string().contains(reason),
        "unexpected failure: {error}; wanted {reason}"
    );
}

#[test]
fn reads_complete_actual_signed_one_and_four_lane_journals() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, 9000);
        let raw = fixture.raw();
        let (scheduled_rows, observations, identity) = fixture.read(lanes).unwrap().into_parts();
        assert_eq!(scheduled_rows.len(), 8);
        assert_eq!(observations.len(), 8);
        assert_eq!(identity.raw_sha256, iroha_crypto::sha256(&raw));
        assert_eq!(identity.byte_length, raw.len() as u64);
        for (i, (request, observation)) in scheduled_rows.iter().zip(observations).enumerate() {
            assert_eq!(request.logical_id, LOGICAL[i]);
            assert_eq!(request.signed_transaction, fixture.signed[i]);
            assert_eq!(
                request.phase,
                if i < 4 {
                    WorkloadPhase::Warmup
                } else {
                    WorkloadPhase::Measurement
                }
            );
            assert_eq!(
                request.route,
                expected(lanes).accounts[(i % 4 + 2) % 4].route
            );
            assert_eq!(observation.sequence, i % 4 + 1);
            assert_eq!(observation.scheduled_offset_ns, scheduled(i));
            assert_eq!(observation.offer_offset_ns, scheduled(i) + 10);
            assert_eq!(observation.acknowledgment_offset_ns, scheduled(i) + 30);
            assert_eq!(observation.applied_offset_ns, scheduled(i) + 2_000_020);
            assert_eq!(observation.block_height, 2);
            assert_eq!(observation.status_attempts, 3);
        }
    }
}

#[test]
fn raw_digest_cap_and_json_framing_are_checked_before_decode() {
    let fixture = Fixture::new(1, 0);
    let raw = fixture.raw();
    assert!(
        read_original_journal(b"not JSON\n", [0; 32], 9, expected(1))
            .err()
            .unwrap()
            .to_string()
            .contains("digest mismatch")
    );
    for cap in [0, raw.len() as u64 - 1, MAX_PROOF_BYTES + 1] {
        assert!(read_original_journal(&raw, iroha_crypto::sha256(&raw), cap, expected(1)).is_err());
    }
    assert!(read_raw(&raw[..raw.len() - 1], expected(1)).is_err());
    for malformed in [
        b"\n".as_slice(),
        b"{}\n\n",
        b"[]\n",
        b"\xff\n",
        b"{\"event\":\"plan\",\"event\":\"plan\"}\n",
    ] {
        assert!(read_raw(malformed, expected(1)).is_err());
    }
    let oversized = vec![b' '; MAX_EVENT_BYTES + 1];
    assert!(parse_event(&oversized).is_err());
    let fixed = b"{\"event\":\"unknown\",\"padding\":\"\"}";
    let exact = format!(
        "{{\"event\":\"unknown\",\"padding\":\"{}\"}}",
        "x".repeat(MAX_EVENT_BYTES - fixed.len())
    );
    assert_eq!(exact.len(), MAX_EVENT_BYTES);
    parse_event(exact.as_bytes()).unwrap();
    assert!(parse_event(format!("{exact} ").as_bytes()).is_err());
    integer_tokens(format!("{{\"n\":{}}}", "9".repeat(128)).as_bytes()).unwrap();
    assert!(integer_tokens(format!("{{\"n\":{}}}", "9".repeat(129)).as_bytes()).is_err());
    parse_event(br#"{"event":"quoted_number","value":"1e999 \" -1.0","n":-1}"#).unwrap();
    let nested = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert!(parse_event(nested.as_bytes()).is_err());
    for token in [
        "1.0",
        "1e0",
        "18446744073709551616",
        "-9223372036854775809",
        "true",
    ] {
        let mut rows = fixture.rows.clone();
        let first = json::to_json(&rows[0]).unwrap();
        let first = first.replace("\"pair_index\":1", &format!("\"pair_index\":{token}"));
        let mut raw = first.into_bytes();
        raw.push(b'\n');
        raw.extend(encode_rows(&rows.split_off(1)));
        assert!(
            read_raw(&raw, expected(1)).is_err(),
            "numeric token {token} accepted"
        );
    }
    // A real plan-sized line reaches native duplicate-key parsing, beyond schedule admission.
    let first = json::to_json(&fixture.rows[0]).unwrap();
    let duplicate = first.replacen('{', "{\"pair_index\":1,", 1);
    let mut raw = duplicate.into_bytes();
    raw.push(b'\n');
    raw.extend(encode_rows(&fixture.rows[1..]));
    assert!(read_raw(&raw, expected(1)).is_err());
}

#[test]
fn independently_derived_rational_schedule_matches_fixed_collector_vectors() {
    for lanes in [1, 4] {
        let mut policy = expected(lanes);
        policy.timing.rate_numerator = 300;
        policy.timing.rate_denominator = 3;
        let derived = schedule::derive(&policy).unwrap();
        assert_eq!((derived.warmup, derived.total, derived.samples), (4, 8, 22));
        for (i, logical) in LOGICAL.iter().enumerate() {
            let row = derived.plan(&policy, i).unwrap();
            assert_eq!(&row.logical_id, logical);
            assert_eq!(row.scheduled_offset_ns, scheduled(i));
            assert_eq!(row.account_index, (i % 4 + 2) % 4);
            row.matches(&plan(i)).unwrap();
        }
        Fixture::new(lanes, 0).read(lanes).unwrap();
    }
    // Non-integral nanosecond periods use floor offsets and ceil counts, as the real collector.
    let mut policy = expected(4);
    policy.timing.rate_numerator = 99;
    let derived = schedule::derive(&policy).unwrap();
    assert_eq!(derived.total, 8);
    assert_eq!(
        derived.plan(&policy, 5).unwrap().scheduled_offset_ns,
        10_101_010
    );
}

#[test]
fn original_plan_cannot_authorize_seed_accounts_routes_or_bounds() {
    let fixture = Fixture::new(4, 0);
    let raw = fixture.raw();
    for change in 0..12 {
        let mut policy = expected(4);
        match change {
            0 => policy.seed = "ff".repeat(32),
            1 => policy.pair_index = 2,
            2 => policy.accounts.swap(0, 1),
            3 => policy.variant = JournalVariant::OneLane,
            4 => policy.bounds.preparation_lookahead = 255,
            5 => policy.bounds.preparation_concurrency = 7,
            6 => policy.bounds.max_in_flight = 4095,
            7 => policy.bounds.max_submissions = 255,
            8 => policy.bounds.max_status_requests = 63,
            9 => policy.bounds.preparation_ahead_ns = 2_000_000,
            10 => policy.bounds.poll_interval_ns = 2_000_000,
            _ => {
                policy.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"not retained network"),
                ))
            }
        }
        assert!(
            read_raw(&raw, policy).is_err(),
            "independent change {change} accepted"
        );
    }
    // Routes are selected by the external owner, never journal fields or signed bytes.
    let mut policy = expected(4);
    for account in &mut policy.accounts {
        account.route.dataspace_id = DataSpaceId::new(90 + account.route.lane_id.as_u32() as u64);
    }
    let (rows, _, _) = read_raw(&raw, policy).unwrap().into_parts();
    assert_eq!(rows[0].route.dataspace_id, DataSpaceId::new(93));
}

#[test]
fn rejects_zero_overflow_incomplete_rounds_and_invalid_independent_geometry() {
    for change in 0..17 {
        let mut policy = expected(4);
        match change {
            0 => policy.timing.rate_numerator = 0,
            1 => policy.timing.rate_denominator = 0,
            2 => policy.timing.rate_denominator = u128::MAX,
            3 => policy.timing.warmup_ns = -1,
            4 => policy.timing.measurement_ns = 0,
            5 => policy.timing.drain_ns = 300_000_000_001,
            6 => policy.timing.submission_lag_bound_ns = 2_500_001,
            7 => policy.timing.measurement_ns = i64::MAX,
            8 => policy.bounds.max_requests = 7,
            9 => policy.accounts.pop().map(|_| ()).unwrap(),
            10 => policy.bounds.preparation_concurrency = 33,
            11 => policy.sampling.interval_ns = 0,
            12 => policy.sampling.response_deadline_ns = 1_000_001,
            13 => policy.sampling.max_start_lag_ns = 1_000_000,
            14 => policy.seed = SEED.to_uppercase(),
            15 => policy.pair_index = 0,
            _ => policy.accounts[3].route = policy.accounts[0].route,
        }
        assert!(
            schedule::derive(&policy).is_err(),
            "independent geometry {change} accepted"
        );
    }
}

#[test]
fn every_physical_event_is_consumed_and_mandatory_rows_cannot_disappear() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, 4500);
        for index in 0..fixture.rows.len() {
            let mut rows = fixture.rows.clone();
            let removed = rows.remove(index);
            assert!(
                read_raw(&encode_rows(&rows), expected(lanes)).is_err(),
                "missing event {index} {} accepted",
                event(&removed)
            );
        }
        for name in [
            "resource_collection_failed",
            "signed_request_other",
            "unknown",
            "prepare_failed",
        ] {
            let mut rows = fixture.rows.clone();
            rows.insert(at(&rows, "prepared", 0), norito::json!({"event": name}));
            rejects(&rows, lanes, "unknown or failed");
        }
    }
}

#[test]
fn exact_fields_and_integer_types_apply_to_every_physical_event() {
    let fixture = Fixture::new(1, 0);
    let mut kinds = BTreeSet::new();
    for original in &fixture.rows {
        if !kinds.insert(event(original)) {
            continue;
        }
        let position = at(&fixture.rows, event(original), 0);
        for key in original.as_object().unwrap().keys() {
            let mut rows = fixture.rows.clone();
            rows[position].as_object_mut().unwrap().remove(key);
            assert!(
                read_raw(&encode_rows(&rows), expected(1)).is_err(),
                "missing {}.{key} accepted",
                event(original)
            );
        }
        let mut rows = fixture.rows.clone();
        rows[position]
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), Value::Null);
        rejects(&rows, 1, "fields differ");
    }
}

#[test]
fn signed_retention_forbids_every_interleaving_and_incomplete_chunk_sequence() {
    let fixture = Fixture::new(4, 9000);
    let begin = at(&fixture.rows, "signed_request_begin", 0);
    let retained = at(&fixture.rows, "signed_request_retained", 0);
    for position in begin + 1..=retained {
        for interrupt in [
            fixture.rows[1].clone(),
            fixture.rows[begin].clone(),
            fixture.rows[retained + 1].clone(),
        ] {
            let mut rows = fixture.rows.clone();
            rows.insert(position, interrupt);
            rejects(&rows, 4, "interleaves");
        }
    }
    for name in ["index", "chunk_index", "offset", "bytes_hex"] {
        let mut rows = fixture.rows.clone();
        let position = at(&rows, "signed_request_chunk", 0);
        let value = if name == "bytes_hex" {
            norito::json!("AB")
        } else {
            norito::json!(9)
        };
        set(&mut rows[position], name, value);
        assert!(read_raw(&encode_rows(&rows), expected(4)).is_err());
    }
    let mut rows = fixture.rows.clone();
    rows.swap(begin + 1, begin + 2);
    rejects(&rows, 4, "count disagrees");
}

#[test]
fn signed_bytes_raw_digest_canonical_hash_signature_and_useful_effect_all_bind() {
    let fixture = Fixture::new(1, 0);
    for name in [
        "hash",
        "canonical_sha256",
        "byte_length",
        "chunk_count",
        "encoding",
    ] {
        for event_name in ["signed_request_begin", "signed_request_retained"] {
            if name == "encoding" && event_name == "signed_request_retained" {
                continue;
            }
            let mut rows = fixture.rows.clone();
            let position = at(&rows, event_name, 0);
            let replacement = match name {
                "hash" | "canonical_sha256" => norito::json!("ff".repeat(32)),
                "encoding" => norito::json!("bare_payload"),
                _ => norito::json!(2),
            };
            set(&mut rows[position], name, replacement);
            assert!(
                read_raw(&encode_rows(&rows), expected(1)).is_err(),
                "changed {event_name}.{name} accepted"
            );
        }
    }
    // Rehash/reframe coherent impostors so the real canonical transaction admission is exercised.
    for (owner, wrong_effect, wrong_network, message) in [
        (0, false, false, "selected authority"),
        (2, true, false, "useful effect"),
        (2, false, true, "network"),
    ] {
        let mut rows = fixture.rows.clone();
        let begin = at(&rows, "signed_request_begin", 0);
        let end = at(&rows, "signed_request_retained", 0);
        rows.splice(
            begin..=end,
            signed_rows(0, &sign(0, 0, owner, wrong_effect, wrong_network)),
        );
        rejects(&rows, 1, message);
    }
    let mut rows = fixture.rows.clone();
    let chunk = at(&rows, "signed_request_chunk", 0);
    let mut encoded = text(&rows[chunk], "bytes_hex").unwrap().to_owned();
    encoded.replace_range(0..2, "00");
    set(&mut rows[chunk], "bytes_hex", norito::json!(encoded));
    rejects(&rows, 1, "raw bytes/digest mismatch");
}

#[test]
fn coherent_signed_frame_mutation_cannot_pass_with_only_a_new_raw_digest() {
    let fixture = Fixture::new(1, 0);
    let mut rows = fixture.rows.clone();
    let mut bytes = fixture.signed[0].clone();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let new_digest = hex::encode(iroha_crypto::sha256(&bytes));
    let begin = at(&rows, "signed_request_begin", 0);
    let retained = at(&rows, "signed_request_retained", 0);
    set(
        &mut rows[begin],
        "canonical_sha256",
        norito::json!(new_digest),
    );
    set(
        &mut rows[retained],
        "canonical_sha256",
        norito::json!(new_digest),
    );
    for (index, chunk) in bytes.chunks(4096).enumerate() {
        set(
            &mut rows[begin + index + 1],
            "bytes_hex",
            norito::json!(hex::encode(chunk)),
        );
    }
    assert!(read_raw(&encode_rows(&rows), expected(1)).is_err());
}

#[test]
fn canonical_framing_and_real_invalid_signatures_reject_even_with_coherent_journal_digests() {
    let fixture = Fixture::new(1, 0);
    let owner_keys = keys();
    let authority = AccountId::new(owner_keys[2].public_key().clone());
    let builder = TransactionBuilder::new(
        network(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(expected_executable(&authority, LOGICAL[0]).unwrap());
    let signature = iroha_crypto::Signature::try_new(
        owner_keys[0].private_key(),
        &builder.payload_hash_bytes(),
    )
    .unwrap();
    let invalid = builder.build_with_signature(signature);
    assert!(invalid.verify_signature().is_err());
    let invalid_bytes = norito::encode_canonical(&invalid).unwrap();
    let valid_hash = canonical::<SignedTransaction>(&fixture.signed[0])
        .unwrap()
        .hash()
        .to_string();
    let mut trailer = fixture.signed[0].clone();
    trailer.push(0);
    let mut header = fixture.signed[0].clone();
    header[0] ^= 1;
    for (bytes, hash) in [
        (invalid_bytes, invalid.hash().to_string()),
        (trailer, valid_hash.clone()),
        (header, valid_hash),
    ] {
        let expected_error = canonical::<SignedTransaction>(&bytes)
            .and_then(|tx| {
                tx.verify_signature()?;
                Ok(())
            })
            .err()
            .expect("actual canonical/signature admission must reject")
            .to_string();
        let mut rows = fixture.rows.clone();
        let begin = at(&rows, "signed_request_begin", 0);
        let retained = at(&rows, "signed_request_retained", 0);
        rows.splice(begin..=retained, raw_signed_rows(0, &bytes, &hash));
        let actual = read_raw(&encode_rows(&rows), expected(1))
            .err()
            .expect("reader must reject");
        assert_eq!(actual.to_string(), expected_error);
    }
}

#[test]
fn signed_admission_counts_cumulative_storage_before_allocating_a_request() {
    let fixture = Fixture::new(1, 0);
    let mut rows = fixture.rows.clone();
    let begin = at(&rows, "signed_request_begin", 0);
    set(
        &mut rows[begin],
        "byte_length",
        norito::json!(MAX_TRANSACTION_BYTES),
    );
    set(&mut rows[begin], "chunk_count", norito::json!(256));
    rejects(&rows, 1, "cumulative byte admission");
    let mut rows = fixture.rows.clone();
    let begin = at(&rows, "signed_request_begin", 0);
    set(
        &mut rows[begin],
        "byte_length",
        norito::json!(MAX_TRANSACTION_BYTES + 1),
    );
    rejects(&rows, 1, "integer outside bound");
}

#[test]
fn preparation_completion_can_reorder_but_measurement_cannot_precede_warmup_drain() {
    let fixture = Fixture::new(4, 0);
    let mut rows = fixture.rows.clone();
    let begin = at(&rows, "signed_request_begin", 1);
    let end = at(&rows, "prepared", 1);
    let mut moved: Vec<_> = rows.drain(begin..=end).collect();
    set(
        moved.last_mut().unwrap(),
        "offset_ns",
        norito::json!(scheduled(0) - 10),
    );
    let target = at(&rows, "signed_request_begin", 0);
    rows.splice(target..target, moved);
    // A larger independently declared lookahead permits request one to finish signing before
    // request zero. The real collector starts these preparations concurrently.
    let mut policy = expected(4);
    policy.bounds.preparation_ahead_ns = 30_000_000;
    set(
        &mut rows[0],
        "preparation_ahead_ns",
        norito::json!(30_000_000),
    );
    let clock = at(&rows, "clock_started", 0);
    set(
        &mut rows[clock],
        "initial_offset_ns",
        norito::json!(-72_000_000),
    );
    read_raw(&encode_rows(&rows), policy).unwrap();
    let mut rows = fixture.rows.clone();
    let begin = at(&rows, "signed_request_begin", 4);
    let end = at(&rows, "prepared", 4);
    let moved: Vec<_> = rows.drain(begin..=end).collect();
    let target = at(&rows, "signed_request_begin", 0);
    rows.splice(target..target, moved);
    rejects(&rows, 4, "measurement retention before complete warmup");
}

#[test]
fn prepared_offer_and_accepted_events_are_unique_and_bound_to_exact_times_and_hashes() {
    let fixture = Fixture::new(4, 0);
    for name in ["prepared", "offer", "accepted"] {
        let position = at(&fixture.rows, name, 0);
        let mut rows = fixture.rows.clone();
        rows.insert(position, rows[position].clone());
        assert!(read_raw(&encode_rows(&rows), expected(4)).is_err());
        let mut rows = fixture.rows.clone();
        set(&mut rows[position], "hash", norito::json!("ff".repeat(32)));
        rejects(&rows, 4, "text disagrees");
        let mut rows = fixture.rows.clone();
        set(&mut rows[position], "offset_ns", norito::json!(-44_000_000));
        rejects(&rows, 4, "before clock origin");
    }
    let mut rows = fixture.rows.clone();
    let offer = at(&rows, "offer", 0);
    set(
        &mut rows[offer],
        "offset_ns",
        norito::json!(scheduled(0) + 1_000_001),
    );
    rejects(&rows, 4, "independent schedule");
}

#[test]
fn cache_or_rejected_status_never_substitutes_for_exact_global_state_applied() {
    let fixture = Fixture::new(1, 0);
    let state = at(&fixture.rows, "status", 1);
    for (name, value) in [
        ("hash_matches", norito::json!(false)),
        ("global_scope_matches", norito::json!(false)),
        ("resolved_from", norito::json!("cache")),
        ("resolved_from", norito::json!("unknown")),
        ("status", norito::json!("Rejected")),
        ("status", norito::json!("Expired")),
        ("status", norito::json!("unknown")),
        ("block_height", norito::json!(0)),
        ("block_height", Value::Null),
    ] {
        let mut rows = fixture.rows.clone();
        set(&mut rows[state], name, value);
        assert!(
            read_raw(&encode_rows(&rows), expected(1)).is_err(),
            "bad state {name} accepted"
        );
    }
    let mut rows = fixture.rows.clone();
    set(
        &mut rows[state],
        "offset_ns",
        norito::json!(scheduled(0) + 1_000_021),
    );
    rejects(&rows, 1, "poll interval");
    let mut rows = fixture.rows.clone();
    rows.insert(state + 1, rows[state].clone());
    rejects(&rows, 1, "after terminal Applied");
}

#[test]
fn final_rows_join_actual_offer_ack_applied_height_and_attempt_events() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, 0);
        for ordinal in [0, 4] {
            let position = at(&fixture.rows, "request_final", ordinal);
            for name in [
                "offer_offset_ns",
                "acknowledgment_offset_ns",
                "applied_offset_ns",
                "block_height",
                "status_attempts",
            ] {
                let mut rows = fixture.rows.clone();
                let n = rows[position].get(name).unwrap().as_i64().unwrap();
                set(&mut rows[position], name, norito::json!(n + 1));
                assert!(
                    read_raw(&encode_rows(&rows), expected(lanes)).is_err(),
                    "changed final {ordinal}.{name} accepted"
                );
            }
        }
        let mut rows = fixture.rows.clone();
        let first = at(&rows, "request_final", 0);
        rows.swap(first, first + 1);
        rejects(&rows, lanes, "disagrees");
    }
}

#[test]
fn resource_reference_order_sampling_deadlines_and_workload_postconditions_are_required() {
    let fixture = Fixture::new(1, 0);
    for (name, key, value) in [
        (
            "resource_preflight",
            "outcome",
            norito::json!("unavailable"),
        ),
        ("resource_request", "sequence", norito::json!(2)),
        ("resource_request", "start_offset_ns", norito::json!(1)),
        (
            "resource_observation",
            "end_offset_ns",
            norito::json!(1_000_000),
        ),
        (
            "workload_account_preflight",
            "expected_effects",
            norito::json!(1),
        ),
        (
            "workload_account_postcondition",
            "account_sha256",
            norito::json!("ff".repeat(32)),
        ),
        (
            "workload_account_postcondition",
            "read_source",
            norito::json!("cached_account"),
        ),
        ("collection_finished", "passed", norito::json!(false)),
    ] {
        let mut rows = fixture.rows.clone();
        let index = at(&rows, name, 0);
        set(&mut rows[index], key, value);
        assert!(
            read_raw(&encode_rows(&rows), expected(1)).is_err(),
            "changed {name}.{key} accepted"
        );
    }
    let mut rows = fixture.rows.clone();
    let preflight = at(&rows, "resource_preflight", 0);
    set(
        rows[preflight].get_mut("manifest").unwrap(),
        "name",
        norito::json!("../sample.json"),
    );
    rejects(&rows, 1, "text disagrees");
    let mut rows = fixture.rows.clone();
    let post = at(&rows, "workload_postconditions_started", 0);
    let row = rows.remove(post);
    let target = at(&rows, "resource_collection_finished", 0);
    rows.insert(target, row);
    rejects(&rows, 1, "postconditions before complete");
}

#[test]
fn terminal_owner_rejects_every_prefix_duplicate_terminal_and_trailing_event() {
    let fixture = Fixture::new(1, 0);
    for length in 0..fixture.rows.len() {
        assert!(
            read_raw(&encode_rows(&fixture.rows[..length]), expected(1)).is_err(),
            "successful prefix {length}"
        );
    }
    for extra in [
        fixture.rows[0].clone(),
        fixture.rows.last().unwrap().clone(),
        norito::json!({"event": "unknown"}),
    ] {
        let mut rows = fixture.rows.clone();
        rows.push(extra);
        rejects(&rows, 1, "after collection finish");
    }
}

fn sort_witness_timed_rows(rows: &mut [Value]) {
    let start = at(rows, "clock_started", 0) + 1;
    let end = at(rows, "workload_postconditions_started", 0);
    rows[start..end].sort_by_key(|row| match event(row) {
        "signed_request_begin" | "signed_request_chunk" | "signed_request_retained" => {
            scheduled(row.get("index").unwrap().as_u64().unwrap() as usize) - 1
        }
        "resource_request" => row.get("start_offset_ns").unwrap().as_i64().unwrap(),
        "resource_observation" | "resource_collection_finished" => {
            row.get("end_offset_ns").unwrap().as_i64().unwrap()
        }
        _ => row.get("offset_ns").unwrap().as_i64().unwrap(),
    });
}

#[test]
fn rejects_reviewed_observable_submission_and_inflight_cap_breaches() {
    for lanes in [1, 4] {
        for (cap, delayed, message) in [
            (
                "max_submissions",
                "accepted",
                "journal observable submission occupancy exceeds declared bound",
            ),
            (
                "max_in_flight",
                "accepted",
                "journal observable in-flight occupancy exceeds declared bound",
            ),
            (
                "max_in_flight",
                "applied",
                "journal observable in-flight occupancy exceeds declared bound",
            ),
        ] {
            let fixture = Fixture::new(lanes, 0);
            let mut rows = fixture.rows.clone();
            let mut policy = expected(lanes);
            if cap == "max_submissions" {
                policy.bounds.max_submissions = 1;
            } else {
                policy.bounds.max_in_flight = 1;
            }
            set(&mut rows[0], cap, norito::json!(1));
            read_raw(&encode_rows(&rows), policy).unwrap();
            let (position, final_key, delay) = if delayed == "accepted" {
                (
                    at(&rows, "accepted", 0),
                    "acknowledgment_offset_ns",
                    12_000_010,
                )
            } else {
                (at(&rows, "status", 1), "applied_offset_ns", 12_000_020)
            };
            set(
                &mut rows[position],
                "offset_ns",
                norito::json!(scheduled(0) + delay),
            );
            let final_row = at(&rows, "request_final", 0);
            set(
                &mut rows[final_row],
                final_key,
                norito::json!(scheduled(0) + delay),
            );
            sort_witness_timed_rows(&mut rows);
            let first_offer = scheduled(0) + 10;
            let second_offer = scheduled(1) + 10;
            let first_end = scheduled(0) + delay;
            assert!(first_offer < second_offer && second_offer < first_end);
            let mut policy = expected(lanes);
            if cap == "max_submissions" {
                policy.bounds.max_submissions = 1;
            } else {
                policy.bounds.max_in_flight = 1;
            }
            let error = read_raw(&encode_rows(&rows), policy)
                .err()
                .expect("observable overlap must fail");
            assert_eq!(error.to_string(), message);
            // The same original signed bytes, physical event order and full joined finals fit
            // exactly two slots. No unrelated request/proof failure can satisfy this regression.
            let mut policy = expected(lanes);
            if cap == "max_submissions" {
                policy.bounds.max_submissions = 2;
            } else {
                policy.bounds.max_in_flight = 2;
            }
            set(&mut rows[0], cap, norito::json!(2));
            let (requests, finals, _) = read_raw(&encode_rows(&rows), policy).unwrap().into_parts();
            assert_eq!(requests.len(), 8);
            assert_eq!(finals.len(), 8);
            for (request, original) in requests.iter().zip(&fixture.signed) {
                assert_eq!(&request.signed_transaction, original);
            }
        }
    }
}

#[test]
fn observable_occupancy_preserves_zero_duration_and_equal_time_half_open_boundaries() {
    for lanes in [1, 4] {
        for (cap, zero_duration) in [
            ("max_submissions", true),
            ("max_submissions", false),
            ("max_in_flight", false),
        ] {
            let fixture = Fixture::new(lanes, 0);
            let mut rows = fixture.rows.clone();
            let mut policy = expected(lanes);
            if cap == "max_submissions" {
                policy.bounds.max_submissions = 1;
            } else {
                policy.bounds.max_in_flight = 1;
            }
            set(&mut rows[0], cap, norito::json!(1));
            let (position, final_key) = if cap == "max_submissions" {
                (at(&rows, "accepted", 0), "acknowledgment_offset_ns")
            } else {
                (at(&rows, "status", 1), "applied_offset_ns")
            };
            let end = scheduled(if zero_duration { 0 } else { 1 }) + 10;
            set(&mut rows[position], "offset_ns", norito::json!(end));
            let final_row = at(&rows, "request_final", 0);
            set(&mut rows[final_row], final_key, norito::json!(end));
            sort_witness_timed_rows(&mut rows);
            let (_, finals, _) = read_raw(&encode_rows(&rows), policy).unwrap().into_parts();
            assert_eq!(finals.len(), 8);
            if cap == "max_submissions" {
                assert_eq!(finals[0].acknowledgment_offset_ns, end);
            } else {
                assert_eq!(finals[0].applied_offset_ns, end);
            }
        }
    }
}

#[test]
fn independent_expectation_admission_uses_original_schedule_rules() {
    for lanes in [1, 4] {
        assert!(admit_expectations(&expected(lanes)).is_ok());
        let mut invalid = expected(lanes);
        invalid.bounds.max_requests = 7;
        assert!(admit_expectations(&invalid).is_err());
        let mut invalid = expected(lanes);
        invalid.timing.rate_denominator = 0;
        assert!(admit_expectations(&invalid).is_err());
    }
}

// Real generated account keys and network can reuse this exact physical-event fixture.
// Resource references remain deliberately unauthenticated fixture observations.
pub(in crate::kura::scaling_evidence::export) fn generated_expectations(
    network_id: NetworkId,
    accounts: &[AccountId],
    lanes: usize,
) -> JournalExpectations {
    assert_eq!(accounts.len(), 4);
    let mut policy = expected(lanes);
    policy.network_id = network_id;
    policy.accounts = accounts
        .iter()
        .enumerate()
        .map(|(index, authority)| JournalAccount {
            authority: authority.clone(),
            route: RoutingDecision::new(
                LaneId::new((index % lanes) as u32),
                DataSpaceId::UNIVERSAL,
            ),
        })
        .collect();
    policy
}
pub(in crate::kura::scaling_evidence::export) fn generated_original(
    network_id: NetworkId,
    account_keys: &[KeyPair],
    lanes: usize,
    height: u64,
) -> Vec<u8> {
    assert_eq!(account_keys.len(), 4);
    let original = Fixture::new(lanes, 0);
    let accounts = account_keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let policy = generated_expectations(network_id, &accounts, lanes);
    let signed = (0..8)
        .map(|index| {
            let owner = (index % 4 + 2) % 4;
            let mut metadata = Metadata::default();
            metadata.insert("gscale_logical_id".parse().unwrap(), LOGICAL[index]);
            let mut builder = TransactionBuilder::new(
                network_id,
                accounts[owner].clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_executable(expected_executable(&accounts[owner], LOGICAL[index]).unwrap())
            .with_metadata(metadata);
            builder.set_creation_time(std::time::Duration::from_millis(1));
            norito::encode_canonical(&builder.sign(account_keys[owner].private_key())).unwrap()
        })
        .collect::<Vec<_>>();
    let old_hashes = original
        .signed
        .iter()
        .map(|raw| {
            canonical::<SignedTransaction>(raw)
                .unwrap()
                .hash()
                .to_string()
        })
        .collect::<Vec<_>>();
    let new_hashes = signed
        .iter()
        .map(|raw| {
            canonical::<SignedTransaction>(raw)
                .unwrap()
                .hash()
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for mut row in original.rows {
        match event(&row) {
            "signed_request_begin" => {
                let index = row.get("index").unwrap().as_u64().unwrap() as usize;
                rows.extend(raw_signed_rows(index, &signed[index], &new_hashes[index]));
                continue;
            }
            "signed_request_chunk" | "signed_request_retained" => continue,
            "plan" => set(
                &mut row,
                "accounts",
                norito::json!(
                    policy
                        .accounts
                        .iter()
                        .map(|a| norito::json!({"authority": (a.authority.to_string())}))
                        .collect::<Vec<_>>()
                ),
            ),
            "workload_account_preflight" | "workload_account_postcondition" => {
                let index = row.get("account_index").unwrap().as_u64().unwrap() as usize;
                set(
                    &mut row,
                    "authority",
                    norito::json!(accounts[index].to_string()),
                );
            }
            _ => {}
        }
        for field in ["hash", "expected_hash"] {
            if let Some(old) = row.get(field).and_then(Value::as_str)
                && let Some(index) = old_hashes.iter().position(|hash| hash == old)
            {
                set(&mut row, field, norito::json!(new_hashes[index]));
            }
        }
        for key in ["block_height", "local_block_height"] {
            if row.get(key).is_some() {
                set(&mut row, key, norito::json!(height));
            }
        }
        rows.push(row);
    }
    let bytes = encode_rows(&rows);
    let (decoded, observations, _) = read_raw(&bytes, policy).unwrap().into_parts();
    assert_eq!(decoded.len(), signed.len());
    for (row, original) in decoded.iter().zip(&signed) {
        assert_eq!(&row.signed_transaction, original);
    }
    assert!(observations.iter().all(|row| row.block_height == height));
    bytes
}

#[test]
fn local_applied_requirement_and_complete_local_witness_are_mandatory() {
    let fixture = Fixture::new(4, 0);
    for requirement in [norito::json!(false), Value::Null, norito::json!("true")] {
        let mut rows = fixture.rows.clone();
        set(&mut rows[0], "local_applied_required", requirement);
        rejects(&rows, 4, "requires peer-local StateApplied");
    }
    let mut rows = fixture.rows.clone();
    rows[0]
        .as_object_mut()
        .unwrap()
        .remove("local_applied_required");
    rejects(&rows, 4, "fields differ");
    let mut rows = fixture.rows.clone();
    rows.retain(|row| !matches!(event(row), "local_status" | "local_status_missing"));
    rejects(&rows, 4, "postconditions before complete");
    for name in [
        "local_applied_offset_ns",
        "local_block_height",
        "local_status_attempts",
    ] {
        let mut rows = fixture.rows.clone();
        let final_row = at(&rows, "request_final", 0);
        rows[final_row].as_object_mut().unwrap().remove(name);
        rejects(&rows, 4, "fields differ");
    }
}

#[test]
fn local_state_requires_exact_hash_scope_positive_shared_height_and_one_terminal() {
    let fixture = Fixture::new(1, 0);
    let terminal = at(&fixture.rows, "local_status", 1);
    for (name, value) in [
        ("expected_hash", norito::json!("ff".repeat(32))),
        ("hash_matches", norito::json!(false)),
        ("local_scope_matches", norito::json!(false)),
        ("resolved_from", norito::json!("cache")),
        ("resolved_from", norito::json!("unknown")),
        ("status", norito::json!("Rejected")),
        ("status", norito::json!("Expired")),
        ("status", norito::json!("unknown")),
        ("block_height", norito::json!(0)),
        ("block_height", norito::json!(3)),
        ("block_height", Value::Null),
    ] {
        let mut rows = fixture.rows.clone();
        set(&mut rows[terminal], name, value);
        assert!(
            read_raw(&encode_rows(&rows), expected(1)).is_err(),
            "local mutation {name} accepted"
        );
    }
    let mut rows = fixture.rows.clone();
    rows.insert(terminal + 1, rows[terminal].clone());
    rejects(&rows, 1, "after terminal Applied");
    let mut rows = fixture.rows.clone();
    let object = rows[terminal].as_object_mut().unwrap();
    let value = object.remove("local_scope_matches").unwrap();
    object.insert("global_scope_matches".to_owned(), value);
    rejects(&rows, 1, "fields differ");
}

#[test]
fn pending_local_cache_queue_and_state_rows_never_substitute_for_applied() {
    let fixture = Fixture::new(4, 0);
    for (status, source) in [
        ("Queued", "state"),
        ("Approved", "state"),
        ("Committed", "state"),
        ("Queued", "cache"),
        ("Approved", "cache"),
        ("Committed", "cache"),
        ("Queued", "queue"),
        ("Approved", "queue"),
        ("Committed", "queue"),
        ("Applied", "cache"),
        ("Rejected", "cache"),
        ("Expired", "cache"),
        ("Applied", "queue"),
        ("Rejected", "queue"),
        ("Expired", "queue"),
    ] {
        let mut rows = fixture.rows.clone();
        let pending = at(&rows, "local_status", 0);
        set(&mut rows[pending], "status", norito::json!(status));
        set(&mut rows[pending], "resolved_from", norito::json!(source));
        let (_, finals, _) = read_raw(&encode_rows(&rows), expected(4))
            .unwrap()
            .into_parts();
        assert_eq!(finals.len(), 8);
        assert_eq!(
            finals[0].status_attempts, 3,
            "local attempts never change global counts"
        );
    }
}

#[test]
fn local_final_and_poll_counters_join_each_exact_observation_scope() {
    let fixture = Fixture::new(1, 0);
    for ordinal in [0, 4] {
        for name in [
            "local_applied_offset_ns",
            "local_block_height",
            "local_status_attempts",
        ] {
            let mut rows = fixture.rows.clone();
            let final_row = at(&rows, "request_final", ordinal);
            let value = rows[final_row].get(name).unwrap().as_i64().unwrap();
            set(&mut rows[final_row], name, norito::json!(value + 1));
            assert!(
                read_raw(&encode_rows(&rows), expected(1)).is_err(),
                "local final {name} accepted"
            );
        }
    }
    let mut rows = fixture.rows.clone();
    let local = at(&rows, "local_status", 1);
    set(
        &mut rows[local],
        "offset_ns",
        norito::json!(scheduled(0) + 1_000_021),
    );
    rejects(&rows, 1, "poll interval");
    let mut rows = fixture.rows.clone();
    let local = at(&rows, "local_status_missing", 0);
    rows.remove(local);
    rejects(&rows, 1, "count disagrees");
    let mut rows = fixture.rows.clone();
    let final_row = at(&rows, "request_final", 0);
    set(
        &mut rows[final_row],
        "local_status_attempts",
        norito::json!(4),
    );
    rejects(&rows, 1, "count disagrees");
}

#[test]
fn local_applied_can_precede_acknowledgment_and_global_applied() {
    let fixture = Fixture::new(4, 0);
    let mut rows = fixture.rows.clone();
    let global = at(&rows, "status", 1);
    let ack = at(&rows, "accepted", 0);
    let final_row = at(&rows, "request_final", 0);
    let global_end = scheduled(0) + 3_000_020;
    let ack_end = scheduled(0) + 4_000_020;
    set(&mut rows[global], "offset_ns", norito::json!(global_end));
    set(&mut rows[ack], "offset_ns", norito::json!(ack_end));
    set(
        &mut rows[final_row],
        "applied_offset_ns",
        norito::json!(global_end),
    );
    set(
        &mut rows[final_row],
        "acknowledgment_offset_ns",
        norito::json!(ack_end),
    );
    sort_witness_timed_rows(&mut rows);
    let (_, finals, _) = read_raw(&encode_rows(&rows), expected(4))
        .unwrap()
        .into_parts();
    assert_eq!(finals[0].applied_offset_ns, global_end);
    assert_eq!(finals[0].acknowledgment_offset_ns, ack_end);
    assert_eq!(finals[0].status_attempts, 3);
}

#[test]
fn exact_deadline_local_completion_may_follow_resource_finish_but_not_postconditions() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, 0);
        let mut rows = fixture.rows.clone();
        let local = at(&rows, "local_status", 15);
        let mut completion = rows.remove(local);
        set(&mut completion, "offset_ns", norito::json!(42_000_000));
        let postconditions = at(&rows, "workload_postconditions_started", 0);
        rows.insert(postconditions, completion.clone());
        let final_row = at(&rows, "request_final", 7);
        set(
            &mut rows[final_row],
            "local_applied_offset_ns",
            norito::json!(42_000_000),
        );
        let (_, finals, _) = read_raw(&encode_rows(&rows), expected(lanes))
            .unwrap()
            .into_parts();
        assert_eq!(finals.len(), 8);
        let postconditions = at(&rows, "workload_postconditions_started", 0);
        let local = postconditions - 1;
        let mut late = rows.clone();
        set(&mut late[local], "offset_ns", norito::json!(42_000_001));
        rejects(&late, lanes, "outside exact offer/deadline");
        let mut after_closed = rows.clone();
        after_closed.insert(postconditions + 1, completion);
        rejects(&after_closed, lanes, "outside collection");
        let mut warmup = fixture.rows.clone();
        let local = at(&warmup, "local_status", 7);
        set(&mut warmup[local], "offset_ns", norito::json!(0));
        rejects(&warmup, lanes, "outside exact offer/deadline");
    }
}

#[test]
fn observable_in_flight_occupancy_includes_slowest_local_applied() {
    for lanes in [1, 4] {
        let fixture = Fixture::new(lanes, 0);
        let mut rows = fixture.rows.clone();
        let local = at(&rows, "local_status", 1);
        let final_row = at(&rows, "request_final", 0);
        let end = scheduled(0) + 12_000_020;
        set(&mut rows[local], "offset_ns", norito::json!(end));
        set(
            &mut rows[final_row],
            "local_applied_offset_ns",
            norito::json!(end),
        );
        sort_witness_timed_rows(&mut rows);
        let mut policy = expected(lanes);
        policy.bounds.max_in_flight = 1;
        set(&mut rows[0], "max_in_flight", norito::json!(1));
        let error = read_raw(&encode_rows(&rows), policy).err().unwrap();
        assert_eq!(
            error.to_string(),
            "journal observable in-flight occupancy exceeds declared bound"
        );
        let mut policy = expected(lanes);
        policy.bounds.max_in_flight = 2;
        set(&mut rows[0], "max_in_flight", norito::json!(2));
        let (requests, finals, _) = read_raw(&encode_rows(&rows), policy).unwrap().into_parts();
        assert_eq!(finals.len(), 8);
        for (request, original) in requests.iter().zip(&fixture.signed) {
            assert_eq!(&request.signed_transaction, original);
        }
    }
}
