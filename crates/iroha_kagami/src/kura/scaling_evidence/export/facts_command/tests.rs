//! Exact public argument geometry, pre-I/O admission and actual reply writer controls.

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod retained;

use super::*;
use clap::{CommandFactory, Parser};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Parser)]
struct Parse {
    #[command(flatten)]
    args: Args,
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"facts command independent test genesis",
    )))
}

fn keys() -> Vec<PublicKey> {
    let mut keys = (0..4)
        .map(|n| {
            KeyPair::try_from_seed(vec![40 + n; 32], Algorithm::BlsNormal)
                .unwrap()
                .public_key()
                .clone()
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn accounts(discriminant: u16) -> Vec<String> {
    (0..4)
        .map(|n| {
            let key = KeyPair::try_from_seed(vec![90 + n; 32], Algorithm::Ed25519).unwrap();
            AccountId::new(key.public_key().clone())
                .to_i105_for_discriminant(discriminant)
                .unwrap()
        })
        .collect()
}

fn arguments() -> Vec<String> {
    let mut args = vec!["facts".to_owned()];
    for (flag, value) in [
        ("invocation-id", "01".repeat(32)),
        ("manifest", "/independent/manifest.json".to_owned()),
        ("manifest-sha256", "02".repeat(32)),
        ("manifest-max-bytes", "4096".to_owned()),
        ("signed-genesis", "/independent/genesis.norito".to_owned()),
        ("signed-genesis-sha256", "03".repeat(32)),
        ("signed-genesis-max-bytes", "4096".to_owned()),
        ("peer-config-max-bytes", "4096".to_owned()),
        ("context", "/independent/context.norito".to_owned()),
        ("context-sha256", "04".repeat(32)),
        ("context-max-bytes", "4096".to_owned()),
        ("journal", "/independent/journal.jsonl".to_owned()),
        ("journal-sha256", "05".repeat(32)),
        ("journal-max-bytes", "4096".to_owned()),
        ("finality", "/independent/finality.norito".to_owned()),
        ("finality-sha256", "06".repeat(32)),
        ("finality-max-bytes", "4096".to_owned()),
        ("queries", "/independent/queries.norito".to_owned()),
        ("queries-sha256", "07".repeat(32)),
        ("queries-max-bytes", "4096".to_owned()),
        ("chain-id", "facts-command-test".to_owned()),
        ("network-id", network().to_string()),
        ("chain-discriminant", "17".to_owned()),
        ("genesis-public-key", keys()[0].to_string()),
        ("lanes", "4".to_owned()),
        ("workload-seed", format!("{}1f", "00".repeat(31))),
        ("pair-index", "1".to_owned()),
        ("rate-numerator", "100".to_owned()),
        ("rate-denominator", "1".to_owned()),
        ("warmup-ns", "40000000".to_owned()),
        ("measurement-ns", "40000000".to_owned()),
        ("drain-ns", "2000000".to_owned()),
        ("submission-lag-bound-ns", "1000000".to_owned()),
        ("preparation-lookahead", "256".to_owned()),
        ("preparation-concurrency", "8".to_owned()),
        ("preparation-ahead-ns", "1000000".to_owned()),
        ("max-submissions", "256".to_owned()),
        ("max-in-flight", "4096".to_owned()),
        ("max-status-requests", "64".to_owned()),
        ("poll-interval-ns", "1000000".to_owned()),
        ("journal-max-requests", "8".to_owned()),
        ("resource-interval-ns", "2000000".to_owned()),
        ("resource-response-deadline-ns", "1000000".to_owned()),
        ("resource-max-start-lag-ns", "0".to_owned()),
        ("proof-max-bytes", "2097152".to_owned()),
        ("verification-input-max-bytes", "1048576".to_owned()),
        ("verification-output-max-bytes", "1048576".to_owned()),
        ("max-heights", "16".to_owned()),
        ("max-requests", "8".to_owned()),
        ("max-leaves-per-carrier", "8".to_owned()),
        ("first-height", "1".to_owned()),
        ("last-height", "2".to_owned()),
        ("max-committed-blocks", "16".to_owned()),
        ("max-store-data-bytes", "1048576".to_owned()),
        ("max-carrier-bytes", "65536".to_owned()),
        ("max-merge-log-bytes", "1048576".to_owned()),
        ("max-merge-frames", "16".to_owned()),
        ("reader-max-output-bytes", "524288".to_owned()),
        ("max-decode-allocation-bytes", "2097152".to_owned()),
        ("owner-uid", "42".to_owned()),
        ("block-store", "/independent/store".to_owned()),
        ("merge-log", "/independent/merge.log".to_owned()),
        (
            "facts-output",
            "/independent/output/facts.norito".to_owned(),
        ),
        ("source-max-bytes", "40960".to_owned()),
        ("facts-max-bytes", "1048576".to_owned()),
        ("total-max-bytes", "1089536".to_owned()),
        ("assembly-decode-max-bytes", "8388608".to_owned()),
        ("reply-max-bytes", "512".to_owned()),
    ] {
        args.extend([format!("--{flag}"), value]);
    }
    args.push("--peer-config".to_owned());
    args.extend((0..4).map(|n| format!("/independent/peer{n}.toml")));
    args.push("--peer-config-sha256".to_owned());
    args.extend((0..4).map(|n| format!("{:02x}", n + 8).repeat(32)));
    args.push("--validator".to_owned());
    args.extend(keys().into_iter().map(|key| key.to_string()));
    args.push("--account".to_owned());
    args.extend(accounts(17));
    args
}

fn args() -> Args {
    Parse::try_parse_from(arguments()).unwrap().args
}

#[test]
fn complete_parser_requires_every_independent_flag_and_accepts_existing_scaling_command() {
    let expected = arguments();
    assert!(Parse::try_parse_from(&expected).is_ok());
    let mut root_args = vec![
        "kagami".to_owned(),
        "advanced".to_owned(),
        "kura".to_owned(),
        "scaling-evidence".to_owned(),
        "facts".to_owned(),
    ];
    root_args.extend(expected[1..].iter().cloned());
    assert!(crate::Cli::try_parse_from(root_args).is_ok());
    for arg in Parse::command()
        .get_arguments()
        .filter(|arg| arg.is_required_set())
    {
        let flag = format!("--{}", arg.get_long().unwrap());
        let mut missing = expected.clone();
        let start = missing.iter().position(|value| value == &flag).unwrap();
        let count = if [
            "--peer-config",
            "--peer-config-sha256",
            "--validator",
            "--account",
        ]
        .contains(&flag.as_str())
        {
            5
        } else {
            2
        };
        missing.drain(start..start + count);
        assert!(
            Parse::try_parse_from(missing).is_err(),
            "accepted missing {flag}"
        );
    }
}

#[test]
fn parser_rejects_short_rosters_unsupported_lanes_and_noncanonical_identities() {
    for flag in [
        "--peer-config",
        "--peer-config-sha256",
        "--validator",
        "--account",
    ] {
        let mut wrong = arguments();
        let index = wrong.iter().position(|s| s == flag).unwrap();
        wrong.remove(index + 1);
        assert!(
            Parse::try_parse_from(wrong).is_err(),
            "accepted short {flag}"
        );
    }
    for (flag, value) in [
        ("--lanes", "2"),
        ("--workload-seed", "development-secret"),
        ("--invocation-id", "AB"),
        ("--network-id", "00"),
        ("--rate-numerator", "+1"),
        ("--rate-denominator", "01"),
        ("--reply-max-bytes", "513"),
    ] {
        let mut wrong = arguments();
        let index = wrong.iter().position(|s| s == flag).unwrap();
        wrong[index + 1] = value.to_owned();
        assert!(
            Parse::try_parse_from(wrong).is_err(),
            "accepted {flag}={value}"
        );
    }
}

#[test]
fn command_preserves_ten_independent_file_pins_in_original_order() {
    let got = args().into_inputs().unwrap();
    assert_eq!(
        got.inputs.manifest.path,
        PathBuf::from("/independent/manifest.json")
    );
    assert_eq!(got.inputs.manifest.sha256, [2; 32]);
    assert_eq!(got.inputs.signed_genesis.sha256, [3; 32]);
    for (n, peer) in got.inputs.peer_configs.iter().enumerate() {
        assert_eq!(
            peer.path,
            PathBuf::from(format!("/independent/peer{n}.toml"))
        );
        assert_eq!(peer.sha256, [8 + n as u8; 32]);
        assert_eq!(peer.max_bytes, 4096);
    }
    assert_eq!(got.inputs.context.sha256, [4; 32]);
    assert_eq!(got.inputs.journal.sha256, [5; 32]);
    assert_eq!(got.inputs.finality.sha256, [6; 32]);
    assert_eq!(got.inputs.queries.sha256, [7; 32]);
    assert_eq!(got.genesis.network_id, network());
    assert_eq!(got.genesis.validators.as_slice(), keys().as_slice());
    assert_eq!(got.genesis.chain_discriminant, 17);
    assert_eq!(got.genesis.chain_id.to_string(), "facts-command-test");
    assert_eq!(
        (
            got.caps.input_bytes,
            got.caps.facts_bytes,
            got.caps.total_bytes,
            got.caps.decode_bytes
        ),
        (40960, 1048576, 1089536, 8388608)
    );
    assert_eq!(
        (
            got.reader.first_height,
            got.reader.last_height,
            got.reader.owner_uid
        ),
        (1, 2, 42)
    );
    assert_eq!(got.invocation_id, [1; 32]);
    assert_eq!(got.reply_max_bytes, 512);
}

#[test]
fn independent_account_order_selects_universal_routes_under_scoped_discriminant() {
    let old = iroha_data_model::account::address::chain_discriminant();
    for variant in [Variant::One, Variant::Four] {
        let mut input = args();
        input.journal.lanes = variant;
        let got = input.into_inputs().unwrap();
        let lane_count = if matches!(variant, Variant::One) {
            1
        } else {
            4
        };
        for (n, account) in got.journal.accounts.iter().enumerate() {
            assert_eq!(
                account.authority.to_i105_for_discriminant(17).unwrap(),
                accounts(17)[n]
            );
            assert_eq!(
                account.route,
                RoutingDecision::new(LaneId::new((n % lane_count) as u32), DataSpaceId::UNIVERSAL)
            );
        }
    }
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        old
    );
    let mut wrong = args();
    wrong.journal.account = accounts(18);
    assert!(wrong.into_inputs().is_err());
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        old
    );
}

#[test]
fn duplicate_accounts_and_validators_fail_while_original_role_order_is_preserved() {
    let mut duplicate = args();
    duplicate.journal.account[1] = duplicate.journal.account[0].clone();
    assert!(duplicate.into_inputs().is_err());
    for count in [0, 3, 5, 65] {
        let mut wrong = args();
        wrong.journal.account.resize(count, accounts(17)[0].clone());
        assert!(wrong.into_inputs().is_err());
    }
    let mut duplicate = args();
    duplicate.genesis.validator[1] = duplicate.genesis.validator[0].clone();
    assert!(duplicate.into_inputs().is_err());
    let mut unsorted = args();
    unsorted.genesis.validator.swap(1, 2);
    let expected = unsorted.genesis.validator.clone();
    assert_eq!(
        unsorted
            .into_inputs()
            .unwrap()
            .genesis
            .validators
            .as_slice(),
        expected.as_slice()
    );
    let mut moved = args();
    moved.journal.account.swap(0, 1);
    let got = moved.into_inputs().unwrap();
    assert_eq!(
        got.journal.accounts[0]
            .authority
            .to_i105_for_discriminant(17)
            .unwrap(),
        accounts(17)[1]
    );
    assert_eq!(got.journal.accounts[0].route.lane_id, LaneId::new(0));
}

#[test]
fn source_caps_include_all_four_configs_and_reject_overflow_before_file_access() {
    let mut under = args();
    under.source_max_bytes -= 1;
    assert!(under.into_inputs().is_err());
    for index in 0..7 {
        let mut wrong = args();
        let selected = match index {
            0 => &mut wrong.originals.manifest_max_bytes,
            1 => &mut wrong.originals.signed_genesis_max_bytes,
            2 => &mut wrong.originals.peer_config_max_bytes,
            3 => &mut wrong.originals.context_max_bytes,
            4 => &mut wrong.originals.journal_max_bytes,
            5 => &mut wrong.originals.finality_max_bytes,
            _ => &mut wrong.originals.queries_max_bytes,
        };
        *selected = u64::MAX;
        assert!(wrong.into_inputs().is_err());
    }
    let mut wrong = args();
    wrong.originals.peer_config_sha256.pop();
    assert!(wrong.into_inputs().is_err());
    let mut wrong = args();
    wrong.total_max_bytes -= 1;
    assert!(wrong.into_inputs().is_err());
    let mut wrong = args();
    wrong.assembly_decode_max_bytes = MAX_BYTES * 2 + 1;
    assert!(wrong.into_inputs().is_err());
}

#[test]
fn verification_and_independent_tip_admission_reject_prefix_and_work_bound_changes() {
    for (flag, value) in [
        ("--first-height", "2"),
        ("--last-height", "0"),
        ("--last-height", "17"),
        ("--max-committed-blocks", "1"),
    ] {
        let mut input = arguments();
        let index = input.iter().position(|s| s == flag).unwrap();
        input[index + 1] = value.to_owned();
        assert!(
            Parse::try_parse_from(input)
                .unwrap()
                .args
                .into_inputs()
                .is_err()
        );
    }
    let mut wrong = args();
    wrong.verification.proof_max_bytes -= 1;
    assert!(wrong.into_inputs().is_err());
    let mut wrong = args();
    wrong.verification.max_requests = 7;
    assert!(wrong.into_inputs().is_err());
    for count in [0, MAX_REQUESTS + 1] {
        let mut wrong = args();
        wrong.verification.max_leaves_per_carrier = count;
        assert!(wrong.into_inputs().is_err());
    }
}

#[test]
fn exact_rate_parser_handles_full_u128_without_floating_point_or_alternate_spellings() {
    assert_eq!(parse_positive_u128("1").unwrap(), 1);
    assert_eq!(
        parse_positive_u128(&u128::MAX.to_string()).unwrap(),
        u128::MAX
    );
    for wrong in [
        "0",
        "00",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "1.0",
        "1e3",
        "340282366920938463463374607431768211456",
    ] {
        assert!(parse_positive_u128(wrong).is_err(), "accepted {wrong}");
    }
}

fn identity() -> PreparedTransportIdentity {
    PreparedTransportIdentity {
        raw_sha256: [0xab; 32],
        byte_length: MAX_BYTES,
    }
}

#[test]
fn facts_reply_is_exact_bounded_scalar_json_with_raw_file_identity() {
    let reply = FactsReply::new([1; 32], 512, identity()).unwrap();
    let exact = reply.len as u64;
    let mut writer = BufWriter::new(Vec::new());
    reply.write(&mut writer).unwrap();
    let written = writer.into_inner().unwrap();
    assert_eq!(written.len() as u64, exact);
    assert_eq!(written.last(), Some(&b'\n'));
    let decoded: norito::json::Value = norito::json::from_slice(&written).unwrap();
    let object = decoded.as_object().unwrap();
    assert_eq!(object.len(), 5);
    assert_eq!(object["version"].as_u64(), Some(1));
    assert_eq!(object["operation"].as_str(), Some("facts"));
    assert_eq!(
        object["invocation_id"].as_str(),
        Some("01".repeat(32).as_str())
    );
    assert_eq!(
        object["facts_sha256"].as_str(),
        Some("ab".repeat(32).as_str())
    );
    assert_eq!(object["facts_bytes"].as_u64(), Some(MAX_BYTES));
    assert!(FactsReply::new([1; 32], exact, identity()).is_ok());
    assert!(FactsReply::new([1; 32], exact - 1, identity()).is_err());
    assert!(FactsReply::new([1; 32], 0, identity()).is_err());
    assert!(FactsReply::new([1; 32], 513, identity()).is_err());
    assert!(
        FactsReply::new(
            [1; 32],
            512,
            PreparedTransportIdentity {
                raw_sha256: [0; 32],
                byte_length: 0
            }
        )
        .is_err()
    );
}

struct WriterControl {
    writes: Arc<AtomicUsize>,
    flushes: Arc<AtomicUsize>,
    fail_write: bool,
    fail_flush: bool,
}
impl Write for WriterControl {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail_write {
            Err(io::Error::other("injected actual facts write failure"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        if self.fail_flush {
            Err(io::Error::other("injected actual facts flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_reply_write_and_flush_failures_propagate_without_default_noop_flush() {
    for (fail_write, fail_flush) in [(false, false), (true, false), (false, true)] {
        let writes = Arc::new(AtomicUsize::new(0));
        let flushes = Arc::new(AtomicUsize::new(0));
        let mut writer = BufWriter::with_capacity(
            1,
            WriterControl {
                writes: Arc::clone(&writes),
                flushes: Arc::clone(&flushes),
                fail_write,
                fail_flush,
            },
        );
        let result = FactsReply::new([1; 32], 512, identity())
            .unwrap()
            .write(&mut writer);
        assert_eq!(result.is_err(), fail_write || fail_flush);
        assert!(writes.load(Ordering::SeqCst) > 0);
        assert_eq!(flushes.load(Ordering::SeqCst), usize::from(!fail_write));
    }
}

#[test]
fn invalid_programmatic_rate_fails_actual_command_before_missing_original_inputs() {
    let mut input = args();
    input.journal.rate_numerator = 0;
    let mut output = BufWriter::new(Vec::new());
    let error = input.run(&mut output).unwrap_err();
    assert!(error.to_string().contains("offered rate must be positive"));
    assert!(output.into_inner().unwrap().is_empty());
}
