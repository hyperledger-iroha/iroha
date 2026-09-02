//! Native Pasta aggregate-state relation for Offline Cash V1.
//!
//! The relation has one fixed shape for all seven operations. It commits every private state
//! field with the parity-native Poseidon construction, proves exact `u128` balance/sequence
//! arithmetic, and always evaluates one full 256-level consumed-credit path. `MintFold` and
//! `ReceiveFold` select that path. All other operations carry the replay root. A 256-bit credit ID therefore has
//! no history/count admission cap and retains a 128-bit collision-security target under the
//! width-3 Poseidon permutation.

use core::marker::PhantomData;

use halo2_base::{
    AssignedValue, Context,
    QuantumCell::Constant,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
};
use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_ASSET_SCALE_MAX_V1, OFFLINE_CASH_HALO2_K_V1, OfflineCashPastaStateCommitmentV1,
};

use super::{OfflineCashOperationV1, OfflineCashReplayInsertWitnessV1};
use crate::zk::{
    offline_cash_v1_poseidon::{
        OFFLINE_CASH_FP_MODULUS_LOW_V1, OFFLINE_CASH_FQ_MODULUS_LOW_V1,
        OFFLINE_CASH_REPLAY_EMPTY_DOMAIN_V1, OFFLINE_CASH_REPLAY_LEAF_DOMAIN_V1,
        OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1, OFFLINE_CASH_STATE_DOMAIN_V1,
        OfflineCashPoseidonChipV1, OfflineCashPoseidonFieldV1, decode, digest_limbs,
        empty_replay_root, from_u128,
    },
    offline_cash_v1_state::{
        DigestV1, OFFLINE_CASH_CONSUMED_CREDIT_TREE_DEPTH_V1, OfflineCashStateV1,
    },
};

pub(super) const PUBLIC_INSTANCE_COUNT: usize = 77;
const MINIMUM_UNUSABLE_ROWS: usize = 9;

/// Public-instance positions shared by both state-proof parities.
pub mod public_instance {
    /// Seven-operation tag.
    pub const OPERATION: usize = 0;
    /// Exact monetary amount; zero only for bootstrap and rotation.
    pub const AMOUNT: usize = 1;
    /// Low 128 bits of the canonical transport statement digest.
    pub const TRANSPORT_LO: usize = 2;
    /// High 128 bits of the canonical transport statement digest.
    pub const TRANSPORT_HI: usize = 3;
    /// Low 128 bits of the normalized GuardBundle statement digest.
    pub const GUARD_LO: usize = 4;
    /// High 128 bits of the normalized GuardBundle statement digest.
    pub const GUARD_HI: usize = 5;
    /// Low 128 bits of the compact predecessor state commitment.
    pub const PREDECESSOR_OUTER_LO: usize = 6;
    /// High 128 bits of the compact predecessor state commitment.
    pub const PREDECESSOR_OUTER_HI: usize = 7;
    /// Low 128 bits of the compact successor state commitment.
    pub const SUCCESSOR_OUTER_LO: usize = 8;
    /// High 128 bits of the compact successor state commitment.
    pub const SUCCESSOR_OUTER_HI: usize = 9;
    /// Low 128 bits of the mint statement digest, nonzero only for `MintFold`.
    pub const MINT_SEMANTIC_LO: usize = 10;
    /// High 128 bits of the mint statement digest, nonzero only for `MintFold`.
    pub const MINT_SEMANTIC_HI: usize = 11;
    /// Low 128 bits of the authenticated proof release.
    pub const RELEASE_LO: usize = 12;
    /// High 128 bits of the authenticated proof release.
    pub const RELEASE_HI: usize = 13;
    /// Low 128 bits of the reserve liability pool.
    pub const LIABILITY_POOL_LO: usize = 14;
    /// High 128 bits of the reserve liability pool.
    pub const LIABILITY_POOL_HI: usize = 15;
    /// Low 128 bits of the receiver-bound credit ID; zero outside peer send/receive.
    pub const PEER_CREDIT_LO: usize = 16;
    /// High 128 bits of the receiver-bound credit ID; zero outside peer send/receive.
    pub const PEER_CREDIT_HI: usize = 17;
    /// Low 128 bits of the peer recipient lane; zero outside peer send/receive.
    pub const PEER_RECIPIENT_LANE_LO: usize = 18;
    /// High 128 bits of the peer recipient lane; zero outside peer send/receive.
    pub const PEER_RECIPIENT_LANE_HI: usize = 19;
    /// Low 128 bits of the common predecessor Eq component.
    pub const PREDECESSOR_EQ_COMPONENT_LO: usize = 20;
    /// High 128 bits of the common predecessor Eq component.
    pub const PREDECESSOR_EQ_COMPONENT_HI: usize = 21;
    /// Low 128 bits of the common predecessor Ep component.
    pub const PREDECESSOR_EP_COMPONENT_LO: usize = 22;
    /// High 128 bits of the common predecessor Ep component.
    pub const PREDECESSOR_EP_COMPONENT_HI: usize = 23;
    /// Low 128 bits of the common successor Eq component.
    pub const SUCCESSOR_EQ_COMPONENT_LO: usize = 24;
    /// High 128 bits of the common successor Eq component.
    pub const SUCCESSOR_EQ_COMPONENT_HI: usize = 25;
    /// Low 128 bits of the common successor Ep component.
    pub const SUCCESSOR_EP_COMPONENT_LO: usize = 26;
    /// High 128 bits of the common successor Ep component.
    pub const SUCCESSOR_EP_COMPONENT_HI: usize = 27;
    /// Parity-native predecessor state component, zero only for bootstrap.
    pub const PREDECESSOR_STATE: usize = 28;
    /// Parity-native successor state component.
    pub const SUCCESSOR_STATE: usize = 29;
    /// Low 128 bits of the canonical Fp Eq compiled-protocol identity.
    pub const EQ_PROTOCOL_LO: usize = 30;
    /// High 128 bits of the canonical Fp Eq compiled-protocol identity.
    pub const EQ_PROTOCOL_HI: usize = 31;
    /// Low 128 bits of the canonical Fq Ep compiled-protocol identity.
    pub const EP_PROTOCOL_LO: usize = 32;
    /// High 128 bits of the canonical Fq Ep compiled-protocol identity.
    pub const EP_PROTOCOL_HI: usize = 33;
    /// Low 128 bits of the canonical Fp Eq GuardBundle compiled-protocol identity.
    pub const GUARD_EQ_PROTOCOL_LO: usize = 34;
    /// High 128 bits of the canonical Fp Eq GuardBundle compiled-protocol identity.
    pub const GUARD_EQ_PROTOCOL_HI: usize = 35;
    /// Low 128 bits of the canonical Fq Ep GuardBundle compiled-protocol identity.
    pub const GUARD_EP_PROTOCOL_LO: usize = 36;
    /// High 128 bits of the canonical Fq Ep GuardBundle compiled-protocol identity.
    pub const GUARD_EP_PROTOCOL_HI: usize = 37;
    /// Low 128 bits of the canonical Fp Eq finalized-mint compiled-protocol identity.
    pub const MINT_EQ_PROTOCOL_LO: usize = 38;
    /// High 128 bits of the canonical Fp Eq finalized-mint compiled-protocol identity.
    pub const MINT_EQ_PROTOCOL_HI: usize = 39;
    /// Low 128 bits of the canonical Fq Ep finalized-mint compiled-protocol identity.
    pub const MINT_EP_PROTOCOL_LO: usize = 40;
    /// High 128 bits of the canonical Fq Ep finalized-mint compiled-protocol identity.
    pub const MINT_EP_PROTOCOL_HI: usize = 41;
    /// Low 128 bits of the Eq credential audit recursively accepted by GuardBundle.
    pub const GUARD_EQ_CREDENTIAL_AUDIT_LO: usize = 42;
    /// High 128 bits of the Eq credential audit recursively accepted by GuardBundle.
    pub const GUARD_EQ_CREDENTIAL_AUDIT_HI: usize = 43;
    /// Low 128 bits of the Ep credential audit recursively accepted by GuardBundle.
    pub const GUARD_EP_CREDENTIAL_AUDIT_LO: usize = 44;
    /// High 128 bits of the Ep credential audit recursively accepted by GuardBundle.
    pub const GUARD_EP_CREDENTIAL_AUDIT_HI: usize = 45;
    /// Low 128 bits of the Eq scalar-verifier deferred-equation audit.
    pub const EQ_DEFERRED_AUDIT_LO: usize = 46;
    /// High 128 bits of the Eq scalar-verifier deferred-equation audit.
    pub const EQ_DEFERRED_AUDIT_HI: usize = 47;
    /// Low 128 bits of the Ep scalar-verifier deferred-equation audit.
    pub const EP_DEFERRED_AUDIT_LO: usize = 48;
    /// High 128 bits of the Ep scalar-verifier deferred-equation audit.
    pub const EP_DEFERRED_AUDIT_HI: usize = 49;
    /// Low 128 bits of the exact paired mint-helper proof binding.
    pub const MINT_PROOF_BINDING_LO: usize = 50;
    /// High 128 bits of the exact paired mint-helper proof binding.
    pub const MINT_PROOF_BINDING_HI: usize = 51;
    /// Low 128 bits of the released lifecycle binding.
    pub const LIFECYCLE_LO: usize = 52;
    /// High 128 bits of the released lifecycle binding.
    pub const LIFECYCLE_HI: usize = 53;
    /// Low 128 bits of the precommit request/ticket/reservation binding.
    pub const PRECOMMIT_LO: usize = 54;
    /// High 128 bits of the precommit request/ticket/reservation binding.
    pub const PRECOMMIT_HI: usize = 55;
    /// Low 128 bits of the consumed suite identity.
    pub const PREDECESSOR_SUITE_LO: usize = 56;
    /// High 128 bits of the consumed suite identity.
    pub const PREDECESSOR_SUITE_HI: usize = 57;
    /// Low 128 bits of the consumed verifier-key digest.
    pub const PREDECESSOR_VK_LO: usize = 58;
    /// High 128 bits of the consumed verifier-key digest.
    pub const PREDECESSOR_VK_HI: usize = 59;
    /// Low 128 bits of the produced suite identity.
    pub const SUCCESSOR_SUITE_LO: usize = 60;
    /// High 128 bits of the produced suite identity.
    pub const SUCCESSOR_SUITE_HI: usize = 61;
    /// Low 128 bits of the produced verifier-key digest.
    pub const SUCCESSOR_VK_LO: usize = 62;
    /// High 128 bits of the produced verifier-key digest.
    pub const SUCCESSOR_VK_HI: usize = 63;
    /// Low 128 bits of the authenticated suite-upgrade bridge.
    pub const SUITE_UPGRADE_AUTHORIZATION_LO: usize = 64;
    /// High 128 bits of the authenticated suite-upgrade bridge.
    pub const SUITE_UPGRADE_AUTHORIZATION_HI: usize = 65;
    /// Exact protocol version carried by the aggregate state.
    pub const PROTOCOL_VERSION: usize = 66;
    /// Low 128 bits of the exact typed asset incarnation carried by the aggregate state.
    pub const ASSET_INCARNATION_LO: usize = 67;
    /// High 128 bits of the exact typed asset incarnation carried by the aggregate state.
    pub const ASSET_INCARNATION_HI: usize = 68;
    /// Low 128 bits of the qualified hardware profile.
    pub const HARDWARE_PROFILE_LO: usize = 69;
    /// High 128 bits of the qualified hardware profile.
    pub const HARDWARE_PROFILE_HI: usize = 70;
    /// Governed hardware policy epoch.
    pub const POLICY_EPOCH: usize = 71;
    /// Low 128 bits of the exact network identity.
    pub const NETWORK_LO: usize = 72;
    /// High 128 bits of the exact network identity.
    pub const NETWORK_HI: usize = 73;
    /// Low 128 bits of the canonical typed asset digest.
    pub const ASSET_LO: usize = 74;
    /// High 128 bits of the canonical typed asset digest.
    pub const ASSET_HI: usize = 75;
    /// Authoritative asset scale.
    pub const ASSET_SCALE: usize = 76;
}

