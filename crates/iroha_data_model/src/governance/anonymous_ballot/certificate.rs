//! The certificate binding of a binding body's ballot (design §3.8 and §12.3).
//!
//! An approved binding body produces one immutable result binding: its closure record and
//! the 64-byte root that commits that record. The binding replaces every timed-OVN and TLE
//! field of the baseline certificate. Counts are public, so the binding carries the whole
//! record: a reader of the certificate sees the seats, the accepted ballots and the three
//! counts without another lookup.
//!
//! The binding is built only for an approval. A rejected or inquorate body rejects its
//! governance attempt and never reaches a certificate.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    AnonymousBallotCodecErrorV1, BallotClosureErrorV1, BallotClosureRecordV1, BallotOutcomeV1,
    Digest512, FrozenBallotPolicyV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, governance::types::ParliamentBody};

/// Why a ballot binding is not a valid certificate binding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnonymousBallotBindingErrorV1 {
    /// The closure record is inconsistent with itself or with its frozen policy.
    #[error(transparent)]
    Closure(#[from] BallotClosureErrorV1),
    /// The body did not approve: only an approval is certified.
    #[error("a certificate binds an approved ballot, not {outcome:?}")]
    NotApproved {
        /// The closure's outcome.
        outcome: BallotOutcomeV1,
    },
    /// The stored result root is not the root of the closure record.
    #[error("the result root is not the root of the closure record")]
    ResultRoot,
    /// The result root could not be computed.
    #[error(transparent)]
    Codec(#[from] AnonymousBallotCodecErrorV1),
}

/// The immutable result binding of an approved binding body.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousBallotCertificateBindingV1"
)]
pub struct AnonymousBallotCertificateBindingV1 {
    /// The closure record of the body's one ballot.
    pub closure: BallotClosureRecordV1,
    /// `result_root = H(Frame("PARL/RESULT/v1"; Enc(closure)))`.
    pub result_root: Digest512,
}

impl AnonymousBallotCertificateBindingV1 {
    /// Bind the approved `closure` of a body whose frozen ballot policy is `policy`.
    ///
    /// # Errors
    /// [`AnonymousBallotBindingErrorV1`] for a record that fails its policy, or one that
    /// did not approve.
    pub fn approve(
        closure: BallotClosureRecordV1,
        policy: &FrozenBallotPolicyV1,
    ) -> Result<Self, AnonymousBallotBindingErrorV1> {
        let binding = Self {
            closure,
            result_root: closure.result_root()?,
        };
        binding.validate(policy)?;
        Ok(binding)
    }

    /// Check the record against its frozen policy, the approval and the result root.
    ///
    /// The record's `policy_hash`, `R` and scope are `P16`-bound digests: the node checks
    /// them against the sealed board through `iroha_parliament_ballot`, which rebuilds the
    /// record it stored.
    ///
    /// # Errors
    /// The first violated rule as an [`AnonymousBallotBindingErrorV1`].
    pub fn validate(
        &self,
        policy: &FrozenBallotPolicyV1,
    ) -> Result<(), AnonymousBallotBindingErrorV1> {
        self.closure.validate(policy)?;
        if self.closure.outcome != BallotOutcomeV1::Approved {
            return Err(AnonymousBallotBindingErrorV1::NotApproved {
                outcome: self.closure.outcome,
            });
        }
        if self.closure.result_root()? != self.result_root {
            return Err(AnonymousBallotBindingErrorV1::ResultRoot);
        }
        Ok(())
    }

    /// Whether this approval needs a disjoint Confirmation Jury before certification: an
    /// approved narrow Policy Jury result. A Confirmation Jury result never does.
    #[must_use]
    pub fn requires_confirmation(&self) -> bool {
        self.closure.body_role == ParliamentBody::PolicyJury && self.closure.narrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::anonymous_ballot::test_fixtures::{closure, policy};

    #[test]
    fn an_approval_binds_its_closure_record_and_its_root() {
        let record = closure(5, 4, 1, 0);
        let binding = AnonymousBallotCertificateBindingV1::approve(record, &policy())
            .expect("approved closure");
        assert_eq!(binding.closure, record);
        assert_eq!(binding.result_root, record.result_root().expect("root"));
        assert_eq!(binding.validate(&policy()), Ok(()));
        assert!(!binding.requires_confirmation());

        let bytes = norito::encode_canonical(&binding).expect("encode");
        assert_eq!(
            norito::decode_canonical::<AnonymousBallotCertificateBindingV1>(&bytes)
                .expect("decode"),
            binding
        );
        let json = norito::json::to_json(&binding).expect("json");
        assert_eq!(
            norito::json::from_str::<AnonymousBallotCertificateBindingV1>(&json)
                .expect("from json"),
            binding
        );
    }

    #[test]
    fn only_an_approval_is_certified() {
        // Quorate and rejected, a tie, and below turnout.
        for (seats, aye, nay, abstain, outcome) in [
            (5, 1, 4, 0, BallotOutcomeV1::Rejected),
            (6, 2, 2, 0, BallotOutcomeV1::Rejected),
            (5, 2, 0, 0, BallotOutcomeV1::NoQuorum),
        ] {
            assert_eq!(
                AnonymousBallotCertificateBindingV1::approve(
                    closure(seats, aye, nay, abstain),
                    &policy()
                ),
                Err(AnonymousBallotBindingErrorV1::NotApproved { outcome }),
                "{aye}/{nay}/{abstain} of {seats}",
            );
        }
    }

    #[test]
    fn a_binding_commits_every_field_of_its_record() {
        let binding = AnonymousBallotCertificateBindingV1::approve(closure(5, 4, 1, 0), &policy())
            .expect("approved closure");

        // Counts that still approve but are not the committed ones.
        let mut other_counts = binding;
        other_counts.closure = closure(5, 5, 0, 0);
        assert_eq!(
            other_counts.validate(&policy()),
            Err(AnonymousBallotBindingErrorV1::ResultRoot)
        );

        let mut other_root = binding;
        other_root.result_root = Digest512::new([0x77; 64]);
        assert_eq!(
            other_root.validate(&policy()),
            Err(AnonymousBallotBindingErrorV1::ResultRoot)
        );

        // An outcome the counts do not give is refused before the root is compared.
        let mut flipped = binding;
        flipped.closure.aye = 1;
        flipped.closure.nay = 4;
        assert!(matches!(
            flipped.validate(&policy()),
            Err(AnonymousBallotBindingErrorV1::Closure(
                BallotClosureErrorV1::Outcome { .. }
            ))
        ));
    }

    #[test]
    fn a_narrow_policy_approval_requires_confirmation() {
        // 51 to 49 of 150 seats: quorate, approved, and a margin under five percent.
        let narrow =
            AnonymousBallotCertificateBindingV1::approve(closure(150, 51, 49, 0), &policy())
                .expect("narrow approval");
        assert!(narrow.closure.narrow);
        assert!(narrow.requires_confirmation());

        // The same record of a Confirmation Jury is final.
        let mut confirmation = narrow;
        confirmation.closure.body_role = ParliamentBody::ConfirmationJury;
        assert!(!confirmation.requires_confirmation());
    }
}
