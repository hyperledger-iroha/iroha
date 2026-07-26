//! Exec-vote helpers: compute `post_state_root` via SMT, build votes, and assemble QCs.
//!
//! This module is internal and side-effect free; consumed by the Sumeragi execution pipeline.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
use iroha_data_model::{
    block::{
        SignedBlock,
        consensus::{LaneBlockCommitment, LaneBlockProposalV1},
        consensus_v2 as wire,
    },
    nexus::{DataSpaceId, LaneId},
    transaction::signed::TransactionResult,
};

use super::{
    consensus::ExecWitness,
    smt::{
        KvPair, build_kagemusha_topup_block_commitment, compute_consensus_post_state_root,
        compute_post_state_root,
    },
};

fn witness_pairs(witness: &ExecWitness) -> (Vec<KvPair>, Vec<KvPair>) {
    let reads = witness
        .reads
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    let writes = witness
        .writes
        .iter()
        .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
        .collect();
    (reads, writes)
}

#[derive(Debug)]
struct NativeAmxApplicationGroup {
    participant_proposal: LaneBlockProposalV1,
    participant_settlement: LaneBlockCommitment,
    participant_settlement_hash: HashOf<LaneBlockCommitment>,
    settlement_source_ids: Vec<[u8; Hash::LENGTH]>,
    members: Vec<wire::NativeAmxApplicationManifestMemberV1>,
    results: Vec<TransactionResult>,
}

/// Full deterministic projection behind one Native AMX manifest leaf.
#[derive(Clone, Debug)]
pub(crate) struct NativeAmxApplicationManifestEntryV1 {
    /// Public consensus leaf committed by the global CommitQC.
    pub(crate) leaf: wire::NativeAmxApplicationManifestLeafV1,
    /// Exact participant proposal retained in the durable receipt.
    pub(crate) participant_proposal: LaneBlockProposalV1,
    /// Exact zero-effect settlement retained in the durable receipt.
    pub(crate) participant_settlement: LaneBlockCommitment,
    /// Exact canonical transaction results aligned with `leaf.members`.
    pub(crate) results: Vec<TransactionResult>,
}

/// Canonical, bounded Native AMX application manifest for one executed block.
#[derive(Clone, Debug)]
pub(crate) struct NativeAmxApplicationManifestV1 {
    executed_block_wire_hash: Hash,
    entries: Vec<NativeAmxApplicationManifestEntryV1>,
    tree: MerkleTree<wire::NativeAmxApplicationManifestLeafV1>,
}

impl NativeAmxApplicationManifestV1 {
    /// Build the canonical empty manifest for a result-bearing wire identity.
    #[must_use]
    pub(crate) fn empty(executed_block_wire_hash: Hash) -> Self {
        Self {
            executed_block_wire_hash,
            entries: Vec::new(),
            tree: MerkleTree::default(),
        }
    }

