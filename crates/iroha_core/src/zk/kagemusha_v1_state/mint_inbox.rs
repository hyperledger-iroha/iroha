//! Typed, non-monetary mint-inbox persistence and exact recovery projections.
//!
//! These records never authenticate their own storage. The state-machine operations must verify
//! qualified hardware reservation/staging certificates and the latest durability anchor before
//! installing a journal. In particular, a decoded reservation is not proof of local key ownership.
//!
//! This is a trusted-native persistence API, not an SDK or peer-wire format. Canonical `Encode`
//! of a reservation, staged record, journal, or containing snapshot includes private plaintext
//! opening material. Its bytes must remain authenticated and confidential inside the qualified
//! native storage boundary; never send them to a host SDK/peer or log their `Debug` projection.
//! Public Rust visibility exists only for qualified native-adapter integration. Borrowed public
//! recovery views deliberately omit the opening and do not authenticate the underlying storage.

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    KagemushaAuthenticatedRecursiveVerifierV1, verify_kagemusha_mint_finality_helper_v1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1, KagemushaHardwareCredentialV1,
    KagemushaMintAuthorizationV1, kagemusha_mint_credit_opening_commitment_v1,
    kagemusha_recipient_credential_commitment_v1,
};

const RESERVATION_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-inbox-reservation";
const JOURNAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:mint-inbox-journal";
// Upper bound for the fixed stage statement, map keys, and all Norito field/sequence framing.
// This is reserved physical storage, not an admission count or transaction-history limit.
const FIXED_STAGE_FRAMING_RESERVATION_BYTES: u64 = 2_048;
const JOURNAL_MAP_FRAMING_BYTES: u64 = 64;

/// Shape-checked pre-debit inputs whose ownership/capacity must be hardware-certified separately.
///
/// Native-only confidential persistence: its canonical encoding contains the private credit
/// opening. It is not an application/SDK transport record, despite public Rust visibility.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct MintInboxReservationV1 {
    authorization: KagemushaMintAuthorizationV1,
    recipient_credential: KagemushaHardwareCredentialV1,
    credit_opening: KagemushaCreditOpeningV1,
    recipient_key_handle_binding: DigestV1,
    reserved_inbox_bytes: u64,
}

impl std::fmt::Debug for MintInboxReservationV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MintInboxReservationV1")
            .field("credit_id", &self.credit_id())
            .field("operation_id", &self.operation_id())
            .field("reserved_inbox_bytes", &self.reserved_inbox_bytes)
            .finish_non_exhaustive()
    }
}

impl MintInboxReservationV1 {
    /// Construct nonauthorizing inputs for the qualified reservation service.
    ///
    /// Hardware must bind the exact opening to authenticated decryption/key provenance and
    /// durably reserve the returned footprint before exposing the mint authorization.
    pub fn new(
        authorization: KagemushaMintAuthorizationV1,
        recipient_credential: KagemushaHardwareCredentialV1,
        credit_opening: KagemushaCreditOpeningV1,
        recipient_key_handle_binding: DigestV1,
        reserved_inbox_bytes: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let record = Self {
            authorization,
            recipient_credential,
            credit_opening,
            recipient_key_handle_binding,
            reserved_inbox_bytes,
        };
        record.validate_inputs()?;
        Ok(record)
    }

    /// Compute the conservative full staging allocation before constructing a reservation.
    pub fn required_reservation_bytes(
        authorization: &KagemushaMintAuthorizationV1,
        recipient_credential: &KagemushaHardwareCredentialV1,
        credit_opening: &KagemushaCreditOpeningV1,
        recipient_key_handle_binding: DigestV1,
    ) -> Result<u64, KagemushaStateErrorV1> {
        // Reject oversized/malformed caller-owned proof arrays before cloning any owned inputs.
        authorization
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        recipient_credential
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        credit_opening
            .validate_shape_against(
                authorization.statement.credit_id,
                authorization.statement.context.amount,
            )
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let record = Self {
            authorization: authorization.clone(),
            recipient_credential: recipient_credential.clone(),
            credit_opening: *credit_opening,
            recipient_key_handle_binding,
            // Use the largest integer representation even if a codec layout packs integers.
            reserved_inbox_bytes: u64::MAX,
        };
        record.validate_inputs()?;
        record.minimum_reserved_bytes()
    }

