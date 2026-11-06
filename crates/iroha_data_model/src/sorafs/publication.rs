//! Publication evidence anchored in an independently selected native finality checkpoint.
//!
//! A proof carries quorum certificates and Merkle paths only, never a block body, so any
//! publisher can authenticate its own assertion without reading other dataspaces.

use super::pin_registry::{PinManifestRecord, ReplicationOrderRecord};
use crate::{
    NetworkId,
    isi::sorafs::AssertSorafsPublicationV1,
    sumeragi_finality::{
        CertifiedTransactionInclusionV1, SumeragiCommitCertificateV1, SumeragiCommitVerifierV1,
        SumeragiFinalityCheckpoint, SumeragiFinalityVerifier,
    },
    transaction::{Executable, SignedTransaction},
};
use norito::{Decode, Encode};

/// Maximum canonical publication response.
pub const PUBLICATION_PROOF_MAX_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum validator-epoch boundaries one publication response may cross from the client's
/// trusted checkpoint to the block that executed the assertion.
pub const PUBLICATION_PROOF_MAX_EPOCH_BOUNDARIES_V1: usize = 256;

/// Bounded request for metadata or one chunk under an exact live native replication assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsAssignedSourceRequestV1")]
pub struct SorafsAssignedSourceRequestV1 {
    /// Assigned provider whose owner authenticates the request.
    pub target_provider: [u8; 32],
    /// Admitted provider expected to serve its verified stored replica.
    pub source_provider: [u8; 32],
    /// Exact live replication order.
    pub order_id: [u8; 32],
    /// Exact assignment revision retained by the worker.
    pub assignment_revision: u64,
    /// Exact approved manifest.
    pub manifest_digest: [u8; 32],
    /// None requests canonical source metadata; Some requests one indexed chunk.
    pub chunk_index: Option<u32>,
    /// Independently retained finalized worker floor.
    pub floor_height: u64,
    /// Exact finalized worker floor hash.
    pub floor_block_hash: [u8; 32],
}

/// Informational preparation read; it becomes evidence only after the signed assertion succeeds.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationPreparationV1")]
pub struct SorafsPublicationPreparationV1 {
    /// Exact currently approved pin and its paid registration record.
    pub pin: PinManifestRecord,
    /// Exact automatic replication assignment and completion records.
    pub order: ReplicationOrderRecord,
}

/// Selector for native publication execution evidence, authenticated as an account request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationProofRequestV1")]
pub struct SorafsPublicationProofRequestV1 {
    /// Exact signed assertion entrypoint hash independently retained by the client.
    pub entry_hash: [u8; 32],
    /// Independently trusted checkpoint height.
    pub floor_height: u64,
    /// Independently trusted checkpoint block hash.
    pub floor_block_hash: [u8; 32],
}

/// Untrusted canonical carrier for a challenged native publication assertion.
///
/// Every component is DATA until [`verify_sorafs_publication_v1`] authenticates it under the
/// client's own checkpoint: no response can supply its trust root or an epoch authority.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::publication::SorafsPublicationProofV1")]
pub struct SorafsPublicationProofV1 {
    /// Incumbent epoch-boundary certificates after the epoch of the client's checkpoint, in
    /// height order, up to the epoch that executed the assertion.
    pub epoch_boundaries: Vec<SumeragiCommitCertificateV1>,
    /// Commit certificate of the block that executed the assertion.
    pub certificate: SumeragiCommitCertificateV1,
    /// Membership of the exact signed assertion and its joined execution output.
    pub inclusion: CertifiedTransactionInclusionV1,
}

/// Opaque successful verification of the client's original challenged assertion.
#[derive(Debug)]
#[expect(
    missing_copy_implementations,
    reason = "verifier-issued publication evidence retains explicit ownership"
)]
pub struct VerifiedSorafsPublicationV1 {
    height: u64,
    completed: bool,
}
impl VerifiedSorafsPublicationV1 {
    /// Height of the quorum-certified block that executed the assertion.
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Whether the proven instruction required every assigned provider's finalized completion.
    pub const fn completed(&self) -> bool {
        self.completed
    }
}

/// Fixed publication proof rejection; raw state reads and HTTP success cannot create evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native SoraFS publication proof rejected")]
pub struct SorafsPublicationProofErrorV1;