    /// Derive the exact manifest from one deterministic result-bearing block.
    ///
    /// Same-route coordinator legs are deliberately excluded by the shared
    /// Native AMX role classifier. Every separate participant route appears at
    /// most once and its source/result members retain canonical block order.
    pub(crate) fn from_result_bearing_block(block: &SignedBlock) -> Result<Self, String> {
        let executed_block_wire_hash = block
            .executed_block_wire_hash()
            .map_err(|error| format!("canonical executed block cannot be hashed: {error}"))?;
        let Some(bundle) = block.execution_context() else {
            return Ok(Self::empty(executed_block_wire_hash));
        };
        if !bundle
            .external
            .iter()
            .any(|context| context.native_amx_receipt.is_some())
        {
            return Ok(Self::empty(executed_block_wire_hash));
        }

        let entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
        let results = block.results().cloned().collect::<Vec<_>>();
        let expected_result_root = block.result_hashes().collect::<MerkleTree<_>>().root();
        if bundle.external.len() != entrypoints.len()
            || results.len() < bundle.external.len()
            || block.header().result_merkle_root() != expected_result_root
        {
            return Err(
                "Native AMX application block result/context alignment is invalid".to_owned(),
            );
        }

        let application_block_height = block.header().height().get();
        let application_block_hash = block.hash();
        let mut groups = BTreeMap::<(LaneId, DataSpaceId, Hash), NativeAmxApplicationGroup>::new();
        for (index, context) in bundle.external.iter().enumerate() {
            let Some(native_receipt) = context.native_amx_receipt.as_ref() else {
                continue;
            };
            let entrypoint = entrypoints.get(index).ok_or_else(|| {
                "Native AMX application block is missing its canonical entrypoint".to_owned()
            })?;
            if entrypoint.hash() != context.entrypoint_hash {
                return Err(
                    "Native AMX execution context does not identify its canonical entrypoint"
                        .to_owned(),
                );
            }
            let result = results.get(index).cloned().ok_or_else(|| {
                "Native AMX application block is missing a committed transaction result".to_owned()
            })?;
            let entrypoint_index = u64::try_from(index).map_err(|_| {
                "Native AMX entrypoint index does not fit the canonical manifest".to_owned()
            })?;

            for leg in &native_receipt.legs {
                match crate::native_amx::native_amx_participant_application_role(
                    native_receipt,
                    leg,
                ) {
                    Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) => {
                        continue;
                    }
                    Ok(
                        crate::native_amx::NativeAmxParticipantApplicationRole::SeparateParticipant,
                    ) => {}
                    Err(error) => {
                        return Err(format!(
                            "Native AMX participant application identity is invalid: {error}"
                        ));
                    }
                }
                crate::lane_consensus::validate_lane_block_proposal(&leg.participant_proposal)
                    .map_err(|_| "Native AMX participant proposal is invalid".to_owned())?;
                let descriptor = &leg.participant_proposal.descriptor;
                let prepare = &leg.prepare_qc.body;
                let commit = &leg.commit_qc.body;
                if descriptor.proposal_height != application_block_height
                    || native_receipt.authority_context_height != application_block_height
                    || prepare.source_id != native_receipt.source_id
                    || commit.source_id != native_receipt.source_id
                    || prepare.tx_entrypoint_hash != context.entrypoint_hash
                    || commit.tx_entrypoint_hash != context.entrypoint_hash
                    || prepare.participant_proposal_hash != leg.participant_proposal.proposal_hash
                    || commit.participant_proposal_hash != leg.participant_proposal.proposal_hash
                    || prepare.participant_settlement_commitment
                        != Hash::from(leg.participant_settlement_hash)
                    || commit.participant_settlement_commitment
                        != Hash::from(leg.participant_settlement_hash)
                {
                    return Err(
                        "Native AMX participant QCs do not bind the canonical source/entrypoint"
                            .to_owned(),
                    );
                }
                let computed_settlement_hash =
                    iroha_data_model::nexus::compute_settlement_hash(&leg.participant_settlement)
                        .map_err(|_| {
                        "Native AMX participant control settlement cannot be hashed".to_owned()
                    })?;
                if computed_settlement_hash != leg.participant_settlement_hash {
                    return Err(
                        "Native AMX participant control settlement hash mismatch".to_owned()
                    );
                }
                let settlement = &leg.participant_settlement;
                if settlement.tx_count
                    != u64::try_from(settlement.receipts.len()).unwrap_or(u64::MAX)
                    || !settlement.total_local_amount.is_zero()
                    || !settlement.total_xor_due.is_zero()
                    || !settlement.total_xor_after_haircut.is_zero()
                    || !settlement.total_xor_variance.is_zero()
                    || settlement.swap_metadata.is_some()
                    || !settlement.nexus_fee_receipts.is_empty()
                    || !settlement.native_amx_receipts.is_empty()
                    || settlement.receipts.is_empty()
                    || settlement.receipts.len() > wire::MAX_NATIVE_AMX_APPLICATION_MANIFEST_MEMBERS
                    || settlement.receipts.iter().any(|receipt| {
                        !receipt.local_amount.is_zero()
                            || !receipt.xor_due.is_zero()
                            || !receipt.xor_after_haircut.is_zero()
                            || !receipt.xor_variance.is_zero()
                            || receipt.timestamp_ms != application_block_height
                    })
                {
                    return Err(
                        "Native AMX participant settlement is not exact zero-effect control evidence"
                            .to_owned(),
                    );
                }

                let key = (
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                );
                let settlement_source_ids = settlement
                    .receipts
                    .iter()
                    .map(|receipt| receipt.source_id)
                    .collect::<Vec<_>>();
                let group = groups
                    .entry(key)
                    .or_insert_with(|| NativeAmxApplicationGroup {
                        participant_proposal: leg.participant_proposal.clone(),
                        participant_settlement: settlement.clone(),
                        participant_settlement_hash: leg.participant_settlement_hash,
                        settlement_source_ids,
                        members: Vec::new(),
                        results: Vec::new(),
                    });
                if group.participant_proposal != leg.participant_proposal
                    || group.participant_settlement != *settlement
                    || group.participant_settlement_hash != leg.participant_settlement_hash
                {
                    return Err(
                        "Native AMX participant route carries conflicting proposal/control claims"
                            .to_owned(),
                    );
                }
                if group
                    .members
                    .iter()
                    .any(|member| member.source_id == native_receipt.source_id)
                {
                    return Err(
                        "Native AMX participant control repeats a source transaction".to_owned(),
                    );
                }
                group
                    .members
                    .push(wire::NativeAmxApplicationManifestMemberV1 {
                        entrypoint_index,
                        source_id: native_receipt.source_id,
                        entrypoint_hash: context.entrypoint_hash,
                        result_hash: result.hash(),
                    });
                group.results.push(result.clone());
            }
        }

        if groups.len()
            > usize::try_from(wire::MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES)
                .expect("manifest leaf bound fits usize")
        {
            return Err("Native AMX application manifest exceeds the route-leaf limit".to_owned());
        }