    /// Exact authorization persisted before reserve debit.
    pub fn authorization(&self) -> &KagemushaMintAuthorizationV1 {
        &self.authorization
    }
    /// Original credential; it remains available across offline epoch rotation.
    pub fn recipient_credential(&self) -> &KagemushaHardwareCredentialV1 {
        &self.recipient_credential
    }
    /// Opaque non-exportable key-handle binding, not an encryption private key.
    pub const fn recipient_key_handle_binding(&self) -> DigestV1 {
        self.recipient_key_handle_binding
    }
    /// Unique consensus top-up operation identity.
    pub fn operation_id(&self) -> DigestV1 {
        self.authorization.statement.context.operation_id
    }
    /// Unique credit identity derived before the top-up is submitted.
    pub fn credit_id(&self) -> CreditIdV1 {
        CreditIdV1(self.authorization.statement.credit_id)
    }
    /// Original one-use recipient encryption key.
    pub fn recipient_one_time_key(&self) -> DigestV1 {
        self.authorization.statement.context.recipient_one_time_key
    }
    /// Physical allocation which remains charged until the staged mint is folded.
    pub const fn reserved_inbox_bytes(&self) -> u64 {
        self.reserved_inbox_bytes
    }
    /// Exact shape-record digest authenticated by the qualified reservation certificate.
    pub fn digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.validate_inputs()?;
        canonical_sha256_digest(RESERVATION_DOMAIN, self)
    }
    /// Minimum allocation covering this record, one maximal credit, and one maximal GuardBundle.
    pub fn minimum_reserved_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        receiver_sequence_entry_bytes(self)?
            .checked_add(KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1 as u64)
            .and_then(|n| n.checked_add(KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 as u64))
            .and_then(|n| n.checked_add(FIXED_STAGE_FRAMING_RESERVATION_BYTES))
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)
    }
    pub(super) fn credit_opening(&self) -> &KagemushaCreditOpeningV1 {
        &self.credit_opening
    }

    /// Validate bounded input shape and exact public/private commitment equality, not authority.
    pub fn validate_inputs(&self) -> Result<(), KagemushaStateErrorV1> {
        self.authorization
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        self.recipient_credential
            .validate_shape()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        let context = &self.authorization.statement.context;
        self.credit_opening
            .validate_shape_against(self.credit_id().0, context.amount)
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        if self.recipient_key_handle_binding == [0; 32]
            || context.hardware_credential_id != self.recipient_credential.credential_id
            || context.network_id != self.recipient_credential.network_id
            || context.hardware_profile_id != self.recipient_credential.hardware_profile_id
            || context.suite_id != self.recipient_credential.suite_id
            || context.policy_epoch != self.recipient_credential.policy_epoch
            || context.recipient_credential_commitment
                != kagemusha_recipient_credential_commitment_v1(
                    context.operation_id,
                    context.hardware_credential_id,
                    self.credit_opening.recipient_binding_opening,
                )
                .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?
            || context.credit_commitment
                != kagemusha_mint_credit_opening_commitment_v1(
                    &context.network_id,
                    &context.asset,
                    context.asset_incarnation,
                    context.scale,
                    context.liability_pool_id,
                    context.amount,
                    &context.recipient,
                    context.recipient_one_time_key,
                    self.credit_opening.credit_commitment_opening,
                )
                .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?
        {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        if self.reserved_inbox_bytes < self.minimum_reserved_bytes()? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }

    fn validate_against_state(
        &self,
        state: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_inputs()?;
        let context = &self.authorization.statement.context;
        let credential = &self.recipient_credential;
        if context.release_id != state.release_id
            || context.suite_id != state.suite_id
            || context.vk_digest != state.vk_digest
            || context.network_id != state.lane.network_id
            || context.asset != state.lane.asset
            || context.asset_incarnation != state.asset_incarnation
            || context.scale != state.lane.scale
            || context.liability_pool_id != state.liability_pool_id
            || context.hardware_profile_id != state.hardware_profile_id
            || context.policy_epoch != state.policy_epoch
            || credential.lane_commitment != state.lane.device_lane_id
            || u128::from(credential.hardware_epoch_generation) > state.hardware_epoch.generation
            || (u128::from(credential.hardware_epoch_generation) == state.hardware_epoch.generation
                && credential.hardware_epoch_id != state.hardware_epoch.epoch_id)
        {
            return Err(KagemushaStateErrorV1::InvalidMintCredit);
        }
        Ok(())
    }
}

/// Exact non-monetary hardware reservation transaction, before online debit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct MintReservationStatementV1 {
    /// State format version.
    pub version: u16,
    /// Stable local recipient lane and asset.
    pub lane: KagemushaLaneIdV1,
    /// Hardware epoch performing this reservation.
    pub hardware_epoch: HardwareEpochV1,
    /// Unchanged aggregate balance commitment.
    pub state_commitment: DigestV1,
    /// Consumed independent inbox revision.
    pub inbox_revision_before: u128,
    /// Exact next independent inbox revision.
    pub inbox_revision_after: u128,
    /// Exact reservation, including credential/opening/key-handle/capacity bindings.
    pub reservation_digest: DigestV1,
    /// Journal commitment consumed by this transaction.
    pub predecessor_journal_commitment: DigestV1,
    /// Journal commitment installed by this transaction.
    pub successor_journal_commitment: DigestV1,
    /// Shared physical receiver-capacity ledger installed atomically.
    pub successor_capacity_commitment: DigestV1,
}

/// Qualified hardware evidence for pre-debit reservation; a signature-only backend is insufficient.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct MintReservationCertificateV1 {
    /// Exact reserved successor.
    pub statement: MintReservationStatementV1,
    /// Complete qualified non-forking journal evidence.
    pub guard_bundle: Vec<u8>,
}

