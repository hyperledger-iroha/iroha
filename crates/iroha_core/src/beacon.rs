//! Canonical global threshold-beacon verification.
//!
//! A first-release pulse is one unique threshold-BLS group signature over an
//! exact network, key session, slot, transcript, and finalized
//! chain anchor. Its public seed is derived only after final-signature
//! verification. Reconstruction shares and signer subsets are deliberately not
//! part of the pulse DTO, so different qualifying subsets cannot create
//! different public representations of the same pulse.
//!
//! The first-release module exposes only this threshold-beacon construction;
//! retired per-validator VRF constructions are deliberately absent.

/// Height-bound production readiness, separate from consensus admission.
pub mod readiness;

use iroha_crypto::{
    Hash,
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript,
        AdaptiveThresholdBlsSecretShare, BeaconPurpose, DasRenDealerCommitment,
        DasRenPartialSignature, DasRenRevealedShare, ThresholdBlsError, ThresholdBlsPublicKey,
        ThresholdBlsSession, ThresholdBlsSignature, ValidatedDealerCommitment,
    },
};
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_crypto::{HashOf, threshold_bls::DasRenDealerSecret};
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_data_model::block::BlockHeader;
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_data_model::consensus::GlobalThresholdBeaconDkgConstantProofV1;
use iroha_data_model::{
    NetworkId,
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconDkgComplaintResponseV1,
        GlobalThresholdBeaconDkgComplaintV1, GlobalThresholdBeaconDkgDealerCommitmentV1,
        GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconDkgTranscriptV1,
        GlobalThresholdBeaconKeySessionV1, GlobalThresholdBeaconPartialSignatureProofV1,
        GlobalThresholdBeaconPartialSignatureV1, GlobalThresholdBeaconPublicShareV1,
    },
};
use iroha_model_base::peer::PeerId;
use mv::storage::StorageReadOnly;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::Encode as _,
    derive::{JsonDeserialize, JsonSerialize},
};
#[cfg(any(test, feature = "iroha-core-tests"))]
use rand::{SeedableRng as _, rngs::StdRng};
use std::{collections::BTreeMap, sync::RwLock};
use thiserror::Error;
use zeroize::Zeroizing;

#[cfg(feature = "test-network-parliament-signers")]
#[doc(hidden)]
pub mod parliament_test_network_signer;

const GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.pulse-payload.v1\0";
const GLOBAL_BEACON_PULSE_ID_DOMAIN_V1: &[u8] = b"iroha.global-threshold-beacon.pulse-id.v1\0";
const GLOBAL_BEACON_NPOS_SUCCESSOR_SEED_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.npos-successor-seed.v1\0";
const GLOBAL_BEACON_LANE_RELAY_SEED_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.lane-relay-seed.v1\0";
const GLOBAL_BEACON_GOVERNANCE_SEED_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.governance-seed.v1\0";

/// Canonical pulse-position round for the first-release beacon protocol.
///
/// Consensus views only route and retransmit partials. They never alter the
/// threshold-signed payload, preventing view-change grinding after a pulse is
/// observed.
pub const GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1: u64 = 0;

/// Read the sole canonical active global-beacon key-session pointer.
///
/// The first-release state shape permits either no entry or exactly one entry
/// at its internal singleton key. Exposing this checked projection keeps
/// dependent crates from treating a corrupt storage key as authoritative.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError::PersistenceConflict`] when the
/// underlying singleton storage contains a noncanonical key or more than one
/// entry.
pub fn active_global_threshold_beacon_session_id_v1(
    world: &impl crate::state::WorldReadOnly,
) -> Result<Option<[u8; 32]>, GlobalThresholdBeaconError> {
    let mut entries = world.global_beacon_active_session().iter();
    let Some((key, session_id)) = entries.next() else {
        return Ok(None);
    };
    if *key != crate::state::GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY || entries.next().is_some() {
        return Err(GlobalThresholdBeaconError::PersistenceConflict);
    }
    Ok(Some(*session_id))
}

/// Hash the exact ordered, domainless validator identities used as DKG seats.
///
/// Consensus power is fixed to one in Sumeragi v2, so the public beacon
/// session binds the canonical `PeerId` roster rather than duplicating that
/// invariant in its DKG transcript.
#[must_use]
pub fn global_threshold_beacon_roster_hash_v1(roster: &[PeerId]) -> [u8; 32] {
    *iroha_crypto::HashOf::new(&roster.to_vec()).as_ref()
}

/// Authenticate a public beacon key against an exact ordered consensus roster.
///
/// The active-key pointer alone is insufficient: a stale or foreign DKG key
/// may still be well formed and produce valid threshold signatures. Every
/// producer and validator must therefore compare both the roster commitment
/// and its exact committee size with the authenticated height context.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError::RosterMismatch`] when the key session
/// does not name precisely the supplied ordered roster.
pub(crate) fn authenticated_global_threshold_beacon_roster_hash_v1(
    session: &GlobalThresholdBeaconKeySessionV1,
    roster: &[PeerId],
) -> Result<[u8; 32], GlobalThresholdBeaconError> {
    let roster_hash = global_threshold_beacon_roster_hash_v1(roster);
    if session.roster_hash != roster_hash || usize::from(session.committee_size) != roster.len() {
        return Err(GlobalThresholdBeaconError::RosterMismatch);
    }
    Ok(roster_hash)
}

/// Exact external bindings required when admitting a global beacon key session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalThresholdBeaconSessionBindingV1 {
    /// Exact deployment identity derived from genesis.
    pub network_id: NetworkId,
    /// Expected unique beacon session identifier.
    pub session_id: [u8; 32],
    /// Expected hash of the frozen ordered validator roster.
    pub roster_hash: [u8; 32],
    /// Expected commitment to the complete public DKG transcript.
    pub transcript_hash: [u8; 32],
}

/// Authoritative monotonic ingestion cursor for finalized pulses.
///
/// The cursor is not part of any later pulse's signed message or seed. It only
/// records the latest admitted slot so persistence can reject late insertion
/// while permitting intentionally skipped optional heights.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::beacon::GlobalThresholdBeaconPulseLinkV1")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct GlobalThresholdBeaconPulseLinkV1 {
    /// Identifier of the latest admitted pulse or genesis origin.
    pub pulse_id: [u8; 32],
    /// Seed of the latest admitted pulse or genesis origin.
    pub seed: [u8; 32],
    /// Latest admitted consensus height.
    pub height: u64,
    /// Latest admitted protocol round.
    pub round: u64,
}

impl GlobalThresholdBeaconPulseLinkV1 {
    /// Validate a non-zero persisted ingestion cursor.
    pub fn validate(self) -> Result<(), GlobalThresholdBeaconError> {
        if is_zero(&self.pulse_id) || is_zero(&self.seed) {
            return Err(GlobalThresholdBeaconError::ZeroPulse);
        }
        Ok(())
    }

    /// Validate the genesis-supplied origin used before the first pulse.
    pub fn validate_origin(self) -> Result<(), GlobalThresholdBeaconError> {
        self.validate()?;
        if self.height != 0 || self.round != 0 {
            return Err(GlobalThresholdBeaconError::NonMonotonicPosition);
        }
        Ok(())
    }
}

/// Public, replayable snapshot of an active adaptive DKG reducer.
///
/// This is deliberately a projection of public broadcasts only. Private share
/// deliveries and threshold-signature partials have no field in this type and
/// therefore cannot enter authoritative World persistence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::beacon::GlobalThresholdBeaconDkgSnapshotV1")]
pub struct GlobalThresholdBeaconDkgSnapshotV1 {
    /// Immutable DKG session and consensus-height schedule.
    pub session: GlobalThresholdBeaconDkgSessionV1,
    /// Purpose-separated Das--Ren `h` generator in canonical compressed G2 form.
    pub generator_h: [u8; 96],
    /// Purpose-separated Das--Ren `v` generator in canonical compressed G2 form.
    pub generator_v: [u8; 96],
    /// Dealer coefficient broadcasts in strictly increasing dealer order.
    pub dealer_commitments: Vec<GlobalThresholdBeaconDkgDealerCommitmentV1>,
    /// Complaints in strictly increasing `(dealer, complainant)` order.
    pub complaints: Vec<GlobalThresholdBeaconDkgComplaintV1>,
    /// Public complaint responses in the same canonical pair order.
    pub complaint_responses: Vec<GlobalThresholdBeaconDkgComplaintResponseV1>,
    /// Greatest committed height consumed by this reducer snapshot.
    pub last_updated_height: u64,
}

impl GlobalThresholdBeaconDkgSnapshotV1 {
    /// Validate all non-secret persisted structure and canonical ordering.
    ///
    /// Cryptographic implementations must additionally restore this snapshot
    /// with [`GlobalThresholdBeaconDkgStateV1::from_snapshot`] before resuming it;
    /// that path re-derives the generators and verifies every public proof.
    pub fn validate(&self) -> Result<(), GlobalThresholdBeaconError> {
        validate_dkg_session(&self.session)?;
        validate_dkg_generators(&self.session, &self.generator_h, &self.generator_v)?;

        let mut commitments = BTreeMap::new();
        for commitment in &self.dealer_commitments {
            validate_participant(&self.session, commitment.dealer_index)?;
            if commitment.coefficient_commitments.len() != usize::from(self.session.threshold)
                || commitments
                    .insert(commitment.dealer_index, commitment)
                    .is_some()
            {
                return Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation);
            }
        }
        if commitments.keys().copied().collect::<Vec<_>>()
            != self
                .dealer_commitments
                .iter()
                .map(|commitment| commitment.dealer_index)
                .collect::<Vec<_>>()
        {
            return Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation);
        }

        let mut complaints = BTreeMap::new();
        for complaint in &self.complaints {
            validate_participant(&self.session, complaint.dealer_index)?;
            validate_participant(&self.session, complaint.complainant_index)?;
            let Some(commitment) = commitments.get(&complaint.dealer_index) else {
                return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
            };
            if complaint.dealer_index == complaint.complainant_index
                || complaint.dealer_commitment_hash
                    != global_threshold_beacon_dkg_dealer_commitment_hash_v1(
                        &self.session,
                        commitment,
                    )
                || complaint.complaint_id
                    != global_threshold_beacon_dkg_complaint_id_v1(
                        &self.session,
                        complaint.dealer_index,
                        complaint.complainant_index,
                        complaint.dealer_commitment_hash,
                        complaint.reason,
                    )
                || complaints
                    .insert(
                        (complaint.dealer_index, complaint.complainant_index),
                        complaint,
                    )
                    .is_some()
            {
                return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
            }
        }
        if complaints.keys().copied().collect::<Vec<_>>()
            != self
                .complaints
                .iter()
                .map(|complaint| (complaint.dealer_index, complaint.complainant_index))
                .collect::<Vec<_>>()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
        }

        let mut responses = BTreeMap::new();
        for response in &self.complaint_responses {
            let key = (response.dealer_index, response.recipient_index);
            let Some(complaint) = complaints.get(&key) else {
                return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
            };
            if response.complaint_id != complaint.complaint_id
                || responses.insert(key, response).is_some()
            {
                return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
            }
        }
        if responses.keys().copied().collect::<Vec<_>>()
            != self
                .complaint_responses
                .iter()
                .map(|response| (response.dealer_index, response.recipient_index))
                .collect::<Vec<_>>()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
        }

        if (!self.dealer_commitments.is_empty()
            && self.last_updated_height < self.session.start_height)
            || (!self.complaints.is_empty()
                && self.last_updated_height < self.session.sharing_end_height)
            || (!self.complaint_responses.is_empty()
                && self.last_updated_height < self.session.complaints_end_height)
        {
            return Err(GlobalThresholdBeaconError::NonMonotonicDkgState);
        }
        Ok(())
    }
}

