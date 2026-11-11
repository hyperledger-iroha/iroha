//! Native suffix Unload verification (design option a, owner go 2026-11-06).
//!
//! An online Unload may withdraw value whose latest Receives are released but not folded.
//! Instead of first proving the twelve-stage recursive fold of every such Receive, the phone
//! and every validator verify the released suffix natively, one complete semantic step at a
//! time, from the last genuine self-verified Omega (the anchor):
//!
//! - the anchor Omega with its opening and both transported accumulator decides, its exact
//!   state opening and the issuer-authenticated current credential;
//! - for each Receive: the complete Payment (payer Omega with all three decides, `σ_send`,
//!   `τ_send`, the signed Request, payer credential and certificates, historical fee and
//!   policy bindings), the own `σ_recv` and hardware `τ_recv`, the consumed-credit
//!   nonmembership and insertion, the exact successor state through the installed `σ_recv`
//!   evaluator, and the lineage credit-digest preserve-first record;
//! - for each prior Unload in the suffix: its `σ_unload`, its hardware receipt over the exact
//!   released `proof_digest` and the complete native Unload transition, including the
//!   recovery-map insertion and the lineage-adjusted burn and pending synchronization;
//! - for the final Unload: the same native Unload arithmetic from the verified tip, its
//!   `σ_unload` and a hardware receipt whose `proof_digest` is the caller's suffix-authority
//!   digest (the ledger's `P_bytes(kgwsufa1, header frame)`).
//!
//! The unchanged Unload consuming relation later folds that same receipt, because it treats
//! `proof_digest` as an opaque nonzero field; the lineage-adjusted fields computed here are
//! the ones the ordinary fold derives.
//!
//! A cursor is constant size, streams one complete step at a time (each step's originals are
//! bounded by their existing decoders) and is cancellable. A step either verifies completely
//! and advances the cursor or leaves it unchanged; no partial result is acceptance.
//!
//! Two routes reach the only capability, [`VerifiedSuffixUnloadV1`]:
//!
//! - sequential (Native): [`SuffixVerifierV1::verify_anchor`], a cursor at that anchor, every
//!   step, [`SuffixVerificationCursorV1::into_tip`], [`SuffixVerifierV1::finish`];
//! - segmented (ledger stages): Begin verifies the anchor; each Append verifies one segment
//!   from its claimed start ([`SuffixVerifierV1::cursor_at_claim`]) and records the
//!   [`SuffixSegmentV1`] DATA in consensus state; Finalize re-verifies the anchor, links every
//!   committed segment from it ([`SuffixVerifierV1::link`]) and calls `finish`. A segment's
//!   claimed start is authority only after linking proves it equals the previous verified end.
//!
//! Ledger mode ([`SuffixVerifierV1::ledger`]) verifies with the installed verifier set alone.
//! Native mode ([`SuffixVerifierV1::native`]) additionally runs, for every Receive, the exact
//! strict circuit predicate assignments of Receive admission under the qualified source grant
//! and refuses any Payment the later fold would burn.
//!
//! A batched verifier ([`SuffixVerifierV1::batched`]) defers each step's final proof work
//! (every σ verification and every Omega's three generator decisions) and decides it in one
//! deterministic batch at [`SuffixVerificationCursorV1::settle`]; every other check of the
//! step still runs before the step is provisionally accepted. A failed settle discards the
//! whole unsettled run, so a segment or tip is only ever built from settled steps.

use iroha_data_model::kagemusha::kagemusha_wallet_v1::*;
use iroha_pasta::{CancellationToken, msm::MemoryBudget};

use crate::{
    kagemusha_wallet_artifacts_v1::{
        InstalledVerifierPackV1, producer_inventory::QualifiedWalletSourcesV1,
    },
    kagemusha_wallet_preparation_v1::{
        PreparationV1, UnloadChargeOriginalsV1, suffix_receive_successor_v1,
        suffix_unload_successor_v1,
    },
    kagemusha_wallet_proofs_v1::{
        ArtifactSet, Error as ProofError, KagemushaWalletDeferredProofsV1,
    },
    kagemusha_wallet_state_v1::{Error as StateError, suffix_receive_burns},
};

/// Format version of [`SuffixSegmentV1`].
pub const SUFFIX_SEGMENT_VERSION_V1: u16 = 1;
/// `P_bytes` domain of a segment's running ordered step-chain digest.
pub const SUFFIX_UNLOAD_CHAIN_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwsufc1");
/// Largest number of steps one suffix accepts across all its segments. Every cursor is
/// constant size; this bounds counters and the work one suffix can demand.
pub const SUFFIX_UNLOAD_MAX_STEPS_V1: u32 = 4_096;
/// Most steps a batched cursor holds with deferred final proof decisions before it settles
/// them in one batch. This bounds the deferred claims a cursor retains.
pub const SUFFIX_UNLOAD_DEFERRED_STEPS_V1: u32 = 64;

const RECEIVE_STEP_TAG: u8 = 1;
const PRIOR_UNLOAD_STEP_TAG: u8 = 2;

/// Native suffix verification failure. No variant accepts any part of a suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SuffixErrorV1 {
    /// The caller cancelled. This is never a verdict; the cursor is unchanged.
    #[error("suffix verification cancelled")]
    Cancelled,
    /// Local verification could not complete (decode budget, kernel, parameters).
    /// This is never a verdict about the suffix; the cursor is unchanged.
    #[error("suffix verification unavailable: {0}")]
    Unavailable(&'static str),
    /// A count or size bound was exceeded before any expensive work.
    #[error("suffix bound exceeded: {0}")]
    Bound(&'static str),
    /// The exact supplied bytes are invalid; the named check failed. Final for these bytes.
    #[error("suffix rejected: {0}")]
    Rejected(&'static str),
}

impl SuffixErrorV1 {
    /// Whether the caller cancelled rather than receiving a verdict.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Whether this is a final refusal of the supplied bytes (not a local interruption).
    #[must_use]
    pub const fn is_rejection(&self) -> bool {
        matches!(self, Self::Rejected(_) | Self::Bound(_))
    }
}

fn proof_error(context: &'static str) -> impl FnOnce(ProofError) -> SuffixErrorV1 {
    move |error| match error {
        ProofError::Cancelled => SuffixErrorV1::Cancelled,
        ProofError::DecodeResource(_)
        | ProofError::Unavailable
        | ProofError::Entropy
        | ProofError::Verification(_)
        | ProofError::Recursion(_) => SuffixErrorV1::Unavailable(context),
        ProofError::Authority
        | ProofError::RuntimeBinding
        | ProofError::Inventory
        | ProofError::Profile
        | ProofError::Proof
        | ProofError::Blocked(_) => SuffixErrorV1::Rejected(context),
    }
}

fn state_error(context: &'static str) -> impl FnOnce(StateError) -> SuffixErrorV1 {
    move |error| match error {
        StateError::Cancelled => SuffixErrorV1::Cancelled,
        StateError::NativeVerification(original) => proof_error(context)(original),
        StateError::DecodeResource(_)
        | StateError::Storage(_)
        | StateError::ArtifactsUnavailable(_)
        | StateError::FinalityRead(_) => SuffixErrorV1::Unavailable(context),
        _ => SuffixErrorV1::Rejected(context),
    }
}

fn model<T>(
    context: &'static str,
    value: Result<T, KagemushaWalletValidationErrorV1>,
) -> Result<T, SuffixErrorV1> {
    value.map_err(|_| SuffixErrorV1::Rejected(context))
}

fn require(check: bool, context: &'static str) -> Result<(), SuffixErrorV1> {
    if check {
        Ok(())
    } else {
        Err(SuffixErrorV1::Rejected(context))
    }
}

fn checkpoint(cancellation: Option<&CancellationToken>) -> Result<(), SuffixErrorV1> {
    CancellationToken::checkpoint(cancellation).map_err(|_| SuffixErrorV1::Cancelled)
}

fn bounded(bytes: &[u8], maximum: usize, context: &'static str) -> Result<(), SuffixErrorV1> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(SuffixErrorV1::Bound(context));
    }
    Ok(())
}

fn canonical<T>(bytes: &[u8], context: &'static str) -> Result<T, SuffixErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::core::classify_decode_attempt(|| {
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
    })
    .map_err(|error| match error.into_local_refusal() {
        Ok(_) => SuffixErrorV1::Unavailable(context),
        Err(_) => SuffixErrorV1::Rejected(context),
    })
}