/// Exact independent inbox transaction accepting one already-finalized mint.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct MintStageStatementV1 {
    /// State format version.
    pub version: u16,
    /// Stable local recipient lane and asset.
    pub lane: KagemushaLaneIdV1,
    /// Hardware epoch performing staging.
    pub hardware_epoch: HardwareEpochV1,
    /// Unchanged aggregate balance commitment.
    pub state_commitment: DigestV1,
    /// Consumed independent inbox revision.
    pub inbox_revision_before: u128,
    /// Exact next independent inbox revision.
    pub inbox_revision_after: u128,
    /// Exact hardware-anchored original reservation.
    pub reservation_digest: DigestV1,
    /// Unique finalized credit identity.
    pub credit_id: CreditIdV1,
    /// Exact canonical finalized mint envelope digest, also used by monetary replay roots.
    pub envelope_digest: DigestV1,
    /// Local trusted staging time; finalized mint validity does not expire at arrival.
    pub staged_at_ms: u64,
    /// Journal commitment consumed by staging.
    pub predecessor_journal_commitment: DigestV1,
    /// Certificate-independent record projection installed by staging.
    pub successor_journal_commitment: DigestV1,
    /// Shared physical receiver-capacity successor installed atomically.
    pub successor_capacity_commitment: DigestV1,
}

/// Qualified hardware evidence of irreversible, recoverable mint staging.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct MintStageCertificateV1 {
    /// Exact accepted successor.
    pub statement: MintStageStatementV1,
    /// Complete qualified journal evidence, retained byte-for-byte across recovery.
    pub guard_bundle: Vec<u8>,
}

/// An exact finalized mint retained in authenticated pending storage.
///
/// Native-only confidential encoding; the enclosed reservation includes private opening material.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct StagedMintCreditV1 {
    reservation: MintInboxReservationV1,
    credit: KagemushaMintCreditV1,
    envelope_digest: DigestV1,
    stage_certificate: MintStageCertificateV1,
}

impl StagedMintCreditV1 {
    /// Original hardware-reserved inputs.
    pub fn reservation(&self) -> &MintInboxReservationV1 {
        &self.reservation
    }
    /// Exact finalized mint bytes represented by the canonical model.
    pub fn credit(&self) -> &KagemushaMintCreditV1 {
        &self.credit
    }
    /// Exact replay identity.
    pub fn credit_id(&self) -> CreditIdV1 {
        self.reservation.credit_id()
    }
    /// Mint amount, still outside the aggregate balance until folding succeeds.
    pub fn amount(&self) -> u128 {
        self.credit.statement.amount
    }
    /// Exact original canonical envelope digest.
    pub const fn envelope_digest(&self) -> DigestV1 {
        self.envelope_digest
    }
    /// Original stage certificate, not regenerated on retry.
    pub fn stage_certificate(&self) -> &MintStageCertificateV1 {
        &self.stage_certificate
    }
    /// Immutable public recovery inputs; no opening or mutable backing buffers are exposed.
    pub fn recovery_view(&self) -> StagedMintRecoveryViewV1<'_> {
        StagedMintRecoveryViewV1 {
            authorization: self.reservation.authorization(),
            credit: &self.credit,
            recipient_credential: self.reservation.recipient_credential(),
            stage_certificate: &self.stage_certificate,
        }
    }
}

/// Borrowed public mint material from an already-authenticated recovery journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagedMintRecoveryViewV1<'a> {
    /// Exact original authorization.
    pub authorization: &'a KagemushaMintAuthorizationV1,
    /// Exact finalized mint, including original ciphertext.
    pub credit: &'a KagemushaMintCreditV1,
    /// Original credential needed to retain old-epoch provenance.
    pub recipient_credential: &'a KagemushaHardwareCredentialV1,
    /// Original non-monetary hardware staging evidence.
    pub stage_certificate: &'a MintStageCertificateV1,
}

/// Compact historical exact-identity receipt retained after the mint has been folded.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct AcceptedMintReceiptV1 {
    credit_id: CreditIdV1,
    operation_id: DigestV1,
    recipient_one_time_key: DigestV1,
    recipient_key_handle_binding: DigestV1,
    authorization_digest: DigestV1,
    envelope_digest: DigestV1,
    reservation_digest: DigestV1,
    stage_certificate: MintStageCertificateV1,
}

impl std::fmt::Debug for AcceptedMintReceiptV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcceptedMintReceiptV1")
            .field("credit_id", &self.credit_id)
            .field("operation_id", &self.operation_id)
            .field(
                "inbox_revision",
                &self.stage_certificate.statement.inbox_revision_after,
            )
            .finish_non_exhaustive()
    }
}

impl AcceptedMintReceiptV1 {
    /// Original credit identity.
    pub const fn credit_id(&self) -> CreditIdV1 {
        self.credit_id
    }
    /// Original top-up operation identity.
    pub const fn operation_id(&self) -> DigestV1 {
        self.operation_id
    }
    /// Original one-use encryption key, retained to prevent reuse after folding.
    pub const fn recipient_one_time_key(&self) -> DigestV1 {
        self.recipient_one_time_key
    }
    /// Original opaque key-handle identity.
    pub const fn recipient_key_handle_binding(&self) -> DigestV1 {
        self.recipient_key_handle_binding
    }
    /// Exact authorization identity, including its proof bytes.
    pub const fn authorization_digest(&self) -> DigestV1 {
        self.authorization_digest
    }
    /// Exact original mint envelope identity.
    pub const fn envelope_digest(&self) -> DigestV1 {
        self.envelope_digest
    }
    /// Exact original reservation identity.
    pub const fn reservation_digest(&self) -> DigestV1 {
        self.reservation_digest
    }
    /// Original hardware evidence, retained byte-for-byte.
    pub fn stage_certificate(&self) -> &MintStageCertificateV1 {
        &self.stage_certificate
    }
    /// Classify only exact bounded authorization and mint bytes as a historical duplicate.
    pub fn matches_delivery(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
        credit: &KagemushaMintCreditV1,
    ) -> Result<bool, KagemushaStateErrorV1> {
        credit
            .validate_shape_against_authorization(authorization)
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        Ok(self.credit_id.0 == credit.statement.lifecycle.credit_id
            && self.authorization_digest
                == authorization
                    .canonical_digest()
                    .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?
            && self.envelope_digest == mint_envelope_digest_v1(credit)?)
    }
}