/// Finalized public beacon-key session with activation and retirement metadata.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1")]
pub struct FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
    /// Canonical finalized threshold-beacon session and full public transcript.
    pub session: GlobalThresholdBeaconKeySessionV1,
    /// Committed height at which this key became the active pulse signer.
    pub activated_at_height: Option<u64>,
    /// Committed height at which this key ceased being active.
    pub retired_at_height: Option<u64>,
}

impl FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
    /// Construct a finalized but not-yet-active key lifecycle record.
    pub fn new(
        session: GlobalThresholdBeaconKeySessionV1,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        let record = Self {
            session,
            activated_at_height: None,
            retired_at_height: None,
        };
        record.validate()?;
        Ok(record)
    }

    /// Validate the full public key transcript and lifecycle ordering.
    pub fn validate(&self) -> Result<(), GlobalThresholdBeaconError> {
        let binding = GlobalThresholdBeaconSessionBindingV1 {
            network_id: self.session.network_id,
            session_id: self.session.session_id,
            roster_hash: self.session.roster_hash,
            transcript_hash: self.session.transcript_hash,
        };
        validate_global_threshold_beacon_session_v1(self.session.clone(), &binding)?;
        match (self.activated_at_height, self.retired_at_height) {
            (None, None) => {}
            (Some(activated), None)
                if activated >= self.session.adaptive_dkg.finalized_at_height => {}
            (Some(activated), Some(retired))
                if activated >= self.session.adaptive_dkg.finalized_at_height
                    && retired > activated => {}
            _ => return Err(GlobalThresholdBeaconError::InvalidKeyLifecycle),
        }
        Ok(())
    }

    /// Mark this key active at a committed height, idempotently at the same height.
    pub fn activate(&mut self, height: u64) -> Result<(), GlobalThresholdBeaconError> {
        if self.retired_at_height.is_some()
            || height < self.session.adaptive_dkg.finalized_at_height
        {
            return Err(GlobalThresholdBeaconError::InvalidKeyLifecycle);
        }
        match self.activated_at_height {
            Some(existing) if existing != height => {
                return Err(GlobalThresholdBeaconError::InvalidKeyLifecycle);
            }
            Some(_) => return Ok(()),
            None => self.activated_at_height = Some(height),
        }
        self.validate()
    }

    /// Mark this active key retired at a strictly later committed height.
    pub fn retire(&mut self, height: u64) -> Result<(), GlobalThresholdBeaconError> {
        let activated = self
            .activated_at_height
            .ok_or(GlobalThresholdBeaconError::InvalidKeyLifecycle)?;
        if height <= activated {
            return Err(GlobalThresholdBeaconError::InvalidKeyLifecycle);
        }
        match self.retired_at_height {
            Some(existing) if existing != height => {
                return Err(GlobalThresholdBeaconError::InvalidKeyLifecycle);
            }
            Some(_) => return Ok(()),
            None => self.retired_at_height = Some(height),
        }
        self.validate()
    }

    /// Return whether this record authorizes pulses at `height`.
    #[must_use]
    pub fn is_active_at(&self, height: u64) -> bool {
        self.activated_at_height
            .is_some_and(|start| start <= height)
            && self.retired_at_height.is_none_or(|end| height < end)
    }
}

/// Validation failures for global threshold-beacon sessions and pulses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GlobalThresholdBeaconError {
    /// The DTO does not use the sole first-release protocol version.
    #[error("unsupported global threshold-beacon version {actual}")]
    UnsupportedVersion {
        /// Version supplied by the DTO.
        actual: u16,
    },
    /// The canonical Norito envelope could not be decoded or re-encoded.
    #[error("invalid global threshold-beacon Norito envelope")]
    InvalidEncoding,
    /// The supplied bytes are not the unique canonical Norito encoding.
    #[error("non-canonical global threshold-beacon Norito encoding")]
    NonCanonicalEncoding,
    /// The pulse or session targets another genesis-derived network.
    #[error("global threshold-beacon network binding mismatch")]
    NetworkMismatch,
    /// The pulse or key uses another threshold-beacon session.
    #[error("global threshold-beacon session binding mismatch")]
    SessionMismatch,
    /// The pulse or key uses another frozen validator roster.
    #[error("global threshold-beacon roster binding mismatch")]
    RosterMismatch,
    /// The supplied public transcript does not match its computed or expected commitment.
    #[error("global threshold-beacon public transcript mismatch")]
    TranscriptMismatch,
    /// A required pulse identifier, seed, height, or finalized anchor is inert.
    #[error("global threshold-beacon pulse contains an inert zero binding")]
    ZeroPulse,
    /// The pulse position does not strictly follow the authoritative ingestion cursor.
    #[error("global threshold-beacon height/round is not strictly monotonic")]
    NonMonotonicPosition,
    /// A finalized pulse used a consensus-view-dependent round.
    #[error("global threshold-beacon pulse round is not the canonical fixed round")]
    NonCanonicalRound,
    /// The pulse does not authenticate the expected finalized-chain point.
    #[error("global threshold-beacon finalized-chain anchor mismatch")]
    FinalizedAnchorMismatch,
    /// The supplied seed is not the unique seed derived from the final signature.
    #[error("global threshold-beacon derived seed mismatch")]
    SeedMismatch,
    /// A pulse reused an already admitted identifier or slot.
    #[error("global threshold-beacon pulse reused an earlier result")]
    ReusedPulse,
    /// The supplied pulse ID is not its canonical computed identifier.
    #[error("global threshold-beacon pulse identifier mismatch")]
    PulseIdMismatch,
    /// DKG height windows or immutable bindings are inconsistent.
    #[error("invalid global threshold-beacon DKG session schedule or binding")]
    InvalidDkgSession,
    /// A DKG message names a participant outside the frozen committee.
    #[error("global threshold-beacon DKG participant index is outside the frozen committee")]
    InvalidDkgParticipant,
    /// A DKG event arrived outside its consensus-height phase.
    #[error("global threshold-beacon DKG event arrived in the wrong phase")]
    WrongDkgPhase,
    /// A dealer broadcast two different commitments for one session.
    #[error("global threshold-beacon DKG dealer commitment equivocation")]
    DealerCommitmentEquivocation,
    /// A complaint is malformed, misbound, or conflicts with a prior complaint.
    #[error("invalid global threshold-beacon DKG complaint")]
    InvalidDkgComplaint,
    /// A complaint response is absent, misbound, or conflicts with a prior response.
    #[error("invalid global threshold-beacon DKG complaint response")]
    InvalidDkgComplaintResponse,
    /// Too few dealers remain qualified to preserve the configured fault bound.
    #[error("global threshold-beacon DKG has too few qualified dealers")]
    InsufficientQualifiedDealers,
    /// The DKG state has already reached a terminal state.
    #[error("global threshold-beacon DKG is already terminal")]
    DkgTerminal,
    /// A persisted DKG snapshot moved its consensus high-water mark backwards.
    #[error("global threshold-beacon DKG snapshot height regressed")]
    NonMonotonicDkgState,
    /// Activation or retirement metadata is inconsistent with DKG finalization.
    #[error("invalid global threshold-beacon key lifecycle")]
    InvalidKeyLifecycle,
    /// Authoritative World persistence conflicts with an existing beacon record.
    #[error("conflicting global threshold-beacon persistent state")]
    PersistenceConflict,
    /// The active key pointer does not authorize the requested beacon operation.
    #[error("global threshold-beacon active key mismatch")]
    ActiveKeyMismatch,
    /// Persisted finalized pulse history or its latest link is inconsistent.
    #[error("invalid global threshold-beacon pulse history")]
    InvalidPulseHistory,
    /// One signer supplied two distinct, individually addressed partial signatures.
    #[error("global threshold-beacon partial-signature equivocation")]
    PartialSignatureEquivocation,
    /// The pulse reducer has not collected the session reconstruction threshold.
    #[error("insufficient verified global threshold-beacon partial signatures")]
    InsufficientPartialSignatures,
    /// Fixed-suite threshold-BLS validation failed.
    #[error(transparent)]
    ThresholdBls(#[from] ThresholdBlsError),
}

/// Consensus-height phase of an adaptive global beacon DKG run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalThresholdBeaconDkgPhaseV1 {
    /// The configured start height has not been reached.
    Pending,
    /// Dealers broadcast coefficient commitments and constant-term proofs.
    Sharing,
    /// Recipients broadcast complaints about missing or invalid private shares.
    Complaints,
    /// Accused dealers publicly reveal the disputed share triples.
    Responses,
    /// The response deadline passed and qualification can be derived.
    Finalizable,
    /// The session produced one canonical public transcript.
    Finalized,
    /// The session could not retain the required honest-dealer floor.
    Aborted,
}

/// Public values derived by the adaptive crypto implementation at DKG finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalThresholdBeaconDkgDerivedPublicV1 {
    /// Canonical compressed G2 group public key.
    pub group_public_key: [u8; 96],
    /// Composite verification keys with roster-and-index participant seat bindings.
    pub public_shares: Vec<GlobalThresholdBeaconPublicShareV1>,
    /// Typed adaptive public-transcript commitment.
    pub transcript_hash: [u8; 32],
}

/// Narrow cryptographic boundary used by the consensus-owned DKG reducer.
///
/// Implementations must use the Das--Ren Figure-2/3/5 construction. In
/// particular, ordinary Feldman shares are not a conforming implementation.
pub trait GlobalThresholdBeaconDkgCryptoV1 {
    /// Derive independent, purpose-separated G2 generators `h` and `v`.
    fn derive_generators(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
    ) -> Result<([u8; 96], [u8; 96]), ThresholdBlsError>;

    /// Validate one complete dealer coefficient vector and constant-term PoK.
    fn verify_dealer_commitment(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) -> Result<(), ThresholdBlsError>;

    /// Validate one public complaint response against its dealer commitment.
    fn verify_complaint_response(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
        response: &GlobalThresholdBeaconDkgComplaintResponseV1,
    ) -> Result<(), ThresholdBlsError>;

    /// Derive the group key, all composite public shares, and ready transcript.
    fn finalize_qualified_dealers(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
        qualified_dealers: &[u16],
        event_hash: [u8; 32],
    ) -> Result<GlobalThresholdBeaconDkgDerivedPublicV1, ThresholdBlsError>;
}

/// Production adaptive threshold-BLS backend for the consensus DKG reducer.
///
/// This adapter owns public verification and transcript finalization only. It
/// never accepts, returns, clones, logs, or serializes a private dealer/share
/// object; zeroizing secret owners remain exclusively in `iroha_crypto` and the
/// authenticated DKG transport layer.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptiveGlobalThresholdBeaconDkgCryptoV1;

impl GlobalThresholdBeaconDkgCryptoV1 for AdaptiveGlobalThresholdBeaconDkgCryptoV1 {
    fn derive_generators(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
    ) -> Result<([u8; 96], [u8; 96]), ThresholdBlsError> {
        let parameters = adaptive_beacon_parameters(session)?;
        Ok((*parameters.h_bytes(), *parameters.v_bytes()))
    }

