//! Fail-closed security evidence for the Jindo signed-monomial profile.
//!
//! The compiled rings admit a complete, machine-checked unit-difference
//! certificate for `{+X^i, -X^i | 0 <= i < 1024}`. That algebraic result does
//! not by itself prove that Fiat--Shamir parallel repetition amplifies
//! knowledge soundness in the qROM, so the production certificate remains
//! unavailable until that separate theorem and its concrete loss are pinned.
use super::{
    JINDO_RING_DEGREE_V1,
    parameters::JINDO_PARALLEL_REPETITIONS_V1,
    ring::{
        JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoPrimeModulusV1, is_prime_modulus_v1,
    },
    transcript::{JINDO_SIGNED_MONOMIAL_CHALLENGE_CARDINALITY_V1, JindoSignedMonomialChallengeV1},
};
use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use std::sync::OnceLock;
use thiserror::Error;

const ROOT_ORDER_V1: u16 = 2048;
const NEGACYCLIC_ROOT_COUNT_V1: u16 = 1024;
const _: () = {
    assert!(ROOT_ORDER_V1 == JINDO_SIGNED_MONOMIAL_CHALLENGE_CARDINALITY_V1);
    assert!(NEGACYCLIC_ROOT_COUNT_V1 as usize == JINDO_RING_DEGREE_V1);
};
const UNIT_DIFFERENCE_CERTIFICATE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.jindo.signed-monomial-unit-difference-certificate.v1";

/// Pinned requirements that still block a production Jindo security claim.
///
/// The 352 terminal-column challenge bits and 32 interactive repetitions are
/// descriptive, not an asserted non-interactive or qROM security level.
pub const JINDO_SECURITY_CERTIFICATE_REQUIREMENTS_V1: &[u8] = b"iroha-jindo-security-certificate-v1|unit-difference=complete-machine-checked-for-2048-signed-monomials-in-all-four-compiled-prime-ring-factors|parallel-repetitions=32|raw-terminal-column-challenge-bits=352|production-status=blocked|required=pinned-collapsing-proof-and-qrom-knowledge-extractor-for-the-exact-IJP3-transcript;concrete-random-oracle-query-bound;alpha-and-c-composition-loss;final-security-bits-at-least-128|warning=interactive-parallel-repetition-bound-must-not-be-reused-for-fiat-shamir-or-qrom";
/// Pinned digest emitted by the complete unit-difference checker.
pub const JINDO_UNIT_DIFFERENCE_CERTIFICATE_DIGEST_V1: [u8; 32] = [
    0x13, 0x1b, 0x08, 0x13, 0xef, 0xc7, 0x75, 0x3d, 0xd9, 0x33, 0xbd, 0xbd, 0x4a, 0xef, 0x89, 0x3f,
    0x02, 0x1e, 0xd2, 0x89, 0x99, 0x93, 0x05, 0x2f, 0x47, 0x81, 0x6f, 0x3b, 0x96, 0xf5, 0x32, 0x6c,
];

/// Digest-bearing evidence that all distinct signed-monomial differences are
/// units in every compiled Jindo RNS factor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JindoUnitDifferenceCertificateV1 {
    digest: [u8; 32],
    checked_root_count: u16,
    checked_difference_classes: u16,
    checked_challenge_pairs: u64,
}

impl JindoUnitDifferenceCertificateV1 {
    /// Return the digest of the checked moduli, roots, set, and proof method.
    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Return the number of negacyclic roots checked across all RNS factors.
    #[must_use]
    pub const fn checked_root_count(self) -> u16 {
        self.checked_root_count
    }

    /// Return the number of non-zero exponent-difference classes checked at
    /// every root.
    #[must_use]
    pub const fn checked_difference_classes(self) -> u16 {
        self.checked_difference_classes
    }

    /// Return the number of distinct unordered signed-monomial pairs covered
    /// by those difference classes.
    #[must_use]
    pub const fn checked_challenge_pairs(self) -> u64 {
        self.checked_challenge_pairs
    }
}

