//! Actual collector, SDK, immutable-source and reply-boundary controls.

use super::super::output::canonical_inputs::RawFileIdentity;
use super::*;
use clap::Parser;

fn limits() -> Limits {
    Limits {
        last_height: 2,
        max_committed_blocks: 2,
        max_store_data_bytes: 4 * MIB as u64,
        max_carrier_bytes: MIB,
        max_merge_log_bytes: 8 * MIB as u64,
        max_merge_frames: 2,
        max_input_bytes: 16 * MIB as u64,
        max_total_leaves: 16,
        max_leaves_per_carrier: 16,
        max_decode_bytes: MAX_DECODE_BYTES,
        max_value_decode_bytes: 64 * MIB,
    }
}

pub(super) fn arguments() -> Args {
    Args {
        invocation_id: "12".repeat(32),
        network_id: crate::fallback_config().network_id,
        client_config_sha256: "34".repeat(32),
        client_config_max_bytes: 1024,
        deadline_monotonic_ns: 1,
        block_store: "/var/empty/blocks".into(),
        merge_log: "/var/empty/merge.log".into(),
        context: "/var/empty/context.norito".into(),
        context_sha256: "ab".repeat(32),
        context_max_bytes: MIB as u64,
        finality_out: "/var/empty/finality.norito".into(),
        queries_out: "/var/empty/queries.norito".into(),
        finality_max_bytes: 8 * MIB as u64,
        queries_max_bytes: 8 * MIB as u64,
        total_max_bytes: 18 * MIB as u64,
        reply_max_bytes: MAX_REPLY_BYTES,
        limits: limits(),
    }
}

#[test]
fn root_command_admits_only_complete_explicit_collection_arguments() {
    let network = crate::fallback_config().network_id.to_string();
    let args = vec![
        "iroha",
        "tx",
        "collect-scaling-inputs",
        "--invocation-id",
        "1212121212121212121212121212121212121212121212121212121212121212",
        "--network-id",
        &network,
        "--client-config-sha256",
        "3434343434343434343434343434343434343434343434343434343434343434",
        "--client-config-max-bytes",
        "1000",
        "--deadline-monotonic-ns",
        "1",
        "--max-committed-blocks",
        "2",
        "--max-store-data-bytes",
        "4194304",
        "--max-carrier-bytes",
        "1048576",
        "--max-merge-log-bytes",
        "8388608",
        "--max-merge-frames",
        "2",
        "--max-input-bytes",
        "16777216",
        "--max-total-leaves",
        "16",
        "--max-leaves-per-carrier",
        "16",
        "--max-decode-bytes",
        "536870912",
        "--max-value-decode-bytes",
        "67108864",
        "--block-store",
        "/var/empty/blocks",
        "--merge-log",
        "/var/empty/merge.log",
        "--context",
        "/var/empty/context.norito",
        "--context-sha256",
        "abababababababababababababababababababababababababababababababab",
        "--context-max-bytes",
        "1000",
        "--finality-out",
        "/var/empty/finality.norito",
        "--queries-out",
        "/var/empty/queries.norito",
        "--finality-max-bytes",
        "1000",
        "--queries-max-bytes",
        "1000",
        "--total-max-bytes",
        "4000",
        "--reply-max-bytes",
        "1000",
        "--last-height",
        "2",
    ];
    let parsed = crate::Args::try_parse_from(&args).unwrap();
    let crate::Command::Tx(crate::transaction::Command::CollectScalingInputs(collection)) =
        parsed.command
    else {
        panic!("actual transaction command registration");
    };
    collection.validate().unwrap();
    for offset in (3..args.len()).step_by(2) {
        let mut missing = args.clone();
        missing.drain(offset..offset + 2);
        assert!(
            crate::Args::try_parse_from(missing).is_err(),
            "missing {}",
            args[offset]
        );
    }
}