/// One private credit in the single-credit receive-fold relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashReceiveFoldCreditV1 {
    /// Positive credit amount in atomic units.
    pub amount: u128,
    /// Exact receiver-bound credit identity.
    pub credit_id: DigestV1,
    /// Recipient lane committed by the incoming sender proof.
    pub recipient_lane_id: DigestV1,
    /// Binding of the exact incoming paired sender proof and history.
    pub incoming_proof_binding_digest: DigestV1,
    /// Exact empty-to-present replay insertion.
    pub replay_insert: OfflineCashReplayInsertWitnessV1,
}

/// Public values reconstructed by a verifier for one aggregate-state proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashStateRelationPublicInputsV1 {
    /// Selected state transition.
    pub operation: OfflineCashOperationV1,
    /// Consumed state; absent only for bootstrap.
    pub predecessor: Option<OfflineCashStateV1>,
    /// Produced state.
    pub successor: OfflineCashStateV1,
    /// Positive monetary amount, or zero for bootstrap/suite-upgrade/rotation.
    pub amount: u128,
    /// Durable journal revision consumed by this transition.
    pub journal_revision_before: u128,
    /// Exact-next durable journal revision produced by this transition.
    pub journal_revision_after: u128,
    /// Exact operation-effect digest authorized by GuardBundle.
    pub transition_effect_digest: DigestV1,
    /// Finalized-mint semantic digest, nonzero only for `MintFold`.
    pub mint_finality_semantic_digest: DigestV1,
    /// Paired mint-helper proof binding, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Receiver-bound credit identity, nonzero for `SendSplit` and `ReceiveFold`.
    pub peer_credit_id: DigestV1,
    /// Receiver lane, nonzero for `SendSplit` and `ReceiveFold`.
    pub peer_recipient_lane_id: DigestV1,
    /// Released lifecycle digest.
    pub lifecycle_binding_digest: DigestV1,
    /// Send/redemption precommit binding.
    pub precommit_binding_digest: DigestV1,
    /// Suite-upgrade authorization binding.
    pub suite_upgrade_authorization_digest: DigestV1,
    /// Common transport statement digest.
    pub transport_semantic_digest: DigestV1,
    /// Normalized GuardBundle statement digest.
    pub guard_statement_digest: DigestV1,
    /// Eq state protocol identity.
    pub eq_protocol_digest: DigestV1,
    /// Ep state protocol identity.
    pub ep_protocol_digest: DigestV1,
    /// Eq GuardBundle protocol identity.
    pub guard_eq_protocol_digest: DigestV1,
    /// Ep GuardBundle protocol identity.
    pub guard_ep_protocol_digest: DigestV1,
    /// Eq mint-authority protocol identity.
    pub mint_eq_protocol_digest: DigestV1,
    /// Ep mint-authority protocol identity.
    pub mint_ep_protocol_digest: DigestV1,
    /// Eq credential audit exposed by GuardBundle.
    pub guard_eq_credential_audit: DigestV1,
    /// Ep credential audit exposed by GuardBundle.
    pub guard_ep_credential_audit: DigestV1,
    /// Eq deferred-equation audit.
    pub eq_deferred_audit: DigestV1,
    /// Ep deferred-equation audit.
    pub ep_deferred_audit: DigestV1,
}

/// Complete private witness for the fixed aggregate state relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashStateRelationWitnessV1 {
    /// Selected state transition.
    pub operation: OfflineCashOperationV1,
    /// Consumed state; absent only for bootstrap.
    pub predecessor: Option<OfflineCashStateV1>,
    /// Produced state.
    pub successor: OfflineCashStateV1,
    /// Positive monetary amount, or zero for bootstrap/suite-upgrade/rotation.
    /// For `ReceiveFold` this is the received credit amount.
    pub amount: u128,
    /// Durable journal revision consumed by this transition.
    pub journal_revision_before: u128,
    /// Exact-next durable journal revision produced by this transition.
    pub journal_revision_after: u128,
    /// Exact operation-effect digest authorized by the normalized GuardBundle.
    pub transition_effect_digest: DigestV1,
    /// Exact finalized-mint statement digest, nonzero only for `MintFold`.
    pub mint_finality_semantic_digest: DigestV1,
    /// Exact binding of both mint-helper parities, nonzero only for `MintFold`.
    pub mint_finality_proof_binding_digest: DigestV1,
    /// Receiver-bound peer credit identity, nonzero for `SendSplit` and `ReceiveFold`.
    pub peer_credit_id: DigestV1,
    /// Receiver lane authorized by the peer credit, nonzero for `SendSplit` and `ReceiveFold`.
    pub peer_recipient_lane_id: DigestV1,
    /// Exact peer credit consumed by `ReceiveFold`; absent for every other operation.
    pub receive_credit: Option<OfflineCashReceiveFoldCreditV1>,
    /// Exact released lifecycle digest copied into the precommit candidate.
    pub lifecycle_binding_digest: DigestV1,
    /// Hiding binding of the request, one-use ticket, and outbox reservation.
    /// Nonzero exactly for `SendSplit` and `RedeemSplit`.
    pub precommit_binding_digest: DigestV1,
    /// Recursive authorization of the old/new suite and verifier-key set.
    /// Nonzero exactly for `SuiteUpgrade`.
    pub suite_upgrade_authorization_digest: DigestV1,
    /// Exact common transport statement digest.
    pub transport_semantic_digest: DigestV1,
    /// Exact normalized hardware GuardBundle statement digest.
    pub guard_statement_digest: DigestV1,
    /// Native Fp Poseidon identity of the exact Eq compiled protocol.
    pub eq_protocol_digest: DigestV1,
    /// Native Fq Poseidon identity of the exact Ep compiled protocol.
    pub ep_protocol_digest: DigestV1,
    /// Native Fp Poseidon identity of the exact Eq GuardBundle compiled protocol.
    pub guard_eq_protocol_digest: DigestV1,
    /// Native Fq Poseidon identity of the exact Ep GuardBundle compiled protocol.
    pub guard_ep_protocol_digest: DigestV1,
    /// Native Fp Poseidon identity of the exact Eq finalized-mint compiled protocol.
    pub mint_eq_protocol_digest: DigestV1,
    /// Native Fq Poseidon identity of the exact Ep finalized-mint compiled protocol.
    pub mint_ep_protocol_digest: DigestV1,
    /// Native Fp Poseidon audit of the credential proofs accepted by GuardBundle.
    pub guard_eq_credential_audit: DigestV1,
    /// Native Fq Poseidon audit of the credential proofs accepted by GuardBundle.
    pub guard_ep_credential_audit: DigestV1,
    /// Native Fp Poseidon audit of every Eq scalar-verifier curve equation.
    pub eq_deferred_audit: DigestV1,
    /// Native Fq Poseidon audit of every Ep scalar-verifier curve equation.
    pub ep_deferred_audit: DigestV1,
    /// Empty-to-present replay insertion for `MintFold`, absent otherwise.
    pub replay_insert: Option<OfflineCashReplayInsertWitnessV1>,
}

