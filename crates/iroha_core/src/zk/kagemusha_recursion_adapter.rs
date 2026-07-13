//! Fail-closed boundary for Kagemusha Pasta-cycle recursion.
//!
//! The reviewed Axiom `PoseidonTranscript` hashes in `C::Scalar` and explicitly
//! assumes that field is native to the verifier circuit.  A generic
//! `Halo2Loader` adapter across the Pasta cycle therefore emulates every
//! transcript scalar.  The measured Ep-to-Fp prototype required 39,275,522
//! advice cells and 7,436,318 lookup cells (about 4.1 GiB live RSS); bounded
//! CRT batching and native curve coordinates still required 18,040,862 advice
//! cells, 2,669,809 lookup cells, 100.35 seconds to construct, and
//! 2,414,559,232 bytes peak RSS.  Proof parsing consumed 8,287,023 advice cells
//! and fold-transcript parsing another 5,835,004.  That construction is
//! structurally outside the wallet's 128 MiB preparation gate and is not kept
//! as a production fallback.
//!
//! The compact wire below retains only the newest and predecessor proofs. The
//! fixed verifier derives every transcript challenge, residual coefficient,
//! and IPA accumulator from those proof bytes; none is caller-selected wire
//! data. Tests retain the smallest sound boundary supported by the pinned
//! dependencies: fixed-key Poseidon proof wires for both Pasta parities,
//! canonical BGH19 IPA folding, exact bounded proof bytes, and native terminal
//! decisions. Production availability stays false until the fixed-VK
//! cross-field leapfrog constrains those same operations without generic
//! scalar emulation and passes the complete archive and device gates.

use iroha_data_model::offline::KagemushaPastaCycleParityV1;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use ff::PrimeField;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};

/// Version of the compact leapfrog proof window.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1: u16 = 1;
/// Maximum augmented IPA proof bytes for one fixed Kagemusha step circuit.
///
/// The measured degree-12 proof is 1,536 bytes. The release contract retains
/// 256 bytes of shape headroom, but does not allow the old 4 KiB-per-proof
/// envelope to silently consume the complete peer budget.
pub const KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1: usize = 1_792;
/// Maximum canonical Norito bytes for the complete newest/predecessor window.
///
/// This is the payload embedded in `KagemushaRecursiveSpendProofV2::proof`;
/// statement, branch-conflict, and output-membership data have a separate
/// budget in the complete peer archive.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1: usize = 3_680;
/// Domain separator for identities of complete compact proof windows.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:leapfrog-proof-window:v1";
/// Number of non-zero, source-combined terms in the fixed degree-12 residual.
///
/// This count is extracted from the exact fixed verifier below. A key or
/// circuit shape that changes it requires a new authenticated release and wire
/// schema; accepting a variable residual would make packet-size and circuit
/// shape claims non-reproducible.
pub const KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1: usize = 38;
/// Domain separator for the cross-layer deferred-equation binding.
pub const KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:deferred-equation:v1";

/// One canonical non-zero coefficient in the fixed verifier's point namespace.
///
/// This is prover/circuit material and is never serialized into a peer proof
/// window. The next two circuit layers recompute and bind its digest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaDeferredEquationTermV1 {
    /// Index into transcript points followed by authenticated fixed-VK points.
    pub point_source_index: u16,
    /// Canonical scalar bytes in the proof curve's scalar field.
    pub coefficient: [u8; 32],
}

/// Complete deterministic residual selected by one fixed proof transcript.
///
/// The native-point half of layer `i + 1` consumes this equation and exposes
/// its digest. The native-scalar half of layer `i + 2` reconstructs the same
/// value from proof `i` and requires digest equality. This joins the two
/// deferred verifier halves without trusting host-provided coefficients.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaDeferredEquationBindingV1 {
    /// Parity of the proof whose residual is described.
    pub parity: KagemushaPastaCycleParityV1,
    /// SHA-256 of the exact augmented proof bytes.
    pub proof_sha256: [u8; 32],
    /// SHA-256 of the exact public-input schema.
    pub public_inputs_schema_sha256: [u8; 32],
    /// SHA-256 of the authenticated fixed verifying key.
    pub verifier_key_sha256: [u8; 32],
    /// SHA-256 of the exact canonical instance columns.
    pub instances_sha256: [u8; 32],
    /// SHA-256 of the authenticated artifact manifest.
    pub manifest_sha256: [u8; 32],
    /// Strictly source-ordered, duplicate-free residual terms.
    pub terms: Vec<KagemushaDeferredEquationTermV1>,
}

fn canonical_nonzero_scalar<F: PrimeField>(bytes: &[u8; 32]) -> bool {
    let mut repr = F::Repr::default();
    if repr.as_ref().len() != bytes.len() {
        return false;
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr)).is_some_and(|value| value != F::ZERO)
}