/// Proof-authenticated exact mint inputs; only installed hardware reservations grant local ownership.
///
/// This native-only capability contains private reservation material and must not be logged or
/// exposed through host SDK/peer interfaces. It has no public decoder or unchecked constructor.
#[derive(Debug)]
pub struct VerifiedMintStageV1 {
    reservation: MintInboxReservationV1,
    credit: KagemushaMintCreditV1,
    envelope_digest: DigestV1,
    mint_finality: VerifiedKagemushaMintFinalityHelperV1,
}

impl VerifiedMintStageV1 {
    /// Exact reservation which state operations must match against their authenticated journal.
    pub fn reservation(&self) -> &MintInboxReservationV1 {
        &self.reservation
    }
    /// Exact proof-authenticated finalized credit.
    pub fn credit(&self) -> &KagemushaMintCreditV1 {
        &self.credit
    }
    /// Canonical replay digest.
    pub const fn envelope_digest(&self) -> DigestV1 {
        self.envelope_digest
    }
    /// Real finalized-helper verification token for the eventual mint fold.
    pub const fn mint_finality(&self) -> VerifiedKagemushaMintFinalityHelperV1 {
        self.mint_finality
    }

    /// Build test-only inputs after a test explicitly substitutes both recursive verifiers.
    #[cfg(test)]
    pub(crate) fn for_tests(
        reservation: MintInboxReservationV1,
        credit: KagemushaMintCreditV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_mint_inputs(&reservation, &credit)?;
        let semantic = credit
            .statement
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        Ok(Self { envelope_digest: mint_envelope_digest_v1(&credit)?, reservation, credit,
            mint_finality: VerifiedKagemushaMintFinalityHelperV1::for_state_tests_after_mock_finality_verification(semantic)
                .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)? })
    }
}

/// Verify both actual release-authenticated proof relations, without a generic accepting backend.
///
/// This proves exact issuance/authorization/opening commitments. The caller must additionally
/// match `reservation` to its hardware-anchored local journal; neither a host-supplied key-handle
/// digest nor this capability by itself proves that the current device owns the private key.
pub fn verify_mint_stage_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    artifacts: KagemushaRecursionArtifactsV1,
    reservation: &MintInboxReservationV1,
    credit: &KagemushaMintCreditV1,
) -> Result<VerifiedMintStageV1, KagemushaStateErrorV1> {
    // Complete borrowed shape/size checks before hashing, proof verification, or owned cloning.
    validate_mint_inputs(reservation, credit)?;
    let envelope_digest = canonical_sha256_digest(MINT_CREDIT_DOMAIN, credit)?;
    verifier
        .verify_mint_authorization(reservation.authorization())
        .map_err(KagemushaStateErrorV1::ProofRejected)?;
    let mint_finality = verify_kagemusha_mint_finality_helper_v1(verifier, artifacts, credit)
        .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
    Ok(VerifiedMintStageV1 {
        reservation: reservation.clone(),
        credit: credit.clone(),
        envelope_digest,
        mint_finality,
    })
}

/// Return the exact same canonical mint identity committed by the existing monetary replay tree.
pub fn mint_envelope_digest_v1(
    credit: &KagemushaMintCreditV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    credit
        .validate_shape()
        .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
    canonical_sha256_digest(MINT_CREDIT_DOMAIN, credit)
}

fn validate_mint_inputs(
    reservation: &MintInboxReservationV1,
    credit: &KagemushaMintCreditV1,
) -> Result<(), KagemushaStateErrorV1> {
    reservation.validate_inputs()?;
    credit
        .validate_shape_against_authorization(reservation.authorization())
        .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)
}

/// Canonical typed mint journal carried as one field of the hardware-anchored wallet snapshot.
///
/// Encoding includes private opening material from live records. Only confidential, authenticated
/// native persistence may consume those bytes; this is never a host SDK or peer-wire message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaMintInboxV1 {
    reservations: BTreeMap<CreditIdV1, MintInboxReservationV1>,
    pending: BTreeMap<CreditIdV1, StagedMintCreditV1>,
    accepted: BTreeMap<CreditIdV1, AcceptedMintReceiptV1>,
}

#[derive(Encode)]
struct MintJournalProjectionV1 {
    reservations: Vec<(CreditIdV1, DigestV1)>,
    pending: Vec<MintReceiptProjectionV1>,
    accepted: Vec<MintReceiptProjectionV1>,
}

#[derive(Encode)]
struct MintReceiptProjectionV1 {
    credit_id: CreditIdV1,
    operation_id: DigestV1,
    recipient_one_time_key: DigestV1,
    recipient_key_handle_binding: DigestV1,
    authorization_digest: DigestV1,
    envelope_digest: DigestV1,
    reservation_digest: DigestV1,
    stage_epoch: HardwareEpochV1,
    stage_revision: u128,
    staged_at_ms: u64,
}

