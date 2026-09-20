//! Real command publication, existing prepare/export/replay and retained reply controls.

use super::*;
use crate::kura::scaling_evidence::export::{
    filesystem::{
        PrepareOutputCaps, PreparedOutputPair, ProofOutput, export_bound_request, open_launcher,
        prepare_bound, replay_bound_request,
    },
    launcher::prepare::assemble::tests::{Fixture, with_facts_assembly_stack},
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::Mutex,
};
use zeroize::Zeroizing;

#[derive(Parser)]
struct ReaderParse {
    #[command(flatten)]
    reader: ReaderArgs,
}

fn from_fixture(fixture: &Fixture) -> Args {
    let mut input = args();
    let b = fixture.bindings();
    input.originals = OriginalArgs {
        manifest: b.manifest.path,
        manifest_sha256: b.manifest.sha256,
        manifest_max_bytes: b.manifest.max_bytes,
        signed_genesis: b.signed_genesis.path,
        signed_genesis_sha256: b.signed_genesis.sha256,
        signed_genesis_max_bytes: b.signed_genesis.max_bytes,
        peer_config: b.peer_configs.iter().map(|p| p.path.clone()).collect(),
        peer_config_sha256: b.peer_configs.iter().map(|p| p.sha256).collect(),
        peer_config_max_bytes: b.peer_configs[0].max_bytes,
        context: b.context.path,
        context_sha256: b.context.sha256,
        context_max_bytes: b.context.max_bytes,
        journal: b.journal.path,
        journal_sha256: b.journal.sha256,
        journal_max_bytes: b.journal.max_bytes,
        finality: b.finality.path,
        finality_sha256: b.finality.sha256,
        finality_max_bytes: b.finality.max_bytes,
        queries: b.queries.path,
        queries_sha256: b.queries.sha256,
        queries_max_bytes: b.queries.max_bytes,
    };
    let genesis = fixture.genesis();
    input.genesis = GenesisArgs {
        chain_id: genesis.chain_id,
        network_id: genesis.network_id,
        chain_discriminant: genesis.chain_discriminant,
        genesis_public_key: genesis.genesis_public_key,
        validator: Vec::from(genesis.validators),
    };
    let j = fixture.journal();
    input.journal = JournalArgs {
        lanes: match j.variant {
            JournalVariant::OneLane => Variant::One,
            JournalVariant::FourLane => Variant::Four,
        },
        workload_seed: parse_sha256(&j.seed).unwrap(),
        pair_index: j.pair_index,
        account: j
            .accounts
            .iter()
            .map(|a| {
                a.authority
                    .to_i105_for_discriminant(input.genesis.chain_discriminant)
                    .unwrap()
            })
            .collect(),
        rate_numerator: j.timing.rate_numerator,
        rate_denominator: j.timing.rate_denominator,
        warmup_ns: j.timing.warmup_ns,
        measurement_ns: j.timing.measurement_ns,
        drain_ns: j.timing.drain_ns,
        submission_lag_bound_ns: j.timing.submission_lag_bound_ns,
        preparation_lookahead: j.bounds.preparation_lookahead,
        preparation_concurrency: j.bounds.preparation_concurrency,
        preparation_ahead_ns: j.bounds.preparation_ahead_ns,
        max_submissions: j.bounds.max_submissions,
        max_in_flight: j.bounds.max_in_flight,
        max_status_requests: j.bounds.max_status_requests,
        poll_interval_ns: j.bounds.poll_interval_ns,
        journal_max_requests: j.bounds.max_requests,
        resource_interval_ns: j.sampling.interval_ns,
        resource_response_deadline_ns: j.sampling.response_deadline_ns,
        resource_max_start_lag_ns: j.sampling.max_start_lag_ns,
    };
    let v = fixture.verification_limits();
    input.verification = VerificationArgs {
        proof_max_bytes: v.admitted_proof_bytes,
        verification_input_max_bytes: v.input_bytes,
        verification_output_max_bytes: v.output_bytes,
        max_heights: v.heights,
        max_requests: v.requests,
        max_leaves_per_carrier: v.leaves_per_carrier,
    };
    let r = fixture.reader_limits();
    let mut reader = vec!["facts-reader".to_owned()];
    for (flag, value) in [
        ("first-height", r.first_height),
        ("last-height", r.last_height),
        ("max-committed-blocks", r.max_committed_blocks),
        ("max-store-data-bytes", r.max_store_data_bytes),
        ("max-carrier-bytes", r.max_carrier_bytes as u64),
        ("max-merge-log-bytes", r.max_merge_log_bytes),
        ("max-merge-frames", r.max_merge_frames),
        ("reader-max-output-bytes", r.max_output_bytes),
        (
            "max-decode-allocation-bytes",
            r.max_decode_allocation_bytes as u64,
        ),
        ("owner-uid", u64::from(r.owner_uid)),
    ] {
        reader.extend([format!("--{flag}"), value.to_string()]);
    }
    input.reader = ReaderParse::try_parse_from(reader).unwrap().reader;
    let caps = fixture.caps();
    input.source_max_bytes = caps.input_bytes;
    input.facts_max_bytes = caps.facts_bytes;
    input.total_max_bytes = caps.total_bytes;
    input.assembly_decode_max_bytes = caps.decode_bytes;
    input.block_store = fixture.block_store().to_owned();
    input.merge_log = fixture.merge_log().to_owned();
    input.facts_output = fixture.output_path();
    input
}

