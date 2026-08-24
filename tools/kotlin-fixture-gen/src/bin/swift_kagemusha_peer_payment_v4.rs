//! Generate the canonical ABI-21 Kagemusha peer-payment fixture used by Swift tests.
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, SnapshotBootstrapAnchor, ValidatorPower,
    },
    offline::{
        KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2, KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2, KagemushaConfidentialMerklePathV2,
        KagemushaNoteMembershipWitnessV2, KagemushaPastaCycleParityV1,
        KagemushaPastaCycleProofEnvelopeV4, KagemushaRecipientPaymentRequestV2,
        KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendBranchClaimV2,
        KagemushaRecursiveSpendBranchV2, KagemushaRecursiveSpendBundleV4,
        KagemushaRecursiveSpendOperationVectorV4, KagemushaRecursiveSpendPeerPaymentV4,
        KagemushaRecursiveSpendPeerSplitTransitionV4, KagemushaRecursiveSpendProofV4,
        KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendStateBoundaryV5,
        KagemushaRecursiveSpendTopUpAnchorV4, KagemushaRecursiveSpendTopUpFinalityEvidenceV4,
        KagemushaRecursiveSpendTopUpProvenanceV4, KagemushaRecursiveSpendTransitionV4,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        KagemushaTopUpAnchorMerkleProofV2, KagemushaTopUpFinalityCompactQcV2,
        KagemushaTopUpFinalityHeightContextV2, KagemushaTopUpFinalityProofV2,
        KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
        kagemusha_recursive_spend_lineage_root_v2, kagemusha_recursive_spend_verifier_key_id_v4,
    },
    peer::PeerId,
    proof::{ProofBox, VerifyingKeyId},
};
use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};
const HEX_LINE_WIDTH: usize = 64;
const USAGE: &str = "usage: swift_kagemusha_peer_payment_v4 \
    --recipient-request-hex PATH [--output PATH | --check PATH]";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, PartialEq, Eq)]
