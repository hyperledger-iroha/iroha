//! Fail-closed security evidence for the experimental Jindo profile.
//!
//! Functional proof verification is not a theorem-backed knowledge-soundness certificate. The
//! current complete uniform `S_35` challenge distribution contains distinct challenges whose
//! difference is a zero divisor. A future certificate therefore needs a distribution-wide numerical
//! bound, its exact extractor/composition loss, and pinned machine-checkable evidence.
use super::{
    JINDO_RING_DEGREE_V1,
    ring::{JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoRnsPolynomialV1},
};
use thiserror::Error;
const JINDO_CHALLENGE_WEIGHT_V1: usize = 35;
/// Requirements for issuing the first theorem-backed Jindo security certificate.
///
/// This descriptor is informational and is deliberately not an activation or transcript input. The
/// current experimental profile remains unchanged and the sealed capability remains unavailable.
pub const JINDO_SECURITY_CERTIFICATE_REQUIREMENTS_V1: &[u8] = b"iroha-jindo-security-certificate-v1|status=blocked|challenge=complete-uniform-S35|required=distribution-wide-numerical-bound-on-Pr[distinct-challenge-difference-is-nonunit]-for-all-compiled-inner-and-outer-ring-factors;knowledge-extractor-and-alpha-c-composition-loss;fiat-shamir-qrom-loss;machine-checkable-artifact-and-pinned-source-digest";
/// Sealed evidence that the compiled Jindo profile has met its complete
/// theorem-backed knowledge-soundness requirements.
///
/// There is intentionally no public constructor. Call
/// [`jindo_security_certificate_v1`] so missing evidence fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JindoSecurityCertificateV1 {
    _sealed: (),
}
/// Reason the compiled Jindo profile cannot issue a security certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoSecurityCertificateErrorV1 {
    /// No pinned theorem and checker bound the complete challenge
    /// distribution's non-unit differences and all resulting extractor losses.
    #[error(
        "Jindo security certificate requires pinned distribution-wide difference-invertibility and knowledge-extraction evidence"
    )]
    MissingDistributionWideKnowledgeSoundnessEvidence,
}
/// Request the theorem-backed security certificate for the compiled Jindo profile.
///
/// The current complete `S_35` profile has no pinned distribution-wide theorem
/// or machine-checkable bound. This function therefore always fails closed;
/// successful native proof verification does not bypass this capability.
///
/// # Errors
///
/// Returns [`JindoSecurityCertificateErrorV1::MissingDistributionWideKnowledgeSoundnessEvidence`]
/// until all requirements in [`JINDO_SECURITY_CERTIFICATE_REQUIREMENTS_V1`] are pinned and checked.
pub const fn jindo_security_certificate_v1()
-> Result<JindoSecurityCertificateV1, JindoSecurityCertificateErrorV1> {
    // This release gate intentionally stays closed until a pinned,
    // machine-checked distribution theorem and its concrete extractor,
    // composition, and Fiat--Shamir losses meet the release target.
    Err(JindoSecurityCertificateErrorV1::MissingDistributionWideKnowledgeSoundnessEvidence)
}
/// Structural failure while checking a pair of current `S_35` challenges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoChallengePairErrorV1 {
    /// The left challenge does not contain exactly 1024 coefficients.
    #[error("left Jindo challenge has {count} coefficients; expected 1024")]
    LeftLength {
        /// Observed coefficient count.
        count: usize,
    },
    /// The right challenge does not contain exactly 1024 coefficients.
    #[error("right Jindo challenge has {count} coefficients; expected 1024")]
    RightLength {
        /// Observed coefficient count.
        count: usize,
    },
    /// One left coefficient is outside the canonical signed ternary alphabet.
    #[error("left Jindo challenge coefficient {index} has invalid value {value}")]
    LeftCoefficient {
        /// Zero-based coefficient index.
        index: usize,
        /// Observed coefficient.
        value: i8,
    },
    /// One right coefficient is outside the canonical signed ternary alphabet.
    #[error("right Jindo challenge coefficient {index} has invalid value {value}")]
    RightCoefficient {
        /// Zero-based coefficient index.
        index: usize,
        /// Observed coefficient.
        value: i8,
    },
    /// The left challenge does not have exact Hamming weight 35.
    #[error("left Jindo challenge has weight {weight}; expected 35")]
    LeftWeight {
        /// Observed non-zero coefficient count.
        weight: usize,
    },
    /// The right challenge does not have exact Hamming weight 35.
    #[error("right Jindo challenge has weight {weight}; expected 35")]
    RightWeight {
        /// Observed non-zero coefficient count.
        weight: usize,
    },
    /// Difference invertibility is defined here only for distinct challenges.
    #[error("Jindo challenge pair is identical")]
    Identical,
}
/// Check whether two canonical, distinct current `S_35` challenges have a unit
/// difference in every compiled inner and outer ring factor.
///
/// This is an exact deterministic diagnostic, not a distribution-wide security certificate.
/// Checking any finite collection of pairs cannot bound the probability of a non-unit difference
/// over the complete `S_35` distribution.
///
/// # Errors
///
/// Returns a structural error unless both inputs are exact signed-ternary,
/// weight-35 vectors of ring degree 1024 and are distinct.
pub fn jindo_challenge_pair_has_unit_difference_v1(
    left: &[i8],
    right: &[i8],
) -> Result<bool, JindoChallengePairErrorV1> {
    validate_challenge_v1(left, true)?;
    validate_challenge_v1(right, false)?;
    if left == right {
        return Err(JindoChallengePairErrorV1::Identical);
    }
    let difference: [i128; JINDO_RING_DEGREE_V1] =
        core::array::from_fn(|index| i128::from(left[index]) - i128::from(right[index]));
    let inner = JindoRnsPolynomialV1::from_balanced_coefficients(difference, JINDO_INNER_MODULI_V1);
    if !inner.is_unit(JINDO_INNER_MODULI_V1) {
        return Ok(false);
    }
    let outer = JindoRnsPolynomialV1::from_balanced_coefficients(difference, JINDO_OUTER_MODULI_V1);
    Ok(outer.is_unit(JINDO_OUTER_MODULI_V1))
}
fn validate_challenge_v1(challenge: &[i8], left: bool) -> Result<(), JindoChallengePairErrorV1> {
    if challenge.len() != JINDO_RING_DEGREE_V1 {
        return Err(if left {
            JindoChallengePairErrorV1::LeftLength {
                count: challenge.len(),
            }
        } else {
            JindoChallengePairErrorV1::RightLength {
                count: challenge.len(),
            }
        });
    }
    let mut weight = 0_usize;
    for (index, value) in challenge.iter().copied().enumerate() {
        match value {
            -1 | 1 => weight += 1,
            0 => {}
            value => {
                return Err(if left {
                    JindoChallengePairErrorV1::LeftCoefficient { index, value }
                } else {
                    JindoChallengePairErrorV1::RightCoefficient { index, value }
                });
            }
        }
    }
    if weight != JINDO_CHALLENGE_WEIGHT_V1 {
        return Err(if left {
            JindoChallengePairErrorV1::LeftWeight { weight }
        } else {
            JindoChallengePairErrorV1::RightWeight { weight }
        });
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    const OUTER_Q0: u64 = 48_591_984_641;
    const OUTER_Q0_PSI: u64 = 25_236_428_417;
    fn challenge(shared: core::ops::Range<usize>, tail: [usize; 3]) -> [i8; 1024] {
        let mut coefficients = [0_i8; 1024];
        for index in shared.chain(tail) {
            coefficients[index] = 1;
        }
        coefficients
    }
    fn evaluate_at_outer_q0_root(coefficients: &[i8; 1024]) -> u64 {
        let mut value = 0_u64;
        let mut power = 1_u64;
        for coefficient in coefficients {
            match coefficient {
                1 => value = (value + power) % OUTER_Q0,
                -1 => value = (value + OUTER_Q0 - power) % OUTER_Q0,
                0 => {}
                _ => unreachable!("test challenges are signed ternary"),
            }
            power = (u128::from(power) * u128::from(OUTER_Q0_PSI) % u128::from(OUTER_Q0)) as u64;
        }
        value
    }
    #[test]
    fn security_certificate_remains_fail_closed_without_distribution_theorem() {
        assert_eq!(
            jindo_security_certificate_v1(),
            Err(JindoSecurityCertificateErrorV1::MissingDistributionWideKnowledgeSoundnessEvidence)
        );
    }
    #[test]
    fn complete_s35_contains_a_pinned_nonunit_difference() {
        let left = challenge(0..32, [71, 74, 784]);
        let right = challenge(0..32, [539, 940, 942]);
        assert_eq!(left.iter().filter(|value| **value != 0).count(), 35);
        assert_eq!(right.iter().filter(|value| **value != 0).count(), 35);
        assert_ne!(left, right);
        // The two three-term tails both evaluate to 45_746_542_050. Adding
        // their common 32-term prefix gives the same full-challenge value at
        // the pinned primitive 2048th root modulo outer q0. Their non-zero
        // difference therefore has a zero NTT coordinate and is a zero
        // divisor.
        assert_eq!(evaluate_at_outer_q0_root(&left), 22_157_043_724);
        assert_eq!(evaluate_at_outer_q0_root(&right), 22_157_043_724);
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&left, &right),
            Ok(false)
        );
    }
    #[test]
    fn pair_checker_accepts_both_signed_monomial_difference_classes() {
        let mut support_swap_left = [0_i8; 1024];
        let mut support_swap_right = [0_i8; 1024];
        for index in 0..34 {
            support_swap_left[index] = 1;
            support_swap_right[index] = 1;
        }
        support_swap_left[100] = 1;
        support_swap_right[900] = 1;
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&support_swap_left, &support_swap_right),
            Ok(true)
        );
        let sign_flip_left = challenge(0..32, [71, 74, 784]);
        let mut sign_flip_right = sign_flip_left;
        sign_flip_right[0] = -1;
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&sign_flip_left, &sign_flip_right),
            Ok(true)
        );
    }
    #[test]
    fn pair_checker_rejects_noncanonical_and_identical_inputs() {
        let canonical = challenge(0..32, [71, 74, 784]);
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&canonical[..1023], &canonical),
            Err(JindoChallengePairErrorV1::LeftLength { count: 1023 })
        );
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&canonical, &canonical[..1023]),
            Err(JindoChallengePairErrorV1::RightLength { count: 1023 })
        );
        let mut invalid_coefficient = canonical;
        invalid_coefficient[1000] = 2;
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&invalid_coefficient, &canonical),
            Err(JindoChallengePairErrorV1::LeftCoefficient {
                index: 1000,
                value: 2,
            })
        );
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&canonical, &invalid_coefficient),
            Err(JindoChallengePairErrorV1::RightCoefficient {
                index: 1000,
                value: 2,
            })
        );
        let mut invalid_weight = canonical;
        invalid_weight[0] = 0;
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&invalid_weight, &canonical),
            Err(JindoChallengePairErrorV1::LeftWeight { weight: 34 })
        );
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&canonical, &invalid_weight),
            Err(JindoChallengePairErrorV1::RightWeight { weight: 34 })
        );
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(&canonical, &canonical),
            Err(JindoChallengePairErrorV1::Identical)
        );
    }
}
