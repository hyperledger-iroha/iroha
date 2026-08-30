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
    peer::PeerId,
};
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
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
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
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
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

/// Process-local zeroizing software owner for one adaptive beacon signing share.
///
/// This is an injection adapter for deployments whose secure runtime unwraps a
/// share into process memory. It deliberately has no `Clone`, `Debug`, byte
/// export, or serialization implementation. HSM/KMS integrations may instead
/// implement [`GlobalThresholdBeaconPartialSignerV1`] directly.
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
        || world
            .global_beacon_pulse_slots()
            .get(&(pulse.network_id, pulse.height))
            != Some(&pulse.pulse_id)
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
fn beacon_fixture_network_id(marker: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([marker; Hash::LENGTH]),
    ))
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn adaptive_dkg_session_fixture() -> GlobalThresholdBeaconDkgSessionV1 {
    GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: beacon_fixture_network_id(0x81),
        session_id: [0x22; 32],
        roster_hash: [0x33; 32],
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        sharing_end_height: 10,
        complaints_end_height: 20,
        responses_end_height: 30,
    }
}

#[cfg(any(test, feature = "iroha-core-tests"))]
struct AdaptiveBeaconFixture {
    session: ValidatedGlobalThresholdBeaconSessionV1,
    #[cfg(test)]
    binding: GlobalThresholdBeaconSessionBindingV1,
    parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
    dealer_secrets: Vec<DasRenDealerSecret<BeaconPurpose>>,
    dealer_commitments: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn dealer_commitment_dto(
    dealer: &ValidatedDealerCommitment<BeaconPurpose>,
) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
    GlobalThresholdBeaconDkgDealerCommitmentV1 {
        dealer_index: dealer.dealer_index(),
        coefficient_commitments: dealer
            .coefficients()
            .iter()
            .map(|coefficient| *coefficient.as_bytes())
            .collect(),
        constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
            commitment: *dealer.constant_proof().commitment_bytes(),
            response: *dealer.constant_proof().response_bytes(),
        },
    }
}

#[cfg(test)]
fn adaptive_beacon_fixture() -> AdaptiveBeaconFixture {
    adaptive_beacon_fixture_for_session(adaptive_dkg_session_fixture())
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn adaptive_beacon_fixture_for_session(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
) -> AdaptiveBeaconFixture {
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let parameters = adaptive_beacon_parameters(&dkg_session).expect("adaptive parameters");
    let mut state = GlobalThresholdBeaconDkgStateV1::new(dkg_session, &crypto)
        .expect("valid adaptive DKG state");
    let mut rng = StdRng::from_seed([0x5A; 32]);
    let mut dealer_secrets = Vec::new();
    let mut dealer_commitments = Vec::new();
    for dealer_index in 1_u16..=dkg_session.committee_size {
        let (secret, commitment) =
            DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                .expect("generate adaptive dealer");
        state
            .record_dealer_commitment(1, dealer_commitment_dto(&commitment), &crypto)
            .expect("verify adaptive dealer broadcast");
        dealer_secrets.push(secret);
        dealer_commitments.push(commitment);
    }
    let record = state
        .finalize(dkg_session.responses_end_height, &crypto)
        .expect("finalize adaptive DKG")
        .clone();
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    };
    let session = validate_global_threshold_beacon_session_v1(record, &binding)
        .expect("validate adaptive beacon transcript DTO");
    AdaptiveBeaconFixture {
        session,
        #[cfg(test)]
        binding,
        parameters,
        dealer_secrets,
        dealer_commitments,
    }
}

