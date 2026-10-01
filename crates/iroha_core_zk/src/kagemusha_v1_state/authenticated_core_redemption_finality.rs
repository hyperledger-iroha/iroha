//! Full consensus-finality admission for an actual native redemption outbox entry.
//!
//! The authority policy must match the opaque release already retained by Core. Native
//! provisioning supplies independent replay/time pins; a response never selects its own
//! checkpoint, wallet, policy or envelope. The compact result remains selector material.

use super::*;
use iroha_data_model::{
    isi::kagemusha_v1::{
        KagemushaFinalityTrustAnchorV1, KagemushaOperationResultV1, KagemushaOperationStateV1,
        KagemushaOperationStatusV1,
    },
    kagemusha::{
        KagemushaHardwareTerminalBodyV1, KagemushaMobileBootstrapPackageV1,
        KagemushaMobileBootstrapPinsV1, KagemushaMobileBootstrapReplayPinV1,
        KagemushaMobileBootstrapScopeV1, KagemushaReleaseAuthorityPolicyV1,
        kagemusha_asset_identity_digest_v1,
    },
    sumeragi_finality::{MAX_FINALITY_BLOCK_BYTES, SumeragiFinalityVerifier},
};

const STATUS_MAX_BYTES: usize = MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;
const STATUS_DOMAIN: &[u8] = b"iroha:kagemusha:v1:authenticated-redemption-status\0";

pub(super) const ORIGINAL_MAX_BYTES: usize = STATUS_MAX_BYTES
    + iroha_data_model::kagemusha::KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1
    + 1024 * 1024;

// Original native policy/freshness selections are retained with both full signed archives.
// Decoding these fields grants no authority: recovery matches the policy to the actual opaque
// release, verifies every signature/membership and requires the exact signed Core/device op12.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::RedemptionFinalityOriginalV1")]
pub(super) struct Original {
    canonical_status: Vec<u8>,
    canonical_bootstrap: Vec<u8>,
    authority_policy: KagemushaReleaseAuthorityPolicyV1,
    minimum_sequence: u64,
    previous: Option<(u64, DigestV1)>,
    trusted_now_ms: u64,
}

impl Original {
    pub(super) fn authenticate(
        &self,
        machine: &Machine,
        operation_id: DigestV1,
    ) -> Result<KagemushaRedemptionTerminalReceiptV1, KagemushaStateErrorV1> {
        self.authenticate_with_envelope(machine, operation_id, None)
    }

    pub(super) fn authenticate_released(
        &self,
        machine: &Machine,
        operation_id: DigestV1,
        envelope: &[u8],
    ) -> Result<KagemushaRedemptionTerminalReceiptV1, KagemushaStateErrorV1> {
        self.authenticate_with_envelope(machine, operation_id, Some(envelope))
    }

    fn authenticate_with_envelope(
        &self,
        machine: &Machine,
        operation_id: DigestV1,
        retained_envelope: Option<&[u8]>,
    ) -> Result<KagemushaRedemptionTerminalReceiptV1, KagemushaStateErrorV1> {
        let release = machine
            .guard_verifier
            .authenticated_release()
            .map_err(material_error)?;
        let state = &machine.state;
        let pins = KagemushaMobileBootstrapPinsV1 {
            authority_policy: &self.authority_policy,
            network_id: state.lane.network_id,
            scope: KagemushaMobileBootstrapScopeV1 {
                asset_identity_digest: kagemusha_asset_identity_digest_v1(&state.lane.asset)
                    .map_err(material_error)?,
                asset_incarnation: *state.asset_incarnation.as_bytes(),
                asset_scale: state.lane.scale,
                liability_pool_id: state.liability_pool_id,
            },
            release_id: release.release_id(),
            release_attestation_digest: release.attestation_digest(),
            minimum_sequence: self.minimum_sequence,
            previous: self.previous.map(|(sequence, checkpoint_digest)| {
                KagemushaMobileBootstrapReplayPinV1 {
                    sequence,
                    checkpoint_digest,
                }
            }),
            trusted_now_ms: self.trusted_now_ms,
        };
        authenticate_originals(
            machine,
            operation_id,
            &self.canonical_status,
            &self.canonical_bootstrap,
            &pins,
            retained_envelope,
        )
    }
}

