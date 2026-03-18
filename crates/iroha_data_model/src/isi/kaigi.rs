use iroha_crypto::Hash;

use super::*;
use crate::kaigi::{
    KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiRelayHealthStatus,
    KaigiRelayManifest, KaigiRelayRegistration, NewKaigi,
};

isi! {
    /// Create a new Kaigi session anchored to a domain.
    pub struct CreateKaigi {
        /// Template describing the call to create.
        pub call: NewKaigi,
    }
}

isi! {
    /// Add a participant to an active Kaigi.
    pub struct JoinKaigi {
        /// Identifier of the call to join.
        pub call_id: KaigiId,
        /// Account joining the call.
        pub participant: AccountId,
    /// Commitment describing the participant (privacy mode only).
    pub commitment: Option<KaigiParticipantCommitment>,
    /// Nullifier preventing duplicate joins (privacy mode only).
    pub nullifier: Option<KaigiParticipantNullifier>,
    /// Merkle root the participant used when generating their proof (privacy mode only).
    pub roster_root: Option<Hash>,
    /// Proof bytes attesting ownership of the commitment (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Remove a participant from an active Kaigi.
    pub struct LeaveKaigi {
        /// Identifier of the call to leave.
        pub call_id: KaigiId,
        /// Account leaving the call.
        pub participant: AccountId,
    /// Commitment describing the participant (privacy mode only).
    pub commitment: Option<KaigiParticipantCommitment>,
    /// Nullifier preventing duplicate leaves (privacy mode only).
    pub nullifier: Option<KaigiParticipantNullifier>,
    /// Merkle root the participant used when generating their proof (privacy mode only).
    pub roster_root: Option<Hash>,
    /// Proof bytes attesting ownership of the commitment (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Conclude an active Kaigi.
    pub struct EndKaigi {
        /// Identifier of the call to end.
        pub call_id: KaigiId,
        /// Optional timestamp in milliseconds when the call ended.
        pub ended_at_ms: Option<u64>,
    }
}

isi! {
    /// Record usage metrics for a Kaigi segment.
    pub struct RecordKaigiUsage {
    /// Identifier of the call to update.
    pub call_id: KaigiId,
    /// Duration in milliseconds for this usage segment.
    pub duration_ms: u64,
    /// Gas billed for this segment (as computed off-ledger).
    pub billed_gas: u64,
    /// Commitment to the usage tuple (privacy mode only).
    pub usage_commitment: Option<Hash>,
    /// Optional proof tying the commitment to encrypted logs (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
}
}

isi! {
    /// Update the relay manifest advertised for a Kaigi session.
    pub struct SetKaigiRelayManifest {
        /// Identifier of the call to update.
        pub call_id: KaigiId,
        /// Optional relay manifest describing the desired relay hops.
        pub relay_manifest: Option<KaigiRelayManifest>,
    }
}

isi! {
    /// Register or update a Kaigi relay within its home domain.
    pub struct RegisterKaigiRelay {
        /// Registration payload describing the relay capabilities.
        pub relay: KaigiRelayRegistration,
    }
}

isi! {
    /// Report the observed health for a relay participating in a Kaigi session.
    pub struct ReportKaigiRelayHealth {
        /// Identifier of the call where the relay was observed.
        pub call_id: KaigiId,
        /// Relay account whose health is being reported.
        pub relay_id: AccountId,
        /// Health status observed by the reporter.
        pub status: KaigiRelayHealthStatus,
        /// Timestamp (milliseconds since epoch) for when the observation occurred.
        pub reported_at_ms: u64,
        /// Optional free-form notes capturing failure context.
        pub notes: Option<String>,
    }
}

// Seal implementations
impl crate::seal::Instruction for CreateKaigi {}
impl crate::seal::Instruction for JoinKaigi {}
impl crate::seal::Instruction for LeaveKaigi {}
impl crate::seal::Instruction for EndKaigi {}
impl crate::seal::Instruction for RecordKaigiUsage {}
impl crate::seal::Instruction for SetKaigiRelayManifest {}
impl crate::seal::Instruction for RegisterKaigiRelay {}
impl crate::seal::Instruction for ReportKaigiRelayHealth {}