/// Build one fully signed, proof-valid persisted beacon fixture.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn signed_persisted_pulse_fixture_for_world(
    network_id: NetworkId,
    height: u64,
) -> (
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    FinalizedGlobalThresholdBeaconPulseV1,
) {
    assert!(height > 4, "fixture pulse follows DKG finalization");
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.sharing_end_height = 2;
    dkg_session.complaints_end_height = 3;
    dkg_session.responses_end_height = 4;
    dkg_session.session_id = Hash::new_from_chunks(&[
        b"iroha.beacon.world-test-session.v1\0",
        network_id.as_bytes(),
    ])
    .into();
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: height - 1,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x88; 32])),
    };
    let mut aggregator =
        GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), height, anchor)
            .expect("open exact world-test pulse reducer");
    let payload = aggregator.payload().to_vec();
    for recipient_index in 1_u16..=fixture.session.transcript.session().threshold() {
        let private_contributions = fixture
            .dealer_secrets
            .iter()
            .zip(&fixture.dealer_commitments)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&fixture.parameters, dealer, recipient_index)
                    .expect("verified private DKG contribution")
            })
            .collect::<Vec<_>>();
        let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            &fixture.session.transcript,
            &private_contributions,
        )
        .expect("aggregate exact qualified private contributions");
        aggregator
            .accept_partial(global_threshold_beacon_partial_signature_dto_v1(
                &signing_share
                    .sign_payload(&fixture.session.transcript, &payload)
                    .expect("sign exact world-test pulse payload"),
            ))
            .expect("accept proof-verified world-test partial");
    }
    let pulse = aggregator.finalize().expect("finalize world-test pulse");
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("construct world-test key lifecycle");
    key_record
        .activate(fixture.session.record().adaptive_dkg.finalized_at_height)
        .expect("activate world-test key at DKG finalization");
    (key_record, pulse)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        governance::parliament::{
            ParliamentAttemptStateV1, ParliamentDecisionModeV1, RequiredParliamentBodyV1,
        },
        kura::Kura,
        query::store::LiveQueryStore,
        state::{
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, MusubiResolverIndexRevisionV1, State, World,
            WorldReadOnly as _,
        },
        sumeragi::v2_beacon::{
            V2GlobalBeaconError, V2GlobalBeaconIngressOutcome, V2GlobalBeaconLifecycle,
        },
        sumeragi::v2_context::{
            V2ContextBuildError, finalized_global_beacon_npos_successor_seed_from_sources,
        },
    };
    use iroha_config::parameters::actual::{Governance, LaneConfig as RuntimeLaneConfig};
    use iroha_crypto::{
        Algorithm, HashOf, KeyPair,
        threshold_bls::{AdaptiveThresholdBlsSecretShare, TleReleasePurpose},
    };
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{BlockHeader, consensus_v2 as wire},
        consensus::{
            GlobalThresholdBeaconDkgComplaintReasonV1, GlobalThresholdBeaconDkgConstantProofV1,
            GlobalThresholdBeaconPublicShareV1, NposConsensusEffects,
        },
        governance::types::{
            BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, GovernanceAttemptId,
            GovernanceAttemptStatusV1, GovernanceAttemptV1, GovernanceExpectedHeadAbsentV1,
            GovernanceExpectedHeadV1, GovernanceStageV1, ParliamentBody, ProposalContentId,
            RiskTierV1, SortitionRequestId, SortitionRequestV1, parliament_candidate_root_v1,
        },
        musubi::MusubiRegistrySnapshotV1,
        peer::PeerId,
    };
    use rand::rngs::StdRng;
    use std::sync::Arc;

    struct AcceptingAdaptiveDkgCrypto;

    #[test]
    fn active_global_beacon_session_projection_rejects_noncanonical_storage() {
        let world = World::new();
        assert_eq!(
            active_global_threshold_beacon_session_id_v1(&world.view()),
            Ok(None)
        );

        let canonical = [0x31; 32];
        {
            let mut block = world.block();
            block
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, canonical);
            block.commit();
        }
        assert_eq!(
            active_global_threshold_beacon_session_id_v1(&world.view()),
            Ok(Some(canonical))
        );

        {
            let mut block = world.block();
            block
                .global_beacon_active_session
                .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
            block.global_beacon_active_session.insert(1, [0x41; 32]);
            block.commit();
        }
        assert_eq!(
            active_global_threshold_beacon_session_id_v1(&world.view()),
            Err(GlobalThresholdBeaconError::PersistenceConflict)
        );

        {
            let mut block = world.block();
            block
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, canonical);
            block.commit();
        }
        assert_eq!(
            active_global_threshold_beacon_session_id_v1(&world.view()),
            Err(GlobalThresholdBeaconError::PersistenceConflict)
        );
    }

    impl GlobalThresholdBeaconDkgCryptoV1 for AcceptingAdaptiveDkgCrypto {
        fn derive_generators(
            &self,
            session: &GlobalThresholdBeaconDkgSessionV1,
        ) -> Result<([u8; 96], [u8; 96]), ThresholdBlsError> {
            let parameters = adaptive_beacon_parameters(session)?;
            Ok((*parameters.h_bytes(), *parameters.v_bytes()))
        }

        fn verify_dealer_commitment(
            &self,
            _session: &GlobalThresholdBeaconDkgSessionV1,
            _generator_h: &[u8; 96],
            _generator_v: &[u8; 96],
            _commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
        ) -> Result<(), ThresholdBlsError> {
            Ok(())
        }

        fn verify_complaint_response(
            &self,
            _session: &GlobalThresholdBeaconDkgSessionV1,
            _generator_h: &[u8; 96],
            _generator_v: &[u8; 96],
            _commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
            _response: &GlobalThresholdBeaconDkgComplaintResponseV1,
        ) -> Result<(), ThresholdBlsError> {
            Ok(())
        }

        fn finalize_qualified_dealers(
            &self,
            session: &GlobalThresholdBeaconDkgSessionV1,
            _generator_h: &[u8; 96],
            _generator_v: &[u8; 96],
            _dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
            _qualified_dealers: &[u16],
            event_hash: [u8; 32],
        ) -> Result<GlobalThresholdBeaconDkgDerivedPublicV1, ThresholdBlsError> {
            Ok(GlobalThresholdBeaconDkgDerivedPublicV1 {
                group_public_key: g2_generator(),
                public_shares: (1..=session.committee_size)
                    .map(|index| GlobalThresholdBeaconPublicShareV1 {
                        index,
                        participant_seat_binding: [index as u8 + 0x40; 32],
                        public_key_share: g2_generator(),
                    })
                    .collect(),
                transcript_hash: event_hash,
            })
        }
    }

    fn adaptive_dkg_dealer_fixture(
        dealer_index: u16,
    ) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
        GlobalThresholdBeaconDkgDealerCommitmentV1 {
            dealer_index,
            coefficient_commitments: vec![g2_generator(), neg_g2_generator()],
            constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
                commitment: g2_generator(),
                response: [dealer_index as u8 + 0x30; 32],
            },
        }
    }

    #[test]
    fn adaptive_dkg_reducer_derives_qualification_after_complaint_resolution() {
        let crypto = AcceptingAdaptiveDkgCrypto;
        let session = adaptive_dkg_session_fixture();
        let mut state = GlobalThresholdBeaconDkgStateV1::new(session, &crypto)
            .expect("valid adaptive DKG state");
        for dealer_index in 1..=4 {
            state
                .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
                .expect("dealer commitment");
        }
        let dealer = state
            .dealer_commitments
            .get(&1)
            .expect("dealer one commitment");
        let dealer_commitment_hash =
            global_threshold_beacon_dkg_dealer_commitment_hash_v1(&session, dealer);
        let reason = GlobalThresholdBeaconDkgComplaintReasonV1::InvalidPrivateShare;
        let complaint_id = global_threshold_beacon_dkg_complaint_id_v1(
            &session,
            1,
            2,
            dealer_commitment_hash,
            reason,
        );
        state
            .record_complaint(
                10,
                GlobalThresholdBeaconDkgComplaintV1 {
                    dealer_index: 1,
                    complainant_index: 2,
                    dealer_commitment_hash,
                    reason,
                    complaint_id,
                },
            )
            .expect("complaint");
        state
            .record_complaint_response(
                20,
                GlobalThresholdBeaconDkgComplaintResponseV1 {
                    complaint_id,
                    dealer_index: 1,
                    recipient_index: 2,
                    s_share: [1; 32],
                    r_share: [2; 32],
                    u_share: [3; 32],
                },
                &crypto,
            )
            .expect("valid public response");
        let finalized = state.finalize(30, &crypto).expect("finalized DKG");
        assert_eq!(finalized.adaptive_dkg.qualified_dealers, vec![1, 2, 3, 4]);
        assert_eq!(
            state.phase_at(30),
            GlobalThresholdBeaconDkgPhaseV1::Finalized
        );
    }

    #[test]
    fn adaptive_dkg_reducer_rejects_phase_errors_equivocation_and_too_small_q() {
        let crypto = AcceptingAdaptiveDkgCrypto;
        let session = adaptive_dkg_session_fixture();
        let mut state = GlobalThresholdBeaconDkgStateV1::new(session, &crypto)
            .expect("valid adaptive DKG state");
        assert_eq!(
            state.record_dealer_commitment(10, adaptive_dkg_dealer_fixture(1), &crypto),
            Err(GlobalThresholdBeaconError::WrongDkgPhase)
        );
        for dealer_index in 1..=4 {
            state
                .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
                .expect("dealer commitment");
        }
        let mut equivocation = adaptive_dkg_dealer_fixture(1);
        equivocation.coefficient_commitments[0][0] ^= 1;
        assert_eq!(
            state.record_dealer_commitment(2, equivocation, &crypto),
            Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation)
        );
        for (dealer_index, complainant_index) in [(1, 3), (2, 4)] {
            let dealer = state
                .dealer_commitments
                .get(&dealer_index)
                .expect("dealer commitment");
            let dealer_commitment_hash =
                global_threshold_beacon_dkg_dealer_commitment_hash_v1(&session, dealer);
            let reason = GlobalThresholdBeaconDkgComplaintReasonV1::MissingPrivateShare;
            state
                .record_complaint(
                    10,
                    GlobalThresholdBeaconDkgComplaintV1 {
                        dealer_index,
                        complainant_index,
                        dealer_commitment_hash,
                        reason,
                        complaint_id: global_threshold_beacon_dkg_complaint_id_v1(
                            &session,
                            dealer_index,
                            complainant_index,
                            dealer_commitment_hash,
                            reason,
                        ),
                    },
                )
                .expect("unresolved complaint");
        }
        assert_eq!(
            state.finalize(30, &crypto),
            Err(GlobalThresholdBeaconError::InsufficientQualifiedDealers)
        );
        assert_eq!(state.phase_at(31), GlobalThresholdBeaconDkgPhaseV1::Aborted);
    }

    #[test]
    fn adaptive_dkg_public_snapshot_roundtrips_and_restores() {
        let crypto = AcceptingAdaptiveDkgCrypto;
        let session = adaptive_dkg_session_fixture();
        let mut state = GlobalThresholdBeaconDkgStateV1::new(session, &crypto)
            .expect("valid adaptive DKG state");
        for dealer_index in 1..=4 {
            state
                .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
                .expect("dealer commitment");
        }
        let snapshot = state.public_snapshot().expect("public snapshot");
        let bytes = norito::to_bytes(&snapshot).expect("encode public DKG snapshot");
        let binary: GlobalThresholdBeaconDkgSnapshotV1 =
            norito::decode_from_bytes(&bytes).expect("decode public DKG snapshot");
        binary.validate().expect("validate decoded DKG snapshot");
        assert_eq!(binary, snapshot);
        let json = norito::json::to_json(&snapshot).expect("encode public DKG snapshot JSON");
        let decoded_json: GlobalThresholdBeaconDkgSnapshotV1 =
            norito::json::from_str(&json).expect("decode public DKG snapshot JSON");
        assert_eq!(decoded_json, snapshot);
        let restored = GlobalThresholdBeaconDkgStateV1::from_snapshot(binary, &crypto)
            .expect("cryptographically restore public snapshot");
        assert_eq!(
            restored.public_snapshot().expect("restored snapshot"),
            snapshot
        );
    }

    #[test]
    fn finalized_key_lifecycle_is_strict_and_roundtrips() {
        let (validated, _) = validated_threshold_session();
        let mut record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(validated.record().clone())
                .expect("valid finalized public key record");
        let finalized_height = record.session.adaptive_dkg.finalized_at_height;
        assert_eq!(
            record.activate(finalized_height - 1),
            Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
        );
        record
            .activate(finalized_height)
            .expect("activate at finalization height");
        assert!(record.is_active_at(finalized_height));
        assert_eq!(
            record.retire(finalized_height),
            Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
        );
        record
            .retire(finalized_height + 1)
            .expect("strictly later retirement");
        assert!(!record.is_active_at(finalized_height + 1));
        let encoded = norito::to_bytes(&record).expect("encode key lifecycle");
        let decoded: FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode key lifecycle");
        decoded.validate().expect("validate decoded lifecycle");
        assert_eq!(decoded, record);
    }

    fn decode_fixed<const N: usize>(encoded: &str) -> [u8; N] {
        let compact = encoded
            .bytes()
            .filter(u8::is_ascii_hexdigit)
            .collect::<Vec<_>>();
        hex::decode(compact)
            .expect("hex test vector")
            .try_into()
            .unwrap_or_else(|bytes: Vec<u8>| panic!("expected {N} bytes, got {}", bytes.len()))
    }

    fn g2_generator() -> [u8; 96] {
        decode_fixed(
            "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049\
             334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051\
             c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8",
        )
    }

    fn neg_g2_generator() -> [u8; 96] {
        let mut encoded = g2_generator();
        encoded[0] ^= 0x20;
        encoded
    }

    fn wrong_g1_signature() -> [u8; 48] {
        decode_fixed(
            "97f1d3a73197d7942695638c4fa9ac0f\
             c3688c4f9774b905a14e3a3f171bac58\
             6c55e83ff97a1aeffb3af00adb22c6bb",
        )
    }

    pub(crate) fn finalized_key_session_fixture_for_context_v1(
        network_id: NetworkId,
        session_id: [u8; 32],
        roster_hash: [u8; 32],
    ) -> FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
        let mut dkg_session = adaptive_dkg_session_fixture();
        dkg_session.network_id = network_id;
        dkg_session.session_id = session_id;
        dkg_session.roster_hash = roster_hash;
        let fixture = adaptive_beacon_fixture_for_session(dkg_session);
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("proof-valid finalized global beacon key fixture")
    }

    fn validated_threshold_session() -> (
        ValidatedGlobalThresholdBeaconSessionV1,
        GlobalThresholdBeaconSessionBindingV1,
    ) {
        let fixture = adaptive_beacon_fixture();
        let validated = fixture.session;
        let expected = fixture.binding;
        (validated, expected)
    }

    fn pulse_fixture(
        session: &ValidatedGlobalThresholdBeaconSessionV1,
    ) -> (
        FinalizedGlobalThresholdBeaconPulseV1,
        GlobalThresholdBeaconPulseLinkV1,
        GlobalThresholdBeaconChainAnchorV1,
    ) {
        let cursor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x66; 32],
            seed: [0x77; 32],
            height: 0,
            round: 0,
        };
        let anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x88; 32])),
        };
        let record = session.record();
        (
            FinalizedGlobalThresholdBeaconPulseV1 {
                version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
                network_id: record.network_id,
                session_id: record.session_id,
                roster_hash: record.roster_hash,
                transcript_hash: record.transcript_hash,
                height: 41,
                round: 0,
                finalized_chain_anchor: anchor,
                signature: wrong_g1_signature(),
                seed: [0x99; 32],
                pulse_id: [0xAA; 32],
            },
            cursor,
            anchor,
        )
    }

    fn signed_pulse_fixture() -> (
        AdaptiveBeaconFixture,
        FinalizedGlobalThresholdBeaconPulseV1,
        GlobalThresholdBeaconPulseLinkV1,
        GlobalThresholdBeaconChainAnchorV1,
    ) {
        let fixture = adaptive_beacon_fixture();
        let (pulse, cursor, anchor) = pulse_fixture(&fixture.session);
        let partials = pulse_partial_signatures(&fixture, &pulse, [0xA7; 32]);
        let mut aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
            fixture.session.clone(),
            pulse.height,
            anchor,
        )
        .expect("open exact pulse reducer");
        for partial in partials.into_iter().take(usize::from(
            fixture.session.transcript.session().threshold(),
        )) {
            aggregator
                .accept_partial(partial)
                .expect("accept proof-verified partial");
        }
        let pulse = aggregator.finalize().expect("finalize unique pulse");
        (fixture, pulse, cursor, anchor)
    }

    fn pulse_partial_signatures(
        fixture: &AdaptiveBeaconFixture,
        pulse: &FinalizedGlobalThresholdBeaconPulseV1,
        rng_seed: [u8; 32],
    ) -> Vec<GlobalThresholdBeaconPartialSignatureV1> {
        let payload = global_threshold_beacon_pulse_payload_v1(&pulse);
        let mut proof_rng = StdRng::from_seed(rng_seed);
        let mut partials = Vec::new();

        for recipient_index in 1_u16..=fixture.session.transcript.session().committee_size() {
            let private_contributions = fixture
                .dealer_secrets
                .iter()
                .zip(&fixture.dealer_commitments)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&fixture.parameters, dealer, recipient_index)
                        .expect("verified private DKG contribution")
                })
                .collect::<Vec<_>>();
            let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                &fixture.session.transcript,
                &private_contributions,
            )
            .expect("aggregate exact qualified private contributions");
            partials.push(global_threshold_beacon_partial_signature_dto_v1(
                &signing_share
                    .sign_payload_with_rng(&fixture.session.transcript, &payload, &mut proof_rng)
                    .expect("adaptive signature share"),
            ));
        }
        partials
    }

    fn live_fixture_in_memory_signer(
        fixture: &AdaptiveBeaconFixture,
        recipient_index: u16,
    ) -> InMemoryGlobalThresholdBeaconPartialSignerV1 {
        let private_contributions = fixture
            .dealer_secrets
            .iter()
            .zip(&fixture.dealer_commitments)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&fixture.parameters, dealer, recipient_index)
                    .expect("verified private DKG contribution")
            })
            .collect::<Vec<_>>();
        let share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            &fixture.session.transcript,
            &private_contributions,
        )
        .expect("aggregate exact qualified private contributions");
        InMemoryGlobalThresholdBeaconPartialSignerV1::from_validated_share(
            fixture.session.clone(),
            share,
        )
        .expect("move the DKG share into the zeroizing runtime provider")
    }

    fn live_fixture_signer(
        fixture: &AdaptiveBeaconFixture,
        recipient_index: u16,
    ) -> Arc<dyn GlobalThresholdBeaconPartialSignerV1> {
        Arc::new(live_fixture_in_memory_signer(fixture, recipient_index))
    }

    #[cfg(feature = "test-network-parliament-signers")]
    struct InvalidOutboundTestBeaconSigner {
        inner: Arc<dyn GlobalThresholdBeaconPartialSignerV1>,
    }

    #[cfg(feature = "test-network-parliament-signers")]
    impl GlobalThresholdBeaconPartialSignerV1 for InvalidOutboundTestBeaconSigner {
        fn sign_partial(
            &self,
            session: &ValidatedGlobalThresholdBeaconSessionV1,
            payload: &[u8],
        ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
            self.inner.sign_partial(session, payload)
        }

        fn test_network_emit_invalid_outbound_partial_v1(&self) -> bool {
            true
        }
    }

    #[test]
    fn runtime_beacon_custody_selects_exact_rotating_session_and_rejects_replacement() {
        let fixture_a = adaptive_beacon_fixture();
        let mut session_b = adaptive_dkg_session_fixture();
        session_b.session_id = [0xB2; 32];
        let fixture_b = adaptive_beacon_fixture_for_session(session_b);
        let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
        custody
            .insert_validated_share(live_fixture_in_memory_signer(&fixture_a, 1))
            .expect("insert key-A share");
        custody
            .insert_validated_share(live_fixture_in_memory_signer(&fixture_b, 1))
            .expect("insert key-B share before key-A retirement");
        assert_eq!(
            custody.insert_validated_share(live_fixture_in_memory_signer(&fixture_a, 2)),
            Err(GlobalThresholdBeaconShareCustodyErrorV1::SessionAlreadyPresent),
        );

        for (fixture, height, anchor_byte) in [(&fixture_a, 41, 0xA1), (&fixture_b, 42, 0xB1)] {
            let anchor = GlobalThresholdBeaconChainAnchorV1 {
                height: height - 1,
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                    [anchor_byte; 32],
                )),
            };
            let mut aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
                fixture.session.clone(),
                height,
                anchor,
            )
            .expect("open rotating-session pulse");
            let partial = custody
                .sign_partial(&fixture.session, aggregator.payload())
                .expect("select exact session share");
            assert!(
                aggregator
                    .accept_partial(partial)
                    .expect("independently verify custody output")
            );
        }
    }

    fn live_producer_keys() -> Vec<KeyPair> {
        let mut keys = (1_u8..=4)
            .map(|marker| {
                KeyPair::try_from_seed(vec![marker; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        keys
    }

    pub(crate) fn pending_batched_sortition_attempt(
        network_id: &NetworkId,
        roster: &[PeerId],
        pulse_height: u64,
    ) -> (
        GovernanceAttemptId,
        Vec<SortitionRequestId>,
        ParliamentAttemptStateV1,
    ) {
        let proposal_content_id = ProposalContentId::new([0x41; 32]);
        let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
        let mut attempt = ParliamentAttemptStateV1::try_new(
            GovernanceAttemptV1 {
                id: governance_attempt_id,
                proposal_content_id,
                sequence: 0,
                risk_tier: RiskTierV1::Standard,
                stage: GovernanceStageV1::Qualification,
                status: GovernanceAttemptStatusV1::Active,
            },
            1,
            10,
            [0x42; 32],
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: [0x43; 32],
            }),
            vec![
                RequiredParliamentBodyV1 {
                    body: ParliamentBody::RulesCommittee,
                    decision_mode: ParliamentDecisionModeV1::PublicFinding,
                },
                RequiredParliamentBodyV1 {
                    body: ParliamentBody::PolicyJury,
                    decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
                },
            ],
        )
        .expect("construct pending Parliament attempt");
        attempt
            .complete_qualification(governance_attempt_id)
            .expect("enter the first required body stage");
        let mut candidates = roster
            .iter()
            .map(|peer| AccountId::new(peer.public_key().clone()))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        let mut request_ids = Vec::new();
        for body in [ParliamentBody::RulesCommittee, ParliamentBody::PolicyJury] {
            let election_attempt_id =
                BodyElectionAttemptId::derive_v1(governance_attempt_id, body, 0);
            let candidate_root =
                parliament_candidate_root_v1(governance_attempt_id, body, &candidates);
            let request = SortitionRequestV1::try_new_canonical(
                governance_attempt_id,
                election_attempt_id,
                body,
                candidate_root,
                u32::try_from(candidates.len()).expect("four candidates"),
                2,
                pulse_height - 10,
                pulse_height,
                BeaconSessionId::for_network_v1(network_id),
                None,
            )
            .expect("construct batched logical-beacon sortition request");
            request_ids.push(request.id);
            attempt
                .register_sortition_request(governance_attempt_id, 0, request, candidates.clone())
                .expect("register pending logical-beacon request");
        }
        request_ids.sort_unstable();
        (governance_attempt_id, request_ids, attempt)
    }

    fn live_producer_context(
        keys: &[KeyPair],
        network_id: NetworkId,
        parent_hash: HashOf<BlockHeader>,
    ) -> wire::HeightContext {
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let parent_round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"threshold beacon fixture parent context",
            ))),
            height: 40,
            view: 0,
        };
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 41,
            epoch: 7,
            epoch_end_height: 42,
            next_epoch_snapshot: None,
            snapshot_bootstrap: None,
            mode: wire::ConsensusMode::Npos,
            parent_commit_qc: Some(wire::QuorumCertificate {
                round: parent_round,
                proposal_round: parent_round,
                phase: wire::GlobalPhase::Commit,
                subject: wire::BlockSubject {
                    parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                        b"threshold beacon fixture grandparent",
                    ))),
                    block_hash: parent_hash,
                    payload_hash: Hash::new(b"threshold beacon fixture parent payload"),
                },
                execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"threshold beacon fixture parent state"),
                    Hash::new(b"threshold beacon fixture post state"),
                    Hash::new(b"threshold beacon fixture ordinary writes"),
                    1,
                    Hash::new(b"threshold beacon fixture executed block"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            }),
            quorum: wire::DualQuorum::from_roster(&roster).expect("four-validator quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"threshold beacon fixture nexus"),
            execution_policy_hash: Hash::new(b"threshold beacon fixture execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 3,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x91; 32],
        };
        context.validate().expect("valid live beacon context");
        context
    }

    fn live_producer_state(
        fixture: &AdaptiveBeaconFixture,
        cursor: GlobalThresholdBeaconPulseLinkV1,
        parent_hash: HashOf<BlockHeader>,
    ) -> State {
        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized public key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized public key");
        let world = World::new();
        {
            let mut block = world.block();
            block
                .global_beacon_key_sessions
                .insert(key_record.session.session_id, key_record.clone());
            block.global_beacon_active_session.insert(
                GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
                key_record.session.session_id,
            );
            block
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, cursor);
            block.commit();
        }
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("live-global-threshold-beacon-producer"),
            fixture.session.record().network_id,
        );
        for marker in 1_u8..40 {
            state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::prehashed(
                [marker; 32],
            )));
        }
        state.push_block_hash_for_testing(parent_hash);
        let snapshot_hashes = state
            .block_hashes
            .view()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let revision = MusubiResolverIndexRevisionV1::default();
        let checkpoint = MusubiRegistrySnapshotV1 {
            finalized_height: 1,
            finalized_block_hash: *snapshot_hashes
                .first()
                .expect("live-producer history has a genesis hash")
                .as_ref(),
            index_revision: revision.get(),
        };
        checkpoint
            .validate()
            .expect("valid live-producer genesis resolver checkpoint");
        {
            let mut block = state.world.block();
            assert!(
                block
                    .musubi_resolver_index_checkpoints
                    .insert(revision, checkpoint)
                    .is_none(),
                "live-producer fixture must install one genesis resolver checkpoint"
            );
            block.commit();
        }
        state
            .kura()
            .extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
            .expect("install verified live-producer snapshot prefix");
        state
    }

    fn beacon_partial_payload(
        message: wire::ConsensusMessageV2,
    ) -> wire::GlobalBeaconPartialSignature {
        match message.payload {
            wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(partial) => partial,
            other => panic!("expected global beacon partial, got {other:?}"),
        }
    }

    #[test]
    fn threshold_beacon_context_roster_binding_rejects_active_foreign_committee() {
        let keys = live_producer_keys();
        let roster = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut dkg_session = adaptive_dkg_session_fixture();
        dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture = adaptive_beacon_fixture_for_session(dkg_session);

        assert_eq!(
            authenticated_global_threshold_beacon_roster_hash_v1(fixture.session.record(), &roster,),
            Ok(fixture.session.record().roster_hash)
        );

        let mut reordered = roster.clone();
        reordered.swap(0, 1);
        assert_eq!(
            authenticated_global_threshold_beacon_roster_hash_v1(
                fixture.session.record(),
                &reordered,
            ),
            Err(GlobalThresholdBeaconError::RosterMismatch)
        );
        assert_eq!(
            authenticated_global_threshold_beacon_roster_hash_v1(
                fixture.session.record(),
                &roster[..roster.len() - 1],
            ),
            Err(GlobalThresholdBeaconError::RosterMismatch)
        );
    }

    #[test]
    fn parliament_requested_slot_survives_key_rotation_and_produces_authoritative_pulse() {
        let keys = live_producer_keys();
        let network_id = beacon_fixture_network_id(0xB1);
        let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD3; 32]));
        let mut context = live_producer_context(&keys, network_id, parent_hash);
        context.epoch_end_height = 50;
        context.validate().expect("valid non-boundary context");
        assert_eq!(
            context.height, 41,
            "the fixture is the first height after boundary block 40"
        );
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut predecessor_roster = roster.clone();
        predecessor_roster.rotate_left(1);
        assert_ne!(predecessor_roster, roster);

        let mut dkg_a = adaptive_dkg_session_fixture();
        dkg_a.network_id = network_id;
        dkg_a.session_id = [0xA1; 32];
        dkg_a.roster_hash = global_threshold_beacon_roster_hash_v1(&predecessor_roster);
        let fixture_a = adaptive_beacon_fixture_for_session(dkg_a);
        let cursor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x63; 32],
            seed: [0x64; 32],
            height: 0,
            round: 0,
        };
        let state = live_producer_state(&fixture_a, cursor, parent_hash);
        let (governance_attempt_id, request_ids, attempt) =
            pending_batched_sortition_attempt(&network_id, &roster, context.height);
        {
            let mut block = state.world.block();
            block
                .parliament_attempts
                .insert(governance_attempt_id, attempt);
            block.commit();
        }
        assert!(matches!(
            V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None),
            Err(V2GlobalBeaconError::RosterMismatch)
        ));

        let mut dkg_b = adaptive_dkg_session_fixture();
        dkg_b.network_id = network_id;
        dkg_b.session_id = [0xB2; 32];
        dkg_b.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture_b = adaptive_beacon_fixture_for_session(dkg_b);
        let mut key_a = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(
            fixture_a.session.record().clone(),
        )
        .expect("valid key A");
        key_a
            .activate(key_a.session.adaptive_dkg.finalized_at_height)
            .expect("activate key A");
        key_a
            .retire(context.height)
            .expect("retire the predecessor at the first successor height");
        let mut key_b = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(
            fixture_b.session.record().clone(),
        )
        .expect("valid key B");
        key_b
            .activate(context.height)
            .expect("activate replacement key B at the first successor height");
        {
            let mut block = state.world.block();
            block
                .global_beacon_key_sessions
                .insert(key_a.session.session_id, key_a);
            block
                .global_beacon_key_sessions
                .insert(key_b.session.session_id, key_b);
            block.global_beacon_active_session.insert(
                GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
                fixture_b.session.record().session_id,
            );
            block.commit();
        }

        let signers = (1_u16..=4)
            .map(|index| live_fixture_signer(&fixture_b, index))
            .collect::<Vec<_>>();
        let mut messages = Vec::new();
        for (index, signer) in signers.iter().enumerate() {
            let mut producer = V2GlobalBeaconLifecycle::open(
                &context,
                &state,
                Some(u32::try_from(index).expect("validator index")),
                Some(Arc::clone(signer)),
            )
            .expect("mandatory Parliament producer opens after key rotation");
            assert!(producer.pulse_requested());
            assert!(producer.pulse_required_for_consensus());
            producer.begin_round(0).expect("sign requested slot");
            messages.push(beacon_partial_payload(
                producer.take_outbound().pop().expect("local partial"),
            ));
        }

        let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
            .expect("open signerless validator reducer after key rotation");
        reducer.begin_round(0).expect("open routing view");
        let mut absent = NposConsensusEffects::default();
        assert!(matches!(
            reducer.attach_candidate_effects(0, &mut absent),
            Err(V2GlobalBeaconError::State(_))
        ));
        assert!(absent.finalized_global_beacon_pulse.is_none());
        let mut invalid_optional_share = messages[0].clone();
        invalid_optional_share.partial.signature_share[0] ^= 1;
        assert!(matches!(
            reducer.accept_partial(invalid_optional_share, &roster[0], 0),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::ThresholdBls(_)
            ))
        ));
        let mut still_absent = NposConsensusEffects::default();
        assert!(matches!(
            reducer.attach_candidate_effects(0, &mut still_absent),
            Err(V2GlobalBeaconError::State(_))
        ));
        assert!(still_absent.finalized_global_beacon_pulse.is_none());
        assert_eq!(
            reducer
                .accept_partial(messages[0].clone(), &roster[0], 0)
                .expect("first key-B share"),
            V2GlobalBeaconIngressOutcome::Accepted
        );
        assert_eq!(
            reducer
                .accept_partial(messages[1].clone(), &roster[1], 0)
                .expect("threshold key-B share"),
            V2GlobalBeaconIngressOutcome::Finalized
        );
        let pulse = reducer
            .finalized_pulse(0)
            .expect("requested pulse finalized");
        assert_eq!(pulse.session_id, fixture_b.session.record().session_id);
        let mut effects = NposConsensusEffects::default();
        reducer
            .attach_candidate_effects(0, &mut effects)
            .expect("attach reconstructed requested pulse");
        assert_eq!(effects.finalized_global_beacon_pulse, Some(pulse));

        let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: parent_hash,
        };
        {
            let mut block = state.world.block();
            {
                let mut transaction =
                    block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
                transaction
                    .verify_and_advance_global_beacon_pulse(
                        &fixture_b.session,
                        pulse,
                        expected_anchor,
                    )
                    .expect("persist replacement-key pulse");
                transaction.apply();
            }
            block.commit();
        }
        let mut attempt = state
            .world
            .view()
            .parliament_attempts()
            .get(&governance_attempt_id)
            .cloned()
            .expect("pending Parliament attempt");
        let governance = Governance {
            rules_committee_size: 2,
            policy_jury_size: 2,
            parliament_alternate_size: 2,
            ..Governance::default()
        };
        let pulse_id = BeaconPulseId::new(pulse.pulse_id);
        let pulse_output = global_threshold_beacon_governance_seed_v1(&pulse, pulse.height);
        assert_eq!(
            attempt.consume_sortition_pulse_batch(
                governance_attempt_id,
                request_ids.clone(),
                BeaconSessionId::new([0xEE; 32]),
                pulse.height,
                pulse_id,
                pulse_output,
                &network_id,
                &governance,
            ),
            Err(crate::governance::parliament::ParliamentReducerErrorV1::PulseBindingMismatch)
        );
        attempt
            .consume_sortition_pulse_batch(
                governance_attempt_id,
                request_ids,
                BeaconSessionId::for_network_v1(&network_id),
                pulse.height,
                pulse_id,
                pulse_output,
                &network_id,
                &governance,
            )
            .expect("logical request consumes replacement-key pulse");
    }

    fn assert_same_block_key_rotation_persists_requested_pulse(parliament_requested_slot: bool) {
        let keys = live_producer_keys();
        let network_id = beacon_fixture_network_id(if parliament_requested_slot {
            0xC1
        } else {
            0xC2
        });
        let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD4; 32]));
        let mut context = live_producer_context(&keys, network_id, parent_hash);
        if parliament_requested_slot {
            context.epoch_end_height = 50;
            context.validate().expect("valid optional-slot context");
        }
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();

        let mut dkg_a = adaptive_dkg_session_fixture();
        dkg_a.network_id = network_id;
        dkg_a.session_id = [0xA5; 32];
        dkg_a.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture_a = adaptive_beacon_fixture_for_session(dkg_a);
        let cursor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x65; 32],
            seed: [0x66; 32],
            height: 0,
            round: 0,
        };
        let mut state = live_producer_state(&fixture_a, cursor, parent_hash);
        let transient_governance_attempt_id = if parliament_requested_slot {
            let (governance_attempt_id, _request_ids, attempt) =
                pending_batched_sortition_attempt(&network_id, &roster, context.height);
            let mut block = state.world.block();
            block
                .parliament_attempts
                .insert(governance_attempt_id, attempt);
            block.commit();
            Some(governance_attempt_id)
        } else {
            None
        };

        let signers = (1_u16..=2)
            .map(|index| live_fixture_signer(&fixture_a, index))
            .collect::<Vec<_>>();
        let mut messages = Vec::new();
        for (index, signer) in signers.iter().enumerate() {
            let mut producer = V2GlobalBeaconLifecycle::open(
                &context,
                &state,
                Some(u32::try_from(index).expect("small validator index")),
                Some(Arc::clone(signer)),
            )
            .expect("open exact pre-transaction pulse producer");
            assert!(producer.pulse_requested());
            assert!(producer.pulse_required_for_consensus());
            producer.begin_round(0).expect("sign exact pulse slot");
            messages.push(beacon_partial_payload(
                producer.take_outbound().pop().expect("local pulse share"),
            ));
        }
        let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
            .expect("open exact signerless validator reducer");
        reducer.begin_round(0).expect("open pulse routing view");
        for (index, message) in messages.into_iter().enumerate() {
            reducer
                .accept_partial(message, &roster[index], 0)
                .expect("accept exact pre-transaction key share");
        }
        let pulse = reducer
            .finalized_pulse(0)
            .expect("threshold reconstructs the pre-transaction pulse");
        let mut effects = NposConsensusEffects::default();
        reducer
            .attach_candidate_effects(0, &mut effects)
            .expect("attach requested pre-transaction pulse");

        let mut dkg_b = adaptive_dkg_session_fixture();
        dkg_b.network_id = network_id;
        dkg_b.session_id = [0xB5; 32];
        dkg_b.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture_b = adaptive_beacon_fixture_for_session(dkg_b);
        let key_b = FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(
            fixture_b.session.record().clone(),
        )
        .expect("valid successor beacon key");
        let next_height = context
            .height
            .checked_add(1)
            .expect("next lifecycle height");
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(context.height).expect("nonzero pulse height"),
            Some(parent_hash),
            None,
            None,
            0,
            0,
        );
        let committed_hash = header.hash();
        let mut state_block = state.block(header);
        let mut transaction = state_block.transaction();
        transaction
            .world
            .retire_global_beacon_key_session(pulse.session_id, next_height)
            .expect("schedule predecessor retirement after the pulse height");
        transaction
            .world
            .put_finalized_global_beacon_key_session(key_b)
            .expect("persist proof-valid successor key");
        transaction
            .world
            .activate_global_beacon_key_session(fixture_b.session.record().session_id, next_height)
            .expect("schedule successor activation after the pulse height");
        let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: context.height - 1,
            block_hash: parent_hash,
        };
        let mut stale_roster = roster.clone();
        stale_roster.reverse();
        let stale_roster_error =
            crate::sumeragi::penalties::apply_npos_consensus_effects_to_transaction(
                &mut transaction,
                &effects,
                Some(expected_anchor),
                &stale_roster,
                context.height,
                0,
                0,
                #[cfg(feature = "telemetry")]
                None,
            )
            .err()
            .expect("a stale height roster must reject the otherwise valid pulse");
        assert!(
            stale_roster_error
                .to_string()
                .contains("authenticated height roster"),
            "unexpected stale-roster diagnostic: {stale_roster_error}"
        );
        crate::sumeragi::penalties::apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            Some(expected_anchor),
            &roster,
            context.height,
            0,
            0,
            #[cfg(feature = "telemetry")]
            None,
        )
        .expect("post-transaction rotation must preserve the parent-authorized pulse");
        if let Some(governance_attempt_id) = transient_governance_attempt_id {
            assert!(
                transaction
                    .world
                    .remove_parliament_attempt_for_testing(&governance_attempt_id)
                    .is_some(),
                "transient logical-pulse request must remain present through effect application"
            );
        }
        transaction.apply();
        state_block
            .commit_world_overlay_for_testing()
            .expect("commit rotated key lifecycle and finalized pulse atomically");
        state.push_block_hash_for_testing(committed_hash);
        let committed_snapshot_hashes = state
            .block_hashes
            .view()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        state
            .kura()
            .extend_hash_only_suffix_from_verified_snapshot(&committed_snapshot_hashes)
            .expect("persist the committed pulse-height hash-only snapshot suffix");

        let snapshot = norito::json::to_value(&state)
            .expect("serialize the committed key rotation and pulse history");
        let restored = crate::state::deserialize::KuraSeed {
            kura: state.kura_handle(),
            query_handle: LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json(snapshot)
        .expect("restart must restore the pulse-height key lifecycle");
        let world = restored.world.view();
        let persisted_a = world
            .global_beacon_key_sessions()
            .get(&pulse.session_id)
            .expect("predecessor key remains in public history");
        let persisted_b = world
            .global_beacon_key_sessions()
            .get(&fixture_b.session.record().session_id)
            .expect("successor key remains in public history");
        assert!(persisted_a.is_active_at(context.height));
        assert!(!persisted_a.is_active_at(next_height));
        assert!(!persisted_b.is_active_at(context.height));
        assert!(persisted_b.is_active_at(next_height));
        assert_eq!(
            world
                .global_beacon_active_session()
                .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY),
            Some(&fixture_b.session.record().session_id)
        );
        assert_eq!(
            verified_latest_global_threshold_beacon_pulse_v1(&world, &network_id, context.height,),
            Ok(pulse),
            "restored state must accept the persisted pulse under key A"
        );

        if !parliament_requested_slot {
            let mut block_hashes = (1_u8..=41)
                .map(|marker| {
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([marker; 32]))
                })
                .collect::<Vec<_>>();
            block_hashes[usize::try_from(expected_anchor.height - 1)
                .expect("small parent-anchor height")] = expected_anchor.block_hash;
            assert_eq!(
                finalized_global_beacon_npos_successor_seed_from_sources(
                    &world,
                    &block_hashes,
                    &network_id,
                    next_height,
                    context.epoch + 1,
                ),
                Ok(global_threshold_beacon_npos_successor_seed_v1(
                    &pulse,
                    next_height,
                    context.epoch + 1,
                )),
                "successor construction must use the pulse-height key, not the post-block pointer"
            );
        }
    }

    #[test]
    fn mandatory_parliament_pulse_persists_across_same_block_key_rotation() {
        assert_same_block_key_rotation_persists_requested_pulse(true);
    }

    #[test]
    fn mandatory_npos_pulse_persists_across_same_block_key_rotation() {
        assert_same_block_key_rotation_persists_requested_pulse(false);
    }

    #[cfg(feature = "test-network-parliament-signers")]
    #[test]
    fn invalid_test_outbound_is_not_locally_counted_and_is_rejected_on_ingress() {
        let keys = live_producer_keys();
        let network_id = beacon_fixture_network_id(0xA7);
        let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD7; 32]));
        let context = live_producer_context(&keys, network_id, parent_hash);
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut dkg_session = adaptive_dkg_session_fixture();
        dkg_session.network_id = network_id;
        dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture = adaptive_beacon_fixture_for_session(dkg_session);
        let cursor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x67; 32],
            seed: [0x68; 32],
            height: 0,
            round: 0,
        };
        let state = live_producer_state(&fixture, cursor, parent_hash);

        let invalid_signer: Arc<dyn GlobalThresholdBeaconPartialSignerV1> =
            Arc::new(InvalidOutboundTestBeaconSigner {
                inner: live_fixture_signer(&fixture, 1),
            });
        let mut invalid_producer =
            V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(invalid_signer))
                .expect("open deliberately invalid feature-only producer");
        invalid_producer
            .begin_round(0)
            .expect("an invalid test share is broadcast without local admission");
        let invalid = beacon_partial_payload(
            invalid_producer
                .take_outbound()
                .pop()
                .expect("invalid outbound share"),
        );

        let mut valid_producer = V2GlobalBeaconLifecycle::open(
            &context,
            &state,
            Some(1),
            Some(live_fixture_signer(&fixture, 2)),
        )
        .expect("open ordinary valid producer");
        valid_producer.begin_round(0).expect("sign valid share");
        let valid = beacon_partial_payload(
            valid_producer
                .take_outbound()
                .pop()
                .expect("valid outbound share"),
        );

        assert_eq!(
            invalid_producer
                .accept_partial(valid, &roster[1], 0)
                .expect("retain the sole proof-valid contribution"),
            V2GlobalBeaconIngressOutcome::Accepted,
            "one valid share must remain below the exact threshold of two",
        );
        assert!(invalid_producer.finalized_pulse(0).is_none());
        assert!(matches!(
            invalid_producer.accept_partial(invalid, &roster[0], 0),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::ThresholdBls(_)
            ))
        ));
        assert!(
            invalid_producer.finalized_pulse(0).is_none(),
            "the malformed outbound share must neither be pre-counted locally nor admitted on ingress",
        );
    }

    #[test]
    fn threshold_beacon_live_v2_producer_is_bound_restartable_and_persists_effect() {
        let keys = live_producer_keys();
        let network_id = beacon_fixture_network_id(0xA1);
        let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD1; 32]));
        let context = live_producer_context(&keys, network_id, parent_hash);
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut dkg_session = adaptive_dkg_session_fixture();
        dkg_session.network_id = network_id;
        dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
        let fixture = adaptive_beacon_fixture_for_session(dkg_session);
        let cursor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x61; 32],
            seed: [0x62; 32],
            height: 0,
            round: 0,
        };
        let state = live_producer_state(&fixture, cursor, parent_hash);
        let signers = (1_u16..=4)
            .map(|index| live_fixture_signer(&fixture, index))
            .collect::<Vec<_>>();

        let mut messages = Vec::new();
        for (index, signer) in signers.iter().enumerate() {
            let mut producer = V2GlobalBeaconLifecycle::open(
                &context,
                &state,
                Some(u32::try_from(index).expect("validator index")),
                Some(Arc::clone(signer)),
            )
            .expect("open validator producer");
            producer.begin_round(0).expect("sign exact round");
            let outbound = producer.take_outbound();
            assert_eq!(outbound.len(), 1);
            messages.push(beacon_partial_payload(
                outbound.into_iter().next().expect("local partial"),
            ));
        }

        let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
            .expect("open signerless validator reducer");
        reducer.begin_round(0).expect("open exact round");
        let mut absent_effects = NposConsensusEffects::default();
        assert!(matches!(
            reducer.attach_candidate_effects(0, &mut absent_effects),
            Err(V2GlobalBeaconError::State(_))
        ));

        let mut wrong_sender = messages[0].clone();
        assert_eq!(wrong_sender.partial.signer_index, 1);
        assert!(matches!(
            reducer.accept_partial(wrong_sender.clone(), &roster[1], 0),
            Err(V2GlobalBeaconError::SenderMismatch)
        ));
        wrong_sender.round.view = 1;
        assert!(matches!(
            reducer.accept_partial(wrong_sender, &roster[0], 0),
            Err(V2GlobalBeaconError::WrongView)
        ));
        let mut wrong_session = messages[0].clone();
        wrong_session.partial.session_id[0] ^= 1;
        assert!(matches!(
            reducer.accept_partial(wrong_session, &roster[0], 0),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::SessionMismatch
            ))
        ));

        assert_eq!(
            reducer
                .accept_partial(messages[0].clone(), &roster[0], 0)
                .expect("first verified share"),
            V2GlobalBeaconIngressOutcome::Accepted
        );
        let mut conflicting = messages[0].clone();
        conflicting.partial.signature_share[0] ^= 1;
        assert!(matches!(
            reducer.accept_partial(conflicting, &roster[0], 0),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::ThresholdBls(_)
            ))
        ));

        let mut restarted_signer =
            V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(Arc::clone(&signers[0])))
                .expect("restart signer");
        restarted_signer.begin_round(0).expect("retry exact share");
        let retry = beacon_partial_payload(
            restarted_signer
                .take_outbound()
                .pop()
                .expect("retried local partial"),
        );
        assert_eq!(
            retry.partial.signature_share,
            messages[0].partial.signature_share
        );
        assert_ne!(retry.partial.proof, messages[0].partial.proof);
        assert_eq!(
            reducer
                .accept_partial(retry, &roster[0], 0)
                .expect("fresh-proof retry"),
            V2GlobalBeaconIngressOutcome::Duplicate,
            "fresh proof randomness for one verified share must stay idempotent"
        );

        assert_eq!(
            reducer
                .accept_partial(messages[1].clone(), &roster[1], 0)
                .expect("threshold share"),
            V2GlobalBeaconIngressOutcome::Finalized
        );
        let pulse = reducer.finalized_pulse(0).expect("unique finalized pulse");
        for index in 2..4 {
            assert_eq!(
                reducer
                    .accept_partial(messages[index].clone(), &roster[index], 0)
                    .expect("additional four-validator-path share"),
                V2GlobalBeaconIngressOutcome::Duplicate
            );
            assert_eq!(reducer.finalized_pulse(0), Some(pulse));
        }

        let mut effects = NposConsensusEffects::default();
        reducer
            .attach_candidate_effects(0, &mut effects)
            .expect("attach exact finalized pulse to candidate effects");
        assert_eq!(effects.finalized_global_beacon_pulse, Some(pulse));

        let mut restarted = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
            .expect("restart signerless validator reducer");
        restarted.begin_round(0).expect("reopen exact round");
        assert_eq!(
            restarted
                .accept_partial(messages[0].clone(), &roster[0], 0)
                .expect("replayed first share after restart"),
            V2GlobalBeaconIngressOutcome::Accepted
        );
        assert_eq!(
            restarted
                .accept_partial(messages[1].clone(), &roster[1], 0)
                .expect("replayed threshold after restart"),
            V2GlobalBeaconIngressOutcome::Finalized
        );
        assert_eq!(restarted.finalized_pulse(0), Some(pulse));
        assert_eq!(pulse.round, GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1);
        restarted
            .begin_round(1)
            .expect("advance routing view without changing pulse payload");
        assert_eq!(
            restarted.finalized_pulse(1),
            Some(pulse),
            "a view change must retain the already reconstructed unique pulse"
        );

        let mut view_one_producer =
            V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(Arc::clone(&signers[0])))
                .expect("view-one producer");
        view_one_producer.begin_round(1).expect("sign view one");
        let view_one_partial = beacon_partial_payload(
            view_one_producer
                .take_outbound()
                .pop()
                .expect("view-one partial"),
        );
        assert_eq!(
            view_one_partial.partial.signature_share, messages[0].partial.signature_share,
            "consensus view must not alter the threshold-signed message"
        );
        assert_eq!(
            restarted
                .accept_partial(view_one_partial, &roster[0], 1)
                .expect("cross-view retry of the same height-bound share"),
            V2GlobalBeaconIngressOutcome::Duplicate
        );

        let wrong_payload = wire::GlobalBeaconPartialSignature {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 1,
            },
            partial: signers[0]
                .sign_partial(&fixture.session, b"wrong beacon payload")
                .expect("sign deliberately wrong payload"),
        };
        assert!(matches!(
            restarted.accept_partial(wrong_payload, &roster[0], 1),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::ThresholdBls(_)
            ))
        ));

        let other_parent = HashOf::from_untyped_unchecked(Hash::prehashed([0xD2; 32]));
        let other_context = live_producer_context(&keys, network_id, other_parent);
        let other_state = live_producer_state(&fixture, cursor, other_parent);
        let mut other_anchor_producer = V2GlobalBeaconLifecycle::open(
            &other_context,
            &other_state,
            Some(0),
            Some(Arc::clone(&signers[0])),
        )
        .expect("other-anchor producer");
        other_anchor_producer
            .begin_round(0)
            .expect("sign other anchor");
        let mut wrong_anchor = beacon_partial_payload(
            other_anchor_producer
                .take_outbound()
                .pop()
                .expect("other-anchor partial"),
        );
        wrong_anchor.round.context_id = context.id();
        wrong_anchor.round.view = 1;
        assert!(matches!(
            restarted.accept_partial(wrong_anchor, &roster[0], 1),
            Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::ThresholdBls(_)
            ))
        ));

        let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: parent_hash,
        };
        {
            let mut block = state.world.block();
            {
                let mut transaction =
                    block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
                transaction
                    .verify_and_advance_global_beacon_pulse(
                        &fixture.session,
                        pulse,
                        expected_anchor,
                    )
                    .expect("persist finalized pulse through authoritative World corridor");
                transaction.apply();
            }
            block.commit();
        }
        assert_eq!(
            verified_latest_global_threshold_beacon_pulse_v1(&state.world.view(), &network_id, 41,),
            Ok(pulse)
        );
    }

    #[test]
    fn threshold_beacon_session_validates_complete_canonical_transcript() {
        let (session, expected) = validated_threshold_session();
        assert_eq!(session.record().transcript_hash, expected.transcript_hash);
        assert_eq!(session.record().public_shares.len(), 4);
        assert_eq!(session.ensure_adaptive_protocol_ready(), Ok(()));
    }

    #[test]
    fn threshold_beacon_session_rejects_wrong_bindings_and_malformed_points() {
        let (session, expected) = validated_threshold_session();
        let record = session.record().clone();

        let mut wrong_network = record.clone();
        wrong_network.network_id = beacon_fixture_network_id(0x82);
        assert_eq!(
            validate_global_threshold_beacon_session_v1(wrong_network, &expected),
            Err(GlobalThresholdBeaconError::NetworkMismatch)
        );

        let mut wrong_roster = record.clone();
        wrong_roster.roster_hash[0] ^= 1;
        assert_eq!(
            validate_global_threshold_beacon_session_v1(wrong_roster, &expected),
            Err(GlobalThresholdBeaconError::RosterMismatch)
        );

        let mut zero_roster = record.clone();
        zero_roster.roster_hash = [0; 32];
        zero_roster.adaptive_dkg.session.roster_hash = [0; 32];
        let zero_roster_binding = GlobalThresholdBeaconSessionBindingV1 {
            roster_hash: [0; 32],
            ..expected
        };
        assert_eq!(
            validate_global_threshold_beacon_session_v1(zero_roster, &zero_roster_binding),
            Err(GlobalThresholdBeaconError::InvalidDkgSession)
        );

        let mut wrong_transcript = record.clone();
        wrong_transcript.transcript_hash[0] ^= 1;
        assert_eq!(
            validate_global_threshold_beacon_session_v1(wrong_transcript, &expected),
            Err(GlobalThresholdBeaconError::TranscriptMismatch)
        );

        let mut malformed_key = record;
        malformed_key.group_public_key = [0; 96];
        assert_eq!(
            validate_global_threshold_beacon_session_v1(malformed_key, &expected),
            Err(GlobalThresholdBeaconError::ThresholdBls(
                ThresholdBlsError::InvalidPublicKey
            ))
        );
    }

    #[test]
    fn threshold_beacon_and_tle_sessions_are_type_and_domain_separated() {
        let (session, _) = validated_threshold_session();
        let record = session.record();
        let beacon = ThresholdBlsSession::<BeaconPurpose>::new(
            *record.network_id.as_bytes(),
            record.session_id,
            record.roster_hash,
            record.committee_size,
            record.threshold,
        )
        .expect("beacon session");
        let tle = ThresholdBlsSession::<TleReleasePurpose>::new(
            *record.network_id.as_bytes(),
            record.session_id,
            record.roster_hash,
            record.committee_size,
            record.threshold,
        )
        .expect("TLE session");
        assert_ne!(
            beacon
                .signing_message(b"same payload")
                .expect("beacon message"),
            tle.signing_message(b"same payload").expect("TLE message")
        );
    }

    #[test]
    fn threshold_beacon_pulse_payload_binds_every_consensus_field() {
        let (session, _) = validated_threshold_session();
        let (pulse, _, _) = pulse_fixture(&session);
        let baseline = global_threshold_beacon_pulse_payload_v1(&pulse);

        let mut mutations = Vec::new();
        let mut changed = pulse.clone();
        changed.network_id = beacon_fixture_network_id(0x82);
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.session_id[0] ^= 1;
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.roster_hash[0] ^= 1;
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.transcript_hash[0] ^= 1;
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.height += 1;
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.round += 1;
        mutations.push(changed);
        let mut changed = pulse;
        changed.finalized_chain_anchor.block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x89; 32]));
        mutations.push(changed);

        for mutation in mutations {
            assert_ne!(
                global_threshold_beacon_pulse_payload_v1(&mutation),
                baseline
            );
        }
    }

    #[test]
    fn threshold_beacon_accepts_one_adaptive_final_signature_and_seed() {
        let (fixture, pulse, _cursor, anchor) = signed_pulse_fixture();

        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&fixture.session, &pulse, anchor,),
            Ok(GlobalThresholdBeaconPulseLinkV1 {
                pulse_id: pulse.pulse_id,
                seed: pulse.seed,
                height: pulse.height,
                round: pulse.round,
            })
        );
    }

    #[test]
    fn first_finalized_pulse_initializes_an_empty_ingestion_cursor() {
        let (fixture, pulse, _origin, anchor) = signed_pulse_fixture();
        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized beacon key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized beacon key");
        let world = World::new();
        {
            let mut block = world.block();
            {
                let mut transaction =
                    block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
                transaction
                    .global_beacon_key_sessions
                    .insert(pulse.session_id, key_record);
                transaction
                    .global_beacon_active_session
                    .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
                assert_eq!(
                    transaction.verify_and_advance_global_beacon_pulse(
                        &fixture.session,
                        pulse,
                        anchor,
                    ),
                    Ok(GlobalThresholdBeaconPulseLinkV1 {
                        pulse_id: pulse.pulse_id,
                        seed: pulse.seed,
                        height: pulse.height,
                        round: pulse.round,
                    })
                );
                transaction.apply();
            }
            block.commit();
        }
        assert_eq!(
            verified_latest_global_threshold_beacon_pulse_v1(
                &world.view(),
                &pulse.network_id,
                pulse.height,
            ),
            Ok(pulse),
            "the first certified pulse must create the authoritative cursor without a test-seeded origin"
        );
        let world_view = world.view();
        assert_eq!(
            world_view.global_beacon_pulse_at_slot(&pulse.network_id, pulse.height),
            Some(&pulse),
            "the authoritative insertion corridor must update the exact slot index atomically"
        );
        drop(world_view);
        assert_eq!(
            verified_global_threshold_beacon_pulse_at_or_before_v1(
                &world.view(),
                &pulse.network_id,
                pulse.height,
            ),
            Ok(pulse)
        );
        assert_eq!(
            verified_global_threshold_beacon_pulse_at_or_before_v1(
                &world.view(),
                &pulse.network_id,
                pulse.height - 1,
            ),
            Err(GlobalThresholdBeaconError::InvalidPulseHistory)
        );

        let mut conflicting_slot = pulse;
        conflicting_slot.pulse_id[0] ^= 1;
        let mut block = world.block();
        let mut transaction = block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
        assert_eq!(
            transaction.verify_and_advance_global_beacon_pulse(
                &fixture.session,
                conflicting_slot,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::ReusedPulse),
            "a distinct pulse id cannot claim an already indexed network-height slot"
        );
    }

    #[test]
    fn late_sortition_pulse_is_rejected_after_parliament_transcript_restart_roundtrip() {
        let (fixture, pulse, _origin, anchor) = signed_pulse_fixture();
        let roster = live_producer_keys()
            .into_iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let (governance_attempt_id, _, mut attempt) =
            pending_batched_sortition_attempt(&pulse.network_id, &roster, pulse.height);
        let failed_election_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::RulesCommittee,
            0,
        );
        attempt
            .fail_body_election_no_roster(
                governance_attempt_id,
                failed_election_id,
                false,
                pulse.height + 1,
            )
            .expect("terminally classify the missing initial sortition slot");
        let logical_session = BeaconSessionId::for_network_v1(&pulse.network_id);
        assert!(attempt.classifies_beacon_pulse_unavailable_at(logical_session, pulse.height));

        let encoded = norito::json::to_json(&attempt)
            .expect("serialize the terminal Parliament transcript for restart");
        let restored_attempt: ParliamentAttemptStateV1 =
            norito::json::from_str(&encoded).expect("restore the terminal Parliament transcript");
        restored_attempt
            .validate()
            .expect("restored missing-pulse transcript remains canonical");
        assert!(
            restored_attempt.classifies_beacon_pulse_unavailable_at(logical_session, pulse.height)
        );

        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized beacon key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized beacon key");
        let world = World::new();
        let mut block = world.block();
        let mut transaction = block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
        transaction
            .global_beacon_key_sessions
            .insert(pulse.session_id, key_record);
        transaction
            .parliament_attempts
            .insert(governance_attempt_id, restored_attempt);
        assert_eq!(
            transaction.verify_and_advance_global_beacon_pulse(&fixture.session, pulse, anchor,),
            Err(GlobalThresholdBeaconError::PersistenceConflict),
            "restart must not reopen a sortition slot already closed as unavailable"
        );
    }

    #[test]
    fn threshold_beacon_slot_is_identical_when_prior_unrelated_height_is_persisted_or_omitted() {
        let fixture = adaptive_beacon_fixture();
        let (template, origin, target_anchor) = pulse_fixture(&fixture.session);
        let finalize_slot =
            |height: u64, anchor: GlobalThresholdBeaconChainAnchorV1, proof_seed: [u8; 32]| {
                let mut unsigned = template;
                unsigned.height = height;
                unsigned.finalized_chain_anchor = anchor;
                let partials = pulse_partial_signatures(&fixture, &unsigned, proof_seed);
                let mut aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
                    fixture.session.clone(),
                    height,
                    anchor,
                )
                .expect("open unchained slot aggregator");
                for partial in partials.into_iter().take(usize::from(
                    fixture.session.transcript.session().threshold(),
                )) {
                    aggregator
                        .accept_partial(partial)
                        .expect("accept exact slot partial");
                }
                aggregator.finalize().expect("finalize exact slot")
            };

        let target_without_prior = finalize_slot(41, target_anchor, [0x31; 32]);
        let prior_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 39,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x87; 32])),
        };
        let prior = finalize_slot(40, prior_anchor, [0x32; 32]);
        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized key");
        let world = World::new();
        {
            let mut block = world.block();
            {
                let mut transaction =
                    block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
                transaction
                    .global_beacon_key_sessions
                    .insert(prior.session_id, key_record);
                transaction
                    .global_beacon_active_session
                    .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, prior.session_id);
                transaction
                    .global_beacon_latest_pulse
                    .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, origin);
                transaction
                    .verify_and_advance_global_beacon_pulse(&fixture.session, prior, prior_anchor)
                    .expect("persist prior unrelated pulse");
                transaction.apply();
            }
            block.commit();
        }
        assert_eq!(
            verified_latest_global_threshold_beacon_pulse_v1(
                &world.view(),
                &prior.network_id,
                prior.height,
            ),
            Ok(prior)
        );

        let target_after_persisted_prior = finalize_slot(41, target_anchor, [0x33; 32]);
        assert_eq!(target_after_persisted_prior, target_without_prior);
        assert_eq!(
            target_after_persisted_prior.seed, target_without_prior.seed,
            "an unrelated earlier pulse must not influence the later slot seed"
        );
    }

    #[test]
    fn parliament_seed_reverifies_valid_and_rejects_tampered_persisted_pulse() {
        let (fixture, pulse, _origin, _anchor) = signed_pulse_fixture();
        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized key");
        let link = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: pulse.pulse_id,
            seed: pulse.seed,
            height: pulse.height,
            round: pulse.round,
        };
        let persisted_world = |stored_pulse: FinalizedGlobalThresholdBeaconPulseV1| {
            let world = World::new();
            {
                let mut block = world.block();
                block
                    .global_beacon_key_sessions
                    .insert(pulse.session_id, key_record.clone());
                block
                    .global_beacon_active_session
                    .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
                block
                    .global_beacon_pulses
                    .insert(stored_pulse.pulse_id, stored_pulse);
                block.global_beacon_pulse_slots.insert(
                    (stored_pulse.network_id, stored_pulse.height),
                    stored_pulse.pulse_id,
                );
                block
                    .global_beacon_latest_pulse
                    .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
                block.commit();
            }
            world
        };

        let valid_world = persisted_world(pulse);
        assert_eq!(
            verified_persisted_global_threshold_beacon_governance_seed_v1(
                &valid_world.view(),
                &pulse.network_id,
                pulse,
                pulse.height,
            ),
            Ok(global_threshold_beacon_governance_seed_v1(
                &pulse,
                pulse.height,
            ))
        );

        let mut tampered = pulse;
        tampered.signature[0] ^= 1;
        let tampered_world = persisted_world(tampered);
        assert!(
            verified_persisted_global_threshold_beacon_governance_seed_v1(
                &tampered_world.view(),
                &pulse.network_id,
                tampered,
                pulse.height,
            )
            .is_err(),
            "Parliament must not derive entropy from an invalid restored pulse"
        );
    }

    #[test]
    fn threshold_beacon_partial_reducer_is_bound_fail_closed_and_subset_invariant() {
        let fixture = adaptive_beacon_fixture();
        let (pulse, _cursor, anchor) = pulse_fixture(&fixture.session);
        let partials = pulse_partial_signatures(&fixture, &pulse, [0xA7; 32]);
        let threshold = usize::from(fixture.session.transcript.session().threshold());

        let open_reducer = || {
            GlobalThresholdBeaconPulseAggregatorV1::new(
                fixture.session.clone(),
                pulse.height,
                anchor,
            )
            .expect("open exact pulse reducer")
        };
        let mut insufficient = open_reducer();
        for partial in partials.iter().take(threshold - 1).cloned() {
            assert_eq!(insufficient.accept_partial(partial), Ok(true));
        }
        assert_eq!(
            insufficient.finalize(),
            Err(GlobalThresholdBeaconError::InsufficientPartialSignatures)
        );

        let mut low_indices = open_reducer();
        for partial in partials.iter().take(threshold).cloned() {
            assert_eq!(low_indices.accept_partial(partial), Ok(true));
        }
        assert_eq!(
            low_indices.accept_partial(partials[0]),
            Ok(false),
            "exact retransmissions must be idempotent"
        );
        let low_pulse = low_indices.finalize().expect("low-index threshold");

        let mut high_indices = open_reducer();
        for partial in partials.iter().rev().take(threshold).cloned() {
            assert_eq!(high_indices.accept_partial(partial), Ok(true));
        }
        assert_eq!(
            high_indices.finalize().expect("high-index threshold"),
            low_pulse,
            "the public pulse must not expose or depend on the reconstruction subset"
        );

        let mut wrong_session = partials[0];
        wrong_session.session_id[0] ^= 1;
        assert_eq!(
            open_reducer().accept_partial(wrong_session),
            Err(GlobalThresholdBeaconError::SessionMismatch)
        );

        let distinct_valid_proof = pulse_partial_signatures(&fixture, &pulse, [0xB8; 32])[0];
        assert_ne!(partials[0], distinct_valid_proof);
        let mut equivocating = open_reducer();
        assert_eq!(equivocating.accept_partial(partials[0]), Ok(true));
        assert_eq!(
            equivocating.accept_partial(distinct_valid_proof),
            Ok(false),
            "fresh proof randomness for the same verified share is an idempotent retry"
        );

        let mismatched_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: pulse.height,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x89; 32])),
        };
        let mut other_height = GlobalThresholdBeaconPulseAggregatorV1::new(
            fixture.session.clone(),
            pulse.height + 1,
            mismatched_anchor,
        )
        .expect("open another exact pulse round");
        assert!(matches!(
            other_height.accept_partial(partials[0]),
            Err(GlobalThresholdBeaconError::ThresholdBls(_))
        ));
    }

    #[test]
    fn npos_successor_seed_requires_one_exact_finalized_pulse_and_chain_anchor() {
        const BOUNDARY_HEIGHT: u64 = 42;
        const SUCCESSOR_EPOCH: u64 = 9;
        let (fixture, pulse, _cursor, anchor) = signed_pulse_fixture();
        let mut key_record =
            FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
                .expect("valid finalized beacon key");
        key_record
            .activate(key_record.session.adaptive_dkg.finalized_at_height)
            .expect("activate finalized beacon key");
        let link = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: pulse.pulse_id,
            seed: pulse.seed,
            height: pulse.height,
            round: pulse.round,
        };
        let world = World::new();
        {
            let mut block = world.block();
            block
                .global_beacon_key_sessions
                .insert(pulse.session_id, key_record);
            block
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
            block.global_beacon_pulses.insert(pulse.pulse_id, pulse);
            block
                .global_beacon_pulse_slots
                .insert((pulse.network_id, pulse.height), pulse.pulse_id);
            block
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
            block.commit();
        }
        let mut block_hashes = (1_u8..=41)
            .map(|marker| {
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([marker; 32]))
            })
            .collect::<Vec<_>>();
        block_hashes[usize::try_from(anchor.height - 1).expect("small anchor height")] =
            anchor.block_hash;
        let world_view = world.view();
        let expected_seed = global_threshold_beacon_npos_successor_seed_v1(
            &pulse,
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH,
        );
        assert_ne!(
            expected_seed, pulse.seed,
            "NPoS must not reuse the raw pulse seed"
        );
        assert_ne!(
            expected_seed,
            global_threshold_beacon_npos_successor_seed_v1(
                &pulse,
                BOUNDARY_HEIGHT,
                SUCCESSOR_EPOCH + 1,
            ),
            "the target epoch is part of the NPoS seed domain"
        );
        assert_eq!(
            finalized_global_beacon_npos_successor_seed_from_sources(
                &world_view,
                &block_hashes,
                &pulse.network_id,
                BOUNDARY_HEIGHT,
                SUCCESSOR_EPOCH,
            ),
            Ok(expected_seed)
        );

        let empty_world = World::new();
        assert_eq!(
            finalized_global_beacon_npos_successor_seed_from_sources(
                &empty_world.view(),
                &block_hashes,
                &pulse.network_id,
                BOUNDARY_HEIGHT,
                SUCCESSOR_EPOCH,
            ),
            Err(V2ContextBuildError::MissingPreBoundaryBeaconPulse)
        );
        assert_eq!(
            finalized_global_beacon_npos_successor_seed_from_sources(
                &world_view,
                &block_hashes,
                &beacon_fixture_network_id(0x82),
                BOUNDARY_HEIGHT,
                SUCCESSOR_EPOCH,
            ),
            Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)
        );
        block_hashes[usize::try_from(anchor.height - 1).expect("small anchor height")] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xFE; 32]));
        assert_eq!(
            finalized_global_beacon_npos_successor_seed_from_sources(
                &world_view,
                &block_hashes,
                &pulse.network_id,
                BOUNDARY_HEIGHT,
                SUCCESSOR_EPOCH,
            ),
            Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)
        );
    }

    #[test]
    fn threshold_beacon_pulse_rejects_zero_noncanonical_round_and_wrong_anchor() {
        let (session, _) = validated_threshold_session();
        let (pulse, _cursor, anchor) = pulse_fixture(&session);

        let mut zero = pulse.clone();
        zero.seed = [0; 32];
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&session, &zero, anchor),
            Err(GlobalThresholdBeaconError::ZeroPulse)
        );

        let mut noncanonical_round = pulse;
        noncanonical_round.round = 1;
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &noncanonical_round,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::NonCanonicalRound)
        );

        let wrong_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: anchor.height + 1,
            ..anchor
        };
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&session, &pulse, wrong_anchor),
            Err(GlobalThresholdBeaconError::FinalizedAnchorMismatch)
        );
    }

    #[test]
    fn threshold_beacon_pulse_rejects_malformed_and_wrong_final_signatures() {
        let (session, _) = validated_threshold_session();
        let (pulse, _cursor, anchor) = pulse_fixture(&session);

        let mut malformed = pulse.clone();
        malformed.signature = [0; 48];
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&session, &malformed, anchor),
            Err(GlobalThresholdBeaconError::ThresholdBls(
                ThresholdBlsError::InvalidSignature
            ))
        );
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&session, &pulse, anchor),
            Err(GlobalThresholdBeaconError::ThresholdBls(
                ThresholdBlsError::SignatureMismatch
            ))
        );
    }

    #[test]
    fn threshold_beacon_canonical_decoders_reject_trailing_wire_data() {
        let (session, expected) = validated_threshold_session();
        let mut encoded_session = norito::to_bytes(session.record()).expect("encode session");
        encoded_session.push(0);
        assert!(matches!(
            decode_global_threshold_beacon_session_v1(&encoded_session, &expected),
            Err(GlobalThresholdBeaconError::InvalidEncoding)
                | Err(GlobalThresholdBeaconError::NonCanonicalEncoding)
        ));

        let (pulse, _cursor, anchor) = pulse_fixture(&session);
        let mut encoded_pulse = norito::to_bytes(&pulse).expect("encode pulse");
        encoded_pulse.push(0);
        assert!(matches!(
            decode_finalized_global_threshold_beacon_pulse_v1(&encoded_pulse, &session, anchor,),
            Err(GlobalThresholdBeaconError::InvalidEncoding)
                | Err(GlobalThresholdBeaconError::NonCanonicalEncoding)
        ));
    }
}