    fn verify_dealer_commitment(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) -> Result<(), ThresholdBlsError> {
        let parameters = adaptive_beacon_parameters(session)?;
        require_adaptive_generators(&parameters, generator_h, generator_v)?;
        verify_adaptive_dealer(&parameters, commitment)?;
        Ok(())
    }

    fn verify_complaint_response(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
        response: &GlobalThresholdBeaconDkgComplaintResponseV1,
    ) -> Result<(), ThresholdBlsError> {
        let parameters = adaptive_beacon_parameters(session)?;
        require_adaptive_generators(&parameters, generator_h, generator_v)?;
        let dealer = verify_adaptive_dealer(&parameters, commitment)?;
        if response.dealer_index != commitment.dealer_index {
            return Err(ThresholdBlsError::InvalidComplaintResponse);
        }
        DasRenRevealedShare::verify(
            &parameters,
            &dealer,
            response.recipient_index,
            response.s_share,
            response.r_share,
            response.u_share,
        )?;
        Ok(())
    }

    fn finalize_qualified_dealers(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        generator_h: &[u8; 96],
        generator_v: &[u8; 96],
        dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
        qualified_dealers: &[u16],
        event_hash: [u8; 32],
    ) -> Result<GlobalThresholdBeaconDkgDerivedPublicV1, ThresholdBlsError> {
        let parameters = adaptive_beacon_parameters(session)?;
        require_adaptive_generators(&parameters, generator_h, generator_v)?;
        let validated = dealer_commitments
            .iter()
            .map(|commitment| {
                verify_adaptive_dealer(&parameters, commitment)
                    .map(|dealer| (commitment.dealer_index, dealer))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let qualified = qualified_dealers
            .iter()
            .map(|index| {
                validated
                    .get(index)
                    .cloned()
                    .ok_or(ThresholdBlsError::NonCanonicalQualifiedSet)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            &parameters,
            &qualified,
            qualified_dealers,
            event_hash,
        )?;
        Ok(GlobalThresholdBeaconDkgDerivedPublicV1 {
            group_public_key: *transcript.group_public_key().as_bytes(),
            public_shares: transcript
                .public_shares()
                .iter()
                .map(|share| GlobalThresholdBeaconPublicShareV1 {
                    index: share.index(),
                    // This is H(typed session, roster commitment, one-based seat),
                    // not an independently supplied identity digest.
                    participant_seat_binding: *share.participant_hash(),
                    public_key_share: *share.as_bytes(),
                })
                .collect(),
            transcript_hash: *transcript.transcript_hash(),
        })
    }
}

fn adaptive_beacon_parameters(
    session: &GlobalThresholdBeaconDkgSessionV1,
) -> Result<AdaptiveThresholdBlsParameters<BeaconPurpose>, ThresholdBlsError> {
    let typed_session = ThresholdBlsSession::<BeaconPurpose>::new(
        *session.network_id.as_bytes(),
        session.session_id,
        session.roster_hash,
        session.committee_size,
        session.threshold,
    )?;
    AdaptiveThresholdBlsParameters::derive(&typed_session)
}

fn require_adaptive_generators(
    parameters: &AdaptiveThresholdBlsParameters<BeaconPurpose>,
    generator_h: &[u8; 96],
    generator_v: &[u8; 96],
) -> Result<(), ThresholdBlsError> {
    if parameters.h_bytes() != generator_h || parameters.v_bytes() != generator_v {
        return Err(ThresholdBlsError::InvalidAdaptiveGenerator);
    }
    Ok(())
}

fn verify_adaptive_dealer(
    parameters: &AdaptiveThresholdBlsParameters<BeaconPurpose>,
    commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
) -> Result<ValidatedDealerCommitment<BeaconPurpose>, ThresholdBlsError> {
    DasRenDealerCommitment::verify(
        parameters,
        commitment.dealer_index,
        &commitment.coefficient_commitments,
        commitment.constant_term_proof.commitment,
        commitment.constant_term_proof.response,
    )
}

/// Deterministic consensus reducer for the public phases of adaptive beacon DKG.
#[derive(Debug, Clone)]
pub struct GlobalThresholdBeaconDkgStateV1 {
    session: GlobalThresholdBeaconDkgSessionV1,
    generator_h: [u8; 96],
    generator_v: [u8; 96],
    dealer_commitments: BTreeMap<u16, GlobalThresholdBeaconDkgDealerCommitmentV1>,
    complaints: BTreeMap<(u16, u16), GlobalThresholdBeaconDkgComplaintV1>,
    complaint_responses: BTreeMap<(u16, u16), GlobalThresholdBeaconDkgComplaintResponseV1>,
    finalized: Option<GlobalThresholdBeaconKeySessionV1>,
    aborted: bool,
    last_updated_height: u64,
}

impl GlobalThresholdBeaconDkgStateV1 {
    /// Start one DKG reducer after validating its immutable schedule and generators.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        crypto: &impl GlobalThresholdBeaconDkgCryptoV1,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        validate_dkg_session(&session)?;
        let (generator_h, generator_v) = crypto.derive_generators(&session)?;
        if is_zero(&generator_h) || is_zero(&generator_v) || generator_h == generator_v {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        let last_updated_height = session.start_height.saturating_sub(1);
        Ok(Self {
            session,
            generator_h,
            generator_v,
            dealer_commitments: BTreeMap::new(),
            complaints: BTreeMap::new(),
            complaint_responses: BTreeMap::new(),
            finalized: None,
            aborted: false,
            last_updated_height,
        })
    }

    /// Restore one public snapshot, re-deriving generators and re-verifying all
    /// public dealer proofs before the reducer can consume another event.
    pub fn from_snapshot(
        snapshot: GlobalThresholdBeaconDkgSnapshotV1,
        crypto: &impl GlobalThresholdBeaconDkgCryptoV1,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        snapshot.validate()?;
        let (generator_h, generator_v) = crypto.derive_generators(&snapshot.session)?;
        if generator_h != snapshot.generator_h || generator_v != snapshot.generator_v {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        for commitment in &snapshot.dealer_commitments {
            crypto.verify_dealer_commitment(
                &snapshot.session,
                &generator_h,
                &generator_v,
                commitment,
            )?;
        }
        let commitments = snapshot
            .dealer_commitments
            .iter()
            .map(|commitment| (commitment.dealer_index, commitment))
            .collect::<BTreeMap<_, _>>();
        for response in &snapshot.complaint_responses {
            let commitment = commitments
                .get(&response.dealer_index)
                .ok_or(GlobalThresholdBeaconError::InvalidDkgComplaintResponse)?;
            crypto.verify_complaint_response(
                &snapshot.session,
                &generator_h,
                &generator_v,
                commitment,
                response,
            )?;
        }
        Ok(Self {
            session: snapshot.session,
            generator_h,
            generator_v,
            dealer_commitments: snapshot
                .dealer_commitments
                .into_iter()
                .map(|commitment| (commitment.dealer_index, commitment))
                .collect(),
            complaints: snapshot
                .complaints
                .into_iter()
                .map(|complaint| {
                    (
                        (complaint.dealer_index, complaint.complainant_index),
                        complaint,
                    )
                })
                .collect(),
            complaint_responses: snapshot
                .complaint_responses
                .into_iter()
                .map(|response| ((response.dealer_index, response.recipient_index), response))
                .collect(),
            finalized: None,
            aborted: false,
            last_updated_height: snapshot.last_updated_height,
        })
    }

    /// Return the canonical public-only persistence projection of this active reducer.
    pub fn public_snapshot(
        &self,
    ) -> Result<GlobalThresholdBeaconDkgSnapshotV1, GlobalThresholdBeaconError> {
        if self.finalized.is_some() || self.aborted {
            return Err(GlobalThresholdBeaconError::DkgTerminal);
        }
        let snapshot = GlobalThresholdBeaconDkgSnapshotV1 {
            session: self.session,
            generator_h: self.generator_h,
            generator_v: self.generator_v,
            dealer_commitments: self.dealer_commitments.values().cloned().collect(),
            complaints: self.complaints.values().copied().collect(),
            complaint_responses: self.complaint_responses.values().copied().collect(),
            last_updated_height: self.last_updated_height,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Return this reducer's immutable session identifier.
    #[must_use]
    pub const fn session_id(&self) -> [u8; 32] {
        self.session.session_id
    }

    fn require_monotonic_height(&self, height: u64) -> Result<(), GlobalThresholdBeaconError> {
        if height < self.last_updated_height {
            return Err(GlobalThresholdBeaconError::NonMonotonicDkgState);
        }
        Ok(())
    }

    /// Return the phase implied by committed height and terminal reducer state.
    #[must_use]
    pub fn phase_at(&self, height: u64) -> GlobalThresholdBeaconDkgPhaseV1 {
        if self.finalized.is_some() {
            return GlobalThresholdBeaconDkgPhaseV1::Finalized;
        }
        if self.aborted {
            return GlobalThresholdBeaconDkgPhaseV1::Aborted;
        }
        if height < self.session.start_height {
            GlobalThresholdBeaconDkgPhaseV1::Pending
        } else if height < self.session.sharing_end_height {
            GlobalThresholdBeaconDkgPhaseV1::Sharing
        } else if height < self.session.complaints_end_height {
            GlobalThresholdBeaconDkgPhaseV1::Complaints
        } else if height < self.session.responses_end_height {
            GlobalThresholdBeaconDkgPhaseV1::Responses
        } else {
            GlobalThresholdBeaconDkgPhaseV1::Finalizable
        }
    }

    /// Admit one dealer broadcast during the sharing window.
    pub fn record_dealer_commitment(
        &mut self,
        height: u64,
        commitment: GlobalThresholdBeaconDkgDealerCommitmentV1,
        crypto: &impl GlobalThresholdBeaconDkgCryptoV1,
    ) -> Result<(), GlobalThresholdBeaconError> {
        self.require_monotonic_height(height)?;
        if self.phase_at(height) != GlobalThresholdBeaconDkgPhaseV1::Sharing {
            return Err(GlobalThresholdBeaconError::WrongDkgPhase);
        }
        validate_participant(&self.session, commitment.dealer_index)?;
        if commitment.coefficient_commitments.len() != usize::from(self.session.threshold) {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        if let Some(existing) = self.dealer_commitments.get(&commitment.dealer_index) {
            return if existing == &commitment {
                self.last_updated_height = height;
                Ok(())
            } else {
                Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation)
            };
        }
        crypto.verify_dealer_commitment(
            &self.session,
            &self.generator_h,
            &self.generator_v,
            &commitment,
        )?;
        self.dealer_commitments
            .insert(commitment.dealer_index, commitment);
        self.last_updated_height = height;
        Ok(())
    }

    /// Admit one canonically bound complaint during the complaint window.
    pub fn record_complaint(
        &mut self,
        height: u64,
        complaint: GlobalThresholdBeaconDkgComplaintV1,
    ) -> Result<(), GlobalThresholdBeaconError> {
        self.require_monotonic_height(height)?;
        if self.phase_at(height) != GlobalThresholdBeaconDkgPhaseV1::Complaints {
            return Err(GlobalThresholdBeaconError::WrongDkgPhase);
        }
        validate_participant(&self.session, complaint.dealer_index)?;
        validate_participant(&self.session, complaint.complainant_index)?;
        if complaint.dealer_index == complaint.complainant_index {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
        }
        let dealer = self
            .dealer_commitments
            .get(&complaint.dealer_index)
            .ok_or(GlobalThresholdBeaconError::InvalidDkgComplaint)?;
        if complaint.dealer_commitment_hash
            != global_threshold_beacon_dkg_dealer_commitment_hash_v1(&self.session, dealer)
            || complaint.complaint_id
                != global_threshold_beacon_dkg_complaint_id_v1(
                    &self.session,
                    complaint.dealer_index,
                    complaint.complainant_index,
                    complaint.dealer_commitment_hash,
                    complaint.reason,
                )
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
        }
        let key = (complaint.dealer_index, complaint.complainant_index);
        if let Some(existing) = self.complaints.get(&key) {
            return if existing == &complaint {
                self.last_updated_height = height;
                Ok(())
            } else {
                Err(GlobalThresholdBeaconError::InvalidDkgComplaint)
            };
        }
        self.complaints.insert(key, complaint);
        self.last_updated_height = height;
        Ok(())
    }

    /// Admit one valid public share reveal during the response window.
    pub fn record_complaint_response(
        &mut self,
        height: u64,
        response: GlobalThresholdBeaconDkgComplaintResponseV1,
        crypto: &impl GlobalThresholdBeaconDkgCryptoV1,
    ) -> Result<(), GlobalThresholdBeaconError> {
        self.require_monotonic_height(height)?;
        if self.phase_at(height) != GlobalThresholdBeaconDkgPhaseV1::Responses {
            return Err(GlobalThresholdBeaconError::WrongDkgPhase);
        }
        let key = (response.dealer_index, response.recipient_index);
        let complaint = self
            .complaints
            .get(&key)
            .ok_or(GlobalThresholdBeaconError::InvalidDkgComplaintResponse)?;
        if response.complaint_id != complaint.complaint_id {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
        }
        if let Some(existing) = self.complaint_responses.get(&key) {
            return if existing == &response {
                self.last_updated_height = height;
                Ok(())
            } else {
                Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse)
            };
        }
        let commitment = self
            .dealer_commitments
            .get(&response.dealer_index)
            .ok_or(GlobalThresholdBeaconError::InvalidDkgComplaintResponse)?;
        crypto.verify_complaint_response(
            &self.session,
            &self.generator_h,
            &self.generator_v,
            commitment,
            &response,
        )?;
        self.complaint_responses.insert(key, response);
        self.last_updated_height = height;
        Ok(())
    }

    /// Derive the qualified set and finalize the sole canonical public transcript.
    pub fn finalize(
        &mut self,
        height: u64,
        crypto: &impl GlobalThresholdBeaconDkgCryptoV1,
    ) -> Result<&GlobalThresholdBeaconKeySessionV1, GlobalThresholdBeaconError> {
        self.require_monotonic_height(height)?;
        if self.finalized.is_some() || self.aborted {
            return Err(GlobalThresholdBeaconError::DkgTerminal);
        }
        if self.phase_at(height) != GlobalThresholdBeaconDkgPhaseV1::Finalizable {
            return Err(GlobalThresholdBeaconError::WrongDkgPhase);
        }
        let qualified_dealers = self
            .dealer_commitments
            .keys()
            .copied()
            .filter(|dealer| {
                self.complaints
                    .keys()
                    .filter(|(accused, _)| accused == dealer)
                    .all(|key| self.complaint_responses.contains_key(key))
            })
            .collect::<Vec<_>>();
        let fault_tolerance = (self.session.committee_size - 1) / 3;
        let required_qualified = self.session.committee_size - fault_tolerance;
        if qualified_dealers.len() < usize::from(required_qualified) {
            self.aborted = true;
            self.last_updated_height = height;
            return Err(GlobalThresholdBeaconError::InsufficientQualifiedDealers);
        }
        let dealer_commitments = self
            .dealer_commitments
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let complaints = self.complaints.values().copied().collect::<Vec<_>>();
        let complaint_responses = self
            .complaint_responses
            .values()
            .copied()
            .collect::<Vec<_>>();
        let event_hash = global_threshold_beacon_dkg_event_hash_v1(
            &self.session,
            &self.generator_h,
            &self.generator_v,
            &dealer_commitments,
            &complaints,
            &complaint_responses,
            &qualified_dealers,
            height,
        );
        let derived = crypto.finalize_qualified_dealers(
            &self.session,
            &self.generator_h,
            &self.generator_v,
            &dealer_commitments,
            &qualified_dealers,
            event_hash,
        )?;
        if derived.public_shares.len() != usize::from(self.session.committee_size)
            || is_zero(&derived.transcript_hash)
        {
            return Err(GlobalThresholdBeaconError::TranscriptMismatch);
        }
        let adaptive_dkg = GlobalThresholdBeaconDkgTranscriptV1 {
            session: self.session,
            generator_h: self.generator_h,
            generator_v: self.generator_v,
            dealer_commitments,
            complaints,
            complaint_responses,
            qualified_dealers,
            event_hash,
            finalized_at_height: height,
        };
        self.finalized = Some(GlobalThresholdBeaconKeySessionV1 {
            version: self.session.version,
            network_id: self.session.network_id,
            session_id: self.session.session_id,
            roster_hash: self.session.roster_hash,
            committee_size: self.session.committee_size,
            threshold: self.session.threshold,
            group_public_key: derived.group_public_key,
            public_shares: derived.public_shares,
            adaptive_dkg,
            dkg_contribution_hash: event_hash,
            transcript_hash: derived.transcript_hash,
        });
        self.last_updated_height = height;
        self.finalized
            .as_ref()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)
    }
}

fn validate_dkg_session(
    session: &GlobalThresholdBeaconDkgSessionV1,
) -> Result<(), GlobalThresholdBeaconError> {
    if session.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1
        || is_zero(&session.roster_hash)
        || session.start_height >= session.sharing_end_height
        || session.sharing_end_height >= session.complaints_end_height
        || session.complaints_end_height >= session.responses_end_height
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgSession);
    }
    ThresholdBlsSession::<BeaconPurpose>::new(
        *session.network_id.as_bytes(),
        session.session_id,
        session.roster_hash,
        session.committee_size,
        session.threshold,
    )?;
    Ok(())
}

fn validate_dkg_generators(
    session: &GlobalThresholdBeaconDkgSessionV1,
    generator_h: &[u8; 96],
    generator_v: &[u8; 96],
) -> Result<(), GlobalThresholdBeaconError> {
    if is_zero(generator_h) || is_zero(generator_v) || generator_h == generator_v {
        return Err(GlobalThresholdBeaconError::InvalidDkgSession);
    }
    let parameters = adaptive_beacon_parameters(session)?;
    require_adaptive_generators(&parameters, generator_h, generator_v)?;
    Ok(())
}

fn validate_participant(
    session: &GlobalThresholdBeaconDkgSessionV1,
    index: u16,
) -> Result<(), GlobalThresholdBeaconError> {
    if index == 0 || index > session.committee_size {
        return Err(GlobalThresholdBeaconError::InvalidDkgParticipant);
    }
    Ok(())
}

/// Compute the canonical identity of one dealer commitment in a typed DKG session.
#[must_use]
pub fn global_threshold_beacon_dkg_dealer_commitment_hash_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha.global-threshold-beacon.dkg-dealer.v1\0");
    preimage.extend_from_slice(session.network_id.as_bytes());
    preimage.extend_from_slice(&session.session_id);
    preimage.extend_from_slice(&session.roster_hash);
    preimage.extend_from_slice(&commitment.encode());
    *Hash::new(&preimage).as_ref()
}

