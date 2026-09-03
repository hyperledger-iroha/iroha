//! Hardware-certified mint reservation, durable staging, and mixed-credit fold scheduling.

use super::*;
use iroha_data_model::kagemusha::KagemushaMintAuthorizationV1;

const MINT_CAPACITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-inbox-capacity";

/// One operation in a deterministic, history-unbounded pending-credit drain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingCreditFoldV1 {
    /// Fold one finalized mint through the monetary `MintFold` relation.
    Mint(CreditIdV1),
    /// Fold one peer credit through the monetary `ReceiveFold` relation.
    Receive(CreditIdV1),
}

/// Stable native inbox boundary for an explicit drain pass.
///
/// Epoch identity prevents a counter reset from reinterpreting an old watermark. This is local
/// scheduling state, not a wire limit or permission to reject later committed money.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaPendingCreditWatermarkV1 {
    hardware_epoch: HardwareEpochV1,
    inbox_revision: u128,
}

/// Durable result of mint delivery; every retry retains the original certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MintCreditStageOutcomeV1 {
    /// Newly installed mint inbox record.
    Staged(MintStageCertificateV1),
    /// Identical mint still awaiting a monetary fold.
    DuplicatePending(MintStageCertificateV1),
    /// Identical mint already incorporated in the aggregate balance.
    DuplicateConsumed(MintStageCertificateV1),
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Borrow the immutable mint recovery journal; this does not expose mutable authority.
    pub fn mint_inbox(&self) -> &KagemushaMintInboxV1 {
        &self.mint_inbox
    }

    /// Capture the current inbox boundary without changing any monetary or journal state.
    pub fn pending_credit_watermark(&self) -> KagemushaPendingCreditWatermarkV1 {
        KagemushaPendingCreditWatermarkV1 {
            hardware_epoch: self.state.hardware_epoch,
            inbox_revision: self.inbox_revision,
        }
    }

    /// Select the next mint/peer operation within a previously captured drain pass.
    ///
    /// Later deliveries remain pending for the next pass. Old-epoch staged money stays included;
    /// rotating during a pass instead requires a new watermark. The selector borrows the ordered
    /// indexes and never copies the complete pending inventory.
    pub fn next_pending_fold_through(
        &self,
        watermark: KagemushaPendingCreditWatermarkV1,
    ) -> Result<Option<PendingCreditFoldV1>, KagemushaStateErrorV1> {
        if watermark.hardware_epoch != self.state.hardware_epoch {
            return Err(KagemushaStateErrorV1::InvalidHardwareRotation);
        }
        if watermark.inbox_revision > self.inbox_revision {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let eligible = |epoch: HardwareEpochV1, revision: u128| {
            epoch.generation < watermark.hardware_epoch.generation
                || (epoch == watermark.hardware_epoch && revision <= watermark.inbox_revision)
        };
        let mut peers = self
            .pending_credits
            .iter()
            .filter_map(|(id, record)| {
                let statement = &record.stage_certificate.statement;
                eligible(
                    statement.receiver_hardware_epoch,
                    statement.journal_revision_after,
                )
                .then_some(*id)
            })
            .peekable();
        let first_mint = self.mint_inbox.pending().iter().find_map(|(id, record)| {
            let statement = &record.stage_certificate().statement;
            eligible(statement.hardware_epoch, statement.inbox_revision_after).then_some(*id)
        });
        if let Some(id) = first_mint {
            if peers.peek().is_none_or(|peer| id < *peer) {
                return Ok(Some(PendingCreditFoldV1::Mint(id)));
            }
            if peers.peek() == Some(&id) {
                return Err(KagemushaStateErrorV1::CreditConflict(id));
            }
        }
        Ok(peers
            .next()
            .filter(|id| first_mint.is_none_or(|mint| *id < mint))
            .map(PendingCreditFoldV1::Receive))
    }

    /// Select only the monetary folds required to cover the requested outgoing amount.
    ///
    /// Mint and peer credits each produce one fold. The plan has no cumulative operation limit;
    /// each lane executes it serially and can reselect after a concurrent inbox arrival.
    pub fn pending_fold_plan_required_for_amount(
        &self,
        amount: u128,
    ) -> Result<Vec<PendingCreditFoldV1>, KagemushaStateErrorV1> {
        let selected = self.pending_credits_required_for_amount(amount)?;
        Ok(selected
            .into_iter()
            .map(|id| {
                if self.mint_inbox.pending_credit(id).is_some() {
                    PendingCreditFoldV1::Mint(id)
                } else {
                    PendingCreditFoldV1::Receive(id)
                }
            })
            .collect())
    }

    /// Preview the pre-debit allocation and sealed local recipient binding.
    ///
    /// The resulting statement is not authorization to expose the mint authorization. The
    /// qualified service must atomically seal it, including the original credential, opening
    /// and key handle, and return a certificate accepted by [`Self::reserve_mint_credit`].
    pub fn preview_mint_reservation(
        &self,
        reservation: &MintInboxReservationV1,
    ) -> Result<MintReservationStatementV1, KagemushaStateErrorV1> {
        self.validate_new_mint_reservation(reservation)?;
        let successor = self.mint_inbox.reserve_successor(reservation)?;
        let capacity = self
            .receiver_inbox_capacity
            .with_mint_inbox_bytes(successor.capacity_charge_bytes()?)?;
        Ok(MintReservationStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            lane: self.state.lane.clone(),
            hardware_epoch: self.state.hardware_epoch,
            state_commitment: self.state.state_commitment,
            inbox_revision_before: self.inbox_revision,
            inbox_revision_after: self
                .inbox_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
            reservation_digest: reservation.digest()?,
            predecessor_journal_commitment: self.mint_inbox.commitment()?,
            successor_journal_commitment: successor.commitment()?,
            successor_capacity_commitment: canonical_sha256_digest(
                MINT_CAPACITY_DOMAIN,
                &capacity,
            )?,
        })
    }

    /// Install one hardware-certified allocation before its authorization can debit online funds.
    ///
    /// Exact retries are idempotent. No monetary sequence or balance is modified. A failed
    /// certificate or capacity check leaves the original allocation and counters untouched.
    pub fn reserve_mint_credit(
        &mut self,
        reservation: &MintInboxReservationV1,
        certificate: &MintReservationCertificateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        reservation.validate_inputs()?;
        if let Some(existing) = self.mint_inbox.reservation(reservation.credit_id()) {
            return if existing == reservation {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(
                    reservation.credit_id(),
                ))
            };
        }
        if let Some(existing) = self.mint_inbox.pending_credit(reservation.credit_id()) {
            return if existing.reservation() == reservation {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(
                    reservation.credit_id(),
                ))
            };
        }
        if let Some(existing) = self.mint_inbox.accepted_receipt(reservation.credit_id()) {
            return if existing.reservation_digest() == reservation.digest()? {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(
                    reservation.credit_id(),
                ))
            };
        }
        let expected = self.preview_mint_reservation(reservation)?;
        if certificate.statement != expected {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        validate_guard_bytes(&certificate.guard_bundle)?;
        self.guard_verifier
            .verify_mint_reservation(&expected, &certificate.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        let successor = self.mint_inbox.reserve_successor(reservation)?;
        let capacity = self
            .receiver_inbox_capacity
            .with_mint_inbox_bytes(successor.capacity_charge_bytes()?)?;
        self.mint_inbox = successor;
        self.receiver_inbox_capacity = capacity;
        self.inbox_revision = expected.inbox_revision_after;
        Ok(())
    }

    /// Preview staging an authenticated finalized mint into its existing durable allocation.
    ///
    /// The proof capability alone does not establish local ownership: it must match the exact
    /// pre-debit record already sealed into this hardware lane, including retained old-epoch keys.
    pub fn preview_stage_mint_credit(
        &self,
        verified: &VerifiedMintStageV1,
        staged_at_ms: u64,
    ) -> Result<MintStageStatementV1, KagemushaStateErrorV1> {
        let reservation = verified.reservation();
        let id = reservation.credit_id();
        if self.mint_inbox.reservation(id) != Some(reservation) || staged_at_ms == 0 {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, verified.credit())?;
        self.ensure_non_mint_credit_id_available(id, envelope_digest)?;
        if self
            .authenticated_history
            .classify_replay(id, envelope_digest)
            .map_err(map_authenticated_history_error)?
            != KagemushaHistoryIdentityClassificationV1::Absent
        {
            return Err(KagemushaStateErrorV1::CreditConflict(id));
        }
        let revision = self
            .inbox_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        let successor = self.mint_inbox.preview_staged_successor(
            verified,
            revision,
            self.state.hardware_epoch,
            staged_at_ms,
        )?;
        // No fresh capacity is required after the online debit. Arrival only materializes a
        // previously promised allocation, irrespective of later peer traffic or delivery age.
        if successor.capacity_charge_bytes()? > self.mint_inbox.capacity_charge_bytes()? {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        let capacity = self
            .receiver_inbox_capacity
            .with_mint_inbox_bytes(successor.capacity_charge_bytes()?)?;
        Ok(MintStageStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            lane: self.state.lane.clone(),
            hardware_epoch: self.state.hardware_epoch,
            state_commitment: self.state.state_commitment,
            inbox_revision_before: self.inbox_revision,
            inbox_revision_after: revision,
            reservation_digest: reservation.digest()?,
            predecessor_journal_commitment: self.mint_inbox.commitment()?,
            successor_journal_commitment: successor.commitment()?,
            successor_capacity_commitment: canonical_sha256_digest(
                MINT_CAPACITY_DOMAIN,
                &capacity,
            )?,
            credit_id: id,
            envelope_digest,
            staged_at_ms,
        })
    }

    /// Stage exact finalized bytes, or recover their original durable receipt without refolding.
    ///
    /// A first delivery requires both the concrete native proof-verification capability and a
    /// qualified staging certificate. A duplicate needs neither: its exact canonical identities
    /// already belong to the sealed inbox. Different bytes under one ID always conflict.
    pub fn stage_mint_credit(
        &mut self,
        authorization: &KagemushaMintAuthorizationV1,
        credit: &KagemushaMintCreditV1,
        verified: Option<&VerifiedMintStageV1>,
        certificate: Option<&MintStageCertificateV1>,
    ) -> Result<MintCreditStageOutcomeV1, KagemushaStateErrorV1> {
        credit
            .validate_shape_against_authorization(authorization)
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let id = CreditIdV1(credit.statement.lifecycle.credit_id);
        let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, credit)?;
        if let Some(existing) = self.mint_inbox.pending_credit(id) {
            if existing.reservation().authorization() != authorization
                || existing.credit() != credit
            {
                return Err(KagemushaStateErrorV1::CreditConflict(id));
            }
            return Ok(MintCreditStageOutcomeV1::DuplicatePending(
                existing.stage_certificate().clone(),
            ));
        }
        if let Some(existing) = self.mint_inbox.accepted_receipt(id) {
            if existing.authorization_digest()
                != authorization
                    .canonical_digest()
                    .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?
                || existing.envelope_digest() != envelope_digest
            {
                return Err(KagemushaStateErrorV1::CreditConflict(id));
            }
            if self.consumed_credits.get(id) != Some(envelope_digest)
                || self
                    .authenticated_history
                    .classify_replay(id, envelope_digest)
                    .map_err(map_authenticated_history_error)?
                    != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return Ok(MintCreditStageOutcomeV1::DuplicateConsumed(
                existing.stage_certificate().clone(),
            ));
        }
        let verified = verified.ok_or(KagemushaStateErrorV1::MissingStageAuthorization)?;
        let certificate = certificate.ok_or(KagemushaStateErrorV1::MissingStageAuthorization)?;
        if verified.reservation().authorization() != authorization || verified.credit() != credit {
            return Err(KagemushaStateErrorV1::MintFinalityMismatch);
        }
        let expected =
            self.preview_stage_mint_credit(verified, certificate.statement.staged_at_ms)?;
        if certificate.statement != expected {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        validate_guard_bytes(&certificate.guard_bundle)?;
        self.guard_verifier
            .verify_mint_stage(&expected, &certificate.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)?;
        let successor = self.mint_inbox.staged_successor(verified, certificate)?;
        let capacity = self
            .receiver_inbox_capacity
            .with_mint_inbox_bytes(successor.capacity_charge_bytes()?)?;
        self.mint_inbox = successor;
        self.receiver_inbox_capacity = capacity;
        self.inbox_revision = expected.inbox_revision_after;
        Ok(MintCreditStageOutcomeV1::Staged(certificate.clone()))
    }

    fn validate_new_mint_reservation(
        &self,
        reservation: &MintInboxReservationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        reservation.validate_inputs()?;
        let context = &reservation.authorization().statement.context;
        let credential = reservation.recipient_credential();
        let enabled = self
            .proof_release
            .enabled_profile(credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        credential
            .validate_against_profile(&enabled.hardware_profile)
            .map_err(|_| KagemushaStateErrorV1::InvalidHardwareProfile)?;
        if credential.lane_commitment != self.state.lane.device_lane_id
            || credential.hardware_epoch_id != self.state.hardware_epoch.epoch_id
            || u128::from(credential.hardware_epoch_generation)
                != self.state.hardware_epoch.generation
            || credential.device_key_reference
                != self.state.device_policy_binding.device_key_reference
            || credential.network_id != self.state.lane.network_id
            || context.network_id != self.state.lane.network_id
            || context.asset != self.state.lane.asset
            || context.scale != self.state.lane.scale
            || context.asset_incarnation != self.state.asset_incarnation
            || context.liability_pool_id != self.state.liability_pool_id
            || context.release_id != self.state.release_id
            || context.suite_id != self.state.suite_id
            || context.vk_digest != self.state.vk_digest
            || context.hardware_profile_id != self.state.hardware_profile_id
            || context.policy_epoch != self.state.policy_epoch
            || context.artifact_manifest_digest
                != self.proof_release.artifacts.artifact_manifest_digest
        {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        let id = reservation.credit_id();
        if self.pending_credits.contains_key(&id)
            || self.accepted_payment_receipts.contains_key(&id)
            || self.consumed_credits.get(id).is_some()
            || self
                .authenticated_history
                .classify_replay(id, reservation.digest()?)
                .map_err(map_authenticated_history_error)?
                != KagemushaHistoryIdentityClassificationV1::Absent
        {
            return Err(KagemushaStateErrorV1::CreditConflict(id));
        }
        Ok(())
    }

    /// Select the exact authenticated pending record used by `MintFold`.
    ///
    /// A detached decoded credit never supplies private witness authority. Recovery may permit
    /// this one pending identity to be present in the external replay history only after the
    /// replay-root CAS committed and before the local aggregate successor was installed.
    pub(super) fn mint_fold_private_inputs_for_credit(
        &self,
        credit: &KagemushaMintCreditV1,
        allow_recovery_replay: bool,
    ) -> Result<KagemushaMintFoldPrivateInputsV1, KagemushaStateErrorV1> {
        credit
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let credit_id = CreditIdV1(credit.statement.lifecycle.credit_id);
        let envelope_digest = mint_envelope_digest_v1(credit)?;
        if let Some(accepted) = self.mint_inbox.accepted_receipt(credit_id) {
            return if accepted.envelope_digest() == envelope_digest {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            };
        }
        let staged = self
            .mint_inbox
            .pending_credit(credit_id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(credit_id))?;
        if staged.envelope_digest() != envelope_digest || staged.credit() != credit {
            return Err(KagemushaStateErrorV1::CreditConflict(credit_id));
        }
        let recovery_replay = allow_recovery_replay.then_some((credit_id, envelope_digest));
        self.validate_mint_inbox_snapshot_allowing_replay(recovery_replay)?;
        KagemushaMintFoldPrivateInputsV1::from_checked_pending(staged)
    }

    pub(super) fn validate_mint_inbox_snapshot(&self) -> Result<(), KagemushaStateErrorV1> {
        self.validate_mint_inbox_snapshot_allowing_replay(None)
    }

    fn validate_mint_inbox_snapshot_allowing_replay(
        &self,
        recovery_replay: Option<(CreditIdV1, DigestV1)>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.mint_inbox.validate_recovered(&self.state)?;
        self.receiver_inbox_capacity
            .validate_mint_inbox_bytes(self.mint_inbox.capacity_charge_bytes()?)?;
        let mut keys = BTreeSet::new();
        let mut revisions = self
            .accepted_payment_receipts
            .values()
            .map(|r| {
                (
                    r.stage_certificate
                        .statement
                        .receiver_hardware_epoch
                        .epoch_id,
                    r.stage_certificate.statement.journal_revision_after,
                )
            })
            .collect::<BTreeSet<_>>();
        for (id, reservation) in self.mint_inbox.reservations() {
            self.validate_mint_cross_kind_identity(*id)?;
            self.validate_retained_mint_credential(reservation)?;
            if !keys.insert(
                reservation
                    .authorization()
                    .statement
                    .context
                    .recipient_one_time_key,
            ) || self.consumed_credits.get(*id).is_some()
                || self
                    .authenticated_history
                    .classify_replay(*id, reservation.digest()?)
                    .map_err(map_authenticated_history_error)?
                    != KagemushaHistoryIdentityClassificationV1::Absent
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        for (id, staged) in self.mint_inbox.pending() {
            self.validate_mint_cross_kind_identity(*id)?;
            self.validate_retained_mint_credential(staged.reservation())?;
            let replay_classification = self
                .authenticated_history
                .classify_replay(*id, staged.envelope_digest())
                .map_err(map_authenticated_history_error)?;
            let replay_is_valid = replay_classification
                == KagemushaHistoryIdentityClassificationV1::Absent
                || (recovery_replay == Some((*id, staged.envelope_digest()))
                    && replay_classification
                        == KagemushaHistoryIdentityClassificationV1::ExactDuplicate);
            if !keys.insert(
                staged
                    .reservation()
                    .authorization()
                    .statement
                    .context
                    .recipient_one_time_key,
            ) || self.consumed_credits.get(*id).is_some()
                || !replay_is_valid
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            self.validate_mint_receipt_revision(staged.stage_certificate(), &mut revisions)?;
        }
        for (id, accepted) in self.mint_inbox.accepted() {
            self.validate_mint_cross_kind_identity(*id)?;
            if !keys.insert(accepted.recipient_one_time_key())
                || self.consumed_credits.get(*id) != Some(accepted.envelope_digest())
                || self
                    .authenticated_history
                    .classify_replay(*id, accepted.envelope_digest())
                    .map_err(map_authenticated_history_error)?
                    != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            self.validate_mint_receipt_revision(accepted.stage_certificate(), &mut revisions)?;
        }
        Ok(())
    }

    fn validate_retained_mint_credential(
        &self,
        reservation: &MintInboxReservationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let credential = reservation.recipient_credential();
        // The state hardware-policy ID and governed credential-profile ID are distinct.
        // Admission certified the complete original state/policy commitment. Recovery retains
        // that signing key and independently authenticates the original profile below.
        if !self
            .accepted_recipient_bindings
            .iter()
            .any(|binding| binding.device_key_reference == credential.device_key_reference)
            || reservation
                .authorization()
                .statement
                .context
                .artifact_manifest_digest
                != self.proof_release.artifacts.artifact_manifest_digest
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let profile = self
            .proof_release
            .enabled_profile(credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::InvalidHardwareProfile)?;
        credential
            .validate_against_profile(&profile.hardware_profile)
            .map_err(|_| KagemushaStateErrorV1::InvalidHardwareProfile)
    }

    pub(super) fn reauthenticate_pending_mint_finality(&self) -> Result<(), KagemushaStateErrorV1> {
        for staged in self.mint_inbox.pending().values() {
            let _reauthenticated_finality =
                super::super::kagemusha_v1_recursion::verify_kagemusha_mint_finality_helper_v1(
                    &self.recursive_verifier,
                    self.proof_release.artifacts,
                    staged.credit(),
                )
                .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        }
        Ok(())
    }

    pub(super) fn mint_recipient_key_is_retained(&self, key: DigestV1) -> bool {
        self.mint_inbox
            .reservations()
            .values()
            .any(|r| r.recipient_one_time_key() == key)
            || self
                .mint_inbox
                .pending()
                .values()
                .any(|r| r.reservation().recipient_one_time_key() == key)
            || self
                .mint_inbox
                .accepted()
                .values()
                .any(|r| r.recipient_one_time_key() == key)
    }

    fn validate_mint_cross_kind_identity(
        &self,
        id: CreditIdV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.pending_credits.contains_key(&id)
            || self.accepted_payment_receipts.contains_key(&id)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn validate_mint_receipt_revision(
        &self,
        certificate: &MintStageCertificateV1,
        seen: &mut BTreeSet<(DigestV1, u128)>,
    ) -> Result<(), KagemushaStateErrorV1> {
        let statement = &certificate.statement;
        if statement.lane != self.state.lane
            || statement.hardware_epoch.generation > self.state.hardware_epoch.generation
            || (statement.hardware_epoch.generation == self.state.hardware_epoch.generation
                && statement.hardware_epoch != self.state.hardware_epoch)
            || (statement.hardware_epoch == self.state.hardware_epoch
                && statement.inbox_revision_after > self.inbox_revision)
            || statement.inbox_revision_before.checked_add(1)
                != Some(statement.inbox_revision_after)
            || !seen.insert((
                statement.hardware_epoch.epoch_id,
                statement.inbox_revision_after,
            ))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        validate_guard_bytes(&certificate.guard_bundle)?;
        self.guard_verifier
            .verify_mint_stage(statement, &certificate.guard_bundle)
            .map_err(KagemushaStateErrorV1::GuardRejected)
    }
}