/// Failure while checking the compiled signed-monomial unit theorem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoUnitDifferenceCertificateErrorV1 {
    /// A compiled modulus is not prime, so root non-vanishing would not prove
    /// invertibility in every CRT coordinate.
    #[error("Jindo compiled ring modulus {modulus} is not prime")]
    ModulusNotPrime {
        /// Composite compiled modulus.
        modulus: u64,
    },
    /// A compiled modulus does not contain the required 2048th roots.
    #[error("Jindo modulus {modulus} is not congruent to one modulo 2048")]
    ModulusNotNttFriendly {
        /// Invalid compiled modulus.
        modulus: u64,
    },
    /// A pinned root does not have exact order 2048.
    #[error("Jindo pinned root {root} does not have exact order 2048 modulo {modulus}")]
    InvalidPrimitiveRoot {
        /// Compiled modulus.
        modulus: u64,
        /// Invalid pinned root.
        root: u64,
    },
    /// A non-zero signed-exponent difference vanished at a negacyclic root.
    #[error(
        "Jindo exponent difference {difference} vanished at negacyclic root {root_index} modulo {modulus}"
    )]
    DifferenceVanished {
        /// Compiled modulus.
        modulus: u64,
        /// Index of the negacyclic root.
        root_index: u16,
        /// Non-zero exponent difference modulo 2048.
        difference: u16,
    },
    /// The checked artifact no longer matches the profile-pinned digest.
    #[error("Jindo unit-difference certificate digest does not match the compiled profile")]
    CertificateDigestMismatch,
}

/// Run the complete signed-monomial unit-difference checker.
///
/// Every challenge is represented as `X^e` for `e` in `Z / 2048 Z`. For two
/// distinct challenges, their difference is a monomial unit times
/// `X^(e0-e1)-1`. The checker enumerates all 2047 non-zero difference classes
/// at all 1024 negacyclic roots for each of the four compiled prime factors.
/// Non-vanishing at every CRT coordinate is equivalent to being a unit.
///
/// # Errors
///
/// Returns a typed error if a compiled modulus/root is invalid or any
/// non-zero difference class vanishes.
pub fn jindo_unit_difference_certificate_v1()
-> Result<JindoUnitDifferenceCertificateV1, JindoUnitDifferenceCertificateErrorV1> {
    static CERTIFICATE: OnceLock<
        Result<JindoUnitDifferenceCertificateV1, JindoUnitDifferenceCertificateErrorV1>,
    > = OnceLock::new();
    *CERTIFICATE.get_or_init(build_unit_difference_certificate_v1)
}

fn build_unit_difference_certificate_v1()
-> Result<JindoUnitDifferenceCertificateV1, JindoUnitDifferenceCertificateErrorV1> {
    let all_moduli = [
        JINDO_INNER_MODULI_V1[0],
        JINDO_INNER_MODULI_V1[1],
        JINDO_OUTER_MODULI_V1[0],
        JINDO_OUTER_MODULI_V1[1],
    ];
    for prime in all_moduli {
        check_prime_difference_classes_v1(prime)?;
    }

    let mut hash = Shake256::default();
    absorb_certificate_field_v1(&mut hash, UNIT_DIFFERENCE_CERTIFICATE_DOMAIN_V1);
    absorb_certificate_field_v1(
        &mut hash,
        &JINDO_SIGNED_MONOMIAL_CHALLENGE_CARDINALITY_V1.to_be_bytes(),
    );
    absorb_certificate_field_v1(&mut hash, &ROOT_ORDER_V1.to_be_bytes());
    absorb_certificate_field_v1(&mut hash, &NEGACYCLIC_ROOT_COUNT_V1.to_be_bytes());
    for prime in all_moduli {
        absorb_certificate_field_v1(&mut hash, &prime.modulus().to_be_bytes());
        absorb_certificate_field_v1(&mut hash, &prime.psi().to_be_bytes());
    }
    absorb_certificate_field_v1(
        &mut hash,
        b"deterministic-u64-miller-rabin-bases-2,325,9375,28178,450775,9780504,1795265022;exhaustive-root-evaluation:for-each-prime,odd-root-index,and-delta-1-through-2047,root^delta!=1",
    );
    let mut digest = [0_u8; 32];
    hash.finalize_xof().read(&mut digest);
    if digest != JINDO_UNIT_DIFFERENCE_CERTIFICATE_DIGEST_V1 {
        return Err(JindoUnitDifferenceCertificateErrorV1::CertificateDigestMismatch);
    }
    let challenge_count = u64::from(JINDO_SIGNED_MONOMIAL_CHALLENGE_CARDINALITY_V1);
    Ok(JindoUnitDifferenceCertificateV1 {
        digest,
        checked_root_count: 4 * NEGACYCLIC_ROOT_COUNT_V1,
        checked_difference_classes: ROOT_ORDER_V1 - 1,
        checked_challenge_pairs: challenge_count * (challenge_count - 1) / 2,
    })
}

