//! What World keeps for the anonymous ballot of a binding body (design §3.2, §3.3, §3.7,
//! §3.8 and §12.3): the compact credential of each accepted assignment and the board of
//! each sealed body.
//!
//! These are the value layouts of `world.parliament_anonymous_registrations` and
//! `world.parliament_anonymous_boards`. They hold public facts only. A registration proof
//! stays in the signed input that carried it; a ballot proof stays in the canonical
//! envelope its corpus entry names.
//!
//! [`ParliamentBallotBoardRecordV1::validate`] checks structure. A node trusts a stored
//! board only after `iroha_parliament_ballot` rebuilt it: the context, the corpus root, the
//! counts and the closure record are recomputed and compared there, because only the proof
//! crate computes the `P16`-bound digests.

use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    AnonymousBallotCorpusEntryV1, AuthenticatedBallotContextV1, BALLOT_CONTEXT_MAX_BYTES_V1,
    BallotClosureRecordV1, BallotContextErrorV1, CredentialCommitmentV1, Digest512,
    MAX_SEALED_SEATS_V1, ParliamentBallotPurposeErrorV1, ParliamentBallotPurposeV1,
    RegistrationAuthorizationV1, framing::AnonymousBallotCodecErrorV1,
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::{
        GovernanceAnchorV1,
        types::{BodyInstanceId, ProposalContentId},
    },
    transaction::TransactionEntrypoint,
};

/// Largest canonical frame of one stored board: the context cap, one compact corpus entry
/// per sealed seat and a fixed allowance for the identities, anchors and closure record.
pub const BALLOT_BOARD_RECORD_MAX_BYTES_V1: usize =
    BALLOT_CONTEXT_MAX_BYTES_V1 + MAX_SEALED_SEATS_V1 as usize * 512 + 8_192;

/// Where finalized history holds the signed input that carried a registration proof.
///
/// The proof is 160,148 bytes and is verified once, when its input executes. State keeps
/// this locator so that the proof can be retrieved and verified again from authenticated
/// block history; the locator is not a substitute for that history's availability.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::ParliamentCredentialSourceV1"
)]
pub struct ParliamentCredentialSourceV1 {
    /// Nonzero height of the block that executed the registration.
    pub height: u64,
    /// Position of the signed input in that block's global network-input projection.
    pub input_index: u64,
    /// Position of the registration among the input's own instructions.
    pub instruction_index: u32,
    /// Hash of the signed input as a transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
}

/// The credential an accepted assignment registered, as World keeps it.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::ParliamentCredentialRecordV1"
)]
pub struct ParliamentCredentialRecordV1 {
    /// Credential commitment `C`.
    pub commitment: CredentialCommitmentV1,
    /// Registration authorization value `auth`, verified under `pi_reg`.
    pub authorization: RegistrationAuthorizationV1,
    /// The signed input that carried `pi_reg`.
    pub source: ParliamentCredentialSourceV1,
}

/// The casting clock of a ballot: the first block executed after its opening block.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::governance::anonymous_ballot::BallotCastingClockV1")]
pub struct BallotCastingClockV1 {
    /// Height of the casting-clock block, `h_open + 1`.
    pub height: u64,
    /// Block time of that block in milliseconds; the closing time is this plus the
    /// frozen casting window.
    pub timestamp_ms: u64,
}