impl OfflineCashStateRelationWitnessV1 {
    /// Validate host-side shape before constructing a prover circuit.
    ///
    /// This is only an early diagnostic; every monetary relation checked here is independently
    /// constrained by the circuit.
    pub fn validate(&self) -> Result<(), String> {
        self.successor
            .validate()
            .map_err(|error| format!("invalid successor state: {error}"))?;
        if self.transport_semantic_digest == [0; 32]
            || self.guard_statement_digest == [0; 32]
            || self.transition_effect_digest == [0; 32]
            || self.eq_protocol_digest == [0; 32]
            || self.ep_protocol_digest == [0; 32]
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.guard_eq_protocol_digest == [0; 32]
            || self.guard_ep_protocol_digest == [0; 32]
            || self.guard_eq_protocol_digest == self.guard_ep_protocol_digest
            || self.guard_eq_protocol_digest == self.eq_protocol_digest
            || self.guard_ep_protocol_digest == self.ep_protocol_digest
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.eq_protocol_digest).is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.ep_protocol_digest).is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.guard_eq_protocol_digest)
                .is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.guard_ep_protocol_digest)
                .is_none()
            || self.mint_eq_protocol_digest == [0; 32]
            || self.mint_ep_protocol_digest == [0; 32]
            || self.mint_eq_protocol_digest == self.mint_ep_protocol_digest
            || self.mint_eq_protocol_digest == self.eq_protocol_digest
            || self.mint_ep_protocol_digest == self.ep_protocol_digest
            || self.mint_eq_protocol_digest == self.guard_eq_protocol_digest
            || self.mint_ep_protocol_digest == self.guard_ep_protocol_digest
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.mint_eq_protocol_digest)
                .is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.mint_ep_protocol_digest)
                .is_none()
            || self.guard_eq_credential_audit == [0; 32]
            || self.guard_ep_credential_audit == [0; 32]
            || self.guard_eq_credential_audit == self.guard_ep_credential_audit
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.guard_eq_credential_audit)
                .is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.guard_ep_credential_audit)
                .is_none()
            || self.eq_deferred_audit == [0; 32]
            || self.ep_deferred_audit == [0; 32]
            || self.eq_deferred_audit == self.ep_deferred_audit
        {
            return Err(
                "state, protocol, and deferred-audit digests must be canonical, nonzero, and role-distinct"
                    .to_owned(),
            );
        }
        let is_bootstrap = self.operation == OfflineCashOperationV1::Bootstrap;
        if is_bootstrap != self.predecessor.is_none() {
            return Err("only Bootstrap may omit the predecessor state".to_owned());
        }
        if let Some(predecessor) = &self.predecessor {
            predecessor
                .validate()
                .map_err(|error| format!("invalid predecessor state: {error}"))?;
        }
        if self.lifecycle_binding_digest == [0; 32] {
            return Err("released lifecycle binding must be nonzero".to_owned());
        }
        let is_mint = self.operation == OfflineCashOperationV1::MintFold;
        if is_mint != self.replay_insert.is_some() {
            return Err("only MintFold requires the singular replay insertion".to_owned());
        }
        if let Some(replay) = &self.replay_insert {
            replay.validate_shape().map_err(|error| error.to_string())?;
            if replay.predecessor_root
                != self
                    .predecessor
                    .as_ref()
                    .ok_or_else(|| "MintFold predecessor is absent".to_owned())?
                    .consumed_credit_root
                || replay.successor_root != self.successor.consumed_credit_root
            {
                return Err("MintFold replay roots do not match aggregate states".to_owned());
            }
        }
        let is_receive = self.operation == OfflineCashOperationV1::ReceiveFold;
        if is_receive {
            let predecessor = self
                .predecessor
                .as_ref()
                .ok_or_else(|| "ReceiveFold predecessor is absent".to_owned())?;
            let credit = self
                .receive_credit
                .as_ref()
                .ok_or_else(|| "ReceiveFold credit is absent".to_owned())?;
            credit
                .replay_insert
                .validate_shape()
                .map_err(|error| error.to_string())?;
            if credit.amount == 0
                || credit.amount != self.amount
                || credit.credit_id == [0; 32]
                || credit.recipient_lane_id == [0; 32]
                || credit.incoming_proof_binding_digest == [0; 32]
                || credit.credit_id != self.peer_credit_id
                || credit.recipient_lane_id != self.peer_recipient_lane_id
                || credit.credit_id != credit.replay_insert.credit_id
                || credit.recipient_lane_id != self.successor.lane.device_lane_id
                || credit.replay_insert.predecessor_root != predecessor.consumed_credit_root
                || credit.replay_insert.successor_root != self.successor.consumed_credit_root
            {
                return Err("invalid ReceiveFold credit or replay transition".to_owned());
            }
        } else if self.receive_credit.is_some() {
            return Err("receive credit must be absent outside ReceiveFold".to_owned());
        }
        if matches!(
            self.operation,
            OfflineCashOperationV1::Bootstrap
                | OfflineCashOperationV1::SuiteUpgrade
                | OfflineCashOperationV1::Rotate
        ) != (self.amount == 0)
        {
            return Err(
                "amount must be zero exactly for Bootstrap, SuiteUpgrade, and Rotate".to_owned(),
            );
        }
        if self.operation == OfflineCashOperationV1::Bootstrap {
            if self.journal_revision_before != 0 || self.journal_revision_after != 0 {
                return Err("Bootstrap journal revisions must both be zero".to_owned());
            }
        } else if self.operation == OfflineCashOperationV1::Rotate {
            if self.journal_revision_after != 0 {
                return Err("Rotate must reset the per-epoch journal revision".to_owned());
            }
        } else if self.journal_revision_after
            != self
                .journal_revision_before
                .checked_add(1)
                .ok_or_else(|| "journal revision overflow".to_owned())?
        {
            return Err("monetary journal revision must advance exactly once".to_owned());
        }
        if (self.operation == OfflineCashOperationV1::MintFold)
            != (self.mint_finality_semantic_digest != [0; 32])
        {
            return Err("mint semantic digest must be nonzero exactly for MintFold".to_owned());
        }
        if (self.operation == OfflineCashOperationV1::MintFold)
            != (self.mint_finality_proof_binding_digest != [0; 32])
        {
            return Err("mint proof binding must be nonzero exactly for MintFold".to_owned());
        }
        let is_peer = matches!(
            self.operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::ReceiveFold
        );
        if is_peer != (self.peer_credit_id != [0; 32])
            || is_peer != (self.peer_recipient_lane_id != [0; 32])
        {
            return Err(
                "peer credit bindings must be nonzero exactly for SendSplit and ReceiveFold"
                    .to_owned(),
            );
        }
        let uses_outbox = matches!(
            self.operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        );
        if uses_outbox != (self.precommit_binding_digest != [0; 32]) {
            return Err(
                "precommit binding must be nonzero exactly for send and redemption".to_owned(),
            );
        }
        let is_upgrade = self.operation == OfflineCashOperationV1::SuiteUpgrade;
        if is_upgrade != (self.suite_upgrade_authorization_digest != [0; 32]) {
            return Err(
                "suite-upgrade authorization must be nonzero exactly for SuiteUpgrade".to_owned(),
            );
        }
        if is_upgrade {
            let predecessor = self
                .predecessor
                .as_ref()
                .ok_or_else(|| "SuiteUpgrade predecessor is absent".to_owned())?;
            if predecessor.suite_id == self.successor.suite_id
                || predecessor.vk_digest == self.successor.vk_digest
                || predecessor.balance != self.successor.balance
                || predecessor.consumed_credit_root != self.successor.consumed_credit_root
                || predecessor.lane != self.successor.lane
                || predecessor.hardware_epoch != self.successor.hardware_epoch
                || predecessor.device_policy_binding != self.successor.device_policy_binding
                || predecessor.asset_incarnation != self.successor.asset_incarnation
                || predecessor.liability_pool_id != self.successor.liability_pool_id
                || predecessor.hardware_profile_id != self.successor.hardware_profile_id
                || predecessor.policy_epoch != self.successor.policy_epoch
            {
                return Err("SuiteUpgrade did not preserve the complete monetary state".to_owned());
            }
        }
        Ok(())
    }

    fn operation_tag(&self) -> u64 {
        match self.operation {
            OfflineCashOperationV1::Bootstrap => 0,
            OfflineCashOperationV1::MintFold => 1,
            OfflineCashOperationV1::SendSplit => 2,
            OfflineCashOperationV1::ReceiveFold => 3,
            OfflineCashOperationV1::RedeemSplit => 4,
            OfflineCashOperationV1::SuiteUpgrade => 5,
            OfflineCashOperationV1::Rotate => 6,
        }
    }

    /// Return one parity's exact public instance column.
    pub fn public_instances<F: OfflineCashPoseidonFieldV1>(&self) -> Result<Vec<F>, String> {
        self.validate()?;
        self.public_instances_unvalidated::<F>()
    }

    fn public_instances_unvalidated<F: OfflineCashPoseidonFieldV1>(
        &self,
    ) -> Result<Vec<F>, String> {
        let predecessor = match &self.predecessor {
            Some(state) => canonical_component::<F>(state.state_commitment_components)?,
            None => F::ZERO,
        };
        let successor = canonical_component::<F>(self.successor.state_commitment_components)?;
        let transport = digest_limbs::<F>(self.transport_semantic_digest);
        let guard = digest_limbs::<F>(self.guard_statement_digest);
        let predecessor_outer = self.predecessor.as_ref().map_or([F::ZERO; 2], |state| {
            digest_limbs::<F>(state.state_commitment)
        });
        let successor_outer = digest_limbs::<F>(self.successor.state_commitment);
        let mint_semantic = digest_limbs::<F>(self.mint_finality_semantic_digest);
        let mint_proof_binding = digest_limbs::<F>(self.mint_finality_proof_binding_digest);
        let release = digest_limbs::<F>(self.successor.release_id);
        let liability_pool = digest_limbs::<F>(self.successor.liability_pool_id);
        let peer_credit = digest_limbs::<F>(self.peer_credit_id);
        let peer_recipient_lane = digest_limbs::<F>(self.peer_recipient_lane_id);
        let predecessor_components = self
            .predecessor
            .as_ref()
            .map_or(OfflineCashPastaStateCommitmentV1::ZERO, |state| {
                state.state_commitment_components
            });
        let predecessor_eq = digest_limbs::<F>(predecessor_components.eq);
        let predecessor_ep = digest_limbs::<F>(predecessor_components.ep);
        let successor_eq = digest_limbs::<F>(self.successor.state_commitment_components.eq);
        let successor_ep = digest_limbs::<F>(self.successor.state_commitment_components.ep);
        let eq_protocol = digest_limbs::<F>(self.eq_protocol_digest);
        let ep_protocol = digest_limbs::<F>(self.ep_protocol_digest);
        let guard_eq_protocol = digest_limbs::<F>(self.guard_eq_protocol_digest);
        let guard_ep_protocol = digest_limbs::<F>(self.guard_ep_protocol_digest);
        let mint_eq_protocol = digest_limbs::<F>(self.mint_eq_protocol_digest);
        let mint_ep_protocol = digest_limbs::<F>(self.mint_ep_protocol_digest);
        let guard_eq_audit = digest_limbs::<F>(self.guard_eq_credential_audit);
        let guard_ep_audit = digest_limbs::<F>(self.guard_ep_credential_audit);
        let eq_audit = digest_limbs::<F>(self.eq_deferred_audit);
        let ep_audit = digest_limbs::<F>(self.ep_deferred_audit);
        let lifecycle = digest_limbs::<F>(self.lifecycle_binding_digest);
        let precommit = digest_limbs::<F>(self.precommit_binding_digest);
        let predecessor_suite = self
            .predecessor
            .as_ref()
            .map_or([F::ZERO; 2], |state| digest_limbs::<F>(state.suite_id));
        let predecessor_vk = self
            .predecessor
            .as_ref()
            .map_or([F::ZERO; 2], |state| digest_limbs::<F>(state.vk_digest));
        let successor_suite = digest_limbs::<F>(self.successor.suite_id);
        let successor_vk = digest_limbs::<F>(self.successor.vk_digest);
        let suite_upgrade = digest_limbs::<F>(self.suite_upgrade_authorization_digest);
        let asset_incarnation = digest_limbs::<F>(*self.successor.asset_incarnation.as_bytes());
        Ok(vec![
            F::from(self.operation_tag()),
            from_u128(self.amount),
            transport[0],
            transport[1],
            guard[0],
            guard[1],
            predecessor_outer[0],
            predecessor_outer[1],
            successor_outer[0],
            successor_outer[1],
            mint_semantic[0],
            mint_semantic[1],
            release[0],
            release[1],
            liability_pool[0],
            liability_pool[1],
            peer_credit[0],
            peer_credit[1],
            peer_recipient_lane[0],
            peer_recipient_lane[1],
            predecessor_eq[0],
            predecessor_eq[1],
            predecessor_ep[0],
            predecessor_ep[1],
            successor_eq[0],
            successor_eq[1],
            successor_ep[0],
            successor_ep[1],
            predecessor,
            successor,
            eq_protocol[0],
            eq_protocol[1],
            ep_protocol[0],
            ep_protocol[1],
            guard_eq_protocol[0],
            guard_eq_protocol[1],
            guard_ep_protocol[0],
            guard_ep_protocol[1],
            mint_eq_protocol[0],
            mint_eq_protocol[1],
            mint_ep_protocol[0],
            mint_ep_protocol[1],
            guard_eq_audit[0],
            guard_eq_audit[1],
            guard_ep_audit[0],
            guard_ep_audit[1],
            eq_audit[0],
            eq_audit[1],
            ep_audit[0],
            ep_audit[1],
            mint_proof_binding[0],
            mint_proof_binding[1],
            lifecycle[0],
            lifecycle[1],
            precommit[0],
            precommit[1],
            predecessor_suite[0],
            predecessor_suite[1],
            predecessor_vk[0],
            predecessor_vk[1],
            successor_suite[0],
            successor_suite[1],
            successor_vk[0],
            successor_vk[1],
            suite_upgrade[0],
            suite_upgrade[1],
            F::from(u64::from(self.successor.protocol_version)),
            asset_incarnation[0],
            asset_incarnation[1],
            digest_limbs::<F>(self.successor.hardware_profile_id)[0],
            digest_limbs::<F>(self.successor.hardware_profile_id)[1],
            F::from(self.successor.policy_epoch),
            digest_limbs::<F>(self.successor.lane.normalized_network_id())[0],
            digest_limbs::<F>(self.successor.lane.normalized_network_id())[1],
            digest_limbs::<F>(
                self.successor
                    .lane
                    .normalized_asset_id()
                    .map_err(|error| error.to_string())?,
            )[0],
            digest_limbs::<F>(
                self.successor
                    .lane
                    .normalized_asset_id()
                    .map_err(|error| error.to_string())?,
            )[1],
            F::from(u64::from(self.successor.lane.scale)),
        ])
    }
}