fn insertion(
    openings: [&[u8]; 2],
    context: &'static str,
) -> Result<KagemushaWalletIndexedInsertV1, SuffixErrorV1> {
    let (Some(low), low_opening) = model(
        context,
        KagemushaWalletIndexedOpeningV1::from_transcript(openings[0]),
    )?
    else {
        return Err(SuffixErrorV1::Rejected(context));
    };
    let (None, slot_opening) = model(
        context,
        KagemushaWalletIndexedOpeningV1::from_transcript(openings[1]),
    )?
    else {
        return Err(SuffixErrorV1::Rejected(context));
    };
    Ok(KagemushaWalletIndexedInsertV1 {
        low,
        low_opening,
        slot_opening,
    })
}

// `adjusted` opens at `state` under the authenticated credential: the head and every stable
// identity/policy field. Burned, pending and credit roots are verification results.
fn require_opening(
    credential: &SuffixCredentialV1,
    state: &KagemushaWalletStateV1,
    adjusted: &KagemushaWalletLineagePublicV1,
    context: &'static str,
) -> Result<(), SuffixErrorV1> {
    model(
        context,
        state.validate_for_credential(&credential.credential),
    )?;
    model(context, adjusted.validate())?;
    let head = model(context, state.commitment())?;
    require(
        adjusted.head == head
            && adjusted.scheme_id == credential.scheme_id
            && adjusted.wallet_id == state.core.wallet_id
            && adjusted.credential_digest == state.core.credential_digest
            && adjusted.credential_digest == credential.credential.credential_digest()
            && adjusted.payment_key == credential.credential.body.payment_key
            && adjusted.lifecycle == state.core.lifecycle
            && adjusted.policy_epoch == state.core.policy_epoch
            && adjusted.enabled_controls == state.core.enabled_controls,
        context,
    )
}

/// The anchor: the last genuine fully folded head of the wallet.
#[derive(Clone, Copy)]
pub struct SuffixAnchorInputV1<'a> {
    /// The wallet's exact current credential; it stays fixed across the supported suffix.
    pub credential: &'a KagemushaWalletCredentialV1,
    /// Certificate set containing the credential's Enrollment-role issuer certificate.
    pub certificates: &'a KagemushaWalletCertificateSetV1,
    /// The genuine anchor Omega, fully verified here.
    pub lineage: &'a KagemushaWalletLineageV1,
    /// Exact state opening of the anchor head.
    pub state: &'a KagemushaWalletStateV1,
}

/// The lineage credit-digest record of one Receive, against the cursor's adjusted root.
#[derive(Clone, Copy)]
pub enum SuffixCreditWitnessV1<'a> {
    /// The credit identity is already recorded (preserve first): its 1,124-byte leaf opening.
    Present {
        /// Exact occupied leaf-opening transcript.
        membership: &'a [u8],
    },
    /// The credit identity is absent: low-leaf (1,124 B) and empty-slot (1,028 B) openings.
    Inserted {
        /// Exact occupied low-leaf opening transcript.
        low: &'a [u8],
        /// Exact intermediate empty-slot opening transcript.
        empty: &'a [u8],
    },
}

/// One retained Receive of the suffix, exactly as released.
#[derive(Clone, Copy)]
pub struct SuffixReceiveInputV1<'a> {
    /// Exact predecessor state opening; must equal the cursor's current state.
    pub before: &'a KagemushaWalletStateV1,
    /// Exact successor state opening.
    pub after: &'a KagemushaWalletStateV1,
    /// Exact released statement.
    pub statement: &'a KagemushaWalletStatementV1,
    /// The own released `σ_recv`.
    pub sigma: &'a KagemushaWalletStepProofV1,
    /// The own hardware receipt `τ_recv`.
    pub receipt: &'a KagemushaWalletReceiptV1,
    /// Consumed-credit insertion: low-leaf then empty-slot opening transcripts.
    pub consumed: [&'a [u8]; 2],
    /// Lineage credit-digest record.
    pub credit: SuffixCreditWitnessV1<'a>,
    /// Exact canonical retained Request original.
    pub request: &'a [u8],
    /// Exact canonical received Payment original.
    pub payment: &'a [u8],
    /// Exact canonical payer credential original.
    pub payer_credential: &'a [u8],
    /// Exact canonical payer certificate-set original.
    pub payer_certificates: &'a [u8],
}

/// One earlier Unload (ordinary or suffix) inside the suffix, exactly as released.
#[derive(Clone, Copy)]
pub struct SuffixPriorUnloadInputV1<'a> {
    /// Exact predecessor state opening; must equal the cursor's current state.
    pub before: &'a KagemushaWalletStateV1,
    /// Exact successor state opening.
    pub after: &'a KagemushaWalletStateV1,
    /// Exact released statement.
    pub statement: &'a KagemushaWalletStatementV1,
    /// The released `σ_unload`.
    pub sigma: &'a KagemushaWalletStepProofV1,
    /// The released hardware receipt.
    pub receipt: &'a KagemushaWalletReceiptV1,
    /// The exact nonzero `proof_digest` that receipt signs: an earlier suffix-authority
    /// digest (or an ordinary carrier digest). The debit is authorized by that signature.
    pub proof_digest: [u8; 32],
    /// Recovery-map insertion: low-leaf then empty-slot opening transcripts.
    pub recovery: [&'a [u8]; 2],
    /// Exact signed charge originals when its statement names a charge quote.
    pub charge: Option<UnloadChargeOriginalsV1<'a>>,
}

/// One complete semantic step of the suffix, in released order.
#[derive(Clone, Copy)]
pub enum SuffixStepInputV1<'a> {
    /// A retained Receive.
    Receive(SuffixReceiveInputV1<'a>),
    /// An earlier Unload whose debit the current suffix carries.
    PriorUnload(SuffixPriorUnloadInputV1<'a>),
}

/// The final suffix Unload, the claims its header makes and its hardware receipt.
#[derive(Clone, Copy)]
pub struct SuffixUnloadClaimsV1<'a> {
    /// Nonzero suffix-authority digest that the receipt signs as `proof_digest`; the ledger
    /// derives it as `P_bytes(kgwsufa1, canonical header frame)`.
    pub authority_digest: [u8; 32],
    /// Claimed anchor sequence.
    pub anchor_sequence: u128,
    /// Claimed number of steps after the anchor.
    pub step_count: u128,
    /// Claimed tip commitment (the Unload predecessor).
    pub tip: KagemushaWalletStateCommitmentV1,
    /// Claimed tip sequence.
    pub tip_sequence: u128,
    /// Claimed lineage-adjusted fields at the tip.
    pub tip_adjusted: &'a KagemushaWalletLineagePublicV1,
    /// Exact Unload statement.
    pub statement: &'a KagemushaWalletStatementV1,
    /// Exact Unload `σ_unload`.
    pub sigma: &'a KagemushaWalletStepProofV1,
    /// Exact successor state opening; its nonce is the Native-chosen successor nonce.
    pub after: &'a KagemushaWalletStateV1,
    /// Recovery-map insertion: low-leaf then empty-slot opening transcripts.
    pub recovery: [&'a [u8]; 2],
    /// Exact signed charge originals; absent for the default zero charge.
    pub charge: Option<UnloadChargeOriginalsV1<'a>>,
    /// The hardware receipt.
    pub receipt: &'a KagemushaWalletReceiptV1,
}

/// Cheap progress of a cursor: scheduling DATA only, never acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuffixVerificationProgressV1 {
    /// Sequence of the cursor's start (the anchor for a sequential cursor).
    pub start_sequence: u128,
    /// Sequence of the last verified step (the start before any step).
    pub tip_sequence: u128,
    /// Verified steps (provisional while a batched cursor has unsettled ones).
    pub completed_steps: u32,
    /// Steps whose deferred proof decisions are settled (all of them when unbatched).
    pub settled_steps: u32,
    /// Verified Receives among them.
    pub receives: u32,
    /// Verified prior Unloads among them.
    pub prior_unloads: u32,
}