impl KagemushaDeferredEquationBindingV1 {
    /// Validate the exact fixed-verifier equation shape and scalar field.
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.proof_sha256,
            self.public_inputs_schema_sha256,
            self.verifier_key_sha256,
            self.instances_sha256,
            self.manifest_sha256,
        ]
        .contains(&[0; 32])
            || self.terms.len() != KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1
        {
            return Err("Kagemusha deferred equation binding shape mismatch".to_owned());
        }
        for (index, term) in self.terms.iter().enumerate() {
            if index > 0 && self.terms[index - 1].point_source_index >= term.point_source_index {
                return Err(
                    "Kagemusha deferred equation point sources are not canonical".to_owned(),
                );
            }
            let canonical = match self.parity {
                KagemushaPastaCycleParityV1::StepEq => {
                    canonical_nonzero_scalar::<Fp>(&term.coefficient)
                }
                KagemushaPastaCycleParityV1::StepEp => {
                    canonical_nonzero_scalar::<Fq>(&term.coefficient)
                }
            };
            if !canonical {
                return Err("Kagemusha deferred equation coefficient is invalid".to_owned());
            }
        }
        Ok(())
    }

    /// Return the cross-layer binding digest for this exact residual.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha deferred equation: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}

/// One fixed-circuit proof retained by the alternating Pasta leapfrog.
///
/// The proof's public instances, fixed verifier key, and authenticated release
/// determine the complete deferred MSM equation. Coefficients, point-source
/// indices, transcript challenges, and IPA accumulator limbs are therefore
/// deliberately absent: accepting caller-serialized copies would both waste
/// the peer budget and permit the circuit and terminal decider to consume
/// different equations.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaLeapfrogStepProofV1 {
    /// Curve/circuit parity of this proof.
    pub parity: KagemushaPastaCycleParityV1,
    /// Recursive transition count proved by this layer.
    pub proof_step_count: u32,
    /// Ordinary Poseidon Halo2/IPA proof plus the canonical folded generator.
    pub proof_bytes: Vec<u8>,
}

impl KagemushaLeapfrogStepProofV1 {
    /// Validate the bounded, non-empty fixed-circuit wire shape.
    pub fn validate(&self) -> Result<(), String> {
        if self.proof_step_count == 0
            || self.proof_bytes.is_empty()
            || self.proof_bytes.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
        {
            return Err("Kagemusha leapfrog step proof shape mismatch".to_owned());
        }
        Ok(())
    }
}

/// Constant-size newest/predecessor proof window transported by one bundle.
///
/// Layer `i` proves the application transition and performs the native-point
/// half of proof `i - 1` plus the native-scalar half of proof `i - 2`. The
/// halves are joined by the exact deferred-equation digest exposed by layer
/// `i - 1`. A terminal verifier fully verifies the newest two ordinary proofs;
/// induction then covers every older layer. Initialization is the only
/// single-proof window and is a circuit base case bound to finalized top-up
/// evidence.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaLeapfrogProofWindowV1 {
    /// Wire layout version.
    pub version: u16,
    /// Proof for the current public statement.
    pub newest: KagemushaLeapfrogStepProofV1,
    /// Previous proof, absent only for recursive step one.
    pub predecessor: Option<KagemushaLeapfrogStepProofV1>,
}

fn opposite_parity(parity: KagemushaPastaCycleParityV1) -> KagemushaPastaCycleParityV1 {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleParityV1::StepEp => KagemushaPastaCycleParityV1::StepEq,
    }
}

