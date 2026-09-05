//! Versioned native race state and gameplay-only signed action records.
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    execution_proofs::{RaceResultV1, RaceTrackV1},
};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Maximum racers accepted by the immutable V1 relation.
pub const RACE_MAX_RACERS_V1: usize = 8;
/// Number of deterministic simulation ticks in one input batch.
pub const RACE_INPUT_BATCH_TICKS_V1: usize = 6;
/// Simulation tick ceiling; deadlines cannot restart it.
pub const RACE_MAX_TICKS_V1: u32 = 5_400;
/// Consensus blocks available to publish a newer certified frontier.
pub const RACE_CHECKPOINT_WINDOW_BLOCKS_V1: u64 = 30;
/// Consensus blocks available per forced commit or reveal phase.
pub const RACE_INPUT_WINDOW_BLOCKS_V1: u64 = 15;

macro_rules! race_record {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$field_meta:meta])* pub $field:ident : $ty:ty,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize))]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        pub struct $name { $($(#[$field_meta])* pub $field : $ty,)* }
    };
}
race_record! {
    /// Immutable selected parameters of the compiled Race V1 rules.
    #[derive(Copy)]
    pub struct RaceRulesV1 {
        /// Schema version, exactly one.
        pub version: u16,
        /// Compiled track; arbitrary track geometry is not admitted.
        pub track: RaceTrackV1,
        /// Number of seats, between two and eight inclusive.
        pub max_racers: u8,
    }
}
race_record! {
    /// Wallet-authorized racer; the input key cannot spend wallet funds.
    pub struct RaceParticipantV1 {
        /// Immutable payout destination.
        pub account: AccountId,
        /// Ed25519 key scoped to this race's gameplay messages.
        pub input_key: PublicKey,
        /// Compiled car configuration, between zero and five inclusive.
        pub car_id: u8,
        /// First simulation tick at which this slot no longer has a collision body.
        pub dnf_at_tick: Option<u32>,
    }
}
race_record! {
    /// Certificate body signed by every consensus-active slot.
    #[derive(Copy)]
    pub struct RaceCheckpointV1 {
        /// Exact race identifier.
        pub race_id: Hash,
        /// Chain-owned dispute generation.
        pub epoch: u64,
        /// Complete number of simulated ticks at this checkpoint.
        pub tick: u32,
        /// Root of the complete canonical control transcript.
        pub transcript_root: Hash,
        /// Deterministic simulation state commitment.
        pub state_root: Hash,
        /// Whether this is a terminal certificate authorizing result proof submission.
        pub terminal: bool,
    }
}
race_record! {
    /// A fixed slot's signature; duplicate or out-of-order slots are rejected.
    pub struct RaceSlotSignatureV1 {
        /// Permanent grid slot.
        pub slot: u8,
        /// Exact domain-separated gameplay-message signature.
        pub signature: Signature,
    }
}
race_record! {
    /// Authenticated cumulative checkpoint, without trusting its computation.
    pub struct SignedRaceCheckpointV1 {
        /// Signed checkpoint body.
        pub checkpoint: RaceCheckpointV1,
        /// One signature for every active slot, in slot order.
        pub signatures: Vec<RaceSlotSignatureV1>,
    }
}
race_record! {
    /// Jointly certified commitments to one pending input batch.
    pub struct RaceCommitmentSetV1 {
        /// Exact race identifier.
        pub race_id: Hash,
        /// Chain-owned dispute generation.
        pub epoch: u64,
        /// First tick controlled by this batch.
        pub start_tick: u32,
        /// Prior certified cumulative transcript root.
        pub parent_transcript_root: Hash,
        /// One commitment per permanent slot, including zero-input DNF slots.
        pub commitments: Vec<Hash>,
        /// Every active slot signs the identical commitment-set body.
        pub signatures: Vec<RaceSlotSignatureV1>,
    }
}
race_record! {
    /// An authenticated per-racer commitment accepted during forced play.
    pub struct RaceInputCommitmentV1 {
        /// Exact race identifier.
        pub race_id: Hash,
        /// Chain-owned dispute generation.
        pub epoch: u64,
        /// First simulation tick of this batch.
        pub start_tick: u32,
        /// Permanent racer slot.
        pub slot: u8,
        /// Commitment to exact bounded controls and salt.
        pub commitment: Hash,
        /// Gameplay-only signature over all preceding fields and the network.
        pub signature: Signature,
    }
}
race_record! {
    /// Reveal of six controls matching a retained commitment; anybody may relay it.
    pub struct RaceInputRevealV1 {
        /// Exact race identifier.
        pub race_id: Hash,
        /// Chain-owned dispute generation.
        pub epoch: u64,
        /// First simulation tick of the batch.
        pub start_tick: u32,
        /// Permanent racer slot.
        pub slot: u8,
        /// Six control masks, using only bits zero through five.
        pub controls: Vec<u16>,
        /// Unpredictable 32-byte opening salt.
        pub salt: Hash,
    }
}
race_record! {
    /// Complete consensus-selected input batch retained for execution-proof binding.
    pub struct RaceForcedBatchV1 {
        /// Generation in which consensus selected these controls.
        pub epoch: u64,
        /// First simulation tick of the batch.
        pub start_tick: u32,
        /// Six controls per permanent slot, in slot order.
        pub controls: Vec<Vec<u16>>,
        /// Slots first declared DNF before this batch is evaluated.
        pub dnf_slots: Vec<u8>,
    }
}
/// Exhaustive native race lifecycle phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum RacePhaseV1 {
    /// Seats may be joined until the height deadline.
    Lobby,
    /// Live peer-to-peer driving with jointly certified inputs.
    Racing,
    /// Fixed window for newer checkpoints and certified pending inputs.
    SelectingCheckpoint,
    /// Height-bounded forced commitments.
    ForcedCommit,
    /// Height-bounded forced input availability.
    ForcedReveal,
    /// Canonical transcript is terminal and awaits a valid execution proof.
    AwaitingProof,
    /// Native proof verification and payout completed atomically.
    Settled,
    /// Pre-start expiry refunded all recorded stakes.
    Cancelled,
}
race_record! {
    /// Canonical query-visible race record; changes participate in the World state root.
    pub struct RaceRecordV1 {
        /// Persisted schema version.
        pub version: u16,
        /// Exact chain identity.
        pub network_id: NetworkId,
        /// Immutable race identifier.
        pub race_id: Hash,
        /// Immutable selected rules.
        pub rules: RaceRulesV1,
        /// Compiled proof profile.
        pub profile_id: Hash,
        /// Exact compiled physics and track relation commitment.
        pub rules_hash: Hash,
        /// Exact canonical XOR definition; aliases are resolved before opening.
        pub asset_definition: AssetDefinitionId,
        /// Recorded entry stake per participant.
        pub stake: Quantity,
        /// Deterministic non-signable native custody account.
        pub custody: AccountId,
        /// Recorded liability; donations never increase it.
        pub liability: Quantity,
        /// Permanent grid order and wallet-authorized gameplay keys.
        pub participants: Vec<RaceParticipantV1>,
        /// Immutable initial roster commitment after start.
        pub roster_hash: Hash,
        /// Current native lifecycle phase.
        pub phase: RacePhaseV1,
        /// Monotonic state revision.
        pub revision: u64,
        /// Monotonic dispute generation.
        pub epoch: u64,
        /// Inclusive current phase deadline, measured only in consensus block height.
        pub deadline_height: u64,
        /// Highest accepted checkpoint and its certificate.
        pub checkpoint: Option<SignedRaceCheckpointV1>,
        /// Certified input frontier that cannot be discarded after reveal.
        pub pending_certificate: Option<RaceCommitmentSetV1>,
        /// Next forced simulation tick.
        pub next_tick: u32,
        /// Slot-indexed current commitments, absent until published.
        pub input_commitments: Vec<Option<Hash>>,
        /// Slot-indexed accepted reveals.
        pub input_reveals: Vec<Option<Vec<u16>>>,
        /// Complete ordered chain-resolved input history.
        pub forced_batches: Vec<RaceForcedBatchV1>,
        /// Exact commitment to checkpoint generation and forced input history.
        pub dispute_root: Hash,
        /// Result retained only after successful native proof verification.
        pub result: Option<RaceResultV1>,
    }
}
/// Domain-separated canonical gameplay-message digest. It never signs a transaction.
pub fn race_message_hash_v1<T: Encode>(network: &NetworkId, domain: &str, body: &T) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha:race:gameplay:v1\0",
        network.as_bytes(),
        domain.as_bytes(),
        &body.encode(),
    ])
}
/// Exact commitment to a reveal, including race, epoch, slot and salt.
pub fn race_input_commitment_v1(network: &NetworkId, reveal: &RaceInputRevealV1) -> Hash {
    race_message_hash_v1(network, "input-reveal", reveal)
}
/// Canonical message signed for a jointly certified commitment frontier.
pub fn race_commitment_set_hash_v1(network: &NetworkId, set: &RaceCommitmentSetV1) -> Hash {
    race_message_hash_v1(
        network,
        "commitment-set",
        &(
            set.race_id,
            set.epoch,
            set.start_tick,
            set.parent_transcript_root,
            set.commitments.clone(),
        ),
    )
}
/// Canonical message signed for an individual forced commitment.
pub fn race_input_message_hash_v1(network: &NetworkId, input: &RaceInputCommitmentV1) -> Hash {
    race_message_hash_v1(
        network,
        "input-commitment",
        &(
            input.race_id,
            input.epoch,
            input.start_tick,
            input.slot,
            input.commitment,
        ),
    )
}