/// Compute the canonical identity of one complaint from its fully bound fields.
#[must_use]
pub fn global_threshold_beacon_dkg_complaint_id_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    dealer_index: u16,
    complainant_index: u16,
    dealer_commitment_hash: [u8; 32],
    reason: iroha_data_model::consensus::GlobalThresholdBeaconDkgComplaintReasonV1,
) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha.global-threshold-beacon.dkg-complaint.v1\0");
    preimage.extend_from_slice(session.network_id.as_bytes());
    preimage.extend_from_slice(&session.session_id);
    preimage.extend_from_slice(&session.roster_hash);
    preimage.extend_from_slice(&dealer_index.to_be_bytes());
    preimage.extend_from_slice(&complainant_index.to_be_bytes());
    preimage.extend_from_slice(&dealer_commitment_hash);
    preimage.push(match reason {
        iroha_data_model::consensus::GlobalThresholdBeaconDkgComplaintReasonV1::MissingPrivateShare => 0,
        iroha_data_model::consensus::GlobalThresholdBeaconDkgComplaintReasonV1::InvalidPrivateShare => 1,
    });
    *Hash::new(&preimage).as_ref()
}

/// Compute the canonical public-event transcript hash for DKG finalization.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn global_threshold_beacon_dkg_event_hash_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
    generator_h: &[u8; 96],
    generator_v: &[u8; 96],
    dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
    complaints: &[GlobalThresholdBeaconDkgComplaintV1],
    complaint_responses: &[GlobalThresholdBeaconDkgComplaintResponseV1],
    qualified_dealers: &[u16],
    finalized_at_height: u64,
) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha.global-threshold-beacon.dkg-events.v1\0");
    preimage.extend_from_slice(&session.encode());
    preimage.extend_from_slice(generator_h);
    preimage.extend_from_slice(generator_v);
    preimage.extend_from_slice(&dealer_commitments.to_vec().encode());
    preimage.extend_from_slice(&complaints.to_vec().encode());
    preimage.extend_from_slice(&complaint_responses.to_vec().encode());
    preimage.extend_from_slice(&qualified_dealers.to_vec().encode());
    preimage.extend_from_slice(&finalized_at_height.to_be_bytes());
    *Hash::new(&preimage).as_ref()
}

fn validate_adaptive_dkg_shape(
    record: &GlobalThresholdBeaconKeySessionV1,
) -> Result<(), GlobalThresholdBeaconError> {
    let transcript = &record.adaptive_dkg;
    validate_dkg_session(&transcript.session)?;
    let session = &transcript.session;
    if session.version != record.version
        || session.network_id != record.network_id
        || session.session_id != record.session_id
        || session.roster_hash != record.roster_hash
        || session.committee_size != record.committee_size
        || session.threshold != record.threshold
        || transcript.finalized_at_height < session.responses_end_height
        || transcript.event_hash != record.dkg_contribution_hash
        || is_zero(&transcript.generator_h)
        || is_zero(&transcript.generator_v)
        || transcript.generator_h == transcript.generator_v
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgSession);
    }

    let mut commitments = BTreeMap::new();
    for commitment in &transcript.dealer_commitments {
        validate_participant(session, commitment.dealer_index)?;
        if commitment.coefficient_commitments.len() != usize::from(session.threshold)
            || commitments
                .insert(commitment.dealer_index, commitment)
                .is_some()
        {
            return Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation);
        }
    }
    if commitments.keys().copied().collect::<Vec<_>>()
        != transcript
            .dealer_commitments
            .iter()
            .map(|commitment| commitment.dealer_index)
            .collect::<Vec<_>>()
    {
        return Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation);
    }

    let mut complaints = BTreeMap::new();
    for complaint in &transcript.complaints {
        validate_participant(session, complaint.dealer_index)?;
        validate_participant(session, complaint.complainant_index)?;
        let Some(commitment) = commitments.get(&complaint.dealer_index) else {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
        };
        if complaint.dealer_index == complaint.complainant_index
            || complaint.dealer_commitment_hash
                != global_threshold_beacon_dkg_dealer_commitment_hash_v1(session, commitment)
            || complaint.complaint_id
                != global_threshold_beacon_dkg_complaint_id_v1(
                    session,
                    complaint.dealer_index,
                    complaint.complainant_index,
                    complaint.dealer_commitment_hash,
                    complaint.reason,
                )
            || complaints
                .insert(
                    (complaint.dealer_index, complaint.complainant_index),
                    complaint,
                )
                .is_some()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
        }
    }
    if complaints.keys().copied().collect::<Vec<_>>()
        != transcript
            .complaints
            .iter()
            .map(|complaint| (complaint.dealer_index, complaint.complainant_index))
            .collect::<Vec<_>>()
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgComplaint);
    }

    let mut responses = BTreeMap::new();
    for response in &transcript.complaint_responses {
        let key = (response.dealer_index, response.recipient_index);
        let Some(complaint) = complaints.get(&key) else {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
        };
        if response.complaint_id != complaint.complaint_id
            || responses.insert(key, response).is_some()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
        }
    }
    if responses.keys().copied().collect::<Vec<_>>()
        != transcript
            .complaint_responses
            .iter()
            .map(|response| (response.dealer_index, response.recipient_index))
            .collect::<Vec<_>>()
    {
        return Err(GlobalThresholdBeaconError::InvalidDkgComplaintResponse);
    }

    let derived_qualified = commitments
        .keys()
        .copied()
        .filter(|dealer| {
            complaints
                .keys()
                .filter(|(accused, _)| accused == dealer)
                .all(|key| responses.contains_key(key))
        })
        .collect::<Vec<_>>();
    let fault_tolerance = (session.committee_size - 1) / 3;
    if derived_qualified != transcript.qualified_dealers
        || derived_qualified.len() < usize::from(session.committee_size - fault_tolerance)
        || global_threshold_beacon_dkg_event_hash_v1(
            session,
            &transcript.generator_h,
            &transcript.generator_v,
            &transcript.dealer_commitments,
            &transcript.complaints,
            &transcript.complaint_responses,
            &transcript.qualified_dealers,
            transcript.finalized_at_height,
        ) != transcript.event_hash
    {
        return Err(GlobalThresholdBeaconError::InsufficientQualifiedDealers);
    }
    Ok(())
}

