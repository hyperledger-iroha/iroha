//! Prepare command integration over original signed facts and real retained output pairs.
//!
//! These tests parse the public command route and run its actual filesystem owner.
//! A preparation reply identifies transports; only the separate verifier below
//! authenticates their real Native carriers and context witnesses.

use super::filesystem::SuppliedEvidenceBundleV1;
use super::*;
use crate::RunArgs;
use clap::Parser;
use std::{
    ffi::OsString,
    fs,
    io::{self, BufWriter, Write},
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

#[path = "../fixture.rs"]
#[allow(dead_code, reason = "fixture is shared by focused test suites")]
mod fixture;

const OUTPUT_CAP: u64 = 1024 * 1024;
const REPLY_CAP: u64 = 1024;
const MAX_BYTES: u64 = 256 * 1024 * 1024;

struct CommandFixture {
    _directory: tempfile::TempDir,
    directory: PathBuf,
    facts: PathBuf,
    original: Vec<u8>,
    signed: fixture::Fixture,
}
impl CommandFixture {
    fn new(lanes: usize) -> Self {
        iroha_genesis::init_instruction_registry();
        let signed = fixture::Fixture::new(lanes);
        let original = launcher::prepare::tests::encode_facts(
            signed.plan(),
            fixture::limits(),
            signed
                .heights
                .iter()
                .enumerate()
                .map(|(index, height)| SuppliedHeightEvidence {
                    height: index as u64 + 1,
                    finality: norito::encode_canonical(&height.proof).unwrap(),
                    contexts: height.evidence.clone(),
                    queries: height.queries(),
                })
                .collect(),
        );
        let directory = tempfile::Builder::new()
            .prefix("prepare-command-")
            .tempdir_in("/tmp")
            .unwrap();
        let absolute = directory.path().canonicalize().unwrap();
        let facts = absolute.join("original facts.norito");
        fs::write(&facts, &original).unwrap();
        fs::set_permissions(&facts, fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            _directory: directory,
            directory: absolute,
            facts,
            original,
            signed,
        }
    }
    fn paths(&self, name: &str) -> (PathBuf, PathBuf) {
        (
            self.directory.join(format!("{name} request.norito")),
            self.directory.join(format!("{name} bundle.norito")),
        )
    }
    fn args(&self, name: &str) -> Vec<OsString> {
        let (request, bundle) = self.paths(name);
        let mut args = ["kagami", "advanced", "kura", "scaling-evidence", "prepare"]
            .map(OsString::from)
            .to_vec();
        for (flag, value) in [
            ("--invocation-id", OsString::from("07".repeat(32))),
            ("--facts", self.facts.as_os_str().to_owned()),
            (
                "--facts-sha256",
                OsString::from(hex::encode(iroha_crypto::sha256(&self.original))),
            ),
            (
                "--facts-max-bytes",
                OsString::from(self.original.len().to_string()),
            ),
            ("--request-output", request.into_os_string()),
            ("--bundle-output", bundle.into_os_string()),
            (
                "--request-max-bytes",
                OsString::from(OUTPUT_CAP.to_string()),
            ),
            ("--bundle-max-bytes", OsString::from(OUTPUT_CAP.to_string())),
            (
                "--total-max-bytes",
                OsString::from((self.original.len() as u64 + 2 * OUTPUT_CAP).to_string()),
            ),
            ("--reply-max-bytes", OsString::from(REPLY_CAP.to_string())),
        ] {
            args.push(flag.into());
            args.push(value);
        }
        args
    }
    fn no_outputs(&self, name: &str) {
        let (request, bundle) = self.paths(name);
        for path in [request, bundle] {
            assert!(!path.exists());
            assert!(!stage(&path).exists());
        }
    }
    fn verify_transports(&self, name: &str) {
        let (request_path, bundle_path) = self.paths(name);
        let request = fs::read(&request_path).unwrap();
        let bundle_bytes = fs::read(&bundle_path).unwrap();
        let (plan, limits, bindings) = launcher::decode(
            &request,
            iroha_crypto::sha256(&request),
            request.len() as u64,
        )
        .unwrap()
        .into_parts();
        let bundle: SuppliedEvidenceBundleV1 = norito::decode_canonical(&bundle_bytes).unwrap();
        assert_eq!(bundle.version, 1);
        assert_eq!(bundle.heights.len(), self.signed.heights.len());
        assert_eq!(bindings.len(), bundle.heights.len());
        for (binding, row) in bindings.iter().zip(&bundle.heights) {
            assert_eq!(binding.height, row.height);
            assert_eq!(
                binding.contexts_hash,
                iroha_crypto::Hash::new(&row.contexts)
            );
            assert_eq!(
                binding.finality_hash,
                iroha_crypto::Hash::new(&row.finality)
            );
            assert_eq!(
                binding.query_hashes,
                row.queries
                    .iter()
                    .map(iroha_crypto::Hash::new)
                    .collect::<Vec<_>>()
            );
        }
        let mut verifier = ScalingProofVerifier::new(plan, limits).unwrap();
        for (index, (row, original)) in bundle.heights.iter().zip(&self.signed.heights).enumerate()
        {
            assert_eq!(row.height, index as u64 + 1);
            assert_eq!(
                row.finality,
                norito::encode_canonical(&original.proof).unwrap()
            );
            assert_eq!(row.contexts, original.evidence);
            assert_eq!(row.queries, original.queries());
            verifier
                .push_height(
                    &row.finality,
                    &original.block.encode_wire().unwrap(),
                    &row.contexts,
                    &row.queries.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .unwrap();
        }
        assert_eq!(verifier.finish().unwrap().rows().len(), 8);
        assert_eq!(fs::read(&self.facts).unwrap(), self.original);
        for path in [&request_path, &bundle_path] {
            let metadata = fs::metadata(path).unwrap();
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            assert!(!stage(path).exists());
        }
        assert_ne!(
            fs::metadata(&request_path).unwrap().ino(),
            fs::metadata(&bundle_path).unwrap().ino()
        );
    }
}

fn stage(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".publishing");
    path.with_file_name(name)
}
fn set(args: &mut [OsString], flag: &str, value: impl Into<OsString>) {
    let index = args.iter().position(|arg| arg == flag).unwrap();
    args[index + 1] = value.into();
}
fn execute<T: Write>(args: Vec<OsString>, writer: &mut BufWriter<T>) -> crate::Outcome {
    crate::Cli::try_parse_from(args)
        .unwrap()
        .command
        .run(writer)
}
fn run_reply(args: Vec<OsString>) -> (crate::Outcome, Vec<u8>) {
    let mut writer = BufWriter::new(Vec::new());
    let result = execute(args, &mut writer);
    let (bytes, pending) = writer.into_parts();
    assert!(pending.unwrap().is_empty());
    (result, bytes)
}
fn verify_reply(fixture: &CommandFixture, name: &str, bytes: &[u8]) {
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert!(bytes.len() <= REPLY_CAP as usize);
    let reply: norito::json::Value = norito::json::from_slice(bytes).unwrap();
    let object = reply.as_object().unwrap();
    assert_eq!(object.len(), 9);
    assert_eq!(reply["version"].as_u64(), Some(1));
    assert_eq!(reply["operation"].as_str(), Some("prepare"));
    assert_eq!(
        reply["invocation_id"].as_str(),
        Some("07".repeat(32).as_str())
    );
    let (request, bundle) = fixture.paths(name);
    for (role, path) in [
        ("facts", &fixture.facts),
        ("request", &request),
        ("bundle", &bundle),
    ] {
        let original = fs::read(path).unwrap();
        assert_eq!(
            reply[format!("{role}_sha256").as_str()].as_str(),
            Some(hex::encode(iroha_crypto::sha256(&original)).as_str())
        );
        assert_eq!(
            reply[format!("{role}_bytes").as_str()].as_u64(),
            Some(original.len() as u64)
        );
    }
    assert!(!object.contains_key("proof_sha256"));
    assert!(!object.contains_key("proof_iroha_hash"));
    assert!(!object.contains_key("rows"));
    assert!(!String::from_utf8_lossy(bytes).contains(fixture.directory.to_str().unwrap()));
}

#[test]
fn prepare_command_runs_real_one_and_four_lane_pairs_through_the_public_parser() {
    for lanes in [1, 4] {
        let fixture = CommandFixture::new(lanes);
        let (result, bytes) = run_reply(fixture.args("complete"));
        result.unwrap();
        verify_reply(&fixture, "complete", &bytes);
        fixture.verify_transports("complete");
    }
}

#[test]
fn prepare_command_requires_every_explicit_flag_and_has_no_proof_or_compatibility_arguments() {
    let fixture = CommandFixture::new(1);
    let args = fixture.args("arguments");
    assert!(crate::Cli::try_parse_from(args.clone()).is_ok());
    for index in (5..args.len()).step_by(2) {
        let mut omitted = args.clone();
        omitted.drain(index..index + 2);
        assert!(crate::Cli::try_parse_from(omitted).is_err());
    }
    for flag in [
        "--request",
        "--input",
        "--output",
        "--proof-iroha-hash",
        "--compatibility",
    ] {
        let mut unknown = args.clone();
        unknown.extend([OsString::from(flag), OsString::from("00".repeat(32))]);
        assert!(crate::Cli::try_parse_from(unknown).is_err());
    }
    for cap in [0, MAX_BYTES + 1, u64::MAX] {
        for flag in [
            "--facts-max-bytes",
            "--request-max-bytes",
            "--bundle-max-bytes",
            "--total-max-bytes",
            "--reply-max-bytes",
        ] {
            let mut invalid = args.clone();
            set(&mut invalid, flag, cap.to_string());
            assert!(crate::Cli::try_parse_from(invalid).is_err());
        }
    }
    for flag in ["--invocation-id", "--facts-sha256"] {
        for invalid in ["A0".repeat(32), "00".repeat(31), "gg".repeat(32)] {
            let mut changed = args.clone();
            set(&mut changed, flag, invalid);
            assert!(crate::Cli::try_parse_from(changed).is_err());
        }
    }
    let mut alias = args;
    alias.remove(1);
    assert!(crate::Cli::try_parse_from(alias).is_err());
    fixture.no_outputs("arguments");
}

#[test]
fn prepare_command_reply_cap_is_exact_and_small_caps_preserve_outputs_without_a_reply() {
    let fixture = CommandFixture::new(4);
    let (result, expected) = run_reply(fixture.args("sized"));
    result.unwrap();
    for (name, cap, succeeds) in [
        ("exact", expected.len() as u64, true),
        ("short", expected.len() as u64 - 1, false),
        ("tiny", 1, false),
    ] {
        let mut args = fixture.args(name);
        set(&mut args, "--reply-max-bytes", cap.to_string());
        let (result, bytes) = run_reply(args);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(bytes, expected);
        } else {
            assert!(bytes.is_empty());
        }
        fixture.verify_transports(name);
    }
}