#[test]
fn all_count_and_byte_limits_fail_before_opening_a_source_or_output() {
    let mut valid = arguments();
    // Exercise one byte below the complete reservation, including client custody.
    valid.total_max_bytes = valid.context_max_bytes
        + valid.client_config_max_bytes
        + valid.finality_max_bytes
        + valid.queries_max_bytes;
    assert_eq!(valid.validate().unwrap(), [0xab; 32]);
    let mutations: &[fn(&mut Args)] = &[
        |a| a.limits.last_height = 0,
        |a| a.limits.last_height = 3,
        |a| a.limits.max_committed_blocks = MAX_HEIGHTS + 1,
        |a| a.limits.max_store_data_bytes = 0,
        |a| a.limits.max_store_data_bytes = 2 * 1024 * 1024 * 1024 + 1,
        |a| a.limits.max_carrier_bytes = 0,
        |a| a.limits.max_carrier_bytes = 32 * MIB + 1,
        |a| a.limits.max_merge_log_bytes = MAX_FRAME_BYTES as u64 + 1,
        |a| a.limits.max_merge_frames = 3,
        |a| a.limits.max_input_bytes = 0,
        |a| a.limits.max_input_bytes = MAX_FRAME_BYTES as u64 + 1,
        |a| a.limits.max_total_leaves = 0,
        |a| a.limits.max_total_leaves = MAX_LEAVES + 1,
        |a| a.limits.max_leaves_per_carrier = 0,
        |a| a.limits.max_leaves_per_carrier = 17,
        |a| a.limits.max_decode_bytes = 0,
        |a| a.limits.max_decode_bytes = MAX_DECODE_BYTES + 1,
        |a| a.limits.max_value_decode_bytes = 0,
        |a| a.limits.max_value_decode_bytes = MAX_DECODE_BYTES + 1,
        |a| a.context_max_bytes = 0,
        |a| a.context_max_bytes = 8 * MIB as u64 + 1,
        |a| a.finality_max_bytes = 0,
        |a| a.queries_max_bytes = 0,
        |a| a.total_max_bytes -= 1,
        |a| a.total_max_bytes = MAX_FRAME_BYTES as u64 + 1,
        |a| a.reply_max_bytes = 0,
        |a| a.reply_max_bytes = MAX_REPLY_BYTES + 1,
    ];
    for mutate in mutations {
        let mut candidate = valid.clone();
        mutate(&mut candidate);
        assert!(candidate.validate().is_err(), "{candidate:?}");
    }
    for invalid in [
        "",
        "a",
        &"a".repeat(63),
        &"a".repeat(65),
        &"A".repeat(64),
        &"g".repeat(64),
        &format!(" {}", "a".repeat(63)),
    ] {
        assert!(raw_sha256(invalid).is_err());
    }
    assert_eq!(raw_sha256(&"00".repeat(32)).unwrap(), [0; 32]);
}

#[test]
fn synchronous_collector_rejects_an_entered_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let entered = runtime.enter();
    assert!(
        arguments()
            .validate()
            .unwrap_err()
            .to_string()
            .contains("Tokio")
    );
    drop(entered);
    arguments().validate().unwrap();
}

#[test]
fn actual_run_rejects_invalid_sdk_context_before_missing_source_paths() {
    use iroha_i18n::{Bundle, Language, Localizer};
    struct Context {
        config: Config,
        localizer: Localizer,
    }
    impl RunContext for Context {
        fn config(&self) -> &Config {
            &self.config
        }
        fn transaction_metadata(&self) -> Option<&iroha_model_base::metadata::Metadata> {
            None
        }
        fn input_instructions(&self) -> bool {
            false
        }
        fn output_instructions(&self) -> bool {
            false
        }
        fn i18n(&self) -> &Localizer {
            &self.localizer
        }
        fn print_data<T: norito::json::JsonSerialize + ?Sized>(
            &mut self,
            _value: &T,
        ) -> Result<()> {
            panic!("collector must retain its real reply writer");
        }
        fn println(&mut self, _value: impl std::fmt::Display) -> Result<()> {
            panic!("collector must retain its real reply writer");
        }
    }
    for bad_endpoint in [true, false] {
        let mut config = crate::fallback_config();
        if bad_endpoint {
            config.torii_api_url = "ftp://127.0.0.1/".parse().unwrap();
        } else {
            config.account_chain_discriminant = 0;
        }
        let mut context = Context {
            config,
            localizer: Localizer::new(Bundle::Cli, Language::English),
        };
        let error = arguments().run(&mut context).unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::Context(_))
            ),
            "{error:#}"
        );
    }
}