fn reconstruct_adaptive_beacon_transcript(
    record: &GlobalThresholdBeaconKeySessionV1,
) -> Result<AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>, GlobalThresholdBeaconError> {
    let public_dkg = &record.adaptive_dkg;
    ThresholdBlsPublicKey::<BeaconPurpose>::from_bytes(
        record.session_id,
        &record.group_public_key,
    )?;
    let parameters = adaptive_beacon_parameters(&public_dkg.session)?;
    require_adaptive_generators(
        &parameters,
        &public_dkg.generator_h,
        &public_dkg.generator_v,
    )?;
    let validated_dealers = public_dkg
        .dealer_commitments
        .iter()
        .map(|commitment| {
            verify_adaptive_dealer(&parameters, commitment)
                .map(|dealer| (commitment.dealer_index, dealer))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for response in &public_dkg.complaint_responses {
        let dealer = validated_dealers
            .get(&response.dealer_index)
            .ok_or(GlobalThresholdBeaconError::InvalidDkgComplaintResponse)?;
        DasRenRevealedShare::verify(
            &parameters,
            dealer,
            response.recipient_index,
            response.s_share,
            response.r_share,
            response.u_share,
        )?;
    }
    let qualified = public_dkg
        .qualified_dealers
        .iter()
        .map(|index| {
            validated_dealers
                .get(index)
                .cloned()
                .ok_or(ThresholdBlsError::NonCanonicalQualifiedSet)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
        &parameters,
        &qualified,
        &public_dkg.qualified_dealers,
        public_dkg.event_hash,
    )?;
    let reconstructed_shares = transcript.public_shares();
    if transcript.group_public_key().as_bytes() != &record.group_public_key
        || transcript.dkg_event_hash() != &record.dkg_contribution_hash
        || transcript.transcript_hash() != &record.transcript_hash
        || reconstructed_shares.len() != record.public_shares.len()
        || reconstructed_shares
            .iter()
            .zip(&record.public_shares)
            .any(|(actual, persisted)| {
                actual.index() != persisted.index
                    || actual.participant_hash() != &persisted.participant_seat_binding
                    || actual.as_bytes() != &persisted.public_key_share
            })
    {
        return Err(GlobalThresholdBeaconError::TranscriptMismatch);
    }
    Ok(transcript)
}

/// A completely validated, typed global threshold-beacon public session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGlobalThresholdBeaconSessionV1 {
    record: GlobalThresholdBeaconKeySessionV1,
    transcript: AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
}

impl ValidatedGlobalThresholdBeaconSessionV1 {
    /// Borrow the canonical data-model record.
    #[must_use]
    pub const fn record(&self) -> &GlobalThresholdBeaconKeySessionV1 {
        &self.record
    }

    /// Re-run the cryptographic release gate for the adaptive DKG/signing protocol.
    ///
    /// Only a transcript reconstructed from verified qualified dealer proofs
    /// can inhabit this validated session type.
    pub fn ensure_adaptive_protocol_ready(&self) -> Result<(), ThresholdBlsError> {
        self.transcript.ensure_adaptive_protocol_ready()
    }
}

/// Runtime-only owner capable of producing one adaptive beacon signature share.
///
/// Implementations are injected by the node's secure runtime boundary. They
/// must keep private DKG material out of configuration, World state, logs, and
/// wire DTOs. Every returned share is independently proof-verified by the
/// consensus reducer, so an unavailable or faulty provider can stop progress
/// but cannot inject unauthenticated randomness.
pub trait GlobalThresholdBeaconPartialSignerV1: Send + Sync {
    /// Attest live custody of the exact validated session and one-based signer seat.
    ///
    /// This performs no signing. Providers must query the same custody owner
    /// used by `sign_partial`; public transcript membership alone is insufficient.
    ///
    /// # Errors
    ///
    /// Returns a closed error for unavailable custody or a session/seat mismatch.
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    >;

    /// Sign the exact pulse payload for the supplied fully validated session.
    ///
    /// # Errors
    ///
    /// Returns a non-secret diagnostic when the requested session is absent,
    /// the provider cannot access its sealed share, or signing fails.
    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String>;

    /// Return whether the feature-isolated test daemon must corrupt this
    /// provider's outbound share after signing and before broadcast.
    ///
    /// This hook does not exist in ordinary builds. Its sole caller skips
    /// local reducer admission for the deliberately malformed share so the
    /// live network exercises receiver-side proof rejection without granting
    /// the faulty validator a hidden local contribution.
    #[cfg(feature = "test-network-parliament-signers")]
    #[doc(hidden)]
    fn test_network_emit_invalid_outbound_partial_v1(&self) -> bool {
        false
    }
}

/// Non-secret exact-session custody result from an authenticated runtime owner.
///
/// This value does not prove ledger activation or authorize a pulse. It must be
/// obtained through a live provider lookup and matched to the committed session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalThresholdBeaconPartialSigningCapabilityV1 {
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    signer_index: u16,
}

impl GlobalThresholdBeaconPartialSigningCapabilityV1 {
    /// Construct the public result after the provider has checked actual custody.
    ///
    /// # Errors
    ///
    /// Rejects a seat absent from the completely validated transcript.
    pub fn for_validated_session(
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        signer_index: u16,
    ) -> Result<Self, GlobalThresholdBeaconCapabilityErrorV1> {
        if !session
            .record()
            .public_shares
            .iter()
            .any(|share| share.index == signer_index)
        {
            return Err(GlobalThresholdBeaconCapabilityErrorV1::InvalidRequest);
        }
        Ok(Self {
            session_id: session.record().session_id,
            transcript_hash: session.record().transcript_hash,
            signer_index,
        })
    }

    /// Return the public key-session identity.
    #[must_use]
    pub const fn session_id(self) -> [u8; 32] {
        self.session_id
    }

    /// Return the exact public transcript hash.
    #[must_use]
    pub const fn transcript_hash(self) -> [u8; 32] {
        self.transcript_hash
    }

    /// Return the one-based signer seat.
    #[must_use]
    pub const fn signer_index(self) -> u16 {
        self.signer_index
    }

    /// Match the live result to the exact requested session and signer seat.
    #[must_use]
    pub fn matches(
        self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        signer_index: u16,
    ) -> bool {
        self.session_id == session.record().session_id
            && self.transcript_hash == session.record().transcript_hash
            && self.signer_index == signer_index
    }
}

/// Closed errors for non-signing global-beacon custody lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GlobalThresholdBeaconCapabilityErrorV1 {
    /// The secure runtime or authenticated lookup is unavailable.
    #[error("global beacon capability attestation is unavailable")]
    Unavailable,
    /// The exact session and signer seat are not owned by this provider.
    #[error("global beacon capability is not owned")]
    NotOwned,
    /// The requested seat is absent from the validated public transcript.
    #[error("global beacon capability request is invalid")]
    InvalidRequest,
}

/// Process-local zeroizing software owner for one adaptive beacon signing share.
///
/// This is an injection adapter for deployments whose secure runtime unwraps a
/// share into process memory. It deliberately has no `Clone`, `Debug`, byte
/// export, or serialization implementation. Deployment-owned providers may
/// instead implement [`GlobalThresholdBeaconPartialSignerV1`] directly.
pub struct InMemoryGlobalThresholdBeaconPartialSignerV1 {
    session: ValidatedGlobalThresholdBeaconSessionV1,
    share: AdaptiveThresholdBlsSecretShare<BeaconPurpose>,
}

impl InMemoryGlobalThresholdBeaconPartialSignerV1 {
    /// Move an adaptive share retained by the secure DKG runtime into the live
    /// signer without exporting or persisting its scalar components.
    ///
    /// # Errors
    ///
    /// Returns a threshold-beacon error when the share was constructed for a
    /// different public session or transcript.
    pub fn from_validated_share(
        session: ValidatedGlobalThresholdBeaconSessionV1,
        share: AdaptiveThresholdBlsSecretShare<BeaconPurpose>,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        let import_challenge = Hash::new_from_chunks(&[
            b"iroha.global-threshold-beacon.runtime-share-import.v1\0",
            session.record.session_id.as_slice(),
            session.record.transcript_hash.as_slice(),
        ]);
        let partial = share.sign_payload(&session.transcript, import_challenge.as_ref())?;
        session
            .transcript
            .verify_partial_signature(import_challenge.as_ref(), &partial)?;
        Ok(Self { session, share })
    }

    /// Import one sealed share, validate it against the complete public DKG
    /// transcript, and consume the zeroizing component buffer.
    ///
    /// # Errors
    ///
    /// Returns a threshold-beacon validation error if the public session or
    /// secret share does not match the frozen transcript and participant seat.
    pub fn from_components(
        record: GlobalThresholdBeaconKeySessionV1,
        binding: &GlobalThresholdBeaconSessionBindingV1,
        signer_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        let session = validate_global_threshold_beacon_session_v1(record, binding)?;
        let share = AdaptiveThresholdBlsSecretShare::from_components(
            &session.transcript,
            signer_index,
            components[0],
            components[1],
            components[2],
        )?;
        Self::from_validated_share(session, share)
    }

    /// Return the one-based frozen DKG signer seat without exposing key material.
    #[must_use]
    pub const fn signer_index(&self) -> u16 {
        self.share.index()
    }

    /// Return the exact public DKG session owned by this adapter.
    #[must_use]
    pub const fn session_id(&self) -> [u8; 32] {
        self.session.record.session_id
    }
}