/// Verify the exact signed assertion against independently pinned network and predecessor state.
///
/// `checkpoint` must come from an independently authenticated local selection, such as one
/// derived from the signed genesis. The caller must first compare its chain label with the
/// configured chain using `SumeragiFinalityVerifier::from_trusted_checkpoint`. A proof response
/// cannot supply its own trust root or epoch authority: every epoch boundary is verified under
/// the incumbent quorum that `checkpoint` selects. The caller must start a monotonic deadline
/// before generating its challenge and reject verification after that deadline.
///
/// # Errors
/// Rejects a proof that fails finality, binding or freshness verification.
pub fn verify_sorafs_publication_v1(
    network: &NetworkId,
    checkpoint: &SumeragiFinalityCheckpoint,
    expected: &SignedTransaction,
    proof: &SorafsPublicationProofV1,
) -> Result<VerifiedSorafsPublicationV1, SorafsPublicationProofErrorV1> {
    let rejected = SorafsPublicationProofErrorV1;
    if proof.epoch_boundaries.len() > PUBLICATION_PROOF_MAX_EPOCH_BOUNDARIES_V1
        || norito::canonical_frame_len(proof).map_err(|_| rejected)?
            > PUBLICATION_PROOF_MAX_BYTES_V1
        || expected.network_id() != Some(network)
    {
        return Err(rejected);
    }
    let Executable::Instructions(instructions) = expected.instructions() else {
        return Err(rejected);
    };
    if instructions.len() != 1 {
        return Err(rejected);
    }
    let assertion = instructions[0]
        .as_any()
        .downcast_ref::<AssertSorafsPublicationV1>()
        .ok_or(rejected)?;
    if assertion.challenge == [0; 32]
        || assertion.minimum_height != checkpoint.height()
        || assertion.minimum_block_hash != *checkpoint.block_hash().as_ref()
    {
        return Err(rejected);
    }
    let native = SumeragiFinalityVerifier::from_trusted_checkpoint(
        checkpoint,
        network,
        checkpoint.chain_id(),
    )
    .map_err(|_| rejected)?;
    // The client's checkpoint fixes the only epoch authority; each boundary must be certified
    // by the incumbent quorum before its successor can sign anything this proof relies on.
    let mut certificates = SumeragiCommitVerifierV1::new(&native).map_err(|_| rejected)?;
    for boundary in &proof.epoch_boundaries {
        certificates
            .verify_epoch_boundary(boundary)
            .map_err(|_| rejected)?;
    }
    let verified = certificates
        .verify(&proof.certificate)
        .map_err(|_| rejected)?;
    if verified.height() <= checkpoint.height() {
        return Err(rejected);
    }
    let result = verified
        .verify_external_execution(network, expected, &proof.inclusion)
        .map_err(|_| rejected)?;
    if result.0.is_err() {
        return Err(rejected);
    }
    Ok(VerifiedSorafsPublicationV1 {
        height: verified.height(),
        completed: assertion.require_complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_source_request_roundtrips_canonical_metadata_and_chunk_selectors() {
        for chunk_index in [None, Some(0), Some(u32::MAX)] {
            let request = SorafsAssignedSourceRequestV1 {
                target_provider: [1; 32],
                source_provider: [2; 32],
                order_id: [3; 32],
                assignment_revision: 4,
                manifest_digest: [5; 32],
                chunk_index,
                floor_height: 6,
                floor_block_hash: [7; 32],
            };
            let mut bytes = norito::encode_canonical(&request).unwrap();
            assert!(bytes.len() < 4096);
            assert_eq!(
                norito::decode_canonical::<SorafsAssignedSourceRequestV1>(&bytes).unwrap(),
                request
            );
            bytes.push(0);
            assert!(norito::decode_canonical::<SorafsAssignedSourceRequestV1>(&bytes).is_err());
        }
    }

    mod compact_proof {
        use super::*;
        use crate::{
            account::AccountId,
            block::{BlockHeader, SignedBlock, builder::BlockBuilder},
            isi::Log,
            level::Level,
            sorafs::pin_registry::{ManifestDigest, ReplicationOrderId},
            sumeragi_finality::test_fixtures::NativeFinalityFixture,
            transaction::{FeePaymentIntent, TransactionBuilder, TransactionResultInner},
        };
        use iroha_crypto::{Algorithm, KeyPair};
        use std::time::Duration;

        struct Published {
            fixture: NativeFinalityFixture,
            checkpoint: SumeragiFinalityCheckpoint,
            signed: SignedTransaction,
            proof: SorafsPublicationProofV1,
        }

        fn assertion_block(
            fixture: &NativeFinalityFixture,
            checkpoint: &SumeragiFinalityCheckpoint,
            seed: u8,
            result: TransactionResultInner,
        ) -> (SignedTransaction, SignedBlock) {
            assertion_in(
                fixture.next_header(),
                fixture.network_id(),
                checkpoint,
                seed,
                true,
                result,
            )
        }

        /// One block at `header` holding a single assertion bound to `checkpoint`.
        fn assertion_in(
            header: BlockHeader,
            network: NetworkId,
            checkpoint: &SumeragiFinalityCheckpoint,
            seed: u8,
            complete: bool,
            result: TransactionResultInner,
        ) -> (SignedTransaction, SignedBlock) {
            // A zero challenge is a negative test input, but signing still needs a valid key.
            let signer = KeyPair::from_seed(vec![seed.max(1); 32], Algorithm::Ed25519);
            let mut builder = TransactionBuilder::new(
                network,
                AccountId::new(signer.public_key().clone()),
                FeePaymentIntent::authority(vec![], None),
            );
            builder.set_creation_time(Duration::from_millis(header.creation_time_ms - 1));
            let signed = builder
                .with_instructions([AssertSorafsPublicationV1 {
                    manifest_digest: ManifestDigest([1; 32]),
                    order_id: ReplicationOrderId([2; 32]),
                    assignment_revision: 1,
                    canonical_order_digest: [3; 32],
                    require_complete: complete,
                    challenge: [seed; 32],
                    minimum_height: checkpoint.height(),
                    minimum_block_hash: *checkpoint.block_hash().as_ref(),
                }])
                .sign(signer.private_key());
            let mut block = BlockBuilder::new(header);
            block.push_transaction(signed.clone());
            let mut block = block.build(crate::block::BlockSignatures::default());
            NativeFinalityFixture::install_network_results(&mut block, vec![result]);
            (signed, block)
        }

        fn inclusion(block: &SignedBlock) -> CertifiedTransactionInclusionV1 {
            CertifiedTransactionInclusionV1 {
                entry_proof: block.network_input_proof(0).unwrap(),
                output: block.execution_outputs()[0].clone(),
                output_proof: block.output_proof(0).unwrap(),
            }
        }

        fn publish(complete: bool, seed: u8, result: TransactionResultInner) -> Published {
            let mut fixture = NativeFinalityFixture::start("sorafs-publication-proof");
            let checkpoint = fixture.checkpoint();
            let (signed, block) = assertion_in(
                fixture.next_header(),
                fixture.network_id(),
                &checkpoint,
                seed,
                complete,
                result,
            );
            let inclusion = inclusion(&block);
            let certified = fixture.certify(block);
            let certificate = SumeragiCommitCertificateV1::from_proof(&certified).unwrap();
            Published {
                fixture,
                checkpoint,
                signed,
                proof: SorafsPublicationProofV1 {
                    epoch_boundaries: Vec::new(),
                    certificate,
                    inclusion,
                },
            }
        }

        #[test]
        fn certificate_and_inclusion_prove_applied_assertion_without_block_bodies() {
            for complete in [false, true] {
                let published = publish(complete, 7, Ok(Vec::new()));
                let bytes = norito::encode_canonical(&published.proof).unwrap();
                let decoded: SorafsPublicationProofV1 = norito::decode_canonical(&bytes).unwrap();
                assert_eq!(decoded, published.proof);
                let verified = verify_sorafs_publication_v1(
                    &published.fixture.network_id(),
                    &published.checkpoint,
                    &published.signed,
                    &decoded,
                )
                .unwrap();
                assert_eq!(verified.height(), 2);
                assert_eq!(verified.completed(), complete);
                assert_eq!(published.fixture.genesis_proof().committee.len(), 4);
                let qc: iroha_sumeragi::message::Qc =
                    norito::decode_canonical(&decoded.certificate.commit_qc).unwrap();
                assert_eq!(qc.signers.count_ones(), 3);
                // The proof is a header, quorum certificate, result and two Merkle paths.
                assert!(bytes.len() < 64 * 1024);
                let mut trailing = bytes;
                trailing.push(0);
                assert!(norito::decode_canonical::<SorafsPublicationProofV1>(&trailing).is_err());
            }
        }

        #[test]
        fn zero_challenge_and_non_assertion_are_not_publication() {
            let published = publish(true, 0, Ok(Vec::new()));
            let network = published.fixture.network_id();
            assert!(
                verify_sorafs_publication_v1(
                    &network,
                    &published.checkpoint,
                    &published.signed,
                    &published.proof,
                )
                .is_err()
            );
            let signer = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
            let transaction = TransactionBuilder::new(
                network,
                AccountId::new(signer.public_key().clone()),
                FeePaymentIntent::authority(vec![], None),
            )
            .with_instructions([Log::new(Level::INFO, "not an assertion".into())])
            .sign(signer.private_key());
            assert!(
                verify_sorafs_publication_v1(
                    &network,
                    &published.checkpoint,
                    &transaction,
                    &published.proof,
                )
                .is_err()
            );
        }

        #[test]
        fn substituted_or_unbound_publication_evidence_is_rejected() {
            let published = publish(true, 7, Ok(Vec::new()));
            let network = published.fixture.network_id();
            let reject = |checkpoint, signed, proof| {
                verify_sorafs_publication_v1(&network, checkpoint, signed, proof).is_err()
            };
            // Network identity comes from genesis, not the separately selected chain label.
            let foreign = NativeFinalityFixture::start_at(
                published.fixture.chain_id(),
                published.fixture.genesis().header().creation_time_ms + 1,
            );
            assert_ne!(foreign.network_id(), network);
            assert!(
                verify_sorafs_publication_v1(
                    &foreign.network_id(),
                    &published.checkpoint,
                    &published.signed,
                    &published.proof,
                )
                .is_err()
            );
            // Another signed assertion cannot borrow this inclusion or certificate.
            let (other, other_block) =
                assertion_block(&published.fixture, &published.checkpoint, 9, Ok(Vec::new()));
            assert!(reject(&published.checkpoint, &other, &published.proof));
            let (other_phase, _) = assertion_in(
                published.fixture.latest().block_header.clone(),
                network,
                &published.checkpoint,
                7,
                false,
                Ok(Vec::new()),
            );
            assert!(reject(
                &published.checkpoint,
                &other_phase,
                &published.proof
            ));
            // Single-leaf paths with identical outputs are identical witnesses. Include a
            // second transaction so the substituted paths bind a different input/output tree.
            let mut other_builder = BlockBuilder::new(other_block.header());
            other_builder.push_transaction(other.clone());
            other_builder.push_transaction(published.signed.clone());
            let mut other_block = other_builder.build(crate::block::BlockSignatures::default());
            NativeFinalityFixture::install_network_results(
                &mut other_block,
                vec![Ok(Vec::new()), Ok(Vec::new())],
            );
            let mut substituted = published.proof.clone();
            substituted.inclusion = inclusion(&other_block);
            assert_ne!(substituted.inclusion, published.proof.inclusion);
            assert!(reject(
                &published.checkpoint,
                &published.signed,
                &substituted
            ));
            let mut forged = published.proof.clone();
            forged.certificate.commit_qc[0] ^= 1;
            assert!(reject(&published.checkpoint, &published.signed, &forged));
            let mut flooded = published.proof.clone();
            flooded.epoch_boundaries = vec![
                published.proof.certificate.clone();
                PUBLICATION_PROOF_MAX_EPOCH_BOUNDARIES_V1 + 1
            ];
            assert!(reject(&published.checkpoint, &published.signed, &flooded));
            // A newer floor than the one the assertion names is not its checkpoint.
            let mut advanced = published.fixture;
            let (_, later) = assertion_block(&advanced, &published.checkpoint, 11, Ok(Vec::new()));
            advanced.certify(later);
            assert!(reject(
                &advanced.checkpoint(),
                &published.signed,
                &published.proof
            ));
        }

        #[test]
        fn independently_valid_fork_does_not_supply_the_assertions_floor() {
            let mut selected = publish(true, 7, Ok(Vec::new())).fixture;
            let fork = publish(true, 9, Ok(Vec::new())).fixture;
            let checkpoint = selected.checkpoint();
            let fork_checkpoint = fork.checkpoint();
            assert_eq!(selected.network_id(), fork.network_id());
            assert_eq!(checkpoint.height(), fork_checkpoint.height());
            assert_ne!(checkpoint.block_hash(), fork_checkpoint.block_hash());
            for (bound, substituted) in [
                (&checkpoint, &fork_checkpoint),
                (&fork_checkpoint, &checkpoint),
            ] {
                let (signed, block) = assertion_block(&selected, bound, 11, Ok(Vec::new()));
                let inclusion = inclusion(&block);
                let certified = selected.certify(block);
                let proof = SorafsPublicationProofV1 {
                    epoch_boundaries: Vec::new(),
                    certificate: SumeragiCommitCertificateV1::from_proof(&certified).unwrap(),
                    inclusion,
                };
                if bound == &checkpoint {
                    assert!(
                        verify_sorafs_publication_v1(
                            &selected.network_id(),
                            bound,
                            &signed,
                            &proof,
                        )
                        .is_ok()
                    );
                }
                assert!(
                    verify_sorafs_publication_v1(
                        &selected.network_id(),
                        substituted,
                        &signed,
                        &proof,
                    )
                    .is_err()
                );
            }
        }

        /// The independently selected signed genesis of `fixture`, without later decisions.
        fn genesis_checkpoint(fixture: &NativeFinalityFixture) -> SumeragiFinalityCheckpoint {
            let mut verifier = SumeragiFinalityVerifier::new(
                fixture.genesis(),
                fixture.chain_id(),
                fixture.genesis_proof().committee.clone(),
            )
            .unwrap();
            verifier.verify(fixture.genesis_proof()).unwrap();
            verifier.export_checkpoint(fixture.genesis_proof()).unwrap()
        }

        #[test]
        fn incumbent_boundary_carries_an_old_checkpoint_into_the_next_epoch() {
            // H1..H3 are epoch zero; H4, the first epoch-one block, executes the assertion.
            let mut assertion = None;
            let (fixture, chain) =
                NativeFinalityFixture::short_npos_boundary_chain_with_successor_block(
                    4,
                    |fixture, header| {
                        let checkpoint = genesis_checkpoint(fixture);
                        let (signed, block) = assertion_in(
                            header,
                            fixture.network_id(),
                            &checkpoint,
                            13,
                            true,
                            Ok(Vec::new()),
                        );
                        assertion = Some((checkpoint, signed, inclusion(&block)));
                        block
                    },
                );
            let (checkpoint, signed, inclusion) = assertion.unwrap();
            let network = fixture.network_id();
            let certificate = |height: usize| {
                SumeragiCommitCertificateV1::from_proof(&chain[height - 1]).unwrap()
            };
            let proof = SorafsPublicationProofV1 {
                epoch_boundaries: vec![certificate(3)],
                certificate: certificate(4),
                inclusion,
            };
            let verified =
                verify_sorafs_publication_v1(&network, &checkpoint, &signed, &proof).unwrap();
            assert_eq!(verified.height(), 4);
            // Epoch one has no authority until epoch zero's quorum certifies its boundary.
            let mut skipped = proof.clone();
            skipped.epoch_boundaries.clear();
            assert!(
                verify_sorafs_publication_v1(&network, &checkpoint, &signed, &skipped).is_err()
            );
            // An ordinary epoch-zero certificate selects no successor.
            let mut ordinary = proof.clone();
            ordinary.epoch_boundaries = vec![certificate(2)];
            assert!(
                verify_sorafs_publication_v1(&network, &checkpoint, &signed, &ordinary).is_err()
            );
            let mut forged = proof;
            forged.epoch_boundaries[0].commit_qc[0] ^= 1;
            assert!(verify_sorafs_publication_v1(&network, &checkpoint, &signed, &forged).is_err());
        }

        #[test]
        fn rejected_execution_is_not_publication() {
            let published = publish(
                true,
                7,
                Err(
                    crate::transaction::error::TransactionRejectionReason::Validation(
                        crate::ValidationFail::NotPermitted("fixture rejection".into()),
                    ),
                ),
            );
            assert!(
                verify_sorafs_publication_v1(
                    &published.fixture.network_id(),
                    &published.checkpoint,
                    &published.signed,
                    &published.proof,
                )
                .is_err()
            );
            let successful = publish(true, 7, Ok(Vec::new()));
            let mut substituted = successful.proof;
            substituted.inclusion.output = published.proof.inclusion.output;
            assert!(
                verify_sorafs_publication_v1(
                    &successful.fixture.network_id(),
                    &successful.checkpoint,
                    &successful.signed,
                    &substituted,
                )
                .is_err()
            );
        }
    }
}