#[test]
fn structural_allocations_charge_shared_budget_before_reserving() {
    let scope = norito::DecodeLimits::new(1000, 1000, 1000, 63, 64);
    norito::with_decode_limits_scope(scope, || {
        assert!(bounded_vec::<u64>(8).is_err());
    });
    norito::with_decode_limits_scope(scope, || {
        let mut values = Vec::<u64>::new();
        assert!(reserve_leaves(&mut values, 8, 8).is_err());
        assert_eq!(values.capacity(), 0);
    });
    norito::with_decode_limits_scope(norito::DecodeLimits::new(1000, 1000, 1000, 128, 64), || {
        let mut values = bounded_vec::<u64>(4).unwrap();
        values.extend([1, 2, 3, 4]);
        reserve_leaves(&mut values, 4, 8).unwrap();
        values.extend([5, 6, 7, 8]);
        assert!(reserve_leaves(&mut values, 1, 8).is_err());
        assert_eq!(values, [1, 2, 3, 4, 5, 6, 7, 8]);
        // Both former and replacement slot allocations remain charged.
        assert!(bounded_vec::<u64>(5).is_err());
    });
}

#[test]
fn dependency_planning_reservation_rejects_overflow_and_excessive_combined_limits() {
    assert!(dependency_work_reservation(&limits()).unwrap() < MAX_DEPENDENCY_WORK_RESERVATION);
    let mut large = limits();
    large.max_value_decode_bytes = MAX_DECODE_BYTES;
    assert!(
        large
            .validate()
            .unwrap_err()
            .to_string()
            .contains("planning reservation")
    );
    large.max_leaves_per_carrier = usize::MAX;
    assert!(dependency_work_reservation(&large).is_err());
}