impl GlobalThresholdBeaconPartialSignerV1 for InMemoryGlobalThresholdBeaconPartialSignerV1 {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    > {
        let expected = GlobalThresholdBeaconPartialSigningCapabilityV1::for_validated_session(
            session,
            expected_signer_index,
        )?;
        if session.record() != self.session.record() || expected_signer_index != self.signer_index()
        {
            return Err(GlobalThresholdBeaconCapabilityErrorV1::NotOwned);
        }
        Ok(expected)
    }

    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        if session.record() != self.session.record() {
            return Err(
                "requested global beacon session does not match the sealed share".to_owned(),
            );
        }
        self.share
            .sign_payload(&self.session.transcript, payload)
            .map(|partial| global_threshold_beacon_partial_signature_dto_v1(&partial))
            .map_err(|error| format!("adaptive global beacon partial signing failed: {error}"))
    }
}

/// Process-local, zeroizing owner for active and retiring beacon signing shares.
///
/// The registry deliberately has no `Clone`, `Debug`, serialization, key-list,
/// or scalar-export surface. Exact-session lookup occurs under a read guard, so
/// a concurrent retirement waits until every in-flight signing call completes.
/// Removing a session synchronously drops its non-cloneable in-memory signer
/// and zeroizes the underlying adaptive scalar share.
pub struct RuntimeGlobalThresholdBeaconShareCustodyV1 {
    sessions: RwLock<BTreeMap<[u8; 32], InMemoryGlobalThresholdBeaconPartialSignerV1>>,
}

impl RuntimeGlobalThresholdBeaconShareCustodyV1 {
    /// Construct an empty, fail-closed runtime custody registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(BTreeMap::new()),
        }
    }

    /// Import one already-validated software share without implicit replacement.
    ///
    /// # Errors
    ///
    /// Returns a closed error if custody is unavailable or the exact key
    /// session is already present.
    pub fn insert_validated_share(
        &self,
        signer: InMemoryGlobalThresholdBeaconPartialSignerV1,
    ) -> Result<(), GlobalThresholdBeaconShareCustodyErrorV1> {
        let session_id = signer.session_id();
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| GlobalThresholdBeaconShareCustodyErrorV1::CustodyUnavailable)?;
        match sessions.entry(session_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(signer);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(GlobalThresholdBeaconShareCustodyErrorV1::SessionAlreadyPresent)
            }
        }
    }

    /// Validate and import one zeroizing scalar triple for a public DKG session.
    ///
    /// # Errors
    ///
    /// Returns a closed error for malformed public state, a mismatched share,
    /// duplicate custody, or an unavailable registry lock.
    pub fn import_components(
        &self,
        record: GlobalThresholdBeaconKeySessionV1,
        binding: &GlobalThresholdBeaconSessionBindingV1,
        signer_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Result<(), GlobalThresholdBeaconShareCustodyErrorV1> {
        let signer = InMemoryGlobalThresholdBeaconPartialSignerV1::from_components(
            record,
            binding,
            signer_index,
            components,
        )
        .map_err(|_| GlobalThresholdBeaconShareCustodyErrorV1::InvalidShare)?;
        self.insert_validated_share(signer)
    }

    /// Import scalar components against an exact public session committed by consensus.
    ///
    /// # Errors
    ///
    /// Returns a closed error when the key session is not committed, its share
    /// is invalid, it is already held, or custody is unavailable.
    pub fn import_committed_components(
        &self,
        state: &impl crate::state::StateReadOnly,
        session_id: [u8; 32],
        signer_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Result<(), GlobalThresholdBeaconShareCustodyErrorV1> {
        use crate::state::WorldReadOnly as _;
        let record = state
            .world()
            .global_beacon_key_sessions()
            .get(&session_id)
            .map(|record| record.session.clone())
            .ok_or(GlobalThresholdBeaconShareCustodyErrorV1::SessionNotCommitted)?;
        let binding = GlobalThresholdBeaconSessionBindingV1 {
            network_id: record.network_id,
            session_id: record.session_id,
            roster_hash: record.roster_hash,
            transcript_hash: record.transcript_hash,
        };
        self.import_components(record, &binding, signer_index, components)
    }

    /// Retire and zeroize one share after consensus has retired that key session.
    ///
    /// A write guard waits for all current signing readers. The committed view
    /// must no longer name the session as active, its lifecycle must contain a
    /// retirement height, and the current committed height must be strictly
    /// later than that retirement boundary.
    ///
    /// # Errors
    ///
    /// Returns a closed error if committed state still permits use, the session
    /// is absent, or custody is unavailable.
    pub fn retire_session(
        &self,
        state: &impl crate::state::StateReadOnly,
        session_id: [u8; 32],
    ) -> Result<(), GlobalThresholdBeaconShareCustodyErrorV1> {
        use crate::state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, WorldReadOnly as _};
        let committed_height = u64::try_from(state.height())
            .map_err(|_| GlobalThresholdBeaconShareCustodyErrorV1::InvalidCommittedState)?;
        if state
            .world()
            .global_beacon_active_session()
            .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
            == Some(&session_id)
        {
            return Err(GlobalThresholdBeaconShareCustodyErrorV1::SessionStillRequired);
        }
        let retired_at = state
            .world()
            .global_beacon_key_sessions()
            .get(&session_id)
            .and_then(|record| record.retired_at_height)
            .ok_or(GlobalThresholdBeaconShareCustodyErrorV1::SessionStillRequired)?;
        if committed_height <= retired_at {
            return Err(GlobalThresholdBeaconShareCustodyErrorV1::SessionStillRequired);
        }
        let retired = self
            .sessions
            .write()
            .map_err(|_| GlobalThresholdBeaconShareCustodyErrorV1::CustodyUnavailable)?
            .remove(&session_id)
            .ok_or(GlobalThresholdBeaconShareCustodyErrorV1::SessionNotPresent)?;
        drop(retired);
        Ok(())
    }
}

impl Default for RuntimeGlobalThresholdBeaconShareCustodyV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalThresholdBeaconPartialSignerV1 for RuntimeGlobalThresholdBeaconShareCustodyV1 {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    > {
        GlobalThresholdBeaconPartialSigningCapabilityV1::for_validated_session(
            session,
            expected_signer_index,
        )?;
        let sessions = self
            .sessions
            .read()
            .map_err(|_| GlobalThresholdBeaconCapabilityErrorV1::Unavailable)?;
        let signer = sessions
            .get(&session.record().session_id)
            .ok_or(GlobalThresholdBeaconCapabilityErrorV1::NotOwned)?;
        signer.attest_partial_signing_capability(session, expected_signer_index)
    }

    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| "global threshold-beacon custody is unavailable".to_owned())?;
        let signer = sessions
            .get(&session.record().session_id)
            .ok_or_else(|| "global threshold-beacon share is unavailable".to_owned())?;
        signer.sign_partial(session, payload)
    }
}

/// Closed runtime beacon share-custody failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GlobalThresholdBeaconShareCustodyErrorV1 {
    /// The runtime custody lock was unavailable.
    #[error("global threshold-beacon custody is unavailable")]
    CustodyUnavailable,
    /// The imported scalar share or public transcript was invalid.
    #[error("global threshold-beacon share is invalid")]
    InvalidShare,
    /// The exact key session is already held and cannot be replaced implicitly.
    #[error("global threshold-beacon session is already present")]
    SessionAlreadyPresent,
    /// The exact key session is not held by this process.
    #[error("global threshold-beacon session is not present")]
    SessionNotPresent,
    /// Consensus state does not contain the requested public key session.
    #[error("global threshold-beacon public key session is not committed")]
    SessionNotCommitted,
    /// The supplied committed view cannot support a safe retirement decision.
    #[error("committed state is invalid for global threshold-beacon retirement")]
    InvalidCommittedState,
    /// Consensus still permits this key session to be used.
    #[error("global threshold-beacon session is still required")]
    SessionStillRequired,
}

/// Convert one locally produced adaptive signature share into its wire DTO.
///
/// The returned DTO is still only a partial signature. A receiver must admit it
/// through [`GlobalThresholdBeaconPulseAggregatorV1::accept_partial`], which
/// reconstructs canonical points and verifies the complete representation proof
/// against the exact pulse payload and public DKG session.
#[must_use]
pub fn global_threshold_beacon_partial_signature_dto_v1(
    partial: &DasRenPartialSignature<BeaconPurpose>,
) -> GlobalThresholdBeaconPartialSignatureV1 {
    let (z_s, z_r, z_u) = partial.response_bytes();
    GlobalThresholdBeaconPartialSignatureV1 {
        session_id: *partial.session_id(),
        signer_index: partial.index(),
        signature_share: *partial.sigma_bytes(),
        proof: GlobalThresholdBeaconPartialSignatureProofV1 {
            x: *partial.proof_x_bytes(),
            y: *partial.proof_y_bytes(),
            z_s: *z_s,
            z_r: *z_r,
            z_u: *z_u,
        },
    }
}

fn adaptive_partial_signature_from_dto_v1(
    partial: &GlobalThresholdBeaconPartialSignatureV1,
) -> Result<DasRenPartialSignature<BeaconPurpose>, GlobalThresholdBeaconError> {
    Ok(DasRenPartialSignature::from_bytes(
        partial.session_id,
        partial.signer_index,
        partial.signature_share,
        partial.proof.x,
        partial.proof.y,
        partial.proof.z_s,
        partial.proof.z_r,
        partial.proof.z_u,
    )?)
}

/// Session- and pulse-bound reducer for adaptive threshold-beacon partials.
///
/// Only proof-verified partial signatures enter this reducer. Signer indices are
/// kept in a canonical ordered map, retransmissions of the same signature share
/// are idempotent even when their zero-knowledge proof uses fresh randomness,
/// and a second distinct signature share from one signer fails closed as
/// equivocation. Final
/// reconstruction uses the lexicographically first threshold of signer indices;
/// the final BLS signature and seed are nevertheless unique for every valid
/// threshold subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalThresholdBeaconPulseAggregatorV1 {
    session: ValidatedGlobalThresholdBeaconSessionV1,
    pulse: FinalizedGlobalThresholdBeaconPulseV1,
    payload: Vec<u8>,
    partials: BTreeMap<u16, DasRenPartialSignature<BeaconPurpose>>,
}