pub(super) fn capture_original(
    machine: &Machine,
    operation_id: DigestV1,
    canonical_status: &[u8],
    canonical_bootstrap: &[u8],
    pins: KagemushaMobileBootstrapPinsV1<'_>,
) -> Result<Original, KagemushaStateErrorV1> {
    authenticate_originals(
        machine,
        operation_id,
        canonical_status,
        canonical_bootstrap,
        &pins,
        None,
    )?;
    Ok(Original {
        canonical_status: canonical_status.to_vec(),
        canonical_bootstrap: canonical_bootstrap.to_vec(),
        authority_policy: pins.authority_policy.clone(),
        minimum_sequence: pins.minimum_sequence,
        previous: pins.previous.map(|v| (v.sequence, v.checkpoint_digest)),
        trusted_now_ms: pins.trusted_now_ms,
    })
}

/// Borrowed admission of one complete finalized result for the actual installed native voucher.
/// It cannot construct a Core, mutate capacity, or promote a decoded compact receipt.
pub struct KagemushaAuthenticatedRedemptionFinalitySelectionV1<'a> {
    owner: &'a KagemushaAuthenticatedCoreOwnerV1,
    operation_id: DigestV1,
    canonical_status: Vec<u8>,
    canonical_bootstrap: Vec<u8>,
    pins: KagemushaMobileBootstrapPinsV1<'a>,
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Admit a full original result against threshold-signed finality and this native outbox.
    ///
    /// `pins` must come from the retained native policy/freshness owner. Their policy digest,
    /// network, release and scope are additionally matched to this opaque authenticated Core;
    /// decoded status and bootstrap data supply none of those selections.
    ///
    /// # Errors
    /// Rejects missing native custody, foreign or uninstalled vouchers, expired/replayed
    /// checkpoints, invalid threshold approvals, consensus certificates or receipt membership.
    pub fn redemption_finality_selection<'a>(
        &'a self,
        operation_id: DigestV1,
        canonical_status: &[u8],
        canonical_bootstrap: &[u8],
        pins: KagemushaMobileBootstrapPinsV1<'a>,
    ) -> Result<KagemushaAuthenticatedRedemptionFinalitySelectionV1<'a>, KagemushaStateErrorV1>
    {
        decode_applied_redemption(canonical_status)?;
        KagemushaMobileBootstrapPackageV1::decode_canonical_exact(canonical_bootstrap)
            .map_err(material_error)?;
        let selection = KagemushaAuthenticatedRedemptionFinalitySelectionV1 {
            owner: self,
            operation_id,
            canonical_status: canonical_status.to_vec(),
            canonical_bootstrap: canonical_bootstrap.to_vec(),
            pins,
        };
        selection.recheck_at_trusted_time(pins.trusted_now_ms)?;
        Ok(selection)
    }
}