/// The exact Unload derivation from a verified tip, for Native `σ_unload` proving, header
/// construction and review. DATA only: it is not an Omega, a capability or an approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuffixUnloadDerivationV1 {
    /// Opened tip the Unload consumes.
    pub predecessor: KagemushaWalletStateV1,
    /// Lineage-adjusted projection of the tip, as the ordinary fold derives it.
    pub predecessor_adjusted: KagemushaWalletLineagePublicV1,
    /// Exact Unload successor state.
    pub successor: KagemushaWalletStateV1,
    /// Exact Unload statement.
    pub statement: KagemushaWalletStatementV1,
    /// Successor projection with the Unload's head.
    pub successor_adjusted: KagemushaWalletLineagePublicV1,
    /// Recovery-map insertion transcripts, low leaf then empty slot.
    pub openings: Vec<Vec<u8>>,
}

/// One executed segment verification: its claimed start and its verified end.
///
/// This is DATA. It is authority only when read back from consensus state that a successful
/// [`SuffixVerificationCursorV1::segment`] wrote, and only inside [`SuffixVerifierV1::link`],
/// which proves each claimed start equals the previous verified end from a freshly verified
/// anchor. Native never persists it as acceptance.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_suffix_unload_v1::SuffixSegmentV1")]
#[repr(align(16))]
pub struct SuffixSegmentV1 {
    /// Exactly [`SUFFIX_SEGMENT_VERSION_V1`].
    pub version: u16,
    /// Installed scheme.
    pub scheme_id: [u8; 32],
    /// Installed signed artifact manifest.
    pub manifest_digest: [u8; 32],
    /// Current credential digest every step ran under.
    pub credential_digest: [u8; 32],
    /// Claimed start state commitment (the first step's predecessor).
    pub start: KagemushaWalletStateCommitmentV1,
    /// Claimed start sequence.
    pub start_sequence: u128,
    /// Claimed lineage-adjusted fields at the start.
    pub start_adjusted: KagemushaWalletLineagePublicV1,
    /// Verified end state opening.
    pub end: KagemushaWalletStateV1,
    /// Verified lineage-adjusted fields at the end.
    pub end_adjusted: KagemushaWalletLineagePublicV1,
    /// Verified steps.
    pub steps: u32,
    /// Verified Receives among them.
    pub receives: u32,
    /// Verified prior Unloads among them.
    pub prior_unloads: u32,
    /// Ordered step-chain digest of this segment.
    pub chain: [u8; 32],
}

impl SuffixSegmentV1 {
    /// Canonical bounded frame for consensus staging rows.
    ///
    /// # Errors
    /// An encoding failure.
    pub fn to_canonical_bytes(self) -> Result<Vec<u8>, SuffixErrorV1> {
        norito::encode_canonical(&self).map_err(|_| SuffixErrorV1::Rejected("segment encoding"))
    }

    /// Decode one canonical segment frame (at most 8 KiB). Decoding grants nothing.
    ///
    /// # Errors
    /// An oversized, noncanonical or other-version frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SuffixErrorV1> {
        bounded(bytes, 8 << 10, "segment frame")?;
        let segment: Self = canonical(bytes, "segment frame")?;
        require(
            segment.version == SUFFIX_SEGMENT_VERSION_V1,
            "segment version",
        )?;
        Ok(segment)
    }
}

struct NativePredicatesV1<'a> {
    installed: &'a InstalledVerifierPackV1,
    sources: &'a QualifiedWalletSourcesV1,
    preparation: PreparationV1<'a>,
}

/// The shared native suffix verifier over one immutable installed verifier set.
#[allow(
    missing_copy_implementations,
    reason = "a verifier borrows its installation; copies would add nothing"
)]
pub struct SuffixVerifierV1<'a> {
    artifacts: &'a ArtifactSet,
    budget: MemoryBudget,
    native: Option<NativePredicatesV1<'a>>,
    batched: bool,
}

/// An issuer-authenticated current credential under the installed scheme. Private fields;
/// only [`SuffixVerifierV1::authenticate_credential`] constructs it.
#[allow(
    missing_copy_implementations,
    reason = "authenticated identity must be re-derived, not duplicated"
)]
#[derive(Debug, PartialEq, Eq)]
pub struct SuffixCredentialV1 {
    scheme_id: [u8; 32],
    manifest_digest: [u8; 32],
    credential: KagemushaWalletCredentialV1,
    issuer: KagemushaWalletSignerCertificateV1,
}

impl SuffixCredentialV1 {
    /// The authenticated credential.
    #[must_use]
    pub const fn credential(&self) -> &KagemushaWalletCredentialV1 {
        &self.credential
    }
}

/// A fully verified anchor: genuine Omega, exact state opening and authenticated credential.
/// Private fields; only [`SuffixVerifierV1::verify_anchor`] constructs it.
#[allow(
    missing_copy_implementations,
    reason = "a verified anchor must not be duplicated or reconstructed"
)]
pub struct SuffixAnchorPointV1 {
    credential: SuffixCredentialV1,
    sequence: u128,
    lineage_digest: [u8; 32],
    state: KagemushaWalletStateV1,
    adjusted: KagemushaWalletLineagePublicV1,
}

impl SuffixAnchorPointV1 {
    /// Anchor sequence and its `P_bytes(kgwlin_1, Omega)` lineage digest.
    #[must_use]
    pub const fn identity(&self) -> (u128, [u8; 32]) {
        (self.sequence, self.lineage_digest)
    }
    /// Opened anchor state and its genuine Omega public fields (DATA).
    #[must_use]
    pub const fn point(&self) -> (&KagemushaWalletStateV1, &KagemushaWalletLineagePublicV1) {
        (&self.state, &self.adjusted)
    }
    /// The authenticated credential.
    #[must_use]
    pub const fn credential(&self) -> &SuffixCredentialV1 {
        &self.credential
    }
}

/// A verified tip: an anchor followed by completely verified steps, linked without gaps.
/// Private fields; only [`SuffixVerificationCursorV1::into_tip`] and
/// [`SuffixVerifierV1::link`] construct it. Only [`SuffixVerifierV1::finish`] consumes it.
#[allow(
    missing_copy_implementations,
    reason = "a verified tip must not be duplicated or reconstructed"
)]
pub struct SuffixTipV1 {
    credential: SuffixCredentialV1,
    native: bool,
    anchor_sequence: u128,
    anchor_digest: [u8; 32],
    state: KagemushaWalletStateV1,
    adjusted: KagemushaWalletLineagePublicV1,
    steps: u32,
    receives: u32,
    prior_unloads: u32,
}