fn check_prime_difference_classes_v1(
    prime: JindoPrimeModulusV1,
) -> Result<(), JindoUnitDifferenceCertificateErrorV1> {
    let modulus = prime.modulus();
    let psi = prime.psi();
    if !is_prime_modulus_v1(modulus) {
        return Err(JindoUnitDifferenceCertificateErrorV1::ModulusNotPrime { modulus });
    }
    if (modulus - 1) % u64::from(ROOT_ORDER_V1) != 0 {
        return Err(JindoUnitDifferenceCertificateErrorV1::ModulusNotNttFriendly { modulus });
    }
    if pow_mod_v1(psi, u64::from(ROOT_ORDER_V1), modulus) != 1
        || pow_mod_v1(psi, u64::from(NEGACYCLIC_ROOT_COUNT_V1), modulus) != modulus - 1
    {
        return Err(
            JindoUnitDifferenceCertificateErrorV1::InvalidPrimitiveRoot { modulus, root: psi },
        );
    }
    for root_index in 0..NEGACYCLIC_ROOT_COUNT_V1 {
        let exponent = u64::from(2 * root_index + 1);
        let root = pow_mod_v1(psi, exponent, modulus);
        let mut power = root;
        for difference in 1..ROOT_ORDER_V1 {
            if power == 1 {
                return Err(JindoUnitDifferenceCertificateErrorV1::DifferenceVanished {
                    modulus,
                    root_index,
                    difference,
                });
            }
            power = mul_mod_v1(power, root, modulus);
        }
        debug_assert_eq!(power, 1);
    }
    Ok(())
}

fn absorb_certificate_field_v1(hash: &mut Shake256, value: &[u8]) {
    hash.update(&(value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn mul_mod_v1(left: u64, right: u64, modulus: u64) -> u64 {
    (u128::from(left) * u128::from(right) % u128::from(modulus)) as u64
}

fn pow_mod_v1(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mul_mod_v1(result, base, modulus);
        }
        base = mul_mod_v1(base, base, modulus);
        exponent >>= 1;
    }
    result
}

/// Sealed evidence that the complete Jindo construction, including its exact
/// qROM Fiat--Shamir transform, reaches the release security target.
///
/// There is intentionally no public constructor. Unit-difference evidence is
/// necessary but insufficient; callers cannot promote it into this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JindoSecurityCertificateV1 {
    _sealed: (),
}

/// Reason the compiled Jindo profile cannot issue a production certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoSecurityCertificateErrorV1 {
    /// The compiled unit-difference checker failed.
    #[error("Jindo signed-monomial unit-difference certificate failed: {0}")]
    UnitDifference(JindoUnitDifferenceCertificateErrorV1),
    /// No pinned theorem gives a concrete qROM extractor loss for the exact
    /// 32-repetition Fiat--Shamir transcript.
    #[error(
        "Jindo has {repetitions} signed-monomial repetitions ({terminal_challenge_bits} raw terminal-column challenge bits), but no pinned qROM parallel Fiat--Shamir extractor-loss theorem certifies {required_security_bits} bits"
    )]
    MissingQromParallelFiatShamirExtractorLoss {
        /// Number of transcript repetitions.
        repetitions: u8,
        /// Sum of terminal-column challenge entropy before reductions.
        terminal_challenge_bits: u16,
        /// First-release minimum qROM security target.
        required_security_bits: u16,
    },
}

impl From<JindoUnitDifferenceCertificateErrorV1> for JindoSecurityCertificateErrorV1 {
    fn from(value: JindoUnitDifferenceCertificateErrorV1) -> Self {
        Self::UnitDifference(value)
    }
}

/// Request the complete theorem-backed security certificate.
///
/// This first checks the exact algebraic unit certificate, then fails closed
/// at the independently required qROM extraction theorem. In particular, it
/// never treats 352 raw challenge bits as 352 qROM security bits.
///
/// # Errors
///
/// Returns the typed unit-check failure or the missing-qROM-theorem failure.
pub fn jindo_security_certificate_v1()
-> Result<JindoSecurityCertificateV1, JindoSecurityCertificateErrorV1> {
    let _unit_certificate = jindo_unit_difference_certificate_v1()?;
    Err(
        JindoSecurityCertificateErrorV1::MissingQromParallelFiatShamirExtractorLoss {
            repetitions: u8::try_from(JINDO_PARALLEL_REPETITIONS_V1)
                .expect("fixed repetition count fits u8"),
            terminal_challenge_bits: u16::try_from(JINDO_PARALLEL_REPETITIONS_V1 * 11)
                .expect("fixed challenge bit count fits u16"),
            required_security_bits: 128,
        },
    )
}

