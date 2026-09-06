use super::{HeightContext, ValidatorId, VotingPower};
use std::{collections::BTreeSet, error::Error, fmt};
/// Equal-vote count and its redundant unit-vote projection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Quorum {
    signer_count: usize,
    voting_power: VotingPower,
}
impl Quorum {
    /// Returns the number of distinct voting validators in the set.
    #[must_use]
    pub const fn signer_count(self) -> usize {
        self.signer_count
    }
    /// Returns the redundant unit-vote projection represented by the set.
    #[must_use]
    pub const fn voting_power(self) -> VotingPower {
        self.voting_power
    }
    /// Calculates quorum totals for a set of signers.
    ///
    /// The input must be strictly ordered. Requiring canonical signer order
    /// makes certificate construction and verification deterministic.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown, duplicate, unordered, or overflowing
    /// signer sets.
    pub fn calculate(
        context: &HeightContext,
        signers: &[ValidatorId],
    ) -> Result<Self, QuorumError> {
        let mut previous = None;
        for signer in signers {
            if previous.is_some_and(|value| value >= *signer) {
                return Err(QuorumError::SignersNotStrictlyOrdered);
            }
            previous = Some(*signer);
            let Some(validator) = context.validator(signer) else {
                return Err(QuorumError::UnknownValidator(*signer));
            };
            if validator.power().get() != 1 {
                return Err(QuorumError::VotingPowerNotOne(*signer));
            }
        }
        let voting_power = u64::try_from(signers.len())
            .map(VotingPower::new)
            .map_err(|_| QuorumError::VotingPowerOverflow)?;
        Ok(Self {
            signer_count: signers.len(),
            voting_power,
        })
    }
    /// Returns whether the `2f + 1` distinct-validator threshold is satisfied.
    #[must_use]
    pub fn satisfies(self, context: &HeightContext) -> bool {
        self.signer_count >= context.minimum_signer_count()
    }
    /// Validates a canonical certificate signer set and requires exactly `2f + 1` members.
    ///
    /// # Errors
    ///
    /// Returns an error when the signer set is malformed or does not have the
    /// canonical certificate cardinality.
    pub fn require(context: &HeightContext, signers: &[ValidatorId]) -> Result<Self, QuorumError> {
        let quorum = Self::calculate(context, signers)?;
        let required_signer_count = context.minimum_signer_count();
        if quorum.signer_count == required_signer_count {
            Ok(quorum)
        } else {
            Err(QuorumError::SignerCountMismatch {
                signer_count: quorum.signer_count,
                required_signer_count,
            })
        }
    }
    /// Calculates a quorum over an iterator while rejecting duplicate signers.
    pub(crate) fn from_iter(
        context: &HeightContext,
        signers: impl IntoIterator<Item = ValidatorId>,
    ) -> Result<(Self, Vec<ValidatorId>), QuorumError> {
        let mut ordered = BTreeSet::new();
        for signer in signers {
            if !ordered.insert(signer) {
                return Err(QuorumError::DuplicateSigner(signer));
            }
        }
        let ordered: Vec<_> = ordered.into_iter().collect();
        let quorum = Self::calculate(context, &ordered)?;
        Ok((quorum, ordered))
    }
}
/// Failure while validating a signer set against a frozen height context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuorumError {
    /// A certificate belongs to a different frozen height context.
    ContextMismatch,
    /// A certificate or vote belongs to a different height.
    HeightMismatch,
    /// A nested certificate has an unexpected phase.
    InvalidPhase,
    /// A vote or certificate is split across distinct proposal and vote rounds.
    InvalidProposalRound,
    /// Two timeout groups contain the same signer.
    OverlappingTimeoutSigner(ValidatorId),
    /// A timeout group does not contain any signer.
    EmptyTimeoutGroup,
    /// Timeout groups are not ordered by their stable high-QC reference.
    TimeoutGroupsNotStrictlyOrdered,
    /// Two highest `PrepareQC`s at the same view certify different subjects.
    ConflictingHighestPrepare,
    /// A timeout certificate reports a `PrepareQC` from a later view.
    HighestPrepareFromFuture,
    /// A signer does not belong to the voting roster.
    UnknownValidator(ValidatorId),
    /// An iterator supplied the same signer more than once.
    DuplicateSigner(ValidatorId),
    /// Signers contain a duplicate or are not in canonical ascending order.
    SignersNotStrictlyOrdered,
    /// The redundant unit-vote projection could not be represented.
    VotingPowerOverflow,
    /// A context validator does not carry the required single consensus vote.
    VotingPowerNotOne(ValidatorId),
    /// A wire certificate does not carry exactly the canonical signer count.
    SignerCountMismatch {
        /// Distinct validators represented by the certificate.
        signer_count: usize,
        /// Exact distinct-validator count required by the height context.
        required_signer_count: usize,
    },
    /// The signer set fails the `2f + 1` distinct-validator threshold.
    Insufficient {
        /// Distinct validators represented by the set.
        signer_count: usize,
        /// Voting power represented by the set.
        voting_power: VotingPower,
        /// Minimum distinct validator count.
        required_signer_count: usize,
        /// Total voting power in the height context.
        total_voting_power: VotingPower,
    },
}
impl fmt::Display for QuorumError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextMismatch => formatter.write_str("certificate context mismatch"),
            Self::HeightMismatch => formatter.write_str("certificate height mismatch"),
            Self::InvalidPhase => formatter.write_str("certificate has an invalid phase"),
            Self::InvalidProposalRound => {
                formatter.write_str("certificate has split proposal and vote rounds")
            }
            Self::OverlappingTimeoutSigner(validator) => {
                write!(
                    formatter,
                    "timeout signer {validator} appears in multiple groups"
                )
            }
            Self::EmptyTimeoutGroup => {
                formatter.write_str("timeout group does not contain any signer")
            }
            Self::TimeoutGroupsNotStrictlyOrdered => {
                formatter.write_str("timeout groups are not strictly ordered")
            }
            Self::ConflictingHighestPrepare => {
                formatter.write_str("conflicting highest PrepareQCs at the same view")
            }
            Self::HighestPrepareFromFuture => {
                formatter.write_str("timeout certificate reports a future PrepareQC")
            }
            Self::UnknownValidator(validator) => {
                write!(formatter, "unknown validator {validator}")
            }
            Self::DuplicateSigner(validator) => {
                write!(formatter, "duplicate signer {validator}")
            }
            Self::SignersNotStrictlyOrdered => {
                formatter.write_str("signers are not strictly ordered")
            }
            Self::VotingPowerOverflow => formatter.write_str("voting power overflow"),
            Self::VotingPowerNotOne(validator) => {
                write!(
                    formatter,
                    "validator {validator} does not have exactly one vote"
                )
            }
            Self::SignerCountMismatch {
                signer_count,
                required_signer_count,
            } => write!(
                formatter,
                "certificate signer count mismatch: expected exactly {required_signer_count}, got {signer_count}"
            ),
            Self::Insufficient {
                signer_count,
                voting_power,
                required_signer_count,
                total_voting_power,
            } => write!(
                formatter,
                "insufficient equal-vote quorum: {signer_count}/{required_signer_count} signers and redundant unit-vote projection {}/{}",
                voting_power.get(),
                total_voting_power.get()
            ),
        }
    }
}
impl Error for QuorumError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::v2_core::{ContextId, Digest, NetworkId, Validator, VotingMode};

    fn validator_id(marker: u8) -> ValidatorId {
        ValidatorId::repeat(marker)
    }

    fn context() -> HeightContext {
        HeightContext::new(
            ContextId::repeat(0x10),
            NetworkId::repeat(0x11),
            1,
            None,
            0,
            (1..=4)
                .map(|marker| Validator::new(validator_id(marker), VotingPower::new(1)))
                .collect(),
            VotingMode::Permissioned,
            Digest::repeat(0x12),
            Digest::repeat(0x13),
            Digest::repeat(0x14),
            Digest::repeat(0x15),
        )
        .expect("valid quorum test context")
    }

    #[test]
    fn from_iter_rejects_duplicate_signers() {
        let context = context();
        let duplicate = validator_id(2);

        assert_eq!(
            Quorum::from_iter(
                &context,
                [validator_id(3), duplicate, validator_id(1), duplicate],
            ),
            Err(QuorumError::DuplicateSigner(duplicate))
        );
    }

    #[test]
    fn from_iter_canonicalizes_unordered_distinct_signers() {
        let context = context();
        let (quorum, ordered) = Quorum::from_iter(
            &context,
            [validator_id(3), validator_id(1), validator_id(2)],
        )
        .expect("distinct known signers form a canonical quorum projection");

        assert_eq!(
            ordered,
            vec![validator_id(1), validator_id(2), validator_id(3)]
        );
        assert_eq!(quorum.signer_count(), 3);
        assert_eq!(quorum.voting_power(), VotingPower::new(3));
    }
}