#[test]
fn prepare_command_rejects_wrong_facts_and_insufficient_reservations_before_publication() {
    let fixture = CommandFixture::new(1);
    for (name, flag, value) in [
        ("pin", "--facts-sha256", "00".repeat(32)),
        (
            "facts",
            "--facts-max-bytes",
            (fixture.original.len() - 1).to_string(),
        ),
        ("request", "--request-max-bytes", "1".to_owned()),
        ("bundle", "--bundle-max-bytes", "1".to_owned()),
        (
            "total",
            "--total-max-bytes",
            (fixture.original.len() as u64 + 2 * OUTPUT_CAP - 1).to_string(),
        ),
    ] {
        let mut args = fixture.args(name);
        set(&mut args, flag, value);
        let (result, bytes) = run_reply(args);
        assert!(result.is_err());
        assert!(bytes.is_empty());
        fixture.no_outputs(name);
        assert_eq!(fs::read(&fixture.facts).unwrap(), fixture.original);
    }
}

#[test]
fn prepare_command_rejects_existing_output_and_stage_roles_without_overwriting() {
    let fixture = CommandFixture::new(1);
    for (index, is_bundle, is_stage) in [
        (0, false, false),
        (1, true, false),
        (2, false, true),
        (3, true, true),
    ] {
        let name = format!("existing-{index}");
        let (request, bundle) = fixture.paths(&name);
        let chosen = if is_bundle { &bundle } else { &request };
        let path = if is_stage {
            stage(chosen)
        } else {
            chosen.clone()
        };
        fs::write(&path, b"independently existing bytes").unwrap();
        let (result, bytes) = run_reply(fixture.args(&name));
        assert!(result.is_err());
        assert!(bytes.is_empty());
        assert_eq!(fs::read(&path).unwrap(), b"independently existing bytes");
        for other in [&request, &bundle] {
            if other != &path {
                assert!(!other.exists());
            }
            let staged = stage(other);
            if staged != path {
                assert!(!staged.exists());
            }
        }
    }
}