        let mut entries = Vec::with_capacity(groups.len());
        for (_, group) in groups {
            let source_ids = group
                .members
                .iter()
                .map(|member| member.source_id)
                .collect::<Vec<_>>();
            if source_ids != group.settlement_source_ids
                || source_ids.iter().copied().collect::<BTreeSet<_>>().len() != source_ids.len()
            {
                return Err(
                    "Native AMX grouped participant settlement does not exactly cover block sources"
                        .to_owned(),
                );
            }
            let descriptor = &group.participant_proposal.descriptor;
            let leaf = wire::NativeAmxApplicationManifestLeafV1 {
                version: wire::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                participant_height: descriptor.lane_block_height,
                participant_view: descriptor.lane_block_view,
                predecessor_height: descriptor.previous_lane_block_height,
                predecessor_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
                descriptor_hash: descriptor.descriptor_hash,
                proposal_hash: group.participant_proposal.proposal_hash,
                settlement_hash: group.participant_settlement_hash,
                members: group.members,
                application_block_height,
                application_block_hash,
                executed_block_wire_hash,
            };
            leaf.validate().map_err(|error| {
                format!("Native AMX application manifest leaf is invalid: {error}")
            })?;
            entries.push(NativeAmxApplicationManifestEntryV1 {
                leaf,
                participant_proposal: group.participant_proposal,
                participant_settlement: group.participant_settlement,
                results: group.results,
            });
        }
        let tree = entries
            .iter()
            .map(|entry| HashOf::new(&entry.leaf))
            .collect::<MerkleTree<_>>();
        Ok(Self {
            executed_block_wire_hash,
            entries,
            tree,
        })
    }

    /// Exact canonical result-bearing block wire hash.
    #[must_use]
    pub(crate) const fn executed_block_wire_hash(&self) -> Hash {
        self.executed_block_wire_hash
    }

    /// Canonically ordered separate-participant entries.
    #[must_use]
    pub(crate) fn entries(&self) -> &[NativeAmxApplicationManifestEntryV1] {
        &self.entries
    }

    /// Canonical manifest root, including the domain-separated empty root.
    #[must_use]
    pub(crate) fn root(&self) -> Hash {
        self.tree
            .root()
            .map(Hash::from)
            .unwrap_or_else(wire::native_amx_application_manifest_empty_root)
    }

    /// Number of committed route/incarnation leaves.
    #[must_use]
    pub(crate) fn count(&self) -> u32 {
        u32::try_from(self.entries.len()).expect("manifest builder enforces the u32 leaf bound")
    }

    /// Inclusion proof for one canonical manifest entry.
    #[must_use]
    pub(crate) fn proof(
        &self,
        index: u32,
    ) -> Option<MerkleProof<wire::NativeAmxApplicationManifestLeafV1>> {
        self.tree.get_proof(index)
    }
}

/// Convert an `ExecWitness` into SMT `KvPair` slices and compute the `post_state_root`.
pub fn post_state_from_witness(w: &ExecWitness) -> Hash {
    try_post_state_from_witness(w).unwrap_or_else(|error| {
        let mut preimage = b"iroha:sumeragi:invalid-exec-witness".to_vec();
        preimage.push(0);
        preimage.extend_from_slice(error.as_bytes());
        Hash::new(preimage)
    })
}

/// Checked variant used before a validator signs execution roots.
pub fn try_post_state_from_witness(w: &ExecWitness) -> Result<Hash, &'static str> {
    let (reads, writes) = witness_pairs(w);
    compute_consensus_post_state_root(&reads, &writes)
}

/// Derive the exact execution commitment authenticated by Sumeragi-v2 votes.
///
/// This is intentionally the only projection used by candidate validation and
/// decided application. It consumes the actual `StateBlock` witness and uses
/// the same bounded Kagemusha subtree builder as finality-proof generation.
pub(crate) fn execution_commitment_from_witness(
    witness: &ExecWitness,
    native_amx_manifest: &NativeAmxApplicationManifestV1,
) -> Result<wire::ExecutionCommitment, &'static str> {
    let (reads, writes) = witness_pairs(witness);
    let parent_state_root = parent_state_from_witness(witness);
    match build_kagemusha_topup_block_commitment(&writes)? {
        Some(kagemusha) => wire::ExecutionCommitment::new_with_native_amx_application_manifest(
            parent_state_root,
            kagemusha.post_state_root,
            kagemusha.ordinary_writes_root,
            Some(kagemusha.topup_anchor_root),
            u32::try_from(kagemusha.leaves.len())
                .map_err(|_| "Kagemusha V2 top-up anchor count does not fit u32")?,
            wire::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_manifest.root(),
            native_amx_manifest.count(),
            native_amx_manifest.executed_block_wire_hash(),
        )
        .map_err(|_| "Kagemusha V2 execution commitment is not canonical"),
        None => wire::ExecutionCommitment::new_with_native_amx_application_manifest(
            parent_state_root,
            compute_consensus_post_state_root(&reads, &writes)?,
            compute_post_state_root(&[], &writes),
            None,
            0,
            wire::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_manifest.root(),
            native_amx_manifest.count(),
            native_amx_manifest.executed_block_wire_hash(),
        )
        .map_err(|_| "Sumeragi V2 execution commitment is not canonical"),
    }
}