impl GlobalThresholdBeaconPulseAggregatorV1 {
    /// Open one exact height-bound pulse against a validated public DKG session.
    ///
    /// The finalized-chain anchor must be the block immediately before the pulse
    /// height. Its hash is supplied by the consensus finalized-chain journal and
    /// is covered by every partial signature.
    pub fn new(
        session: ValidatedGlobalThresholdBeaconSessionV1,
        height: u64,
        finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        session.ensure_adaptive_protocol_ready()?;
        if height == 0 {
            return Err(GlobalThresholdBeaconError::NonMonotonicPosition);
        }
        if finalized_chain_anchor.height.checked_add(1) != Some(height)
            || is_zero(finalized_chain_anchor.block_hash.as_ref())
        {
            return Err(GlobalThresholdBeaconError::FinalizedAnchorMismatch);
        }

        let record = session.record();
        let pulse = FinalizedGlobalThresholdBeaconPulseV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: record.network_id,
            session_id: record.session_id,
            roster_hash: record.roster_hash,
            transcript_hash: record.transcript_hash,
            height,
            round: GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1,
            finalized_chain_anchor,
            signature: [0; 48],
            seed: [0; 32],
            pulse_id: [0; 32],
        };
        let payload = global_threshold_beacon_pulse_payload_v1(&pulse);
        Ok(Self {
            session,
            pulse,
            payload,
            partials: BTreeMap::new(),
        })
    }

    /// Return the immutable fully consuming payload signed in this round.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Borrow the proof-revalidated public key session for this pulse.
    #[must_use]
    pub const fn session(&self) -> &ValidatedGlobalThresholdBeaconSessionV1 {
        &self.session
    }

    /// Return the number of distinct proof-verified signer shares admitted.
    #[must_use]
    pub fn verified_partial_count(&self) -> usize {
        self.partials.len()
    }

    /// Verify and admit one authenticated partial-signature DTO.
    ///
    /// Returns `true` for a newly admitted signer and `false` when the same
    /// verified signature share is retried, including with fresh proof randomness.
    pub fn accept_partial(
        &mut self,
        partial: GlobalThresholdBeaconPartialSignatureV1,
    ) -> Result<bool, GlobalThresholdBeaconError> {
        if partial.session_id != self.pulse.session_id {
            return Err(GlobalThresholdBeaconError::SessionMismatch);
        }
        let partial = adaptive_partial_signature_from_dto_v1(&partial)?;
        self.session
            .transcript
            .verify_partial_signature(&self.payload, &partial)?;
        match self.partials.get(&partial.index()) {
            Some(previous) if previous.sigma_bytes() == partial.sigma_bytes() => Ok(false),
            Some(_) => Err(GlobalThresholdBeaconError::PartialSignatureEquivocation),
            None => {
                self.partials.insert(partial.index(), partial);
                Ok(true)
            }
        }
    }

    /// Reconstruct, final-verify, and return the unique public pulse.
    pub fn finalize(
        &self,
    ) -> Result<FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconError> {
        let threshold = usize::from(self.session.transcript.session().threshold());
        if self.partials.len() < threshold {
            return Err(GlobalThresholdBeaconError::InsufficientPartialSignatures);
        }
        let canonical_subset = self
            .partials
            .values()
            .take(threshold)
            .copied()
            .collect::<Vec<_>>();
        let signature = self
            .session
            .transcript
            .combine_partial_signatures(&self.payload, &canonical_subset)?;
        let mut pulse = self.pulse;
        pulse.signature = *signature.as_bytes();
        pulse.seed = self
            .session
            .transcript
            .finalized_seed(&self.payload, &signature)?;
        pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);
        verify_finalized_global_threshold_beacon_pulse_v1(
            &self.session,
            &pulse,
            self.pulse.finalized_chain_anchor,
        )?;
        Ok(pulse)
    }
}

/// Validate a decoded global threshold-beacon key-session record.
///
/// The complete public transcript is reconstructed with the fixed
/// [`BeaconPurpose`] type. This makes it impossible to admit a Parliament TLE
/// key in the beacon role even if all raw bytes happen to match.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError`] for any version, external binding,
/// point encoding, participant ordering, or transcript commitment mismatch.
pub fn validate_global_threshold_beacon_session_v1(
    record: GlobalThresholdBeaconKeySessionV1,
    expected: &GlobalThresholdBeaconSessionBindingV1,
) -> Result<ValidatedGlobalThresholdBeaconSessionV1, GlobalThresholdBeaconError> {
    if record.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1 {
        return Err(GlobalThresholdBeaconError::UnsupportedVersion {
            actual: record.version,
        });
    }
    if record.network_id != expected.network_id {
        return Err(GlobalThresholdBeaconError::NetworkMismatch);
    }
    if record.session_id != expected.session_id {
        return Err(GlobalThresholdBeaconError::SessionMismatch);
    }
    if record.roster_hash != expected.roster_hash {
        return Err(GlobalThresholdBeaconError::RosterMismatch);
    }
    if record.transcript_hash != expected.transcript_hash {
        return Err(GlobalThresholdBeaconError::TranscriptMismatch);
    }
    validate_adaptive_dkg_shape(&record)?;

    let transcript = reconstruct_adaptive_beacon_transcript(&record)?;

    Ok(ValidatedGlobalThresholdBeaconSessionV1 { record, transcript })
}

/// Decode and validate one canonical Norito key-session envelope.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError`] when decoding, canonical re-encoding,
/// or typed session validation fails.
pub fn decode_global_threshold_beacon_session_v1(
    encoded: &[u8],
    expected: &GlobalThresholdBeaconSessionBindingV1,
) -> Result<ValidatedGlobalThresholdBeaconSessionV1, GlobalThresholdBeaconError> {
    let record: GlobalThresholdBeaconKeySessionV1 = norito::decode_from_bytes(encoded)
        .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    let canonical =
        norito::to_bytes(&record).map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    if canonical != encoded {
        return Err(GlobalThresholdBeaconError::NonCanonicalEncoding);
    }
    validate_global_threshold_beacon_session_v1(record, expected)
}

/// Build the exact fully consuming payload signed by a finalized beacon pulse.
///
/// The signature, derived seed, and derived pulse ID are omitted because each
/// depends on this payload. All integer fields are encoded big-endian and every
/// remaining field has a fixed width. No earlier pulse identifier or seed is
/// included: every `(session, height, fixed round, finalized parent)` slot is
/// independently unique, so skipping an optional governance slot cannot alter
/// a later mandatory NPoS pulse.
#[must_use]
pub fn global_threshold_beacon_pulse_payload_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
) -> Vec<u8> {
    let mut payload =
        Vec::with_capacity(GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 5 + 8 * 3);
    payload.extend_from_slice(GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1);
    payload.extend_from_slice(&pulse.version.to_be_bytes());
    payload.extend_from_slice(pulse.network_id.as_bytes());
    payload.extend_from_slice(&pulse.session_id);
    payload.extend_from_slice(&pulse.roster_hash);
    payload.extend_from_slice(&pulse.transcript_hash);
    payload.extend_from_slice(&pulse.height.to_be_bytes());
    payload.extend_from_slice(&pulse.round.to_be_bytes());
    payload.extend_from_slice(&pulse.finalized_chain_anchor.height.to_be_bytes());
    payload.extend_from_slice(pulse.finalized_chain_anchor.block_hash.as_ref());
    payload
}

/// Public slot recovered from one canonical threshold-beacon signing payload.
///
/// This is the credential-free projection used when a runtime signer lives
/// behind the authenticated provider broker. The complete public key session
/// travels alongside it; no private DKG component is included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalThresholdBeaconPulseSigningSlotV1 {
    /// Exact finalized height whose pulse is being produced.
    pub height: u64,
    /// Exact finalized parent authenticated by the pulse.
    pub finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1,
}

/// Recover and validate the exact public slot from a beacon signing payload.
///
/// The parser accepts only the fixed V1 payload length, canonical fixed round,
/// and a byte-for-byte payload reconstructed from `session`, `height`, and the
/// finalized-chain anchor. This prevents a broker client from using the beacon
/// provider as a generic threshold-BLS signing oracle.
///
/// # Errors
///
/// Returns a closed threshold-beacon error for a truncated, extended, foreign,
/// view-dependent, or otherwise noncanonical payload.
pub fn global_threshold_beacon_pulse_signing_slot_v1(
    session: &ValidatedGlobalThresholdBeaconSessionV1,
    payload: &[u8],
) -> Result<GlobalThresholdBeaconPulseSigningSlotV1, GlobalThresholdBeaconError> {
    let expected_len = GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 5 + 8 * 3;
    if payload.len() != expected_len {
        return Err(GlobalThresholdBeaconError::InvalidEncoding);
    }
    let height_offset = payload.len() - (8 * 3 + 32);
    let round_offset = height_offset + 8;
    let anchor_height_offset = round_offset + 8;
    let anchor_hash_offset = anchor_height_offset + 8;
    let height = u64::from_be_bytes(
        payload[height_offset..round_offset]
            .try_into()
            .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?,
    );
    let round = u64::from_be_bytes(
        payload[round_offset..anchor_height_offset]
            .try_into()
            .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?,
    );
    if round != GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1 {
        return Err(GlobalThresholdBeaconError::NonCanonicalRound);
    }
    let anchor_height = u64::from_be_bytes(
        payload[anchor_height_offset..anchor_hash_offset]
            .try_into()
            .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?,
    );
    let anchor_hash: [u8; 32] = payload[anchor_hash_offset..]
        .try_into()
        .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    let finalized_chain_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: anchor_height,
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed(anchor_hash)),
    };
    let reconstructed = GlobalThresholdBeaconPulseAggregatorV1::new(
        session.clone(),
        height,
        finalized_chain_anchor,
    )?;
    if reconstructed.payload() != payload {
        return Err(GlobalThresholdBeaconError::InvalidEncoding);
    }
    Ok(GlobalThresholdBeaconPulseSigningSlotV1 {
        height,
        finalized_chain_anchor,
    })
}

/// Derive the canonical pulse identifier after final-signature verification.
#[must_use]
pub fn global_threshold_beacon_pulse_id_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    verified_seed: [u8; 32],
) -> [u8; 32] {
    let payload = global_threshold_beacon_pulse_payload_v1(pulse);
    let mut preimage = Vec::with_capacity(
        GLOBAL_BEACON_PULSE_ID_DOMAIN_V1.len() + 4 + payload.len() + pulse.signature.len() + 32,
    );
    preimage.extend_from_slice(GLOBAL_BEACON_PULSE_ID_DOMAIN_V1);
    preimage.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    preimage.extend_from_slice(&payload);
    preimage.extend_from_slice(&pulse.signature);
    preimage.extend_from_slice(&verified_seed);
    *Hash::new(&preimage).as_ref()
}

/// Derive the NPoS successor seed from one already-verified global beacon pulse.
///
/// The dedicated domain prevents a pulse seed consumed by Parliament or another
/// protocol from being reused as the raw NPoS PRF key. The target boundary and
/// successor epoch are explicit even though the pulse identifier already binds
/// its signed position; this makes accidental cross-epoch reuse impossible at
/// the consensus call site.
#[must_use]
pub fn global_threshold_beacon_npos_successor_seed_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    boundary_height: u64,
    successor_epoch: u64,
) -> [u8; 32] {
    let boundary_height = boundary_height.to_be_bytes();
    let successor_epoch = successor_epoch.to_be_bytes();
    *Hash::new_from_chunks(&[
        GLOBAL_BEACON_NPOS_SUCCESSOR_SEED_DOMAIN_V1,
        pulse.network_id.as_bytes(),
        pulse.session_id.as_slice(),
        pulse.pulse_id.as_slice(),
        pulse.seed.as_slice(),
        pulse.height.to_be_bytes().as_slice(),
        pulse.finalized_chain_anchor.height.to_be_bytes().as_slice(),
        boundary_height.as_slice(),
        successor_epoch.as_slice(),
    ])
    .as_ref()
}

/// Derive one lane-relay committee seed from an already-verified global pulse.
#[must_use]
pub fn global_threshold_beacon_lane_relay_seed_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    block_height: u64,
    dataspace_id: u64,
    lane_id: u32,
) -> [u8; 32] {
    *Hash::new_from_chunks(&[
        GLOBAL_BEACON_LANE_RELAY_SEED_DOMAIN_V1,
        pulse.network_id.as_bytes(),
        pulse.session_id.as_slice(),
        pulse.pulse_id.as_slice(),
        pulse.seed.as_slice(),
        pulse.height.to_be_bytes().as_slice(),
        block_height.to_be_bytes().as_slice(),
        dataspace_id.to_be_bytes().as_slice(),
        lane_id.to_be_bytes().as_slice(),
    ])
    .as_ref()
}