impl SuffixTipV1 {
    /// Opened tip state and its lineage-adjusted fields (DATA).
    #[must_use]
    pub const fn point(&self) -> (&KagemushaWalletStateV1, &KagemushaWalletLineagePublicV1) {
        (&self.state, &self.adjusted)
    }
    /// Anchor sequence and lineage digest.
    #[must_use]
    pub const fn anchor(&self) -> (u128, [u8; 32]) {
        (self.anchor_sequence, self.anchor_digest)
    }
    /// Verified steps, Receives and prior Unloads after the anchor.
    #[must_use]
    pub const fn steps(&self) -> (u32, u32, u32) {
        (self.steps, self.receives, self.prior_unloads)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Anchor { sequence: u128, digest: [u8; 32] },
    Claimed,
}

/// Private, non-serializable verification cursor over a completely verified run of steps.
/// It has no constructor from progress or projected fields, no codec and no clone: a restart
/// re-verifies from its start.
pub struct SuffixVerificationCursorV1 {
    credential: SuffixCredentialV1,
    native: bool,
    origin: Origin,
    start: KagemushaWalletStateCommitmentV1,
    start_sequence: u128,
    start_adjusted: KagemushaWalletLineagePublicV1,
    state: KagemushaWalletStateV1,
    adjusted: KagemushaWalletLineagePublicV1,
    completed: u32,
    receives: u32,
    prior_unloads: u32,
    chain: [u8; 32],
    deferred: Option<KagemushaWalletDeferredProofsV1>,
    settled: SettledV1,
}

// The cursor as of its last settled step; a failed settle restores it.
#[derive(Clone, Copy)]
struct SettledV1 {
    state: KagemushaWalletStateV1,
    adjusted: KagemushaWalletLineagePublicV1,
    completed: u32,
    receives: u32,
    prior_unloads: u32,
    chain: [u8; 32],
}

struct StepResult {
    state: KagemushaWalletStateV1,
    adjusted: KagemushaWalletLineagePublicV1,
    link: Vec<u8>,
    tag: u8,
}

impl<'a> SuffixVerifierV1<'a> {
    /// Ledger verifier: every proof, signature, map, state and arithmetic check under the
    /// consensus-installed verifier set, with no proving material.
    #[must_use]
    pub const fn ledger(artifacts: &'a ArtifactSet, budget: MemoryBudget) -> Self {
        Self {
            artifacts,
            budget,
            native: None,
            batched: false,
        }
    }

    /// Defer every step's final σ verification and generator decisions into one batch per
    /// settle (deterministic weights over every item). Every other check is unchanged.
    #[must_use]
    pub fn batched(self) -> Self {
        Self {
            batched: true,
            ..self
        }
    }

    /// Native (phone) verifier: the ledger checks plus Receive admission's strict circuit
    /// predicate assignments under the wallet's complete qualified source grant.
    ///
    /// # Errors
    /// The source grant belongs to another installation.
    pub fn native(
        installed: &'a InstalledVerifierPackV1,
        sources: &'a QualifiedWalletSourcesV1,
        budget: MemoryBudget,
    ) -> Result<Self, SuffixErrorV1> {
        let verifier = installed.verifier();
        require(
            sources.installation() == (verifier.scheme().scheme_id(), verifier.manifest_digest()),
            "native source installation",
        )?;
        let preparation =
            PreparationV1::new(installed).map_err(proof_error("native preparation owner"))?;
        Ok(Self {
            artifacts: verifier,
            budget,
            native: Some(NativePredicatesV1 {
                installed,
                sources,
                preparation,
            }),
            batched: false,
        })
    }

    /// Authenticate the current credential and its Enrollment issuer under the installed
    /// scheme. Cheap: signatures only.
    ///
    /// # Errors
    /// Another scheme, an invalid certificate set, issuer or credential signature.
    pub fn authenticate_credential(
        &self,
        credential: &KagemushaWalletCredentialV1,
        certificates: &KagemushaWalletCertificateSetV1,
    ) -> Result<SuffixCredentialV1, SuffixErrorV1> {
        let scheme = self.artifacts.scheme();
        model("credential", credential.validate())?;
        require(
            credential.body.scheme_id == scheme.scheme_id(),
            "credential scheme",
        )?;
        model("credential certificates", certificates.verify(scheme))?;
        let issuer = model(
            "credential issuer certificate",
            certificates.certificate(
                &credential.body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            ),
        )?;
        model("credential issuer", credential.verify(scheme, issuer))?;
        Ok(SuffixCredentialV1 {
            scheme_id: scheme.scheme_id(),
            manifest_digest: self.artifacts.manifest_digest(),
            credential: *credential,
            issuer: *issuer,
        })
    }

    fn require_credential(&self, credential: &SuffixCredentialV1) -> Result<(), SuffixErrorV1> {
        require(
            credential.scheme_id == self.artifacts.scheme().scheme_id()
                && credential.manifest_digest == self.artifacts.manifest_digest(),
            "credential installation",
        )
    }

    /// Fully verify the anchor: credential, Omega with all three decides, and its opening.
    ///
    /// # Errors
    /// Another scheme, an unauthenticated credential, a failed anchor Omega or decide, a
    /// state that does not open the anchor head, cancellation or local unavailability.
    pub fn verify_anchor(
        &self,
        anchor: SuffixAnchorInputV1<'_>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<SuffixAnchorPointV1, SuffixErrorV1> {
        self.anchor_point(anchor, cancellation, true)
    }

    fn anchor_point(
        &self,
        anchor: SuffixAnchorInputV1<'_>,
        cancellation: Option<&CancellationToken>,
        verify_omega: bool,
    ) -> Result<SuffixAnchorPointV1, SuffixErrorV1> {
        checkpoint(cancellation)?;
        let scheme = self.artifacts.scheme();
        let credential = self.authenticate_credential(anchor.credential, anchor.certificates)?;
        let public = anchor.lineage.public;
        model("anchor lineage", anchor.lineage.validate())?;
        require(
            public.scheme_id == scheme.scheme_id() && public.relation_id == scheme.relation_id,
            "anchor lineage scheme",
        )?;
        let state = *anchor.state;
        model(
            "anchor state credential",
            state.validate_for_credential(&credential.credential),
        )?;
        let head = model("anchor state commitment", state.commitment())?;
        require(
            public.head == head
                && public.wallet_id == state.core.wallet_id
                && public.credential_digest == state.core.credential_digest
                && public.credential_digest == credential.credential.credential_digest()
                && public.payment_key == credential.credential.body.payment_key
                && public.lifecycle == state.core.lifecycle
                && public.policy_epoch == state.core.policy_epoch
                && public.enabled_controls == state.core.enabled_controls,
            "anchor opening",
        )?;
        model("anchor spendable", state.spendable_with(&public))?;
        if verify_omega {
            self.artifacts
                .verify_lineage_cancellable(anchor.lineage, self.budget, cancellation)
                .map_err(proof_error("anchor Omega"))?;
        }
        checkpoint(cancellation)?;
        Ok(SuffixAnchorPointV1 {
            credential,
            sequence: state.core.sequence,
            lineage_digest: anchor.lineage.lineage_digest(),
            state,
            adjusted: public,
        })
    }

    /// Restore the anchor of a committed Begin without re-verifying its Omega: the
    /// credential is re-authenticated and the state opening is re-checked, but the Omega
    /// proof and decides are not repeated.
    ///
    /// Ledger only. This is the consensus trust boundary: call it solely with the exact
    /// anchor frame of a Begin row that only a successful [`Self::verify_anchor`] wrote.
    /// Native and every SDK path must use [`Self::verify_anchor`].
    ///
    /// # Errors
    /// What [`Self::verify_anchor`] rejects before its Omega verification.
    pub fn committed_anchor(
        &self,
        anchor: SuffixAnchorInputV1<'_>,
    ) -> Result<SuffixAnchorPointV1, SuffixErrorV1> {
        require(self.native.is_none(), "committed anchor is ledger-only")?;
        self.anchor_point(anchor, None, false)
    }

    /// Verify an Unload `σ` against its exact statement under the installed Unload key
    /// (ledger Begin's early check; [`Self::finish`] verifies it again in full).
    ///
    /// # Errors
    /// A statement of another scheme or kind, a wrong-length or failed proof, cancellation
    /// or local unavailability.
    pub fn verify_unload_sigma(
        &self,
        statement: &KagemushaWalletStatementV1,
        sigma: &KagemushaWalletStepProofV1,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), SuffixErrorV1> {
        require(
            statement.effect.kind() == KagemushaWalletOperationKindV1::Unload,
            "Unload kind",
        )?;
        self.artifacts
            .verify_step_proof_cancellable(
                statement,
                sigma,
                KagemushaWalletOperationKindV1::Unload,
                0,
                self.budget,
                cancellation,
            )
            .map_err(proof_error("Unload sigma"))
    }