/// Why a stored board is structurally invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParliamentBallotBoardRecordErrorV1 {
    /// The purpose is not one this release may record.
    #[error(transparent)]
    Purpose(#[from] ParliamentBallotPurposeErrorV1),
    /// The authenticated context is structurally invalid.
    #[error(transparent)]
    Context(#[from] BallotContextErrorV1),
    /// The corpus holds more entries than sealed seats.
    #[error("the corpus holds {entries} entries for {seats} sealed seats")]
    CorpusSize {
        /// Entries stored.
        entries: usize,
        /// Sealed seats `S`.
        seats: u16,
    },
    /// The record exceeds its frame cap or cannot be encoded.
    #[error(transparent)]
    Codec(#[from] AnonymousBallotCodecErrorV1),
}

/// One sealed binding body's ballot, as World keeps it from seal to closure.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::ParliamentBallotBoardRecordV1"
)]
pub struct ParliamentBallotBoardRecordV1 {
    /// Purpose of the attempt the body belongs to, fixed when the attempt was created.
    pub purpose: ParliamentBallotPurposeV1,
    /// The authenticated context of the sealed body, with the complete sealed list.
    pub context: AuthenticatedBallotContextV1,
    /// The sealed body instance, derived from [`Self::roster_root`].
    pub body_instance_id: BodyInstanceId,
    /// The proposal content identifier recorded at closure.
    pub proposal_content_id: ProposalContentId,
    /// The 64-byte roster root of the extended preimage `(assignments, R, S, profile_id)`.
    pub roster_root: Digest512,
    /// The registration-mapping digest of the sealed `(assignment_id, C)` pairs.
    pub registration_mapping_digest: Digest512,
    /// Block time of the seal in milliseconds; the vote notice runs from it.
    pub sealed_timestamp_ms: u64,
    /// `opening_at`, fixed when the body entered Reflection.
    pub opening_at_ms: Option<u64>,
    /// The opening anchor `s0`.
    pub opening: Option<GovernanceAnchorV1>,
    /// The casting clock of `h_open + 1`.
    pub casting_clock: Option<BallotCastingClockV1>,
    /// The accepted corpus in canonical order, at most one entry per sealed seat.
    pub entries: Vec<AnonymousBallotCorpusEntryV1>,
    /// The closure record, once the ballot closed.
    pub closure: Option<BallotClosureRecordV1>,
}

impl ParliamentBallotBoardRecordV1 {
    /// Kind named in size errors.
    const KIND: &'static str = "ballot board record";

    /// Check the purpose, the context's structure, the corpus size and the frame cap.
    ///
    /// This is not the authenticity check: the derived digests, the corpus root, the
    /// anchors' order and the closure record are recomputed by `iroha_parliament_ballot`.
    ///
    /// # Errors
    /// The first violated rule as a [`ParliamentBallotBoardRecordErrorV1`].
    pub fn validate(&self) -> Result<(), ParliamentBallotBoardRecordErrorV1> {
        self.purpose.validate()?;
        self.context.validate()?;
        if self.entries.len() > usize::from(self.context.seats) {
            return Err(ParliamentBallotBoardRecordErrorV1::CorpusSize {
                entries: self.entries.len(),
                seats: self.context.seats,
            });
        }
        let actual = norito::canonical_frame_len(self)
            .map_err(|error| AnonymousBallotCodecErrorV1::codec(&error))?;
        if actual > BALLOT_BOARD_RECORD_MAX_BYTES_V1 {
            return Err(AnonymousBallotCodecErrorV1::TooLarge {
                kind: Self::KIND,
                actual,
                max: BALLOT_BOARD_RECORD_MAX_BYTES_V1,
            }
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;

    use super::*;
    use crate::governance::{
        anonymous_ballot::{BallotChoiceV1, ParliamentCandidateTestingPolicyV1, test_fixtures},
        types::{ParliamentBody, RiskTierV1},
    };

    fn candidate() -> ParliamentBallotPurposeV1 {
        ParliamentBallotPurposeV1::CandidateTesting(
            ParliamentCandidateTestingPolicyV1::CANDIDATE_V1,
        )
    }

    fn record(seats: u16) -> ParliamentBallotBoardRecordV1 {
        let mut context = test_fixtures::context();
        let list = test_fixtures::sealed_list(seats);
        context.seats = list.seats;
        context.commitments = list.commitments;
        ParliamentBallotBoardRecordV1 {
            purpose: candidate(),
            context,
            body_instance_id: BodyInstanceId::new([0x03; 32]),
            proposal_content_id: ProposalContentId::new([0x01; 32]),
            roster_root: Digest512::new([0x0a; 64]),
            registration_mapping_digest: Digest512::new([0x0b; 64]),
            sealed_timestamp_ms: 400_000,
            opening_at_ms: None,
            opening: None,
            casting_clock: None,
            entries: Vec::new(),
            closure: None,
        }
    }

    fn entry(nullifier: u8, height: u64, input_index: u32) -> AnonymousBallotCorpusEntryV1 {
        AnonymousBallotCorpusEntryV1::from_envelope(
            &test_fixtures::envelope(BallotChoiceV1::Aye, nullifier),
            Digest512::new([nullifier; 64]),
            height,
            input_index,
        )
        .expect("corpus entry")
    }

    #[test]
    fn credential_records_roundtrip_and_name_their_source() {
        let record = ParliamentCredentialRecordV1 {
            commitment: test_fixtures::commitment(7),
            authorization: RegistrationAuthorizationV1::new([0x41; 64]),
            source: ParliamentCredentialSourceV1 {
                height: 12,
                input_index: 3,
                instruction_index: 0,
                entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0x5c; 32])),
            },
        };
        let bytes = norito::encode_canonical(&record).expect("encode");
        assert_eq!(
            norito::decode_canonical::<ParliamentCredentialRecordV1>(&bytes).expect("decode"),
            record
        );
        let json = norito::json::to_json(&record).expect("json");
        assert_eq!(
            norito::json::from_str::<ParliamentCredentialRecordV1>(&json).expect("from json"),
            record
        );
        // The compact record is a few hundred bytes; the proof is not in it.
        assert!(bytes.len() < 512, "{} bytes", bytes.len());
    }

