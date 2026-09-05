//! Native-only private inputs retained from authenticated mint staging into `MintFold`.
//!
//! A caller-provided credit and host-computed digest cannot establish recipient ownership. This
//! projection is constructed only after the state machine selects an exact current pending-map
//! entry and rechecks its authenticated snapshot, replay, retained-credential and Guard evidence.
//! It retains the original credential across ordinary epoch rotation and is deliberately not
//! serializable.

use super::*;
use iroha_data_model::kagemusha::{KagemushaHardwareCredentialV1, KagemushaMintAuthorizationV1};

/// Exact private mint material forwarded from authenticated staging to the recursive witness.
///
/// The custom debug representation omits the plaintext opening, authorization and credit. This
/// value must not cross an SDK, peer, log or unauthenticated persistence boundary.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct KagemushaMintFoldPrivateInputsV1 {
    authorization: KagemushaMintAuthorizationV1,
    recipient_credential: KagemushaHardwareCredentialV1,
    credit_opening: KagemushaCreditOpeningV1,
    credit: KagemushaMintCreditV1,
    stage_certificate: MintStageCertificateV1,
}

/// Borrowed recursive opening of one exact authenticated staged-mint projection.
///
/// There is deliberately no detached constructor. It is created only inside the opaque
/// capability derived from the checked pending entry. Staging evidence remains native journal
/// provenance; it is not presented as circuit authority.
#[derive(Clone, Copy)]
pub(crate) struct KagemushaMintFoldOpeningWitnessV1<'a> {
    authorization: &'a KagemushaMintAuthorizationV1,
    recipient_credential: &'a KagemushaHardwareCredentialV1,
    credit_opening: &'a KagemushaCreditOpeningV1,
    credit: &'a KagemushaMintCreditV1,
}

impl std::fmt::Debug for KagemushaMintFoldOpeningWitnessV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldOpeningWitnessV1")
            .field(
                "credit_id",
                &CreditIdV1(self.credit.statement.lifecycle.credit_id),
            )
            .finish_non_exhaustive()
    }
}

/// Opaque authority to disclose the recursive opening of one checked `MintFold` preview.
///
/// This capability is borrowed from the state-owned private inputs, cannot be constructed by
/// callers, and deliberately has no serialization or default representation. Its debug output
/// identifies the public credit only and omits all opening material.
#[derive(Clone, Copy)]
pub struct KagemushaMintFoldOpeningCapabilityV1<'a> {
    opening: KagemushaMintFoldOpeningWitnessV1<'a>,
}

impl std::fmt::Debug for KagemushaMintFoldOpeningCapabilityV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldOpeningCapabilityV1")
            .field(
                "credit_id",
                &CreditIdV1(self.opening.credit.statement.lifecycle.credit_id),
            )
            .finish_non_exhaustive()
    }
}

impl<'a> KagemushaMintFoldOpeningCapabilityV1<'a> {
    /// Reveal the private recursive witness only inside `iroha_core`.
    pub(crate) fn opening(self) -> KagemushaMintFoldOpeningWitnessV1<'a> {
        self.opening
    }
}

impl<'a> KagemushaMintFoldOpeningWitnessV1<'a> {
    /// Exact paired recipient authorization selected by authenticated staging.
    pub(crate) fn authorization(self) -> &'a KagemushaMintAuthorizationV1 {
        self.authorization
    }

    /// Original enrolled credential; ordinary rotation does not rewrite provenance.
    pub(crate) fn recipient_credential(self) -> &'a KagemushaHardwareCredentialV1 {
        self.recipient_credential
    }

    /// Private commitment openings recovered by the authenticated recipient.
    pub(crate) fn credit_opening(self) -> &'a KagemushaCreditOpeningV1 {
        self.credit_opening
    }

    /// Exact finalized credit whose canonical envelope is consumed by `MintFold`.
    pub(crate) fn credit(self) -> &'a KagemushaMintCreditV1 {
        self.credit
    }
}

impl std::fmt::Debug for KagemushaMintFoldPrivateInputsV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldPrivateInputsV1")
            .field(
                "credit_id",
                &CreditIdV1(self.credit.statement.lifecycle.credit_id),
            )
            .field(
                "hardware_epoch",
                &self.stage_certificate.statement.hardware_epoch,
            )
            .field(
                "inbox_revision",
                &self.stage_certificate.statement.inbox_revision_after,
            )
            .finish_non_exhaustive()
    }
}

impl KagemushaMintFoldPrivateInputsV1 {
    /// Clone a record only after the state machine has authenticated that exact pending entry.
    ///
    /// This constructor is state-module-private. Its caller must select the record from the
    /// machine's current pending map and validate the complete snapshot and GuardBundle; detached
    /// decoded `StagedMintCreditV1` values are not accepted by the public state API.
    pub(super) fn from_checked_pending(
        staged: &StagedMintCreditV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let reservation = staged.reservation();
        reservation.validate_inputs()?;
        staged
            .credit()
            .validate_shape_against_authorization(reservation.authorization())
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        if staged.envelope_digest() != mint_envelope_digest_v1(staged.credit())?
            || staged.credit_id().0 != staged.credit().statement.lifecycle.credit_id
            || staged.stage_certificate().statement.credit_id != staged.credit_id()
            || staged.stage_certificate().statement.envelope_digest != staged.envelope_digest()
            || staged.stage_certificate().statement.reservation_digest != reservation.digest()?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(Self {
            authorization: reservation.authorization().clone(),
            recipient_credential: reservation.recipient_credential().clone(),
            credit_opening: *reservation.credit_opening(),
            credit: staged.credit().clone(),
            stage_certificate: staged.stage_certificate().clone(),
        })
    }

    /// Exact authorization, including the proof whose assigned bytes the circuit must hash.
    pub(crate) fn authorization(&self) -> &KagemushaMintAuthorizationV1 {
        &self.authorization
    }

    /// Original enrolled credential; ordinary rotation must not rewrite committed provenance.
    pub(crate) fn recipient_credential(&self) -> &KagemushaHardwareCredentialV1 {
        &self.recipient_credential
    }

    /// Plaintext commitment openings known only after authenticated recipient decryption.
    pub(crate) fn credit_opening(&self) -> &KagemushaCreditOpeningV1 {
        &self.credit_opening
    }

    /// Exact finalized credit whose complete canonical envelope enters the replay leaf.
    pub(crate) fn credit(&self) -> &KagemushaMintCreditV1 {
        &self.credit
    }

    /// Original irreversible staging evidence, retained byte-for-byte across recovery.
    pub(crate) fn stage_certificate(&self) -> &MintStageCertificateV1 {
        &self.stage_certificate
    }

    /// Borrow the sole public capability derived from this checked pending entry.
    pub(super) fn opening_capability(&self) -> KagemushaMintFoldOpeningCapabilityV1<'_> {
        KagemushaMintFoldOpeningCapabilityV1 {
            opening: self.recursive_witness(),
        }
    }

    /// Borrow the recursive witness projection retained inside the opaque capability.
    fn recursive_witness(&self) -> KagemushaMintFoldOpeningWitnessV1<'_> {
        KagemushaMintFoldOpeningWitnessV1 {
            authorization: &self.authorization,
            recipient_credential: &self.recipient_credential,
            credit_opening: &self.credit_opening,
            credit: &self.credit,
        }
    }
}