impl OfflineCashStateRelationPublicInputsV1 {
    /// Return one parity's verifier-reconstructed 77-cell state column.
    ///
    /// The private receive credit and replay path are intentionally absent: they are constrained by
    /// the proof but do not belong to its public instance ABI.
    pub fn public_instances<F: OfflineCashPoseidonFieldV1>(&self) -> Result<Vec<F>, String> {
        OfflineCashStateRelationWitnessV1 {
            operation: self.operation,
            predecessor: self.predecessor.clone(),
            successor: self.successor.clone(),
            amount: self.amount,
            journal_revision_before: self.journal_revision_before,
            journal_revision_after: self.journal_revision_after,
            transition_effect_digest: self.transition_effect_digest,
            mint_finality_semantic_digest: self.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: self.mint_finality_proof_binding_digest,
            peer_credit_id: self.peer_credit_id,
            peer_recipient_lane_id: self.peer_recipient_lane_id,
            receive_credit: None,
            lifecycle_binding_digest: self.lifecycle_binding_digest,
            precommit_binding_digest: self.precommit_binding_digest,
            suite_upgrade_authorization_digest: self.suite_upgrade_authorization_digest,
            transport_semantic_digest: self.transport_semantic_digest,
            guard_statement_digest: self.guard_statement_digest,
            eq_protocol_digest: self.eq_protocol_digest,
            ep_protocol_digest: self.ep_protocol_digest,
            guard_eq_protocol_digest: self.guard_eq_protocol_digest,
            guard_ep_protocol_digest: self.guard_ep_protocol_digest,
            mint_eq_protocol_digest: self.mint_eq_protocol_digest,
            mint_ep_protocol_digest: self.mint_ep_protocol_digest,
            guard_eq_credential_audit: self.guard_eq_credential_audit,
            guard_ep_credential_audit: self.guard_ep_credential_audit,
            eq_deferred_audit: self.eq_deferred_audit,
            ep_deferred_audit: self.ep_deferred_audit,
            replay_insert: None,
        }
        .public_instances_unvalidated::<F>()
    }
}

/// Fixed aggregate-state circuit used by both Eq/Fp and Ep/Fq artifacts.
#[derive(Clone, Debug)]
pub struct OfflineCashStateRelationCircuitV1<F> {
    witness: Option<OfflineCashStateRelationWitnessV1>,
    marker: PhantomData<F>,
}

impl<F> Default for OfflineCashStateRelationCircuitV1<F> {
    fn default() -> Self {
        Self {
            witness: None,
            marker: PhantomData,
        }
    }
}

impl<F> OfflineCashStateRelationCircuitV1<F>
where
    F: OfflineCashPoseidonFieldV1,
{
    /// Construct a witnessed relation after complete structural validation.
    pub fn new(witness: OfflineCashStateRelationWitnessV1) -> Result<Self, String> {
        witness.validate()?;
        Ok(Self {
            witness: Some(witness),
            marker: PhantomData,
        })
    }
}

impl<F> Circuit<F> for OfflineCashStateRelationCircuitV1<F>
where
    F: OfflineCashPoseidonFieldV1,
{
    type Config = BaseConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let params: BaseCircuitParams = relation_builder::<F>(None)
            .expect("witness-free Offline Cash state relation has a fixed valid shape")
            .config_params;
        BaseConfig::configure(meta, params)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let builder =
            relation_builder::<F>(self.witness.as_ref()).map_err(|_| PlonkError::Synthesis)?;
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(&builder, config, layouter)
    }
}