#[test]
fn prepare_command_rejects_output_aliases_and_symlinked_facts() {
    let fixture = CommandFixture::new(1);
    for (name, alias) in [("same", false), ("stage-alias", true)] {
        let (request, _) = fixture.paths(name);
        let mut args = fixture.args(name);
        set(
            &mut args,
            "--bundle-output",
            if alias { stage(&request) } else { request }.into_os_string(),
        );
        let (result, bytes) = run_reply(args);
        assert!(result.is_err());
        assert!(bytes.is_empty());
        fixture.no_outputs(name);
    }
    let alias = fixture.directory.join("facts-link");
    symlink(&fixture.facts, &alias).unwrap();
    let mut args = fixture.args("facts-link");
    set(&mut args, "--facts", alias.into_os_string());
    let (result, bytes) = run_reply(args);
    assert!(result.is_err());
    assert!(bytes.is_empty());
    fixture.no_outputs("facts-link");
    assert_eq!(fs::read(&fixture.facts).unwrap(), fixture.original);
}

#[derive(Debug)]
enum WriteAction {
    FailWrite,
    FailFlush,
    MutateWrite(PathBuf),
    MutateFlush(PathBuf),
}
#[derive(Debug)]
struct ObservedWriter {
    action: WriteAction,
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    mutations: usize,
}
impl ObservedWriter {
    fn new(action: WriteAction) -> Self {
        Self {
            action,
            bytes: vec![],
            writes: 0,
            flushes: 0,
            mutations: 0,
        }
    }
    fn mutate(path: &Path) -> io::Result<()> {
        let mut bytes = fs::read(path)?;
        let byte = bytes
            .first_mut()
            .ok_or_else(|| io::Error::other("empty target"))?;
        *byte ^= 1;
        fs::write(path, bytes)
    }
}
impl Write for ObservedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if matches!(self.action, WriteAction::FailWrite) {
            return Err(io::Error::other("prepare integration write failure"));
        }
        if let WriteAction::MutateWrite(path) = &self.action {
            if self.mutations == 0 {
                Self::mutate(path)?;
                self.mutations += 1;
            }
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if matches!(self.action, WriteAction::FailFlush) {
            return Err(io::Error::other("prepare integration flush failure"));
        }
        if let WriteAction::MutateFlush(path) = &self.action {
            if self.mutations == 0 {
                Self::mutate(path)?;
                self.mutations += 1;
            }
        }
        Ok(())
    }
}

