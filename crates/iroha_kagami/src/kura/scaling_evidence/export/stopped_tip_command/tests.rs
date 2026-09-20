//! Exact command admission and real fixed-buffer reply failure controls.

use super::*;
use crate::kura::scaling_evidence::export::filesystem::PreparedTransportIdentity;
use clap::Parser;
use iroha_crypto::{Hash, HashOf};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod retained;

#[derive(Parser)]
struct Parse {
    #[command(flatten)]
    args: Args,
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"stopped-tip command test genesis",
    )))
}

fn arguments() -> Vec<String> {
    let mut args = vec!["stopped-tip".to_owned()];
    for (flag, value) in [
        ("invocation-id", "01".repeat(32)),
        ("signed-genesis", "/independent/genesis.nrt".to_owned()),
        ("signed-genesis-sha256", "02".repeat(32)),
        ("signed-genesis-max-bytes", "4096".to_owned()),
        ("network-id", network().to_string()),
        ("block-store", "/independent/kura".to_owned()),
        ("merge-log", "/independent/kura/merge.log".to_owned()),
        ("reply-max-bytes", "512".to_owned()),
        ("first-height", "1".to_owned()),
        ("last-height", "1".to_owned()),
        ("max-committed-blocks", "8".to_owned()),
        ("max-store-data-bytes", "16777216".to_owned()),
        ("max-carrier-bytes", "8388608".to_owned()),
        ("max-merge-log-bytes", "16777216".to_owned()),
        ("max-merge-frames", "8".to_owned()),
        ("reader-max-output-bytes", "16777216".to_owned()),
        ("max-decode-allocation-bytes", "134217728".to_owned()),
        ("owner-uid", "501".to_owned()),
    ] {
        args.extend([format!("--{flag}"), value]);
    }
    args
}

fn args() -> Args {
    Parse::try_parse_from(arguments()).unwrap().args
}

fn set(arguments: &mut [String], flag: &str, value: impl ToString) {
    let index = arguments.iter().position(|v| v == flag).unwrap();
    arguments[index + 1] = value.to_string();
}

fn identity() -> StoppedTipIdentity {
    StoppedTipIdentity {
        genesis: PreparedTransportIdentity {
            raw_sha256: [2; 32],
            byte_length: 123,
        },
        network_id: network(),
        committed_height: 987,
    }
}

#[test]
fn public_parser_requires_every_independent_flag_and_dispatches_existing_scaling_command() {
    let values = arguments();
    let mut public = vec!["kagami", "advanced", "kura", "scaling-evidence"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    public.extend(values.clone());
    crate::Cli::try_parse_from(public).unwrap();
    for index in (1..values.len()).step_by(2) {
        let mut missing = values.clone();
        missing.drain(index..index + 2);
        assert!(Parse::try_parse_from(missing).is_err(), "{}", values[index]);
    }
}

#[test]
fn parser_rejects_noncanonical_hashes_networks_and_unbounded_scalar_caps() {
    for (flag, value) in [
        ("--invocation-id", "AA".repeat(32)),
        ("--signed-genesis-sha256", "f".repeat(63)),
        ("--network-id", hex::encode(network().as_bytes())),
        ("--signed-genesis-max-bytes", "0".to_owned()),
        ("--signed-genesis-max-bytes", "33554433".to_owned()),
        ("--reply-max-bytes", "0".to_owned()),
        ("--reply-max-bytes", "513".to_owned()),
    ] {
        let mut values = arguments();
        set(&mut values, flag, value);
        assert!(Parse::try_parse_from(values).is_err(), "{flag}");
    }
}

#[test]
fn actual_run_rejects_non_genesis_intervals_and_programmatic_caps_before_missing_inputs() {
    for (first, last) in [(0, 1), (1, 2), (2, 2), (1, 0)] {
        let mut values = arguments();
        set(&mut values, "--first-height", first);
        set(&mut values, "--last-height", last);
        let mut writer = BufWriter::new(Vec::new());
        let error = Parse::try_parse_from(values)
            .unwrap()
            .args
            .run(&mut writer)
            .unwrap_err();
        assert!(error.to_string().contains("exactly the genesis interval"));
        assert!(writer.into_inner().unwrap().is_empty());
    }
    for invalid_reply in [false, true] {
        let mut input = args();
        if invalid_reply {
            input.reply_max_bytes = 513;
        } else {
            input.signed_genesis_max_bytes = 0;
        }
        let error = input.run(&mut BufWriter::new(Vec::new())).unwrap_err();
        assert!(error.to_string().contains("genesis or reply reservation"));
    }
}

#[test]
fn reply_contains_exactly_six_scalar_fields_and_counts_the_final_newline() {
    let reply = StoppedTipReply::new([1; 32], 512, identity()).unwrap();
    let raw = &reply.bytes[..reply.len];
    let value: norito::json::Value = norito::json::from_slice(raw).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 6);
    assert_eq!(value["version"].as_u64(), Some(1));
    assert_eq!(value["operation"].as_str(), Some("stopped_tip"));
    assert_eq!(
        value["invocation_id"].as_str(),
        Some("01".repeat(32).as_str())
    );
    assert_eq!(
        value["genesis_sha256"].as_str(),
        Some("02".repeat(32).as_str())
    );
    assert_eq!(value["genesis_bytes"].as_u64(), Some(123));
    assert_eq!(value["committed_height"].as_u64(), Some(987));
    assert_eq!(raw.last(), Some(&b'\n'));
    assert_eq!(raw.iter().filter(|&&b| b == b'\n').count(), 1);
    assert!(StoppedTipReply::new([1; 32], reply.len as u64, identity()).is_ok());
    assert!(StoppedTipReply::new([1; 32], reply.len as u64 - 1, identity()).is_err());
}

#[test]
fn reply_rejects_impossible_identity_bounds_before_writing() {
    for (length, height) in [(0, 1), (MAX_GENESIS_BYTES + 1, 1), (1, 0), (1, 1_000_001)] {
        let mut observed = identity();
        observed.genesis.byte_length = length;
        observed.committed_height = height;
        assert!(StoppedTipReply::new([1; 32], 512, observed).is_err());
    }
    let mut observed = identity();
    observed.genesis.byte_length = MAX_GENESIS_BYTES;
    observed.committed_height = 1_000_000;
    assert!(StoppedTipReply::new([1; 32], 512, observed).is_ok());
}

struct Sink {
    fail_write: bool,
    fail_flush: bool,
    writes: Arc<AtomicUsize>,
    flushes: Arc<AtomicUsize>,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail_write {
            Err(io::Error::other("test reply write"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        if self.fail_flush {
            Err(io::Error::other("test reply flush"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_buffered_reply_propagates_write_and_flush_failures() {
    for (fail_write, fail_flush) in [(true, false), (false, true), (false, false)] {
        let writes = Arc::new(AtomicUsize::new(0));
        let flushes = Arc::new(AtomicUsize::new(0));
        let mut writer = BufWriter::with_capacity(
            1,
            Sink {
                fail_write,
                fail_flush,
                writes: writes.clone(),
                flushes: flushes.clone(),
            },
        );
        let result = StoppedTipReply::new([1; 32], 512, identity())
            .unwrap()
            .write(&mut writer);
        assert_eq!(result.is_err(), fail_write || fail_flush);
        assert!(writes.load(Ordering::SeqCst) > 0);
        assert_eq!(flushes.load(Ordering::SeqCst), usize::from(!fail_write));
    }
}
