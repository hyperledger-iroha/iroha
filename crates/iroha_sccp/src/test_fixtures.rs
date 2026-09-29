//! Deterministic finalized-block fixtures for crate and downstream integration tests.
//!
//! The fixtures sign complete blocks with a fixed four-validator test roster and exact
//! Sumeragi-v2 finality. They are compiled only for crate tests or with the `test-fixtures`
//! feature and carry no SCCP message content.
//!
//! TODO(ws70): move the generic finalized-block fixture out of `iroha_sccp` once the `SoraFS`
//! sessions land.
use halo2curves::{
    CurveAffine,
    group::{Curve, GroupEncoding},
    pasta::{Fp as PastaFp, Fq as PastaFq, PallasAffine, VestaAffine},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, MerkleTree, Signature};
#[cfg(test)]
use iroha_data_model::block::{BlockHeader, output_budget::ExecutionOutputLimits};
use iroha_data_model::{
    block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
    },
    block::{SignedBlock, execution_output::ExecutionOutputV1},
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof},
    transaction::TransactionEntrypoint,
};
use iroha_model_base::peer::PeerId;

/// Genesis-derived TAIRA network identity bound into the finalized-block fixtures.
const SCCP_TAIRA_FINALITY_NETWORK_ID_V1: &str =
    "hash:0466DA18C70CA8CBD51B8CC60B1D4A4802FC5D7F928D505806D7CD6CB61D60EF#BA85";
/// Return the fixed TAIRA network identity used by the finalized-block fixtures.
fn sccp_taira_finality_network_id_v1() -> iroha_data_model::NetworkId {
    SCCP_TAIRA_FINALITY_NETWORK_ID_V1
        .parse()
        .expect("fixture Taira network identity must be canonical")
}

// These finite limits belong only to bounded public test fixtures, never runtime policy.
#[cfg(test)]
fn exact_fixture_output_limits() -> ExecutionOutputLimits {
    ExecutionOutputLimits {
        max_outputs: 64,
        max_output_bytes: 1024 * 1024,
        max_total_output_bytes: 4 * 1024 * 1024,
        max_executed_wire_bytes: 8 * 1024 * 1024,
    }
}

/// Complete block plus finality produced only through the exact test signer.
///
/// Private fields keep the parent invariant closed: a same-epoch successor can
/// inherit only a parent `CommitQC` already bound to both its canonical resultless
/// proposal and complete result-bearing block wire images by this module.
#[derive(Clone, Debug)]
pub struct SccpFinalizedBlockTestFixtureV1 {
    block: SignedBlock,
    proof: BridgeFinalityProof,
}

fn sccp_mint_finality_genesis_test_fixture_v1(
    network_id: iroha_data_model::NetworkId,
    roster: &[ValidatorPower],
    last_height: u64,
) -> (
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
) {
    use iroha_data_model::isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationTemplateV1,
        KagemushaMintFinalityEpochAuthorizationV1, KagemushaMintFinalityGenesisParametersV1,
        KagemushaMintFinalityValidatorKeysV1,
    };

    let genesis = KagemushaMintFinalityGenesisParametersV1 {
        authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            generation: 0,
            validators: roster
                .iter()
                .enumerate()
                .map(|(index, validator)| {
                    let scalar = u64::try_from(index + 1).expect("small SCCP fixture roster");
                    let pallas_encoded = (<PallasAffine as CurveAffine>::CurveExt::generator()
                        * PastaFq::from(scalar))
                    .to_affine()
                    .to_bytes();
                    let vesta_encoded = (<VestaAffine as CurveAffine>::CurveExt::generator()
                        * PastaFp::from(scalar))
                    .to_affine()
                    .to_bytes();
                    let mut pallas_key = [0_u8; 32];
                    pallas_key.copy_from_slice(pallas_encoded.as_ref());
                    let mut vesta_key = [0_u8; 32];
                    vesta_key.copy_from_slice(vesta_encoded.as_ref());
                    KagemushaMintFinalityValidatorKeysV1 {
                        validator: validator.validator.clone(),
                        eq_proof_public_key: pallas_key,
                        ep_proof_public_key: vesta_key,
                    }
                })
                .collect(),
        },
    };
    genesis
        .validate()
        .expect("valid generation-zero SCCP mint-finality template");
    let authority = genesis
        .authority_generation
        .bind_network_id(network_id)
        .expect("bind SCCP mint-finality authority to exact genesis network");
    let authorization = KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, last_height)
        .expect("canonical SCCP genesis scheduling authorization");
    (authorization, authority)
}