/// Compute the `parent_state_root` using only the witnessed reads (pre-values).
/// When a block writes state, only pre-values for written keys are included.
/// Read-only access witnesses can vary across execution strategies and should
/// not perturb the commit vote for an otherwise identical state transition.
pub fn parent_state_from_witness(w: &ExecWitness) -> Hash {
    let reads: Vec<KvPair> = if w.writes.is_empty() {
        w.reads
            .iter()
            .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
            .collect()
    } else {
        let write_keys: std::collections::BTreeSet<&[u8]> =
            w.writes.iter().map(|kv| kv.key.as_slice()).collect();
        w.reads
            .iter()
            .filter(|kv| write_keys.contains(kv.key.as_slice()))
            .map(|kv| KvPair::new(kv.key.clone(), kv.value.clone()))
            .collect()
    };
    compute_post_state_root(&reads, &[])
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, time::Duration};

    use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{
            BlockHeader, BlockSignature,
            consensus::{
                LaneBlockDescriptorV1, LaneSettlementReceipt, NativeAmxAttestationBodyV2,
                NativeAmxAttestationQcV2, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
            },
            execution_context::{
                BlockExecutionContextBundle, ExternalExecutionContext, ExternalExecutionRouteRole,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        peer::PeerId,
        transaction::{
            FeePaymentIntent,
            signed::{TransactionBuilder, TransactionEntrypoint, TransactionResultInner},
        },
        trigger::DataTriggerSequence,
    };
    use iroha_primitives::{numeric::Quantity, time::TimeSource};

    use super::super::consensus::{ExecKv, ExecWitness};
    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};

    const MANIFEST_APPLICATION_HEIGHT: u64 = 40;
    const MANIFEST_LANE_BLOCK_HEIGHT: u64 = 5;
    const MANIFEST_COORDINATOR_VIEW: u64 = 9;

    #[derive(Clone)]
    struct ManifestParticipantFixture {
        proposal: LaneBlockProposalV1,
        settlement: LaneBlockCommitment,
        settlement_hash: HashOf<LaneBlockCommitment>,
    }

    pub(super) struct ManifestBlockFixture {
        pub(super) block: SignedBlock,
        source_ids: [[u8; Hash::LENGTH]; 2],
        first_route: (LaneId, DataSpaceId),
        second_route: (LaneId, DataSpaceId),
    }

    fn fixture_key(seed: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("deterministic manifest fixture key")
    }

    #[allow(clippy::too_many_lines)]
    fn manifest_participant(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_view: u64,
        seed: u8,
        validator: &PeerId,
        accepted_entrypoints: &[HashOf<TransactionEntrypoint>],
        source_ids: &[[u8; Hash::LENGTH]],
    ) -> ManifestParticipantFixture {
        let validator_set = vec![validator.clone()];
        let validator_set_hash = HashOf::new(&validator_set);
        let lane_incarnation =
            Hash::new([b"manifest participant incarnation:".as_slice(), &[seed]].concat());
        let accepted_transaction_hashes = accepted_entrypoints
            .iter()
            .copied()
            .map(Hash::from)
            .collect::<Vec<_>>();
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height: MANIFEST_APPLICATION_HEIGHT,
            previous_lane_block_height: MANIFEST_LANE_BLOCK_HEIGHT - 1,
            previous_lane_block_descriptor_hash: Some(Hash::new(
                [b"manifest participant predecessor:".as_slice(), &[seed]].concat(),
            )),
            lane_block_height: MANIFEST_LANE_BLOCK_HEIGHT,
            lane_block_view: lane_view,
            subject_hash: Hash::new(
                [b"manifest participant subject:".as_slice(), &[seed]].concat(),
            ),
            payload_ownership_hash: Hash::new(
                [b"manifest participant ownership:".as_slice(), &[seed]].concat(),
            ),
            rbc_instance_hash: Hash::new(
                [b"manifest participant rbc:".as_slice(), &[seed]].concat(),
            ),
            accepted_candidate_indices: (0..accepted_entrypoints.len())
                .map(|index| u64::try_from(index).expect("fixture entrypoint index fits u64"))
                .collect(),
            accepted_transaction_hashes,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:native-amx-v2".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        crate::lane_consensus::validate_lane_block_proposal(&proposal)
            .expect("canonical manifest participant proposal");

        let receipts = source_ids
            .iter()
            .copied()
            .map(|source_id| LaneSettlementReceipt {
                source_id,
                local_amount: Quantity::zero(),
                xor_due: Quantity::zero(),
                xor_after_haircut: Quantity::zero(),
                xor_variance: Quantity::zero(),
                timestamp_ms: MANIFEST_APPLICATION_HEIGHT,
            })
            .collect::<Vec<_>>();
        let settlement = LaneBlockCommitment {
            block_height: MANIFEST_LANE_BLOCK_HEIGHT,
            lane_id,
            lane_incarnation,
            dataspace_id,
            tx_count: u64::try_from(receipts.len()).expect("fixture receipt count fits u64"),
            total_local_amount: Quantity::zero(),
            total_xor_due: Quantity::zero(),
            total_xor_after_haircut: Quantity::zero(),
            total_xor_variance: Quantity::zero(),
            swap_metadata: None,
            receipts,
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
            .expect("hash manifest participant settlement");
        ManifestParticipantFixture {
            proposal,
            settlement,
            settlement_hash,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn manifest_attestation_body(
        participant: &ManifestParticipantFixture,
        source_id: [u8; Hash::LENGTH],
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        phase: NativeAmxPhase,
        coordinator: &ManifestParticipantFixture,
        chain_id_hash: Hash,
        plan_digest: Hash,
    ) -> NativeAmxAttestationBodyV2 {
        let descriptor = &participant.proposal.descriptor;
        NativeAmxAttestationBodyV2 {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"manifest fixture context",
                ))),
                height: MANIFEST_APPLICATION_HEIGHT,
                view: 6,
            },
            epoch: 3,
            chain_id_hash,
            source_id,
            tx_entrypoint_hash: entrypoint_hash,
            plan_digest,
            phase,
            coordinator_lane_id: coordinator.proposal.descriptor.lane_id,
            coordinator_dataspace_id: coordinator.proposal.descriptor.dataspace_id,
            coordinator_lane_incarnation: coordinator.proposal.descriptor.lane_incarnation,
            participant_lane_id: descriptor.lane_id,
            participant_dataspace_id: descriptor.dataspace_id,
            participant_lane_incarnation: descriptor.lane_incarnation,
            participant_previous_block_height: descriptor.previous_lane_block_height,
            participant_previous_block_descriptor_hash: descriptor
                .previous_lane_block_descriptor_hash,
            participant_lane_block_height: descriptor.lane_block_height,
            participant_lane_block_view: descriptor.lane_block_view,
            participant_proposal_hash: participant.proposal.proposal_hash,
            participant_settlement_commitment: Hash::from(participant.settlement_hash),
            participant_validator_set_hash: descriptor.validator_set_hash,
            participant_validator_count: descriptor.validator_count,
            participant_min_quorum: descriptor.min_quorum,
            authority_context_height: MANIFEST_APPLICATION_HEIGHT,
            planned_coordinator_block_height: MANIFEST_LANE_BLOCK_HEIGHT,
            coordinator_lane_block_view: MANIFEST_COORDINATOR_VIEW,
            coordinator_proposal_hash: coordinator.proposal.proposal_hash,
        }
    }

    fn manifest_qc(
        body: NativeAmxAttestationBodyV2,
        validator_key: &KeyPair,
        validator: &PeerId,
    ) -> NativeAmxAttestationQcV2 {
        let signature = Signature::try_new(validator_key.private_key(), &body.signature_preimage())
            .expect("sign manifest attestation body");
        let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&[signature.payload()])
            .expect("aggregate manifest attestation signature");
        let validator_set = vec![validator.clone()];
        let validator_set_hash = HashOf::new(&validator_set);
        NativeAmxAttestationQcV2 {
            body,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            validator_set_pops: vec![
                iroha_crypto::bls_normal_pop_prove(validator_key.private_key())
                    .expect("manifest fixture validator PoP"),
            ],
            signers_bitmap: vec![1],
            bls_aggregate_signature: aggregate,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn manifest_leg(
        participant: &ManifestParticipantFixture,
        source_id: [u8; Hash::LENGTH],
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        coordinator: &ManifestParticipantFixture,
        chain_id_hash: Hash,
        plan_digest: Hash,
        validator_key: &KeyPair,
        validator: &PeerId,
    ) -> NativeAmxLegRecordV2 {
        let prepare = manifest_attestation_body(
            participant,
            source_id,
            entrypoint_hash,
            NativeAmxPhase::Prepare,
            coordinator,
            chain_id_hash,
            plan_digest,
        );
        let mut commit = prepare;
        commit.phase = NativeAmxPhase::Commit;
        NativeAmxLegRecordV2 {
            lane_id: participant.proposal.descriptor.lane_id,
            dataspace_id: participant.proposal.descriptor.dataspace_id,
            participant_proposal: participant.proposal.clone(),
            participant_settlement: participant.settlement.clone(),
            participant_settlement_hash: participant.settlement_hash,
            prepare_qc: manifest_qc(prepare, validator_key, validator),
            commit_qc: manifest_qc(commit, validator_key, validator),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn manifest_receipt(
        source_id: [u8; Hash::LENGTH],
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        coordinator: &ManifestParticipantFixture,
        first: &ManifestParticipantFixture,
        second: &ManifestParticipantFixture,
        validator_key: &KeyPair,
        validator: &PeerId,
        chain_id_hash: Hash,
        plan_digest: Hash,
    ) -> NativeAmxReceipt {
        NativeAmxReceipt {
            version: 2,
            source_id,
            chain_id_hash,
            plan_digest,
            lane_id: coordinator.proposal.descriptor.lane_id,
            dataspace_id: coordinator.proposal.descriptor.dataspace_id,
            lane_incarnation: coordinator.proposal.descriptor.lane_incarnation,
            authority_context_height: MANIFEST_APPLICATION_HEIGHT,
            lane_block_height: MANIFEST_LANE_BLOCK_HEIGHT,
            lane_block_view: MANIFEST_COORDINATOR_VIEW,
            coordinator_proposal_hash: coordinator.proposal.proposal_hash,
            // Canonical routing-plan order deliberately differs from manifest
            // lane order; the manifest must also omit the same-route leg.
            legs: vec![
                manifest_leg(
                    second,
                    source_id,
                    entrypoint_hash,
                    coordinator,
                    chain_id_hash,
                    plan_digest,
                    validator_key,
                    validator,
                ),
                manifest_leg(
                    first,
                    source_id,
                    entrypoint_hash,
                    coordinator,
                    chain_id_hash,
                    plan_digest,
                    validator_key,
                    validator,
                ),
                manifest_leg(
                    coordinator,
                    source_id,
                    entrypoint_hash,
                    coordinator,
                    chain_id_hash,
                    plan_digest,
                    validator_key,
                    validator,
                ),
            ],
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn result_bearing_native_manifest_block() -> ManifestBlockFixture {
        let transaction_keys = [
            fixture_key(0x31, Algorithm::Ed25519),
            fixture_key(0x32, Algorithm::Ed25519),
        ];
        let chain_id = ChainId::from("native-manifest-exec-test");
        let transaction_time =
            TimeSource::new_fixed(Duration::from_millis(MANIFEST_APPLICATION_HEIGHT));
        let transactions = transaction_keys
            .iter()
            .map(|key| {
                let authority = AccountId::new(key.public_key().clone());
                TransactionBuilder::new_with_time_source(
                    chain_id.clone(),
                    authority,
                    &transaction_time,
                    FeePaymentIntent::authority(Vec::new(), None),
                )
                .sign(key.private_key())
            })
            .collect::<Vec<_>>();
        let entrypoints = transactions
            .iter()
            .map(|transaction| transaction.hash_as_entrypoint())
            .collect::<Vec<_>>();
        let source_ids: [[u8; Hash::LENGTH]; 2] = std::array::from_fn(|index| {
            let mut source_id = [0; Hash::LENGTH];
            source_id.copy_from_slice(transactions[index].hash().as_ref());
            source_id
        });
        let validator_key = fixture_key(0x41, Algorithm::BlsNormal);
        let validator = PeerId::new(validator_key.public_key().clone());
        let coordinator = manifest_participant(
            LaneId::new(7),
            DataSpaceId::new(11),
            MANIFEST_COORDINATOR_VIEW,
            0x70,
            &validator,
            &entrypoints,
            &source_ids,
        );
        let first = manifest_participant(
            LaneId::new(2),
            DataSpaceId::new(8),
            3,
            0x20,
            &validator,
            &entrypoints,
            &source_ids,
        );
        let second = manifest_participant(
            LaneId::new(9),
            DataSpaceId::new(3),
            4,
            0x90,
            &validator,
            &entrypoints,
            &source_ids,
        );
        let chain_id_hash = Hash::new(chain_id.as_str().as_bytes());
        let coordinator_route = RoutingDecision::new(
            coordinator.proposal.descriptor.lane_id,
            coordinator.proposal.descriptor.dataspace_id,
        );
        let routing_plan = RoutingPlan::native_amx(
            coordinator_route,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(
                        first.proposal.descriptor.lane_id,
                        first.proposal.descriptor.dataspace_id,
                    ),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(
                        second.proposal.descriptor.lane_id,
                        second.proposal.descriptor.dataspace_id,
                    ),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(coordinator_route, RouteLegRole::Participant),
            ],
        );
        let plan_digest = routing_plan.digest();
        let routing_plan_legs =
            crate::queue::execution_context_legs_for_routing_plan(&routing_plan);
        let contexts = entrypoints
            .iter()
            .copied()
            .zip(source_ids)
            .map(|(entrypoint_hash, source_id)| {
                let receipt = manifest_receipt(
                    source_id,
                    entrypoint_hash,
                    &coordinator,
                    &first,
                    &second,
                    &validator_key,
                    &validator,
                    chain_id_hash,
                    plan_digest,
                );
                assert!(
                    crate::native_amx::receipt_shape_matches_coordinator_payload(
                        Some(&receipt),
                        &routing_plan,
                        &source_id,
                        Hash::from(entrypoint_hash),
                        chain_id_hash,
                        &coordinator.proposal,
                    ),
                    "manifest fixture must carry a canonical grouped Native AMX receipt"
                );
                ExternalExecutionContext::with_routing_plan(
                    entrypoint_hash,
                    coordinator.proposal.descriptor.lane_id,
                    coordinator.proposal.descriptor.dataspace_id,
                    plan_digest,
                    routing_plan_legs.clone(),
                )
                .with_native_amx_receipt(receipt)
            })
            .collect::<Vec<_>>();

        let header = BlockHeader::new(
            NonZeroU64::new(MANIFEST_APPLICATION_HEIGHT).expect("non-zero fixture height"),
            None,
            None,
            None,
            MANIFEST_APPLICATION_HEIGHT,
            6,
        );
        let initial_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(validator_key.private_key(), header.hash())
                .expect("sign initial manifest block"),
        );
        let mut block = SignedBlock::presigned(initial_signature, header, transactions);
        block.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
        block
            .set_transaction_results(
                Vec::new(),
                &entrypoints,
                vec![
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                    TransactionResultInner::Ok(DataTriggerSequence::default()),
                ],
            )
            .expect("attach exact manifest fixture results");
        let final_signature = BlockSignature::new(
            0,
            SignatureOf::try_from_hash(validator_key.private_key(), block.header().hash())
                .expect("sign finalized manifest block header"),
        );
        block
            .replace_signatures([final_signature].into_iter().collect())
            .expect("replace manifest fixture signature");

        ManifestBlockFixture {
            block,
            source_ids,
            first_route: (
                first.proposal.descriptor.lane_id,
                first.proposal.descriptor.dataspace_id,
            ),
            second_route: (
                second.proposal.descriptor.lane_id,
                second.proposal.descriptor.dataspace_id,
            ),
        }
    }

    fn kv(key: &str, value: &str) -> ExecKv {
        ExecKv {
            key: key.as_bytes().to_vec(),
            value: value.as_bytes().to_vec(),
        }
    }

    fn witness(reads: Vec<ExecKv>, writes: Vec<ExecKv>) -> ExecWitness {
        ExecWitness {
            reads,
            writes,
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        }
    }

    #[test]
    fn post_root_projection_matches_formal_empty_pure_read_write_and_conflict_cases() {
        let empty = witness(Vec::new(), Vec::new());
        assert_eq!(
            post_state_from_witness(&empty),
            compute_post_state_root(&[], &[])
        );

        let pure_reads = witness(vec![kv("account", "old")], Vec::new());
        assert_eq!(
            post_state_from_witness(&pure_reads),
            compute_post_state_root(&[KvPair::new(b"account", b"old")], &[])
        );
        assert_ne!(
            post_state_from_witness(&pure_reads),
            post_state_from_witness(&empty)
        );

        let writes_with_incidental_reads = witness(
            vec![kv("account", "old"), kv("permission-cache", "true")],
            vec![kv("account", "new")],
        );
        let writes_only = witness(Vec::new(), vec![kv("account", "new")]);
        assert_eq!(
            post_state_from_witness(&writes_with_incidental_reads),
            post_state_from_witness(&writes_only)
        );
        assert_ne!(
            post_state_from_witness(&writes_with_incidental_reads),
            post_state_from_witness(&pure_reads)
        );
    }

    #[test]
    fn parent_root_projection_matches_formal_empty_read_only_and_write_filter_cases() {
        let empty = witness(Vec::new(), Vec::new());
        assert_eq!(
            parent_state_from_witness(&empty),
            compute_post_state_root(&[], &[])
        );

        let read_only = witness(vec![kv("config", "1"), kv("other", "2")], Vec::new());
        assert_eq!(
            parent_state_from_witness(&read_only),
            compute_post_state_root(
                &[KvPair::new(b"config", b"1"), KvPair::new(b"other", b"2")],
                &[]
            )
        );

        let witness_with_writes = witness(
            vec![kv("balance", "10"), kv("permission-cache", "true")],
            vec![kv("balance", "7"), kv("write-only", "created")],
        );
        let parent = parent_state_from_witness(&witness_with_writes);
        assert_eq!(
            parent,
            compute_post_state_root(&[KvPair::new(b"balance", b"10")], &[])
        );

        let changed_write_values = witness(
            witness_with_writes.reads.clone(),
            vec![kv("balance", "999"), kv("write-only", "different")],
        );
        assert_eq!(parent, parent_state_from_witness(&changed_write_values));
        assert_ne!(parent, post_state_from_witness(&witness_with_writes));
    }

    #[test]
    fn root_projection_is_order_independent_and_deduplicates_identical_keys() {
        let ordered = witness(
            vec![kv("a", "old-a"), kv("b", "old-b")],
            vec![kv("a", "new-a"), kv("b", "new-b")],
        );
        let reordered = witness(
            vec![kv("b", "old-b"), kv("a", "old-a")],
            vec![kv("b", "new-b"), kv("a", "new-a")],
        );
        assert_eq!(
            post_state_from_witness(&ordered),
            post_state_from_witness(&reordered)
        );
        assert_eq!(
            parent_state_from_witness(&ordered),
            parent_state_from_witness(&reordered)
        );

        let duplicated_reads = witness(vec![kv("config", "1"), kv("config", "1")], Vec::new());
        let single_read = witness(vec![kv("config", "1")], Vec::new());
        assert_eq!(
            post_state_from_witness(&duplicated_reads),
            post_state_from_witness(&single_read)
        );

        let duplicated_writes = witness(Vec::new(), vec![kv("balance", "7"), kv("balance", "7")]);
        let single_write = witness(Vec::new(), vec![kv("balance", "7")]);
        assert_eq!(
            post_state_from_witness(&duplicated_writes),
            post_state_from_witness(&single_write)
        );
    }

    #[test]
    fn v2_execution_commitment_exposes_exact_bounded_topup_projection() {
        let mut operation_key = vec![super::super::smt::KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG];
        operation_key.extend_from_slice(&[0xA1; 32]);
        let witness = ExecWitness {
            reads: vec![ExecKv {
                key: operation_key.clone(),
                value: Vec::new(),
            }],
            writes: vec![
                ExecKv {
                    key: b"ordinary".to_vec(),
                    value: b"write".to_vec(),
                },
                ExecKv {
                    key: operation_key,
                    value: vec![0xB2; 32],
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };

        let executed_block_wire_hash = Hash::new(b"executed block wire");
        let native_manifest = NativeAmxApplicationManifestV1::empty(executed_block_wire_hash);
        let commitment = execution_commitment_from_witness(&witness, &native_manifest)
            .expect("valid top-up commitment");
        assert_eq!(commitment.topup_anchor_count, 1);
        assert!(commitment.topup_anchor_root.is_some());
        assert_eq!(commitment.validate(), Ok(()));
        assert_eq!(
            commitment.executed_block_wire_hash,
            executed_block_wire_hash
        );
        assert_eq!(
            commitment.post_state_root,
            try_post_state_from_witness(&witness).expect("same consensus post root")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn native_amx_manifest_reconstructs_grouped_mixed_routes_and_binds_wire_and_root() {
        let fixture = result_bearing_native_manifest_block();
        let context = fixture
            .block
            .execution_context()
            .and_then(|bundle| bundle.external.first())
            .expect("fixture has an external execution context");
        let coordinator_leg = context
            .routing_plan_legs
            .first()
            .expect("fixture has a coordinator leg");
        assert_eq!(
            coordinator_leg.role,
            ExternalExecutionRouteRole::Coordinator
        );
        assert_eq!(
            context
                .routing_plan_legs
                .iter()
                .filter(|leg| leg.role == ExternalExecutionRouteRole::Participant)
                .count(),
            3
        );
        assert!(
            context.routing_plan_legs.iter().skip(1).any(|leg| {
                leg.role == ExternalExecutionRouteRole::Participant
                    && leg.lane_id == coordinator_leg.lane_id
                    && leg.dataspace_id == coordinator_leg.dataspace_id
            }),
            "fixture must exercise the coordinator route in both plan roles"
        );
        let manifest = NativeAmxApplicationManifestV1::from_result_bearing_block(&fixture.block)
            .expect("reconstruct canonical Native AMX manifest");
        assert_eq!(
            manifest.count(),
            2,
            "same-route coordinator evidence must not create a third leaf"
        );
        assert_eq!(
            manifest
                .entries()
                .iter()
                .map(|entry| (entry.leaf.lane_id, entry.leaf.dataspace_id))
                .collect::<Vec<_>>(),
            vec![fixture.first_route, fixture.second_route],
            "multi-route leaves must use canonical lane/dataspace order"
        );
        for entry in manifest.entries() {
            assert_eq!(
                entry
                    .leaf
                    .members
                    .iter()
                    .map(|member| member.source_id)
                    .collect::<Vec<_>>(),
                fixture.source_ids.to_vec(),
                "each grouped route must cover both block sources in order"
            );
            assert_eq!(
                entry
                    .leaf
                    .members
                    .iter()
                    .map(|member| member.entrypoint_index)
                    .collect::<Vec<_>>(),
                vec![0, 1]
            );
        }

        let typed_root =
            HashOf::<MerkleTree<wire::NativeAmxApplicationManifestLeafV1>>::from_untyped_unchecked(
                manifest.root(),
            );
        for (index, entry) in manifest.entries().iter().enumerate() {
            let proof = manifest
                .proof(u32::try_from(index).expect("fixture proof index fits u32"))
                .expect("manifest inclusion proof");
            assert!(
                proof.verify(&HashOf::new(&entry.leaf), &typed_root, 32),
                "canonical route leaf must verify against the committed root"
            );
        }
        let forged_root =
            HashOf::<MerkleTree<wire::NativeAmxApplicationManifestLeafV1>>::from_untyped_unchecked(
                Hash::new(b"forged Native AMX manifest root"),
            );
        assert!(
            !manifest.proof(0).expect("first manifest proof").verify(
                &HashOf::new(&manifest.entries()[0].leaf),
                &forged_root,
                32,
            ),
            "the canonical proof must reject a substituted QC root"
        );
        let mut wire_identity_tampered_leaf = manifest.entries()[0].leaf.clone();
        wire_identity_tampered_leaf.executed_block_wire_hash =
            Hash::new(b"forged Native AMX executed block wire");
        assert!(
            !manifest.proof(0).expect("first manifest proof").verify(
                &HashOf::new(&wire_identity_tampered_leaf),
                &typed_root,
                32,
            ),
            "the committed proof must reject an executed-wire identity substitution"
        );

        let mut wire_tampered = fixture.block.clone();
        let extra_signer = fixture_key(0x42, Algorithm::BlsNormal);
        wire_tampered
            .try_sign(extra_signer.private_key(), 1)
            .expect("add a valid extra block signature");
        let wire_tampered_manifest =
            NativeAmxApplicationManifestV1::from_result_bearing_block(&wire_tampered)
                .expect("reconstruct signature-tampered executed wire");
        assert_eq!(wire_tampered.hash(), fixture.block.hash());
        assert_ne!(
            wire_tampered_manifest.executed_block_wire_hash(),
            manifest.executed_block_wire_hash(),
            "any canonical executed-wire change must change the manifest wire identity"
        );
        assert_ne!(
            wire_tampered_manifest.root(),
            manifest.root(),
            "the executed-wire identity in every leaf must make the manifest root change"
        );

        let mut result_root_tampered = fixture.block.clone();
        let header = result_root_tampered.header();
        let execution_context = result_root_tampered.execution_context().cloned();
        let forged_header = BlockHeader::new(
            header.height(),
            header.prev_block_hash(),
            header.merkle_root(),
            Some(HashOf::from_untyped_unchecked(Hash::new(
                b"forged Native AMX result root",
            ))),
            u64::try_from(header.creation_time().as_millis())
                .expect("fixture creation time fits u64"),
            header.view_change_index(),
        );
        result_root_tampered.replace_header_for_testing(forged_header);
        result_root_tampered.set_execution_context(execution_context);
        assert!(
            NativeAmxApplicationManifestV1::from_result_bearing_block(&result_root_tampered)
                .is_err(),
            "a header/result-tree mismatch must not reconstruct an authenticated manifest"
        );
    }

    #[test]
    fn roots_ignore_fastpq_payloads_match_formal_gate() {
        use iroha_data_model::fastpq::{
            FastpqOperationKind, FastpqPublicInputs, FastpqStateTransition, FastpqTransitionBatch,
            TransferTranscriptBundle,
        };

        let base = witness(vec![kv("balance", "10")], vec![kv("balance", "7")]);
        let mut with_fastpq = base.clone();
        with_fastpq
            .fastpq_transcripts
            .push(TransferTranscriptBundle {
                entry_hash: Hash::prehashed([0x11; Hash::LENGTH]),
                transcripts: Vec::new(),
            });
        with_fastpq.fastpq_batches.push(FastpqTransitionBatch {
            parameter: String::from("test-params"),
            public_inputs: FastpqPublicInputs {
                dsid: [0x01; 16],
                slot: 7,
                old_root: [0x02; 32],
                new_root: [0x03; 32],
                perm_root: [0x04; 32],
                tx_set_hash: [0x05; 32],
            },
            transitions: vec![FastpqStateTransition {
                key: b"fastpq-key".to_vec(),
                pre_value: b"fastpq-pre".to_vec(),
                post_value: b"fastpq-post".to_vec(),
                operation: FastpqOperationKind::Transfer,
            }],
            metadata: std::collections::BTreeMap::from([(String::from("entry"), vec![0xAA])]),
        });

        assert_eq!(
            post_state_from_witness(&base),
            post_state_from_witness(&with_fastpq)
        );
        assert_eq!(
            parent_state_from_witness(&base),
            parent_state_from_witness(&with_fastpq)
        );
    }
}

#[cfg(test)]
pub(crate) fn result_bearing_native_manifest_block_for_tests() -> SignedBlock {
    tests::result_bearing_native_manifest_block().block
}