impl From<&AcceptedMintReceiptV1> for MintReceiptProjectionV1 {
    fn from(receipt: &AcceptedMintReceiptV1) -> Self {
        Self {
            credit_id: receipt.credit_id,
            operation_id: receipt.operation_id,
            recipient_one_time_key: receipt.recipient_one_time_key,
            recipient_key_handle_binding: receipt.recipient_key_handle_binding,
            authorization_digest: receipt.authorization_digest,
            envelope_digest: receipt.envelope_digest,
            reservation_digest: receipt.reservation_digest,
            stage_epoch: receipt.stage_certificate.statement.hardware_epoch,
            stage_revision: receipt.stage_certificate.statement.inbox_revision_after,
            staged_at_ms: receipt.stage_certificate.statement.staged_at_ms,
        }
    }
}

fn receipt_from_staged(
    staged: &StagedMintCreditV1,
) -> Result<AcceptedMintReceiptV1, KagemushaStateErrorV1> {
    Ok(AcceptedMintReceiptV1 {
        credit_id: staged.credit_id(),
        operation_id: staged.reservation.operation_id(),
        recipient_one_time_key: staged.reservation.recipient_one_time_key(),
        recipient_key_handle_binding: staged.reservation.recipient_key_handle_binding(),
        authorization_digest: staged
            .reservation
            .authorization
            .canonical_digest()
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?,
        envelope_digest: staged.envelope_digest,
        reservation_digest: staged.reservation.digest()?,
        stage_certificate: staged.stage_certificate.clone(),
    })
}

impl KagemushaMintInboxV1 {
    /// Number of pending finalized mints, without a protocol count ceiling.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    /// Whether an outstanding authorization or finalized mint must survive recovery/rotation.
    pub fn has_unresolved_credits(&self) -> bool {
        !self.reservations.is_empty() || !self.pending.is_empty()
    }
    /// Borrow all pending records in deterministic credit-ID order.
    pub fn pending_values(&self) -> impl Iterator<Item = &StagedMintCreditV1> {
        self.pending.values()
    }
    /// Borrow every live pre-debit reservation in deterministic order.
    pub fn reservations(&self) -> &BTreeMap<CreditIdV1, MintInboxReservationV1> {
        &self.reservations
    }
    /// Borrow every pending mint record in deterministic order.
    pub fn pending(&self) -> &BTreeMap<CreditIdV1, StagedMintCreditV1> {
        &self.pending
    }
    /// Borrow every historical consumed-mint receipt in deterministic order.
    pub fn accepted(&self) -> &BTreeMap<CreditIdV1, AcceptedMintReceiptV1> {
        &self.accepted
    }
    /// Whether any live or historical mint record already owns this credit identity.
    pub fn contains_credit_id(&self, id: CreditIdV1) -> bool {
        self.reservations.contains_key(&id)
            || self.pending.contains_key(&id)
            || self.accepted.contains_key(&id)
    }
    /// Borrow one live original reservation.
    pub fn reservation(&self, id: CreditIdV1) -> Option<&MintInboxReservationV1> {
        self.reservations.get(&id)
    }
    /// Borrow one pending mint.
    pub fn pending_credit(&self, id: CreditIdV1) -> Option<&StagedMintCreditV1> {
        self.pending.get(&id)
    }
    /// Borrow one consumed-mint receipt.
    pub fn accepted_receipt(&self, id: CreditIdV1) -> Option<&AcceptedMintReceiptV1> {
        self.accepted.get(&id)
    }

    /// Canonical logical projection, excluding certificates to avoid a successor-hash fixed point.
    /// Full canonical snapshots separately bind every original certificate and GuardBundle byte.
    pub fn commitment(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        let reservations = self
            .reservations
            .iter()
            .map(|(id, record)| Ok((*id, record.digest()?)))
            .collect::<Result<Vec<_>, KagemushaStateErrorV1>>()?;
        let pending = self
            .pending
            .values()
            .map(|record| Ok(MintReceiptProjectionV1::from(&receipt_from_staged(record)?)))
            .collect::<Result<Vec<_>, KagemushaStateErrorV1>>()?;
        let accepted = self
            .accepted
            .values()
            .map(MintReceiptProjectionV1::from)
            .collect();
        canonical_sha256_digest(
            JOURNAL_DOMAIN,
            &MintJournalProjectionV1 {
                reservations,
                pending,
                accepted,
            },
        )
    }