#[derive(Clone, Copy)]
pub(super) struct AssignedState<F: OfflineCashPoseidonFieldV1> {
    pub(super) protocol_version: AssignedValue<F>,
    pub(super) suite_id: [AssignedValue<F>; 2],
    pub(super) vk_digest: [AssignedValue<F>; 2],
    pub(super) balance: AssignedValue<F>,
    pub(super) sequence: AssignedValue<F>,
    pub(super) epoch_generation: AssignedValue<F>,
    pub(super) epoch_id: [AssignedValue<F>; 2],
    pub(super) key_reference: [AssignedValue<F>; 2],
    pub(super) policy_id: [AssignedValue<F>; 2],
    pub(super) nonce: [AssignedValue<F>; 2],
    pub(super) replay_root: AssignedValue<F>,
    pub(super) commitment: AssignedValue<F>,
    pub(super) release_id: [AssignedValue<F>; 2],
    pub(super) asset_incarnation: [AssignedValue<F>; 2],
    pub(super) liability_pool_id: [AssignedValue<F>; 2],
    pub(super) hardware_profile_id: [AssignedValue<F>; 2],
    pub(super) policy_epoch: AssignedValue<F>,
    pub(super) network_id: [AssignedValue<F>; 2],
    pub(super) asset_id: [AssignedValue<F>; 2],
    pub(super) scale: AssignedValue<F>,
    pub(super) lane_id: [AssignedValue<F>; 2],
}

/// Assigned cells for the single receive credit.
#[derive(Clone, Copy)]
pub(super) struct OfflineCashAssignedReceiveFoldCreditV1<F: OfflineCashPoseidonFieldV1> {
    pub(super) amount: AssignedValue<F>,
    pub(super) credit_id: [AssignedValue<F>; 2],
    pub(super) recipient_lane_id: [AssignedValue<F>; 2],
    pub(super) incoming_proof_binding_digest: [AssignedValue<F>; 2],
    pub(super) envelope_digest: [AssignedValue<F>; 2],
}

/// Assigned state-transition cells shared with the recursive GuardBundle relation.
#[derive(Clone, Copy)]
pub(super) struct OfflineCashAssignedStateRelationV1<F: OfflineCashPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) predecessor: AssignedState<F>,
    pub(super) successor: AssignedState<F>,
    pub(super) predecessor_outer: [AssignedValue<F>; 2],
    pub(super) successor_outer: [AssignedValue<F>; 2],
    pub(super) guard_digest: [AssignedValue<F>; 2],
    pub(super) journal_revision_before: AssignedValue<F>,
    pub(super) journal_revision_after: AssignedValue<F>,
    pub(super) transition_effect_digest: [AssignedValue<F>; 2],
    pub(super) mint_finality_semantic_digest: [AssignedValue<F>; 2],
    pub(super) mint_finality_proof_binding_digest: [AssignedValue<F>; 2],
    pub(super) peer_credit_id: [AssignedValue<F>; 2],
    pub(super) peer_recipient_lane_id: [AssignedValue<F>; 2],
    pub(super) receive_credit: OfflineCashAssignedReceiveFoldCreditV1<F>,
    pub(super) lifecycle_binding_digest: [AssignedValue<F>; 2],
    pub(super) precommit_binding_digest: [AssignedValue<F>; 2],
    pub(super) suite_upgrade_authorization_digest: [AssignedValue<F>; 2],
    pub(super) predecessor_eq_components: [AssignedValue<F>; 2],
    pub(super) predecessor_ep_components: [AssignedValue<F>; 2],
    pub(super) successor_eq_components: [AssignedValue<F>; 2],
    pub(super) successor_ep_components: [AssignedValue<F>; 2],
    pub(super) replay_credit_id: [AssignedValue<F>; 2],
    pub(super) replay_envelope_digest: [AssignedValue<F>; 2],
}

pub(super) fn relation_builder<F>(
    witness: Option<&OfflineCashStateRelationWitnessV1>,
) -> Result<BaseCircuitBuilder<F>, String>
where
    F: OfflineCashPoseidonFieldV1,
{
    relation_builder_with_bindings(witness).map(|(builder, _)| builder)
}