fn identity() -> CanonicalInputsIdentity {
    CanonicalInputsIdentity {
        context: RawFileIdentity {
            raw_sha256: [0xab; 32],
            byte_length: 11,
        },
        finality: RawFileIdentity {
            raw_sha256: [0xcd; 32],
            byte_length: 22,
        },
        queries: RawFileIdentity {
            raw_sha256: [0xef; 32],
            byte_length: 33,
        },
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    fail_write: bool,
    fail_flush: bool,
    flushed: usize,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.fail_write {
            return Err(std::io::Error::other("forced write failure"));
        }
        let count = bytes.len().min(3);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed += 1;
        if self.fail_flush {
            Err(std::io::Error::other("forced flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn reply_exact_cap_includes_newline_and_propagates_partial_write_and_flush_errors() {
    let mut writer = Writer::default();
    write_reply(
        &mut writer,
        MAX_REPLY_BYTES,
        2,
        2,
        8,
        identity(),
        &"12".repeat(32),
        [0x34; 32],
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(writer.flushed, 1);
    assert_eq!(writer.bytes.iter().filter(|b| **b == b'\n').count(), 1);
    assert_eq!(writer.bytes.last(), Some(&b'\n'));
    let value: norito::json::Value = norito::json::from_slice(&writer.bytes).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 13);
    assert_eq!(
        value.get("context_sha256").unwrap().as_str(),
        Some("abababababababababababababababababababababababababababababababab")
    );
    let exact = writer.bytes.len();
    let mut positive = Writer::default();
    write_reply(
        &mut positive,
        exact,
        2,
        2,
        8,
        identity(),
        &"12".repeat(32),
        [0x34; 32],
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(positive.bytes, writer.bytes);
    let mut short = Writer::default();
    assert!(
        write_reply(
            &mut short,
            exact - 1,
            2,
            2,
            8,
            identity(),
            &"12".repeat(32),
            [0x34; 32],
            &|| Ok(())
        )
        .is_err()
    );
    assert!(short.bytes.is_empty());
    assert_eq!(short.flushed, 0);
    let mut failed = Writer {
        fail_write: true,
        ..Writer::default()
    };
    assert!(
        write_reply(
            &mut failed,
            exact,
            2,
            2,
            8,
            identity(),
            &"12".repeat(32),
            [0x34; 32],
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(failed.flushed, 0);
    let mut failed = Writer {
        fail_flush: true,
        ..Writer::default()
    };
    assert!(
        write_reply(
            &mut failed,
            exact,
            2,
            2,
            8,
            identity(),
            &"12".repeat(32),
            [0x34; 32],
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(failed.bytes, positive.bytes);
    assert_eq!(failed.flushed, 1);
    let mut buffer = Reply {
        bytes: [0; MAX_REPLY_BYTES],
        length: 0,
        maximum: 1,
    };
    assert!(std::fmt::Write::write_str(&mut buffer, "ab").is_err());
    assert_eq!(buffer.length, 0);
}

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod retained {
    use super::*;
    use crate::transaction_load::collect_inputs::fixture::{
        Fixture, Reply as HttpReply, ScriptedTransport,
    };
    use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
    use std::{
        fs, io,
        os::unix::fs::OpenOptionsExt,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct Prepared {
        _root: tempfile::TempDir,
        args: Args,
        config: Config,
        client: Client,
        transport: Arc<ScriptedTransport>,
    }
    fn replies(fixture: &Fixture) -> Vec<HttpReply> {
        let mut replies = vec![
            HttpReply::capabilities(),
            HttpReply::finality(&fixture.first),
            HttpReply::finality(&fixture.second),
        ];
        replies.extend(fixture.queries().into_iter().map(HttpReply::details));
        replies
    }
    fn prepare(fixture: &Fixture, responses: Vec<HttpReply>) -> Prepared {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        let mut args = arguments();
        let (blocks, merge) = fixture.paths();
        args.block_store = blocks;
        args.merge_log = merge;
        args.context = path.join("original-context.norito");
        args.finality_out = path.join("finality.norito");
        args.queries_out = path.join("queries.norito");
        let bytes =
            norito::encode_canonical(&fixture.first.finality_artifact.height_context).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&args.context)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        args.context_sha256 = hex::encode(iroha_crypto::sha256(&bytes));
        let transport = ScriptedTransport::new(responses);
        let client = fixture.client(transport.clone());
        Prepared {
            _root: root,
            args,
            client,
            config: fixture.config(),
            transport,
        }
    }
    fn no_outputs(args: &Args) {
        assert!(!args.finality_out.exists());
        assert!(!args.queries_out.exists());
        assert!(
            !args
                .finality_out
                .with_file_name("finality.norito.collecting")
                .exists()
        );
        assert!(
            !args
                .queries_out
                .with_file_name("queries.norito.collecting")
                .exists()
        );
    }

    #[test]
    fn complete_one_and_four_lane_collections_preserve_exact_canonical_roots_and_rejections() {
        for lanes in [1, 4] {
            for rejected in [None, Some(5)] {
                let fixture = Fixture::new(lanes, rejected);
                let prepared = prepare(&fixture, replies(&fixture));
                let mut reply = Writer::default();
                prepared
                    .args
                    .clone()
                    .run_with_client(&prepared.config, &prepared.client, &mut reply, &|| Ok(()))
                    .unwrap();
                prepared.transport.assert_drained(11);
                let proofs: Vec<BridgeFinalityProof> =
                    norito::decode_canonical(&fs::read(&prepared.args.finality_out).unwrap())
                        .unwrap();
                let queries: Vec<CommittedTransaction> =
                    norito::decode_canonical(&fs::read(&prepared.args.queries_out).unwrap())
                        .unwrap();
                assert_eq!(proofs, [fixture.first.clone(), fixture.second.clone()]);
                assert_eq!(queries, fixture.queries());
                assert_eq!(
                    queries.iter().filter(|q| q.result.0.is_err()).count(),
                    usize::from(rejected.is_some())
                );
                assert_eq!(reply.flushed, 1);
                assert_eq!(
                    fs::read(&prepared.args.finality_out).unwrap(),
                    norito::encode_canonical(&proofs).unwrap()
                );
            }
        }
    }

    #[test]
    fn complete_collector_rejects_bad_anchor_shortened_store_and_decode_or_leaf_caps() {
        let mutations: &[fn(&mut Args)] = &[
            |a| a.context_sha256 = "00".repeat(32),
            |a| a.limits.last_height = 1,
            |a| {
                a.limits.max_decode_bytes = 64;
                a.limits.max_value_decode_bytes = 64;
            },
            |a| {
                a.limits.max_total_leaves = 4;
                a.limits.max_leaves_per_carrier = 4;
            },
            |a| a.limits.max_merge_frames = 0,
            |a| a.limits.max_input_bytes = 1,
            |a| a.limits.max_carrier_bytes = 1,
            |a| a.finality_max_bytes = 1,
            |a| a.queries_max_bytes = 1,
        ];
        for lanes in [1, 4] {
            for mutate in mutations {
                let fixture = Fixture::new(lanes, None);
                let mut prepared = prepare(&fixture, replies(&fixture));
                mutate(&mut prepared.args);
                let mut reply = Writer::default();
                assert!(
                    prepared
                        .args
                        .clone()
                        .run_with_client(&prepared.config, &prepared.client, &mut reply, &|| Ok(()))
                        .is_err()
                );
                assert!(reply.bytes.is_empty());
                no_outputs(&prepared.args);
            }
        }
    }

    #[test]
    fn sdk_failures_and_wrong_leaf_identity_never_publish_a_partial_collection() {
        for lanes in [1, 4] {
            for control in 0..9 {
                let fixture = Fixture::new(lanes, None);
                let mut responses = replies(&fixture);
                match control {
                    0 => responses[0].body = b"{}".to_vec(),
                    1 => responses[1].body = norito::encode_canonical(&fixture.second).unwrap(),
                    2 => responses[2].body = norito::encode_canonical(&fixture.first).unwrap(),
                    3 => responses[2].body.pop().map(|_| ()).unwrap(),
                    4 => {
                        let first = responses[3].body.clone();
                        responses[3].body = responses[4].body.clone();
                        responses[4].body = first;
                    }
                    5 => {
                        let mut query = fixture.queries().remove(0);
                        query.block_hash = fixture.genesis.hash();
                        responses[3] = HttpReply::details(query);
                    }
                    6 => {
                        responses.pop();
                    }
                    7 => {
                        let leaves = fixture.queries();
                        let mut query = leaves[0].clone();
                        query.entrypoint_proof = leaves[1].entrypoint_proof.clone();
                        query.result_proof = leaves[1].result_proof.clone();
                        responses[3] = HttpReply::details(query);
                    }
                    8 => {
                        let mut query = fixture.queries().remove(0);
                        query.merge_inclusion = None;
                        responses[3] = HttpReply::details(query);
                    }
                    _ => unreachable!(),
                }
                let prepared = prepare(&fixture, responses);
                let mut reply = Writer::default();
                assert!(
                    prepared
                        .args
                        .clone()
                        .run_with_client(&prepared.config, &prepared.client, &mut reply, &|| Ok(()))
                        .is_err()
                );
                assert!(reply.bytes.is_empty());
                no_outputs(&prepared.args);
            }
        }
    }

    #[test]
    fn independent_context_network_and_height_are_checked_before_sdk_or_output_admission() {
        for control in 0..3 {
            let fixture = Fixture::new(4, None);
            let mut prepared = prepare(&fixture, replies(&fixture));
            let mut anchor = fixture.first.finality_artifact.height_context.clone();
            match control {
                0 => anchor.network_id = NetworkId::from_genesis_hash(fixture.carrier.hash()),
                1 => anchor = fixture.second.finality_artifact.height_context.clone(),
                2 => {
                    anchor.parent_commit_qc =
                        Some(fixture.first.finality_artifact.commit_qc.clone())
                }
                _ => unreachable!(),
            }
            let bytes = norito::encode_canonical(&anchor).unwrap();
            fs::write(&prepared.args.context, &bytes).unwrap();
            prepared.args.context_sha256 = hex::encode(iroha_crypto::sha256(&bytes));
            assert!(
                prepared
                    .args
                    .clone()
                    .run_with_client(
                        &prepared.config,
                        &prepared.client,
                        &mut Writer::default(),
                        &|| Ok(())
                    )
                    .is_err()
            );
            assert_eq!(prepared.transport.consumed(), 0);
            no_outputs(&prepared.args);
        }
    }

    #[test]
    fn reordered_or_unaligned_full_merge_transcripts_fail_before_any_leaf_query() {
        for control in 0..2 {
            let mut fixture = Fixture::new(4, None);
            fixture
                .rewrite_entry_for_test(|entry| {
                    let batch = entry.execution_batch.as_mut().unwrap();
                    if control == 0 {
                        batch.lanes.reverse();
                    } else {
                        batch.lanes[0].entrypoint_hashes.pop();
                    }
                })
                .unwrap();
            let prepared = prepare(
                &fixture,
                vec![
                    HttpReply::capabilities(),
                    HttpReply::finality(&fixture.first),
                    HttpReply::finality(&fixture.second),
                ],
            );
            assert!(
                prepared
                    .args
                    .clone()
                    .run_with_client(
                        &prepared.config,
                        &prepared.client,
                        &mut Writer::default(),
                        &|| Ok(())
                    )
                    .is_err()
            );
            prepared.transport.assert_drained(3);
            no_outputs(&prepared.args);
        }
    }

    #[test]
    fn valid_finality_for_another_executed_wire_cannot_attest_this_kura_carrier() {
        let fixture = Fixture::new(4, None);
        let wrong = fixture.second_with_wrong_executed_wire();
        let mut verifier = BridgeFinalityVerifier::with_context(
            fixture.network_id,
            fixture.first.finality_artifact.context_id(),
        );
        verifier.verify(&fixture.first).unwrap();
        verifier.verify(&wrong).unwrap();
        assert_eq!(wrong.block_header, fixture.second.block_header);
        let mut responses = replies(&fixture);
        responses[2] = HttpReply::finality(&wrong);
        let prepared = prepare(&fixture, responses);
        let error = prepared
            .args
            .clone()
            .run_with_client(
                &prepared.config,
                &prepared.client,
                &mut Writer::default(),
                &|| Ok(()),
            )
            .unwrap_err();
        assert!(error.to_string().contains("exact executed Kura wire"));
        no_outputs(&prepared.args);
    }

    #[test]
    fn typed_output_exact_caps_pass_and_one_byte_less_fails_before_staging() {
        let fixture = Fixture::new(4, None);
        let sizes = [
            norito::canonical_frame_len(&vec![fixture.first.clone(), fixture.second.clone()])
                .unwrap() as u64,
            norito::canonical_frame_len(&fixture.queries()).unwrap() as u64,
        ];
        for smaller in [None, Some(0), Some(1), Some(2)] {
            let mut prepared = prepare(&fixture, replies(&fixture));
            prepared.args.context_max_bytes = fs::metadata(&prepared.args.context).unwrap().len();
            prepared.args.finality_max_bytes = sizes[0] - u64::from(smaller == Some(0));
            prepared.args.queries_max_bytes = sizes[1] - u64::from(smaller == Some(1));
            prepared.args.total_max_bytes = prepared.args.context_max_bytes
                + prepared.args.client_config_max_bytes
                + prepared.args.finality_max_bytes
                + prepared.args.queries_max_bytes
                - u64::from(smaller == Some(2));
            let result = prepared.args.clone().run_with_client(
                &prepared.config,
                &prepared.client,
                &mut Writer::default(),
                &|| Ok(()),
            );
            assert_eq!(
                result.is_ok(),
                smaller.is_none(),
                "cap case {smaller:?}: {:?}",
                result.as_ref().err().map(|error| format!("{error:#}"))
            );
            if smaller == Some(2) {
                assert!(
                    result
                        .as_ref()
                        .unwrap_err()
                        .to_string()
                        .contains("transport reservations exceed aggregate cap")
                );
            }
            if smaller.is_some() {
                no_outputs(&prepared.args);
            }
        }
    }

    #[derive(Debug)]
    struct MutatingTransport {
        inner: Arc<ScriptedTransport>,
        at: usize,
        calls: AtomicUsize,
        replacement: Mutex<Option<PathBuf>>,
    }

    #[test]
    fn duplicate_committed_entrypoint_is_rejected_after_complete_transcript_collection() {
        let mut fixture = Fixture::new(1, None);
        fixture
            .rewrite_entry_for_test(|entry| {
                let lane = &mut entry.execution_batch.as_mut().unwrap().lanes[0];
                lane.entrypoints[1] = lane.entrypoints[0].clone();
                lane.entrypoint_hashes[1] = Hash::from(lane.entrypoints[1].hash());
            })
            .unwrap();
        let prepared = prepare(&fixture, replies(&fixture));
        let error = prepared
            .args
            .clone()
            .run_with_client(
                &prepared.config,
                &prepared.client,
                &mut Writer::default(),
                &|| Ok(()),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("duplicate entrypoint"),
            "{error:#}"
        );
        prepared.transport.assert_drained(11);
        no_outputs(&prepared.args);
    }
    impl HttpTransport for MutatingTransport {
        fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
            let response = self.inner.send_blocking(request)?;
            if self.calls.fetch_add(1, Ordering::SeqCst) + 1 == self.at {
                let path = self.replacement.lock().unwrap().take().unwrap();
                let bytes = fs::read(&path)?;
                let replacement = path.with_extension("replacement");
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&replacement)?
                    .write_all(&bytes)?;
                fs::rename(replacement, path)?;
            }
            Ok(response)
        }
        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move { self.send_blocking(request) })
        }
    }

    #[test]
    fn original_anchor_and_kura_replacement_during_sdk_queries_fail_closed() {
        for replace_anchor in [true, false] {
            let fixture = Fixture::new(4, None);
            let prepared = prepare(&fixture, replies(&fixture));
            let source = if replace_anchor {
                prepared.args.context.clone()
            } else {
                prepared.args.block_store.join("blocks.data")
            };
            let client = fixture.client(Arc::new(MutatingTransport {
                inner: prepared.transport.clone(),
                at: 4,
                calls: AtomicUsize::new(0),
                replacement: Mutex::new(Some(source)),
            }));
            assert!(
                prepared
                    .args
                    .clone()
                    .run_with_client(
                        &prepared.config,
                        &client,
                        &mut Writer::default(),
                        &|| Ok(())
                    )
                    .is_err()
            );
            no_outputs(&prepared.args);
        }
    }

    struct MutatingWriter {
        path: PathBuf,
        at_flush: bool,
        mutated: bool,
        bytes: Vec<u8>,
    }
    impl MutatingWriter {
        fn mutate(&mut self) -> io::Result<()> {
            if !self.mutated {
                let mut bytes = fs::read(&self.path)?;
                bytes[0] ^= 0x80;
                // Core retains full metadata and inode bindings, while output/context owners also
                // rehash content. A fresh inode makes this control independent of timestamp resolution.
                let replacement = self.path.with_extension("reply-replacement");
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&replacement)?
                    .write_all(&bytes)?;
                fs::rename(replacement, &self.path)?;
                self.mutated = true;
            }
            Ok(())
        }
    }
    impl Write for MutatingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.at_flush {
                self.mutate()?;
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.at_flush {
                self.mutate()?;
            }
            Ok(())
        }
    }

    #[test]
    fn actual_reply_write_and_flush_keep_all_originals_and_outputs_retained() {
        for at_flush in [false, true] {
            for target in 0..4 {
                let fixture = Fixture::new(1, None);
                let prepared = prepare(&fixture, replies(&fixture));
                let path = match target {
                    0 => prepared.args.context.clone(),
                    1 => prepared.args.finality_out.clone(),
                    2 => prepared.args.queries_out.clone(),
                    3 => prepared.args.block_store.join("blocks.data"),
                    _ => unreachable!(),
                };
                let mut writer = MutatingWriter {
                    path,
                    at_flush,
                    mutated: false,
                    bytes: Vec::new(),
                };
                assert!(
                    prepared
                        .args
                        .clone()
                        .run_with_client(
                            &prepared.config,
                            &prepared.client,
                            &mut writer,
                            &|| Ok(())
                        )
                        .is_err()
                );
                assert!(writer.mutated);
                assert!(!writer.bytes.is_empty());
                // Failure is reported after publication; no pathname cleanup can erase another owner.
                assert!(prepared.args.finality_out.exists());
                assert!(prepared.args.queries_out.exists());
            }
        }
    }

    #[test]
    fn reply_failure_retains_completed_pair_and_does_not_report_success() {
        for failure in 0..3 {
            let fixture = Fixture::new(1, None);
            let mut prepared = prepare(&fixture, replies(&fixture));
            let mut writer = Writer {
                fail_write: failure == 0,
                fail_flush: failure == 1,
                ..Writer::default()
            };
            if failure == 2 {
                prepared.args.reply_max_bytes = 1;
            }
            assert!(
                prepared
                    .args
                    .clone()
                    .run_with_client(&prepared.config, &prepared.client, &mut writer, &|| Ok(()))
                    .is_err()
            );
            assert!(prepared.args.finality_out.exists());
            assert!(prepared.args.queries_out.exists());
            if failure == 2 {
                assert!(writer.bytes.is_empty());
            }
        }
    }
    #[test]
    fn original_guard_failure_between_due_native_reads_refuses_the_next_dispatch() {
        for completed_reads in [0, 2, 3, 4] {
            let fixture = Fixture::new(4, None);
            let prepared = prepare(&fixture, replies(&fixture));
            let verify = || {
                ensure!(
                    prepared.transport.consumed() < completed_reads,
                    "original client custody or deadline failed"
                );
                Ok(())
            };
            let mut reply = Vec::new();
            assert!(
                prepared
                    .args
                    .clone()
                    .run_with_client(&prepared.config, &prepared.client, &mut reply, &verify)
                    .is_err()
            );
            assert_eq!(prepared.transport.consumed(), completed_reads);
            assert!(reply.is_empty());
            no_outputs(&prepared.args);
        }
    }
}