impl KagemushaLeapfrogProofWindowV1 {
    /// Validate the exact two-layer window and its canonical archive budget.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1 {
            return Err("Kagemusha leapfrog proof-window version mismatch".to_owned());
        }
        self.newest.validate()?;
        match (&self.predecessor, self.newest.proof_step_count) {
            (None, 1) => {}
            (Some(predecessor), newest_step) if newest_step > 1 => {
                predecessor.validate()?;
                if predecessor.proof_step_count.checked_add(1) != Some(newest_step)
                    || predecessor.parity != opposite_parity(self.newest.parity)
                    || predecessor.proof_bytes == self.newest.proof_bytes
                {
                    return Err("Kagemusha leapfrog predecessor binding mismatch".to_owned());
                }
            }
            _ => {
                return Err("Kagemusha leapfrog predecessor presence mismatch".to_owned());
            }
        }
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof window: {error}"))?;
        if encoded.len() > KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1 {
            return Err(format!(
                "Kagemusha leapfrog proof window is {} bytes; maximum is {}",
                encoded.len(),
                KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1
            ));
        }
        Ok(())
    }

    /// Construct the next constant-size window from one newly generated proof.
    ///
    /// Cryptographic callers must first prove that `newest` binds the old
    /// window's newest proof digest, deferred equation, result state, manifest,
    /// and application transition. This method only performs the canonical
    /// lossless window rotation after that proof has been generated.
    pub fn advance(previous: &Self, newest: KagemushaLeapfrogStepProofV1) -> Result<Self, String> {
        previous.validate()?;
        newest.validate()?;
        if newest.proof_step_count != previous.newest.proof_step_count.saturating_add(1)
            || newest.parity != opposite_parity(previous.newest.parity)
            || newest.proof_bytes == previous.newest.proof_bytes
        {
            return Err("Kagemusha leapfrog window advance mismatch".to_owned());
        }
        let window = Self {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest,
            predecessor: Some(previous.newest.clone()),
        };
        window.validate()?;
        Ok(window)
    }

    /// Return a domain-separated identity of the exact canonical window.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof window: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::to_bytes;

    use halo2_proofs::{
        arithmetic::Field,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use crate::zk::halo2_backend::assign_advice_compat;

    fn leapfrog_step(
        parity: KagemushaPastaCycleParityV1,
        proof_step_count: u32,
        byte: u8,
    ) -> KagemushaLeapfrogStepProofV1 {
        KagemushaLeapfrogStepProofV1 {
            parity,
            proof_step_count,
            proof_bytes: vec![byte; 1_536],
        }
    }

    #[test]
    fn compact_leapfrog_window_is_constant_through_step_64() {
        let mut window = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1),
            predecessor: None,
        };
        window.validate().expect("valid initialization window");
        let init_size = to_bytes(&window).expect("encode init window").len();

        let mut steady_size = None;
        for step in 2_u32..=64 {
            let parity = opposite_parity(window.newest.parity);
            window = KagemushaLeapfrogProofWindowV1::advance(
                &window,
                leapfrog_step(parity, step, u8::try_from(step).expect("bounded step")),
            )
            .expect("advance leapfrog window");
            let encoded = to_bytes(&window).expect("encode steady window");
            assert!(encoded.len() > init_size);
            assert!(encoded.len() <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1);
            assert_eq!(
                *steady_size.get_or_insert(encoded.len()),
                encoded.len(),
                "the proof window must not grow with recursive depth"
            );
            assert_eq!(
                window
                    .predecessor
                    .as_ref()
                    .expect("predecessor")
                    .proof_step_count,
                step - 1
            );
        }
    }

    #[test]
    fn compact_leapfrog_window_rejects_parity_step_and_proof_substitution() {
        let init = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1),
            predecessor: None,
        };
        let valid = KagemushaLeapfrogProofWindowV1::advance(
            &init,
            leapfrog_step(KagemushaPastaCycleParityV1::StepEp, 2, 2),
        )
        .expect("valid second layer");

        let mut wrong_version = valid.clone();
        wrong_version.version = wrong_version.version.saturating_add(1);
        assert!(wrong_version.validate().is_err());

        let mut missing_predecessor = valid.clone();
        missing_predecessor.predecessor = None;
        assert!(missing_predecessor.validate().is_err());

        let mut wrong_step = valid.clone();
        wrong_step
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_step_count = 2;
        assert!(wrong_step.validate().is_err());

        let mut wrong_parity = valid.clone();
        wrong_parity
            .predecessor
            .as_mut()
            .expect("predecessor")
            .parity = KagemushaPastaCycleParityV1::StepEp;
        assert!(wrong_parity.validate().is_err());

        let mut duplicated_proof = valid.clone();
        let newest_proof = duplicated_proof.newest.proof_bytes.clone();
        duplicated_proof
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_bytes = newest_proof;
        assert!(duplicated_proof.validate().is_err());

        let original_digest = valid.digest().expect("valid digest");
        let mut substituted = valid;
        substituted.newest.proof_bytes[0] ^= 1;
        assert_ne!(
            original_digest,
            substituted
                .digest()
                .expect("substituted window remains shaped")
        );
    }

    #[test]
    fn compact_leapfrog_window_rejects_per_step_and_total_budget_overflow() {
        let oversized = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEq,
                proof_step_count: 1,
                proof_bytes: vec![0xA5; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 + 1],
            },
            predecessor: None,
        };
        assert!(oversized.validate().is_err());

        let maximum = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEp,
                proof_step_count: 2,
                proof_bytes: vec![0xA5; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1],
            },
            predecessor: Some(KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEq,
                proof_step_count: 1,
                proof_bytes: vec![0x5A; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1],
            }),
        };
        let encoded_len = to_bytes(&maximum).expect("encode maximum window").len();
        assert!(
            encoded_len <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
            "declared per-step maxima must fit the complete window: {encoded_len}"
        );
        maximum.validate().expect("bounded maximum window");
    }

    fn deferred_equation(
        parity: KagemushaPastaCycleParityV1,
    ) -> KagemushaDeferredEquationBindingV1 {
        let terms = (0..KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1)
            .map(|index| {
                let mut coefficient = [0_u8; 32];
                match parity {
                    KagemushaPastaCycleParityV1::StepEq => {
                        let repr =
                            Fp::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                    KagemushaPastaCycleParityV1::StepEp => {
                        let repr =
                            Fq::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                }
                KagemushaDeferredEquationTermV1 {
                    point_source_index: u16::try_from(index).expect("bounded source"),
                    coefficient,
                }
            })
            .collect();
        KagemushaDeferredEquationBindingV1 {
            parity,
            proof_sha256: [1; 32],
            public_inputs_schema_sha256: [2; 32],
            verifier_key_sha256: [3; 32],
            instances_sha256: [4; 32],
            manifest_sha256: [5; 32],
            terms,
        }
    }

    #[test]
    fn deferred_equation_digest_rejects_omission_reordering_and_substitution() {
        for parity in [
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleParityV1::StepEp,
        ] {
            let binding = deferred_equation(parity);
            binding.validate().expect("canonical deferred equation");
            let digest = binding.digest().expect("deferred equation digest");

            let mut omitted = binding.clone();
            omitted.terms.pop();
            assert!(omitted.validate().is_err());

            let mut duplicate_source = binding.clone();
            duplicate_source.terms[1].point_source_index =
                duplicate_source.terms[0].point_source_index;
            assert!(duplicate_source.validate().is_err());

            let mut reordered = binding.clone();
            reordered.terms.swap(0, 1);
            assert!(reordered.validate().is_err());

            let mut noncanonical = binding.clone();
            noncanonical.terms[0].coefficient = [0xFF; 32];
            assert!(noncanonical.validate().is_err());

            let mut zero = binding.clone();
            zero.terms[0].coefficient = [0; 32];
            assert!(zero.validate().is_err());

            let mut substituted = binding;
            substituted.proof_sha256[0] ^= 1;
            assert_ne!(digest, substituted.digest().expect("bound substitution"));
        }
    }

    /// Native-value loader which preserves every MSM as a canonical linear
    /// equation instead of evaluating it away.  This is audit instrumentation
    /// for the fixed-VK deferred-verifier wire: scalar arithmetic remains the
    /// exact field arithmetic used by `snark-verifier`, while every curve
    /// assertion records the complete base/coefficient vector that the
    /// opposite-field circuit would have to authenticate.
    mod deferred_audit {
        use std::{
            cell::RefCell,
            fmt,
            io::Read,
            marker::PhantomData,
            ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
            rc::Rc,
        };

        use snark_verifier::{
            Error,
            loader::{EcPointLoader, LoadedEcPoint, LoadedScalar, Loader, ScalarLoader},
            util::{
                arithmetic::{
                    Curve, CurveAffine, Field, FieldExt, FieldOps, Group, PrimeField, fe_to_fe,
                },
                hash::Poseidon,
                transcript::{Transcript, TranscriptRead},
            },
        };

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct EquationTerm {
            pub(super) point: Vec<u8>,
            pub(super) coefficient: Vec<u8>,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct Equation {
            pub(super) annotation: String,
            pub(super) terms: Vec<EquationTerm>,
        }

        struct State {
            equations: Vec<Equation>,
        }

        #[derive(Clone)]
        pub(super) struct RecordingLoader<C: CurveAffine> {
            state: Rc<RefCell<State>>,
            _curve: PhantomData<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordingLoader<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordingLoader").finish_non_exhaustive()
            }
        }

        impl<C: CurveAffine> RecordingLoader<C> {
            pub(super) fn new() -> Self {
                Self {
                    state: Rc::new(RefCell::new(State {
                        equations: Vec::new(),
                    })),
                    _curve: PhantomData,
                }
            }

            pub(super) fn equations(&self) -> Vec<Equation> {
                self.state.borrow().equations.clone()
            }

            fn same(&self, other: &Self) {
                assert!(
                    Rc::ptr_eq(&self.state, &other.state),
                    "deferred audit values cannot cross loader instances"
                );
            }
        }

        #[derive(Clone)]
        pub(super) struct RecordedScalar<C: CurveAffine> {
            value: C::Scalar,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedScalar<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple("RecordedScalar").field(&self.value).finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedScalar<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedScalar<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_repr().as_ref().to_vec()
            }
        }

        macro_rules! scalar_binop {
            ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:tt) => {
                impl<C: CurveAffine> $trait for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $trait<&Self> for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: &Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $assign_trait for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }

                impl<C: CurveAffine> $assign_trait<&Self> for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: &Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }
            };
        }

        scalar_binop!(Add, add, AddAssign, add_assign, +);
        scalar_binop!(Sub, sub, SubAssign, sub_assign, -);
        scalar_binop!(Mul, mul, MulAssign, mul_assign, *);

        impl<C: CurveAffine> Neg for RecordedScalar<C> {
            type Output = Self;

            fn neg(mut self) -> Self::Output {
                self.value = -self.value;
                self
            }
        }

        impl<C: CurveAffine> FieldOps for RecordedScalar<C> {
            fn invert(&self) -> Option<Self> {
                Option::<C::Scalar>::from(Field::invert(&self.value)).map(|value| Self {
                    value,
                    loader: self.loader.clone(),
                })
            }
        }

        impl<C: CurveAffine> LoadedScalar<C::Scalar> for RecordedScalar<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }

            fn pow_var(&self, exp: &Self, _: usize) -> Self {
                self.loader.same(&exp.loader);
                let repr = exp.value.to_repr();
                let mut limbs = Vec::with_capacity(repr.as_ref().len().div_ceil(8));
                for chunk in repr.as_ref().chunks(8) {
                    let mut limb = [0_u8; 8];
                    limb[..chunk.len()].copy_from_slice(chunk);
                    limbs.push(u64::from_le_bytes(limb));
                }
                Self {
                    value: self.value.pow_vartime(limbs),
                    loader: self.loader.clone(),
                }
            }
        }

        #[derive(Clone)]
        struct LinearTerm<C: CurveAffine> {
            point: C,
            coefficient: C::Scalar,
        }

        #[derive(Clone)]
        pub(super) struct RecordedPoint<C: CurveAffine> {
            value: C,
            terms: Vec<LinearTerm<C>>,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedPoint<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordedPoint")
                    .field("value", &self.value)
                    .field("terms", &self.terms.len())
                    .finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedPoint<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedPoint<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_bytes().as_ref().to_vec()
            }
        }

        impl<C: CurveAffine> LoadedEcPoint<C> for RecordedPoint<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }
        }

        fn push_term<C: CurveAffine>(
            terms: &mut Vec<LinearTerm<C>>,
            point: C,
            coefficient: C::Scalar,
        ) {
            if coefficient == C::Scalar::ZERO {
                return;
            }
            if let Some(existing) = terms.iter_mut().find(|term| term.point == point) {
                existing.coefficient += coefficient;
                if existing.coefficient == C::Scalar::ZERO {
                    let index = terms
                        .iter()
                        .position(|term| term.point == point)
                        .expect("existing term index");
                    terms.remove(index);
                }
            } else {
                terms.push(LinearTerm { point, coefficient });
            }
        }

        impl<C: CurveAffine> ScalarLoader<C::Scalar> for RecordingLoader<C> {
            type LoadedScalar = RecordedScalar<C>;

            fn load_const(&self, value: &C::Scalar) -> Self::LoadedScalar {
                RecordedScalar {
                    value: *value,
                    loader: self.clone(),
                }
            }

            fn assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedScalar,
                rhs: &Self::LoadedScalar,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
            }
        }

        impl<C: CurveAffine> EcPointLoader<C> for RecordingLoader<C> {
            type LoadedEcPoint = RecordedPoint<C>;

            fn ec_point_load_const(&self, value: &C) -> Self::LoadedEcPoint {
                RecordedPoint {
                    value: *value,
                    terms: vec![LinearTerm {
                        point: *value,
                        coefficient: C::Scalar::ONE,
                    }],
                    loader: self.clone(),
                }
            }

            fn ec_point_assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedEcPoint,
                rhs: &Self::LoadedEcPoint,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
                let mut terms = Vec::new();
                for term in &lhs.terms {
                    push_term(&mut terms, term.point, term.coefficient);
                }
                for term in &rhs.terms {
                    push_term(&mut terms, term.point, -term.coefficient);
                }
                let terms = terms
                    .into_iter()
                    .map(|term| EquationTerm {
                        point: term.point.to_bytes().as_ref().to_vec(),
                        coefficient: term.coefficient.to_repr().as_ref().to_vec(),
                    })
                    .collect();
                self.state.borrow_mut().equations.push(Equation {
                    annotation: annotation.to_owned(),
                    terms,
                });
            }

            fn multi_scalar_multiplication(
                pairs: &[(
                    &<Self as ScalarLoader<C::Scalar>>::LoadedScalar,
                    &Self::LoadedEcPoint,
                )],
            ) -> Self::LoadedEcPoint {
                let (first_scalar, first_point) = pairs.first().expect("non-empty MSM");
                let loader = first_scalar.loader.clone();
                first_point.loader.same(&loader);
                let mut value = C::Curve::identity();
                let mut terms = Vec::new();
                for (scalar, point) in pairs {
                    scalar.loader.same(&loader);
                    point.loader.same(&loader);
                    value += point.value * scalar.value;
                    for term in &point.terms {
                        push_term(&mut terms, term.point, term.coefficient * scalar.value);
                    }
                }
                RecordedPoint {
                    value: value.to_affine(),
                    terms,
                    loader,
                }
            }
        }

        impl<C: CurveAffine> Loader<C> for RecordingLoader<C> {}

        pub(super) struct RecordingPoseidonTranscript<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > {
            loader: RecordingLoader<C>,
            stream: R,
            poseidon: Poseidon<C::Scalar, RecordedScalar<C>, T, RATE>,
            pub(super) scalar_count: usize,
            pub(super) point_count: usize,
            pub(super) point_sources: Vec<Vec<u8>>,
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            pub(super) fn new<const SECURE_MDS: usize>(
                loader: RecordingLoader<C>,
                stream: R,
            ) -> Self {
                let poseidon = Poseidon::new::<R_F, R_P, SECURE_MDS>(&loader);
                Self {
                    loader,
                    stream,
                    poseidon,
                    scalar_count: 0,
                    point_count: 0,
                    point_sources: Vec::new(),
                }
            }
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > Transcript<C, RecordingLoader<C>> for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn loader(&self) -> &RecordingLoader<C> {
                &self.loader
            }

            fn squeeze_challenge(&mut self) -> RecordedScalar<C> {
                self.poseidon.squeeze()
            }

            fn common_ec_point(&mut self, point: &RecordedPoint<C>) -> Result<(), Error> {
                point.loader.same(&self.loader);
                let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<C>> =
                    point.value.coordinates().into();
                let coordinates = coordinates.ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "identity point cannot enter the Poseidon transcript".to_owned(),
                    )
                })?;
                let x = self.loader.load_const(&fe_to_fe(*coordinates.x()));
                let y = self.loader.load_const(&fe_to_fe(*coordinates.y()));
                self.poseidon.update(&[x, y]);
                Ok(())
            }

            fn common_scalar(&mut self, scalar: &RecordedScalar<C>) -> Result<(), Error> {
                scalar.loader.same(&self.loader);
                self.poseidon.update(std::slice::from_ref(scalar));
                Ok(())
            }
        }

        impl<
            C: CurveAffine,
            R: Read,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > TranscriptRead<C, RecordingLoader<C>>
            for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn read_scalar(&mut self) -> Result<RecordedScalar<C>, Error> {
                let mut repr = <C::Scalar as PrimeField>::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated scalar field".to_owned())
                })?;
                let value = C::Scalar::from_repr_vartime(repr).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical scalar field".to_owned(),
                    )
                })?;
                let value = self.loader.load_const(&value);
                self.common_scalar(&value)?;
                self.scalar_count += 1;
                Ok(value)
            }

            fn read_ec_point(&mut self) -> Result<RecordedPoint<C>, Error> {
                let mut repr = C::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated curve point".to_owned())
                })?;
                let value = Option::<C>::from(C::from_bytes(&repr)).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical curve point".to_owned(),
                    )
                })?;
                self.point_sources.push(repr.as_ref().to_vec());
                let value = self.loader.ec_point_load_const(&value);
                self.common_ec_point(&value)?;
                self.point_count += 1;
                Ok(value)
            }
        }
    }

    #[derive(Clone, Default)]
    struct PublicValue<F: Field> {
        value: F,
    }

    impl<F: Field> Circuit<F> for PublicValue<F> {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let cell = layouter.assign_region(
                || "public value",
                |mut region| {
                    let cell = assign_advice_compat(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )?;
                    Ok(cell.cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    /// Fixed-key compatibility and soundness checks for the Eq proof/fold wire.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Eq, EqAffine, Fp},
            },
            plonk::{Circuit, ProvingKey, create_proof, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::ScalarLoader,
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript, TranscriptObject},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::deferred_audit::{RecordingLoader, RecordingPoseidonTranscript};
        use super::{
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1, KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1,
            KagemushaLeapfrogProofWindowV1, KagemushaLeapfrogStepProofV1,
            KagemushaPastaCycleParityV1, PublicValue,
        };
        use crate::zk::halo2_backend::{Scalar, keygen_pk, keygen_vk, params_new};
        use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EqAffine, Bgh19>;
        type FullVerifier = PlonkVerifier<As>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EqAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EqAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
            deciding_key: IpaDecidingKey<EqAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fp>>,
        }

        fn canonical_svk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
            let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn canonical_folding_key(params: &ParamsIPA<EqAffine>) -> IpaProvingKey<EqAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof<CircuitT>(
            params: &ParamsIPA<EqAffine>,
            pk: &ProvingKey<EqAffine>,
            circuit: CircuitT,
            instances: &[&[&[Scalar]]],
        ) -> Vec<u8>
        where
            CircuitT: Circuit<Scalar>,
        {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EqAffine>,
                ProverIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EqAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EqAffine>,
            proof: &[u8],
            instances: &[&[&[Scalar]]],
        ) -> EqAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EqAffine>,
                VerifierIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = params_new(INNER_K);
            let value = Scalar::from(7);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny Pasta proving key");
            let column = [value];
            let columns: [&[Scalar]; 1] = [&column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EqAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse augmented Axiom IPA proof as BGH19");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify the full PLONK residual and produce an IPA accumulator");
            assert_eq!(accumulators.len(), 1, "one proof yields one accumulator");
            accumulators.pop().expect("one accumulator")
        }

        fn create_fold_proof(
            params: &ParamsIPA<EqAffine>,
            accumulators: &[IpaAccumulator<EqAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EqAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EqAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create canonical Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn transition_proof_omits_recomputable_deferred_material_from_the_wire() {
            use crate::zk::kagemusha_v2::{
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS,
                KagemushaRecursiveSpendTransitionCircuitV2,
                kagemusha_recursive_spend_transition_instance_column_v2,
            };

            const PRODUCTION_K: u32 = 12;
            let params = params_new(PRODUCTION_K);
            let circuit = KagemushaRecursiveSpendTransitionCircuitV2::default();
            let instance_column =
                kagemusha_recursive_spend_transition_instance_column_v2(&circuit.values);
            assert_eq!(
                instance_column.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
            );
            let vk = keygen_vk(&params, &circuit).expect("transition deferred-packet VK");
            let pk =
                keygen_pk(&params, vk.clone(), &circuit).expect("transition deferred-packet PK");
            let columns: [&[Scalar]; 1] = [&instance_column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator =
                folded_generator(&params, &vk, &proof_without_generator, &proof_instances);
            let mut proof_bytes = proof_without_generator;
            proof_bytes.extend_from_slice(generator.to_bytes().as_ref());

            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(
                &params,
                &vk,
                Config::ipa().with_num_instance(vec![instance_column.len()]),
            );
            let instances = vec![instance_column];
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let parsed = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &protocol,
                &instances,
                &mut transcript,
            )
            .expect("parse fixed transition proof");
            let scalar_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::Scalar(_)))
                .count();
            let point_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::EcPoint(_)))
                .count();
            let explicit_challenge_count = parsed.challenges.len() + 1;
            let mut accumulators =
                SuccinctVerifier::verify(deciding_key.as_ref(), &protocol, &instances, &parsed)
                    .expect("verify fixed transition proof");
            assert_eq!(accumulators.len(), 1);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &deciding_key,
                accumulators.pop().expect("one transition accumulator"),
            )
            .expect("terminal transition decision");

            // Re-run the exact fixed-key verifier with native scalar
            // arithmetic and symbolic curve arithmetic. This extracts the
            // complete MSM coefficient vectors rather than guessing from the
            // number of transcript objects.
            let recording_loader = RecordingLoader::<EqAffine>::new();
            let loaded_protocol = protocol.loaded(&recording_loader);
            let loaded_instances = instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| recording_loader.load_const(value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut recording_transcript =
                RecordingPoseidonTranscript::<EqAffine, _, T, RATE, R_F, R_P>::new::<SECURE_MDS>(
                    recording_loader.clone(),
                    proof_bytes.as_slice(),
                );
            let recorded = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut recording_transcript,
            )
            .expect("parse fixed transition proof for deferred audit");
            let recorded_accumulators = SuccinctVerifier::verify(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &recorded,
            )
            .expect("extract fixed transition residual equations");
            assert_eq!(recorded_accumulators.len(), 1);
            let recorded_accumulator = &recorded_accumulators[0];
            assert_eq!(recorded_accumulator.xi.len(), PRODUCTION_K as usize);
            let equations = recording_loader.equations();
            assert_eq!(
                equations.len(),
                1,
                "the fixed IPA verifier must expose exactly one opening-residual MSM"
            );

            // Canonical point-source namespace: transcript points first in
            // transcript order, followed by fixed protocol/SVK points. The
            // packet carries only a u16 source index plus a canonical scalar;
            // proof and artifact bytes supply the points themselves.
            let mut point_sources = recording_transcript.point_sources.clone();
            let svk = deciding_key.as_ref();
            let mut add_fixed_source = |point: EqAffine| {
                let bytes = point.to_bytes().as_ref().to_vec();
                if !point_sources.iter().any(|existing| existing == &bytes) {
                    point_sources.push(bytes);
                }
            };
            for point in &protocol.preprocessed {
                add_fixed_source(*point);
            }
            add_fixed_source(svk.g);
            add_fixed_source(svk.h);
            if let Some(point) = svk.s {
                add_fixed_source(point);
            }
            add_fixed_source(EqAffine::generator());
            if let Some(instance_key) = &protocol.instance_committing_key {
                for point in &instance_key.bases {
                    add_fixed_source(*point);
                }
                if let Some(point) = instance_key.constant {
                    add_fixed_source(point);
                }
            }
            assert!(
                point_sources.len() <= usize::from(u16::MAX),
                "deferred packet point namespace must fit u16"
            );

            let mut coefficient_count = 0_usize;
            for equation in &equations {
                assert!(!equation.terms.is_empty());
                for term in &equation.terms {
                    assert_eq!(term.point.len(), 32);
                    assert_eq!(term.coefficient.len(), 32);
                    assert!(
                        point_sources.iter().any(|source| source == &term.point),
                        "every residual base must resolve to proof or fixed-VK material"
                    );
                }
                coefficient_count += equation.terms.len();
            }
            let accumulator_u = recorded_accumulator.u.canonical_bytes();
            assert!(
                point_sources.iter().any(|source| source == &accumulator_u),
                "the output accumulator point must be a proof point"
            );
            for xi in &recorded_accumulator.xi {
                assert_eq!(xi.canonical_bytes().len(), 32);
            }

            // Coefficients and accumulator limbs are verifier-derived material,
            // not peer wire fields. Both the fixed leapfrog circuit and the
            // native terminal verifier reconstruct them from these proof bytes,
            // the authenticated fixed VK/protocol, and the exact instances.
            // This removes a redundant 1,858 bytes per proof and, more
            // importantly, prevents a serialized-equation substitution from
            // selecting a different MSM than the proof transcript selects.
            const EQUATION_HEADER_BYTES: usize = 2;
            const EQUATION_TERM_BYTES: usize = 2 + 32;
            let recomputed_material_bytes = equations.len() * EQUATION_HEADER_BYTES
                + coefficient_count * EQUATION_TERM_BYTES
                + recorded_accumulator.xi.len() * 32
                + 2;
            eprintln!(
                "Kagemusha compact proof={} scalars={} points={} explicit_challenges={} preprocessed={} residual_equations={} residual_coefficients={} point_sources={} derived_not_transported={}",
                proof_bytes.len(),
                scalar_count,
                point_count,
                explicit_challenge_count,
                protocol.preprocessed.len(),
                equations.len(),
                coefficient_count,
                point_sources.len(),
                recomputed_material_bytes,
            );
            assert!(
                proof_bytes.len() <= KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1,
                "the measured fixed step proof must fit its exact wire slot"
            );

            let predecessor_bytes = proof_bytes.clone();
            let mut newest_bytes = proof_bytes;
            newest_bytes[0] ^= 1;
            let window = KagemushaLeapfrogProofWindowV1 {
                version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
                newest: KagemushaLeapfrogStepProofV1 {
                    parity: KagemushaPastaCycleParityV1::StepEp,
                    proof_step_count: 2,
                    proof_bytes: newest_bytes,
                },
                predecessor: Some(KagemushaLeapfrogStepProofV1 {
                    parity: KagemushaPastaCycleParityV1::StepEq,
                    proof_step_count: 1,
                    proof_bytes: predecessor_bytes,
                }),
            };
            window.validate().expect("bounded two-proof window");
            assert!(
                norito::to_bytes(&window)
                    .expect("encode compact proof window")
                    .len()
                    <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
                "the newest/predecessor proof window must fit its reserved archive budget"
            );
        }

        #[test]
        fn canonical_ipa_fold_is_constant_size_decidable_and_substitution_safe() {
            let fixture = fixture();
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (proof_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            let expected_wire_bytes = (8 + 2 * INNER_K as usize) * 32;
            assert_eq!(
                proof_bytes.len(),
                expected_wire_bytes,
                "the canonical Poseidon IPA fold wire must not gain metadata or a host receipt"
            );
            assert!(
                proof_bytes.len() <= 4_096,
                "canonical IPA fold proof must fit the recursive proof budget"
            );

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse canonical IPA fold proof");
            let folded =
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify canonical IPA fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide folded IPA accumulator");

            let mut substituted_inputs = inputs;
            substituted_inputs[0].u = fixture.params.get_g()[1];
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
                let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                    &svk,
                    &substituted_inputs,
                    &mut transcript,
                )
                .expect("a canonical substituted point remains parseable");
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(
                    &svk,
                    &substituted_inputs,
                    &proof,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "an input-accumulator substitution must invalidate the fold"
            );
        }

        #[test]
        fn axiom_poseidon_wire_appends_exactly_one_folded_generator() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EqAffine as GroupEncoding>::Repr>(),
                "the recursion wire is the ordinary Axiom proof plus one compressed point"
            );

            let accumulator = succinct_accumulator(&fixture);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                accumulator.clone(),
            )
            .expect("terminal decision recomputes the folded canonical generator basis");

            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = FullVerifier::read_proof(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("full verifier parses augmented proof");
            FullVerifier::verify(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("full verifier includes terminal IPA decision");

            let substituted =
                IpaAccumulator::new(accumulator.xi.clone(), fixture.params.get_g()[1]);
            assert!(
                <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                    &fixture.deciding_key,
                    substituted,
                )
                .is_err(),
                "carrying a substituted accumulator point is not a terminal decision"
            );
        }

        #[test]
        fn folded_generator_is_constrained_by_the_plonk_opening_residual() {
            let fixture = fixture();
            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());

            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a substituted folded generator must fail the constrained residual"
            );
        }
    }

    /// Reciprocal Pasta parity.  The production cycle is sound only if an
    /// Ep/Pallas proof over Fq is authenticated inside an Fp circuit with the
    /// same transcript, VK, public-instance, and fold bindings as Eq/Vesta.
    mod pasta_ipa_poseidon_wire_ep {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Ep, EpAffine, Fq},
            },
            plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{SnarkVerifier, plonk::PlonkSuccinctVerifier},
        };

        use super::PublicValue;

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EpAffine, Bgh19>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EpAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EpAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
            deciding_key: IpaDecidingKey<EpAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fq>>,
        }

        fn canonical_svk(params: &ParamsIPA<EpAffine>) -> IpaSuccinctVerifyingKey<EpAffine> {
            let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn canonical_folding_key(params: &ParamsIPA<EpAffine>) -> IpaProvingKey<EpAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof(
            params: &ParamsIPA<EpAffine>,
            pk: &ProvingKey<EpAffine>,
            circuit: PublicValue<Fq>,
            instances: &[&[&[Fq]]],
        ) -> Vec<u8> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EpAffine>,
                ProverIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create reciprocal Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EpAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EpAffine>,
            proof: &[u8],
            instances: &[&[&[Fq]]],
        ) -> EpAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EpAffine>,
                VerifierIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete reciprocal native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = ParamsIPA::<EpAffine>::new(INNER_K);
            let value = Fq::from(11);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny reciprocal Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit)
                .expect("tiny reciprocal Pasta proving key");
            let column = [value];
            let columns: [&[Fq]; 1] = [&column];
            let proof_instances: [&[&[Fq]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EpAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse reciprocal augmented IPA proof");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify reciprocal PLONK residual");
            assert_eq!(accumulators.len(), 1);
            accumulators.pop().expect("one reciprocal accumulator")
        }

        fn create_fold_proof(
            params: &ParamsIPA<EpAffine>,
            accumulators: &[IpaAccumulator<EpAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EpAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EpAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create reciprocal Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn reciprocal_poseidon_wire_fold_and_tamper_contract() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EpAffine as GroupEncoding>::Repr>()
            );
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (fold_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            assert_eq!(fold_bytes.len(), (8 + 2 * INNER_K as usize) * 32);

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(fold_bytes.as_slice());
            let proof = <As as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse reciprocal fold proof");
            let folded =
                <As as AccumulationScheme<EpAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify reciprocal fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EpAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide reciprocal folded accumulator");

            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a reciprocal substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a reciprocal folded-generator substitution must reject"
            );
        }
    }
}