pub(super) fn relation_builder_with_bindings<F>(
    witness: Option<&OfflineCashStateRelationWitnessV1>,
) -> Result<(BaseCircuitBuilder<F>, OfflineCashAssignedStateRelationV1<F>), String>
where
    F: OfflineCashPoseidonFieldV1,
{
    if let Some(witness) = witness {
        witness.validate()?;
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(OFFLINE_CASH_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(
            usize::try_from(OFFLINE_CASH_HALO2_K_V1 - 1).expect("lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let poseidon = OfflineCashPoseidonChipV1::new(ctx, &range);

    let operation_tag = ctx.load_witness(F::from(witness.map_or(0, |value| value.operation_tag())));
    range.range_check(ctx, operation_tag, 3);
    let selectors: [AssignedValue<F>; 7] = core::array::from_fn(|tag| {
        gate.is_equal(ctx, operation_tag, Constant(F::from(tag as u64)))
    });
    let selector_sum = selectors
        .iter()
        .copied()
        .reduce(|left, right| gate.add(ctx, left, right))
        .expect("seven selectors");
    gate.assert_is_const(ctx, &selector_sum, &F::ONE);
    let bootstrap = selectors[0];
    let mint = selectors[1];
    let send = selectors[2];
    let receive = selectors[3];
    let redeem = selectors[4];
    let suite_upgrade = selectors[5];
    let rotate = selectors[6];
    let inbound = gate.add(ctx, mint, receive);
    let outbound = gate.add(ctx, send, redeem);
    let monetary = gate.add(ctx, inbound, outbound);
    let exact_next = gate.add(ctx, monetary, suite_upgrade);
    let no_inbound = gate.not(ctx, inbound);
    let non_bootstrap = gate.not(ctx, bootstrap);
    let active_successor = ctx.load_constant(F::ONE);

    let successor = assign_state(
        ctx,
        &range,
        &poseidon,
        witness.map(|value| &value.successor),
        active_successor,
    )?;
    let predecessor = assign_state(
        ctx,
        &range,
        &poseidon,
        witness.and_then(|value| value.predecessor.as_ref()),
        non_bootstrap,
    )?;
    let amount = assign_u128(ctx, &range, witness.map_or(0, |value| value.amount));
    let predecessor_outer = assign_digest(
        ctx,
        &range,
        witness
            .and_then(|value| value.predecessor.as_ref())
            .map_or([0; 32], |state| state.state_commitment),
    );
    let successor_outer = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.successor.state_commitment),
    );
    let predecessor_components = witness
        .and_then(|value| value.predecessor.as_ref())
        .map_or(OfflineCashPastaStateCommitmentV1::ZERO, |state| {
            state.state_commitment_components
        });
    let successor_components = witness.map_or(OfflineCashPastaStateCommitmentV1::ZERO, |value| {
        value.successor.state_commitment_components
    });
    let predecessor_eq_components = assign_digest(ctx, &range, predecessor_components.eq);
    let predecessor_ep_components = assign_digest(ctx, &range, predecessor_components.ep);
    let successor_eq_components = assign_digest(ctx, &range, successor_components.eq);
    let successor_ep_components = assign_digest(ctx, &range, successor_components.ep);
    assert_component_canonical(
        ctx,
        &range,
        predecessor_eq_components,
        OFFLINE_CASH_FP_MODULUS_LOW_V1,
        non_bootstrap,
    );
    assert_component_canonical(
        ctx,
        &range,
        predecessor_ep_components,
        OFFLINE_CASH_FQ_MODULUS_LOW_V1,
        non_bootstrap,
    );
    assert_component_canonical(
        ctx,
        &range,
        successor_eq_components,
        OFFLINE_CASH_FP_MODULUS_LOW_V1,
        active_successor,
    );
    assert_component_canonical(
        ctx,
        &range,
        successor_ep_components,
        OFFLINE_CASH_FQ_MODULUS_LOW_V1,
        active_successor,
    );
    let predecessor_selected_components = if F::IS_EQ_PARITY {
        predecessor_eq_components
    } else {
        predecessor_ep_components
    };
    let successor_selected_components = if F::IS_EQ_PARITY {
        successor_eq_components
    } else {
        successor_ep_components
    };
    let predecessor_component = compose_component(ctx, &range, predecessor_selected_components);
    let successor_component = compose_component(ctx, &range, successor_selected_components);
    assert_if_equal(
        ctx,
        &range,
        non_bootstrap,
        predecessor.commitment,
        predecessor_component,
    );
    assert_if_equal(
        ctx,
        &range,
        active_successor,
        successor.commitment,
        successor_component,
    );
    let journal_revision_before = assign_u128(
        ctx,
        &range,
        witness.map_or(0, |value| value.journal_revision_before),
    );
    let journal_revision_after = assign_u128(
        ctx,
        &range,
        witness.map_or(0, |value| value.journal_revision_after),
    );
    let transition_effect_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.transition_effect_digest),
    );
    let mint_finality_semantic_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.mint_finality_semantic_digest),
    );
    let mint_finality_proof_binding_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.mint_finality_proof_binding_digest),
    );
    let peer_credit_id = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.peer_credit_id),
    );
    let peer_recipient_lane_id = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.peer_recipient_lane_id),
    );
    let zero = ctx.load_zero();
    let not_mint = gate.not(ctx, mint);

    // Exact arithmetic and monotonicity.
    assert_if_equal(ctx, &range, bootstrap, successor.balance, Constant(F::ZERO));
    assert_if_equal(
        ctx,
        &range,
        bootstrap,
        successor.sequence,
        Constant(F::ZERO),
    );
    assert_if_equal(ctx, &range, bootstrap, amount, Constant(F::ZERO));
    assert_if_equal(
        ctx,
        &range,
        bootstrap,
        successor.replay_root,
        Constant(empty_replay_root::<F>()),
    );
    let inbound_sum = gate.add(ctx, predecessor.balance, amount);
    assert_if_equal(ctx, &range, inbound, successor.balance, inbound_sum);
    let outbound_sum = gate.add(ctx, successor.balance, amount);
    assert_if_equal(ctx, &range, outbound, predecessor.balance, outbound_sum);
    assert_if_nonzero(ctx, &range, monetary, amount);
    assert_if_equal(ctx, &range, suite_upgrade, amount, Constant(F::ZERO));
    let next_sequence = gate.inc(ctx, predecessor.sequence);
    assert_if_equal(ctx, &range, exact_next, successor.sequence, next_sequence);
    assert_if_equal(ctx, &range, rotate, successor.sequence, Constant(F::ZERO));
    assert_if_equal(ctx, &range, rotate, successor.balance, predecessor.balance);
    assert_if_equal(
        ctx,
        &range,
        suite_upgrade,
        successor.balance,
        predecessor.balance,
    );
    assert_if_equal(ctx, &range, rotate, amount, Constant(F::ZERO));
    assert_if_equal(
        ctx,
        &range,
        bootstrap,
        journal_revision_before,
        Constant(F::ZERO),
    );
    assert_if_equal(
        ctx,
        &range,
        bootstrap,
        journal_revision_after,
        Constant(F::ZERO),
    );
    let next_journal_revision = gate.inc(ctx, journal_revision_before);
    assert_if_equal(
        ctx,
        &range,
        exact_next,
        journal_revision_after,
        next_journal_revision,
    );
    assert_if_equal(
        ctx,
        &range,
        rotate,
        journal_revision_after,
        Constant(F::ZERO),
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        transition_effect_digest,
        [zero; 2],
    );
    assert_if_digest_different(ctx, &range, mint, mint_finality_semantic_digest, [zero; 2]);
    for limb in mint_finality_semantic_digest {
        assert_if_equal(ctx, &range, not_mint, limb, Constant(F::ZERO));
    }
    assert_if_digest_different(
        ctx,
        &range,
        mint,
        mint_finality_proof_binding_digest,
        [zero; 2],
    );
    for limb in mint_finality_proof_binding_digest {
        assert_if_equal(ctx, &range, not_mint, limb, Constant(F::ZERO));
    }
    let peer = gate.add(ctx, send, receive);
    let not_peer = gate.not(ctx, peer);
    for digest in [peer_credit_id, peer_recipient_lane_id] {
        assert_if_digest_different(ctx, &range, peer, digest, [zero; 2]);
        for limb in digest {
            assert_if_equal(ctx, &range, not_peer, limb, Constant(F::ZERO));
        }
    }
    let not_receive = gate.not(ctx, receive);

    // Stable lane/asset/release scope and operation-specific epoch rules.
    for (after, before) in successor
        .release_id
        .into_iter()
        .zip(predecessor.release_id)
        .chain(
            successor
                .liability_pool_id
                .into_iter()
                .zip(predecessor.liability_pool_id),
        )
        .chain(successor.network_id.into_iter().zip(predecessor.network_id))
        .chain(successor.asset_id.into_iter().zip(predecessor.asset_id))
        .chain(successor.lane_id.into_iter().zip(predecessor.lane_id))
        .chain(
            successor
                .hardware_profile_id
                .into_iter()
                .zip(predecessor.hardware_profile_id),
        )
    {
        assert_if_equal(ctx, &range, non_bootstrap, after, before);
    }
    assert_if_equal(
        ctx,
        &range,
        non_bootstrap,
        successor.scale,
        predecessor.scale,
    );
    assert_if_equal(
        ctx,
        &range,
        non_bootstrap,
        successor.protocol_version,
        predecessor.protocol_version,
    );
    for (after, before) in successor
        .asset_incarnation
        .into_iter()
        .zip(predecessor.asset_incarnation)
    {
        assert_if_equal(ctx, &range, non_bootstrap, after, before);
    }
    assert_if_equal(
        ctx,
        &range,
        non_bootstrap,
        successor.policy_epoch,
        predecessor.policy_epoch,
    );
    let same_suite = gate.add(ctx, monetary, rotate);
    for (after, before) in successor
        .suite_id
        .into_iter()
        .zip(predecessor.suite_id)
        .chain(successor.vk_digest.into_iter().zip(predecessor.vk_digest))
    {
        assert_if_equal(ctx, &range, same_suite, after, before);
    }
    assert_if_digest_different(
        ctx,
        &range,
        suite_upgrade,
        successor.suite_id,
        predecessor.suite_id,
    );
    assert_if_digest_different(
        ctx,
        &range,
        suite_upgrade,
        successor.vk_digest,
        predecessor.vk_digest,
    );
    let ordinary = gate.add(ctx, monetary, suite_upgrade);
    assert_if_equal(
        ctx,
        &range,
        ordinary,
        successor.epoch_generation,
        predecessor.epoch_generation,
    );
    for (after, before) in successor
        .epoch_id
        .into_iter()
        .zip(predecessor.epoch_id)
        .chain(
            successor
                .key_reference
                .into_iter()
                .zip(predecessor.key_reference),
        )
        .chain(successor.policy_id.into_iter().zip(predecessor.policy_id))
    {
        assert_if_equal(ctx, &range, ordinary, after, before);
    }
    let next_epoch = gate.inc(ctx, predecessor.epoch_generation);
    assert_if_equal(ctx, &range, rotate, successor.epoch_generation, next_epoch);
    for (after, before) in successor.policy_id.into_iter().zip(predecessor.policy_id) {
        assert_if_equal(ctx, &range, rotate, after, before);
    }
    assert_if_digest_different(
        ctx,
        &range,
        non_bootstrap,
        successor.nonce,
        predecessor.nonce,
    );
    assert_if_digest_different(
        ctx,
        &range,
        rotate,
        successor.epoch_id,
        predecessor.epoch_id,
    );
    assert_if_digest_different(
        ctx,
        &range,
        rotate,
        successor.key_reference,
        predecessor.key_reference,
    );

    // MintFold and ReceiveFold each evaluate one fixed replay path. Every other operation keeps
    // the replay root unchanged, so circuit shape is independent of history length.
    let replay_zero = zero;
    let mint_replay = witness.and_then(|value| value.replay_insert.as_ref());
    let (credit_limbs, envelope_limbs, mint_empty_path, mint_present_path) =
        assign_replay_path_v1(ctx, &range, &poseidon, mint_replay);
    assert_if_digest_different(ctx, &range, mint, credit_limbs, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, mint, envelope_limbs, [replay_zero; 2]);
    assert_if_equal(ctx, &range, mint, predecessor.replay_root, mint_empty_path);
    assert_if_equal(ctx, &range, mint, successor.replay_root, mint_present_path);

    let receive_credit = witness.and_then(|value| value.receive_credit.as_ref());
    let receive_amount = assign_u128(ctx, &range, receive_credit.map_or(0, |value| value.amount));
    let receive_credit_id = assign_digest(
        ctx,
        &range,
        receive_credit.map_or([0; 32], |value| value.credit_id),
    );
    let receive_recipient_lane_id = assign_digest(
        ctx,
        &range,
        receive_credit.map_or([0; 32], |value| value.recipient_lane_id),
    );
    let receive_incoming_proof_binding_digest = assign_digest(
        ctx,
        &range,
        receive_credit.map_or([0; 32], |value| value.incoming_proof_binding_digest),
    );
    let (receive_replay_credit, receive_envelope_digest, receive_empty_path, receive_present_path) =
        assign_replay_path_v1(
            ctx,
            &range,
            &poseidon,
            receive_credit.map(|value| &value.replay_insert),
        );
    assert_if_nonzero(ctx, &range, receive, receive_amount);
    assert_if_equal(ctx, &range, receive, receive_amount, amount);
    for digest in [
        receive_credit_id,
        receive_recipient_lane_id,
        receive_incoming_proof_binding_digest,
        receive_envelope_digest,
    ] {
        assert_if_digest_different(ctx, &range, receive, digest, [replay_zero; 2]);
        for limb in digest {
            assert_if_equal(ctx, &range, not_receive, limb, Constant(F::ZERO));
        }
    }
    assert_if_equal(ctx, &range, not_receive, receive_amount, Constant(F::ZERO));
    for ((credit, peer), replay) in receive_credit_id
        .into_iter()
        .zip(peer_credit_id)
        .zip(receive_replay_credit)
    {
        assert_if_equal(ctx, &range, receive, credit, peer);
        assert_if_equal(ctx, &range, receive, credit, replay);
    }
    for ((recipient, peer), lane) in receive_recipient_lane_id
        .into_iter()
        .zip(peer_recipient_lane_id)
        .zip(successor.lane_id)
    {
        assert_if_equal(ctx, &range, receive, recipient, peer);
        assert_if_equal(ctx, &range, receive, recipient, lane);
    }
    assert_if_equal(
        ctx,
        &range,
        receive,
        predecessor.replay_root,
        receive_empty_path,
    );
    assert_if_equal(
        ctx,
        &range,
        receive,
        successor.replay_root,
        receive_present_path,
    );
    let receive_credit = OfflineCashAssignedReceiveFoldCreditV1 {
        amount: receive_amount,
        credit_id: receive_credit_id,
        recipient_lane_id: receive_recipient_lane_id,
        incoming_proof_binding_digest: receive_incoming_proof_binding_digest,
        envelope_digest: receive_envelope_digest,
    };
    assert_if_equal(
        ctx,
        &range,
        no_inbound,
        successor.replay_root,
        predecessor.replay_root,
    );

    let transport = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.transport_semantic_digest),
    );
    let guard = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.guard_statement_digest),
    );
    let eq_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.eq_protocol_digest),
    );
    let ep_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.ep_protocol_digest),
    );
    let guard_eq_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.guard_eq_protocol_digest),
    );
    let guard_ep_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.guard_ep_protocol_digest),
    );
    let mint_eq_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.mint_eq_protocol_digest),
    );
    let mint_ep_protocol = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.mint_ep_protocol_digest),
    );
    let guard_eq_audit = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.guard_eq_credential_audit),
    );
    let guard_ep_audit = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.guard_ep_credential_audit),
    );
    let eq_audit = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.eq_deferred_audit),
    );
    let ep_audit = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.ep_deferred_audit),
    );
    let lifecycle_binding_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.lifecycle_binding_digest),
    );
    let precommit_binding_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.precommit_binding_digest),
    );
    let suite_upgrade_authorization_digest = assign_digest(
        ctx,
        &range,
        witness.map_or([0; 32], |value| value.suite_upgrade_authorization_digest),
    );
    assert_if_digest_different(ctx, &range, active_successor, transport, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, guard, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, eq_protocol, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, ep_protocol, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, eq_protocol, ep_protocol);
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_eq_protocol,
        [replay_zero; 2],
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_ep_protocol,
        [replay_zero; 2],
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_eq_protocol,
        guard_ep_protocol,
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_eq_protocol,
        eq_protocol,
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_ep_protocol,
        ep_protocol,
    );
    for protocol in [mint_eq_protocol, mint_ep_protocol] {
        assert_if_digest_different(ctx, &range, active_successor, protocol, [replay_zero; 2]);
    }
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        mint_eq_protocol,
        mint_ep_protocol,
    );
    assert_if_digest_different(ctx, &range, active_successor, mint_eq_protocol, eq_protocol);
    assert_if_digest_different(ctx, &range, active_successor, mint_ep_protocol, ep_protocol);
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        mint_eq_protocol,
        guard_eq_protocol,
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        mint_ep_protocol,
        guard_ep_protocol,
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_eq_audit,
        [replay_zero; 2],
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_ep_audit,
        [replay_zero; 2],
    );
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        guard_eq_audit,
        guard_ep_audit,
    );
    assert_if_digest_different(ctx, &range, active_successor, eq_audit, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, ep_audit, [replay_zero; 2]);
    assert_if_digest_different(ctx, &range, active_successor, eq_audit, ep_audit);
    assert_if_digest_different(
        ctx,
        &range,
        active_successor,
        lifecycle_binding_digest,
        [replay_zero; 2],
    );
    for (selector, digest) in [
        (outbound, precommit_binding_digest),
        (suite_upgrade, suite_upgrade_authorization_digest),
    ] {
        assert_if_digest_different(ctx, &range, selector, digest, [replay_zero; 2]);
        let inactive = gate.not(ctx, selector);
        for limb in digest {
            assert_if_equal(ctx, &range, inactive, limb, Constant(F::ZERO));
        }
    }
    let predecessor_public = gate.select(
        ctx,
        predecessor.commitment,
        Constant(F::ZERO),
        non_bootstrap,
    );
    let public = vec![
        operation_tag,
        amount,
        transport[0],
        transport[1],
        guard[0],
        guard[1],
        predecessor_outer[0],
        predecessor_outer[1],
        successor_outer[0],
        successor_outer[1],
        mint_finality_semantic_digest[0],
        mint_finality_semantic_digest[1],
        successor.release_id[0],
        successor.release_id[1],
        successor.liability_pool_id[0],
        successor.liability_pool_id[1],
        peer_credit_id[0],
        peer_credit_id[1],
        peer_recipient_lane_id[0],
        peer_recipient_lane_id[1],
        predecessor_eq_components[0],
        predecessor_eq_components[1],
        predecessor_ep_components[0],
        predecessor_ep_components[1],
        successor_eq_components[0],
        successor_eq_components[1],
        successor_ep_components[0],
        successor_ep_components[1],
        predecessor_public,
        successor.commitment,
        eq_protocol[0],
        eq_protocol[1],
        ep_protocol[0],
        ep_protocol[1],
        guard_eq_protocol[0],
        guard_eq_protocol[1],
        guard_ep_protocol[0],
        guard_ep_protocol[1],
        mint_eq_protocol[0],
        mint_eq_protocol[1],
        mint_ep_protocol[0],
        mint_ep_protocol[1],
        guard_eq_audit[0],
        guard_eq_audit[1],
        guard_ep_audit[0],
        guard_ep_audit[1],
        eq_audit[0],
        eq_audit[1],
        ep_audit[0],
        ep_audit[1],
        mint_finality_proof_binding_digest[0],
        mint_finality_proof_binding_digest[1],
        lifecycle_binding_digest[0],
        lifecycle_binding_digest[1],
        precommit_binding_digest[0],
        precommit_binding_digest[1],
        predecessor.suite_id[0],
        predecessor.suite_id[1],
        predecessor.vk_digest[0],
        predecessor.vk_digest[1],
        successor.suite_id[0],
        successor.suite_id[1],
        successor.vk_digest[0],
        successor.vk_digest[1],
        suite_upgrade_authorization_digest[0],
        suite_upgrade_authorization_digest[1],
        successor.protocol_version,
        successor.asset_incarnation[0],
        successor.asset_incarnation[1],
        successor.hardware_profile_id[0],
        successor.hardware_profile_id[1],
        successor.policy_epoch,
        successor.network_id[0],
        successor.network_id[1],
        successor.asset_id[0],
        successor.asset_id[1],
        successor.scale,
    ];
    debug_assert_eq!(public.len(), PUBLIC_INSTANCE_COUNT);
    builder.assigned_instances = vec![public];
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    Ok((
        builder,
        OfflineCashAssignedStateRelationV1 {
            operation: operation_tag,
            amount,
            predecessor,
            successor,
            predecessor_outer,
            successor_outer,
            guard_digest: guard,
            journal_revision_before,
            journal_revision_after,
            transition_effect_digest,
            mint_finality_semantic_digest,
            mint_finality_proof_binding_digest,
            peer_credit_id,
            peer_recipient_lane_id,
            receive_credit,
            lifecycle_binding_digest,
            precommit_binding_digest,
            suite_upgrade_authorization_digest,
            predecessor_eq_components,
            predecessor_ep_components,
            successor_eq_components,
            successor_ep_components,
            replay_credit_id: credit_limbs,
            replay_envelope_digest: envelope_limbs,
        },
    ))
}