    /// A sequential cursor at a verified anchor (Native preparation).
    ///
    /// # Errors
    /// An anchor of another installation.
    pub fn cursor_at_anchor(
        &self,
        anchor: &SuffixAnchorPointV1,
    ) -> Result<SuffixVerificationCursorV1, SuffixErrorV1> {
        self.require_credential(&anchor.credential)?;
        Ok(SuffixVerificationCursorV1 {
            credential: anchor.credential.copy_authenticated(),
            native: self.native.is_some(),
            origin: Origin::Anchor {
                sequence: anchor.sequence,
                digest: anchor.lineage_digest,
            },
            start: anchor.adjusted.head,
            start_sequence: anchor.sequence,
            start_adjusted: anchor.adjusted,
            state: anchor.state,
            adjusted: anchor.adjusted,
            completed: 0,
            receives: 0,
            prior_unloads: 0,
            chain: [0; 32],
            deferred: self.batched.then(|| self.artifacts.deferred_proofs()),
            settled: SettledV1 {
                state: anchor.state,
                adjusted: anchor.adjusted,
                completed: 0,
                receives: 0,
                prior_unloads: 0,
                chain: [0; 32],
            },
        })
    }

    /// A segment cursor at a claimed start (ledger Append). Its verified segment is authority
    /// only after [`Self::link`] proves the claimed start.
    ///
    /// # Errors
    /// A credential of another installation, or a start that is not an opening of its
    /// claimed head under that credential.
    pub fn cursor_at_claim(
        &self,
        credential: &SuffixCredentialV1,
        start: &KagemushaWalletStateV1,
        start_adjusted: &KagemushaWalletLineagePublicV1,
    ) -> Result<SuffixVerificationCursorV1, SuffixErrorV1> {
        self.require_credential(credential)?;
        model(
            "claimed start credential",
            start.validate_for_credential(&credential.credential),
        )?;
        require_opening(credential, start, start_adjusted, "claimed start")?;
        let head = start_adjusted.head;
        Ok(SuffixVerificationCursorV1 {
            credential: credential.copy_authenticated(),
            native: self.native.is_some(),
            origin: Origin::Claimed,
            start: head,
            start_sequence: start.core.sequence,
            start_adjusted: *start_adjusted,
            state: *start,
            adjusted: *start_adjusted,
            completed: 0,
            receives: 0,
            prior_unloads: 0,
            chain: [0; 32],
            deferred: self.batched.then(|| self.artifacts.deferred_proofs()),
            settled: SettledV1 {
                state: *start,
                adjusted: *start_adjusted,
                completed: 0,
                receives: 0,
                prior_unloads: 0,
                chain: [0; 32],
            },
        })
    }

    /// Link committed segments, in order, from an authenticated anchor into a verified
    /// tip. Every segment must be of this installation and credential, its claimed start
    /// must equal the anchor or the previous verified end exactly, and the total stays
    /// within [`SUFFIX_UNLOAD_MAX_STEPS_V1`].
    ///
    /// Ledger only: every segment must come from an authenticated consensus row written
    /// after a successful `verify_step` and `settle`. A decoded segment alone is DATA and
    /// must never enter this boundary from a request. Native verifies its exact originals
    /// sequentially and obtains its tip through [`SuffixVerificationCursorV1::into_tip`].
    ///
    /// # Errors
    /// An empty list, another installation or credential, a gap, overlap or reorder, a
    /// changed adjusted field, or a step-count overflow.
    pub fn link(
        &self,
        anchor: &SuffixAnchorPointV1,
        segments: &[SuffixSegmentV1],
    ) -> Result<SuffixTipV1, SuffixErrorV1> {
        require(self.native.is_none(), "committed segments are ledger-only")?;
        self.require_credential(&anchor.credential)?;
        require(!segments.is_empty(), "no suffix segment")?;
        let credential_digest = anchor.credential.credential.credential_digest();
        let mut state = anchor.state;
        let mut adjusted = anchor.adjusted;
        let (mut steps, mut receives, mut prior_unloads) = (0_u32, 0_u32, 0_u32);
        for segment in segments {
            require(
                segment.version == SUFFIX_SEGMENT_VERSION_V1
                    && segment.scheme_id == anchor.credential.scheme_id
                    && segment.manifest_digest == anchor.credential.manifest_digest
                    && segment.credential_digest == credential_digest,
                "segment installation",
            )?;
            let previous = model("segment predecessor", state.commitment())?;
            require(
                segment.start == previous
                    && segment.start_sequence == state.core.sequence
                    && segment.start_adjusted == adjusted,
                "segment start",
            )?;
            require(
                segment.steps > 0
                    && segment.receives.checked_add(segment.prior_unloads) == Some(segment.steps)
                    && segment.end.core.sequence
                        == segment
                            .start_sequence
                            .checked_add(u128::from(segment.steps))
                            .ok_or(SuffixErrorV1::Bound("segment sequence"))?,
                "segment counts",
            )?;
            steps = steps
                .checked_add(segment.steps)
                .filter(|total| *total <= SUFFIX_UNLOAD_MAX_STEPS_V1)
                .ok_or(SuffixErrorV1::Bound("suffix step count"))?;
            receives += segment.receives;
            prior_unloads += segment.prior_unloads;
            require_opening(
                &anchor.credential,
                &segment.end,
                &segment.end_adjusted,
                "segment end",
            )?;
            state = segment.end;
            adjusted = segment.end_adjusted;
        }
        Ok(SuffixTipV1 {
            credential: anchor.credential.copy_authenticated(),
            native: self.native.is_some(),
            anchor_sequence: anchor.sequence,
            anchor_digest: anchor.lineage_digest,
            state,
            adjusted,
            steps,
            receives,
            prior_unloads,
        })
    }

    /// Derive the final Unload from a verified tip: the exact successor and statement the
    /// phone proves `σ_unload` for and its header names. DATA only.
    ///
    /// # Errors
    /// What the native Unload owner rejects for these terms, or a tip without a step.
    pub fn derive_unload(
        &self,
        tip: &SuffixTipV1,
        amount: u128,
        recovery: &KagemushaWalletIndexedInsertV1,
        charge: Option<UnloadChargeOriginalsV1<'_>>,
        nonce: [u8; 32],
    ) -> Result<SuffixUnloadDerivationV1, SuffixErrorV1> {
        self.require_credential(&tip.credential)?;
        require(tip.native == self.native.is_some(), "tip verifier mode")?;
        require(tip.steps > 0, "no unfolded suffix step")?;
        let derived = suffix_unload_successor_v1(
            self.artifacts.scheme(),
            &tip.credential.credential,
            &tip.state,
            &tip.adjusted,
            amount,
            recovery,
            charge,
            nonce,
        )
        .map_err(proof_error("Unload transition"))?;
        Ok(SuffixUnloadDerivationV1 {
            predecessor: tip.state,
            predecessor_adjusted: tip.adjusted,
            successor: derived.state,
            statement: derived.statement,
            successor_adjusted: derived.projection,
            openings: derived.openings,
        })
    }

