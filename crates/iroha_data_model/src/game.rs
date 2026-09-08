//! Generic deterministic multiplayer session manifests, authorization and retained consensus state.
pub use crate::game_resources::*;
use crate::{NetworkId, account::AccountId, asset::AssetDefinitionId};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Maximum participants admitted by the generic bounded lifecycle.
pub const GAME_MAX_PARTICIPANTS_V1: usize = 32;
/// Consensus blocks for the immutable checkpoint selection window.
pub const GAME_CHECKPOINT_WINDOW_BLOCKS_V1: u64 = 30;
/// Consensus blocks per forced commitment or reveal phase.
pub const GAME_INPUT_WINDOW_BLOCKS_V1: u64 = 15;
macro_rules! game_record {
 ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $field:ident : $ty:ty,)* }) => {
  $(#[$meta])*
  #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
  #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
  #[norito (deny_unknown_fields)]
  pub struct $name { $($(#[$fm])* pub $field:$ty,)* }
 };
}
/// Entry authorization independent of transaction spending authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "public_key", rename_all = "snake_case")]
pub enum GameAccessV1 {
    /// Any wallet may join an available seat.
    Public,
    /// Join additionally requires a wallet-bound invitation signature.
    Invite(PublicKey),
}
/// Immutable payout interpretation of a verified generic outcome.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GamePayoutPolicyV1 {
    /// No stakes or transfers; retain a verified outcome only.
    NoPayout,
    /// Divide the retained pool among verified winners; no winners returns original stakes.
    EqualWinnersOrRefund,
}
game_record! {
 /// Canonical GameManifestV1 with application-independent bounded fields.
 pub struct GameManifestV1 {
  /// Canonical version bound by the session or proof.
  pub version:u16,
  /// Application namespace for compatible clients, committed in the manifest.
  pub application_id:Hash,
  /// Immutable compiled execution-verifier profile identifier.
  pub profile_id:Hash,
  /// Opaque canonical parameters validated by the selected execution relation.
  pub application_parameters:Vec<u8>,
  /// Minimum active participants; falling below ends input progress and requires a proved outcome.
  pub min_participants:u8,
  /// Maximum fixed roster slots, assigned in join order.
  pub max_participants:u8,
  /// Logical ticks covered by each participant input payload.
  pub batch_ticks:u16,
  /// Maximum logical ticks, divisible by the batch size.
  pub max_ticks:u32,
  /// Maximum length of one participant input payload.
  pub max_input_bytes:u16,
  /// Maximum length of one entrant's immutable application data.
  pub max_participant_data_bytes:u16,
  /// Public or wallet-bound invitation admission.
  pub access:GameAccessV1,
  /// Immutable interpretation of a proved outcome for the retained stake pool.
  pub payout_policy:GamePayoutPolicyV1,
 }
}
game_record! {
 /// Canonical GameOutcomeV1 with application-independent bounded fields.
 pub struct GameOutcomeV1 {
  /// Proved terminal logical tick; no future input may change the outcome.
  pub terminal_tick:u32,
  /// Unique eligible payout slots in increasing order, authenticated by the relation.
  pub winner_slots:Vec<u8>,
  /// Opaque canonical application result, authenticated by the execution proof.
  pub result:Vec<u8>,
 }
}
game_record! {
 /// An immutable awarded prize or refund and its independently claimable unpaid balance.
 pub struct GamePayoutClaimV1 {
  /// Original permanent roster slot that owns the claim.
  pub slot:u8,
  /// Exact amount fixed at proof settlement or lobby expiry.
  pub amount:Quantity,
  /// Exact amount still backed by the session's protected custody.
  pub remaining:Quantity,
 }
}
game_record! {
 /// Canonical GameParticipantV1 with application-independent bounded fields.
 pub struct GameParticipantV1 {
  /// Wallet that authorizes the exact stake and receives any eventual payout.
  pub account:AccountId,
  /// Ed25519 gameplay key with no wallet-spending authority.
  pub input_key:PublicKey,
  /// Immutable participant data interpreted only by the selected application adapter.
  pub application_data:Vec<u8>,
  /// First consensus removal tick; removes forced-input authority but preserves checkpoint consent.
  pub dnf_at_tick:Option<u32>,
 }
}
game_record! {
 /// Canonical GameCheckpointV1 with application-independent bounded fields.
 pub struct GameCheckpointV1 {
  /// Exact immutable session identifier.
  pub session_id:Hash,
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// Complete logical tick count at this certified checkpoint.
  pub tick:u32,
  /// Domain-separated commitment to the complete canonical input transcript prefix.
  pub transcript_root:Hash,
  /// Commitment to opaque canonical adapter state bytes at this checkpoint.
  pub state_root:Hash,
  /// Whether signers claim a terminal prefix; an execution proof must still validate it.
  pub terminal:bool,
 }
}
game_record! {
 /// Canonical GameSlotSignatureV1 with application-independent bounded fields.
 pub struct GameSlotSignatureV1 {
  /// Permanent participant slot assigned at admission.
  pub slot:u8,
  /// Canonical Ed25519 signature over the exact domain-separated body.
  pub signature:Signature,
 }
}
game_record! {
 /// Canonical SignedGameCheckpointV1 with application-independent bounded fields.
 pub struct SignedGameCheckpointV1 {
  /// Cumulative certified checkpoint; the native genesis checkpoint needs no signatures.
  pub checkpoint:GameCheckpointV1,
  /// Every original participant signature in increasing slot order.
  pub signatures:Vec<GameSlotSignatureV1>,
 }
}
game_record! {
 /// Canonical GameCommitmentSetV1 with application-independent bounded fields.
 pub struct GameCommitmentSetV1 {
  /// Exact immutable session identifier.
  pub session_id:Hash,
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// First logical tick controlled by this batch.
  pub start_tick:u32,
  /// Prior certified input transcript commitment extended by this batch.
  pub parent_transcript_root:Hash,
  /// One exact input commitment per permanent slot.
  pub commitments:Vec<Hash>,
  /// Every original participant signature in increasing slot order.
  pub signatures:Vec<GameSlotSignatureV1>,
 }
}
game_record! {
 /// Canonical GameInputCommitmentV1 with application-independent bounded fields.
 pub struct GameInputCommitmentV1 {
  /// Exact immutable session identifier.
  pub session_id:Hash,
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// First logical tick controlled by this batch.
  pub start_tick:u32,
  /// Permanent participant slot assigned at admission.
  pub slot:u8,
  /// Hash of exact session, epoch, tick, slot, opaque payload and salt.
  pub commitment:Hash,
  /// Canonical Ed25519 signature over the exact domain-separated body.
  pub signature:Signature,
 }
}
game_record! {
 /// Canonical GameInputRevealV1 with application-independent bounded fields.
 pub struct GameInputRevealV1 {
  /// Exact immutable session identifier.
  pub session_id:Hash,
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// First logical tick controlled by this batch.
  pub start_tick:u32,
  /// Permanent participant slot assigned at admission.
  pub slot:u8,
  /// Opaque bounded input bytes checked by the selected execution relation.
  pub payload:Vec<u8>,
  /// Unpredictable input-commitment salt, disclosed with the exact payload.
  pub salt:Hash,
 }
}
game_record! {
 /// Canonical GameForcedBatchV1 with application-independent bounded fields.
 pub struct GameForcedBatchV1 {
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// First logical tick controlled by this batch.
  pub start_tick:u32,
  /// Opaque payload per permanent slot; already removed slots use empty bytes.
  pub inputs:Vec<Vec<u8>>,
  /// Slots newly removed by an expired consensus deadline before this batch.
  pub dnf_slots:Vec<u8>,
 }
}
game_record! {
 /// Canonical GameTranscriptBatchV1 with application-independent bounded fields.
 pub struct GameTranscriptBatchV1 {
  /// First logical tick controlled by this batch.
  pub start_tick:u32,
  /// Opaque payload per permanent slot; already removed slots use empty bytes.
  pub inputs:Vec<Vec<u8>>,
 }
}
game_record! {
 /// Canonical GameDnfEventV1 with application-independent bounded fields.
 pub struct GameDnfEventV1 {
  /// Complete logical tick count at this certified checkpoint.
  pub tick:u32,
  /// Ordered slots removed at this exact logical tick.
  pub slots:Vec<u8>,
 }
}
game_record! {
 /// Canonical GameTranscriptV1 with application-independent bounded fields.
 pub struct GameTranscriptV1 {
  /// Complete ordered opaque input batches without gaps or overlaps.
  pub batches:Vec<GameTranscriptBatchV1>,
  /// Canonical consensus removals included at their exact logical boundary.
  pub dnf_events:Vec<GameDnfEventV1>,
 }
}
game_record! {
    /// An authenticated transcript prefix retained when session authority shrinks.
    pub struct GameTranscriptAnchorV1 {
        /// Complete tick count at the immutable certified prefix.
        pub tick:u32,
        /// Exact cumulative input transcript commitment.
        pub transcript_root:Hash,
    }
}
impl Copy for GameTranscriptAnchorV1 {}
game_record! {
 /// One explicitly wallet-staked indivisible item with an immutable native award policy.
 pub struct GameItemStakeV1 {
  /// Permanent original-owner slot; V1 admits at most one NFT per slot.
  pub slot:u8,
  /// Exact NFT selected by its authenticated owner.
  pub nft_id:crate::nft::NftId,
  /// Non-signing shared native NFT custody identity.
  pub custody:AccountId,
  /// Frozen original metadata commitment.
  pub metadata_hash:Hash,
  /// Native sole winner, or original owner for ties, no winner and cancellation.
  pub recipient:Option<AccountId>,
  /// True only once the NFT transfer has completed atomically with the terminal result.
  pub claimed:bool,
 }
}
game_record! {
 /// Canonical GameSessionRecordV1 with application-independent bounded fields.
 pub struct GameSessionRecordV1 {
  /// Canonical version bound by the session or proof.
  pub version:u16,
  /// Canonical network id bound by the session or proof.
  pub network_id:NetworkId,
  /// Exact immutable session identifier.
  pub session_id:Hash,
  /// Immutable application, execution profile, limits, access and payout policy.
  pub manifest:GameManifestV1,
  /// Immutable compiled execution-verifier profile identifier.
  pub profile_id:Hash,
  /// Network-bound canonical manifest commitment.
  pub manifest_hash:Hash,
  /// Numeric asset funding the optional entry stakes.
  pub asset_definition:AssetDefinitionId,
  /// Exact equal stake per wallet; zero enables a session without custody transfers.
  pub stake:Quantity,
  /// Immutable payout quantum: asset scale, or the protocol maximum for unrestricted assets.
  pub payout_scale:u32,
  /// Derived non-signing custody account, permanently protected from generic debits.
  pub custody:AccountId,
  /// Exact unpaid stake pool or outstanding claim sum, matching the exclusive custody balance.
  pub liability:Quantity,
  /// Immutable awards with bounded outstanding amounts; closed sessions retain custody until paid.
  pub payout_claims:Vec<GamePayoutClaimV1>,
  /// Explicit item stakes ordered by original slot; frozen into the starting roster commitment.
  pub item_stakes:Vec<GameItemStakeV1>,
  /// Explicit returnable equipment retained in permanent slot and role order.
  pub resources:Vec<GameResourceReservationRecordV1>,
  /// Immutable wallet/key/data roster with consensus-owned removal markers.
  pub participants:Vec<GameParticipantV1>,
  /// Commitment to the original admitted roster before removal markers change.
  pub roster_hash:Hash,
  /// Current native lifecycle phase.
  pub phase:GamePhaseV1,
  /// Monotone native state revision used by clients and transition events.
  pub revision:u64,
  /// Consensus-owned generation preventing replay of previous forced input batches.
  pub epoch:u64,
  /// Consensus height through which the current phase accepts evidence.
  pub deadline_height:u64,
  /// Cumulative certified checkpoint; the native genesis checkpoint needs no signatures.
  pub checkpoint:Option<SignedGameCheckpointV1>,
  /// Certified pending input frontier that cannot be discarded after disclosure.
  pub pending_certificate:Option<GameCommitmentSetV1>,
  /// Next batch boundary for permissionless forced progress.
  pub next_tick:u32,
  /// Exact accepted commitments for the current forced batch, indexed by slot.
  pub input_commitments:Vec<Option<Hash>>,
  /// Available matching opaque reveals for the current forced batch.
  pub input_reveals:Vec<Option<Vec<u8>>>,
  /// Canonical forced batches bound by the session or proof.
  pub transcript_anchors:Vec<GameTranscriptAnchorV1>,
  /// Complete immutable native forced progress.
  pub forced_batches:Vec<GameForcedBatchV1>,
  /// Commitment to exact checkpoint, epoch, immutable anchors and all forced batches.
  pub dispute_root:Hash,
  /// Compact first mathematical-verification receipt; it may predate settlement.
  pub verification_id:Option<Hash>,
  /// Consensus height of the legal Settled or Cancelled transition.
  pub terminal_at_height:Option<u64>,
  /// Opaque canonical application result, authenticated by the execution proof.
  pub result:Option<GameOutcomeV1>,
 }
}
/// Monotone lifecycle phases; only proof validation authorizes settlement.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GamePhaseV1 {
    /// Wallet-bound participant admission.
    Lobby,
    /// Live off-chain exchange of certified inputs.
    Playing,
    /// Fixed opportunity to publish the newest jointly certified checkpoint.
    SelectingCheckpoint,
    /// Bounded on-chain input commitments.
    ForcedCommit,
    /// Bounded on-chain input data availability.
    ForcedReveal,
    /// The selected history awaits a genuine terminal execution proof.
    AwaitingProof,
    /// Proof and awards finalized; fungible claims may remain unpaid.
    Settled,
    /// An unstartable lobby expired and fixed exact stake refunds.
    Cancelled,
}
/// Canonical domain-separated message; never a wallet transaction signature.
pub fn game_message_hash_v1<T: Encode>(network: &NetworkId, domain: &str, body: &T) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha:game:session:v1\0",
        network.as_bytes(),
        domain.as_bytes(),
        &body.encode(),
    ])
}
/// Commit to exact session, generation, slot, opaque input bytes, and salt.
pub fn game_input_commitment_v1(network: &NetworkId, input: &GameInputRevealV1) -> Hash {
    game_message_hash_v1(network, "input-reveal", input)
}
/// Exclude signatures from the jointly signed commitment body.
pub fn game_commitment_set_hash_v1(network: &NetworkId, set: &GameCommitmentSetV1) -> Hash {
    game_message_hash_v1(
        network,
        "commitment-set",
        &(
            set.session_id,
            set.epoch,
            set.start_tick,
            set.parent_transcript_root,
            set.commitments.clone(),
        ),
    )
}
/// Exclude the authentication signature from an individual commitment body.
pub fn game_input_message_hash_v1(network: &NetworkId, input: &GameInputCommitmentV1) -> Hash {
    game_message_hash_v1(
        network,
        "input-commitment",
        &(
            input.session_id,
            input.epoch,
            input.start_tick,
            input.slot,
            input.commitment,
        ),
    )
}
/// Bind a private invitation to the exact paying wallet and application data.
pub fn game_invitation_hash_v1(
    network: &NetworkId,
    session_id: Hash,
    wallet: &AccountId,
    input_key: &PublicKey,
    application_data: &[u8],
) -> Hash {
    game_message_hash_v1(
        network,
        "invitation",
        &(
            session_id,
            wallet.clone(),
            input_key.clone(),
            application_data.to_vec(),
        ),
    )
}