impl KagemushaAuthenticatedRedemptionFinalitySelectionV1<'_> {
    /// Copy the actual installed native record for the exact release signer.
    ///
    /// # Errors
    /// Rejects stale time, lost custody or an unavailable installed voucher.
    pub fn record_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<KagemushaOutgoingOperationRecordV1, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        let record = self
            .owner
            .outgoing_record_for_operation(self.operation_id)?
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(record)
    }

    /// Copy the exact native voucher already independently authenticated in this selection.
    ///
    /// # Errors
    /// Rejects stale time, changed originals or unavailable current hardware custody.
    pub fn canonical_envelope_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        let envelope = self.owner.original_terminal_envelope(self.operation_id)?;
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(envelope)
    }
    /// Reauthenticate full originals at the current independently trusted native time.
    /// Moving the caller's clock backwards cannot renew an expired checkpoint.
    ///
    /// # Errors
    /// Rejects backwards time, lost current custody or changed/expired finality evidence.
    pub fn recheck_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        if trusted_native_now_ms < self.pins.trusted_now_ms {
            return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
        }
        self.owner.current_recovery_selection()?;
        let mut pins = self.pins;
        pins.trusted_now_ms = trusted_native_now_ms;
        authenticate_originals(
            &self.owner.machine,
            self.operation_id,
            &self.canonical_status,
            &self.canonical_bootstrap,
            &pins,
            None,
        )?;
        self.owner.current_recovery_selection()?;
        Ok(())
    }

    /// Derive the bounded selector from the fully verified original decision and reserve receipt.
    /// The result alone authorizes no device release or capacity change.
    ///
    /// # Errors
    /// Rejects lost native custody, expired authority or any changed original finality binding.
    pub fn terminal_receipt_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<KagemushaRedemptionTerminalReceiptV1, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        let mut pins = self.pins;
        pins.trusted_now_ms = trusted_native_now_ms;
        let receipt = authenticate_originals(
            &self.owner.machine,
            self.operation_id,
            &self.canonical_status,
            &self.canonical_bootstrap,
            &pins,
            None,
        )?;
        self.owner.current_recovery_selection()?;
        Ok(receipt)
    }

    /// Borrow the actual installed native preparation for the fixed release signer.
    ///
    /// # Errors
    /// Rejects stale native time, changed custody or an unavailable original voucher.
    pub fn prepared_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<&PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        Ok(
            &selected_redemption(&self.owner.machine, self.operation_id)?
                .committed
                .candidate
                .prepared,
        )
    }

    /// Recompute the exact native terminal nullifier, outcome and reservation for signing.
    ///
    /// # Errors
    /// Rejects stale native time, changed original custody or inconsistent preparation.
    pub fn hardware_terminal_body_at_trusted_time(
        &self,
        trusted_native_now_ms: u64,
    ) -> Result<KagemushaHardwareTerminalBodyV1, KagemushaStateErrorV1> {
        self.recheck_at_trusted_time(trusted_native_now_ms)?;
        let body = selected_redemption(&self.owner.machine, self.operation_id)?
            .committed
            .candidate
            .hardware_terminal_body()?;
        self.owner.current_recovery_selection()?;
        Ok(body)
    }
}

