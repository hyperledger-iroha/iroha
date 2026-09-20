//! Strict argument, complete reply, and output error controls for the command adapter.

use super::*;
use std::io;

fn write_reply<T: Write>(
    writer: &mut BufWriter<T>,
    common: &CommonArgs,
    operation: &str,
    identity: CanonicalProofIdentity,
    projection: Option<&[u8]>,
) -> Outcome {
    ReplyPrefix::new(common, operation, identity)?.write(writer, projection)
}

fn common() -> CommonArgs {
    CommonArgs {
        invocation_id: [1; 32],
        request: PathBuf::from("/independently/retained/request.norito"),
        request_sha256: [2; 32],
        request_max_bytes: 4096,
        input: PathBuf::from("/independently/retained/proof.norito"),
        input_sha256: [3; 32],
        input_max_bytes: 8192,
        reply_max_bytes: 4096,
    }
}

fn identity() -> CanonicalProofIdentity {
    let bytes = b"exact canonical bytes are separately verified by the filesystem owner";
    CanonicalProofIdentity {
        raw_sha256: iroha_crypto::sha256(bytes),
        iroha_hash: Hash::new(bytes),
        byte_length: bytes.len() as u64,
    }
}

#[test]
fn raw_sha256_parser_requires_one_lowercase_fixed_width_value() {
    assert_eq!(parse_sha256(&"0f".repeat(32)).unwrap(), [15; 32]);
    for bad in [
        "0".repeat(63),
        "0".repeat(65),
        "AB".repeat(32),
        "gg".repeat(32),
        format!(" {}", "0".repeat(64)),
        "é".repeat(32),
    ] {
        assert!(parse_sha256(&bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn iroha_hash_parser_preserves_the_marker_without_converting_raw_sha256() {
    let digest = Hash::new(b"proof");
    assert_eq!(parse_iroha_hash(&digest.to_string()).unwrap(), digest);
    assert!(parse_iroha_hash(&"02".repeat(32)).is_err());
    assert!(parse_iroha_hash(&digest.to_string().to_uppercase()).is_err());
    assert_eq!(parse_sha256(&"02".repeat(32)).unwrap(), [2; 32]);
}

#[test]
fn binding_admission_preserves_distinct_exact_pins_and_rejects_all_invalid_caps() {
    let expected = common();
    let (request, input) = expected.bindings().unwrap();
    assert_eq!(request.path, expected.request);
    assert_eq!(request.sha256, expected.request_sha256);
    assert_eq!(request.max_bytes, expected.request_max_bytes);
    assert_eq!(input.path, expected.input);
    assert_eq!(input.sha256, expected.input_sha256);
    assert_eq!(input.max_bytes, expected.input_max_bytes);
    for cap in [0, MAX_BYTES + 1, u64::MAX] {
        for field in 0..3 {
            let mut changed = common();
            match field {
                0 => changed.request_max_bytes = cap,
                1 => changed.input_max_bytes = cap,
                _ => changed.reply_max_bytes = cap,
            }
            assert!(changed.bindings().is_err());
        }
    }
}

#[test]
fn reader_limits_preserve_each_independently_supplied_bound() {
    let limits = ReaderArgs {
        first_height: 2,
        last_height: 4,
        max_committed_blocks: 9,
        max_store_data_bytes: 1000,
        max_carrier_bytes: 100,
        max_merge_log_bytes: 200,
        max_merge_frames: 7,
        reader_max_output_bytes: 900,
        max_decode_allocation_bytes: 500,
        owner_uid: 42,
    }
    .into_limits()
    .unwrap();
    assert_eq!(
        (
            limits.first_height,
            limits.last_height,
            limits.max_committed_blocks
        ),
        (2, 4, 9)
    );
    assert_eq!(
        (
            limits.max_store_data_bytes,
            limits.max_carrier_bytes,
            limits.max_merge_log_bytes
        ),
        (1000, 100, 200)
    );
    assert_eq!(
        (
            limits.max_merge_frames,
            limits.max_output_bytes,
            limits.max_decode_allocation_bytes,
            limits.owner_uid
        ),
        (7, 900, 500, 42)
    );
}

#[test]
fn reply_binds_exact_identities_and_appends_the_complete_projection_once() {
    let common = common();
    let identity = identity();
    let mut writer = BufWriter::new(Vec::new());
    let projection = br#"[{"sequence":1},{"sequence":2}]"#;
    write_reply(&mut writer, &common, "replay", identity, Some(projection)).unwrap();
    let bytes = writer.into_inner().unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    let reply: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let object = reply.as_object().unwrap();
    assert_eq!(object.len(), 9);
    assert_eq!(reply["version"].as_u64(), Some(1));
    assert_eq!(reply["operation"].as_str(), Some("replay"));
    assert_eq!(
        reply["invocation_id"].as_str(),
        Some(hex::encode(common.invocation_id).as_str())
    );
    assert_eq!(
        reply["request_sha256"].as_str(),
        Some(hex::encode(common.request_sha256).as_str())
    );
    assert_eq!(
        reply["input_sha256"].as_str(),
        Some(hex::encode(common.input_sha256).as_str())
    );
    assert_eq!(
        reply["proof_sha256"].as_str(),
        Some(hex::encode(identity.raw_sha256).as_str())
    );
    assert_eq!(
        reply["proof_iroha_hash"].as_str(),
        Some(identity.iroha_hash.to_string().as_str())
    );
    assert_eq!(reply["proof_bytes"].as_u64(), Some(identity.byte_length));
    assert_eq!(reply["rows"].as_array().unwrap().len(), 2);
    assert!(
        bytes
            .windows(projection.len())
            .filter(|part| *part == projection)
            .count()
            == 1
    );
}

#[test]
fn exact_reply_cap_includes_newline_and_rejects_before_first_output_byte() {
    for projection in [None, Some(b"[]".as_slice())] {
        let operation = if projection.is_some() {
            "replay"
        } else {
            "export"
        };
        let mut common = common();
        let mut full = BufWriter::new(Vec::new());
        write_reply(&mut full, &common, operation, identity(), projection).unwrap();
        let bytes = full.into_inner().unwrap();
        common.reply_max_bytes = bytes.len() as u64;
        let mut exact = BufWriter::new(Vec::new());
        write_reply(&mut exact, &common, operation, identity(), projection).unwrap();
        assert_eq!(exact.into_inner().unwrap(), bytes);
        common.reply_max_bytes -= 1;
        let mut short = BufWriter::new(Vec::new());
        assert!(write_reply(&mut short, &common, operation, identity(), projection).is_err());
        assert!(short.into_inner().unwrap().is_empty());
    }
}

#[test]
fn invalid_reply_shape_never_writes_partial_success() {
    for (operation, projection) in [
        ("prepare", None),
        ("export", Some(b"[]".as_slice())),
        ("replay", None),
    ] {
        let mut writer = BufWriter::new(Vec::new());
        assert!(write_reply(&mut writer, &common(), operation, identity(), projection).is_err());
        assert!(writer.into_inner().unwrap().is_empty());
    }
}

#[derive(Debug)]
struct BrokenWriter {
    fail_write: bool,
    bytes: Vec<u8>,
}
impl Write for BrokenWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "closed reply pipe",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("reply flush failed"))
    }
}

#[test]
fn reply_write_and_flush_errors_propagate_to_terminal_failure() {
    for fail_write in [true, false] {
        let mut writer = BufWriter::new(BrokenWriter {
            fail_write,
            bytes: Vec::new(),
        });
        assert!(write_reply(&mut writer, &common(), "export", identity(), None).is_err());
        assert_eq!(writer.get_ref().bytes.is_empty(), fail_write);
    }
}

#[test]
fn reply_prefix_reserves_exact_projection_space_before_materialization() {
    let mut common = common();
    let first = ReplyPrefix::new(&common, "replay", identity()).unwrap();
    let framing = first.bytes.len() as u64 + 2;
    common.reply_max_bytes = framing + 513;
    let exact = ReplyPrefix::new(&common, "replay", identity()).unwrap();
    assert_eq!(exact.projection_bytes, 513);
    for short in [0, 1] {
        common.reply_max_bytes = framing + short;
        assert!(ReplyPrefix::new(&common, "replay", identity()).is_err());
    }
}

fn prepare_args() -> PrepareArgs {
    PrepareArgs {
        invocation_id: [1; 32],
        facts: PathBuf::from("/independently/retained/facts.norito"),
        facts_sha256: [2; 32],
        facts_max_bytes: 1000,
        request_output: PathBuf::from("/independently/retained/request.norito"),
        bundle_output: PathBuf::from("/independently/retained/bundle.norito"),
        request_max_bytes: 2000,
        bundle_max_bytes: 3000,
        total_max_bytes: 6000,
        reply_max_bytes: 1024,
    }
}

fn prepare_identity() -> PreparedLaunchIdentity {
    use super::super::export::filesystem::PreparedTransportIdentity;
    PreparedLaunchIdentity {
        facts: PreparedTransportIdentity {
            raw_sha256: [2; 32],
            byte_length: 101,
        },
        request: PreparedTransportIdentity {
            raw_sha256: [3; 32],
            byte_length: 202,
        },
        bundle: PreparedTransportIdentity {
            raw_sha256: [4; 32],
            byte_length: 303,
        },
    }
}

#[test]
fn preparation_bindings_require_each_bound_and_the_complete_aggregate() {
    let args = prepare_args();
    let (facts, caps) = args.bindings().unwrap();
    assert_eq!(facts.path, args.facts);
    assert_eq!(facts.sha256, args.facts_sha256);
    assert_eq!(facts.max_bytes, args.facts_max_bytes);
    assert_eq!(caps.request_bytes, args.request_max_bytes);
    assert_eq!(caps.bundle_bytes, args.bundle_max_bytes);
    assert_eq!(caps.total_bytes, args.total_max_bytes);
    for invalid in [0, MAX_BYTES + 1, u64::MAX] {
        for field in 0..5 {
            let mut args = prepare_args();
            match field {
                0 => args.facts_max_bytes = invalid,
                1 => args.request_max_bytes = invalid,
                2 => args.bundle_max_bytes = invalid,
                3 => args.total_max_bytes = invalid,
                _ => args.reply_max_bytes = invalid,
            }
            assert!(args.bindings().is_err());
        }
    }
    let mut short = prepare_args();
    short.total_max_bytes -= 1;
    assert!(short.bindings().is_err());
    let mut oversized = prepare_args();
    oversized.facts_max_bytes = MAX_BYTES;
    oversized.request_max_bytes = MAX_BYTES;
    oversized.bundle_max_bytes = MAX_BYTES;
    oversized.total_max_bytes = MAX_BYTES;
    assert!(oversized.bindings().is_err());
}

#[test]
fn preparation_reply_binds_only_the_three_raw_file_identities() {
    let args = prepare_args();
    let identity = prepare_identity();
    let mut writer = BufWriter::new(Vec::new());
    PrepareReply::new(&args, identity)
        .unwrap()
        .write(&mut writer)
        .unwrap();
    let bytes = writer.into_inner().unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    let reply: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    assert_eq!(reply.as_object().unwrap().len(), 9);
    assert_eq!(reply["version"].as_u64(), Some(1));
    assert_eq!(reply["operation"].as_str(), Some("prepare"));
    assert_eq!(
        reply["invocation_id"].as_str(),
        Some(hex::encode(args.invocation_id).as_str())
    );
    for (role, file) in [
        ("facts", identity.facts),
        ("request", identity.request),
        ("bundle", identity.bundle),
    ] {
        assert_eq!(
            reply[format!("{role}_sha256").as_str()].as_str(),
            Some(hex::encode(file.raw_sha256).as_str())
        );
        assert_eq!(
            reply[format!("{role}_bytes").as_str()].as_u64(),
            Some(file.byte_length)
        );
    }
    let mut changed = identity;
    changed.facts.raw_sha256[0] ^= 1;
    assert!(PrepareReply::new(&args, changed).is_err());
}

#[test]
fn preparation_reply_cap_includes_newline_and_precedes_any_reply_write() {
    let mut args = prepare_args();
    let identity = prepare_identity();
    let first = PrepareReply::new(&args, identity).unwrap();
    assert!(first.bytes.len() <= MAX_REPLY_HEADER_BYTES);
    args.reply_max_bytes = first.bytes.len() as u64;
    let exact = PrepareReply::new(&args, identity).unwrap();
    assert_eq!(exact.bytes, first.bytes);
    args.reply_max_bytes -= 1;
    assert!(PrepareReply::new(&args, identity).is_err());
    for invalid in [0, MAX_BYTES + 1, u64::MAX] {
        args.reply_max_bytes = invalid;
        assert!(PrepareReply::new(&args, identity).is_err());
    }
    let mut largest = identity;
    largest.facts.byte_length = u64::MAX;
    largest.request.byte_length = u64::MAX;
    largest.bundle.byte_length = u64::MAX;
    let largest = PrepareReply::new(&prepare_args(), largest).unwrap();
    assert!(largest.bytes.len() <= MAX_REPLY_HEADER_BYTES);
}

#[test]
fn preparation_reply_write_and_flush_failures_propagate() {
    for fail_write in [true, false] {
        let mut writer = BufWriter::new(BrokenWriter {
            fail_write,
            bytes: Vec::new(),
        });
        assert!(
            PrepareReply::new(&prepare_args(), prepare_identity())
                .unwrap()
                .write(&mut writer)
                .is_err()
        );
        assert_eq!(writer.get_ref().bytes.is_empty(), fail_write);
    }
}