impl Copy for GamePayoutPolicyV1 {}
impl Copy for GameCheckpointV1 {}

/// Commit exactly the immutable admission projection for every session.
pub fn game_roster_hash_v1(
    network: &NetworkId,
    session_id: &Hash,
    admission: &GameAdmissionBodyV1,
) -> Hash {
    game_message_hash_v1(network, "roster", &(*session_id, admission.clone()))
}

/// Bounded application data in one immutable admission participant.
pub const GAME_ADMISSION_MAX_PARTICIPANT_DATA_BYTES_V1: usize = 4096;
/// Encoded controller and account framing bound, independently of display geometry.
pub const GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1: usize = 32 * 1024;
/// Encoded NFT identifier bound, independently of display geometry.
pub const GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1: usize = 1024;
/// Complete admission encoding bound; individual fields and cardinalities are also bounded.
pub const GAME_ADMISSION_MAX_BYTES_V1: usize = 1024 * 1024;
/// All wagers and returnable resources in one cumulative native NFT operation.
pub const GAME_MAX_NFT_RESERVATIONS_V1: usize =
    GAME_MAX_PARTICIPANTS_V1 + GAME_MAX_RESOURCE_RECORDS_V1;
game_record! {
    /// Immutable authenticated participant facts, without removal markers.
    pub struct GameAdmissionParticipantV1 {
        /// Wallet authorizing admission.
        pub account: AccountId,
        /// Race-specific Ed25519 gameplay authority.
        pub input_key: PublicKey,
        /// Compiled adapter data approved by the wallet.
        pub application_data: Vec<u8>,
    }
}
game_record! {
    /// Immutable wager facts; custody and recipient are derived by Core.
    pub struct GameAdmissionWagerV1 {
        /// Original participant slot.
        pub slot: u8,
        /// Explicitly wagered NFT.
        pub nft_id: crate::nft::NftId,
        /// Complete frozen metadata commitment.
        pub metadata_hash: Hash,
    }
}
game_record! {
    /// Immutable returnable equipment authorization.
    pub struct GameAdmissionResourceV1 {
        /// Original participant slot.
        pub slot: u8,
        /// Explicitly reserved NFT.
        pub nft_id: crate::nft::NftId,
        /// Complete frozen metadata commitment.
        pub metadata_hash: Hash,
        /// Exact compiled role.
        pub role_id: Hash,
        /// Immutable terminal return condition.
        pub policy: GameResourceReturnPolicyV1,
    }
}
game_record! {
    /// One compact immutable admission body shared by ledger, proof and browser.
    pub struct GameAdmissionBodyV1 {
        /// Exactly one.
        pub version: u16,
        /// Original participants in permanent slot order.
        pub participants: Vec<GameAdmissionParticipantV1>,
        /// Wagers in strictly increasing original slot order.
        pub wagers: Vec<GameAdmissionWagerV1>,
        /// Equipment in strictly increasing (slot, role_id) order.
        pub resources: Vec<GameAdmissionResourceV1>,
    }
}
impl GameAdmissionBodyV1 {
    /// Project immutable facts without sorting, normalizing or accepting aliases.
    pub fn from_session(session: &GameSessionRecordV1) -> Self {
        Self {
            version: 1,
            participants: session
                .participants
                .iter()
                .map(|p| GameAdmissionParticipantV1 {
                    account: p.account.clone(),
                    input_key: p.input_key.clone(),
                    application_data: p.application_data.clone(),
                })
                .collect(),
            wagers: session
                .item_stakes
                .iter()
                .map(|item| GameAdmissionWagerV1 {
                    slot: item.slot,
                    nft_id: item.nft_id.clone(),
                    metadata_hash: item.metadata_hash,
                })
                .collect(),
            resources: session
                .resources
                .iter()
                .map(|r| GameAdmissionResourceV1 {
                    slot: r.slot,
                    nft_id: r.nft_id.clone(),
                    metadata_hash: r.metadata_hash,
                    role_id: r.role_id,
                    policy: r.policy,
                })
                .collect(),
        }
    }
    /// Validate bounded canonical authorization geometry. Adapters impose their own exact roster requirements.
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::BTreeSet;
        if self.version != 1
            || self.participants.len() > GAME_MAX_PARTICIPANTS_V1
            || self.wagers.len() > GAME_MAX_PARTICIPANTS_V1
            || self.resources.len() > GAME_MAX_RESOURCE_RECORDS_V1
        {
            return Err("invalid game admission version or cardinality".into());
        }
        let mut accounts = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for p in &self.participants {
            if p.account.to_string().len() > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1
                || p.account.encode().len() > GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1
                || p.application_data.len() > GAME_ADMISSION_MAX_PARTICIPANT_DATA_BYTES_V1
                || p.input_key.algorithm() != iroha_crypto::Algorithm::Ed25519
                || !accounts.insert(&p.account)
                || !keys.insert(&p.input_key)
            {
                return Err("invalid or duplicated game admission participant".into());
            }
        }
        let mut nfts = BTreeSet::new();
        let mut previous = None;
        for wager in &self.wagers {
            validate_game_nft_identity_v1(&wager.nft_id).map_err(str::to_owned)?;
            if usize::from(wager.slot) >= self.participants.len()
                || previous.is_some_and(|slot| slot >= wager.slot)
                || wager.nft_id.to_string().len() > GAME_RESOURCE_MAX_NFT_ID_BYTES_V1
                || wager.nft_id.encode().len() > GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1
                || !nfts.insert(&wager.nft_id)
            {
                return Err("noncanonical game admission wagers".into());
            }
            previous = Some(wager.slot);
        }
        let mut previous = None;
        let mut counts = [0_usize; GAME_MAX_PARTICIPANTS_V1];
        for r in &self.resources {
            validate_game_nft_identity_v1(&r.nft_id).map_err(str::to_owned)?;
            let slot = usize::from(r.slot);
            let key = (r.slot, r.role_id);
            if slot >= self.participants.len()
                || previous.is_some_and(|old| old >= key)
                || r.nft_id.to_string().len() > GAME_RESOURCE_MAX_NFT_ID_BYTES_V1
                || r.nft_id.encode().len() > GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1
                || !nfts.insert(&r.nft_id)
            {
                return Err("noncanonical or overlapping game admission resources".into());
            }
            counts[slot] += 1;
            if counts[slot] > GAME_MAX_RESOURCES_PER_PARTICIPANT_V1 {
                return Err("too many admission resources for one participant".into());
            }
            previous = Some(key);
        }
        if self.encode().len() > GAME_ADMISSION_MAX_BYTES_V1 {
            return Err("game admission exceeds its encoded byte bound".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "game_admission_tests.rs"]
mod admission_tests;