fn input_binding(path: &Path, maximum: u64) -> ProofInputBinding {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() as u64 <= maximum);
    ProofInputBinding {
        path: path.to_owned(),
        sha256: iroha_crypto::sha256(bytes),
        max_bytes: maximum,
    }
}

#[test]
fn actual_facts_command_flows_through_existing_prepare_export_and_independent_replay() {
    with_facts_assembly_stack(|| {
        for lanes in [1, 4] {
            let fixture = Fixture::new(lanes);
            let mut writer = BufWriter::new(Vec::new());
            from_fixture(&fixture).run(&mut writer).unwrap();
            let reply = writer.into_inner().unwrap();
            let metadata: norito::json::Value = norito::json::from_slice(&reply).unwrap();
            assert_eq!(metadata.as_object().unwrap().len(), 5);
            let raw = fs::read(fixture.output_path()).unwrap();
            assert_eq!(
                metadata["facts_sha256"].as_str(),
                Some(hex::encode(iroha_crypto::sha256(&raw)).as_str())
            );
            assert_eq!(metadata["facts_bytes"].as_u64(), Some(raw.len() as u64));
            let parent = fixture.output_path().parent().unwrap().to_owned();
            let request = parent.join("request.nrt");
            let bundle = parent.join("bundle.nrt");
            let transport_cap = 8 * 1024 * 1024;
            let prepared = prepare_bound(
                input_binding(&fixture.output_path(), fixture.caps().facts_bytes),
                PreparedOutputPair::admit(
                    &request,
                    &bundle,
                    PrepareOutputCaps {
                        request_bytes: transport_cap,
                        bundle_bytes: transport_cap,
                        total_bytes: fixture.caps().facts_bytes + 2 * transport_cap,
                    },
                )
                .unwrap(),
            )
            .unwrap();
            let prepared_identity = prepared.identity().unwrap();
            assert_eq!(
                prepared_identity.facts.raw_sha256,
                iroha_crypto::sha256(&raw)
            );
            assert_eq!(
                prepared_identity.request.raw_sha256,
                input_binding(&request, transport_cap).sha256
            );
            assert_eq!(
                prepared_identity.bundle.raw_sha256,
                input_binding(&bundle, transport_cap).sha256
            );
            let verified = export_bound_request(
                open_launcher(input_binding(&request, transport_cap)).unwrap(),
                fixture.block_store(),
                fixture.merge_log(),
                fixture.reader_limits(),
                input_binding(&bundle, transport_cap),
            )
            .unwrap();
            let proof_path = parent.join("proof.nrt");
            let proof_cap = fixture.verification_limits().output_bytes;
            let proof = ProofOutput::admit(&proof_path, proof_cap)
                .unwrap()
                .publish(verified)
                .unwrap();
            let proof_identity = proof.identity().unwrap();
            let replayed = replay_bound_request(
                open_launcher(input_binding(&request, transport_cap)).unwrap(),
                proof_identity.iroha_hash,
                input_binding(&proof_path, proof_cap),
            )
            .unwrap();
            assert_eq!(replayed.identity().unwrap(), proof_identity);
            let rows: norito::json::Value =
                norito::json::from_slice(&replayed.json_projection(1024 * 1024).unwrap()).unwrap();
            let rows = rows.as_array().unwrap();
            assert_eq!(rows.len(), 8);
            assert!(
                rows.iter()
                    .all(|row| row["carrier_height"].as_u64() == Some(2))
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| row["phase"].as_str() == Some("warmup"))
                    .count(),
                4
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| row["phase"].as_str() == Some("measurement"))
                    .count(),
                4
            );
            assert_eq!(prepared.identity().unwrap(), prepared_identity);
        }
    });
}

