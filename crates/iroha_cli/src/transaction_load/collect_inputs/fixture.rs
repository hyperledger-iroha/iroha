//! Offline collection fixture with real four-validator signatures and canonical BlockStore storage.
//!
//! This fixture proves authentication and transcript reconciliation. It does not
//! run State, synthesize a state membership proof, or claim runtime execution of
//! its opaque retained source bundle. All global, merge, lane and READY quorum
//! signatures use exactly three of the same four BLS-normal validators.
//! Adapted from the separately retained Kagami authentication fixture; this module
//! imports public Core/data-model/SDK APIs only and defines no wire DTO or production shim.
//! The test-local merge-log writer uses the existing four-byte payload-length framing.
//! Opaque source_bundle bytes do not pass autonomous source admission and cannot establish
//! State execution, valid runtime autonomous payloads, or full live Kura writer acceptance.

use eyre::{Result, ensure, eyre};
use iroha::http::{HttpTransport, Method, Response, TransportFuture, TransportRequest};
use iroha_core::merge::{
    MergeLedgerCandidate, merge_execution_batch_hash, merge_execution_entrypoint_merkle_root,
    merge_execution_result_merkle_root, merge_execution_root, merge_expected_post_state_hash,
    merge_qc_message_digest,
};
use iroha_core::{
    kura::{
        BlockStore, CanonicalKuraEvidenceComplete, CanonicalKuraEvidenceLimits,
        CanonicalKuraEvidenceReader, CanonicalKuraMergeRequest,
    },
    merge::merge_activation_root,
    queue::{LaneQueueReservationKeyV1, RoutingDecision, RoutingPlan},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{
        BlockHeader, SignedBlock,
        consensus::{CertPhase, LaneBlockProposalV1},
        consensus_v2::MergeCarrierCommitmentV1,
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{InstructionBox, SetKeyValue},
    merge::{MergeLaneAuthorityCatalogV1, MergeLaneBinding, MergeLaneExecution, MergeLedgerEntry},
    nexus::{LaneLifecycleIncarnationEntry, LaneLifecycleParameterV1},
    query::CommittedTransaction,
    transaction::{Executable, SignedTransaction, signed::TransactionEntrypoint},
};
use iroha_data_model::{
    block::{
        BlockExecutionContextBundle, CertifiedMergeLedgerReference,
        builder::BlockBuilder,
        consensus::{
            LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockQcV1,
            LanePayloadAvailabilityBodyV1, LanePayloadAvailabilityQcV1,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, Vote, finality::V2FinalityArtifact,
        },
    },
    bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
        KagemushaMintFinalityValidatorKeysV1,
    },
    merge::{MergeExecutionBatch, MergeLaneSignerProof, MergeQuorumCertificate, MergeSignerProof},
    query::CertifiedMergeTransactionInclusion,
    transaction::{FeePaymentIntent, TransactionBuilder, signed::TransactionResult},
    trigger::DataTriggerSequence,
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::json::Json;
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn h(label: &str) -> Hash {
    Hash::new(label.as_bytes())
}
fn keys() -> Vec<KeyPair> {
    let mut keys: Vec<_> = (1..=4)
        .map(|n| KeyPair::try_from_seed(vec![n; 32], Algorithm::BlsNormal).unwrap())
        .collect();
    keys.sort_by_key(|k| PeerId::new(k.public_key().clone()));
    keys
}
fn peers(keys: &[KeyPair]) -> Vec<PeerId> {
    keys.iter()
        .map(|k| PeerId::new(k.public_key().clone()))
        .collect()
}
fn aggregate(keys: &[KeyPair], message: &[u8]) -> Vec<u8> {
    let shares: Vec<_> = keys
        .iter()
        .take(3)
        .map(|k| {
            Signature::try_new(k.private_key(), message)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect();
    iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap()
}
fn pops(keys: &[KeyPair]) -> Vec<Vec<u8>> {
    keys.iter()
        .map(|k| iroha_crypto::bls_normal_pop_prove(k.private_key()).unwrap())
        .collect()
}
fn context(keys: &[KeyPair], network_id: NetworkId) -> HeightContext {
    let roster: Vec<_> = peers(keys)
        .into_iter()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect();
    let mint = KagemushaMintFinalityEpochRosterV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        epoch: 0,
        validators: roster
            .iter()
            .enumerate()
            .map(|(i, v)| KagemushaMintFinalityValidatorKeysV1 {
                validator: v.validator.clone(),
                eq_proof_public_key: [u8::try_from(i + 1).unwrap(); 32],
                ep_proof_public_key: [u8::try_from(i + 17).unwrap(); 32],
            })
            .collect(),
    };
    HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        kagemusha_mint_finality_epoch_id: mint.finality_epoch_id().unwrap(),
        kagemusha_mint_finality_epoch_roster: mint,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster,
        nexus_amx_context_hash: h("nexus execution context"),
        execution_policy_hash: h("runtime execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0xA5; 32],
    }
}
fn signed_proof(
    keys: &[KeyPair],
    context: HeightContext,
    block: &SignedBlock,
    merge: Option<HashOf<MergeLedgerEntry>>,
) -> BridgeFinalityProof {
    let wire = block.encode_wire().unwrap();
    let mut execution = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        h("pre state"),
        h("post state"),
        h("ordinary writes"),
        wire.len() as u64,
        Hash::new(&wire),
    );
    execution.merge_carrier = merge.map(MergeCarrierCommitmentV1::new);
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: block.header().view_change_index(),
    };
    let vote = Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: execution,
        signer: 0,
        signature: Vec::new(),
    };
    let qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: execution,
        signers: vec![0, 1, 2],
        aggregate_signature: aggregate(keys, &vote.signature_preimage()),
    };
    let artifact = V2FinalityArtifact::new(context, subject, qc, pops(keys));
    artifact.verify().expect("actual 3-of-4 global finality");
    artifact.validate_for_header(&block.header()).unwrap();
    BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: block.header(),
        finality_artifact: artifact,
    }
}
fn binding(index: usize) -> MergeLaneBinding {
    MergeLaneBinding {
        lane_id: LaneId::new(index as u32),
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_config_hash: h(&format!("lane config {index}")),
        incarnation: h(&format!("incarnation {index}")),
        activation_height: 1,
    }
}
fn lane(
    keys: &[KeyPair],
    network_id: NetworkId,
    binding: &MergeLaneBinding,
    txs: Vec<TransactionEntrypoint>,
) -> MergeLaneExecution {
    let validators = peers(keys);
    let hashes: Vec<_> = txs.iter().map(|t| Hash::from(t.hash())).collect();
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: binding.lane_id,
        dataspace_id: binding.dataspace_id,
        lane_incarnation: binding.incarnation,
        proposal_height: 1,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: h("lane subject"),
        payload_ownership_hash: h("payload owner"),
        rbc_instance_hash: h("rbc instance"),
        accepted_candidate_indices: (0..txs.len() as u64).collect(),
        accepted_transaction_hashes: hashes.clone(),
        validator_set_hash_version: 1,
        validator_set_hash: HashOf::new(&validators),
        validator_set: validators.clone(),
        validator_count: 4,
        min_quorum: 3,
        qc_mode_tag: "permissioned:canonical-authentication-fixture".to_owned(),
        descriptor_hash: h("unset descriptor"),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: h("unset proposal"),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let d = &proposal.descriptor;
    let payload_hash = h("retained autonomous payload identity");
    let ready = LanePayloadAvailabilityBodyV1 {
        version: 1,
        network_id,
        epoch: 0,
        lane_id: d.lane_id,
        dataspace_id: d.dataspace_id,
        lane_incarnation: d.lane_incarnation,
        proposal_height: 1,
        lane_block_height: 1,
        origin_lane_block_view: 0,
        origin_proposal_hash: proposal.proposal_hash,
        origin_descriptor_hash: d.descriptor_hash,
        current_lane_block_view: 0,
        current_proposal_hash: proposal.proposal_hash,
        current_descriptor_hash: d.descriptor_hash,
        current_subject_hash: d.subject_hash,
        current_payload_ownership_hash: d.payload_ownership_hash,
        current_rbc_instance_hash: d.rbc_instance_hash,
        executable_payload_hash: payload_hash,
        validator_set_hash_version: 1,
        validator_set_hash: d.validator_set_hash,
        validator_count: 4,
        min_quorum: 3,
        qc_mode_tag: d.qc_mode_tag.clone(),
    };
    let ready_qc = LanePayloadAvailabilityQcV1 {
        body: ready.clone(),
        validator_set_hash_version: 1,
        validator_set_hash: d.validator_set_hash,
        validator_set: validators.clone(),
        validator_set_pops: pops(keys),
        signers_bitmap: vec![7],
        bls_aggregate_signature: aggregate(keys, &ready.signature_preimage()),
    };
    let qc = |phase| {
        let body = proposal.vote_body(phase);
        LaneBlockQcV1 {
            bls_aggregate_signature: aggregate(keys, &body.signature_preimage()),
            body,
            validator_set_hash_version: 1,
            validator_set_hash: d.validator_set_hash,
            validator_set: validators.clone(),
            signers_bitmap: vec![7],
            payload_availability_qc: (phase == CertPhase::Prepare).then(|| ready_qc.clone()),
        }
    };
    let routing = RoutingPlan::single(RoutingDecision::new(d.lane_id, d.dataspace_id));
    let reservations: Vec<_> = txs
        .iter()
        .map(|tx| {
            norito::encode_canonical(&LaneQueueReservationKeyV1 {
                version: 1,
                entrypoint_hash: tx.hash(),
                queue_plan_admission_binding_hash: h("signed queue admission binding"),
                routing_plan_digest: routing.digest(),
                coordinator_leg: routing.coordinator_leg(),
                lane_id: d.lane_id,
                dataspace_id: d.dataspace_id,
                lane_incarnation: d.lane_incarnation,
                proposal_height: 1,
                lane_block_height: 1,
                lane_block_view: 0,
                reservation_owner_hash: h("reservation owner"),
                proposal_identity_hash: proposal.proposal_hash,
            })
            .unwrap()
        })
        .collect();
    let results: Vec<_> = txs
        .iter()
        .map(|_| TransactionResult::from(Ok(DataTriggerSequence::default())))
        .collect();
    let settlement = LaneBlockCommitment {
        block_height: 1,
        lane_id: d.lane_id,
        lane_incarnation: d.lane_incarnation,
        dataspace_id: d.dataspace_id,
        tx_count: txs.len() as u64,
        total_local_amount: 0u32.into(),
        total_xor_due: 0u32.into(),
        total_xor_after_haircut: 0u32.into(),
        total_xor_variance: 0u32.into(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    // Authentication consumer treats source bytes as globally authenticated opaque
    // evidence. Runtime producer/State replay is a separate owner and fixture.
    let source_bundle = norito::encode_canonical(&txs).unwrap();
    MergeLaneExecution {
        source_bundle_hash: Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
            &source_bundle,
        ]),
        source_bundle,
        prepare_qc: qc(CertPhase::Prepare),
        commit_qc: qc(CertPhase::Commit),
        signer_proofs: keys
            .iter()
            .take(3)
            .map(|k| MergeLaneSignerProof {
                public_key: k.public_key().clone(),
                proof_of_possession: iroha_crypto::bls_normal_pop_prove(k.private_key()).unwrap(),
            })
            .collect(),
        autonomous_network_id: network_id,
        autonomous_epoch: 0,
        autonomous_payload_hash: payload_hash,
        entrypoint_hashes: hashes,
        authenticated_signed_replay_aliases: vec![None; txs.len()],
        reservation_keys: reservations,
        routing_plans: vec![norito::encode_canonical(&routing).unwrap(); txs.len()],
        native_amx_receipts: vec![None; txs.len()],
        result_hashes: results.iter().map(|r| Hash::from(r.hash())).collect(),
        entrypoints: txs,
        results,
        settlement_hash: HashOf::new(&settlement),
        settlement_commitment: settlement,
        origin_proposal: proposal.clone(),
        proposal,
        fastpq_transcripts: Default::default(),
    }
}
fn rehash_batch(batch: &mut MergeExecutionBatch) {
    batch.entrypoint_count = batch.lanes.iter().map(|l| l.entrypoints.len() as u64).sum();
    batch.entrypoint_merkle_root = merge_execution_entrypoint_merkle_root(&batch.lanes).unwrap();
    batch.result_merkle_root = merge_execution_result_merkle_root(&batch.lanes).unwrap();
    batch.execution_root = merge_execution_root(&batch.lanes);
    batch.expected_post_state_hash = merge_expected_post_state_hash(
        batch.base_state_height,
        batch.base_state_hash,
        batch.write_set_root,
    );
    batch.batch_hash = merge_execution_batch_hash(batch);
}
fn sign_merge(entry: &mut MergeLedgerEntry, keys: &[KeyPair], network_id: NetworkId) {
    let digest = merge_qc_message_digest(
        &network_id,
        &MergeLedgerCandidate::from(&*entry),
        1,
        entry.merge_qc.validator_set_hash,
    );
    entry.merge_qc.message_digest = digest;
    entry.merge_qc.aggregate_signature = aggregate(keys, digest.as_ref());
}
/// Build the exact compact merge carrier without copying any storage representation.
fn make_carrier(keys: &[KeyPair], genesis: &SignedBlock, entry: &MergeLedgerEntry) -> SignedBlock {
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(genesis.hash()),
        None,
        None,
        100,
        0,
    ));
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(entry)),
    ));
    builder.build_with_signature(0, keys[0].private_key())
}
/// Offline archive only: public BlockStore owns every marker/index/hash/data byte.
/// The merge log framing matches CanonicalKuraEvidenceReader::scan_merge_entries;
/// source bundles intentionally remain opaque to this structural/crypto fixture.
fn write_archive(
    genesis: &SignedBlock,
    carrier: &SignedBlock,
    entry: &MergeLedgerEntry,
) -> Result<tempfile::TempDir> {
    use norito::codec::{Decode as _, Encode as _};
    use std::{
        fs,
        io::Write,
        os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    };
    let archive = tempfile::tempdir()?;
    let root = archive.path().canonicalize()?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let mut store = BlockStore::new(&root);
    store.create_files_if_they_do_not_exist()?;
    store.append_block_to_chain(genesis)?;
    store.append_block_to_chain(carrier)?;
    drop(store);
    for name in [
        "blocks.data",
        "blocks.index",
        "blocks.hashes",
        "blocks.count.norito",
    ] {
        let path = root.join(name);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        fs::File::open(path)?.sync_all()?;
    }
    let canonical = entry.canonical_bytes();
    ensure!(
        canonical.len() <= iroha_data_model::merge::MAX_MERGE_LEDGER_ENTRY_BYTES,
        "canonical merge size"
    );
    ensure!(
        norito::decode_canonical::<MergeLedgerEntry>(&canonical)? == *entry,
        "canonical merge roundtrip"
    );
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload = entry.encode();
    ensure!(
        !payload.is_empty()
            && payload.len() <= iroha_data_model::merge::MAX_MERGE_LEDGER_ENTRY_BYTES,
        "stored merge size"
    );
    let mut cursor = std::io::Cursor::new(payload.as_slice());
    let decoded = MergeLedgerEntry::decode(&mut cursor)?;
    ensure!(
        cursor.position() as usize == payload.len()
            && decoded == *entry
            && decoded.encode() == payload,
        "stored merge exact codec"
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join("merge.log"))?;
    file.write_all(&u32::try_from(payload.len())?.to_le_bytes())?;
    file.write_all(&payload)?;
    file.sync_all()?;
    fs::File::open(&root)?.sync_all()?;
    Ok(archive)
}
/// An independently signed two-height transcript stored through public Core writers.
/// It has no State execution or network-performance authority.
pub(super) struct Fixture {
    /// Keeps the physical temporary archive alive through every reader capability.
    _archive: tempfile::TempDir,
    pub keys: Vec<KeyPair>,
    pub network_id: NetworkId,
    pub genesis: SignedBlock,
    pub first: BridgeFinalityProof,
    pub carrier: SignedBlock,
    pub second: BridgeFinalityProof,
    pub entry: MergeLedgerEntry,
    pub requests: Vec<Request>,
}
/// Original independent offer order; queries are separately stored in merge-leaf order.
pub(super) struct Request {
    pub logical_id: String,
    pub signed: SignedTransaction,
    pub route: RoutingDecision,
    pub warmup: bool,
}
impl Fixture {
    /// Four warmup and four measured requests, with an optional authenticated rejection.
    pub fn new(lane_count: usize, rejected_logical_index: Option<usize>) -> Self {
        assert!(matches!(lane_count, 1 | 4));
        assert!(rejected_logical_index.is_none_or(|i| i < 8));
        let request_count = 8;
        let keys = keys();
        let genesis = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        ))
        .build_with_signature(0, keys[0].private_key());
        let network_id = NetworkId::from_genesis_hash(genesis.hash());
        let first = signed_proof(&keys, context(&keys, network_id), &genesis, None);
        let bindings: Vec<_> = (0..lane_count).map(binding).collect();
        let authorities =
            MergeLaneAuthorityCatalogV1::from_lane_committees(&vec![peers(&keys); lane_count])
                .unwrap();
        let mut requests = Vec::new();
        let mut groups = vec![Vec::new(); lane_count];
        for index in 0..request_count {
            let owner_key =
                KeyPair::try_from_seed(vec![80 + (index % 4) as u8; 32], Algorithm::Ed25519)
                    .unwrap();
            let authority = AccountId::new(owner_key.public_key().clone());
            let logical = format!("{index:064x}");
            let route = RoutingDecision::new(
                bindings[index % lane_count].lane_id,
                bindings[index % lane_count].dataspace_id,
            );
            let tx = TransactionBuilder::new(
                network_id,
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(SetKeyValue::account(
                    authority.clone(),
                    format!("gscale_{logical}").parse().unwrap(),
                    Json::try_new(logical.as_str()).unwrap(),
                ))]
                .into(),
            ))
            .sign(owner_key.private_key());
            groups[index % lane_count].push(TransactionEntrypoint::External(tx.clone()));
            requests.push(Request {
                logical_id: logical,
                signed: tx,
                route,
                warmup: index < 4,
            });
        }
        let header = BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(genesis.hash()),
            None,
            None,
            100,
            0,
        );
        let mut batch = MergeExecutionBatch {
            version: 1,
            base_state_height: 1,
            base_state_hash: genesis.hash(),
            application_block_header: header,
            lanes: bindings
                .iter()
                .zip(groups)
                .map(|(b, txs)| lane(&keys, network_id, b, txs))
                .collect(),
            entrypoint_count: 0,
            entrypoint_merkle_root: HashOf::from_untyped_unchecked(h("unset entry root")),
            result_merkle_root: HashOf::from_untyped_unchecked(h("unset result root")),
            execution_root: h("unset execution"),
            application_write_set_root: h("application writes"),
            write_set_root: h("full writes"),
            expected_post_state_hash: HashOf::from_untyped_unchecked(h("unset post state")),
            batch_hash: h("unset batch"),
        };
        if let Some(index) = rejected_logical_index {
            let target = requests[index].signed.hash_as_entrypoint();
            let lane = batch
                .lanes
                .iter_mut()
                .find(|lane| lane.entrypoints.iter().any(|tx| tx.hash() == target))
                .unwrap();
            let position = lane
                .entrypoints
                .iter()
                .position(|tx| tx.hash() == target)
                .unwrap();
            lane.results[position] = TransactionResult::from(Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "committed collection fixture rejection".into(),
                    ),
                ),
            ));
            lane.result_hashes[position] = Hash::from(lane.results[position].hash());
        }
        rehash_batch(&mut batch);
        let validators = peers(&keys);
        let mut entry = MergeLedgerEntry {
            version: 3,
            epoch_id: 1,
            lane_catalog_hash: h("independently pinned catalog"),
            incarnation_root: LaneLifecycleParameterV1::incarnation_root(
                &bindings
                    .iter()
                    .map(|b| LaneLifecycleIncarnationEntry {
                        lane_id: b.lane_id,
                        incarnation: b.incarnation,
                    })
                    .collect::<Vec<_>>(),
            ),
            activation_root: merge_activation_root(&bindings),
            active_lanes: bindings,
            lane_authority_catalog: authorities,
            lane_snapshots: Vec::new(),
            global_state_root: h("global reduction"),
            execution_batch: Some(batch),
            lane_drain_certificates: Vec::new(),
            merge_qc: MergeQuorumCertificate::new(
                0,
                1,
                2,
                genesis.hash(),
                network_id,
                1,
                HashOf::new(&validators),
                validators,
                vec![7],
                pops(&keys)
                    .into_iter()
                    .take(3)
                    .enumerate()
                    .map(|(signer, proof_of_possession)| MergeSignerProof {
                        signer: signer as u32,
                        proof_of_possession,
                    })
                    .collect(),
                Vec::new(),
                h("unset merge signature"),
            ),
        };
        sign_merge(&mut entry, &keys, network_id);
        let carrier = make_carrier(&keys, &genesis, &entry);
        let mut next = first.finality_artifact.height_context.clone();
        next.height = 2;
        next.parent_commit_qc = Some(first.finality_artifact.commit_qc.clone());
        let second = signed_proof(&keys, next, &carrier, Some(entry.canonical_hash()));
        let archive = write_archive(&genesis, &carrier, &entry)
            .expect("public offline block store and exact merge log");
        Self {
            _archive: archive,
            network_id,
            keys,
            genesis,
            first,
            carrier,
            second,
            entry,
            requests,
        }
    }
    pub fn queries(&self) -> Vec<CommittedTransaction> {
        let batch = self.entry.execution_batch.as_ref().unwrap();
        let entries: Vec<_> = batch
            .lanes
            .iter()
            .flat_map(|l| l.entrypoints.iter())
            .collect();
        let results: Vec<_> = batch.lanes.iter().flat_map(|l| l.results.iter()).collect();
        let et: MerkleTree<TransactionEntrypoint> = entries.iter().map(|e| e.hash()).collect();
        let rt: MerkleTree<TransactionResult> = results.iter().map(|r| r.hash()).collect();
        entries
            .iter()
            .zip(results)
            .enumerate()
            .map(|(i, (e, r))| CommittedTransaction {
                block_hash: self.carrier.hash(),
                entrypoint_hash: e.hash(),
                entrypoint_proof: et.get_proof(i as u32).unwrap(),
                entrypoint: (*e).clone(),
                result_hash: r.hash(),
                result_proof: rt.get_proof(i as u32).unwrap(),
                result: r.clone(),
                merge_inclusion: Some(CertifiedMergeTransactionInclusion {
                    version: 1,
                    merge_entry_hash: self.entry.canonical_hash(),
                    merge_epoch_id: self.entry.epoch_id,
                    execution_batch_hash: batch.batch_hash,
                    entrypoint_count: batch.entrypoint_count,
                    entrypoint_merkle_root: batch.entrypoint_merkle_root,
                    result_merkle_root: batch.result_merkle_root,
                }),
            })
            .collect()
    }
    /// Independently selected config for command and injected SDK admission.
    pub fn config(&self) -> iroha::config::Config {
        use iroha_service_model::{
            sorafs::*,
            soranet::{AnonymityPolicy, RolloutPhase},
        };
        use std::time::Duration;
        let key_pair = KeyPair::try_from_seed(vec![80; 32], Algorithm::Ed25519).unwrap();
        iroha::config::Config {
            chain: iroha_model_base::chain::ChainId::from("00000000-0000-0000-0000-000000000001"),
            network_id: self.network_id,
            account: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            torii_api_url: "http://127.0.0.1:1/".parse().unwrap(),
            torii_request_timeout: Duration::from_secs(2),
            basic_auth: None,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(2),
            transaction_add_nonce: false,
            sorafs_alias_cache: sorafs_manifest::alias_cache::AliasCachePolicy::new(
                Duration::from_secs(DEFAULT_ALIAS_POSITIVE_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_REFRESH_WINDOW_SECS),
                Duration::from_secs(DEFAULT_ALIAS_HARD_EXPIRY_SECS),
                Duration::from_secs(DEFAULT_ALIAS_NEGATIVE_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_REVOCATION_TTL_SECS),
                Duration::from_secs(DEFAULT_ALIAS_ROTATION_MAX_AGE_SECS),
                Duration::from_secs(DEFAULT_ALIAS_SUCCESSOR_GRACE_SECS),
                Duration::from_secs(DEFAULT_ALIAS_GOVERNANCE_GRACE_SECS),
            ),
            sorafs_anonymity_policy: AnonymityPolicy::GuardPq,
            sorafs_rollout_phase: RolloutPhase::Canary,
        }
    }
    /// Validated SDK context using one of the exact independently signed request keys.
    pub fn client(&self, transport: Arc<dyn HttpTransport>) -> iroha::client::Client {
        iroha::client::Client::builder(self.config())
            .http_transport(transport)
            .build()
            .unwrap()
    }
    /// Valid signatures and exact header, but a deliberately incorrect executed-wire tuple.
    pub fn second_with_wrong_executed_wire(&self) -> BridgeFinalityProof {
        let mut proof = self.second.clone();
        let qc = &mut proof.finality_artifact.commit_qc;
        qc.execution_commitment.executed_block_wire_hash = h("wrong but signed executed wire");
        qc.execution_commitment.executed_block_wire_len += 1;
        let vote = Vote {
            round: qc.round,
            proposal_round: qc.proposal_round,
            phase: qc.phase,
            subject: qc.subject,
            execution_commitment: qc.execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        qc.aggregate_signature = aggregate(&self.keys, &vote.signature_preimage());
        proof.finality_artifact.verify().unwrap();
        proof
            .finality_artifact
            .validate_for_header(&self.carrier.header())
            .unwrap();
        proof
    }
    /// Rebuild a bounded nonempty malformed transcript through the public offline block writer.
    /// The test-local log writer permits semantic mutants; errors leave this fixture unchanged.
    pub fn rewrite_entry_for_test(
        &mut self,
        mutate: impl FnOnce(&mut MergeLedgerEntry),
    ) -> Result<()> {
        let mut entry = self.entry.clone();
        mutate(&mut entry);
        let batch = entry
            .execution_batch
            .as_mut()
            .ok_or_else(|| eyre!("fixture requires a batch"))?;
        ensure!((1..=4).contains(&batch.lanes.len()), "fixture lane bound");
        ensure!(
            batch.lanes.iter().all(|lane| !lane.entrypoints.is_empty()
                && lane.entrypoints.len() <= 8
                && !lane.results.is_empty()
                && lane.results.len() <= 8),
            "fixture leaf bound"
        );
        rehash_batch(batch);
        sign_merge(&mut entry, &self.keys, self.network_id);
        let carrier = make_carrier(&self.keys, &self.genesis, &entry);
        let mut next = self.first.finality_artifact.height_context.clone();
        next.height = 2;
        next.parent_commit_qc = Some(self.first.finality_artifact.commit_qc.clone());
        let second = signed_proof(&self.keys, next, &carrier, Some(entry.canonical_hash()));
        let archive = write_archive(&self.genesis, &carrier, &entry)?;
        self.entry = entry;
        self.carrier = carrier;
        self.second = second;
        self._archive = archive;
        Ok(())
    }
    /// Actual canonicalized offline archive paths retained through the fixture lifetime.
    pub fn paths(&self) -> (PathBuf, PathBuf) {
        let root = self._archive.path().canonicalize().unwrap();
        (root.clone(), root.join("merge.log"))
    }
    /// Independently fixed reader budget for the complete two-height archive.
    pub fn limits(&self) -> CanonicalKuraEvidenceLimits {
        CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: 2,
            max_committed_blocks: 2,
            max_store_data_bytes: 4 * 1024 * 1024,
            max_carrier_bytes: 1024 * 1024,
            max_merge_log_bytes: 8 * 1024 * 1024,
            max_merge_frames: 2,
            max_output_bytes: 16 * 1024 * 1024,
            max_decode_allocation_bytes: 64 * 1024 * 1024,
            owner_uid: rustix::process::getuid().as_raw(),
        }
    }
    pub fn open_reader(&self) -> CanonicalKuraEvidenceReader {
        let (blocks, merge) = self.paths();
        CanonicalKuraEvidenceReader::open(&blocks, &merge, self.limits())
            .expect("real archive admission")
    }
    /// Retain the returned capability and `self` through the collection/publication check.
    pub fn complete(&self) -> CanonicalKuraEvidenceComplete {
        let mut reader = self.open_reader();
        assert_eq!(
            reader.read_carrier(1).unwrap(),
            self.genesis.encode_wire().unwrap()
        );
        assert_eq!(
            reader.read_carrier(2).unwrap(),
            self.carrier.encode_wire().unwrap()
        );
        let mut seen = 0;
        reader
            .scan_merge_entries(
                &[CanonicalKuraMergeRequest {
                    carrier_height: 2,
                    reference: CertifiedMergeLedgerReference::new(&self.entry),
                }],
                |height, entry, canonical| {
                    assert_eq!(height, 2);
                    assert_eq!(entry, &self.entry);
                    assert_eq!(canonical, self.entry.canonical_bytes().as_slice());
                    seen += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(seen, 1);
        reader
            .finish()
            .expect("all carriers and the entire merge log consumed")
    }
}

/// An exact ordered SDK response, selected by the test before Client admission.
#[derive(Debug)]
pub(super) struct Reply {
    pub method: Method,
    pub path: String,
    pub media_type: &'static str,
    /// Expected signed request selection, retained separately from mutable response bytes.
    pub expected_query_hash: Option<HashOf<TransactionEntrypoint>>,
    pub body: Vec<u8>,
}
impl Reply {
    /// Exercise the public compatibility handshake using the actual nominal schema.
    pub fn capabilities() -> Self {
        Self {
            method: Method::GET, path: "/v1/node/capabilities".into(), media_type: "application/json", expected_query_hash: None,
            body: norito::json::to_vec(&norito::json!({
                "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
                "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>())),
            })).unwrap(),
        }
    }
    pub fn finality(proof: &BridgeFinalityProof) -> Self {
        Self {
            method: Method::GET,
            path: iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
                .path()
                .replace("{height}", &proof.block_header.height().get().to_string()),
            media_type: "application/x-norito",
            expected_query_hash: None,
            body: norito::encode_canonical(proof).unwrap(),
        }
    }
    pub fn details(transaction: CommittedTransaction) -> Self {
        Self {
            method: Method::POST,
            path: iroha_torii_shared::uri::TRANSACTION_DETAILS.into(),
            media_type: "application/x-norito",
            expected_query_hash: Some(transaction.entrypoint_hash),
            body: norito::encode_canonical(
                &iroha_torii_shared::PipelineTransactionDetailsResponse {
                    hash: transaction.entrypoint_hash.to_string(),
                    transaction,
                    trigger_completions: Vec::new(),
                },
            )
            .unwrap(),
        }
    }
}
/// Decode and authenticate the exact SDK query, including its native payload selectors.
fn validate_details_request(
    request: &TransportRequest,
    expected: HashOf<TransactionEntrypoint>,
) -> Result<()> {
    use iroha_data_model::query::{
        CommittedTxFilters, Query, QueryRequest, SignedQuery,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::QueryParams,
        transaction::prelude::FindTransactions,
    };
    use iroha_version::codec::DecodeVersioned as _;
    use norito::codec::Decode as _;
    ensure!(
        request.url.query().is_none(),
        "unexpected details URL query"
    );
    ensure!(
        request.max_response_bytes == 64 * 1024 * 1024,
        "details response cap"
    );
    for name in ["accept", "content-type"] {
        let values = request
            .headers
            .iter()
            .filter(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
            .collect::<Vec<_>>();
        ensure!(
            values.len() == 1 && values[0].1.to_str()? == "application/x-norito",
            "details representation header"
        );
    }
    let signed = SignedQuery::decode_all_versioned(&request.body)?;
    signed.verify_signature()?;
    let QueryRequest::Start(query) = signed.request() else {
        return Err(eyre!("details query is not Start"));
    };
    let find = FindTransactions::new();
    ensure!(
        query.query_payload == find.dyn_encode()
            && query.item == find.query_item_kind()
            && query.params == QueryParams::default(),
        "exact details query owner"
    );
    let mut cursor = std::io::Cursor::new(query.predicate_bytes.as_slice());
    let predicate = CompoundPredicate::<CommittedTransaction>::decode(&mut cursor)?;
    ensure!(
        cursor.position() as usize == query.predicate_bytes.len(),
        "predicate trailing bytes"
    );
    ensure!(
        predicate.committed_tx_filters()
            == Some(CommittedTxFilters {
                entry_eq: Some(expected),
                ..CommittedTxFilters::default()
            }),
        "exact queried entrypoint hash"
    );
    let mut cursor = std::io::Cursor::new(query.selector_bytes.as_slice());
    let selector = SelectorTuple::<CommittedTransaction>::decode(&mut cursor)?;
    ensure!(
        cursor.position() as usize == query.selector_bytes.len()
            && selector == SelectorTuple::<CommittedTransaction>::default(),
        "exact details selector"
    );
    Ok(())
}
/// Public HTTP injection with no sockets, retries, fallback responses or test-only SDK API.
#[derive(Debug)]
pub(super) struct ScriptedTransport {
    replies: Mutex<VecDeque<Reply>>,
    consumed: Mutex<usize>,
}
impl ScriptedTransport {
    pub fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            consumed: Mutex::new(0),
        })
    }
    pub fn consumed(&self) -> usize {
        *self.consumed.lock().unwrap()
    }
    pub fn assert_drained(&self, expected: usize) {
        assert!(self.replies.lock().unwrap().is_empty());
        assert_eq!(*self.consumed.lock().unwrap(), expected);
    }
}
impl HttpTransport for ScriptedTransport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| eyre!("unexpected SDK request"))?;
        *self.consumed.lock().unwrap() += 1;
        ensure!(
            request.method == reply.method,
            "unexpected SDK request method"
        );
        ensure!(
            request.url.path() == reply.path,
            "unexpected SDK request path"
        );
        if let Some(expected) = reply.expected_query_hash {
            validate_details_request(&request, expected)?;
        }
        ensure!(
            reply.body.len() <= request.max_response_bytes,
            "SDK response byte cap"
        );
        Ok(Response::builder()
            .status(200)
            .header("content-type", reply.media_type)
            .body(reply.body)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}

#[cfg(test)]
mod tests;
