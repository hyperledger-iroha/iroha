//! Authoritative read-only finality checks for private Musubi storage coordination.
//!
//! This module deliberately owns no listener, queue submission, SoraFS mutation, or service
//! activation. It only derives one current archive record from daemon-owned finalized state and
//! exact Kura history; the stock publication service remains unavailable without deployment
//! injection.
use std::{num::NonZeroUsize, sync::Arc};
use iroha_core::{
    smartcontracts::isi::musubi::validate_musubi_registry_snapshot_history_v1,
    state::{State, StateReadOnly as _, WorldReadOnly as _, WorldStateSnapshot as _},
};
use iroha_data_model::{
    NetworkId,
    block::{SignedBlock, consensus_v2::finality::V2FinalityArtifact},
    isi::musubi::RegisterMusubiArchiveV1,
    musubi::{
        MusubiArchiveRecordV1, MusubiArchiveRegistrationProjectionV1, MusubiRegistrySnapshotV1,
    },
    transaction::{Executable, TransactionEntrypoint},
};
use mv::storage::StorageReadOnly as _;
/// Exact immutable evidence needed to recover a finalized archive registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiPublicationFinalizedArchiveRegistrationQueryV1 {
    /// Closed schema version; must equal one.
    pub version: u8,
    /// Exact deployment identity derived from the committed genesis header.
    pub network_id: NetworkId,
    /// Canonical identity of the signed registration transaction.
    pub transaction_hash: [u8; 32],
    /// Finalized registry snapshot at or after archive registration.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Immutable registration projection supplied by the authenticated publisher operation.
    pub registration: MusubiArchiveRegistrationProjectionV1,
    /// Exact registry policy revision encoded by the native registration instruction.
    pub expected_policy_revision: u64,
}
impl MusubiPublicationFinalizedArchiveRegistrationQueryV1 {
    fn validate(&self) -> Result<(), MusubiPublicationFinalizedArchiveRegistrationReadErrorV1> {
        self.snapshot.validate().map_err(|_| invalid())?;
        self.registration.validate().map_err(|_| invalid())?;
        if self.version != 1
            || self.network_id.as_bytes()[31] & 1 != 1
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || self.expected_policy_revision == 0
            || self.registration.staging_receipt.payload.binding.network_id != self.network_id
            || self.registration.registered_at_height > self.snapshot.finalized_height
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/// Closed, redacted failure from the daemon-owned finalized reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationFinalizedArchiveRegistrationReadErrorV1 {
    /// The supplied evidence is ahead of this node's coherent finalized view.
    LocallyAhead,
    /// Evidence is malformed, substituted, absent from canonical history, or otherwise invalid.
    Invalid,
}
impl MusubiPublicationFinalizedArchiveRegistrationReadErrorV1 {
    /// Whether retrying after the local finalized view advances may succeed.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::LocallyAhead)
    }
}
impl core::fmt::Display for MusubiPublicationFinalizedArchiveRegistrationReadErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::LocallyAhead => {
                "finalized Musubi archive-registration evidence is ahead of local state"
            }
            Self::Invalid => "finalized Musubi archive-registration evidence is invalid",
        })
    }
}
impl std::error::Error for MusubiPublicationFinalizedArchiveRegistrationReadErrorV1 {}
const fn invalid() -> MusubiPublicationFinalizedArchiveRegistrationReadErrorV1 {
    MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::Invalid
}
/// Read-only daemon adapter for exact finalized archive registrations.
#[derive(Clone)]
pub struct MusubiPublicationFinalizedArchiveRegistrationReaderV1 {
    network_id: NetworkId,
    state: Arc<State>,
}
impl core::fmt::Debug for MusubiPublicationFinalizedArchiveRegistrationReaderV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationFinalizedArchiveRegistrationReaderV1")
            .field("network_id", &self.network_id)
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationFinalizedArchiveRegistrationReaderV1 {
    /// Bind a reader to one exact daemon network and finalized-state handle.
    ///
    /// # Errors
    ///
    /// Returns a permanent invalid-evidence error when the explicit network differs from the
    /// state handle's genesis-derived network identity.
    pub fn new(
        network_id: NetworkId,
        state: Arc<State>,
    ) -> Result<Self, MusubiPublicationFinalizedArchiveRegistrationReadErrorV1> {
        if state.network_id_ref() != &network_id {
            return Err(invalid());
        }
        Ok(Self { network_id, state })
    }
    pub(super) fn from_validated_context(network_id: NetworkId, state: Arc<State>) -> Self {
        debug_assert_eq!(state.network_id_ref(), &network_id);
        Self { network_id, state }
    }
    /// Read and independently authenticate one current archive record from finalized history.
    ///
    /// A single Core query view binds the world and canonical block-hash journal. The reader then
    /// validates Core's resolver-revision history, requires Kura's cryptographically verified V2
    /// finality artifact to commit to the exact result-bearing registration-height block wire,
    /// authenticates the unique successful native registration transaction, and compares its
    /// immutable projection with the current archive record. No state or storage effect occurs.
    ///
    /// # Errors
    ///
    /// Returns [`MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead`] only
    /// when the named snapshot height or resolver revision is beyond the captured local view. All
    /// malformed, missing, rejected, substituted, or inconsistent evidence is permanently invalid.
    pub fn read_current_archive(
        &self,
        query: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> Result<MusubiArchiveRecordV1, MusubiPublicationFinalizedArchiveRegistrationReadErrorV1>
    {
        query.validate()?;
        if query.network_id != self.network_id {
            return Err(invalid());
        }
        let view = self.state.query_view();
        let local_height = u64::try_from(view.block_hashes().len()).map_err(|_| invalid())?;
        let local_revision = view.world().musubi_resolver_index_revision();
        if query.snapshot.finalized_height > local_height
            || query.snapshot.index_revision > local_revision
        {
            return Err(MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead);
        }
        validate_musubi_registry_snapshot_history_v1(&query.snapshot, &view)
            .map_err(|_| invalid())?;
        let registered_height = usize::try_from(query.registration.registered_at_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(invalid())?;
        let canonical_hash = view
            .block_hashes()
            .get(registered_height.get() - 1)
            .copied()
            .ok_or(invalid())?;
        let block = view.kura().get_block(registered_height).ok_or(invalid())?;
        let finality = view
            .kura()
            .v2_finality_artifact(query.registration.registered_at_height)
            .map_err(|_| invalid())?
            .ok_or(invalid())?;
        if !validate_finalized_block_wire(
            &query.network_id,
            query.registration.registered_at_height,
            canonical_hash,
            &block,
            &finality,
        ) || !validate_registration_transaction(query, &block)
        {
            return Err(invalid());
        }
        let archive = view
            .world()
            .musubi_archives()
            .get(&query.registration.archive_id)
            .ok_or_else(invalid)?;
        archive.validate().map_err(|_| invalid())?;
        if archive.registration_projection() != query.registration {
            return Err(invalid());
        }
        Ok(archive.clone())
    }
}
fn validate_finalized_block_wire(
    network_id: &NetworkId,
    registered_height: u64,
    canonical_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    block: &SignedBlock,
    finality: &V2FinalityArtifact,
) -> bool {
    block.header().height().get() == registered_height
        && block.hash() == canonical_hash
        && finality.height == registered_height
        && finality.height_context.height == registered_height
        && &finality.height_context.network_id == network_id
        && finality.block_hash == canonical_hash
        && finality.subject.block_hash == canonical_hash
        && block.executed_block_wire_hash().is_ok_and(|wire_hash| {
            finality
                .commit_qc
                .execution_commitment
                .executed_block_wire_hash
                == wire_hash
        })
}
fn validate_registration_transaction(
    query: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    block: &SignedBlock,
) -> bool {
    if !block.has_results() || block.entrypoint_hashes().len() != block.results().len() {
        return false;
    }
    let mut found = false;
    for (_, entrypoint, result) in block.entrypoint_results() {
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => {
                if *reveal.signed_transaction().hash().as_ref() == query.transaction_hash {
                    return false;
                }
                continue;
            }
            TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                continue;
            }
        };
        if *transaction.hash().as_ref() != query.transaction_hash {
            continue;
        }
        if found
            || result.as_ref().is_err()
            || transaction.verify_signature().is_err()
            || transaction.network_id() != Some(&query.network_id)
            || transaction.authority() != &query.registration.registered_by
        {
            return false;
        }
        found = true;
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return false;
        };
        let [instruction] = instructions.as_ref() else {
            return false;
        };
        let Some(register) = instruction
            .as_any()
            .downcast_ref::<RegisterMusubiArchiveV1>()
        else {
            return false;
        };
        if register.commitment != query.registration.commitment
            || register.staging_receipt != query.registration.staging_receipt
            || register.expected_policy_revision != query.expected_policy_revision
        {
            return false;
        }
    }
    found
}
#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64, sync::Arc, time::Duration};
    use super::*;
    use iroha_core::{
        block::BlockBuilder,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId, Registrable as _, ValidationFail,
        account::{Account, AccountId},
        asset::AssetDefinition,
        block::{
            BlockHeader, SignedBlock,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                QuorumCertificate, ValidatorPower,
            },
        },
        domain::Domain,
        isi::{InstructionBox, musubi::RegisterMusubiArchiveV1},
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiArchiveCommitmentV1, MusubiContentDigestV1,
            MusubiSeedIngressReceiptApprovalV1, MusubiSeedIngressReceiptBindingV1,
            MusubiSeedIngressReceiptPayloadV1, MusubiSeedIngressReceiptV1,
            MusubiSemanticReleaseDigestV1,
        },
        peer::PeerId,
        sorafs::{
            capacity::ProviderId,
            pin_registry::{ChunkerProfileHandle, ManifestRootCid},
        },
        transaction::{
            DataTriggerSequence, FeePaymentIntent, SignedTransaction, TransactionBuilder,
            TransactionResultInner, error::TransactionRejectionReason,
        },
    };
    use iroha_primitives::time::TimeSource;
    struct RegistrationMaterial {
        network_id: NetworkId,
        publisher_key: KeyPair,
        archive: MusubiArchiveRecordV1,
    }
    struct ReaderFixture {
        reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
        state: Arc<State>,
        query: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        archive: MusubiArchiveRecordV1,
    }
    fn keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives an Ed25519 keypair")
    }
    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed | 1; 32]),
        ))
    }
    fn archive_commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
            por_root: MusubiContentDigestV1::new([3; 32]),
            content_length: 1_024,
            car_digest: MusubiContentDigestV1::new([4; 32]),
            car_size: 2_048,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: 2,
            chunk_count: 4,
        }
    }
    fn registration_material() -> RegistrationMaterial {
        let network_id = network_id(0x15);
        let publisher_key = keypair(0x31);
        let publisher = AccountId::new(publisher_key.public_key().clone());
        let broker_key = keypair(0x32);
        let broker = AccountId::new(broker_key.public_key().clone());
        let commitment = archive_commitment();
        let binding = MusubiSeedIngressReceiptBindingV1 {
            network_id,
            publisher: publisher.clone(),
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x33; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x34; 32]),
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x35; 32],
        };
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding,
            issued_at_ms: 500,
            expires_at_ms: 2_000,
        };
        let receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_key.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign staging receipt"),
            }],
            payload,
        };
        let archive = MusubiArchiveRecordV1 {
            archive_id: commitment.archive_id(),
            commitment,
            staging_receipt: receipt,
            registered_by: publisher,
            registered_at_height: 1,
            location_revision: 2,
            location_ids: Vec::new(),
        };
        archive.validate().expect("valid archive fixture");
        RegistrationMaterial {
            network_id,
            publisher_key,
            archive,
        }
    }
    fn registration_instruction(archive: &MusubiArchiveRecordV1) -> RegisterMusubiArchiveV1 {
        RegisterMusubiArchiveV1::new(
            archive.commitment.clone(),
            archive.staging_receipt.clone(),
            1,
        )
    }
    fn signed_transaction(
        network_id: NetworkId,
        key: &KeyPair,
        instructions: Vec<InstructionBox>,
    ) -> SignedTransaction {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1_000));
        TransactionBuilder::new_with_time_source(
            network_id,
            AccountId::new(key.public_key().clone()),
            &time_source,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(key.private_key())
    }
    fn successful_registration_transaction(material: &RegistrationMaterial) -> SignedTransaction {
        signed_transaction(
            material.network_id,
            &material.publisher_key,
            vec![registration_instruction(&material.archive).into()],
        )
    }
    fn signed_block_with_results(
        transactions: Vec<SignedTransaction>,
        rejected_index: Option<usize>,
    ) -> SignedBlock {
        let entrypoint_hashes = transactions
            .iter()
            .map(SignedTransaction::hash_as_entrypoint)
            .collect::<Vec<_>>();
        let accepted = transactions
            .into_iter()
            .map(|transaction| {
                iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
            })
            .collect();
        let mut block: SignedBlock = BlockBuilder::new(accepted)
            .chain(0, None)
            .sign(keypair(0x41).private_key())
            .unpack(|_| {})
            .into();
        let results = entrypoint_hashes
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if rejected_index == Some(index) {
                    TransactionResultInner::Err(TransactionRejectionReason::Validation(
                        ValidationFail::NotPermitted("rejected registration fixture".to_owned()),
                    ))
                } else {
                    TransactionResultInner::Ok(DataTriggerSequence::default())
                }
            })
            .collect();
        block
            .set_transaction_results(Vec::new(), &entrypoint_hashes, results)
            .expect("fixture result hashes match entrypoints");
        block
    }
    fn finality_keypairs() -> Vec<KeyPair> {
        let mut keypairs = (0_u8..4)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![0xA0_u8.saturating_add(index); 32],
                    Algorithm::BlsNormal,
                )
                .expect("derive deterministic finality BLS fixture key")
            })
            .collect::<Vec<_>>();
        keypairs.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        keypairs
    }
    fn finality_artifact(block: &SignedBlock, network_id: NetworkId) -> V2FinalityArtifact {
        let keypairs = finality_keypairs();
        let roster = keypairs
            .iter()
            .map(|keypair| ValidatorPower {
                validator: PeerId::new(keypair.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let height = block.header().height().get();
        let context = HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid finality fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Musubi finality fixture Nexus context"),
            execution_policy_hash: Hash::new(b"Musubi finality fixture execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4_096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        };
        let executed_wire = block.encode_wire().expect("canonical executed block wire");
        let execution_commitment = ExecutionCommitment::new_without_merge_carrier(
            Hash::new(b"Musubi finality fixture parent state"),
            Hash::new(b"Musubi finality fixture post state"),
            Hash::new(b"Musubi finality fixture ordinary writes"),
            None,
            0,
            u64::try_from(executed_wire.len()).expect("fixture wire length fits u64"),
            Hash::new(&executed_wire),
        )
        .expect("canonical finality fixture execution commitment");
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal wire hash"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: block.header().view_change_index(),
        };
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = commit_qc
            .signer_preimage(&context, 0)
            .expect("valid finality fixture signer");
        let signatures = commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keypairs[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign finality fixture vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate finality fixture votes");
        let validator_set_pops = keypairs
            .iter()
            .map(|keypair| {
                bls_normal_pop_prove(keypair.private_key())
                    .expect("derive finality fixture proof of possession")
            })
            .collect();
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact
            .verify()
            .expect("finality fixture is cryptographically valid");
        artifact
    }
    fn seeded_world(material: &RegistrationMaterial) -> World {
        let publisher = material.archive.registered_by.clone();
        let account = Account::new(publisher.clone()).build(&publisher);
        let mut world = World::with(
            std::iter::empty::<Domain>(),
            [account],
            std::iter::empty::<AssetDefinition>(),
        );
        let binding = &material.archive.staging_receipt.payload.binding;
        world
            .provider_owners_mut_for_testing()
            .insert(binding.seed_provider, binding.ingress_broker.clone());
        world
    }
    fn reader_fixture() -> ReaderFixture {
        reader_fixture_with_finality(true)
    }
    fn reader_fixture_with_finality(store_finality: bool) -> ReaderFixture {
        let material = registration_material();
        let registration_transaction = successful_registration_transaction(&material);
        let transaction_hash = *registration_transaction.hash().as_ref();
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            seeded_world(&material),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            ChainId::from("musubi-finality-reader-test"),
            material.network_id,
        ));
        let (_block_time_handle, block_time_source) =
            TimeSource::new_mock(Duration::from_millis(1_500));
        let new_block = BlockBuilder::new_with_time_source(
            vec![iroha_core::tx::AcceptedTransaction::new_unchecked(
                Cow::Owned(registration_transaction),
            )],
            block_time_source,
        )
        .chain(0, None)
        .sign(keypair(0x42).private_key())
        .unpack(|_| {});
        let mut state_block = state.block(new_block.header());
        let valid = new_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let committed = valid.commit_unchecked().unpack(|_| {});
        assert!(committed.as_ref().error(0).is_none());
        let canonical_block = committed.as_ref().clone();
        kura.store_block(Arc::new(canonical_block.clone()))
            .expect("store fixture Kura block");
        if store_finality {
            let _ = kura
                .store_v2_finality_artifact(&finality_artifact(
                    &canonical_block,
                    material.network_id,
                ))
                .expect("store fixture V2 finality artifact");
        }
        let _ = state_block.apply_without_execution(&committed, Vec::new());
        state_block.commit().expect("commit fixture state block");
        let registered = state
            .query_view()
            .world()
            .musubi_archives()
            .get(&material.archive.archive_id)
            .cloned()
            .expect("the native instruction creates the archive");
        assert_eq!(
            registered.registration_projection(),
            material.archive.registration_projection()
        );
        assert_eq!(registered.location_revision, 1);
        replace_current_archive(&state, *canonical_block.hash().as_ref(), &material.archive);
        let query = MusubiPublicationFinalizedArchiveRegistrationQueryV1 {
            version: 1,
            network_id: material.network_id,
            transaction_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: *canonical_block.hash().as_ref(),
                index_revision: 1,
            },
            registration: material.archive.registration_projection(),
            expected_policy_revision: 1,
        };
        let reader = MusubiPublicationFinalizedArchiveRegistrationReaderV1::new(
            material.network_id,
            Arc::clone(&state),
        )
        .expect("reader binds exact fixture state");
        ReaderFixture {
            reader,
            state,
            query,
            archive: material.archive,
        }
    }
    fn replace_current_archive(
        state: &State,
        canonical_hash: [u8; 32],
        archive: &MusubiArchiveRecordV1,
    ) {
        let header = BlockHeader::new(
            NonZeroU64::new(2).expect("nonzero fixture height"),
            Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                canonical_hash,
            ))),
            None,
            None,
            2_000,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction
            .world_mut_for_testing()
            .musubi_archives_mut()
            .insert(archive.archive_id, archive.clone());
        transaction.apply();
        block.commit().expect("commit current archive substitution");
    }
    #[test]
    fn exact_finalized_registration_returns_current_mutable_record() {
        let fixture = reader_fixture();
        let archive = fixture
            .reader
            .read_current_archive(&fixture.query)
            .expect("exact finalized registration reads successfully");
        assert_eq!(archive, fixture.archive);
        assert_eq!(archive.location_revision, 2);
        assert_eq!(archive.location_ids, fixture.archive.location_ids);
    }
    #[test]
    fn registration_without_verified_v2_finality_is_invalid() {
        let fixture = reader_fixture_with_finality(false);
        assert_eq!(
            fixture
                .reader
                .read_current_archive(&fixture.query)
                .expect_err("an uncertified Kura body must fail closed"),
            invalid()
        );
    }
    #[test]
    fn missing_duplicate_and_rejected_transactions_are_invalid() {
        let fixture = reader_fixture();
        let mut missing = fixture.query.clone();
        missing.transaction_hash = [0x71; 32];
        let error = fixture
            .reader
            .read_current_archive(&missing)
            .expect_err("missing transaction must fail closed");
        assert_eq!(error, invalid());
        let material = registration_material();
        let transaction = successful_registration_transaction(&material);
        let mut query = fixture.query.clone();
        query.transaction_hash = *transaction.hash().as_ref();
        let duplicate =
            signed_block_with_results(vec![transaction.clone(), transaction.clone()], None);
        assert!(!validate_registration_transaction(&query, &duplicate));
        let rejected = signed_block_with_results(vec![transaction], Some(0));
        assert!(!validate_registration_transaction(&query, &rejected));
    }
    #[test]
    fn multi_instruction_registration_is_invalid() {
        let fixture = reader_fixture();
        let material = registration_material();
        let register = registration_instruction(&material.archive);
        let transaction = signed_transaction(
            material.network_id,
            &material.publisher_key,
            vec![register.clone().into(), register.into()],
        );
        let mut query = fixture.query;
        query.transaction_hash = *transaction.hash().as_ref();
        let block = signed_block_with_results(vec![transaction], None);
        assert!(!validate_registration_transaction(&query, &block));
    }
    #[test]
    fn wrong_authority_registration_is_invalid() {
        let fixture = reader_fixture();
        let material = registration_material();
        let transaction = signed_transaction(
            material.network_id,
            &keypair(0x51),
            vec![registration_instruction(&material.archive).into()],
        );
        let mut query = fixture.query;
        query.transaction_hash = *transaction.hash().as_ref();
        let block = signed_block_with_results(vec![transaction], None);
        assert!(!validate_registration_transaction(&query, &block));
    }
    #[test]
    fn wrong_network_registration_is_invalid() {
        let fixture = reader_fixture();
        let material = registration_material();
        let transaction = signed_transaction(
            network_id(0x25),
            &material.publisher_key,
            vec![registration_instruction(&material.archive).into()],
        );
        let mut query = fixture.query;
        query.transaction_hash = *transaction.hash().as_ref();
        let block = signed_block_with_results(vec![transaction], None);
        assert!(!validate_registration_transaction(&query, &block));
    }
    #[test]
    fn snapshot_and_finalized_wire_substitution_are_invalid() {
        let fixture = reader_fixture();
        let mut substituted_snapshot = fixture.query.clone();
        substituted_snapshot.snapshot.finalized_block_hash = [0x61; 32];
        assert_eq!(
            fixture
                .reader
                .read_current_archive(&substituted_snapshot)
                .expect_err("snapshot substitution must fail closed"),
            invalid()
        );
        let view = fixture.state.query_view();
        let block = view
            .kura()
            .get_block(NonZeroUsize::new(1).expect("nonzero fixture height"))
            .expect("fixture Kura block");
        let finality = view
            .kura()
            .v2_finality_artifact(1)
            .expect("read fixture finality")
            .expect("fixture finality exists");
        let canonical_hash = block.hash();
        assert!(validate_finalized_block_wire(
            &fixture.query.network_id,
            1,
            canonical_hash,
            &block,
            &finality,
        ));
        let mut substituted = block.as_ref().clone();
        let entrypoint_hashes = substituted.entrypoint_hashes().collect::<Vec<_>>();
        substituted
            .set_transaction_results(
                Vec::new(),
                &entrypoint_hashes,
                vec![TransactionResultInner::Err(
                    TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                        "substituted Kura result".to_owned(),
                    )),
                )],
            )
            .expect("replace the result while retaining the consensus header hash");
        assert_eq!(substituted.hash(), canonical_hash);
        assert!(!validate_finalized_block_wire(
            &fixture.query.network_id,
            1,
            canonical_hash,
            &substituted,
            &finality,
        ));
    }
    #[test]
    fn current_registration_projection_substitution_is_invalid() {
        let fixture = reader_fixture();
        let mut substituted = fixture.archive.clone();
        substituted.staging_receipt.payload.binding.nonce = [0x62; 32];
        substituted
            .validate()
            .expect("substituted record remains structural");
        replace_current_archive(
            &fixture.state,
            fixture.query.snapshot.finalized_block_hash,
            &substituted,
        );
        assert_eq!(
            fixture
                .reader
                .read_current_archive(&fixture.query)
                .expect_err("current immutable projection substitution must fail closed"),
            invalid()
        );
    }
    #[test]
    fn only_evidence_ahead_of_local_finality_is_retryable() {
        let fixture = reader_fixture();
        let wrong_reader_error = MusubiPublicationFinalizedArchiveRegistrationReaderV1::new(
            network_id(0x25),
            Arc::clone(&fixture.state),
        )
        .expect_err("reader must reject a state handle from another exact network");
        assert_eq!(wrong_reader_error, invalid());
        assert!(!wrong_reader_error.is_retryable());
        let mut height_ahead = fixture.query.clone();
        height_ahead.snapshot.finalized_height = 2;
        height_ahead.snapshot.finalized_block_hash = [0x63; 32];
        let height_error = fixture
            .reader
            .read_current_archive(&height_ahead)
            .expect_err("future finalized height is locally ahead");
        assert_eq!(
            height_error,
            MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead
        );
        assert!(height_error.is_retryable());
        let mut revision_ahead = fixture.query.clone();
        revision_ahead.snapshot.index_revision = 2;
        let revision_error = fixture
            .reader
            .read_current_archive(&revision_ahead)
            .expect_err("future resolver revision is locally ahead");
        assert_eq!(
            revision_error,
            MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead
        );
        assert!(revision_error.is_retryable());
        let mut substituted = fixture.query;
        substituted.snapshot.finalized_block_hash = [0x64; 32];
        let invalid_error = fixture
            .reader
            .read_current_archive(&substituted)
            .expect_err("same-height fork evidence is invalid");
        assert_eq!(invalid_error, invalid());
        assert!(!invalid_error.is_retryable());
    }
}
