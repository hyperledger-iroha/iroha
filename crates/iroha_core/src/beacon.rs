//! Canonical global threshold-beacon verification.
//!
//! A first-release pulse is one unique threshold-BLS group signature over an
//! exact network, key session, predecessor, position, transcript, and finalized
//! chain anchor. Its public seed is derived only after final-signature
//! verification. Reconstruction shares and signer subsets are deliberately not
//! part of the pulse DTO, so different qualifying subsets cannot create
//! different public representations of the same pulse.
//!
//! The older per-validator VRF input and aggregation helpers remain below in an
//! explicitly legacy section while their existing call sites are removed. They
//! are not consulted by the threshold-beacon validator.

use iroha_crypto::{
    Hash,
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript, BeaconPurpose,
        DasRenDealerCommitment, DasRenRevealedShare, ThresholdBlsError, ThresholdBlsPublicKey,
        ThresholdBlsSession, ThresholdBlsSignature, ValidatedDealerCommitment,
    },
};
use iroha_data_model::{
    NetworkId,
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconDkgComplaintResponseV1,
        GlobalThresholdBeaconDkgComplaintV1, GlobalThresholdBeaconDkgDealerCommitmentV1,
        GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconDkgTranscriptV1,
        GlobalThresholdBeaconKeySessionV1, GlobalThresholdBeaconPublicShareV1,
    },
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::Encode as _,
    derive::{JsonDeserialize, JsonSerialize},
};
use std::collections::BTreeMap;
use thiserror::Error;

const GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1: &[u8] =
    b"iroha.global-threshold-beacon.pulse-payload.v1\0";
const GLOBAL_BEACON_PULSE_ID_DOMAIN_V1: &[u8] = b"iroha.global-threshold-beacon.pulse-id.v1\0";

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

/// Authoritative predecessor link used to admit the next pulse.
///
/// For the first pulse, consensus supplies a non-zero origin ID and seed bound
/// by genesis configuration, with position `(0, 0)`.
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
    /// Identifier of the preceding pulse or genesis origin.
    pub pulse_id: [u8; 32],
    /// Seed of the preceding pulse or genesis origin.
    pub seed: [u8; 32],
    /// Preceding consensus height.
    pub height: u64,
    /// Preceding consensus round.
    pub round: u64,
}