enum OutputMode {
    Stdout,
    Output(PathBuf),
    Check(PathBuf),
}
#[derive(Debug, PartialEq, Eq)]
struct Cli {
    recipient_request_hex: PathBuf,
    output_mode: OutputMode,
}
#[derive(Debug, PartialEq, Eq)]
enum ParseOutcome {
    Run(Cli),
    Help,
}
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<ParseOutcome, String> {
    let mut recipient_request_hex = None;
    let mut output = None;
    let mut check = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let flag = argument
            .to_str()
            .ok_or_else(|| format!("argument name is not valid UTF-8: {argument:?}"))?;
        if matches!(flag, "-h" | "--help") {
            return Ok(ParseOutcome::Help);
        }
        let value = match flag {
            "--recipient-request-hex" | "--output" | "--check" => args
                .next()
                .ok_or_else(|| format!("{flag} requires a path"))?,
            _ => return Err(format!("unknown argument: {flag}")),
        };
        if value.is_empty() {
            return Err(format!("{flag} requires a non-empty path"));
        }
        let path = PathBuf::from(value);
        match flag {
            "--recipient-request-hex" => {
                if recipient_request_hex.replace(path).is_some() {
                    return Err("--recipient-request-hex may be specified only once".to_owned());
                }
            }
            "--output" => {
                if output.replace(path).is_some() {
                    return Err("--output may be specified only once".to_owned());
                }
            }
            "--check" => {
                if check.replace(path).is_some() {
                    return Err("--check may be specified only once".to_owned());
                }
            }
            _ => unreachable!("argument names were validated above"),
        }
    }
    if output.is_some() && check.is_some() {
        return Err("--output and --check are mutually exclusive".to_owned());
    }
    let recipient_request_hex =
        recipient_request_hex.ok_or_else(|| "--recipient-request-hex is required".to_owned())?;
    let output_mode = match (output, check) {
        (Some(path), None) => OutputMode::Output(path),
        (None, Some(path)) => OutputMode::Check(path),
        (None, None) => OutputMode::Stdout,
        (Some(_), Some(_)) => unreachable!("mutually exclusive modes were validated above"),
    };
    Ok(ParseOutcome::Run(Cli {
        recipient_request_hex,
        output_mode,
    }))
}
fn render_fixture_hex(bytes: &[u8]) -> String {
    let encoded = hex::encode(bytes);
    let mut rendered = String::with_capacity(encoded.len() + encoded.len() / HEX_LINE_WIDTH + 1);
    for line in encoded.as_bytes().chunks(HEX_LINE_WIDTH) {
        rendered.push_str(std::str::from_utf8(line).expect("hex output is valid UTF-8"));
        rendered.push('\n');
    }
    rendered
}
fn ensure_plain_file(path: &Path, purpose: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {purpose} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{purpose} must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    Ok(())
}
fn check_output(path: &Path, expected: &[u8]) -> Result<(), String> {
    ensure_plain_file(path, "checked fixture")?;
    let actual = fs::read(path)
        .map_err(|error| format!("failed to read checked fixture {}: {error}", path.display()))?;
    if actual != expected {
        return Err(format!(
            "generated fixture differs from {} (expected {} bytes, found {} bytes)",
            path.display(),
            expected.len(),
            actual.len()
        ));
    }
    Ok(())
}
fn write_output_atomically(path: &Path, contents: &[u8]) -> Result<bool, String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::metadata(parent).map_err(|error| {
        format!(
            "failed to inspect output directory {}: {error}",
            parent.display()
        )
    })?;
    if !parent_metadata.is_dir() {
        return Err(format!(
            "output parent is not a directory: {}",
            parent.display()
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "output must be a regular non-symlink file when it exists: {}",
                    path.display()
                ));
            }
            let current = fs::read(path).map_err(|error| {
                format!("failed to read existing output {}: {error}", path.display())
            })?;
            if current == contents {
                return Ok(false);
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect output {}: {error}",
                path.display()
            ));
        }
    }
    for _ in 0..128 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".swift_kagemusha_peer_payment_v4.{}.{}.tmp",
            process::id(),
            sequence
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create temporary output {}: {error}",
                    temporary.display()
                ));
            }
        };
        let staged = file.write_all(contents).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = staged {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "failed to stage output {}: {error}",
                temporary.display()
            ));
        }
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "failed to atomically publish output {}: {error}",
                path.display()
            ));
        }
        return Ok(true);
    }
    Err(format!(
        "failed to allocate a temporary output beside {}",
        path.display()
    ))
}
fn execution_commitment(seed: u8) -> ExecutionCommitment {
    let ordinary_writes_root = Hash::new([seed, 3]);
    let topup_anchor_root = Hash::new([seed, 4]);
    let executed_block_wire = [seed, 5];
    ExecutionCommitment::new_without_merge_carrier(
        Hash::new([seed, 1]),
        ExecutionCommitment::topup_post_state_root(1, ordinary_writes_root, topup_anchor_root),
        ordinary_writes_root,
        Some(topup_anchor_root),
        1,
        u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64"),
        Hash::new(executed_block_wire),
    )
    .expect("fixture execution commitment must be canonical")
}
fn finality_evidence(
    network_id: NetworkId,
    asset: &AssetDefinitionId,
    amount: KagemushaScaledAmountV2,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    validator_set: &[ValidatorPower],
    seed: u8,
) -> KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
    let payer_key =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture payer key");
    let payer = AccountId::new(payer_key.public_key().clone());
    let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
        network_id,
        payer: payer.clone(),
        asset: AssetId::new(asset.clone(), payer),
        asset_scale: amount.scale,
        amount,
        initial_root: [seed.wrapping_add(1); 32],
        finalized_root: [seed.wrapping_add(2); 32],
        shield_leaf_index: u32::from(seed),
        current_note: KagemushaSpendableNoteDescriptorV2 {
            network_id,
            asset: asset.clone(),
            note_commitment: [seed.wrapping_add(3); 32],
            spend_nullifier: [seed.wrapping_add(4); 32],
            amount,
        },
        topup_operation_id: [seed.wrapping_add(5); 32],
        shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
        shield_verifier_commitment: [seed.wrapping_add(6); 32],
        artifact_binding: binding.clone(),
        finalized_height: 42,
        finalized_tx_hash: [seed.wrapping_add(7); 32],
        anchor_digest: [0; 32],
    }
    .finalize_digest()
    .expect("fixture top-up anchor must be canonical");
    let complete_context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: anchor.finalized_height,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: Some(SnapshotBootstrapAnchor {
            snapshot_height: anchor.finalized_height - 1,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 8])),
            snapshot_block_creation_time_ms: 1_700_000_000_000,
            snapshot_state_hash: Hash::new([seed, 9]),
        }),
        roster: validator_set.to_vec(),
        quorum: DualQuorum::from_roster(validator_set).expect("fixture roster quorum"),
        nexus_amx_context_hash: Hash::new([seed, 11]),
        execution_policy_hash: Hash::new([seed, 12]),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [seed.wrapping_add(12); 32],
    };
    complete_context
        .validate()
        .expect("fixture height context must be canonical");
    let context_id = complete_context.id();
    let round = ConsensusRound {
        context_id,
        height: anchor.finalized_height,
        view: 0,
    };
    let certificate = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject: BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 9])),
            payload_hash: Hash::new([seed, 10]),
        },
        execution_commitment: execution_commitment(seed),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![seed; 96],
    };
    let proof = KagemushaTopUpFinalityProofV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
        anchor: anchor.compact_ref().expect("fixture compact anchor"),
        commit_qc: KagemushaTopUpFinalityCompactQcV2 {
            height_context: KagemushaTopUpFinalityHeightContextV2 {
                context_id,
                network_id,
                protocol_version: PROTOCOL_VERSION,
                height: anchor.finalized_height,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: complete_context.snapshot_bootstrap,
                nexus_amx_context_hash: Hash::new([seed, 11]),
                execution_policy_hash: Hash::new([seed, 12]),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [seed.wrapping_add(12); 32],
            },
            certificate,
        },
        anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
            leaf_index: 0,
            leaf_count: 1,
            siblings: Vec::new(),
        },
    };
    KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        topup_anchor: anchor,
        topup_finality_proof: proof,
    }
}
fn membership_path(
    leaf_index: u32,
    root: [u8; 32],
    sibling_seed: u8,
) -> KagemushaConfidentialMerklePathV2 {
    KagemushaConfidentialMerklePathV2 {
        siblings: (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
            .map(|offset| [sibling_seed.wrapping_add(offset as u8); 32])
            .collect(),
        directions: (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
            .map(|level| ((leaf_index >> level) & 1) as u8)
            .collect(),
        root,
    }
}
fn fixture(request: &KagemushaRecipientPaymentRequestV2) -> KagemushaRecursiveSpendPeerPaymentV4 {
    request
        .validate_public_binding()
        .expect("fixture recipient request must be canonical");
    let network_id = *request.network_id();
    let asset = request.asset().clone();
    let binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "swift-kagemusha-abi21-fixture".to_owned(),
        manifest_sha256: [0x51; 32],
    };
    let mut validator_keys = (0_u8..4)
        .map(|offset| {
            KeyPair::try_from_seed(vec![0x61 + offset; 32], Algorithm::BlsNormal)
                .expect("fixture validator")
        })
        .collect::<Vec<_>>();
    validator_keys.sort_unstable_by_key(|key| PeerId::new(key.public_key().clone()));
    let validator_set = validator_keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let validator_set_pops = validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator PoP")
                .try_into()
                .expect("BLS normal PoP is 96 bytes")
        })
        .collect();
    let roster = KagemushaTopUpFinalityRosterArtifactV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
        network_id,
        artifact_generation: binding.generation.clone(),
        windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
            activates_at_height: 1,
            withdraws_at_height: 100,
            consensus_mode: ConsensusMode::Permissioned,
            validator_set: validator_set.clone(),
            validator_set_pops,
        }],
    };
    let evidence = finality_evidence(
        network_id,
        &asset,
        request.amount(),
        &binding,
        &validator_set,
        0x31,
    );
    let anchor_ref = evidence
        .topup_anchor
        .compact_ref()
        .expect("fixture compact anchor");
    let lineage_root = kagemusha_recursive_spend_lineage_root_v2(anchor_ref.anchor_digest)
        .expect("fixture lineage root");
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        binding.manifest_sha256,
    );
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        network_id,
        asset: asset.clone(),
        asset_scale: request.amount().scale,
        final_root: [0x71; 32],
        next_zero_leaf_index: 7,
        topup_anchor_refs: vec![anchor_ref],
        proof_step_count: 2,
        peer_hop_count: 1,
        current_note: request.recipient_output().clone(),
        branch_claims: vec![
            KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)
                .expect("fixture root branch claim"),
        ],
        transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest: [0x74; 32],
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request
                    .digest()
                    .expect("fixture recipient request digest"),
                operation_id: [0x76; 32],
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        )),
        artifact_binding: binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest().expect("fixture statement digest");
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
    let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
        artifact_generation: binding.generation.clone(),
        manifest_sha256: binding.manifest_sha256,
        step_eq_parameter_generation: "swift-kagemusha-eq-params".to_owned(),
        step_ep_parameter_generation: "swift-kagemusha-ep-params".to_owned(),
        step_eq_circuit_params_sha256: [0x5b; 32],
        step_ep_circuit_params_sha256: [0x5c; 32],
        step_eq_verifier_key_sha256: [0x5d; 32],
        step_ep_verifier_key_sha256: [0x5e; 32],
        state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state_limbs)
            .expect("fixture state boundary"),
        proof: ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
            vec![0x5f],
        ),
    };
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = 1;
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: KagemushaRecursiveSpendOperationVectorV4 {
            limbs: operation_limbs,
        },
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope,
        },
    };
    let witness = KagemushaNoteMembershipWitnessV2 {
        leaf_index: 5,
        input_path: membership_path(5, bundle.statement.final_root, 0x11),
        dummy_input_path: membership_path(
            bundle.statement.next_zero_leaf_index,
            bundle.statement.final_root,
            0x31,
        ),
    };
    let payment = KagemushaRecursiveSpendPeerPaymentV4 {
        recipient_bundle: bundle,
        recipient_membership_witness: witness,
        topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4 {
            topup_finality_roster_artifact: roster,
            topup_finality_evidence: vec![evidence],
        },
    };
    payment
        .validate_public_binding()
        .expect("fixture peer payment must be canonical");
    payment
}
fn read_recipient_request(path: &Path) -> (Vec<u8>, KagemushaRecipientPaymentRequestV2) {
    let encoded = fs::read_to_string(path).expect("read recipient-request hex fixture");
    let compact = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let bytes = hex::decode(compact).expect("decode recipient-request hex fixture");
    let request = norito::decode_from_bytes::<KagemushaRecipientPaymentRequestV2>(&bytes)
        .expect("decode recipient-request fixture");
    let canonical = norito::to_bytes(&request).expect("re-encode recipient-request fixture");
    assert_eq!(
        canonical, bytes,
        "recipient-request fixture must already be canonical"
    );
    (canonical, request)
}
fn run(cli: Cli) -> Result<(), String> {
    let (request_bytes, request) = read_recipient_request(&cli.recipient_request_hex);
    let request_digest = request.digest().expect("derive recipient-request digest");
    let payment = fixture(&request);
    let bytes = norito::to_bytes(&payment).expect("encode fixture peer payment");
    let rendered = render_fixture_hex(&bytes);
    eprintln!("recipient_request_bytes={}", request_bytes.len());
    eprintln!("recipient_request_digest={}", hex::encode(request_digest));
    eprintln!("payment_archive_bytes={}", bytes.len());
    match cli.output_mode {
        OutputMode::Stdout => io::stdout()
            .lock()
            .write_all(rendered.as_bytes())
            .map_err(|error| format!("failed to write fixture to stdout: {error}")),
        OutputMode::Output(path) => {
            let changed = write_output_atomically(&path, rendered.as_bytes())?;
            eprintln!(
                "fixture_output={} status={}",
                path.display(),
                if changed { "written" } else { "unchanged" }
            );
            Ok(())
        }
        OutputMode::Check(path) => {
            check_output(&path, rendered.as_bytes())?;
            eprintln!("fixture_check={} status=ok", path.display());
            Ok(())
        }
    }
}
fn main() {
    let cli = match parse_args(env::args_os().skip(1)) {
        Ok(ParseOutcome::Run(cli)) => cli,
        Ok(ParseOutcome::Help) => {
            println!("{USAGE}");
            return;
        }
        Err(error) => {
            eprintln!("error: {error}\n{USAGE}");
            process::exit(2);
        }
    };
    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(arguments: &[&str]) -> Result<ParseOutcome, String> {
        parse_args(arguments.iter().map(|argument| OsString::from(*argument)))
    }
    #[test]
    fn parses_output_and_check_modes() {
        assert_eq!(
            parse(&[
                "--recipient-request-hex",
                "recipient.hex",
                "--output",
                "payment.hex",
            ]),
            Ok(ParseOutcome::Run(Cli {
                recipient_request_hex: PathBuf::from("recipient.hex"),
                output_mode: OutputMode::Output(PathBuf::from("payment.hex")),
            }))
        );
        assert_eq!(
            parse(&[
                "--check",
                "payment.hex",
                "--recipient-request-hex",
                "recipient.hex",
            ]),
            Ok(ParseOutcome::Run(Cli {
                recipient_request_hex: PathBuf::from("recipient.hex"),
                output_mode: OutputMode::Check(PathBuf::from("payment.hex")),
            }))
        );
    }
    #[test]
    fn rejects_ambiguous_or_duplicate_modes() {
        let ambiguous = parse(&[
            "--recipient-request-hex",
            "recipient.hex",
            "--output",
            "output.hex",
            "--check",
            "checked.hex",
        ])
        .expect_err("output and check must be mutually exclusive");
        assert!(ambiguous.contains("mutually exclusive"));
        let duplicate = parse(&[
            "--recipient-request-hex",
            "one.hex",
            "--recipient-request-hex",
            "two.hex",
        ])
        .expect_err("the input path must be unique");
        assert!(duplicate.contains("only once"));
    }
    #[test]
    fn fixture_rendering_is_lowercase_wrapped_and_newline_terminated() {
        let bytes = (0_u8..33).collect::<Vec<_>>();
        let rendered = render_fixture_hex(&bytes);
        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(
            lines.iter().map(|line| line.len()).collect::<Vec<_>>(),
            [64, 2]
        );
        assert_eq!(lines.concat(), hex::encode(bytes));
        assert!(rendered.ends_with('\n'));
        assert_eq!(
            rendered,
            render_fixture_hex(&(0_u8..33).collect::<Vec<_>>())
        );
    }
    #[test]
    fn fixture_uses_a_canonical_four_validator_commit_quorum() {
        let request_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../crates/connect_norito_bridge/tests/fixtures/\
             offline_recipient_payment_request_v2.hex",
        );
        let (_, request) = read_recipient_request(&request_path);
        let payment = fixture(&request);
        payment
            .validate_public_binding()
            .expect("generated peer payment must remain canonical");

        let window = &payment
            .topup_provenance
            .topup_finality_roster_artifact
            .windows[0];
        window.validate().expect("fixture roster");
        assert_eq!(window.validator_set.len(), 4);
        assert_eq!(window.validator_set_pops.len(), 4);
        let compact_qc = &payment.topup_provenance.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc;
        let context = compact_qc
            .height_context
            .reconstruct_for_roster_window(window)
            .expect("fixture height context");
        let certificate = &compact_qc.certificate;
        assert_eq!(certificate.signers, [0, 1, 2]);
        certificate
            .validate(&context)
            .expect("fixture quorum certificate");
    }
    #[test]
    fn output_is_exactly_checkable_and_unchanged_writes_are_noops() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "swift_kagemusha_peer_payment_v4-test-{}-{sequence}.hex",
            process::id()
        ));
        let _ = fs::remove_file(&path);
        assert!(write_output_atomically(&path, b"fixture\n").expect("write fixture"));
        assert_eq!(fs::read(&path).expect("read fixture"), b"fixture\n");
        assert!(!write_output_atomically(&path, b"fixture\n").expect("keep identical fixture"));
        check_output(&path, b"fixture\n").expect("check exact fixture");
        assert!(
            check_output(&path, b"different\n")
                .expect_err("drift must fail")
                .contains("differs")
        );
        fs::remove_file(path).expect("remove fixture");
    }
}
