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
        mut state_block: StateBlock<'_>,
        committed: CommittedBlock,
    ) -> Result<(), String> {
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
            v2_context::build_genesis_height_context(v2_context::GenesisContextInputs {
                network_id: self.network_id,
                election: v2_context::FrozenElectionInputs {
                    epoch: 0,
                    kagemusha_mint_finality_epoch_roster:
                        crate::kagemusha_v1_test_fixtures::mint_finality_roster(
                            self.network_id,
                            0,
                            &roster,
                        ),
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
        let _events = state_block
            .apply_without_execution_with_verified_v2_finality(&committed)
            .map_err(|error| error.to_string())?;
        state_block.commit().map_err(|error| error.to_string())?;
        self.kura
            .promote_kagemusha_finality_sidecar(&artifact, &receipt)
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "execution_publication_test_support_tests.rs"]
mod tests;