struct ReplacingWriter {
    target: PathBuf,
    saved: PathBuf,
    at_flush: bool,
    changed: bool,
    output: Arc<Mutex<Vec<u8>>>,
}
impl ReplacingWriter {
    fn replace(&mut self) -> io::Result<()> {
        if self.changed {
            return Ok(());
        }
        let prior = fs::metadata(&self.target)?;
        let bytes = Zeroizing::new(fs::read(&self.target)?);
        fs::rename(&self.target, &self.saved)?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&self.target)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        assert_ne!(fs::metadata(&self.target)?.ino(), prior.ino());
        self.changed = true;
        Ok(())
    }
}
impl Write for ReplacingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.at_flush {
            self.replace()?;
        }
        self.output.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.at_flush {
            self.replace()?;
        }
        Ok(())
    }
}

#[test]
fn actual_command_detects_original_config_core_and_output_replacement_during_reply() {
    with_facts_assembly_stack(|| {
        let fixture = Fixture::new(4);
        let parent = fixture.output_path().parent().unwrap().to_owned();
        let originals = fixture.original_paths();
        for at_flush in [false, true] {
            for role in 0..4 {
                let mut input = from_fixture(&fixture);
                input.facts_output =
                    parent.join(format!("reply-{}-{role}.nrt", u8::from(at_flush)));
                let target = match role {
                    0 => originals[0].clone(),
                    1 => originals[2].clone(),
                    2 => fixture.block_store().join("blocks.count.norito"),
                    _ => input.facts_output.clone(),
                };
                let saved = parent.join(format!("saved-{}-{role}.nrt", u8::from(at_flush)));
                let output = Arc::new(Mutex::new(Vec::new()));
                let mut writer = BufWriter::with_capacity(
                    1,
                    ReplacingWriter {
                        target: target.clone(),
                        saved: saved.clone(),
                        at_flush,
                        changed: false,
                        output: Arc::clone(&output),
                    },
                );
                let result = input.run(&mut writer);
                assert!(
                    result.is_err(),
                    "accepted changed role{role} at_flush{at_flush}"
                );
                assert!(
                    writer.get_ref().changed,
                    "mutation did not reach the actual reply boundary"
                );
                assert!(
                    !output.lock().unwrap().is_empty(),
                    "no actual reply bytes were written"
                );
                // The command retains surviving published artifacts. Only this fixture restores
                // an original namespace after all failed owners have been consumed.
                if role < 3 {
                    fs::rename(&saved, &target).unwrap();
                } else {
                    assert!(saved.is_file() && target.is_file());
                }
            }
        }
    });
}
