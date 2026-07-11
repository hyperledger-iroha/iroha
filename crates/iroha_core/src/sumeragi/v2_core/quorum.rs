use std::{collections::BTreeSet, error::Error, fmt};

use super::{HeightContext, ValidatorId, VotingPower};

/// Count and voting-power totals represented by a signer set.
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

    /// Returns the total voting power represented by the set.
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
        let mut power = 0_u128;
        for signer in signers {
            if previous.is_some_and(|value| value >= *signer) {
                return Err(QuorumError::SignersNotStrictlyOrdered);
            }
            previous = Some(*signer);
            let Some(validator) = context.validator(signer) else {
                return Err(QuorumError::UnknownValidator(*signer));
            };
            power = power
                .checked_add(u128::from(validator.power().get()))
                .ok_or(QuorumError::VotingPowerOverflow)?;
        }
        let voting_power = u64::try_from(power)
            .map(VotingPower::new)
            .map_err(|_| QuorumError::VotingPowerOverflow)?;
        Ok(Self {
            signer_count: signers.len(),
            voting_power,
        })
    }

    /// Returns whether both the distinct-validator and voting-power thresholds
    /// are satisfied.
    #[must_use]
    pub fn satisfies(self, context: &HeightContext) -> bool {
        self.signer_count >= context.minimum_signer_count()
            && u128::from(self.voting_power.get()) * 3
                > u128::from(context.total_voting_power().get()) * 2
    }

    /// Validates a canonical signer set and requires it to meet both quorum
    /// thresholds.
    ///
    /// # Errors
    ///
    /// Returns an error when the signer set is malformed or does not satisfy
    /// both thresholds.
    pub fn require(context: &HeightContext, signers: &[ValidatorId]) -> Result<Self, QuorumError> {
        let quorum = Self::calculate(context, signers)?;
        if quorum.satisfies(context) {
            Ok(quorum)
        } else {
            Err(QuorumError::Insufficient {
                signer_count: quorum.signer_count,
                voting_power: quorum.voting_power,
                required_signer_count: context.minimum_signer_count(),
                total_voting_power: context.total_voting_power(),
            })
        }
    }

    /// Calculates a quorum over an iterator while rejecting duplicate signers.
    pub(crate) fn from_iter(
        context: &HeightContext,
        signers: impl IntoIterator<Item = ValidatorId>,
    ) -> Result<(Self, Vec<ValidatorId>), QuorumError> {
        let ordered: BTreeSet<_> = signers.into_iter().collect();
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
    /// Two timeout groups contain the same signer.
    OverlappingTimeoutSigner(ValidatorId),
    /// Timeout groups are not ordered by their stable high-QC reference.
    TimeoutGroupsNotStrictlyOrdered,
    /// Two highest `PrepareQC`s at the same view certify different subjects.
    ConflictingHighestPrepare,
    /// A timeout certificate reports a `PrepareQC` from a later view.
    HighestPrepareFromFuture,
    /// A signer does not belong to the voting roster.
    UnknownValidator(ValidatorId),
    /// Signers contain a duplicate or are not in canonical ascending order.
    SignersNotStrictlyOrdered,
    /// Voting powers could not be summed without overflow.
    VotingPowerOverflow,
    /// The signer set fails either the count or power threshold.
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
            Self::OverlappingTimeoutSigner(validator) => {
                write!(
                    formatter,
                    "timeout signer {validator} appears in multiple groups"
                )
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
            Self::SignersNotStrictlyOrdered => {
                formatter.write_str("signers are not strictly ordered")
            }
            Self::VotingPowerOverflow => formatter.write_str("voting power overflow"),
            Self::Insufficient {
                signer_count,
                voting_power,
                required_signer_count,
                total_voting_power,
            } => write!(
                formatter,
                "insufficient quorum: {signer_count}/{required_signer_count} signers and {}/{}, power",
                voting_power.get(),
                total_voting_power.get()
            ),
        }
    }
}

impl Error for QuorumError {}