impl GlobalThresholdBeaconPulseLinkV1 {
    /// Validate a non-zero persisted predecessor link.
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
    /// The pulse is not linked to the exact authoritative predecessor.
    #[error("global threshold-beacon predecessor mismatch")]
    PredecessorMismatch,
    /// The pulse position does not strictly follow its authoritative predecessor.
    #[error("global threshold-beacon height/round is not strictly monotonic")]
    NonMonotonicPosition,
    /// The pulse does not authenticate the expected finalized-chain point.
    #[error("global threshold-beacon finalized-chain anchor mismatch")]
    FinalizedAnchorMismatch,
    /// The supplied seed is not the unique seed derived from the final signature.
    #[error("global threshold-beacon derived seed mismatch")]
    SeedMismatch,
    /// A pulse reused its predecessor's ID or seed.
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
            parameters,
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
        parameters,
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
    pub const fn ensure_adaptive_protocol_ready(&self) -> Result<(), ThresholdBlsError> {
        self.transcript.ensure_adaptive_protocol_ready()
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
/// remaining field has a fixed width.
#[must_use]
pub fn global_threshold_beacon_pulse_payload_v1(
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
) -> Vec<u8> {
    let mut payload =
        Vec::with_capacity(GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 8 + 8 * 3);
    payload.extend_from_slice(GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1);
    payload.extend_from_slice(&pulse.version.to_be_bytes());
    payload.extend_from_slice(pulse.network_id.as_bytes());
    payload.extend_from_slice(&pulse.session_id);
    payload.extend_from_slice(&pulse.roster_hash);
    payload.extend_from_slice(&pulse.transcript_hash);
    payload.extend_from_slice(&pulse.height.to_be_bytes());
    payload.extend_from_slice(&pulse.round.to_be_bytes());
    payload.extend_from_slice(&pulse.previous_pulse_id);
    payload.extend_from_slice(&pulse.previous_seed);
    payload.extend_from_slice(&pulse.finalized_chain_anchor.height.to_be_bytes());
    payload.extend_from_slice(pulse.finalized_chain_anchor.block_hash.as_ref());
    payload
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
    if pulse.height == 0
        || is_zero(&pulse.pulse_id)
        || is_zero(&pulse.seed)
        || is_zero(&pulse.previous_pulse_id)
        || is_zero(&pulse.previous_seed)
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
    if pulse.pulse_id == pulse.previous_pulse_id || pulse.seed == pulse.previous_seed {
        return Err(GlobalThresholdBeaconError::ReusedPulse);
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
    predecessor: GlobalThresholdBeaconPulseLinkV1,
    expected_anchor: GlobalThresholdBeaconChainAnchorV1,
) -> Result<GlobalThresholdBeaconPulseLinkV1, GlobalThresholdBeaconError> {
    if pulse.version != GLOBAL_THRESHOLD_BEACON_VERSION_V1 {
        return Err(GlobalThresholdBeaconError::UnsupportedVersion {
            actual: pulse.version,
        });
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
        || is_zero(&pulse.previous_pulse_id)
        || is_zero(&pulse.previous_seed)
        || is_zero(predecessor.pulse_id.as_slice())
        || is_zero(predecessor.seed.as_slice())
        || is_zero(pulse.finalized_chain_anchor.block_hash.as_ref())
    {
        return Err(GlobalThresholdBeaconError::ZeroPulse);
    }
    if pulse.previous_pulse_id != predecessor.pulse_id || pulse.previous_seed != predecessor.seed {
        return Err(GlobalThresholdBeaconError::PredecessorMismatch);
    }
    if pulse.height < predecessor.height
        || (pulse.height == predecessor.height && pulse.round <= predecessor.round)
    {
        return Err(GlobalThresholdBeaconError::NonMonotonicPosition);
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
    if pulse_id == predecessor.pulse_id || seed == predecessor.seed {
        return Err(GlobalThresholdBeaconError::ReusedPulse);
    }

    Ok(GlobalThresholdBeaconPulseLinkV1 {
        pulse_id,
        seed,
        height: pulse.height,
        round: pulse.round,
    })
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
    predecessor: GlobalThresholdBeaconPulseLinkV1,
    expected_anchor: GlobalThresholdBeaconChainAnchorV1,
) -> Result<GlobalThresholdBeaconPulseLinkV1, GlobalThresholdBeaconError> {
    let pulse: FinalizedGlobalThresholdBeaconPulseV1 = norito::decode_from_bytes(encoded)
        .map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    let canonical =
        norito::to_bytes(&pulse).map_err(|_| GlobalThresholdBeaconError::InvalidEncoding)?;
    if canonical != encoded {
        return Err(GlobalThresholdBeaconError::NonCanonicalEncoding);
    }
    verify_finalized_global_threshold_beacon_pulse_v1(session, &pulse, predecessor, expected_anchor)
}

fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

// Legacy VRF compatibility helpers. New global beacon pulses MUST NOT call
// these functions; they remain only until their existing consensus call sites
// are removed in the integrated cutover.

/// Build the canonical epoch VRF input bytes.
///
/// Layout: `b"iroha:beacon:v1" || network_id[32] || epoch_be || prev_finalized_hash` where
/// - `network_id` is the exact genesis-derived deployment identity;
/// - `epoch_be` is the big‑endian encoding of the epoch number;
/// - `prev_finalized_hash` is the 32‑byte block hash anchoring this epoch.
pub fn epoch_input(network_id: &NetworkId, epoch: u64, prev_finalized_hash: [u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + 32 + 8 + 32);
    v.extend_from_slice(b"iroha:beacon:v1");
    v.extend_from_slice(network_id.as_bytes());
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v
}
/// Build the canonical leader‑election VRF input (slot‑bound, pk‑bound).
///
/// Layout: `b"iroha:vrf:v1:input|leader|" || network_id[32] || epoch_be || slot_be || prev_finalized_hash || pk_bytes`
pub fn leader_input(
    network_id: &NetworkId,
    epoch: u64,
    slot: u64,
    prev_finalized_hash: [u8; 32],
    pk_bytes: &[u8],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(24 + 32 + 8 + 8 + 32 + pk_bytes.len());
    v.extend_from_slice(b"iroha:vrf:v1:input|leader|");
    v.extend_from_slice(network_id.as_bytes());
    v.extend_from_slice(&epoch.to_be_bytes());
    v.extend_from_slice(&slot.to_be_bytes());
    v.extend_from_slice(&prev_finalized_hash);
    v.extend_from_slice(pk_bytes);
    v
}
/// Aggregate a set of per‑validator VRF outputs deterministically.
///
/// Construction: `Hash(b"iroha:beacon:v1:agg" || network_id[32] || sort(outputs))` where sorting
/// is lexicographic on the raw 32‑byte outputs. This prevents order‑based
/// malleability and yields identical results across peers.
pub fn aggregate_outputs(network_id: &NetworkId, mut outputs: Vec<[u8; 32]>) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf = Vec::with_capacity(16 + 32 + outputs.len() * 32);
    buf.extend_from_slice(b"iroha:beacon:v1:agg");
    buf.extend_from_slice(network_id.as_bytes());
    for y in outputs {
        buf.extend_from_slice(&y);
    }
    *Hash::new(&buf).as_ref()
}
/// Aggregate outputs with metadata binding: committee root and a reveal bitmap.
///
/// Layout: `b"iroha:beacon:v1:agg|" || network_id[32] || epoch_be || committee_root || bitmap_len_be || bitmap_bytes || concat(sort_lex(y_i))`
pub fn aggregate_outputs_with_meta(
    network_id: &NetworkId,
    epoch: u64,
    committee_root: [u8; 32],
    reveal_bitmap: &[u8],
    mut outputs: Vec<[u8; 32]>,
) -> [u8; 32] {
    outputs.sort_unstable();
    outputs.dedup();
    let mut buf =
        Vec::with_capacity(24 + 32 + 8 + 32 + 8 + reveal_bitmap.len() + outputs.len() * 32);
    buf.extend_from_slice(b"iroha:beacon:v1:agg|");
    buf.extend_from_slice(network_id.as_bytes());
    buf.extend_from_slice(&epoch.to_be_bytes());
    buf.extend_from_slice(&committee_root);
    buf.extend_from_slice(&(reveal_bitmap.len() as u64).to_be_bytes());
    buf.extend_from_slice(reveal_bitmap);
    for y in outputs {
        buf.extend_from_slice(&y);
    }
    *Hash::new(&buf).as_ref()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{
        HashOf,
        threshold_bls::{AdaptiveThresholdBlsSecretShare, DasRenDealerSecret, TleReleasePurpose},
    };
    use iroha_data_model::{
        block::BlockHeader,
        consensus::{
            GlobalThresholdBeaconDkgComplaintReasonV1, GlobalThresholdBeaconDkgConstantProofV1,
            GlobalThresholdBeaconPublicShareV1,
        },
    };
    use rand::{SeedableRng as _, rngs::StdRng};

    struct AcceptingAdaptiveDkgCrypto;

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

    fn network_id(marker: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([marker; Hash::LENGTH]),
        ))
    }

    fn adaptive_dkg_session_fixture() -> GlobalThresholdBeaconDkgSessionV1 {
        GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: network_id(0x81),
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

    struct AdaptiveBeaconFixture {
        session: ValidatedGlobalThresholdBeaconSessionV1,
        binding: GlobalThresholdBeaconSessionBindingV1,
        parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
        dealer_secrets: Vec<DasRenDealerSecret<BeaconPurpose>>,
        dealer_commitments: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
    }

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

    fn adaptive_beacon_fixture() -> AdaptiveBeaconFixture {
        let dkg_session = adaptive_dkg_session_fixture();
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
            binding,
            parameters,
            dealer_secrets,
            dealer_commitments,
        }
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
        let predecessor = GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: [0x66; 32],
            seed: [0x77; 32],
            height: 40,
            round: 8,
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
                previous_pulse_id: predecessor.pulse_id,
                previous_seed: predecessor.seed,
                finalized_chain_anchor: anchor,
                signature: wrong_g1_signature(),
                seed: [0x99; 32],
                pulse_id: [0xAA; 32],
            },
            predecessor,
            anchor,
        )
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
        wrong_network.network_id = network_id(0x82);
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
        changed.network_id = network_id(0x82);
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
        let mut changed = pulse.clone();
        changed.previous_pulse_id[0] ^= 1;
        mutations.push(changed);
        let mut changed = pulse.clone();
        changed.previous_seed[0] ^= 1;
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
        let fixture = adaptive_beacon_fixture();
        let (mut pulse, predecessor, anchor) = pulse_fixture(&fixture.session);
        let payload = global_threshold_beacon_pulse_payload_v1(&pulse);
        let mut proof_rng = StdRng::from_seed([0xA7; 32]);
        let mut partials = Vec::new();

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
            partials.push(
                signing_share
                    .sign_payload_with_rng(&fixture.session.transcript, &payload, &mut proof_rng)
                    .expect("adaptive signature share"),
            );
        }