#[test]
fn prepare_command_propagates_real_writer_failures_and_retains_both_transports() {
    let fixture = CommandFixture::new(1);
    for (name, action, message) in [
        (
            "write",
            WriteAction::FailWrite,
            "prepare integration write failure",
        ),
        (
            "flush",
            WriteAction::FailFlush,
            "prepare integration flush failure",
        ),
    ] {
        let mut writer = BufWriter::with_capacity(1, ObservedWriter::new(action));
        let error = execute(fixture.args(name), &mut writer).unwrap_err();
        assert_eq!(error.to_string(), message);
        let (sink, _) = writer.into_parts();
        assert!(sink.writes > 0);
        if name == "write" {
            assert!(sink.bytes.is_empty());
            assert_eq!(sink.flushes, 0);
        } else {
            assert_eq!(sink.flushes, 1);
            verify_reply(&fixture, name, &sink.bytes);
        }
        fixture.verify_transports(name);
    }
}

#[test]
fn prepare_command_rechecks_original_facts_and_both_outputs_after_reply_write_and_flush() {
    for during_flush in [false, true] {
        for role in ["facts", "request", "bundle"] {
            let fixture = CommandFixture::new(4);
            let (request, bundle) = fixture.paths("mutation");
            let target = match role {
                "facts" => fixture.facts.clone(),
                "request" => request.clone(),
                _ => bundle.clone(),
            };
            let action = if during_flush {
                WriteAction::MutateFlush(target.clone())
            } else {
                WriteAction::MutateWrite(target.clone())
            };
            let mut writer = BufWriter::with_capacity(1, ObservedWriter::new(action));
            let result = execute(fixture.args("mutation"), &mut writer);
            assert!(
                result.is_err(),
                "accepted {role} mutation during flush={during_flush}"
            );
            let (sink, pending) = writer.into_parts();
            assert!(pending.unwrap().is_empty());
            assert_eq!(sink.mutations, 1);
            assert!(sink.writes > 0);
            assert_eq!(sink.flushes, 1);
            assert_eq!(sink.bytes.last(), Some(&b'\n'));
            let reply: norito::json::Value = norito::json::from_slice(&sink.bytes).unwrap();
            let changed = fs::read(&target).unwrap();
            assert_eq!(
                reply[format!("{role}_bytes").as_str()].as_u64(),
                Some(changed.len() as u64)
            );
            assert_ne!(
                reply[format!("{role}_sha256").as_str()].as_str(),
                Some(hex::encode(iroha_crypto::sha256(&changed)).as_str())
            );
            assert!(request.exists() && bundle.exists());
            assert!(!stage(&request).exists() && !stage(&bundle).exists());
        }
    }
}