    /// Verify the final Unload against a verified tip and the header's claims, its
    /// `σ_unload` and its hardware receipt over the authority digest; only then construct the
    /// opaque verified capability. Consumes the tip.
    ///
    /// # Errors
    /// A claim that differs from the verified tip, terms that do not derive exactly from it,
    /// a failed `σ_unload`, a receipt that does not sign this authority digest, cancellation
    /// or local unavailability.
    pub fn finish(
        &self,
        tip: SuffixTipV1,
        claims: SuffixUnloadClaimsV1<'_>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<VerifiedSuffixUnloadV1, SuffixErrorV1> {
        checkpoint(cancellation)?;
        let commitment = model("tip commitment", tip.state.commitment())?;
        require(
            claims.anchor_sequence == tip.anchor_sequence
                && claims.step_count == u128::from(tip.steps)
                && claims.tip == commitment
                && claims.tip_sequence == tip.state.core.sequence
                && *claims.tip_adjusted == tip.adjusted,
            "header tip claims",
        )?;
        require(
            claims.authority_digest != [0; 32],
            "zero suffix authority digest",
        )?;
        let KagemushaWalletEffectV1::Unload { amount, .. } = claims.statement.effect else {
            return Err(SuffixErrorV1::Rejected("Unload kind"));
        };
        let recovery = insertion(claims.recovery, "Unload recovery insertion")?;
        let derivation = self.derive_unload(
            &tip,
            amount,
            &recovery,
            claims.charge,
            claims.after.core.state_nonce,
        )?;
        require(
            derivation.successor == *claims.after,
            "Unload successor state",
        )?;
        require(
            derivation.statement == *claims.statement,
            "Unload statement",
        )?;
        require(
            derivation.openings.len() == 2
                && derivation.openings[0] == claims.recovery[0]
                && derivation.openings[1] == claims.recovery[1],
            "Unload recovery openings",
        )?;
        self.artifacts
            .verify_step_proof_cancellable(
                &derivation.statement,
                claims.sigma,
                KagemushaWalletOperationKindV1::Unload,
                0,
                self.budget,
                cancellation,
            )
            .map_err(proof_error("Unload sigma"))?;
        let signer = model(
            "Unload signer",
            KagemushaWalletReceiptSignerV1::from_credential(&tip.credential.credential),
        )?;
        let receipt_digest = model(
            "Unload receipt",
            claims
                .receipt
                .verify(&signer, &derivation.statement, &claims.authority_digest),
        )?;
        let statement_digest = model(
            "Unload statement digest",
            derivation.statement.statement_digest(),
        )?;
        let KagemushaWalletEffectV1::Unload {
            nullifier,
            redeem_ordinal,
            amount,
            online_charge,
            charge_quote,
        } = derivation.statement.effect
        else {
            return Err(SuffixErrorV1::Rejected("Unload effect"));
        };
        checkpoint(cancellation)?;
        Ok(VerifiedSuffixUnloadV1 {
            credential: tip.credential,
            authority_digest: claims.authority_digest,
            receipt: *claims.receipt,
            receipt_digest,
            anchor_sequence: tip.anchor_sequence,
            anchor_digest: tip.anchor_digest,
            steps: tip.steps,
            receives: tip.receives,
            tip_state: tip.state,
            tip_adjusted: tip.adjusted,
            successor: derivation.successor,
            successor_adjusted: derivation.successor_adjusted,
            statement: derivation.statement,
            statement_digest,
            sigma: claims.sigma.clone(),
            nullifier,
            redeem_ordinal,
            amount,
            online_charge,
            charge_quote,
        })
    }
}

impl SuffixCredentialV1 {
    // Internal duplication for a cursor or tip of the same verification only.
    const fn copy_authenticated(&self) -> Self {
        Self {
            scheme_id: self.scheme_id,
            manifest_digest: self.manifest_digest,
            credential: self.credential,
            issuer: self.issuer,
        }
    }
}

impl SuffixVerificationCursorV1 {
    /// Cheap progress DATA.
    #[must_use]
    pub const fn progress(&self) -> SuffixVerificationProgressV1 {
        SuffixVerificationProgressV1 {
            start_sequence: self.start_sequence,
            tip_sequence: self.state.core.sequence,
            completed_steps: self.completed,
            settled_steps: self.settled.completed,
            receives: self.receives,
            prior_unloads: self.prior_unloads,
        }
    }

    /// Opened state and lineage-adjusted fields of the last verified step (DATA). A chunk's
    /// claimed start is the previous chunk's end exactly.
    #[must_use]
    pub const fn point(&self) -> (&KagemushaWalletStateV1, &KagemushaWalletLineagePublicV1) {
        (&self.state, &self.adjusted)
    }

    /// Verify one complete step and advance. On any error the cursor is unchanged, except
    /// that a batched cursor reaching [`SUFFIX_UNLOAD_DEFERRED_STEPS_V1`] unsettled steps
    /// settles them, and a failed settle returns it to its last settled step.
    ///
    /// # Errors
    /// Any failed check of the step (a final rejection of these bytes), a step bound,
    /// another verifier mode, a failed automatic settle, cancellation or unavailability.
    pub fn verify_step(
        &mut self,
        verifier: &SuffixVerifierV1<'_>,
        step: SuffixStepInputV1<'_>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), SuffixErrorV1> {
        checkpoint(cancellation)?;
        verifier.require_credential(&self.credential)?;
        require(
            self.native == verifier.native.is_some(),
            "cursor verifier mode",
        )?;
        if self.completed >= SUFFIX_UNLOAD_MAX_STEPS_V1 {
            return Err(SuffixErrorV1::Bound("suffix step count"));
        }
        require(
            self.deferred.is_some() == verifier.batched,
            "cursor batching mode",
        )?;
        let mut staged = verifier
            .batched
            .then(|| verifier.artifacts.deferred_proofs());
        let result = match step {
            SuffixStepInputV1::Receive(receive) => {
                require(*receive.before == self.state, "Receive predecessor")?;
                self.verify_receive(verifier, receive, staged.as_mut(), cancellation)?
            }
            SuffixStepInputV1::PriorUnload(unload) => {
                require(*unload.before == self.state, "prior Unload predecessor")?;
                self.verify_prior_unload(verifier, unload, staged.as_mut(), cancellation)?
            }
        };
        checkpoint(cancellation)?;
        if let (Some(pending), Some(staged)) = (self.deferred.as_mut(), staged) {
            pending
                .append(staged)
                .map_err(proof_error("deferred proofs"))?;
        }
        // Commit point: every non-deferred check of this step has passed.
        let mut link = Vec::with_capacity(32 + 1 + result.link.len());
        link.extend_from_slice(&self.chain);
        link.push(result.tag);
        link.extend_from_slice(&result.link);
        self.chain = kagemusha_wallet_poseidon_bytes_v1(SUFFIX_UNLOAD_CHAIN_DOMAIN_V1, &link);
        self.state = result.state;
        self.adjusted = result.adjusted;
        self.completed += 1;
        if result.tag == RECEIVE_STEP_TAG {
            self.receives += 1;
        } else {
            self.prior_unloads += 1;
        }
        if self.deferred.is_none() {
            self.settled = self.snapshot();
        } else if self.completed - self.settled.completed >= SUFFIX_UNLOAD_DEFERRED_STEPS_V1 {
            self.settle(verifier, cancellation)?;
        }
        Ok(())
    }

    const fn snapshot(&self) -> SettledV1 {
        SettledV1 {
            state: self.state,
            adjusted: self.adjusted,
            completed: self.completed,
            receives: self.receives,
            prior_unloads: self.prior_unloads,
            chain: self.chain,
        }
    }

    /// Decide every deferred proof of the unsettled steps in one batch. On success they are
    /// settled; on any failure (including cancellation) the whole unsettled run is discarded
    /// and the cursor returns to its last settled step. A no-op when unbatched.
    ///
    /// # Errors
    /// A failed batched decision (a final rejection of the unsettled run), cancellation or
    /// local unavailability.
    pub fn settle(
        &mut self,
        verifier: &SuffixVerifierV1<'_>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), SuffixErrorV1> {
        verifier.require_credential(&self.credential)?;
        require(
            self.native == verifier.native.is_some(),
            "cursor verifier mode",
        )?;
        require(
            self.deferred.is_some() == verifier.batched,
            "cursor batching mode",
        )?;
        let Some(pending) = self.deferred.take() else {
            return Ok(());
        };
        self.deferred = Some(verifier.artifacts.deferred_proofs());
        let decided = if pending.is_empty() {
            Ok(())
        } else {
            verifier
                .artifacts
                .decide_deferred(pending, verifier.budget, cancellation)
                .map_err(proof_error("batched proof decisions"))
        };
        match decided {
            Ok(()) => {
                self.settled = self.snapshot();
                Ok(())
            }
            Err(error) => {
                let settled = self.settled;
                self.state = settled.state;
                self.adjusted = settled.adjusted;
                self.completed = settled.completed;
                self.receives = settled.receives;
                self.prior_unloads = settled.prior_unloads;
                self.chain = settled.chain;
                Err(error)
            }
        }
    }