/// Pairwise diagnostic failure for signed-monomial challenges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoChallengePairErrorV1 {
    /// Difference invertibility is defined only for distinct challenges.
    #[error("Jindo signed-monomial challenge pair is identical")]
    Identical,
}

/// Check a distinct signed-monomial pair against all compiled ring factors.
///
/// The production unit certificate checks the entire set more efficiently by
/// enumerating exponent-difference classes. This helper is an independent
/// per-pair diagnostic using the same root theorem.
///
/// # Errors
///
/// Returns [`JindoChallengePairErrorV1::Identical`] for an identical pair.
pub fn jindo_challenge_pair_has_unit_difference_v1(
    left: JindoSignedMonomialChallengeV1,
    right: JindoSignedMonomialChallengeV1,
) -> Result<bool, JindoChallengePairErrorV1> {
    if left == right {
        return Err(JindoChallengePairErrorV1::Identical);
    }
    let difference = (u32::from(left.canonical_exponent()) + u32::from(ROOT_ORDER_V1)
        - u32::from(right.canonical_exponent()))
        % u32::from(ROOT_ORDER_V1);
    debug_assert_ne!(difference, 0);
    Ok([
        JINDO_INNER_MODULI_V1[0],
        JINDO_INNER_MODULI_V1[1],
        JINDO_OUTER_MODULI_V1[0],
        JINDO_OUTER_MODULI_V1[1],
    ]
    .into_iter()
    .all(|prime| {
        (0..JINDO_RING_DEGREE_V1).all(|root_index| {
            let root = pow_mod_v1(prime.psi(), (2 * root_index + 1) as u64, prime.modulus());
            pow_mod_v1(root, u64::from(difference), prime.modulus()) != 1
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge(exponent: u16) -> JindoSignedMonomialChallengeV1 {
        JindoSignedMonomialChallengeV1::from_canonical_exponent(exponent).unwrap()
    }

    #[test]
    fn complete_unit_difference_certificate_covers_every_pair_and_factor() {
        let certificate = jindo_unit_difference_certificate_v1().unwrap();
        assert_eq!(certificate.checked_root_count(), 4 * 1024);
        assert_eq!(certificate.checked_difference_classes(), 2047);
        assert_eq!(certificate.checked_challenge_pairs(), 2_096_128);
        assert_eq!(
            certificate.digest(),
            JINDO_UNIT_DIFFERENCE_CERTIFICATE_DIGEST_V1
        );
    }

    #[test]
    fn pair_checker_accepts_support_swaps_and_sign_flips() {
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(challenge(100), challenge(900)),
            Ok(true)
        );
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(challenge(17), challenge(1041)),
            Ok(true)
        );
    }

    #[test]
    fn pair_checker_rejects_identical_challenges() {
        assert_eq!(
            jindo_challenge_pair_has_unit_difference_v1(challenge(784), challenge(784)),
            Err(JindoChallengePairErrorV1::Identical)
        );
        assert!(JindoSignedMonomialChallengeV1::from_canonical_exponent(2048).is_none());
    }

    #[test]
    fn unit_certificate_cannot_promote_itself_to_a_security_certificate() {
        let unit = jindo_unit_difference_certificate_v1().unwrap();
        assert_ne!(unit.digest(), [0; 32]);
        assert_eq!(
            jindo_security_certificate_v1(),
            Err(
                JindoSecurityCertificateErrorV1::MissingQromParallelFiatShamirExtractorLoss {
                    repetitions: 32,
                    terminal_challenge_bits: 352,
                    required_security_bits: 128,
                }
            )
        );
    }

    #[test]
    fn requirements_forbid_interactive_or_raw_entropy_promotion() {
        let requirements =
            core::str::from_utf8(JINDO_SECURITY_CERTIFICATE_REQUIREMENTS_V1).unwrap();
        assert!(requirements.contains("production-status=blocked"));
        assert!(requirements.contains("pinned-collapsing-proof-and-qrom-knowledge-extractor"));
        assert!(requirements.contains("must-not-be-reused-for-fiat-shamir-or-qrom"));
        assert!(!requirements.contains("production-qualified"));
    }
}
