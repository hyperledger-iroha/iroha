//! Non-shipping publication fixtures using actual execution and verified finality.
//!
//! The four deterministic signing keys are test authority, not a qualification
//! of production genesis bootstrap. Signed voting peers, when present, must
//! match them exactly. Successors inherit only the actual durable parent.

use super::{State, StateBlock};
use crate::{
    block::{CommittedBlock, ValidBlock, VerifiedV2FinalityArtifact},
    sumeragi::{exec, v2_context, v2_recovery},
};
use iroha_crypto::{Algorithm, HashOf, KeyPair, Signature};
use iroha_data_model::block::consensus_v2 as wire;
use iroha_model_base::peer::PeerId;

fn keys() -> Result<Vec<KeyPair>, String> {
    let mut keys = (0xD2_u8..=0xD5)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    Ok(keys)
}

impl State {
    /// Publish the standard sample-key genesis for an in-crate component test.
    ///
    /// # Errors
    /// Returns the execution and publication errors from the explicit-key fixture.
    #[cfg(test)]
    pub fn seed_genesis_for_testing(&self) -> Result<iroha_data_model::block::SignedBlock, String> {
        self.seed_signed_genesis_for_testing(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
    }

    /// Execute and publish a signed, fee-free genesis for a component fixture.
    ///
    /// The fixture's initial World is supplied by its constructor. Genesis
    /// registers its signing account when absent, then establishes real execution
    /// ownership, history and durable finality before ordinary height-two work.
    ///
    /// # Errors
    /// Rejects a nonempty history or any genesis execution/publication failure.
    #[doc(hidden)]
    pub fn seed_signed_genesis_for_testing(
        &self,
        genesis_keypair: &KeyPair,
    ) -> Result<iroha_data_model::block::SignedBlock, String> {
        use super::{StateReadOnly, WorldReadOnly};
        use crate::sumeragi::network_topology::Topology;
        use iroha_data_model::block::{BlockHeader, builder::BlockBuilder};
        use iroha_data_model::{
            account::{Account, AccountId},
            isi::{InstructionBox, Log, Register},
            level::Level,
            transaction::{FeePaymentIntent, TransactionBuilder},
        };
        use iroha_primitives::time::TimeSource;
        use std::{num::NonZeroU64, time::Duration};

        if self.committed_height() != 0 {
            return Err("genesis fixture requires an empty history".into());
        }
        let genesis_account = AccountId::new(genesis_keypair.public_key().clone());
        let (_, time) = TimeSource::new_mock(Duration::ZERO);
        let mut instructions = Vec::<InstructionBox>::new();
        if self.query_view().world().account(&genesis_account).is_err() {
            // Missing-authority admission requires exact self-registration first.
            instructions.push(Register::account(Account::new(genesis_account.clone())).into());
        }
        instructions.push(Log::new(Level::DEBUG, "component fixture genesis".to_owned()).into());
        let transaction = TransactionBuilder::new_genesis_with_time_source(
            genesis_account.clone(),
            &time,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(genesis_keypair.private_key());
        // Static admission requires every transaction to strictly precede its block.
        let block_time_ms = u64::try_from(transaction.creation_time().as_millis())
            .map_err(|_| "genesis fixture timestamp exceeds u64")?
            .checked_add(1)
            .ok_or("genesis fixture block timestamp overflows")?;
        let time = TimeSource::new_fixed(Duration::from_millis(block_time_ms));
        let mut header = BlockHeader::new(
            NonZeroU64::new(1).expect("positive genesis height"),
            None,
            None,
            block_time_ms,
            0,
        );
        {
            let view = self.query_view();
            let digest = super::compute_confidential_feature_digest(
                view.world(),
                view.zk(),
                view.sccp_registry(),
                1,
            );
            header.set_confidential_features((!digest.is_empty()).then_some(digest));
        }
        let mut builder = BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_da_proof_policies(Some(crate::da::proof_policy_bundle(
            &self.nexus_snapshot().lane_config,
        )));
        let source = builder.build_with_signature(0, genesis_keypair.private_key());
        let topology = Topology::new(
            keys()?
                .into_iter()
                .map(|key| PeerId::new(key.public_key().clone())),
        );
        let mut staged = self.block(source.header());
        let valid = ValidBlock::validate_sumeragi_v2_fixture(
            source,
            &topology,
            &genesis_account,
            &time,
            &mut staged,
        )
        .unpack(|_| {})
        .map_err(|(_, error)| error.to_string())?;
        let committed = valid.commit_unchecked().unpack(|_| {});
        let signed = committed.as_ref().clone();
        self.commit_executed_block_for_testing(staged, committed)?;
        Ok(signed)
    }

    /// Replay complete fixture outputs and publish the actual execution under
    /// the same four-validator authority as a freshly executed fixture.
    ///
    /// # Errors
    /// Rejects unauthenticated genesis, changed results, mismatched predecessor,
    /// and every durable-finality or publication failure.
    #[doc(hidden)]
    pub fn replay_and_commit_fixture_block_for_testing(
        &self,
        committed: CommittedBlock,
        genesis_account: Option<&iroha_data_model::account::AccountId>,
    ) -> Result<(), String> {
        let mut replayed = committed.as_ref().clone();
        let mut state_block = self.block(replayed.header());
        ValidBlock::execute_block_outputs_for_test(
            &mut replayed,
            &mut state_block,
            genesis_account,
        )
        .map_err(|error| error.to_string())?;
        super::replay_outputs::ensure_replayed_results_match_committed(
            committed.as_ref().header().height().get(),
            committed.as_ref(),
            &replayed,
        )
        .map_err(|error| error.to_string())?;
        self.commit_executed_block_for_testing(state_block, committed)
            .map(|_| ())
    }

    /// Publish an actually executed fixture block through native output finality.
    ///
    /// This test-only authority signs the retained execution using four fixed BLS
    /// validators (seeds `0xD2..=0xD5`), three votes and all four real PoPs. It
    /// preserves the production seal, durable finality and publication checks.
    /// Genesis without a signed voting roster has explicitly synthetic authority;
    /// this helper does not establish production consensus bootstrap eligibility.
    /// Kagemusha mint top-ups and epoch rotation require their dedicated paired-
    /// Pasta fixture authority and are rejected here before any durable writes.
    ///
    /// # Errors
    /// Rejects a foreign State, changed sealed body, absent captured witness,
    /// mismatched signed voting keys, or a block not extending the exact durable
    /// parent. Persistence or State publication failures retain their errors.
    #[doc(hidden)]
    pub fn commit_executed_block_for_testing(
        &self,
        state_block: StateBlock<'_>,
        committed: CommittedBlock,
    ) -> Result<Vec<iroha_data_model::events::EventBox>, String> {
        self.commit_executed_block_with_precommit_for_testing(state_block, committed, |_| {})
    }

    /// Publish an actually executed fixture while observing its authorized precommit state.
    ///
    /// The observer runs after exact output finality and deterministic Apply have
    /// completed, immediately before the real State commit. It cannot mutate or
    /// replace the execution or publication authority.
    ///
    /// # Errors
    /// Returns the same execution, finality and publication errors as
    /// [`Self::commit_executed_block_for_testing`].
    #[doc(hidden)]
    pub(crate) fn commit_executed_block_with_precommit_for_testing(
        &self,
        mut state_block: StateBlock<'_>,
        committed: CommittedBlock,
        before_commit: impl FnOnce(&StateBlock<'_>),
    ) -> Result<Vec<iroha_data_model::events::EventBox>, String> {
        if !std::ptr::eq(self, state_block.state_ref) {
            return Err("execution publication belongs to a different State".into());
        }
        let valid = ValidBlock::from(committed);
        let block = valid.as_ref();
        state_block.verify_execution_output_seal(block)?;
        let expected_height = u64::try_from(self.committed_height())
            .ok()
            .and_then(|height| height.checked_add(1))
            .ok_or("height overflow")?;
        if block.header().height().get() != expected_height
            || block.header().prev_block_hash() != self.latest_block_hash_fast()
        {
            return Err("execution publication does not extend this State's exact parent".into());
        }
        let witness = state_block
            .take_exec_witness()
            .ok_or("execution publication requires the actual captured witness")?;
        let casting = state_block
            .take_parliament_timed_ovn_casting_bindings()
            .ok_or("execution publication requires actual captured casting bindings")?;
        let keys = keys()?;
        let context = if block.header().is_genesis() {
            let signed_peers = v2_context::signed_genesis_voting_peers(
                &iroha_genesis::GenesisBlock(block.clone()),
            )
            .map_err(|error| error.to_string())?;
            let roster = keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            if !signed_peers.is_empty()
                && signed_peers
                    != roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect::<Vec<_>>()
            {
                return Err(
                    "signed genesis voting keys differ from publication fixture keys".into(),
                );
            }
            let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
                crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                    self.network_id, u64::MAX, &roster,
                );
            v2_context::build_genesis_height_context(v2_context::GenesisContextInputs {
                network_id: self.network_id,
                election: v2_context::FrozenElectionInputs {
                    epoch: 0,
                    kagemusha_mint_finality_authorization,
                    kagemusha_mint_finality_authority,
                    epoch_end_height: u64::MAX,
                    mode: wire::ConsensusMode::Permissioned,
                    roster,
                    leader_seed: [0xD2; 32],
                },
                next_epoch_snapshot: None,
                nexus_amx_context_hash: v2_context::staged_genesis_nexus_amx_context_hash(
                    &state_block,
                ),
                execution_policy_hash: v2_context::staged_genesis_execution_policy_hash(
                    &state_block,
                )
                .map_err(|error| error.to_string())?,
                da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            })
            .map_err(|error| error.to_string())?
        } else {
            let parent_height = expected_height - 1;
            let parent_context = self
                .sumeragi_v2_height_context(parent_height)
                .map_err(|error| error.to_string())?
                .ok_or("execution publication requires its durable parent context")?;
            let parent = self
                .kura
                .v2_finality_artifact(parent_height)
                .map_err(|error| error.to_string())?
                .ok_or("execution publication requires its durable parent finality")?;
            if parent.height_context != parent_context
                || Some(parent.block_hash) != block.header().prev_block_hash()
            {
                return Err(
                    "execution publication parent authority differs from this State".into(),
                );
            }
            let nexus = v2_recovery::committed_nexus_amx_context_hash(self)
                .map_err(|error| error.to_string())?;
            let view = self.try_view().map_err(|error| error.to_string())?;
            v2_context::build_successor_height_context_from_state(&parent, &view, nexus)
                .map_err(|error| error.to_string())?
        };
        if context.network_id != self.network_id
            || context.height != expected_height
            || context.roster.len() != keys.len()
            || context.roster.iter().zip(&keys).any(|(validator, key)| {
                validator.power != 1 || validator.validator.public_key() != key.public_key()
            })
        {
            return Err(
                "execution publication authority differs from its four signing keys".into(),
            );
        }
        let native =
            exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
                block,
                state_block.staged_merge_entry(),
            )?;
        let lanes = exec::LaneFinalityManifestV1::from_result_bearing_block(block)?;
        let execution_commitment =
            exec::execution_commitment_from_validated_block(&witness, &native, &lanes, block)
                .map_err(str::to_owned)?;
        if context.next_epoch_snapshot.is_some()
            || execution_commitment.kagemusha_top_up_count != 0
            || execution_commitment.kagemusha_top_up_root.is_some()
        {
            return Err("execution publication fixture requires dedicated authority for mint top-ups or epoch rotation".into());
        }
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .map_err(|error| error.to_string())?,
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: block.header().view_change_index(),
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = keys
            .iter()
            .take(3)
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .map(|signature| signature.payload().to_vec())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .map_err(|error| error.to_string())?,
        };
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let artifact = wire::finality::V2FinalityArtifact::new(context, subject, certificate, pops);
        let verified = VerifiedV2FinalityArtifact::verify(artifact.clone())
            .map_err(|error| error.to_string())?;
        let committed = valid
            .commit_with_verified_v2_artifact(verified, execution_commitment)
            .unpack(|_| {})
            .map_err(|(_, error)| error.to_string())?;
        self.kura
            .stage_kagemusha_finality_sidecar(
                artifact.height,
                artifact.block_hash,
                &witness,
                execution_commitment,
                &casting,
            )
            .map_err(|error| error.to_string())?;
        self.kura
            .store_block(committed.clone())
            .map_err(|error| error.to_string())?;
        let receipt = self
            .kura
            .store_v2_finality_artifact(&artifact)
            .map_err(|error| error.to_string())?;
        if receipt.artifact_hash() != HashOf::new(&artifact)
            || receipt.block_hash() != committed.as_ref().hash()
        {
            return Err("durable finality receipt differs from actual execution".into());
        }
        state_block.authorize_execution_output_publication(&committed, &witness)?;
        let events = state_block
            .apply_without_execution_with_verified_v2_finality(&committed)
            .map_err(|error| error.to_string())?;
        before_commit(&state_block);
        state_block.commit().map_err(|error| error.to_string())?;
        self.kura
            .promote_kagemusha_finality_sidecar(&artifact, &receipt)
            .map_err(|error| error.to_string())?;
        Ok(events)
    }
}

#[cfg(test)]
#[path = "execution_publication_test_support_tests.rs"]
mod tests;