impl SccpFinalizedBlockTestFixtureV1 {
    /// Return the complete signed block authenticated by this fixture.
    #[must_use]
    pub const fn block(&self) -> &SignedBlock {
        &self.block
    }
    /// Return the exact finality proof bound to the proposal and executed wire images.
    #[must_use]
    pub const fn proof(&self) -> &BridgeFinalityProof {
        &self.proof
    }
}
fn exact_fixture_proposal_wire_hash(block: &SignedBlock) -> Hash {
    block
        .canonical_proposal_wire_hash()
        .expect("exact SCCP fixture proposal has canonical wire bytes")
}
fn exact_fixture_executed_wire_hash(block: &SignedBlock) -> Hash {
    block
        .executed_block_wire_hash()
        .expect("exact SCCP fixture executed block has canonical wire bytes")
}
fn exact_fixture_executed_wire_len(block: &SignedBlock) -> u64 {
    u64::try_from(
        block
            .encode_wire()
            .expect("exact SCCP fixture executed block has canonical wire bytes")
            .len(),
    )
    .expect("exact SCCP fixture executed block wire length fits u64")
}
fn assert_exact_finalized_block_fixture(fixture: &SccpFinalizedBlockTestFixtureV1) {
    assert_eq!(fixture.proof.block_header, fixture.block.header());
    assert_eq!(
        fixture.proof.finality_artifact.block_hash,
        fixture.block.hash()
    );
    assert_eq!(
        fixture.proof.finality_artifact.subject.payload_hash,
        exact_fixture_proposal_wire_hash(&fixture.block),
        "the finality subject must bind the canonical resultless proposal wire image"
    );
    assert_eq!(
        fixture
            .proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_len,
        exact_fixture_executed_wire_len(&fixture.block),
        "the execution commitment must bind the complete result-bearing block wire length"
    );
    assert_eq!(
        fixture
            .proof
            .finality_artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash,
        exact_fixture_executed_wire_hash(&fixture.block),
        "the execution commitment must bind the complete result-bearing block wire image"
    );
    fixture
        .proof
        .finality_artifact
        .validate_for_header(&fixture.block.header())
        .expect("exact SCCP fixture finality binds its complete block header");
    fixture
        .proof
        .finality_artifact
        .verify()
        .expect("exact SCCP fixture finality is cryptographically valid");
}
fn assert_exact_fixture_block_body(block: &SignedBlock) {
    assert!(
        block.has_results(),
        "an exact finalized block fixture must carry its complete execution results"
    );
    let external_entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
    let external_root = external_entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<MerkleTree<TransactionEntrypoint>>()
        .root();
    assert_eq!(
        block.header().merkle_root(),
        external_root,
        "the finalized header must commit the exact external entrypoint order"
    );
    block
        .validate_output_merkle_cache()
        .expect("the full typed outputs, source joins and retained cache must be canonical");
    let network = block.network_entrypoints().collect::<Vec<_>>();
    assert_eq!(
        block.network_input_hashes().collect::<Vec<_>>(),
        network
            .iter()
            .map(|entrypoint| entrypoint.hash())
            .collect::<Vec<_>>(),
        "the complete Network input tree must match actual sources"
    );
    let output_hashes = block
        .execution_outputs()
        .iter()
        .map(iroha_crypto::HashOf::new)
        .collect::<Vec<_>>();
    assert_eq!(block.output_hashes().collect::<Vec<_>>(), output_hashes);
    assert_eq!(
        block.output_merkle_commitment(),
        output_hashes
            .into_iter()
            .collect::<MerkleTree<ExecutionOutputV1>>()
            .commitment(),
        "the retained output tree must match all actual typed outputs, including internal rows"
    );
    assert_eq!(
        block
            .execution_outputs()
            .iter()
            .filter(|row| matches!(row, ExecutionOutputV1::Network(_)))
            .count(),
        network.len(),
        "each Network source has exactly one explicitly joined Network output"
    );
    for (input_index, _) in network.into_iter().enumerate() {
        let (output_index, output) = block
            .network_output_at(u32::try_from(input_index).expect("fixture input index"))
            .expect("exact Network source/output join");
        assert_eq!(output.input_index as usize, input_index);
        assert_eq!(
            &block.execution_outputs()[output_index as usize],
            &ExecutionOutputV1::Network(output.clone())
        );
    }
}
/// Finalize a complete block with the test-only Taira roster.
///
/// This helper is available only to crate tests or consumers of the existing
/// `test-fixtures` feature. It provides no caller-selected signing material and
/// must not be used by production release tooling. The returned opaque parent
/// type proves that a successor reuses the exact `CommitQC` of a proof already
/// bound to both canonical proposal and result-bearing signed-block wire images.
///
/// # Panics
///
/// Panics if `block` is outside heights 1 through 9 of the fixed fixture window, has an
/// invalid or non-exact parent, has malformed source/output joins or a stale output cache,
/// or cannot be bound to a cryptographically valid artifact.
#[must_use]
pub fn sccp_finalize_taira_block_test_fixture_v1(
    block: &SignedBlock,
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
) -> SccpFinalizedBlockTestFixtureV1 {
    sccp_finalize_taira_block_with_epoch_schedule_test_fixture_v1(
        block,
        parent,
        SccpFinalityFixtureEpochSchedule::Ordinary,
    )
}

