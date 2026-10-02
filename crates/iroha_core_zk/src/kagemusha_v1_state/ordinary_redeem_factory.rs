//! Native redemption originals fixed before W2. Disk data alone never grants authority.
use super::*;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KagemushaOrdinaryRedemptionOutputV1, kagemusha_asset_identity_digest_v1,
        kagemusha_ordinary_app_account_binding_v1, kagemusha_ordinary_transition_nullifier_v1,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryRedeemOriginalsV1")]
pub(super) struct RedeemOriginals {
    operation: DigestV1,
    beneficiary: AccountId,
    manifest_original: Vec<u8>,
    output: KagemushaOrdinaryRedemptionOutputV1,
}
impl RedeemOriginals {
    pub(super) fn create(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        amount: u128,
        successor: &KagemushaStateV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        let pending = owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation
            || pending.selected.is_some()
            || pending.fenced
            || pending.reservation.operation_kind != KagemushaOperationKindV1::RedeemSplit
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let before = &owner.state;
        let beneficiary = owner
            .publication
            .cash_financial()
            .enrollment()
            .certificate()
            .subject
            .owner
            .account_id
            .clone();
        let release = admitted_release(&owner.verifier)?;
        let lifecycle = terminal_lifecycle_binding_v1(
            before,
            KagemushaOperationKindV1::RedeemSplit,
            [0; 32],
            [0; 32],
            [0; 32],
        );
        let clock = pending.preparation_clock;
        let output = KagemushaOrdinaryRedemptionOutputV1 {
            version: 1,
            release_id: before.release_id,
            network_id: *before.lane.network_id.as_bytes(),
            normalized_asset_id: kagemusha_asset_identity_digest_v1(&before.lane.asset)
                .map_err(material)?,
            asset_incarnation: *before.asset_incarnation.as_bytes(),
            scale: before.lane.scale,
            reserve_pool_id: before.liability_pool_id,
            amount,
            beneficiary_account_binding: kagemusha_ordinary_app_account_binding_v1(&beneficiary),
            sender_before_commitment: before.state_commitment,
            sender_after_commitment: successor.state_commitment,
            transition_nullifier: kagemusha_ordinary_transition_nullifier_v1(
                before.state_commitment,
                before.secure_index,
                before.hardware_epoch.epoch_id,
                *before.lane.network_id.as_bytes(),
                before.lane.device_lane_id,
                before.liability_pool_id,
            )
            .map_err(material)?,
            lifecycle_digest: lifecycle.canonical_digest().map_err(material)?,
            artifact_manifest_digest: release.manifest_digest(),
            clock_context_digest: clock.binding_digest().map_err(material)?,
            prepared_at_ms: clock.upper_at_ms,
        };
        let this = Self {
            operation,
            beneficiary,
            manifest_original: release.canonical_manifest_original().map_err(material)?,
            output,
        };
        this.recheck_original_data(owner, operation, successor)?;
        owner.require_current_financial_control()?;
        Ok(this)
    }

    pub(super) fn recheck_original_data(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        successor: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.publication.recheck_historical_cash_custody()?;
        owner.journal.check_owned().map_err(storage)?;
        if owner.journal.recovery_prefix().map_err(storage)? != owner.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let before = &owner.state;
        let release = admitted_release(&owner.verifier)?;
        self.output
            .validate_against_originals(&self.beneficiary, &pending.preparation_clock, &release)
            .map_err(material)?;
        let expected = KagemushaStateV1::build(
            before.context(),
            before.liability_pool_id,
            before.lane.clone(),
            before
                .balance
                .checked_sub(self.output.amount)
                .ok_or(KagemushaStateErrorV1::InsufficientBalance)?,
            before
                .logical_sequence
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
            before
                .secure_index
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
            before.hardware_epoch,
            before.device_policy_binding,
            successor.state_nonce_commitment,
            before.consumed_credit_root,
        )?;
        let lifecycle = terminal_lifecycle_binding_v1(
            before,
            KagemushaOperationKindV1::RedeemSplit,
            [0; 32],
            [0; 32],
            [0; 32],
        );
        let nullifier = kagemusha_ordinary_transition_nullifier_v1(
            before.state_commitment,
            before.secure_index,
            before.hardware_epoch.epoch_id,
            *before.lane.network_id.as_bytes(),
            before.lane.device_lane_id,
            before.liability_pool_id,
        )
        .map_err(material)?;
        if self.operation != operation
            || pending.operation != operation
            || pending.reservation.operation_kind != KagemushaOperationKindV1::RedeemSplit
            || self.beneficiary
                != owner
                    .publication
                    .cash_financial()
                    .enrollment()
                    .certificate()
                    .subject
                    .owner
                    .account_id
            || self.manifest_original != release.canonical_manifest_original().map_err(material)?
            || expected != *successor
            || successor.state_nonce_commitment == before.state_nonce_commitment
            || before.next_one_use_key_reference != [0; 32]
            || self.output.sender_before_commitment != before.state_commitment
            || self.output.sender_after_commitment != successor.state_commitment
            || self.output.transition_nullifier != nullifier
            || self.output.normalized_asset_id
                != kagemusha_asset_identity_digest_v1(&before.lane.asset).map_err(material)?
            || self.output.asset_incarnation != *before.asset_incarnation.as_bytes()
            || self.output.scale != before.lane.scale
            || self.output.reserve_pool_id != before.liability_pool_id
            || self.output.lifecycle_digest != lifecycle.canonical_digest().map_err(material)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
    pub(super) fn require_statement(
        &self,
        statement: &TransitionProofStatementV1,
        successor: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if statement.kind != KagemushaTransitionKindV1::RedeemSplit
            || statement.amount != self.output.amount
            || statement.predecessor_commitment != self.output.sender_before_commitment
            || statement.successor_commitment != self.output.sender_after_commitment
            || successor.state_commitment != self.output.sender_after_commitment
            || statement.lifecycle_binding_digest != self.output.lifecycle_digest
            || statement.peer_credit_id != [0; 32]
            || statement.recipient_encryption_key_binding != [0; 32]
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
    pub(super) fn operation(&self) -> DigestV1 {
        self.operation
    }
    pub(super) fn output(&self) -> &KagemushaOrdinaryRedemptionOutputV1 {
        &self.output
    }
    pub(super) fn beneficiary(&self) -> &AccountId {
        &self.beneficiary
    }
    pub(super) fn manifest_original(&self) -> &[u8] {
        &self.manifest_original
    }
}
