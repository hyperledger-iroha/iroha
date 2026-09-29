//! Authoritative read-only finality checks for private Musubi storage coordination.
//!
//! This module deliberately owns no listener, queue submission, `SoraFS` mutation, or service
//! activation. It only derives one current archive record from daemon-owned finalized state and
//! exact Kura history; the stock publication service remains unavailable without deployment
//! injection.
use iroha_core::{
    smartcontracts::isi::musubi::validate_musubi_registry_snapshot_history_v1,
    state::{State, StateQueryView, StateReadOnly as _, WorldReadOnly, WorldStateSnapshot as _},
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::{SignedBlock, consensus_v2::finality::V2FinalityArtifact},
    isi::musubi::RegisterMusubiArchiveV1,
    musubi::{
        MusubiArchiveLocationV1, MusubiArchiveRecordV1, MusubiArchiveRegistrationProjectionV1,
        MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleAttestationRecordV1,
        MusubiRegistrySnapshotV1, musubi_provider_bundle_attestation_set_digest_v1,
    },
    sorafs::capacity::ProviderId,
    transaction::{Executable, TransactionEntrypoint},
};
use mv::storage::StorageReadOnly as _;
use std::{num::NonZeroUsize, sync::Arc};
/// Exact immutable evidence needed to recover a finalized archive registration.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "irohad::musubi_publication_service::MusubiPublicationFinalizedArchiveRegistrationQueryV1"
)]
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
        self.read_current_archive_in_view(query, &self.state.query_view())
    }
    pub(super) fn read_current_archive_in_view(
        &self,
        query: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        view: &StateQueryView<'_>,
    ) -> Result<MusubiArchiveRecordV1, MusubiPublicationFinalizedArchiveRegistrationReadErrorV1>
    {
        query.validate()?;
        if query.network_id != self.network_id {
            return Err(invalid());
        }
        let local_height = u64::try_from(view.block_hashes().len()).map_err(|_| invalid())?;
        let local_revision = view.world().musubi_resolver_index_revision();
        if query.snapshot.finalized_height > local_height
            || query.snapshot.index_revision > local_revision
        {
            return Err(MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead);
        }
        validate_musubi_registry_snapshot_history_v1(&query.snapshot, view)
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
    /// Authenticate the selected readback location against one coherent current State cut.
    ///
    /// The registration block and current tip both require exact Kura V2 finality artifacts. The
    /// location, provider-owner, and immutable signed provider-attestation rows are read from the
    /// same State query view as the archive. A later owner replacement cannot reuse the former
    /// completion attestation.
    /// This relies on State's committed-world ownership; a complete State-root witness remains
    /// a separate release gate.
    pub(super) fn validate_current_readback_target(
        &self,
        query: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<(), MusubiPublicationFinalizedArchiveRegistrationReadErrorV1> {
        let view = self.state.query_view();
        let archive = self.read_current_archive_in_view(query, &view)?;
        let tip_height = u64::try_from(view.block_hashes().len()).map_err(|_| invalid())?;
        if location.finalized_height > tip_height {
            return Err(MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead);
        }
        let tip_number = NonZeroUsize::new(view.block_hashes().len()).ok_or_else(invalid)?;
        let tip_hash = view
            .block_hashes()
            .get(tip_number.get() - 1)
            .copied()
            .ok_or_else(invalid)?;
        let tip_block = view.kura().get_block(tip_number).ok_or_else(invalid)?;
        let tip_finality = view
            .kura()
            .v2_finality_artifact(tip_height)
            .map_err(|_| invalid())?
            .ok_or(MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead)?;
        let attestation_key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: archive.archive_id,
            replication_order: location.replication_order,
            provider_id: provider,
        };
        if !validate_finalized_block_wire(
            &query.network_id,
            tip_height,
            tip_hash,
            &tip_block,
            &tip_finality,
        ) || !complete_location_attestations_match(&archive, location, view.world())
            || !current_readback_target_matches(
                &archive,
                location,
                view.world().musubi_archive_locations().get(&location.key()),
                view.world().provider_owners().get(&provider),
                view.world()
                    .musubi_provider_bundle_attestations()
                    .get(&attestation_key),
            )
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn complete_location_attestations_match(
    archive: &MusubiArchiveRecordV1,
    location: &MusubiArchiveLocationV1,
    world: &impl WorldReadOnly,
) -> bool {
    let references = location
        .providers
        .iter()
        .map(|provider| {
            let key = MusubiProviderBundleAttestationKeyV1 {
                archive_id: archive.archive_id,
                replication_order: location.replication_order,
                provider_id: *provider,
            };
            let record = world.musubi_provider_bundle_attestations().get(&key)?;
            let binding = &record.attestation.payload.binding;
            (record.validate().is_ok()
                && record.attestation.verify(binding).is_ok()
                && record.registered_at_height >= archive.registered_at_height
                && binding.network_id == archive.staging_receipt.payload.binding.network_id
                && binding.bundle_digest == archive.commitment.bundle_digest
                && binding.descriptor_digest == archive.commitment.descriptor_digest
                && binding.source_tree_digest == archive.commitment.source_tree_digest
                && binding.semantic_release_manifest_digest
                    == archive
                        .staging_receipt
                        .payload
                        .binding
                        .semantic_release_manifest_digest)
                .then(|| record.attestation.reference())
        })
        .collect::<Option<Vec<_>>>();
    references.is_some_and(|references| {
        musubi_provider_bundle_attestation_set_digest_v1(
            archive.archive_id,
            location.replication_order,
            &references,
        )
        .is_ok_and(|digest| digest == location.provider_attestation_set_digest)
    })
}
fn current_readback_target_matches(
    archive: &MusubiArchiveRecordV1,
    expected: &MusubiArchiveLocationV1,
    current: Option<&MusubiArchiveLocationV1>,
    owner: Option<&AccountId>,
    attestation: Option<&MusubiProviderBundleAttestationRecordV1>,
) -> bool {
    let (Some(owner), Some(attestation)) = (owner, attestation) else {
        return false;
    };
    let binding = &attestation.attestation.payload.binding;
    archive
        .location_ids
        .binary_search(&expected.location_id)
        .is_ok()
        && current == Some(expected)
        && attestation.validate().is_ok()
        && attestation.attestation.verify(binding).is_ok()
        && attestation.key.archive_id == archive.archive_id
        && attestation.key.replication_order == expected.replication_order
        && expected
            .providers
            .binary_search(&attestation.key.provider_id)
            .is_ok()
        && attestation.registered_at_height >= archive.registered_at_height
        && binding.network_id == archive.staging_receipt.payload.binding.network_id
        && binding.completed_by == *owner
        && binding.completion_authority.provider_owner == *owner
        && binding.archive_id == archive.archive_id
        && binding.bundle_digest == archive.commitment.bundle_digest
        && binding.descriptor_digest == archive.commitment.descriptor_digest
        && binding.source_tree_digest == archive.commitment.source_tree_digest
        && binding.semantic_release_manifest_digest
            == archive
                .staging_receipt
                .payload
                .binding
                .semantic_release_manifest_digest
}
pub(super) fn validate_finalized_block_wire(
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
    if block.validate_output_merkle_cache().is_err() {
        return false;
    }
    let mut found = false;
    for (input_index, entrypoint) in block.network_entrypoints().enumerate() {
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => {
                if *reveal.signed_transaction().hash().as_ref() == query.transaction_hash {
                    return false;
                }
                continue;
            }
            TransactionEntrypoint::SealedCommitment(_) => {
                continue;
            }
        };
        if *transaction.hash().as_ref() != query.transaction_hash {
            continue;
        }
        let Some((_, output)) = u32::try_from(input_index)
            .ok()
            .and_then(|index| block.network_output_at(index))
        else {
            return false;
        };
        if found
            || output.result.is_err()
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
pub(crate) mod tests {
    use super::*;
    use iroha_core::{
        block::{BlockBuilder, ValidBlock},
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_crypto::{
        Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf, bls_normal_pop_prove,
    };
    use iroha_data_model::{
        Registrable as _, ValidationFail,
        account::{Account, AccountId},
        asset::AssetDefinition,
        block::{
            BlockHeader, SignedBlock,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                QuorumCertificate, ValidatorPower,
            },
            execution_output::{
                ExecutionOutputV1, InvocationCompletionV1, NetworkExecutionOutputV1,
                PipelineEventPositionV1, PipelineExecutionOutputV1, PipelineInvocationV1,
                TimeExecutionOutputV1, TimeInvocationV1, TriggerUseV1,
            },
        },
        domain::Domain,
        isi::{
            InstructionBox,
            kagemusha_v1::{
                KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1,
            },
            musubi::RegisterMusubiArchiveV1,
        },
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1,
            MusubiArchiveLocationStateV1, MusubiContentDigestV1,
            MusubiProviderBundleAttestationSetDigestV1, MusubiProviderBundleVerificationApprovalV1,
            MusubiProviderBundleVerificationAttestationV1,
            MusubiProviderBundleVerificationBindingV1, MusubiProviderBundleVerificationPayloadV1,
            MusubiSeedIngressReceiptApprovalV1, MusubiSeedIngressReceiptBindingV1,
            MusubiSeedIngressReceiptPayloadV1, MusubiSeedIngressReceiptV1,
            MusubiSemanticReleaseDigestV1, MusubiVerificationLockDigestV1,
        },
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ChunkerProfileHandle, ManifestDigest, ManifestRootCid,
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
                ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
            },
        },
        transaction::{
            DataTriggerSequence, FeePaymentIntent, SignedTransaction, TransactionBuilder,
            TransactionResultInner, error::TransactionRejectionReason,
        },
    };
    use iroha_model_base::chain::ChainId;
    use iroha_model_base::peer::PeerId;
    use iroha_primitives::time::TimeSource;
    use std::{borrow::Cow, num::NonZeroU64, sync::Arc, time::Duration};
    struct RegistrationMaterial {
        network_id: NetworkId,
        publisher_key: KeyPair,
        archive: MusubiArchiveRecordV1,
    }
    pub(crate) struct ReaderFixture {
        pub(crate) reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
        pub(crate) state: Arc<State>,
        pub(crate) query: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        pub(crate) archive: MusubiArchiveRecordV1,
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
        registration_material_at(network_id(0x15), 1)
    }
    fn registration_material_at(
        network_id: NetworkId,
        registered_at_height: u64,
    ) -> RegistrationMaterial {
        registration_material_at_with_commitment(
            network_id,
            registered_at_height,
            archive_commitment(),
            MusubiSemanticReleaseDigestV1::new([0x34; 32]),
        )
    }
    fn registration_material_at_with_commitment(
        network_id: NetworkId,
        registered_at_height: u64,
        commitment: MusubiArchiveCommitmentV1,
        semantic_digest: MusubiSemanticReleaseDigestV1,
    ) -> RegistrationMaterial {
        let publisher_key = keypair(0x31);
        let publisher = AccountId::new(publisher_key.public_key().clone());
        let broker_key = keypair(0x32);
        let broker = AccountId::new(broker_key.public_key().clone());
        let binding = MusubiSeedIngressReceiptBindingV1 {
            network_id,
            publisher: publisher.clone(),
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x33; 32]),
            semantic_release_manifest_digest: semantic_digest,
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
            registered_at_height,
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
    #[test]
    fn pin_outbox_high_water_requires_exact_successful_signed_advance() {
        use iroha_data_model::isi::musubi::AdvanceMusubiPinOutboxV1;
        let material = registration_material();
        let advance = AdvanceMusubiPinOutboxV1 {
            network_id: material.network_id,
            pin_authority: AccountId::new(material.publisher_key.public_key().clone()),
            session_id: [0x81; 32],
            expected_revision: 0,
            expected_inventory_digest: [0; 32],
            inventory_digest: [0x82; 32],
        };
        let transaction = signed_transaction(
            material.network_id,
            &material.publisher_key,
            vec![advance.clone().into()],
        );
        let block = signed_block_with_results(vec![transaction.clone()], None);
        let record = advance
            .recorded_high_water(block.header().height().get(), *transaction.hash().as_ref())
            .expect("canonical high-water");
        assert!(super::super::pin_outbox_finality::validate_advance_transaction(&record, &block));
        for case in 0..8 {
            let mut changed = record.clone();
            match case {
                0 => changed.version = 2,
                1 => {
                    changed.network_id = NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new([0x91; 32])),
                    );
                }
                2 => changed.pin_authority = AccountId::new(keypair(0x92).public_key().clone()),
                3 => changed.session_id = [0x93; 32],
                4 => changed.revision += 1,
                5 => changed.inventory_digest = [0x94; 32],
                6 => changed.recorded_at_height += 1,
                7 => changed.transaction_hash = [0x95; 32],
                _ => unreachable!("closed mutation matrix"),
            }
            assert!(
                !super::super::pin_outbox_finality::validate_advance_transaction(&changed, &block),
                "mutation {case} must fail",
            );
        }
        let rejected = signed_block_with_results(vec![transaction], Some(0));
        assert!(
            !super::super::pin_outbox_finality::validate_advance_transaction(&record, &rejected)
        );
    }
    #[test]
    fn pin_outbox_high_water_reader_binds_network_and_reports_absence_without_invention() {
        let fixture = reader_fixture();
        let reader =
            super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReaderV1::new(
                fixture.query.network_id,
                Arc::clone(&fixture.state),
            )
            .expect("same-network finalized reader");
        assert!(
            reader
                .read_current(&fixture.archive.registered_by)
                .expect("no high-water has been submitted")
                .is_none()
        );
        let anchor = reader
            .read_current_anchor(&fixture.archive.registered_by)
            .expect("the empty signer lineage still has an authenticated local tip");
        assert_eq!(anchor.network_id, fixture.query.network_id);
        assert_eq!(anchor.tip_height, fixture.query.snapshot.finalized_height);
        assert_eq!(
            anchor.tip_block_hash,
            fixture.query.snapshot.finalized_block_hash
        );
        assert!(anchor.high_water.is_none());
        let uncommitted_height = anchor.tip_height + 1;
        let synthetic_header = BlockHeader::new(
            NonZeroU64::new(uncommitted_height).expect("next height is nonzero"),
            Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                anchor.tip_block_hash,
            ))),
            None,
            2_001,
            0,
        );
        fixture
            .state
            .block(synthetic_header)
            .commit_empty_block_for_testing()
            .expect("advance State only for a mismatched-tip test");
        assert_eq!(
            reader.read_current_anchor(&fixture.archive.registered_by),
            Err(super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReadErrorV1::LocallyAhead),
            "a State tip beyond durable Kura cannot yield a signer anchor",
        );
        assert!(
            super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReaderV1::new(
                network_id(0x77),
                fixture.state,
            )
            .is_err()
        );
    }
    struct StagedPinOutboxAdvance<'state> {
        state_block: iroha_core::state::StateBlock<'state>,
        valid: ValidBlock,
        finality: V2FinalityArtifact,
        high_water: iroha_data_model::musubi::MusubiPinOutboxHighWaterV1,
    }

    fn stage_pin_outbox_advance(fixture: &ReaderFixture) -> StagedPinOutboxAdvance<'_> {
        use iroha_data_model::isi::musubi::AdvanceMusubiPinOutboxV1;
        let (parent, parent_finality) = {
            let view = fixture.state.query_view();
            let height = NonZeroUsize::new(view.block_hashes().len())
                .expect("the fixture has a finalized parent");
            let parent = view
                .kura()
                .get_block(height)
                .expect("canonical parent body");
            let finality = view
                .kura()
                .v2_finality_artifact(u64::try_from(height.get()).expect("bounded height"))
                .expect("read parent finality")
                .expect("parent is finalized");
            (parent, finality)
        };
        let advance = AdvanceMusubiPinOutboxV1 {
            network_id: fixture.query.network_id,
            pin_authority: fixture.archive.registered_by.clone(),
            session_id: [0x81; 32],
            expected_revision: 0,
            expected_inventory_digest: [0; 32],
            inventory_digest: [0x82; 32],
        };
        let transaction = signed_transaction(
            fixture.query.network_id,
            &keypair(0x31),
            vec![advance.clone().into()],
        );
        let transaction_hash = *transaction.hash().as_ref();
        let (_block_clock, block_time) = TimeSource::new_mock(Duration::from_millis(2_500));
        let new_block = BlockBuilder::new_with_time_source(
            vec![iroha_core::tx::AcceptedTransaction::new_unchecked(
                Cow::Owned(transaction),
            )],
            block_time,
        )
        .chain(0, Some(parent.as_ref()))
        .sign(finality_keypairs()[0].private_key())
        .unpack(|_| {});
        let mut proposal = SignedBlock::from(new_block);
        for (index, key) in finality_keypairs().iter().enumerate().take(3).skip(1) {
            proposal
                .add_signature(iroha_data_model::block::BlockSignature::new(
                    u64::try_from(index).expect("bounded validator index"),
                    SignatureOf::from_hash(key.private_key(), proposal.hash()),
                ))
                .expect("sign exact pin-outbox proposal");
        }
        let mut state_block = fixture.state.block(proposal.header());
        let valid = ValidBlock::validate_unchecked(proposal, &mut state_block).unpack(|_| {});
        assert!(
            valid
                .as_ref()
                .network_output_at(0)
                .is_some_and(|(_, output)| output.result.is_ok()),
            "the real native high-water advance must execute successfully",
        );
        let execution = state_block
            .execution_commitment_for_testing(&valid)
            .expect("actual high-water execution commitment");
        let mut context = parent_finality.height_context;
        context.height = valid.as_ref().header().height().get();
        context.parent_commit_qc = Some(parent_finality.commit_qc);
        let finality = sign_finality_artifact(valid.as_ref(), context, execution);
        let high_water = advance
            .recorded_high_water(valid.as_ref().header().height().get(), transaction_hash)
            .expect("exact successful high-water record");
        StagedPinOutboxAdvance {
            state_block,
            valid,
            finality,
            high_water,
        }
    }

    #[test]
    fn pin_outbox_high_water_reader_authenticates_executed_finalized_advance() {
        let fixture = reader_fixture();
        let StagedPinOutboxAdvance {
            state_block,
            valid,
            finality,
            high_water,
        } = stage_pin_outbox_advance(&fixture);
        let (committed, _) = fixture
            .state
            .publish_preverified_fixture_block_for_testing(state_block, valid, finality)
            .expect("publish the actual signed high-water execution through V2 finality");
        let reader =
            super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReaderV1::new(
                fixture.query.network_id,
                Arc::clone(&fixture.state),
            )
            .expect("same-network finalized reader");
        let anchor = reader
            .read_current_anchor(&fixture.archive.registered_by)
            .expect("authenticate the real State record and its exact successful transaction");
        assert_eq!(anchor.network_id, fixture.query.network_id);
        assert_eq!(
            anchor.tip_height,
            committed.as_ref().header().height().get()
        );
        assert_eq!(anchor.tip_block_hash, *committed.as_ref().hash().as_ref());
        assert_eq!(anchor.high_water, Some(high_water.clone()));
        assert_eq!(
            reader.read_current(&fixture.archive.registered_by),
            Ok(Some(high_water))
        );
    }

    #[test]
    fn pin_outbox_high_water_reader_rejects_kura_ahead_of_state() {
        let fixture = reader_fixture();
        let StagedPinOutboxAdvance {
            state_block, valid, ..
        } = stage_pin_outbox_advance(&fixture);
        let topology = Topology::new(
            finality_keypairs()
                .iter()
                .map(|key| PeerId::new(key.public_key().clone())),
        );
        let committed = valid
            .commit(&topology)
            .unpack(|_| {})
            .expect("retain the signed executed body for the mismatched-tip negative control");
        drop(state_block);
        let state_height = {
            let view = fixture.state.query_view();
            let state_height = view.block_hashes().len();
            view.kura()
                .store_block(committed)
                .expect("advance durable Kura without publishing the State overlay");
            assert_eq!(
                view.kura()
                    .exact_durable_blocks_count()
                    .expect("durable height"),
                state_height + 1
            );
            state_height
        };
        assert_eq!(
            fixture.state.query_view().block_hashes().len(),
            state_height
        );
        let reader =
            super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReaderV1::new(
                fixture.query.network_id,
                Arc::clone(&fixture.state),
            )
            .expect("same-network finalized reader");
        assert_eq!(reader.read_current_anchor(&fixture.archive.registered_by),
            Err(super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReadErrorV1::LocallyAhead),
            "an absent State record cannot bypass a durable Kura tip ahead of State");
    }
    fn signed_proposal(transactions: Vec<SignedTransaction>) -> SignedBlock {
        let accepted = transactions
            .into_iter()
            .map(|transaction| {
                iroha_core::tx::AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
            })
            .collect();
        let block: SignedBlock = BlockBuilder::new(accepted)
            .chain(0, None)
            .sign(keypair(0x41).private_key())
            .unpack(|_| {})
            .into();
        block
            .validate_proposal_commitments()
            .expect("fixture proposal commits its actual inputs");
        block
    }

    fn network_outputs(
        block: &SignedBlock,
        rejected_index: Option<usize>,
    ) -> Vec<ExecutionOutputV1> {
        block
            .network_entrypoints()
            .enumerate()
            .map(|(index, _)| {
                ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                    input_index: u32::try_from(index).expect("bounded fixture input index"),
                    result: (if rejected_index == Some(index) {
                        TransactionResultInner::Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted(
                                "rejected registration fixture".to_owned(),
                            ),
                        ))
                    } else {
                        TransactionResultInner::Ok(DataTriggerSequence::default())
                    })
                    .into(),
                    completions: Vec::new(),
                })
            })
            .collect()
    }

    // This helper installs structural negative-test evidence, not execution authority.
    // Positive finalized-reader fixtures below still execute the real registration.
    fn install_fixture_outputs(
        block: &mut SignedBlock,
        outputs: Vec<ExecutionOutputV1>,
        committed_fragments: u64,
    ) -> Result<(), iroha_data_model::block::SetExecutionOutputsError> {
        let header = block.header();
        let signatures = block.signatures().cloned().collect::<Vec<_>>();
        let installed = block.set_execution_outputs(
            outputs,
            committed_fragments,
            if block.has_results() {
                block.fastpq_transcripts().clone()
            } else {
                Default::default()
            },
            block.axt_envelopes().unwrap_or_default().to_vec(),
            block.axt_policy_snapshot().cloned().unwrap_or_default(),
            block
                .axt_transitioned_dataspaces()
                .cloned()
                .unwrap_or_default(),
            block.lane_finality_statements().to_vec(),
            &iroha_data_model::parameter::ExecutionOutputPolicyV1::bootstrap().limits(),
        );
        assert_eq!(
            block.header(),
            header,
            "outputs cannot rewrite proposal commitments"
        );
        assert_eq!(block.signatures().cloned().collect::<Vec<_>>(), signatures);
        installed
    }

    fn signed_block_with_results(
        transactions: Vec<SignedTransaction>,
        rejected_index: Option<usize>,
    ) -> SignedBlock {
        let mut block = signed_proposal(transactions);
        let outputs = network_outputs(&block, rejected_index);
        // Every successful fixture input has exactly one structural execution fragment.
        let fragments =
            u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
        install_fixture_outputs(&mut block, outputs, fragments)
            .expect("fixture typed Network outputs match their immutable inputs");
        block
    }

    fn structural_registration_callbacks(
        archive: &MusubiArchiveRecordV1,
    ) -> Vec<ExecutionOutputV1> {
        use iroha_data_model::{
            events::{
                time::{TimeEvent, TimeInterval},
                trigger_completed::TriggerCompletedOutcome,
            },
            transaction::signed::ExecutionStep,
            trigger::DataTriggerStep,
        };
        let callback = |name: &str| {
            let trigger = TriggerUseV1 {
                trigger_id: name.parse().unwrap(),
                registered_at_height: 0,
                action_hash: Hash::new(name.as_bytes()),
            };
            let result = Ok(vec![DataTriggerStep {
                id: trigger.trigger_id.clone(),
                instructions: ExecutionStep(vec![registration_instruction(archive).into()].into()),
            }])
            .into();
            let completions = vec![InvocationCompletionV1 {
                callback_index: 0,
                trigger_id: trigger.trigger_id.clone(),
                outcome: TriggerCompletedOutcome::Success,
            }];
            (trigger, result, completions)
        };
        // Public descriptors only establish distinct structural owners here.
        // Exact executed-wire finality remains mandatory in the production reader.
        let (trigger, result, completions) = callback("registration-pipeline");
        let pipeline = ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
            invocation: PipelineInvocationV1 {
                event: PipelineEventPositionV1::BlockApproved,
                candidate_index: 0,
                trigger,
            },
            result,
            failure_root: None,
            completions,
        });
        let (trigger, result, completions) = callback("registration-time");
        let time = ExecutionOutputV1::Time(TimeExecutionOutputV1 {
            invocation: TimeInvocationV1 {
                schedule_index: 0,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: 0,
                        length_ms: 1,
                    },
                },
                trigger,
            },
            result,
            failure_root: None,
            completions,
        });
        vec![pipeline, time]
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
    fn genesis_proposal(
        parameters: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters,
    ) -> SignedBlock {
        use iroha_data_model::isi::kagemusha_v1::{
            KagemushaMintFinalityAuthorityGenerationTemplateV1,
            KagemushaMintFinalityGenesisParametersV1,
        };
        use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};

        iroha_genesis::init_instruction_registry();
        let topology = finality_keypairs()
            .iter()
            .map(|key| {
                GenesisTopologyEntry::new(
                    PeerId::new(key.public_key().clone()),
                    bls_normal_pop_prove(key.private_key()).expect("genesis validator PoP"),
                )
            })
            .collect::<Vec<_>>();
        let validators = topology
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let seed = 0xC0_u8 + u8::try_from(index).unwrap();
                iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                    &[seed; 32],
                    0,
                    entry.peer.clone(),
                )
                .expect("genesis mint-finality validator keys")
            })
            .collect();
        GenesisBuilder::new_without_executor(ChainId::from("musubi-finality-reader-test"), ".")
            .set_topology(topology)
            .with_sumeragi_v2_context_parameters(parameters)
            .with_kagemusha_mint_finality_genesis_parameters(
                KagemushaMintFinalityGenesisParametersV1 {
                    authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                        version: KAGEMUSHA_CHAIN_VERSION_V1,
                        generation: 0,
                        validators,
                    },
                },
            )
            .build_raw()
            .expect("complete genesis with exact four-validator authority")
            .with_consensus_meta()
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
                &keypair(0x42),
                None,
                None,
                100,
            )
            .expect("sign deterministic genesis proposal")
            .0
    }
    fn seeded_world(material: &RegistrationMaterial) -> World {
        let publisher = material.archive.registered_by.clone();
        let account = Account::new(publisher.clone()).build(&publisher);
        let genesis_account = AccountId::new(keypair(0x42).public_key().clone());
        let mut world = World::with(
            [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account)],
            [
                account,
                Account::new(genesis_account.clone()).build(&genesis_account),
            ],
            std::iter::empty::<AssetDefinition>(),
        );
        let binding = &material.archive.staging_receipt.payload.binding;
        world
            .provider_owners_mut_for_testing()
            .insert(binding.seed_provider, binding.ingress_broker.clone());
        world
    }
    pub(crate) fn reader_fixture() -> ReaderFixture {
        reader_fixture_with_finality(true)
    }
    fn fixture_state(material: &RegistrationMaterial) -> (Arc<State>, Arc<Kura>) {
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
            seeded_world(material),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            ChainId::from("musubi-finality-reader-test"),
            material.network_id,
        ));
        let nexus = state.nexus_snapshot();
        state.install_lane_manifests_for_testing(&Arc::new(
            iroha_core::governance::manifest::LaneManifestRegistry::empty()
                .rebind(&nexus.lane_catalog, &nexus.governance),
        ));
        (state, kura)
    }
    fn stage_genesis<'state>(
        state: &'state State,
        genesis: SignedBlock,
        topology: &Topology,
    ) -> (ValidBlock, Box<iroha_core::state::StateBlock<'state>>) {
        let genesis_account = AccountId::new(keypair(0x42).public_key().clone());
        let (_validation_clock, validation_time) =
            TimeSource::new_mock(Duration::from_millis(1_500));
        ValidBlock::validate_signed_genesis(
            genesis,
            topology,
            &genesis_account,
            &validation_time,
            state,
            ConsensusMode::Permissioned,
        )
        .unpack(|_| {})
        .unwrap_or_else(|(block, error)| {
            panic!(
                "fixture genesis admission failed: {error}; outputs={:?}",
                block.execution_outputs()
            )
        })
    }
    fn reader_fixture_with_finality(store_finality: bool) -> ReaderFixture {
        reader_fixture_with_finality_using(store_finality, None)
    }
    fn reader_fixture_with_finality_using(
        store_finality: bool,
        seed: Option<(MusubiArchiveCommitmentV1, MusubiSemanticReleaseDigestV1)>,
    ) -> ReaderFixture {
        let topology = Topology::new(
            finality_keypairs()
                .iter()
                .map(|key| PeerId::new(key.public_key().clone())),
        );
        let mut parameters =
            iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(
            );
        {
            // Signing binds the policy computed by actual genesis execution;
            // the provisional overlay is dropped without State or Kura publication.
            let provisional = genesis_proposal(parameters);
            let provisional_network = NetworkId::from_genesis_hash(provisional.hash());
            let material = seed.as_ref().map_or_else(
                || registration_material_at(provisional_network, 2),
                |(commitment, semantic)| {
                    registration_material_at_with_commitment(
                        provisional_network,
                        2,
                        commitment.clone(),
                        semantic.clone(),
                    )
                },
            );
            let (state, _) = fixture_state(&material);
            let (_, staged) = stage_genesis(&state, provisional, &topology);
            parameters.nexus_amx_context_hash =
                *iroha_core::sumeragi::staged_genesis_nexus_amx_context_hash(&staged).as_ref();
            parameters.execution_policy_hash =
                *iroha_core::sumeragi::staged_genesis_execution_policy_hash(&staged)
                    .expect("staged genesis execution policy")
                    .as_ref();
        }
        let genesis = genesis_proposal(parameters);
        let finalized_network = NetworkId::from_genesis_hash(genesis.hash());
        let material = seed.as_ref().map_or_else(
            || registration_material_at(finalized_network, 2),
            |(commitment, semantic)| {
                registration_material_at_with_commitment(
                    finalized_network,
                    2,
                    commitment.clone(),
                    semantic.clone(),
                )
            },
        );
        let registration_transaction = successful_registration_transaction(&material);
        let transaction_hash = *registration_transaction.hash().as_ref();
        let (state, kura) = fixture_state(&material);
        let (genesis, genesis_finality) = {
            let signed_genesis = iroha_genesis::GenesisBlock(genesis.clone());
            let (valid_genesis, genesis_state) = stage_genesis(&state, genesis, &topology);
            let bootstrap = iroha_core::sumeragi::freeze_staged_genesis_v2(
                &signed_genesis,
                &genesis_state,
                ConsensusMode::Permissioned,
            )
            .expect("genesis context derives from exact signed and staged policy");
            let genesis_execution = genesis_state
                .execution_commitment_for_testing(&valid_genesis)
                .expect(
                    "genesis commitment derives from the exact retained witness and output seal",
                );
            let genesis_finality = sign_finality_artifact(
                valid_genesis.as_ref(),
                bootstrap.context().clone(),
                genesis_execution,
            );
            iroha_core::sumeragi::validate_signed_genesis_v2_authority(
                &signed_genesis,
                &genesis_finality.height_context,
                &genesis_finality.validator_set_pops,
            )
            .expect("finality preserves signed genesis authority");
            let (genesis, _) = state
                .publish_preverified_fixture_block_for_testing(
                    *genesis_state,
                    valid_genesis,
                    genesis_finality.clone(),
                )
                .expect("publish genesis through exact verified V2 finality owner");
            (genesis, genesis_finality)
        };
        let (_block_time_handle, block_time_source) =
            TimeSource::new_mock(Duration::from_millis(1_500));
        let new_block = BlockBuilder::new_with_time_source(
            vec![iroha_core::tx::AcceptedTransaction::new_unchecked(
                Cow::Owned(registration_transaction),
            )],
            block_time_source,
        )
        .chain(0, Some(genesis.as_ref()))
        .sign(finality_keypairs()[0].private_key())
        .unpack(|_| {});
        let mut proposal = SignedBlock::from(new_block);
        for (index, key) in finality_keypairs().iter().enumerate().take(3).skip(1) {
            proposal
                .add_signature(iroha_data_model::block::BlockSignature::new(
                    u64::try_from(index).unwrap(),
                    SignatureOf::from_hash(key.private_key(), proposal.hash()),
                ))
                .expect("attach validator signatures before the exact output wire is sealed");
        }
        let mut state_block = state.block(proposal.header());
        let valid = ValidBlock::validate_unchecked(proposal, &mut state_block).unpack(|_| {});
        let execution = state_block
            .execution_commitment_for_testing(&valid)
            .expect("registration commitment derives from exact sealed execution");
        let canonical_block = if store_finality {
            let mut context = genesis_finality.height_context.clone();
            context.height = valid.as_ref().header().height().get();
            context.parent_commit_qc = Some(genesis_finality.commit_qc.clone());
            let finality = sign_finality_artifact(valid.as_ref(), context, execution);
            let (committed, _) = state
                .publish_preverified_fixture_block_for_testing(state_block, valid, finality)
                .expect("publish registration through exact verified V2 finality owner");
            let canonical = committed.as_ref().clone();
            assert!(
                canonical
                    .network_output_at(0)
                    .is_some_and(|(_, output)| output.result.is_ok())
            );
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
            replace_current_archive(&state, *canonical.hash().as_ref(), &material.archive);
            canonical
        } else {
            // An uncertified Kura body is deliberately not an executable State
            // publication. The synthetic empty membership only lets the reader
            // reach its missing-finality rejection at the claimed height.
            let committed = valid
                .commit(&topology)
                .unpack(|_| {})
                .expect("uncertified negative fixture retains signed body");
            let canonical = committed.as_ref().clone();
            kura.store_block(Arc::new(canonical.clone()))
                .expect("store uncertified negative fixture body");
            drop(state_block);
            state
                .block(canonical.header())
                .commit_empty_block_for_testing()
                .expect("index synthetic negative fixture height");
            canonical
        };
        let query = MusubiPublicationFinalizedArchiveRegistrationQueryV1 {
            version: 1,
            network_id: material.network_id,
            transaction_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: material.archive.registered_at_height,
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
        replace_current_archive_with_location(state, canonical_hash, archive, None, None, None);
    }
    fn replace_current_archive_with_location(
        state: &State,
        canonical_hash: [u8; 32],
        archive: &MusubiArchiveRecordV1,
        location: Option<&MusubiArchiveLocationV1>,
        attestation: Option<&MusubiProviderBundleAttestationRecordV1>,
        owner_override: Option<(ProviderId, AccountId)>,
    ) {
        let header = BlockHeader::new(
            NonZeroU64::new(archive.registered_at_height + 1).expect("nonzero fixture height"),
            Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                canonical_hash,
            ))),
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
        if let Some(location) = location {
            transaction
                .world_mut_for_testing()
                .musubi_archive_locations_mut_for_testing()
                .insert(location.key(), location.clone());
        }
        if let Some(attestation) = attestation {
            transaction
                .world_mut_for_testing()
                .musubi_provider_bundle_attestations_mut_for_testing()
                .insert(attestation.key, attestation.clone());
        }
        if let Some((provider, owner)) = owner_override {
            transaction
                .world_mut_for_testing()
                .provider_owners_mut_for_testing()
                .insert(provider, owner);
        }
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit current archive substitution");
    }
    fn readback_target(
        archive: &MusubiArchiveRecordV1,
    ) -> (
        MusubiArchiveLocationV1,
        MusubiProviderBundleAttestationRecordV1,
    ) {
        let mut location = MusubiArchiveLocationV1 {
            location_id: MusubiArchiveLocationIdV1::new([0x61; 32]),
            archive_id: archive.archive_id,
            pin_manifest: ManifestDigest::new([0x62; 32]),
            replication_order: ReplicationOrderId::new([0x63; 32]),
            providers: vec![archive.staging_receipt.payload.binding.seed_provider],
            provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1::new(
                [0x64; 32],
            ),
            renew_after_epoch: 1,
            expires_at_epoch: 2,
            finalized_height: archive.registered_at_height,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        };
        let provider_key = keypair(0x32);
        let owner = AccountId::new(provider_key.public_key().clone());
        let binding = MusubiProviderBundleVerificationBindingV1 {
            network_id: archive.staging_receipt.payload.binding.network_id,
            provider_id: location.providers[0],
            completed_by: owner.clone(),
            completion_authority: ProviderIngestCompletionAuthorityV1::new(
                owner,
                ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [0x71; 32],
                    revision: 1,
                    predecessor_digest: None,
                    policy_digest: [0x72; 32],
                },
            ),
            replication_order: location.replication_order,
            assignment_revision: 1,
            completion_epoch: 1,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 1,
                block_hash: [0x73; 32],
            },
            archive_id: archive.archive_id,
            bundle_digest: archive.commitment.bundle_digest,
            descriptor_digest: archive.commitment.descriptor_digest,
            semantic_release_manifest_digest: archive
                .staging_receipt
                .payload
                .binding
                .semantic_release_manifest_digest,
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0x74; 32]),
            source_tree_digest: archive.commitment.source_tree_digest,
        };
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: 1,
            binding,
        };
        let attestation = MusubiProviderBundleVerificationAttestationV1 {
            approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                public_key: provider_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    provider_key.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign provider attestation"),
            }],
            payload,
        };
        location.provider_attestation_set_digest =
            musubi_provider_bundle_attestation_set_digest_v1(
                archive.archive_id,
                location.replication_order,
                &[attestation.reference()],
            )
            .expect("provider-attestation set digest");
        let record = MusubiProviderBundleAttestationRecordV1 {
            key: attestation.key(),
            attestation_digest: attestation.digest(),
            attestation,
            registered_by: archive.registered_by.clone(),
            registered_at_height: archive.registered_at_height,
        };
        record
            .validate()
            .expect("valid signed provider attestation");
        (location, record)
    }
    #[test]
    fn current_readback_target_requires_exact_location_and_registered_provider() {
        let mut archive = registration_material().archive;
        let (location, record) = readback_target(&archive);
        let owner = &record.attestation.payload.binding.completed_by;
        archive.location_ids.push(location.location_id);
        assert!(current_readback_target_matches(
            &archive,
            &location,
            Some(&location),
            Some(owner),
            Some(&record),
        ));
        let mut changed = location.clone();
        changed.pin_manifest = ManifestDigest::new([0x65; 32]);
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            Some(&changed),
            Some(owner),
            Some(&record),
        ));
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            None,
            Some(owner),
            Some(&record),
        ));
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            Some(&location),
            None,
            Some(&record),
        ));
        let changed_owner = AccountId::new(keypair(0x35).public_key().clone());
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            Some(&location),
            Some(&changed_owner),
            Some(&record),
        ));
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            Some(&location),
            Some(owner),
            None,
        ));
        archive.location_ids.clear();
        assert!(!current_readback_target_matches(
            &archive,
            &location,
            Some(&location),
            Some(owner),
            Some(&record),
        ));
    }
    #[test]
    fn readback_target_rechecks_current_location_after_mutation_and_reader_restart() {
        let fixture = reader_fixture();
        let (location, record) = readback_target(&fixture.archive);
        let mut archive = fixture.archive.clone();
        archive.location_ids.push(location.location_id);
        replace_current_archive_with_location(
            &fixture.state,
            fixture.query.snapshot.finalized_block_hash,
            &archive,
            Some(&location),
            Some(&record),
            None,
        );
        let provider = location.providers[0];
        fixture
            .reader
            .validate_current_readback_target(&fixture.query, &location, provider)
            .expect("exact current finalized target");

        let mut changed = location.clone();
        changed.pin_manifest = ManifestDigest::new([0x65; 32]);
        replace_current_archive_with_location(
            &fixture.state,
            fixture.query.snapshot.finalized_block_hash,
            &archive,
            Some(&changed),
            Some(&record),
            None,
        );
        assert_eq!(
            fixture
                .reader
                .validate_current_readback_target(&fixture.query, &location, provider),
            Err(invalid()),
        );
        let reopened = MusubiPublicationFinalizedArchiveRegistrationReaderV1::new(
            fixture.query.network_id,
            Arc::clone(&fixture.state),
        )
        .expect("reader restart uses the same authoritative State handle");
        assert_eq!(
            reopened.validate_current_readback_target(&fixture.query, &location, provider),
            Err(invalid()),
        );
        let replacement_owner = AccountId::new(keypair(0x36).public_key().clone());
        replace_current_archive_with_location(
            &fixture.state,
            fixture.query.snapshot.finalized_block_hash,
            &archive,
            Some(&location),
            Some(&record),
            Some((provider, replacement_owner)),
        );
        assert_eq!(
            reopened.validate_current_readback_target(&fixture.query, &location, provider),
            Err(invalid()),
        );
        let mut substituted_attestation = record.clone();
        substituted_attestation
            .attestation
            .payload
            .binding
            .bundle_digest = MusubiContentDigestV1::new([0x75; 32]);
        replace_current_archive_with_location(
            &fixture.state,
            fixture.query.snapshot.finalized_block_hash,
            &archive,
            Some(&location),
            Some(&substituted_attestation),
            Some((
                provider,
                record.attestation.payload.binding.completed_by.clone(),
            )),
        );
        assert_eq!(
            reopened.validate_current_readback_target(&fixture.query, &location, provider),
            Err(invalid()),
        );
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
    #[cfg(unix)]
    #[test]
    fn storage_preflight_never_dispatches_unfinalized_registration() {
        use super::super::storage_coordination::FinalizedRegistrationCheckedStorageBackendV1;
        use iroha_musubi_service::{
            MusubiFinalizedArchiveRegistrationEvidenceV1, MusubiPublicationServiceBackendErrorV1,
            MusubiStorageCoordinationBackendV1, MusubiStorageCoordinationRequestV1,
            MusubiStorageCoordinationResponseV1,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct MutationProbe(Arc<AtomicUsize>);
        impl MusubiStorageCoordinationBackendV1 for MutationProbe {
            fn verify_current_registration(
                &self,
                _request: &MusubiStorageCoordinationRequestV1,
            ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
                Err(MusubiPublicationServiceBackendErrorV1::Retryable)
            }

            fn coordinate_storage(
                &mut self,
                _request: &MusubiStorageCoordinationRequestV1,
            ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1>
            {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err(MusubiPublicationServiceBackendErrorV1::Retryable)
            }
        }
        fn request(fixture: &ReaderFixture) -> MusubiStorageCoordinationRequestV1 {
            let query = &fixture.query;
            let archive = &fixture.archive;
            MusubiStorageCoordinationRequestV1 {
                version: 1,
                operation_id: [0x81; 32],
                generation: 1,
                prior_location_ids: Vec::new(),
                network_id: query.network_id,
                publisher: archive.registered_by.clone(),
                commitment: archive.commitment.clone(),
                verification_lock_digest: MusubiVerificationLockDigestV1::new([0x74; 32]),
                staging_receipt: archive.staging_receipt.clone(),
                expected_policy_revision: query.expected_policy_revision,
                finalized_registration: MusubiFinalizedArchiveRegistrationEvidenceV1 {
                    version: 1,
                    network_id: query.network_id,
                    transaction_hash: query.transaction_hash,
                    snapshot: query.snapshot,
                    registration: query.registration.clone(),
                },
            }
        }

        let fixture = reader_fixture();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut checked = FinalizedRegistrationCheckedStorageBackendV1::new(
            fixture.reader.clone(),
            Box::new(MutationProbe(Arc::clone(&calls))),
        );
        let exact = request(&fixture);
        exact.validate().expect("canonical coordination request");
        assert_eq!(checked.verify_current_registration(&exact), Ok(()));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            checked.coordinate_storage(&exact),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable),
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let mut wrong_transaction = exact.clone();
        wrong_transaction.finalized_registration.transaction_hash = [0x91; 32];
        assert_eq!(
            checked.coordinate_storage(&wrong_transaction),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        let mut wrong_policy = exact.clone();
        wrong_policy.expected_policy_revision += 1;
        assert_eq!(
            checked.coordinate_storage(&wrong_policy),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        let mut foreign_network = exact.clone();
        foreign_network.network_id = network_id(0x67);
        assert_eq!(
            checked.coordinate_storage(&foreign_network),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        let mut locally_ahead = exact.clone();
        locally_ahead
            .finalized_registration
            .snapshot
            .finalized_height += 1;
        assert_eq!(
            checked.coordinate_storage(&locally_ahead),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable),
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let uncertified = reader_fixture_with_finality(false);
        let mut checked_uncertified = FinalizedRegistrationCheckedStorageBackendV1::new(
            uncertified.reader.clone(),
            Box::new(MutationProbe(Arc::clone(&calls))),
        );
        assert_eq!(
            checked_uncertified.coordinate_storage(&request(&uncertified)),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
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
        let pin_reader =
            super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReaderV1::new(
                fixture.query.network_id,
                Arc::clone(&fixture.state),
            )
            .expect("same-network pin-outbox reader");
        assert_eq!(
            pin_reader.read_current_anchor(&fixture.archive.registered_by),
            Err(super::super::pin_outbox_finality::MusubiPublicationPinOutboxHighWaterReadErrorV1::Invalid),
            "an absent high-water cannot bypass missing tip finality",
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
        let material = registration_material_at(
            fixture.query.network_id,
            fixture.archive.registered_at_height,
        );
        let transaction = successful_registration_transaction(&material);
        let mut query = fixture.query.clone();
        query.transaction_hash = *transaction.hash().as_ref();
        let mut duplicate = signed_proposal(vec![transaction.clone(), transaction.clone()]);
        let duplicate_before = duplicate.canonical_resultless_proposal();
        let duplicate_outputs = network_outputs(&duplicate, None);
        assert!(install_fixture_outputs(&mut duplicate, duplicate_outputs, 2).is_err());
        assert_eq!(duplicate.canonical_resultless_proposal(), duplicate_before);
        assert!(
            !duplicate.has_results(),
            "duplicate execution calls cannot acquire outputs"
        );
        assert!(!validate_registration_transaction(&query, &duplicate));
        let rejected = signed_block_with_results(vec![transaction], Some(0));
        assert!(!validate_registration_transaction(&query, &rejected));
    }
    #[test]
    fn registration_joins_only_network_success_with_pipeline_and_time_outputs() {
        let material = registration_material();
        let transaction = successful_registration_transaction(&material);
        let mut block = signed_block_with_results(vec![transaction.clone()], None);
        let query = MusubiPublicationFinalizedArchiveRegistrationQueryV1 {
            version: 1,
            network_id: material.network_id,
            transaction_hash: *transaction.hash().as_ref(),
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 1,
                finalized_block_hash: *block.hash().as_ref(),
                index_revision: 1,
            },
            registration: material.archive.registration_projection(),
            expected_policy_revision: 1,
        };
        let original_finality = finality_artifact(&block, material.network_id, None);
        let mut outputs = block.execution_outputs().to_vec();
        outputs.extend(structural_registration_callbacks(&material.archive));
        install_fixture_outputs(&mut block, outputs, 3).unwrap();
        assert_eq!(block.network_entrypoint_count(), 1);
        assert_eq!(block.execution_outputs().len(), 3);
        assert!(block.network_output_at(1).is_none());
        assert!(validate_registration_transaction(&query, &block));
        assert!(
            !validate_finalized_block_wire(
                &material.network_id,
                1,
                block.hash(),
                &block,
                &original_finality,
            ),
            "adding internal outputs must invalidate the old exact-wire finality"
        );

        let mut rejected = signed_block_with_results(vec![transaction], Some(0));
        let mut outputs = rejected.execution_outputs().to_vec();
        outputs.extend(structural_registration_callbacks(&material.archive));
        install_fixture_outputs(&mut rejected, outputs, 2).unwrap();
        assert!(
            !validate_registration_transaction(&query, &rejected),
            "successful internal registration traces cannot replace rejected Network execution"
        );

        let mut missing = signed_block_with_results(
            vec![signed_transaction(
                material.network_id,
                &material.publisher_key,
                Vec::new(),
            )],
            None,
        );
        let mut outputs = missing.execution_outputs().to_vec();
        outputs.extend(structural_registration_callbacks(&material.archive));
        install_fixture_outputs(&mut missing, outputs, 3).unwrap();
        assert!(
            !validate_registration_transaction(&query, &missing),
            "internal registration traces cannot supply a missing signed Network registration"
        );

        let before = block.encode_wire().unwrap();
        assert!(
            install_fixture_outputs(
                &mut block,
                structural_registration_callbacks(&material.archive),
                2,
            )
            .is_err(),
            "internal outputs cannot replace the required Network owner"
        );
        assert_eq!(block.encode_wire().unwrap(), before);
    }
    #[test]
    fn multi_instruction_registration_is_invalid() {
        let fixture = reader_fixture();
        let material = registration_material_at(
            fixture.query.network_id,
            fixture.archive.registered_at_height,
        );
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
        let material = registration_material_at(
            fixture.query.network_id,
            fixture.archive.registered_at_height,
        );
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
        let material = registration_material_at(
            fixture.query.network_id,
            fixture.archive.registered_at_height,
        );
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
        let height = fixture.archive.registered_at_height;
        let block = view
            .kura()
            .get_block(
                NonZeroUsize::new(usize::try_from(height).unwrap())
                    .expect("nonzero fixture height"),
            )
            .expect("fixture Kura block");
        let finality = view
            .kura()
            .v2_finality_artifact(height)
            .expect("read fixture finality")
            .expect("fixture finality exists");
        let canonical_hash = block.hash();
        assert!(validate_finalized_block_wire(
            &fixture.query.network_id,
            height,
            canonical_hash,
            &block,
            &finality,
        ));
        let mut substituted = block.as_ref().clone();
        let mut outputs = substituted.execution_outputs().to_vec();
        let ExecutionOutputV1::Network(output) = &mut outputs[0] else {
            panic!("registration fixture owns Network output zero");
        };
        assert_eq!(output.input_index, 0);
        output.result = TransactionResultInner::Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("substituted Kura result".to_owned()),
        ))
        .into();
        output.completions.clear();
        install_fixture_outputs(&mut substituted, outputs, 0)
            .expect("replace the result while retaining the consensus header hash");
        assert_eq!(substituted.hash(), canonical_hash);
        assert!(!validate_finalized_block_wire(
            &fixture.query.network_id,
            height,
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
    #[cfg(unix)]
    #[test]
    fn finalized_seed_capability_reads_exact_car_once_per_bounded_lease() {
        use super::super::{
            seed_staging::{MusubiSeedStagingBackendV1, MusubiSeedStagingErrorV1, tests::fixture},
            shared_seed_staging::SharedSeedStagingBackendV1,
        };
        use iroha_musubi_service::MusubiSeedIngressBackendV1;
        use std::{fs, os::unix::fs::PermissionsExt as _};

        let (seed_binding, commitment, plan, car) = fixture();
        let fixture = reader_fixture_with_finality_using(
            true,
            Some((commitment, seed_binding.semantic_release_manifest_digest)),
        );
        let binding = &fixture.archive.staging_receipt.payload.binding;
        let root = tempfile::tempdir().expect("private seed fixture root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("owner-only seed directory");
        let seed_root = root
            .path()
            .canonicalize()
            .expect("canonical seed fixture root");
        let seed = MusubiSeedStagingBackendV1::open(
            &seed_root,
            binding.seed_provider,
            2,
            128 * 1024 * 1024,
        )
        .expect("seed owner");
        let (mut ingress, capability) =
            SharedSeedStagingBackendV1::share(seed, fixture.reader.clone());
        ingress
            .stage_exact_car(
                [0x61; 32],
                binding,
                &fixture.archive.commitment,
                &plan,
                &car,
            )
            .expect("shared ingress stages exact finalized CAR");
        ingress
            .verify_staged_car(
                [0x61; 32],
                binding,
                &fixture.archive.commitment,
                &plan,
                &car,
            )
            .expect("shared ingress re-reads exact staged CAR");
        assert_eq!(capability.provider_id(), binding.seed_provider);
        let mut wrong_transaction = fixture.query.clone();
        wrong_transaction.transaction_hash = [0xaa; 32];
        assert_eq!(
            capability
                .read_finalized_seed(&wrong_transaction)
                .unwrap_err(),
            MusubiSeedStagingErrorV1::Invalid,
        );
        let lease = capability
            .read_finalized_seed(&fixture.query)
            .expect("exact finalized registration opens exact staged bytes");
        assert_eq!(lease.plan(), &plan);
        assert_eq!(lease.car(), car.as_slice());
        assert_eq!(
            capability.read_finalized_seed(&fixture.query).unwrap_err(),
            MusubiSeedStagingErrorV1::Capacity,
        );
        drop(lease);
        let replay = capability
            .read_finalized_seed(&fixture.query)
            .expect("released reservation permits exact re-read");
        assert_eq!(replay.car(), car.as_slice());
        drop(replay);
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
            capability.read_finalized_seed(&fixture.query).unwrap_err(),
            MusubiSeedStagingErrorV1::Invalid,
            "a completed seed lease must not authorize a substituted current registration",
        );
        drop(ingress);
        drop(capability);
        MusubiSeedStagingBackendV1::open(&seed_root, binding.seed_provider, 2, 128 * 1024 * 1024)
            .expect("last owner releases exclusive seed lease");
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
        height_ahead.snapshot.finalized_height = fixture.query.snapshot.finalized_height + 1;
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