fn selected_redemption(
    machine: &Machine,
    operation_id: DigestV1,
) -> Result<&DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
    // This independently authenticates the real installed paired proof and native journal.
    machine.original_terminal_envelope(operation_id)?;
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    if record.phase != KagemushaOutgoingOperationPhaseV1::Installed {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let original = machine
        .outgoing_candidate_journal
        .finalized_envelope(record.outbox_reservation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    if !matches!(
        original.envelope,
        KagemushaOutgoingEnvelopeV1::Redemption(_)
    ) {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    Ok(original)
}

fn authenticate_originals(
    machine: &Machine,
    operation_id: DigestV1,
    canonical_status: &[u8],
    canonical_bootstrap: &[u8],
    pins: &KagemushaMobileBootstrapPinsV1<'_>,
    retained_envelope: Option<&[u8]>,
) -> Result<KagemushaRedemptionTerminalReceiptV1, KagemushaStateErrorV1> {
    let release = machine
        .guard_verifier
        .authenticated_release()
        .map_err(material_error)?;
    let state = &machine.state;
    let expected_scope = KagemushaMobileBootstrapScopeV1 {
        asset_identity_digest: kagemusha_asset_identity_digest_v1(&state.lane.asset)
            .map_err(material_error)?,
        asset_incarnation: *state.asset_incarnation.as_bytes(),
        asset_scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
    };
    if pins
        .authority_policy
        .canonical_digest()
        .map_err(material_error)?
        != release.authority_policy_digest()
        || pins.network_id != state.lane.network_id
        || pins.network_id != release.network_id()
        || pins.scope != expected_scope
        || pins.release_id != release.release_id()
        || pins.release_id != state.release_id
        || pins.release_attestation_digest != release.attestation_digest()
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    let (voucher, envelope_digest) = if let Some(bytes) = retained_envelope {
        // Only the already-selected release recovery path supplies these bytes, after
        // matching the exact original signed command to the freshly selected tombstone.
        let voucher = iroha_data_model::kagemusha::KagemushaRedemptionVoucherV1::decode_canonical_shape_exact(bytes)
            .map_err(material_error)?;
        let digest = crate::kagemusha_sender_wire::terminal_envelope_digest_v1(bytes)
            .map_err(material_error)?;
        (voucher, digest)
    } else {
        let original = selected_redemption(machine, operation_id)?;
        let KagemushaOutgoingEnvelopeV1::Redemption(voucher) = &original.envelope else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        (voucher.clone(), original.envelope_digest)
    };
    let status = decode_applied_redemption(canonical_status)?;
    let Some(KagemushaOperationResultV1::Redemption(result)) = &status.result else {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    };
    if status.operation_id != operation_id
        || result.request.operation_id != operation_id
        || result.request.voucher != voucher
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    let package = KagemushaMobileBootstrapPackageV1::decode_canonical_exact(canonical_bootstrap)
        .map_err(material_error)?;
    package.authenticate(pins).map_err(material_error)?;
    let anchor = KagemushaFinalityTrustAnchorV1 {
        network_id: pins.network_id,
        checkpoint: package
            .checkpoint
            .decode_finality_checkpoint()
            .map_err(material_error)?,
    };
    status.validate_against(&anchor).map_err(material_error)?;
    let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &anchor.checkpoint,
        &anchor.network_id,
        anchor.checkpoint.chain_id(),
    )
    .map_err(material_error)?;
    let decision = verifier
        .verify_same_decision(anchor.checkpoint.tip(), &result.finality.finality_proof)
        .map_err(material_error)?;
    let receipt = KagemushaRedemptionTerminalReceiptV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        network_id: pins.network_id,
        operation_id,
        redemption_id: voucher.statement.redemption_id,
        terminal_nullifier: voucher.statement.terminal_nullifier,
        envelope_digest,
        reserve_receipt_digest: result
            .finality
            .reserve_receipt_witness
            .receipt
            .canonical_digest()
            .map_err(material_error)?,
        authenticated_status_digest: canonical_sha256_digest(STATUS_DOMAIN, &status)?,
        finalized_block_height: decision.height(),
        finalized_block_hash: *result.finality.finality_proof.block_header.hash().as_ref(),
        finalized_core_hash: decision.core_hash().0,
        finalized_result: decision.result().0,
    };
    receipt.validate_shape()?;
    Ok(receipt)
}

fn decode_applied_redemption(
    bytes: &[u8],
) -> Result<KagemushaOperationStatusV1, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() > STATUS_MAX_BYTES {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    let status: KagemushaOperationStatusV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(material_error)?;
    if status.state != KagemushaOperationStateV1::Applied
        || status.rejection.is_some()
        || !matches!(
            status.result,
            Some(KagemushaOperationResultV1::Redemption(_))
        )
    {
        return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
    }
    Ok(status)
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::isi::kagemusha_v1::{
        KagemushaOperationKindV1, KagemushaOperationRejectionCodeV1, KagemushaOperationRejectionV1,
    };

    #[test]
    fn pending_or_rejected_status_never_becomes_a_redemption_release_receipt() {
        let mut status = KagemushaOperationStatusV1 {
            version: 1,
            operation_id: [0x71; 32],
            kind: KagemushaOperationKindV1::Redemption,
            state: KagemushaOperationStateV1::Pending,
            result: None,
            rejection: None,
        };
        status.validate().unwrap();
        assert!(decode_applied_redemption(&norito::encode_canonical(&status).unwrap()).is_err());
        status.state = KagemushaOperationStateV1::Rejected;
        status.rejection = Some(KagemushaOperationRejectionV1 {
            code: KagemushaOperationRejectionCodeV1::InvalidProof,
            detail_digest: [0x72; 32],
        });
        status.validate().unwrap();
        assert!(decode_applied_redemption(&norito::encode_canonical(&status).unwrap()).is_err());
    }

    #[test]
    fn finality_original_decode_is_bounded_and_requires_a_complete_canonical_frame() {
        assert!(decode_applied_redemption(&[]).is_err());
        assert!(decode_applied_redemption(&[0x73]).is_err());
        assert!(decode_applied_redemption(&vec![0; STATUS_MAX_BYTES + 1]).is_err());
    }
}
