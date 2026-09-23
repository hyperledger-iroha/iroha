//! Authoritative read-only finality checks for private Musubi storage coordination.
//!
//! This module deliberately owns no listener, queue submission, `SoraFS` mutation, or service
//! activation. It only derives one current archive record from daemon-owned finalized state and
//! exact Kura history; the stock publication service remains unavailable without deployment
//! injection.
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
use std::{num::NonZeroUsize, sync::Arc};
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
mod tests {
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
            kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1},
            musubi::RegisterMusubiArchiveV1,
        },
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiArchiveCommitmentV1, MusubiContentDigestV1,
            MusubiSeedIngressReceiptApprovalV1, MusubiSeedIngressReceiptBindingV1,
            MusubiSeedIngressReceiptPayloadV1, MusubiSeedIngressReceiptV1,
            MusubiSemanticReleaseDigestV1,
        },
        sorafs::{
            capacity::ProviderId,
            pin_registry::{ChunkerProfileHandle, ManifestRootCid},
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
        registration_material_at(network_id(0x15), 1)
    }
    fn registration_material_at(
        network_id: NetworkId,
        registered_at_height: u64,
    ) -> RegistrationMaterial {
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
            KagemushaMintFinalityAuthorityGenerationTemplateV1, KagemushaMintFinalityGenesisParametersV1,
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
    fn finality_artifact(
        block: &SignedBlock,
        network_id: NetworkId,
        parent: Option<&V2FinalityArtifact>,
    ) -> V2FinalityArtifact {
        let keypairs = finality_keypairs();
        let roster = keypairs
            .iter()
            .map(|keypair| ValidatorPower {
                validator: PeerId::new(keypair.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let kagemusha_mint_finality_authority = KagemushaMintFinalityAuthorityGenerationV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                network_id,
                generation: 0,
                validators: roster
                    .iter()
                    .enumerate()
                    .map(|(index, validator)| {
                        let seed = 0xC0_u8.wrapping_add(
                            u8::try_from(index)
                                .expect("Musubi finality validator index fits in one byte"),
                        );
                        iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                            &[seed; 32],
                            0,
                            validator.validator.clone(),
                        )
                        .expect("derive independent Musubi finality Pasta authority")
                    })
                    .collect(),
            };
        kagemusha_mint_finality_authority
            .validate()
            .expect("Musubi finality Pasta authority must be canonical");
        let kagemusha_mint_finality_authorization = {
            let authority = &kagemusha_mint_finality_authority;
            let authorization = iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
                version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
                network_id: authority.network_id,
                epoch: 0,
                first_height: 1,
                last_height: 100,
                authority_generation: authority.generation,
                authority_id: authority.authority_id().expect("fixture authority identity"),
                beacon: iroha_data_model::isi::kagemusha_v1::BeaconEpochBindingV1::Bootstrap,
                previous_authorization_id: [0; 32],
                transition_id: [0; 32],
                decision: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::Genesis,
            };
            authorization.validate_against_authority(authority).expect("complete genesis fixture authorization");
            authorization
        };
        let height = block.header().height().get();
        let context = HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: parent.map(|parent| parent.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid finality fixture quorum"),
            roster,
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
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
        // This helper belongs only to structural finality/output join controls.
        // Stateful fixtures below derive their commitment from actual execution.
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
        sign_finality_artifact(block, context, execution_commitment)
    }
    fn sign_finality_artifact(
        block: &SignedBlock,
        context: HeightContext,
        execution_commitment: ExecutionCommitment,
    ) -> V2FinalityArtifact {
        let keypairs = finality_keypairs();
        let height = block.header().height().get();
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
    fn reader_fixture() -> ReaderFixture {
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
        state.install_lane_manifests(&Arc::new(
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
        ValidBlock::validate_signed_genesis_keep_voting_block(
            genesis,
            topology,
            &genesis_account,
            &validation_time,
            state,
            &mut None,
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
            let material =
                registration_material_at(NetworkId::from_genesis_hash(provisional.hash()), 2);
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
        let material = registration_material_at(NetworkId::from_genesis_hash(genesis.hash()), 2);
        let registration_transaction = successful_registration_transaction(&material);
        let transaction_hash = *registration_transaction.hash().as_ref();
        let (state, kura) = fixture_state(&material);
        let (genesis, genesis_finality) = {
            let signed_genesis = iroha_genesis::GenesisBlock(genesis.clone());
            let (valid_genesis, mut genesis_state) = stage_genesis(&state, genesis, &topology);
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
            let genesis = valid_genesis
                .commit(&topology)
                .unpack(|_| {})
                .expect("authenticated genesis commit");
            let genesis_finality = sign_finality_artifact(
                genesis.as_ref(),
                bootstrap.context().clone(),
                genesis_execution,
            );
            iroha_core::sumeragi::validate_signed_genesis_v2_authority(
                &signed_genesis,
                &genesis_finality.height_context,
                &genesis_finality.validator_set_pops,
            )
            .expect("finality preserves signed genesis authority");
            kura.store_block(Arc::new(genesis.as_ref().clone()))
                .expect("store executed genesis");
            let _ = kura
                .store_v2_finality_artifact(&genesis_finality)
                .expect("store exact genesis finality");
            let _ = genesis_state.apply_without_execution(&genesis, topology.as_ref().to_vec());
            // TODO: complete the canonical State publication owner. The output seal
            // intentionally keeps this commit gated; this positive fixture must fail
            // until real publication authority exists, rather than clearing its guard.
            (*genesis_state)
                .commit()
                .expect("publish authenticated genesis through the complete State owner");
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
        let committed = valid
            .commit(&topology)
            .unpack(|_| {})
            .expect("registration block signature quorum");
        assert!(
            committed
                .as_ref()
                .network_output_at(0)
                .is_some_and(|(_, output)| output.result.is_ok())
        );
        let canonical_block = committed.as_ref().clone();
        kura.store_block(Arc::new(canonical_block.clone()))
            .expect("store fixture Kura block");
        if store_finality {
            let mut context = genesis_finality.height_context.clone();
            context.height = canonical_block.header().height().get();
            context.parent_commit_qc = Some(genesis_finality.commit_qc.clone());
            let _ = kura
                .store_v2_finality_artifact(&sign_finality_artifact(
                    &canonical_block,
                    context,
                    execution,
                ))
                .expect("store fixture V2 finality artifact");
        }
        let _ = state_block.apply_without_execution(&committed, topology.as_ref().to_vec());
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
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit current archive substitution");
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