    #[test]
    fn a_sealed_board_roundtrips_through_every_phase() {
        let mut board = record(3);
        assert_eq!(board.validate(), Ok(()));
        board.opening_at_ms = Some(86_800_000);
        board.opening = Some(test_fixtures::anchor(100, 2, 87_000_000));
        board.casting_clock = Some(BallotCastingClockV1 {
            height: 101,
            timestamp_ms: 87_001_000,
        });
        board.entries = vec![entry(1, 100, 3), entry(2, 101, 0), entry(3, 101, 1)];
        board.closure = Some(test_fixtures::closure(3, 3, 0, 0));
        assert_eq!(board.validate(), Ok(()));
        let bytes = norito::encode_canonical(&board).expect("encode");
        assert_eq!(
            norito::decode_canonical::<ParliamentBallotBoardRecordV1>(&bytes).expect("decode"),
            board
        );
        let json = norito::json::to_json(&board).expect("json");
        assert_eq!(
            norito::json::from_str::<ParliamentBallotBoardRecordV1>(&json).expect("from json"),
            board
        );
    }

    #[test]
    fn a_full_board_of_a_thousand_seats_fits_its_frame_cap() {
        // Only the Confirmation Jury seats a thousand; the Policy Jury target is 500.
        let mut board = record(3);
        let list = test_fixtures::sealed_list(MAX_SEALED_SEATS_V1);
        board.context.body_context.body_role = ParliamentBody::ConfirmationJury;
        board.context.policy = test_fixtures::attempt_policy()
            .ballot_policy(RiskTierV1::Standard, ParliamentBody::ConfirmationJury)
            .expect("confirmation ballot policy");
        board.context.seats = list.seats;
        board.context.commitments = list.commitments;
        assert_eq!(
            record(MAX_SEALED_SEATS_V1).validate(),
            Err(ParliamentBallotBoardRecordErrorV1::Context(
                BallotContextErrorV1::SeatsAboveTarget {
                    seats: MAX_SEALED_SEATS_V1,
                    target_seats: 500
                }
            )),
            "a Policy Jury board never holds more seats than its target",
        );
        board.entries = (0..MAX_SEALED_SEATS_V1)
            .map(|index| {
                let mut entry = entry(1, 100, u32::from(index));
                let mut nullifier = [0_u8; 128];
                nullifier[..2].copy_from_slice(&index.to_le_bytes());
                entry.nullifier = nullifier.into();
                entry
            })
            .collect();
        board.closure = Some(test_fixtures::closure(MAX_SEALED_SEATS_V1, 1_000, 0, 0));
        assert_eq!(board.validate(), Ok(()));
        let bytes = norito::encode_canonical(&board).expect("encode").len();
        assert!(bytes <= BALLOT_BOARD_RECORD_MAX_BYTES_V1, "{bytes} bytes");
    }

    #[test]
    fn structural_violations_are_refused() {
        let mut oversubscribed = record(3);
        oversubscribed.entries = (1..=4).map(|n| entry(n, 100, u32::from(n))).collect();
        assert_eq!(
            oversubscribed.validate(),
            Err(ParliamentBallotBoardRecordErrorV1::CorpusSize {
                entries: 4,
                seats: 3
            })
        );

        let mut unqualified = record(3);
        unqualified.purpose = ParliamentBallotPurposeV1::Qualified(Digest512::new([9; 64]));
        assert_eq!(
            unqualified.validate(),
            Err(ParliamentBallotBoardRecordErrorV1::Purpose(
                ParliamentBallotPurposeErrorV1::Unqualified
            ))
        );

        let mut misordered = record(3);
        misordered.context.commitments.swap(0, 1);
        assert!(matches!(
            misordered.validate(),
            Err(ParliamentBallotBoardRecordErrorV1::Context(_))
        ));
    }
}