        let signature = fixture
            .session
            .transcript
            .combine_partial_signatures(&payload, &partials)
            .expect("unique adaptive final signature");
        pulse.signature = *signature.as_bytes();
        pulse.seed = fixture
            .session
            .transcript
            .finalized_seed(&payload, &signature)
            .expect("derive verified beacon seed");
        pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);

        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &fixture.session,
                &pulse,
                predecessor,
                anchor,
            ),
            Ok(GlobalThresholdBeaconPulseLinkV1 {
                pulse_id: pulse.pulse_id,
                seed: pulse.seed,
                height: pulse.height,
                round: pulse.round,
            })
        );
    }

    #[test]
    fn threshold_beacon_pulse_rejects_zero_replay_nonmonotonic_and_wrong_anchor() {
        let (session, _) = validated_threshold_session();
        let (pulse, predecessor, anchor) = pulse_fixture(&session);

        let mut zero = pulse.clone();
        zero.seed = [0; 32];
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(&session, &zero, predecessor, anchor),
            Err(GlobalThresholdBeaconError::ZeroPulse)
        );

        let mut wrong_predecessor = pulse.clone();
        wrong_predecessor.previous_seed[0] ^= 1;
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &wrong_predecessor,
                predecessor,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::PredecessorMismatch)
        );

        let mut nonmonotonic = pulse.clone();
        nonmonotonic.height = predecessor.height;
        nonmonotonic.round = predecessor.round;
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &nonmonotonic,
                predecessor,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::NonMonotonicPosition)
        );

        let wrong_anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: anchor.height + 1,
            ..anchor
        };
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &pulse,
                predecessor,
                wrong_anchor,
            ),
            Err(GlobalThresholdBeaconError::FinalizedAnchorMismatch)
        );
    }

    #[test]
    fn threshold_beacon_pulse_rejects_malformed_and_wrong_final_signatures() {
        let (session, _) = validated_threshold_session();
        let (pulse, predecessor, anchor) = pulse_fixture(&session);

        let mut malformed = pulse.clone();
        malformed.signature = [0; 48];
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &malformed,
                predecessor,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::ThresholdBls(
                ThresholdBlsError::InvalidSignature
            ))
        );
        assert_eq!(
            verify_finalized_global_threshold_beacon_pulse_v1(
                &session,
                &pulse,
                predecessor,
                anchor,
            ),
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

        let (pulse, predecessor, anchor) = pulse_fixture(&session);
        let mut encoded_pulse = norito::to_bytes(&pulse).expect("encode pulse");
        encoded_pulse.push(0);
        assert!(matches!(
            decode_finalized_global_threshold_beacon_pulse_v1(
                &encoded_pulse,
                &session,
                predecessor,
                anchor,
            ),
            Err(GlobalThresholdBeaconError::InvalidEncoding)
                | Err(GlobalThresholdBeaconError::NonCanonicalEncoding)
        ));
    }
    #[test]
    fn epoch_input_has_domain_and_sizes() {
        let prev = [7u8; 32];
        let network_id = network_id(0x81);
        let x = epoch_input(&network_id, 42, prev);
        assert!(x.starts_with(b"iroha:beacon:v1"));
        assert_eq!(x.len(), b"iroha:beacon:v1".len() + 32 + 8 + 32);
    }
    #[test]
    fn aggregate_is_order_independent() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let c = [3u8; 32];
        let network_id = network_id(0x81);
        let r1 = aggregate_outputs(&network_id, vec![a, b, c]);
        let r2 = aggregate_outputs(&network_id, vec![c, a, b]);
        assert_eq!(r1, r2);
    }
    #[test]
    fn aggregate_deduplicates_outputs() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let network_id = network_id(0x81);
        let r1 = aggregate_outputs(&network_id, vec![a, b]);
        let r2 = aggregate_outputs(&network_id, vec![a, b, a, b]);
        assert_eq!(r1, r2, "duplicate VRF outputs must not skew the beacon");
    }
    #[test]
    fn leader_input_binds_pk_and_slot() {
        let network_id = network_id(0x81);
        let prev = [7u8; 32];
        let pk = vec![5u8; 48];
        let x = leader_input(&network_id, 42, 9, prev, &pk);
        assert!(x.starts_with(b"iroha:vrf:v1:input|leader|"));
        assert_eq!(
            x.len(),
            b"iroha:vrf:v1:input|leader|".len() + 32 + 8 + 8 + 32 + pk.len()
        );
    }
    #[test]
    fn aggregate_with_meta_changes_with_bitmap() {
        let network_id = network_id(0x81);
        let out = [[1u8; 32], [2u8; 32]].to_vec();
        let r1 = aggregate_outputs_with_meta(&network_id, 1, [9u8; 32], &[0b11], out.clone());
        let r2 = aggregate_outputs_with_meta(&network_id, 1, [9u8; 32], &[0b01], out);
        assert_ne!(r1, r2);
    }
    #[test]
    fn aggregate_with_meta_deduplicates_outputs() {
        let network_id = network_id(0x81);
        let base = aggregate_outputs_with_meta(&network_id, 7, [9u8; 32], &[0b11], vec![[1u8; 32]]);
        let duped = aggregate_outputs_with_meta(
            &network_id,
            7,
            [9u8; 32],
            &[0b11],
            vec![[1u8; 32], [1u8; 32]],
        );
        assert_eq!(base, duped);
    }
    #[test]
    fn every_beacon_domain_rejects_same_label_different_genesis_by_construction() {
        let first = network_id(0x81);
        let second = network_id(0x82);
        let prev = [7_u8; 32];
        let output = vec![[1_u8; 32]];
        assert_ne!(
            epoch_input(&first, 42, prev),
            epoch_input(&second, 42, prev)
        );
        assert_ne!(
            leader_input(&first, 42, 9, prev, &[5_u8; 48]),
            leader_input(&second, 42, 9, prev, &[5_u8; 48])
        );
        assert_ne!(
            aggregate_outputs(&first, output.clone()),
            aggregate_outputs(&second, output)
        );
    }
}