    /// Shared-ledger physical charge: live reservations/pending records keep their full ceiling;
    /// folded receipts keep only their exact metadata/certificate footprint.
    pub fn capacity_charge_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        if self.reservations.is_empty() && self.pending.is_empty() && self.accepted.is_empty() {
            return Ok(0);
        }
        let mut total = JOURNAL_MAP_FRAMING_BYTES;
        for bytes in self
            .reservations
            .values()
            .map(MintInboxReservationV1::reserved_inbox_bytes)
            .chain(
                self.pending
                    .values()
                    .map(|record| record.reservation.reserved_inbox_bytes()),
            )
        {
            total = total
                .checked_add(bytes)
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        }
        for receipt in self.accepted.values() {
            total = total
                .checked_add(receiver_sequence_entry_bytes(&(
                    receipt.credit_id,
                    receipt.clone(),
                ))?)
                .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        }
        Ok(total)
    }

    /// Compute a reservation successor only; the caller must certify it before installation.
    pub fn reserve_successor(
        &self,
        record: &MintInboxReservationV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        record.validate_inputs()?;
        if let Some(existing) = self.reservations.get(&record.credit_id()) {
            return if existing == record {
                Ok(self.clone())
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(record.credit_id()))
            };
        }
        if self.contains_credit_id(record.credit_id()) {
            return Err(KagemushaStateErrorV1::CreditAlreadyConsumed(
                record.credit_id(),
            ));
        }
        self.ensure_unique_operation_key(
            record.operation_id(),
            record.recipient_one_time_key(),
            record.recipient_key_handle_binding(),
        )?;
        let mut next = self.clone();
        next.reservations.insert(record.credit_id(), record.clone());
        next.capacity_charge_bytes()?;
        Ok(next)
    }

    /// Compute nonauthorizing staging projection before hardware supplies the final certificate.
    /// Its placeholder certificate cannot be installed via `staged_successor` or recovered.
    pub fn preview_staged_successor(
        &self,
        verified: &VerifiedMintStageV1,
        inbox_revision_after: u128,
        hardware_epoch: HardwareEpochV1,
        staged_at_ms: u64,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let reservation = verified.reservation();
        let context = &reservation.authorization.statement.context;
        let certificate = MintStageCertificateV1 {
            statement: MintStageStatementV1 {
                version: KAGEMUSHA_STATE_VERSION_V1,
                lane: KagemushaLaneIdV1 {
                    network_id: context.network_id,
                    device_lane_id: reservation.recipient_credential.lane_commitment,
                    asset: context.asset.clone(),
                    scale: context.scale,
                },
                hardware_epoch,
                state_commitment: [0; 32],
                inbox_revision_before: inbox_revision_after
                    .checked_sub(1)
                    .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
                inbox_revision_after,
                reservation_digest: reservation.digest()?,
                credit_id: reservation.credit_id(),
                envelope_digest: verified.envelope_digest(),
                staged_at_ms,
                predecessor_journal_commitment: [0; 32],
                successor_journal_commitment: [0; 32],
                successor_capacity_commitment: [0; 32],
            },
            guard_bundle: vec![1],
        };
        self.stage_projection(verified, &certificate)
    }

    /// Compute the final checked staging successor; hardware/physical-ledger verification remains
    /// mandatory in the state-machine operation before publishing it.
    pub fn staged_successor(
        &self,
        verified: &VerifiedMintStageV1,
        certificate: &MintStageCertificateV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        validate_stage_certificate(certificate)?;
        let next = self.stage_projection(verified, certificate)?;
        if certificate.statement.predecessor_journal_commitment != self.commitment()?
            || certificate.statement.successor_journal_commitment != next.commitment()?
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        Ok(next)
    }

    fn stage_projection(
        &self,
        verified: &VerifiedMintStageV1,
        certificate: &MintStageCertificateV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let record = verified.reservation();
        let id = record.credit_id();
        if self.reservations.get(&id) != Some(record)
            || self.pending.contains_key(&id)
            || self.accepted.contains_key(&id)
        {
            return Err(KagemushaStateErrorV1::CreditConflict(id));
        }
        let statement = &certificate.statement;
        let context = &record.authorization.statement.context;
        if statement.credit_id != id
            || statement.envelope_digest != verified.envelope_digest()
            || statement.reservation_digest != record.digest()?
            || statement.lane.device_lane_id != record.recipient_credential.lane_commitment
            || statement.lane.network_id != context.network_id
            || statement.lane.asset != context.asset
            || statement.lane.scale != context.scale
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let staged = StagedMintCreditV1 {
            reservation: record.clone(),
            credit: verified.credit.clone(),
            envelope_digest: verified.envelope_digest,
            stage_certificate: certificate.clone(),
        };
        if receiver_sequence_entry_bytes(&(id, staged.clone()))? > record.reserved_inbox_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let mut next = self.clone();
        next.reservations.remove(&id);
        next.pending.insert(id, staged);
        if next.capacity_charge_bytes()? > self.capacity_charge_bytes()? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(next)
    }

    /// Require the exact pending bytes before a monetary fold may consume them.
    pub fn validate_fold(
        &self,
        credit: &KagemushaMintCreditV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let id = CreditIdV1(credit.statement.lifecycle.credit_id);
        let digest = mint_envelope_digest_v1(credit)?;
        if let Some(staged) = self.pending.get(&id) {
            if staged.credit == *credit && staged.envelope_digest == digest {
                return Ok(());
            }
            return Err(KagemushaStateErrorV1::CreditConflict(id));
        }
        if let Some(receipt) = self.accepted.get(&id) {
            return Err(if receipt.envelope_digest == digest {
                KagemushaStateErrorV1::CreditAlreadyConsumed(id)
            } else {
                KagemushaStateErrorV1::CreditConflict(id)
            });
        }
        Err(KagemushaStateErrorV1::CreditNotStaged(id))
    }

    /// Prepare pending removal/compact receipt installation before the irreversible replay CAS.
    pub fn folded_successor(
        &self,
        credit: &KagemushaMintCreditV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        self.validate_fold(credit)?;
        let id = CreditIdV1(credit.statement.lifecycle.credit_id);
        let mut next = self.clone();
        let staged = next
            .pending
            .remove(&id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(id))?;
        next.accepted.insert(id, receipt_from_staged(&staged)?);
        if next.capacity_charge_bytes()? > self.capacity_charge_bytes()? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(next)
    }

    /// Validate canonical record consistency only. Recovery must additionally authenticate the
    /// snapshot/GuardBundles, both replay indexes, and current-release proofs of every pending mint.
    pub fn validate_recovered(
        &self,
        state: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let mut ids = BTreeSet::new();
        let mut operations = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut handles = BTreeSet::new();
        let mut revisions = BTreeSet::new();
        for (id, reservation) in &self.reservations {
            reservation.validate_against_state(state)?;
            if *id != reservation.credit_id()
                || !ids.insert(*id)
                || !operations.insert(reservation.operation_id())
                || !keys.insert(reservation.recipient_one_time_key())
                || !handles.insert(reservation.recipient_key_handle_binding())
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        for (id, staged) in &self.pending {
            staged.reservation.validate_against_state(state)?;
            validate_mint_inputs(&staged.reservation, &staged.credit)?;
            if *id != staged.credit_id()
                || staged.envelope_digest != mint_envelope_digest_v1(&staged.credit)?
                || receiver_sequence_entry_bytes(&(*id, staged.clone()))?
                    > staged.reservation.reserved_inbox_bytes()
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let receipt = receipt_from_staged(staged)?;
            validate_receipt(&receipt, state)?;
            if !ids.insert(*id)
                || !operations.insert(receipt.operation_id)
                || !keys.insert(receipt.recipient_one_time_key)
                || !handles.insert(receipt.recipient_key_handle_binding)
                || !revisions.insert((
                    receipt.stage_certificate.statement.hardware_epoch.epoch_id,
                    receipt.stage_certificate.statement.inbox_revision_after,
                ))
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        for (id, receipt) in &self.accepted {
            validate_receipt(receipt, state)?;
            if *id != receipt.credit_id
                || !ids.insert(*id)
                || !operations.insert(receipt.operation_id)
                || !keys.insert(receipt.recipient_one_time_key)
                || !handles.insert(receipt.recipient_key_handle_binding)
                || !revisions.insert((
                    receipt.stage_certificate.statement.hardware_epoch.epoch_id,
                    receipt.stage_certificate.statement.inbox_revision_after,
                ))
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        self.capacity_charge_bytes()?;
        Ok(())
    }

    fn ensure_unique_operation_key(
        &self,
        operation: DigestV1,
        key: DigestV1,
        handle: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let conflict = self
            .reservations
            .values()
            .chain(self.pending.values().map(|record| &record.reservation))
            .any(|record| {
                record.operation_id() == operation
                    || record.recipient_one_time_key() == key
                    || record.recipient_key_handle_binding() == handle
            })
            || self.accepted.values().any(|record| {
                record.operation_id == operation
                    || record.recipient_one_time_key == key
                    || record.recipient_key_handle_binding == handle
            });
        if conflict {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        Ok(())
    }
}

fn validate_stage_certificate(
    certificate: &MintStageCertificateV1,
) -> Result<(), KagemushaStateErrorV1> {
    validate_guard_bytes(&certificate.guard_bundle)?;
    let statement = &certificate.statement;
    statement.lane.validate()?;
    statement.hardware_epoch.validate()?;
    if statement.version != KAGEMUSHA_STATE_VERSION_V1
        || statement.credit_id.is_zero()
        || statement.state_commitment == [0; 32]
        || statement.reservation_digest == [0; 32]
        || statement.envelope_digest == [0; 32]
        || statement.staged_at_ms == 0
        || statement.predecessor_journal_commitment == [0; 32]
        || statement.successor_journal_commitment == [0; 32]
        || statement.successor_capacity_commitment == [0; 32]
        || statement.inbox_revision_before.checked_add(1) != Some(statement.inbox_revision_after)
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    Ok(())
}

fn validate_receipt(
    receipt: &AcceptedMintReceiptV1,
    state: &KagemushaStateV1,
) -> Result<(), KagemushaStateErrorV1> {
    validate_stage_certificate(&receipt.stage_certificate)?;
    let statement = &receipt.stage_certificate.statement;
    if receipt.credit_id != statement.credit_id
        || receipt.envelope_digest != statement.envelope_digest
        || receipt.reservation_digest != statement.reservation_digest
        || receipt.operation_id == [0; 32]
        || receipt.recipient_one_time_key == [0; 32]
        || receipt.recipient_key_handle_binding == [0; 32]
        || receipt.authorization_digest == [0; 32]
        || statement.lane != state.lane
        || statement.hardware_epoch.generation > state.hardware_epoch.generation
        || (statement.hardware_epoch.generation == state.hardware_epoch.generation
            && statement.hardware_epoch != state.hardware_epoch)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{block::BlockHeader, domain::DomainId};

    // Deliberately nonauthorizing structural data for projection/accounting tests only.
    fn historical_receipt() -> AcceptedMintReceiptV1 {
        let lane = KagemushaLaneIdV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"mint-inbox-structural-tests",
                )),
            ),
            device_lane_id: [10; 32],
            asset: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            scale: 4,
        };
        AcceptedMintReceiptV1 {
            credit_id: CreditIdV1([1; 32]),
            operation_id: [2; 32],
            recipient_one_time_key: [3; 32],
            recipient_key_handle_binding: [4; 32],
            authorization_digest: [5; 32],
            envelope_digest: [6; 32],
            reservation_digest: [7; 32],
            stage_certificate: MintStageCertificateV1 {
                statement: MintStageStatementV1 {
                    version: KAGEMUSHA_STATE_VERSION_V1,
                    lane,
                    hardware_epoch: HardwareEpochV1 {
                        generation: 1,
                        epoch_id: [8; 32],
                    },
                    state_commitment: [9; 32],
                    inbox_revision_before: 0,
                    inbox_revision_after: 1,
                    reservation_digest: [7; 32],
                    credit_id: CreditIdV1([1; 32]),
                    envelope_digest: [6; 32],
                    staged_at_ms: 1,
                    predecessor_journal_commitment: [11; 32],
                    successor_journal_commitment: [12; 32],
                    successor_capacity_commitment: [13; 32],
                },
                guard_bundle: vec![1],
            },
        }
    }

    #[test]
    fn empty_mint_journal_has_no_live_capacity_charge_or_unresolved_credit() {
        let journal = KagemushaMintInboxV1::default();
        assert_eq!(journal.capacity_charge_bytes().unwrap(), 0);
        assert_eq!(journal.pending_count(), 0);
        assert!(!journal.has_unresolved_credits());
        assert!(!journal.contains_credit_id(CreditIdV1([1; 32])));
        assert!(journal.pending_values().next().is_none());
    }

    #[test]
    fn empty_mint_journal_roundtrip_keeps_exact_logical_commitment() {
        let journal = KagemushaMintInboxV1::default();
        let bytes = norito::encode_canonical(&journal).unwrap();
        let decoded: KagemushaMintInboxV1 = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(decoded, journal);
        assert_eq!(decoded.commitment().unwrap(), journal.commitment().unwrap());
        assert_ne!(journal.commitment().unwrap(), [0; 32]);
    }

    #[test]
    fn certificate_witness_changes_snapshot_bytes_without_creating_a_journal_hash_cycle() {
        let receipt = historical_receipt();
        let mut journal = KagemushaMintInboxV1::default();
        journal.accepted.insert(receipt.credit_id, receipt);
        let original_commitment = journal.commitment().unwrap();
        let original_bytes = norito::encode_canonical(&journal).unwrap();
        let changed = journal.accepted.get_mut(&CreditIdV1([1; 32])).unwrap();
        changed.stage_certificate.guard_bundle = vec![2; 100];
        changed
            .stage_certificate
            .statement
            .successor_journal_commitment = [14; 32];
        assert_eq!(original_commitment, journal.commitment().unwrap());
        assert_ne!(original_bytes, norito::encode_canonical(&journal).unwrap());
        journal
            .accepted
            .get_mut(&CreditIdV1([1; 32]))
            .unwrap()
            .stage_certificate
            .statement
            .inbox_revision_after = 2;
        assert_ne!(original_commitment, journal.commitment().unwrap());
    }

    #[test]
    fn compact_history_retains_key_and_operation_uniqueness() {
        let receipt = historical_receipt();
        let mut journal = KagemushaMintInboxV1::default();
        journal.accepted.insert(receipt.credit_id, receipt);
        for (operation, key, handle) in [
            ([2; 32], [20; 32], [21; 32]),
            ([20; 32], [3; 32], [21; 32]),
            ([20; 32], [21; 32], [4; 32]),
        ] {
            assert!(
                journal
                    .ensure_unique_operation_key(operation, key, handle)
                    .is_err()
            );
        }
        assert!(
            journal
                .ensure_unique_operation_key([20; 32], [21; 32], [22; 32])
                .is_ok()
        );
        assert!(journal.contains_credit_id(CreditIdV1([1; 32])));
        assert_eq!(journal.pending_count(), 0);
        assert!(!journal.has_unresolved_credits());
    }

    #[test]
    fn compact_history_capacity_is_exact_and_includes_its_original_guard() {
        let receipt = historical_receipt();
        let mut journal = KagemushaMintInboxV1::default();
        journal.accepted.insert(receipt.credit_id, receipt.clone());
        assert_eq!(
            journal.capacity_charge_bytes().unwrap(),
            JOURNAL_MAP_FRAMING_BYTES
                + receiver_sequence_entry_bytes(&(receipt.credit_id, receipt.clone())).unwrap()
        );
        let original = journal.capacity_charge_bytes().unwrap();
        journal
            .accepted
            .get_mut(&receipt.credit_id)
            .unwrap()
            .stage_certificate
            .guard_bundle = vec![1; 256];
        assert!(journal.capacity_charge_bytes().unwrap() > original);
    }

    #[test]
    fn stage_certificate_shape_rejects_skip_placeholder_and_oversized_guard() {
        let certificate = historical_receipt().stage_certificate;
        validate_stage_certificate(&certificate).unwrap();
        let mut skipped = certificate.clone();
        skipped.statement.inbox_revision_after = 2;
        assert!(validate_stage_certificate(&skipped).is_err());
        let mut provisional = certificate.clone();
        provisional.statement.successor_journal_commitment = [0; 32];
        assert!(validate_stage_certificate(&provisional).is_err());
        let mut oversized = certificate;
        oversized.guard_bundle = vec![1; KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 + 1];
        assert!(validate_stage_certificate(&oversized).is_err());
    }
}