/// Finalize one exact native-operation test block in a bounded 255-height epoch.
///
/// The four-validator roster, canonical source/output checks, three-vote `CommitQC` and signed
/// RS16 layout are identical to the short SCCP fixture. This separate schedule supports the
/// sequential Reserve/Check/Complete rounds of native services without changing bridge fixtures.
///
/// # Panics
/// Panics outside heights 1 through 255, on a wrong parent or epoch schedule, or malformed wire.
#[must_use]
pub fn sccp_finalize_taira_native_operation_block_test_fixture_v1(
    block: &SignedBlock,
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
) -> SccpFinalizedBlockTestFixtureV1 {
    sccp_finalize_taira_block_with_epoch_schedule_test_fixture_v1(
        block,
        parent,
        SccpFinalityFixtureEpochSchedule::NativeOperations,
    )
}

#[cfg(test)]
fn sccp_finalize_taira_epoch_boundary_test_fixture_v1(
    block: &SignedBlock,
) -> SccpFinalizedBlockTestFixtureV1 {
    sccp_finalize_taira_block_with_epoch_schedule_test_fixture_v1(
        block,
        None,
        SccpFinalityFixtureEpochSchedule::GenesisBoundary,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SccpFinalityFixtureEpochSchedule {
    Ordinary,
    NativeOperations,
    GenesisBoundary,
}

#[expect(
    clippy::too_many_lines,
    reason = "the test-only signer keeps the ordered block, roster, context, QC, and aggregate-binding checks cohesive"
)]
fn sccp_finalize_taira_block_with_epoch_schedule_test_fixture_v1(
    block: &SignedBlock,
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
    epoch_schedule: SccpFinalityFixtureEpochSchedule,
) -> SccpFinalizedBlockTestFixtureV1 {
    let block_header = block.header();
    let height = block_header.height().get();
    assert!(
        match epoch_schedule {
            SccpFinalityFixtureEpochSchedule::Ordinary => (1..=9).contains(&height),
            SccpFinalityFixtureEpochSchedule::NativeOperations => (1..=255).contains(&height),
            SccpFinalityFixtureEpochSchedule::GenesisBoundary => height == 1,
        },
        "the exact SCCP finality signer received a height outside its selected epoch schedule"
    );
    assert_exact_fixture_block_body(block);
    let mut keypairs = [
        KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal).expect("BLS fixture key 1"),
        KeyPair::try_from_seed(vec![2; 32], Algorithm::BlsNormal).expect("BLS fixture key 2"),
        KeyPair::try_from_seed(vec![3; 32], Algorithm::BlsNormal).expect("BLS fixture key 3"),
        KeyPair::try_from_seed(vec![4; 32], Algorithm::BlsNormal).expect("BLS fixture key 4"),
    ];
    keypairs.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = keypairs
        .iter()
        .zip([1_u64; 4])
        .map(|(keypair, power)| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();
    let quorum = DualQuorum::from_roster(&roster).expect("valid SCCP fixture roster");
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            iroha_crypto::bls_normal_pop_prove(keypair.private_key()).expect("BLS fixture PoP")
        })
        .collect::<Vec<_>>();
    let da_layout = DataAvailabilityLayout {
        encoding: PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 1024,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 4096,
        max_chunk_count: 8,
    };
    let network_id = sccp_taira_finality_network_id_v1();
    let context = match (height, block_header.prev_block_hash(), parent) {
        (1, None, None) => {
            use iroha_data_model::isi::kagemusha_v1::{
                BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
                KagemushaMintFinalityEpochAuthorizationV1, KagemushaMintFinalityEpochDecisionV1,
            };

            let first_epoch_end = match epoch_schedule {
                SccpFinalityFixtureEpochSchedule::Ordinary => 10,
                SccpFinalityFixtureEpochSchedule::NativeOperations => 256,
                SccpFinalityFixtureEpochSchedule::GenesisBoundary => 1,
            };
            let (authorization, authority) =
                sccp_mint_finality_genesis_test_fixture_v1(network_id, &roster, first_epoch_end);
            let next_epoch_snapshot =
                (epoch_schedule == SccpFinalityFixtureEpochSchedule::GenesisBoundary).then(|| {
                    let successor = KagemushaMintFinalityEpochAuthorizationV1 {
                        version: authorization.version,
                        network_id,
                        epoch: 1,
                        first_height: 2,
                        last_height: 10,
                        authority_generation: authority.generation,
                        authority_id: authorization.authority_id,
                        beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                            session_id: [0xC4; 32],
                            transcript_hash: [0xC5; 32],
                        }),
                        previous_authorization_id: authorization
                            .authorization_id()
                            .expect("exact SCCP predecessor authorization identity"),
                        transition_id: [0; 32],
                        decision: KagemushaMintFinalityEpochDecisionV1::Retain,
                    };
                    successor
                        .validate_against_authority(&authority)
                        .expect("exact SCCP epoch-one authorization binds its authority");
                    successor
                        .validate_successor(&authorization)
                        .expect("exact SCCP epoch-one authorization is contiguous");
                    iroha_data_model::block::consensus_v2::finality::FinalizedNextEpochSnapshot {
                        committee_preparation: None,
                        epoch: successor.epoch,
                        kagemusha_mint_finality_authorization: successor,
                        kagemusha_mint_finality_authority: authority.clone(),
                        epoch_end_height: successor.last_height,
                        mode: ConsensusMode::Npos,
                        roster: roster.clone(),
                        validator_set_pops: validator_set_pops.clone(),
                        quorum,
                        leader_seed: [0x5a; 32],
                    }
                });
            HeightContext {
                network_id,
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch: authorization.epoch,
                epoch_end_height: authorization.last_height,
                next_epoch_snapshot,
                mode: ConsensusMode::Npos,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum,
                roster,
                nexus_amx_context_hash: Hash::new(b"exact SCCP fixture Nexus/AMX context"),
                execution_policy_hash: Hash::new(b"exact SCCP fixture execution policy"),
                da_layout,
                leader_seed: [0x5a; 32],
                kagemusha_mint_finality_authorization: authorization,
                kagemusha_mint_finality_authority: authority,
            }
        }
        (2..=255, Some(parent_hash), Some(parent)) => {
            assert_exact_finalized_block_fixture(parent);
            assert_eq!(
                parent.block().header().height().get().checked_add(1),
                Some(height)
            );
            assert_eq!(parent_hash, parent.block().hash());
            assert_eq!(
                parent.proof().finality_artifact.height.checked_add(1),
                Some(height)
            );
            let parent_context = &parent.proof().finality_artifact.height_context;
            if epoch_schedule == SccpFinalityFixtureEpochSchedule::NativeOperations {
                assert_eq!(
                    parent_context.epoch_end_height, 256,
                    "native operation fixture cannot switch epoch schedules"
                );
            }
            let (
                epoch,
                epoch_end_height,
                mode,
                roster,
                quorum,
                leader_seed,
                authorization,
                authority,
            ) = parent_context.next_epoch_snapshot.as_ref().map_or_else(
                || {
                    assert!(height < parent_context.epoch_end_height);
                    (
                        parent_context.epoch,
                        parent_context.epoch_end_height,
                        parent_context.mode,
                        parent_context.roster.clone(),
                        parent_context.quorum,
                        parent_context.leader_seed,
                        parent_context.kagemusha_mint_finality_authorization,
                        parent_context.kagemusha_mint_finality_authority.clone(),
                    )
                },
                |next| {
                    assert_eq!(
                        parent_context.epoch_end_height.checked_add(1),
                        Some(height),
                        "the certified epoch snapshot belongs to this exact successor"
                    );
                    assert_eq!(next.validator_set_pops, validator_set_pops);
                    (
                        next.epoch,
                        next.epoch_end_height,
                        next.mode,
                        next.roster.clone(),
                        next.quorum,
                        next.leader_seed,
                        next.kagemusha_mint_finality_authorization,
                        next.kagemusha_mint_finality_authority.clone(),
                    )
                },
            );
            HeightContext {
                network_id: parent_context.network_id,
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch,
                epoch_end_height,
                next_epoch_snapshot: None,
                mode,
                parent_commit_qc: Some(parent.proof().finality_artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                quorum,
                roster,
                nexus_amx_context_hash: parent_context.nexus_amx_context_hash,
                execution_policy_hash: parent_context.execution_policy_hash,
                da_layout: parent_context.da_layout,
                leader_seed,
                kagemusha_mint_finality_authorization: authorization,
                kagemusha_mint_finality_authority: authority,
            }
        }
        _ => panic!(
            "height one requires no parent and every successor requires its exact complete parent"
        ),
    };
    let subject = BlockSubject {
        parent_block_hash: block_header.prev_block_hash(),
        block_hash: block_header.hash(),
        payload_hash: exact_fixture_proposal_wire_hash(block),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        // The finality artifact duplicates the finalized header's
        // view-change index and must bind it exactly.
        view: block_header.view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"exact SCCP fixture parent state"),
            Hash::new(b"exact SCCP fixture post state"),
            Hash::new(b"exact SCCP fixture ordinary writes"),
            exact_fixture_executed_wire_len(block),
            exact_fixture_executed_wire_hash(block),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let message = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid exact Sumeragi-v2 commit certificate");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("fixture signer index fits usize");
            Signature::try_new(keypairs[index].private_key(), &message)
                .expect("BLS fixture commit vote")
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures
        .iter()
        .map(Signature::payload)
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate BLS fixture votes");
    let finality_artifact =
        V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    finality_artifact
        .validate_for_header(&block_header)
        .expect("exact SCCP finality artifact binds the supplied block header");
    finality_artifact
        .verify()
        .expect("exact SCCP finality artifact is cryptographically valid");
    let finalized = SccpFinalizedBlockTestFixtureV1 {
        block: block.clone(),
        proof: BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header,
            finality_artifact,
        },
    };
    assert_exact_finalized_block_fixture(&finalized);
    finalized
}

#[cfg(test)]
mod finality_descendant_tests;