/// Derive a governance-sortition seed from an already-verified global pulse.
#[must_use]
pub fn global_threshold_beacon_governance_seed_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    epoch: u64,
) -> [u8; 32] {
    *Hash::new_from_chunks(&[
        GLOBAL_BEACON_GOVERNANCE_SEED_DOMAIN_V1,
        pulse.network_id.as_bytes(),
        pulse.session_id.as_slice(),
        pulse.pulse_id.as_slice(),
        pulse.seed.as_slice(),
        pulse.height.to_be_bytes().as_slice(),
        epoch.to_be_bytes().as_slice(),
    ])
    .as_ref()
}

/// Re-verify a persisted pulse before deriving Parliament sortition/release entropy.
///
/// This is the restored-state trust boundary for governance: an authenticated
/// snapshot or storage image is not treated as a substitute for public DKG and
/// final threshold-signature verification.
///
/// # Errors
///
/// Returns a threshold-beacon error when the stored pulse is absent, belongs
/// to another network or height, references an invalid key session, or fails
/// final signature/seed verification.
pub(crate) fn verified_persisted_global_threshold_beacon_governance_seed_v1(
    world: &impl crate::state::WorldReadOnly,
    network_id: &NetworkId,
    pulse: FinalizedGlobalThresholdBeaconPulseV1,
    height: u64,
) -> Result<[u8; 32], GlobalThresholdBeaconError> {
    if pulse.height != height {
        return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
    }
    let pulse = verified_persisted_global_threshold_beacon_pulse_v1(world, network_id, pulse)?;
    Ok(global_threshold_beacon_governance_seed_v1(&pulse, height))
}

/// Validate the canonical public shape of a persisted finalized pulse.
///
/// This checks all inert/replay bindings, the compressed signature encoding,
/// and the deterministic identifier. Full BLS verification remains mandatory
/// in [`verify_finalized_global_threshold_beacon_pulse_v1`] before insertion.
pub(crate) fn validate_persisted_global_threshold_beacon_pulse_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
) -> Result<GlobalThresholdBeaconPulseLinkV1, GlobalThresholdBeaconError> {
    if pulse.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1 {
        return Err(GlobalThresholdBeaconError::UnsupportedVersion {
            actual: pulse.version,
        });
    }
    if pulse.round != GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1 {
        return Err(GlobalThresholdBeaconError::NonCanonicalRound);
    }
    if pulse.height == 0
        || is_zero(&pulse.pulse_id)
        || is_zero(&pulse.seed)
        || is_zero(&pulse.roster_hash)
        || is_zero(&pulse.transcript_hash)
        || is_zero(pulse.finalized_chain_anchor.block_hash.as_ref())
    {
        return Err(GlobalThresholdBeaconError::ZeroPulse);
    }
    ThresholdBlsSignature::<BeaconPurpose>::from_bytes(pulse.session_id, &pulse.signature)?;
    if global_threshold_beacon_pulse_id_v1(pulse, pulse.seed) != pulse.pulse_id {
        return Err(GlobalThresholdBeaconError::PulseIdMismatch);
    }
    Ok(GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: pulse.pulse_id,
        seed: pulse.seed,
        height: pulse.height,
        round: pulse.round,
    })
}

/// Verify one decoded finalized pulse against authoritative session and chain state.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError`] for an inert/replayed/nonmonotonic
/// pulse, any binding mismatch, malformed signature, signature failure, or a
/// non-canonical derived seed/identifier.
pub fn verify_finalized_global_threshold_beacon_pulse_v1(
    session: &ValidatedGlobalThresholdBeaconSessionV1,
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    expected_anchor: GlobalThresholdBeaconChainAnchorV1,
) -> Result<GlobalThresholdBeaconPulseLinkV1, GlobalThresholdBeaconError> {
    if pulse.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1 {
        return Err(GlobalThresholdBeaconError::UnsupportedVersion {
            actual: pulse.version,
        });
    }
    if pulse.round != GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1 {
        return Err(GlobalThresholdBeaconError::NonCanonicalRound);
    }
    let session_record = session.record();
    if pulse.network_id != session_record.network_id {
        return Err(GlobalThresholdBeaconError::NetworkMismatch);
    }
    if pulse.session_id != session_record.session_id {
        return Err(GlobalThresholdBeaconError::SessionMismatch);
    }
    if pulse.roster_hash != session_record.roster_hash {
        return Err(GlobalThresholdBeaconError::RosterMismatch);
    }
    if pulse.transcript_hash != session_record.transcript_hash {
        return Err(GlobalThresholdBeaconError::TranscriptMismatch);
    }
    if pulse.height == 0
        || is_zero(&pulse.pulse_id)
        || is_zero(&pulse.seed)
        || is_zero(pulse.finalized_chain_anchor.block_hash.as_ref())
    {
        return Err(GlobalThresholdBeaconError::ZeroPulse);
    }
    if pulse.finalized_chain_anchor != expected_anchor {
        return Err(GlobalThresholdBeaconError::FinalizedAnchorMismatch);
    }

    let signature =
        ThresholdBlsSignature::<BeaconPurpose>::from_bytes(pulse.session_id, &pulse.signature)?;
    let payload = global_threshold_beacon_pulse_payload_v1(pulse);
    let seed = session.transcript.finalized_seed(&payload, &signature)?;
    session.ensure_adaptive_protocol_ready()?;
    if is_zero(&seed) || seed != pulse.seed {
        return Err(GlobalThresholdBeaconError::SeedMismatch);
    }
    let pulse_id = global_threshold_beacon_pulse_id_v1(pulse, seed);
    if pulse_id != pulse.pulse_id {
        return Err(GlobalThresholdBeaconError::PulseIdMismatch);
    }
    Ok(GlobalThresholdBeaconPulseLinkV1 {
        pulse_id,
        seed,
        height: pulse.height,
        round: pulse.round,
    })
}

/// Re-verify one persisted global pulse and its complete public DKG session.
pub(crate) fn verified_persisted_global_threshold_beacon_pulse_v1(
    world: &impl crate::state::WorldReadOnly,
    network_id: &NetworkId,
    pulse: FinalizedGlobalThresholdBeaconPulseV1,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconError> {
    if world.global_beacon_pulses().get(&pulse.pulse_id) != Some(&pulse)
        || world.global_beacon_pulse_slots().get(&(
            iroha_data_model::governance::types::BeaconSessionId::for_network_v1(&pulse.network_id),
            pulse.height,
        )) != Some(&pulse.pulse_id)
        || pulse.network_id != *network_id
        || pulse.finalized_chain_anchor.height.checked_add(1) != Some(pulse.height)
    {
        return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
    }
    let key_record = world
        .global_beacon_key_sessions()
        .get(&pulse.session_id)
        .ok_or(GlobalThresholdBeaconError::ActiveKeyMismatch)?;
    if !key_record.is_active_at(pulse.height) {
        return Err(GlobalThresholdBeaconError::ActiveKeyMismatch);
    }
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: *network_id,
        session_id: pulse.session_id,
        roster_hash: pulse.roster_hash,
        transcript_hash: pulse.transcript_hash,
    };
    let session =
        validate_global_threshold_beacon_session_v1(key_record.session.clone(), &binding)?;
    let verified = verify_finalized_global_threshold_beacon_pulse_v1(
        &session,
        &pulse,
        pulse.finalized_chain_anchor,
    )?;
    if validate_persisted_global_threshold_beacon_pulse_v1(&pulse)? != verified {
        return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
    }
    Ok(pulse)
}

/// Re-verify the newest persisted global pulse at or before a block height.
///
/// Selection is deterministic and fails closed if restored state contains two
/// pulses at the selected height. The returned pulse is then checked against
/// its complete public DKG session and final threshold signature.
///
/// # Errors
///
/// Returns a threshold-beacon error when no eligible pulse exists, the selected
/// height is ambiguous, or the selected pulse fails persisted-state, session,
/// signature, seed, or identifier verification.
pub fn verified_global_threshold_beacon_pulse_at_or_before_v1(
    world: &impl crate::state::WorldReadOnly,
    network_id: &NetworkId,
    maximum_height: u64,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconError> {
    let mut selected = None;
    for (_, candidate) in world.global_beacon_pulses().iter() {
        if candidate.height > maximum_height {
            continue;
        }
        match selected {
            None => selected = Some(*candidate),
            Some(current) if candidate.height > current.height => selected = Some(*candidate),
            Some(current) if candidate.height == current.height => {
                return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
            }
            Some(_) => {}
        }
    }
    let pulse = selected.ok_or(GlobalThresholdBeaconError::InvalidPulseHistory)?;
    verified_persisted_global_threshold_beacon_pulse_v1(world, network_id, pulse)
}

/// Re-verify the latest persisted global pulse and its complete public session.
///
/// This is the shared read boundary for deterministic consumers outside the
/// Sumeragi epoch transition. The pulse must be the unique history tail and
/// must not come from the future relative to `maximum_height`. The finalized
/// chain anchor remains covered by the threshold signature; insertion into
/// this authoritative store is restricted to the consensus effect corridor,
/// which independently binds that anchor to the candidate's exact parent hash.
pub(crate) fn verified_latest_global_threshold_beacon_pulse_v1(
    world: &impl crate::state::WorldReadOnly,
    network_id: &NetworkId,
    maximum_height: u64,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconError> {
    let latest = world
        .global_beacon_latest_pulse()
        .get(&crate::state::GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .copied()
        .ok_or(GlobalThresholdBeaconError::InvalidPulseHistory)?;
    let pulse = world
        .global_beacon_pulses()
        .get(&latest.pulse_id)
        .copied()
        .ok_or(GlobalThresholdBeaconError::InvalidPulseHistory)?;
    if pulse.height > maximum_height
        || world
            .global_beacon_pulses()
            .iter()
            .any(|(_, candidate)| (candidate.height, candidate.round) > (pulse.height, pulse.round))
    {
        return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
    }
    let pulse = verified_persisted_global_threshold_beacon_pulse_v1(world, network_id, pulse)?;
    if validate_persisted_global_threshold_beacon_pulse_v1(&pulse)? != latest {
        return Err(GlobalThresholdBeaconError::InvalidPulseHistory);
    }
    Ok(pulse)
}

/// Decode and verify one canonical Norito finalized-pulse envelope.
///
/// # Errors
///
/// Returns [`GlobalThresholdBeaconError`] when decoding, canonical re-encoding,
/// or pulse verification fails.
pub fn decode_finalized_global_threshold_beacon_pulse_v1(
    encoded: &[u8],
    session: &ValidatedGlobalThresholdBeaconSessionV1,
    expected_anchor: GlobalThresholdBeaconChainAnchorV1,
) -> Result<GlobalThresholdBeaconPulseLinkV1, GlobalThresholdBeaconError> {
    let pulse: FinalizedGlobalThresholdBeaconPulseV1 = norito::decode_from_bytes(encoded)
        .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    let canonical =
        norito::to_bytes(&pulse).map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    if canonical != encoded {
        return Err(GlobalThresholdBeaconError::NonCanonicalEncoding);
    }
    verify_finalized_global_threshold_beacon_pulse_v1(session, &pulse, expected_anchor)
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(any(test, feature = "iroha-core-tests"))]
mod fixtures;
#[cfg(any(test, feature = "iroha-core-tests"))]
pub use fixtures::signed_persisted_pulse_fixture_for_world;
#[cfg(test)]
pub(crate) use fixtures::signed_pulses_fixture_for_roster_and_anchors;

#[cfg(test)]
pub(crate) mod tests;