fn assign_replay_path_v1<F>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    poseidon: &OfflineCashPoseidonChipV1<F>,
    replay: Option<&OfflineCashReplayInsertWitnessV1>,
) -> (
    [AssignedValue<F>; 2],
    [AssignedValue<F>; 2],
    AssignedValue<F>,
    AssignedValue<F>,
)
where
    F: OfflineCashPoseidonFieldV1,
{
    let gate = range.gate();
    let credit_limbs = assign_digest(ctx, range, replay.map_or([0; 32], |value| value.credit_id));
    let envelope_limbs = assign_digest(
        ctx,
        range,
        replay.map_or([0; 32], |value| value.envelope_digest),
    );
    let mut key_bits = Vec::with_capacity(OFFLINE_CASH_CONSUMED_CREDIT_TREE_DEPTH_V1);
    for limb in credit_limbs {
        key_bits.extend(gate.num_to_bits(ctx, limb, 128));
    }
    let mut empty_path = poseidon.hash(ctx, range, OFFLINE_CASH_REPLAY_EMPTY_DOMAIN_V1, &[]);
    let mut present_path = poseidon.hash(
        ctx,
        range,
        OFFLINE_CASH_REPLAY_LEAF_DOMAIN_V1,
        &[
            credit_limbs[0],
            credit_limbs[1],
            envelope_limbs[0],
            envelope_limbs[1],
        ],
    );
    for parent_depth in (0..OFFLINE_CASH_CONSUMED_CREDIT_TREE_DEPTH_V1).rev() {
        let sibling_bytes = replay.map_or(OfflineCashPastaStateCommitmentV1::ZERO, |value| {
            value.siblings_root_to_leaf[parent_depth]
        });
        let sibling = ctx.load_witness(canonical_component_or_zero::<F>(sibling_bytes));
        let key_index = (parent_depth / 8) * 8 + (7 - parent_depth % 8);
        let direction = key_bits[key_index];
        let empty_left = gate.select(ctx, sibling, empty_path, direction);
        let empty_right = gate.select(ctx, empty_path, sibling, direction);
        empty_path = poseidon.hash(
            ctx,
            range,
            OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1,
            &[empty_left, empty_right],
        );
        let present_left = gate.select(ctx, sibling, present_path, direction);
        let present_right = gate.select(ctx, present_path, sibling, direction);
        present_path = poseidon.hash(
            ctx,
            range,
            OFFLINE_CASH_REPLAY_NODE_DOMAIN_V1,
            &[present_left, present_right],
        );
    }
    (credit_limbs, envelope_limbs, empty_path, present_path)
}