    fn require_settled(&self) -> Result<(), SuffixErrorV1> {
        require(
            self.settled.completed == self.completed
                && self
                    .deferred
                    .as_ref()
                    .is_none_or(KagemushaWalletDeferredProofsV1::is_empty),
            "unsettled steps",
        )
    }

    #[allow(clippy::too_many_lines)]
    fn verify_receive(
        &self,
        verifier: &SuffixVerifierV1<'_>,
        step: SuffixReceiveInputV1<'_>,
        mut deferred: Option<&mut KagemushaWalletDeferredProofsV1>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<StepResult, SuffixErrorV1> {
        let scheme = verifier.artifacts.scheme();
        let scheme_id = scheme.scheme_id();
        let credential = &self.credential.credential;
        bounded(
            step.request,
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            "Receive Request original",
        )?;
        bounded(
            step.payment,
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            "Receive Payment original",
        )?;
        bounded(
            step.payer_credential,
            KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
            "Receive payer credential original",
        )?;
        bounded(
            step.payer_certificates,
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            "Receive payer certificates original",
        )?;
        require(
            step.statement.effect.kind() == KagemushaWalletOperationKindV1::Receive,
            "Receive kind",
        )?;
        let request: KagemushaWalletRequestV1 = canonical(step.request, "Receive Request")?;
        model("Receive Request", request.verify(scheme))?;
        let payment = model(
            "Receive Payment",
            KagemushaWalletPaymentV1::decode_canonical(step.payment, &scheme_id),
        )?;
        let payer = model(
            "Receive payer credential",
            KagemushaWalletCredentialV1::decode_canonical(step.payer_credential, &scheme_id),
        )?;
        let payer_certificates: KagemushaWalletCertificateSetV1 =
            canonical(step.payer_certificates, "Receive payer certificates")?;
        // Complete Payment verification against the historical signed Request, payer
        // credential, certificates and fee controls, then the stable receiver identity.
        let prepared = model(
            "Receive Payment verification",
            payment.prepare_receive(
                scheme,
                &payer,
                &payer_certificates,
                &request,
                credential,
                &self.state,
                None,
            ),
        )?;
        checkpoint(cancellation)?;
        // Payer sigma, payer Omega opening and both transported accumulators.
        match deferred.as_deref_mut() {
            Some(deferred) => verifier.artifacts.defer_package_proofs_cancellable(
                &payment.send,
                deferred,
                verifier.budget,
                cancellation,
            ),
            None => verifier.artifacts.verify_package_proofs_cancellable(
                &payment.send,
                verifier.budget,
                cancellation,
            ),
        }
        .map_err(proof_error("Receive payer proofs"))?;
        checkpoint(cancellation)?;
        // The exact successor through the installed sigma_recv evaluator.
        let consumed = insertion(step.consumed, "Receive consumed-credit insertion")?;
        let (state, statement) = suffix_receive_successor_v1(
            scheme,
            credential,
            &self.state,
            step.request,
            &consumed,
            step.after.core.state_nonce,
            prepared.effect,
        )
        .map_err(proof_error("Receive transition"))?;
        require(state == *step.after, "Receive successor state")?;
        require(statement == *step.statement, "Receive statement")?;
        // The own package: the ordinary Receive proof digest, the hardware receipt over the
        // exact Payment digest, and the own sigma under the installed key.
        let package = KagemushaWalletPackageV1::new(
            statement,
            KagemushaWalletLineageSlotV1::None,
            step.sigma.clone(),
            *step.receipt,
        );
        let digests = model("Receive own receipt", package.verify(credential))?;
        require(
            step.receipt.payment_digest == prepared.digests.payment,
            "Receive receipt Payment digest",
        )?;
        match deferred.as_deref_mut() {
            Some(deferred) => verifier.artifacts.defer_package_proofs_cancellable(
                &package,
                deferred,
                verifier.budget,
                cancellation,
            ),
            None => verifier.artifacts.verify_package_proofs_cancellable(
                &package,
                verifier.budget,
                cancellation,
            ),
        }
        .map_err(proof_error("Receive own sigma"))?;
        checkpoint(cancellation)?;
        // Lineage credit-digest record from the adjusted root: preserve first or insert.
        let KagemushaWalletEffectV1::Receive { credit_id, .. } = statement.effect else {
            return Err(SuffixErrorV1::Rejected("Receive effect"));
        };
        let credit = KagemushaWalletCreditDigestLeafV1 {
            credit_id,
            payment_digest: prepared.digests.payment,
            burned: false,
        };
        let record = match step.credit {
            SuffixCreditWitnessV1::Present { membership } => {
                let (Some(leaf), opening) = model(
                    "Receive credit membership",
                    KagemushaWalletIndexedOpeningV1::from_transcript(membership),
                )?
                else {
                    return Err(SuffixErrorV1::Rejected("Receive credit membership"));
                };
                KagemushaWalletCreditDigestRecordV1::Present { leaf, opening }
            }
            SuffixCreditWitnessV1::Inserted { low, empty } => {
                KagemushaWalletCreditDigestRecordV1::Inserted {
                    witness: insertion([low, empty], "Receive credit insertion")?,
                }
            }
        };
        let credit_root = model(
            "Receive credit record",
            record.verify(&self.adjusted.credit_digest_root, &credit),
        )?;
        // Native only: the exact strict circuit predicates of Receive admission.
        if let Some(native) = &verifier.native {
            let preparation = &native.preparation;
            let owner = preparation
                .authenticate_credential(
                    &model("Receive predicates", credential.to_canonical_bytes())?,
                    &model(
                        "Receive predicates",
                        self.credential.issuer.to_canonical_bytes(),
                    )?,
                )
                .map_err(proof_error("Receive predicates"))?;
            let fields = preparation
                .suffix_receive_admission_fields(
                    &owner,
                    &self.state,
                    &state,
                    &statement,
                    step.request,
                    step.payment,
                    step.payer_credential,
                    &consumed,
                )
                .map_err(proof_error("Receive predicate tapes"))?;
            let burns = suffix_receive_burns(
                native.installed,
                native.sources,
                verifier.budget,
                fields,
                cancellation,
            )
            .map_err(state_error("Receive predicates"))?;
            require(!burns, "Receive strict predicates would burn")?;
        }
        let mut adjusted = self.adjusted;
        adjusted.head = statement.successor;
        adjusted.credit_digest_root = credit_root;
        adjusted.lifecycle = state.core.lifecycle;
        adjusted.policy_epoch = state.core.policy_epoch;
        adjusted.enabled_controls = state.core.enabled_controls;
        model("Receive adjusted fields", adjusted.validate())?;
        let mut link = Vec::with_capacity(5 * 32);
        link.extend_from_slice(&digests.statement);
        link.extend_from_slice(&digests.receipt);
        link.extend_from_slice(&prepared.digests.payment);
        link.extend_from_slice(&statement.successor.value);
        link.extend_from_slice(&credit_root);
        Ok(StepResult {
            state,
            adjusted,
            link,
            tag: RECEIVE_STEP_TAG,
        })
    }

    fn verify_prior_unload(
        &self,
        verifier: &SuffixVerifierV1<'_>,
        step: SuffixPriorUnloadInputV1<'_>,
        deferred: Option<&mut KagemushaWalletDeferredProofsV1>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<StepResult, SuffixErrorV1> {
        let credential = &self.credential.credential;
        let KagemushaWalletEffectV1::Unload { amount, .. } = step.statement.effect else {
            return Err(SuffixErrorV1::Rejected("prior Unload kind"));
        };
        let recovery = insertion(step.recovery, "prior Unload recovery insertion")?;
        let derived = suffix_unload_successor_v1(
            verifier.artifacts.scheme(),
            credential,
            &self.state,
            &self.adjusted,
            amount,
            &recovery,
            step.charge,
            step.after.core.state_nonce,
        )
        .map_err(proof_error("prior Unload transition"))?;
        require(derived.state == *step.after, "prior Unload successor state")?;
        require(
            derived.statement == *step.statement,
            "prior Unload statement",
        )?;
        // The hardware receipt over the exact released proof digest authorizes the debit.
        let signer = model(
            "prior Unload signer",
            KagemushaWalletReceiptSignerV1::from_credential(credential),
        )?;
        let receipt = model(
            "prior Unload receipt",
            step.receipt
                .verify(&signer, &derived.statement, &step.proof_digest),
        )?;
        match deferred {
            Some(deferred) => verifier.artifacts.defer_step_proof(
                &derived.statement,
                step.sigma,
                KagemushaWalletOperationKindV1::Unload,
                0,
                deferred,
            ),
            None => verifier.artifacts.verify_step_proof_cancellable(
                &derived.statement,
                step.sigma,
                KagemushaWalletOperationKindV1::Unload,
                0,
                verifier.budget,
                cancellation,
            ),
        }
        .map_err(proof_error("prior Unload sigma"))?;
        let mut adjusted = derived.projection;
        adjusted.policy_epoch = derived.state.core.policy_epoch;
        adjusted.enabled_controls = derived.state.core.enabled_controls;
        model("prior Unload adjusted fields", adjusted.validate())?;
        let statement_digest = model(
            "prior Unload statement digest",
            derived.statement.statement_digest(),
        )?;
        let mut link = Vec::with_capacity(4 * 32);
        link.extend_from_slice(&statement_digest);
        link.extend_from_slice(&receipt);
        link.extend_from_slice(&step.proof_digest);
        link.extend_from_slice(&derived.statement.successor.value);
        Ok(StepResult {
            state: derived.state,
            adjusted,
            link,
            tag: PRIOR_UNLOAD_STEP_TAG,
        })
    }

    /// The verified segment DATA of this cursor, for a consensus staging row.
    ///
    /// # Errors
    /// A cursor without a verified step, or with unsettled steps.
    pub fn segment(&self) -> Result<SuffixSegmentV1, SuffixErrorV1> {
        self.require_settled()?;
        require(self.completed > 0, "empty segment")?;
        Ok(SuffixSegmentV1 {
            version: SUFFIX_SEGMENT_VERSION_V1,
            scheme_id: self.credential.scheme_id,
            manifest_digest: self.credential.manifest_digest,
            credential_digest: self.credential.credential.credential_digest(),
            start: self.start,
            start_sequence: self.start_sequence,
            start_adjusted: self.start_adjusted,
            end: self.state,
            end_adjusted: self.adjusted,
            steps: self.completed,
            receives: self.receives,
            prior_unloads: self.prior_unloads,
            chain: self.chain,
        })
    }

    /// The verified tip of a sequential cursor that began at a verified anchor.
    ///
    /// # Errors
    /// A cursor at a claimed start, no verified step, or unsettled steps.
    pub fn into_tip(self) -> Result<SuffixTipV1, SuffixErrorV1> {
        self.require_settled()?;
        let Origin::Anchor { sequence, digest } = self.origin else {
            return Err(SuffixErrorV1::Rejected("claimed start is not an anchor"));
        };
        require(self.completed > 0, "no unfolded suffix step")?;
        Ok(SuffixTipV1 {
            credential: self.credential,
            native: self.native,
            anchor_sequence: sequence,
            anchor_digest: digest,
            state: self.state,
            adjusted: self.adjusted,
            steps: self.completed,
            receives: self.receives,
            prior_unloads: self.prior_unloads,
        })
    }
}

/// The opaque result of complete suffix verification. Only [`SuffixVerifierV1::finish`]
/// constructs it; it has no codec, clone or public constructor. It exposes the payout and
/// continuation fields for the ledger and Native.
pub struct VerifiedSuffixUnloadV1 {
    credential: SuffixCredentialV1,
    authority_digest: [u8; 32],
    receipt: KagemushaWalletReceiptV1,
    receipt_digest: [u8; 32],
    anchor_sequence: u128,
    anchor_digest: [u8; 32],
    steps: u32,
    receives: u32,
    tip_state: KagemushaWalletStateV1,
    tip_adjusted: KagemushaWalletLineagePublicV1,
    successor: KagemushaWalletStateV1,
    successor_adjusted: KagemushaWalletLineagePublicV1,
    statement: KagemushaWalletStatementV1,
    statement_digest: [u8; 32],
    sigma: KagemushaWalletStepProofV1,
    nullifier: [u8; 32],
    redeem_ordinal: u128,
    amount: u128,
    online_charge: u128,
    charge_quote: [u8; 32],
}

impl VerifiedSuffixUnloadV1 {
    /// Installed scheme and signed artifact-manifest identities.
    #[must_use]
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        (self.credential.scheme_id, self.credential.manifest_digest)
    }
    /// The authenticated current credential (wallet, account digest, payment key).
    #[must_use]
    pub const fn credential(&self) -> &KagemushaWalletCredentialV1 {
        &self.credential.credential
    }
    /// The suffix-authority digest the receipt signs as `proof_digest`.
    #[must_use]
    pub const fn authority_digest(&self) -> [u8; 32] {
        self.authority_digest
    }
    /// The exact original hardware receipt; ordinary folding later consumes it unchanged.
    #[must_use]
    pub const fn receipt(&self) -> &KagemushaWalletReceiptV1 {
        &self.receipt
    }
    /// Signed-object digest of that receipt.
    #[must_use]
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
    /// Anchor head sequence and its lineage digest.
    #[must_use]
    pub const fn anchor(&self) -> (u128, [u8; 32]) {
        (self.anchor_sequence, self.anchor_digest)
    }
    /// Verified steps after the anchor, and how many are Receives.
    #[must_use]
    pub const fn steps(&self) -> (u32, u32) {
        (self.steps, self.receives)
    }
    /// Opened tip and its lineage-adjusted fields.
    #[must_use]
    pub const fn tip(&self) -> (&KagemushaWalletStateV1, &KagemushaWalletLineagePublicV1) {
        (&self.tip_state, &self.tip_adjusted)
    }
    /// Unload successor state and its adjusted projection.
    #[must_use]
    pub const fn successor(&self) -> (&KagemushaWalletStateV1, &KagemushaWalletLineagePublicV1) {
        (&self.successor, &self.successor_adjusted)
    }
    /// The exact Unload statement and its digest.
    #[must_use]
    pub const fn statement(&self) -> (&KagemushaWalletStatementV1, [u8; 32]) {
        (&self.statement, self.statement_digest)
    }
    /// The verified Unload `σ`.
    #[must_use]
    pub const fn sigma(&self) -> &KagemushaWalletStepProofV1 {
        &self.sigma
    }
    /// Redemption nullifier and ordinal: the shared Unload payout namespace.
    #[must_use]
    pub const fn nullifier(&self) -> ([u8; 32], u128) {
        (self.nullifier, self.redeem_ordinal)
    }
    /// Offline face amount, authenticated online charge and charge-quote digest (zero for
    /// the default zero charge).
    #[must_use]
    pub const fn terms(&self) -> (u128, u128, [u8; 32]) {
        (self.amount, self.online_charge, self.charge_quote)
    }
}