fn assign_state<F>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    poseidon: &OfflineCashPoseidonChipV1<F>,
    state: Option<&OfflineCashStateV1>,
    active: AssignedValue<F>,
) -> Result<AssignedState<F>, String>
where
    F: OfflineCashPoseidonFieldV1,
{
    let protocol_version = ctx.load_witness(F::from(u64::from(
        state.map_or(0, |value| value.protocol_version),
    )));
    range.range_check(ctx, protocol_version, 16);
    let suite_id = assign_digest(ctx, range, state.map_or([0; 32], |value| value.suite_id));
    let vk_digest = assign_digest(ctx, range, state.map_or([0; 32], |value| value.vk_digest));
    let release_id = assign_digest(ctx, range, state.map_or([0; 32], |value| value.release_id));
    let asset_incarnation = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| *value.asset_incarnation.as_bytes()),
    );
    let liability_pool_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.liability_pool_id),
    );
    let hardware_profile_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.hardware_profile_id),
    );
    let policy_epoch = ctx.load_witness(F::from(state.map_or(0, |value| value.policy_epoch)));
    range.range_check(ctx, policy_epoch, 64);
    let network_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.lane.normalized_network_id()),
    );
    let asset_id_bytes = state
        .map(|value| {
            value
                .lane
                .normalized_asset_id()
                .map_err(|error| error.to_string())
        })
        .transpose()?
        .unwrap_or([0; 32]);
    let asset_id = assign_digest(ctx, range, asset_id_bytes);
    let scale = ctx.load_witness(F::from(u64::from(
        state.map_or(0, |value| value.lane.scale),
    )));
    range.range_check(ctx, scale, 32);
    // V1's authoritative asset scale is currently bounded well below u32::MAX.
    let scale_ok =
        range.is_less_than_safe(ctx, scale, u64::from(OFFLINE_CASH_ASSET_SCALE_MAX_V1) + 1);
    range.gate().assert_is_const(ctx, &scale_ok, &F::ONE);
    let lane_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.lane.device_lane_id),
    );
    let balance = assign_u128(ctx, range, state.map_or(0, |value| value.balance));
    let sequence = assign_u128(ctx, range, state.map_or(0, |value| value.logical_sequence));
    let epoch_generation = assign_u128(
        ctx,
        range,
        state.map_or(0, |value| value.hardware_epoch.generation),
    );
    let epoch_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.hardware_epoch.epoch_id),
    );
    let key_reference = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| {
            value.device_policy_binding.device_key_reference
        }),
    );
    let policy_id = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| {
            value.device_policy_binding.hardware_policy_id
        }),
    );
    let nonce = assign_digest(
        ctx,
        range,
        state.map_or([0; 32], |value| value.state_nonce_commitment),
    );
    let replay_bytes = state.map_or(OfflineCashPastaStateCommitmentV1::ZERO, |value| {
        value.consumed_credit_root
    });
    let replay_root = ctx.load_witness(canonical_component_or_zero::<F>(replay_bytes));
    let version = ctx.load_witness(F::from(u64::from(state.map_or(0, |value| value.version))));
    range.range_check(ctx, version, 16);
    assert_if_equal(ctx, range, active, version, Constant(F::ONE));
    assert_if_equal(ctx, range, active, protocol_version, Constant(F::ONE));
    let zero = ctx.load_zero();
    for digest in [
        suite_id,
        vk_digest,
        release_id,
        asset_incarnation,
        hardware_profile_id,
        liability_pool_id,
        network_id,
        asset_id,
        lane_id,
        epoch_id,
        key_reference,
        policy_id,
        nonce,
    ] {
        assert_if_digest_different(ctx, range, active, digest, [zero; 2]);
    }
    assert_if_nonzero(ctx, range, active, policy_epoch);
    assert_if_nonzero(ctx, range, active, epoch_generation);
    assert_if_nonzero(ctx, range, active, replay_root);
    let commitment = poseidon.hash(
        ctx,
        range,
        OFFLINE_CASH_STATE_DOMAIN_V1,
        &[
            version,
            protocol_version,
            suite_id[0],
            suite_id[1],
            vk_digest[0],
            vk_digest[1],
            release_id[0],
            release_id[1],
            asset_incarnation[0],
            asset_incarnation[1],
            liability_pool_id[0],
            liability_pool_id[1],
            hardware_profile_id[0],
            hardware_profile_id[1],
            policy_epoch,
            network_id[0],
            network_id[1],
            asset_id[0],
            asset_id[1],
            scale,
            lane_id[0],
            lane_id[1],
            balance,
            sequence,
            epoch_generation,
            epoch_id[0],
            epoch_id[1],
            key_reference[0],
            key_reference[1],
            policy_id[0],
            policy_id[1],
            nonce[0],
            nonce[1],
            replay_root,
        ],
    );
    let claimed_bytes = state.map_or(OfflineCashPastaStateCommitmentV1::ZERO, |value| {
        value.state_commitment_components
    });
    let claimed = ctx.load_witness(canonical_component_or_zero::<F>(claimed_bytes));
    assert_if_equal(ctx, range, active, commitment, claimed);
    Ok(AssignedState {
        protocol_version,
        suite_id,
        vk_digest,
        balance,
        sequence,
        epoch_generation,
        epoch_id,
        key_reference,
        policy_id,
        nonce,
        replay_root,
        commitment,
        release_id,
        asset_incarnation,
        liability_pool_id,
        hardware_profile_id,
        policy_epoch,
        network_id,
        asset_id,
        scale,
        lane_id,
    })
}

fn assign_u128<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
) -> AssignedValue<F> {
    let assigned = ctx.load_witness(from_u128(value));
    range.range_check(ctx, assigned, 128);
    assigned
}

fn assign_digest<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    let limbs = digest_limbs::<F>(digest);
    limbs.map(|limb| {
        let assigned = ctx.load_witness(limb);
        range.range_check(ctx, assigned, 128);
        assigned
    })
}

fn assert_component_canonical<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: [AssignedValue<F>; 2],
    modulus_low: u128,
    active: AssignedValue<F>,
) {
    // Both Pasta moduli have high limb 2^126.  Canonicality is therefore
    // `high < 2^126 || (high == 2^126 && low < modulus_low)`.
    let modulus_high = ctx.load_constant(F::from_u128(1_u128 << 126));
    let modulus_low = ctx.load_constant(F::from_u128(modulus_low));
    let high_less = range.is_less_than(ctx, limbs[1], modulus_high, 128);
    let high_equal = range.gate().is_equal(ctx, limbs[1], modulus_high);
    let low_less = range.is_less_than(ctx, limbs[0], modulus_low, 128);
    let equal_high_and_low_less = range.gate().and(ctx, high_equal, low_less);
    let canonical = range.gate().or(ctx, high_less, equal_high_and_low_less);
    assert_if_equal(ctx, range, active, canonical, Constant(F::ONE));
}

fn compose_component<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: [AssignedValue<F>; 2],
) -> AssignedValue<F> {
    let two_pow_127 = F::from_u128(1_u128 << 127);
    let two_pow_128 = two_pow_127 + two_pow_127;
    range.gate().mul_add(
        ctx,
        halo2_base::QuantumCell::Existing(limbs[1]),
        Constant(two_pow_128),
        halo2_base::QuantumCell::Existing(limbs[0]),
    )
}

fn assert_if_equal<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: AssignedValue<F>,
    right: impl Into<halo2_base::QuantumCell<F>>,
) {
    let delta = range.gate().sub(ctx, left, right);
    let selected = range.gate().mul(ctx, selector, delta);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_if_nonzero<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let is_zero = range.gate().is_zero(ctx, value);
    let selected = range.gate().mul(ctx, selector, is_zero);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_if_digest_different<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: [AssignedValue<F>; 2],
    right: [AssignedValue<F>; 2],
) {
    let lo_equal = range.gate().is_equal(ctx, left[0], right[0]);
    let hi_equal = range.gate().is_equal(ctx, left[1], right[1]);
    let equal = range.gate().and(ctx, lo_equal, hi_equal);
    let selected = range.gate().mul(ctx, selector, equal);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn canonical_component<F: OfflineCashPoseidonFieldV1>(
    pair: OfflineCashPastaStateCommitmentV1,
) -> Result<F, String> {
    decode(F::select_component(pair)).ok_or_else(|| "noncanonical Pasta component".to_owned())
}

fn canonical_component_or_zero<F: OfflineCashPoseidonFieldV1>(
    pair: OfflineCashPastaStateCommitmentV1,
) -> F {
    canonical_component(pair).unwrap_or(F::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::offline_cash_v1_poseidon::hash;
    use ff::Field as _;
    use halo2_proofs::halo2curves::pasta::{Fp, Fq};

    #[test]
    fn public_instance_abi_is_identical_across_parities() {
        assert_eq!(PUBLIC_INSTANCE_COUNT, 77);
        assert_eq!(public_instance::OPERATION, 0);
        assert_eq!(public_instance::AMOUNT, 1);
        assert_eq!(public_instance::SUCCESSOR_STATE, 29);
        assert_eq!(public_instance::GUARD_EQ_PROTOCOL_LO, 34);
        assert_eq!(public_instance::MINT_EQ_PROTOCOL_LO, 38);
        assert_eq!(public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO, 42);
        assert_eq!(public_instance::EP_DEFERRED_AUDIT_HI, 49);
        assert_eq!(public_instance::LIFECYCLE_LO, 52);
        assert_eq!(public_instance::ASSET_INCARNATION_LO, 67);
        assert_eq!(public_instance::ASSET_INCARNATION_HI, 68);
        assert_eq!(public_instance::ASSET_SCALE, 76);
        // Both fields use the same semantic ordering and injective u128 digest limbs.
        assert_eq!(
            digest_limbs::<Fp>([7; 32]).len(),
            digest_limbs::<Fq>([7; 32]).len()
        );
    }

    #[test]
    fn native_and_circuit_poseidon_domains_match() {
        assert_ne!(
            hash::<Fp>(OFFLINE_CASH_REPLAY_EMPTY_DOMAIN_V1, &[]),
            hash::<Fp>(OFFLINE_CASH_REPLAY_LEAF_DOMAIN_V1, &[Fp::ZERO; 4]),
        );
    }
}
