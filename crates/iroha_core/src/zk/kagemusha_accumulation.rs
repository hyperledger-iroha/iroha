//! Constant-depth IPA accumulation for Kagemusha Pasta-cycle steps.
//!
//! A recursive step cannot merely carry the IPA opening claim emitted while
//! verifying its parent: doing so would leave one undecided claim per hop.
//! This module defines the canonical native wire used to fold the current
//! Halo2 opening claim with the single claim exposed by the parent step.  The
//! result is decided against the authenticated `ParamsIPA` generator vector at
//! every terminal verification path.  The in-circuit verifier consumes the
//! same wire through the split scalar/point verifier; this native implementation
//! is the reference oracle and the terminal soundness boundary.

use ff::PrimeField;
use halo2_proofs::{
    halo2curves::{
        CurveExt as _,
        group::{Curve as _, GroupEncoding},
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    poly::commitment::Params as _,
};
use norito::codec::{Decode, Encode};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
        ipa::{
            Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey, IpaSuccinctVerifyingKey,
        },
    },
    system::halo2::transcript::halo2::PoseidonTranscript,
    util::arithmetic::{Domain, root_of_unity},
};

/// Version of the canonical accumulated-opening wire.
pub const KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1: u16 = 1;
/// Number of IPA round challenges for the authenticated degree-12 release.
pub const KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1: usize =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1 as usize;
/// Exact size of a non-ZK BGH19 accumulation proof at degree 12.
pub const KAGEMUSHA_IPA_ACCUMULATION_PROOF_BYTES_V1: usize =
    (8 + 2 * KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1) * 32;

const POSEIDON_WIDTH: usize = 3;
const POSEIDON_RATE: usize = 2;
const POSEIDON_FULL_ROUNDS: usize = 8;
const POSEIDON_PARTIAL_ROUNDS: usize = 57;
const POSEIDON_SECURE_MDS: usize = 0;

type EqAccumulation = IpaAs<EqAffine, Bgh19>;
type EpAccumulation = IpaAs<EpAffine, Bgh19>;
type EqTranscript<S> = PoseidonTranscript<
    EqAffine,
    NativeLoader,
    S,
    POSEIDON_WIDTH,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;
type EpTranscript<S> = PoseidonTranscript<
    EpAffine,
    NativeLoader,
    S,
    POSEIDON_WIDTH,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;

/// Field-neutral encoding of one IPA accumulator.
///
/// Scalar and point encodings remain canonical curve encodings.  No field
/// reduction is permitted while crossing from the Eq half to the Ep half (or
/// vice versa).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaIpaAccumulatorWireV1 {
    /// Wire layout version.
    pub version: u16,
    /// Ordered canonical IPA round challenges.
    pub round_challenges: Vec<[u8; 32]>,
    /// Canonical compressed accumulated generator.
    pub folded_generator: [u8; 32],
}

impl KagemushaIpaAccumulatorWireV1 {
    /// Encode an Eq/Vesta accumulator without reducing its Fp challenges.
    #[must_use]
    pub fn from_eq(accumulator: &IpaAccumulator<EqAffine, NativeLoader>) -> Self {
        Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            round_challenges: accumulator
                .xi
                .iter()
                .map(|scalar| {
                    let mut bytes = [0_u8; 32];
                    bytes.copy_from_slice(scalar.to_repr().as_ref());
                    bytes
                })
                .collect(),
            folded_generator: {
                let mut bytes = [0_u8; 32];
                bytes.copy_from_slice(accumulator.u.to_bytes().as_ref());
                bytes
            },
        }
    }

    /// Encode an Ep/Pallas accumulator without reducing its Fq challenges.
    #[must_use]
    pub fn from_ep(accumulator: &IpaAccumulator<EpAffine, NativeLoader>) -> Self {
        Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            round_challenges: accumulator
                .xi
                .iter()
                .map(|scalar| {
                    let mut bytes = [0_u8; 32];
                    bytes.copy_from_slice(scalar.to_repr().as_ref());
                    bytes
                })
                .collect(),
            folded_generator: {
                let mut bytes = [0_u8; 32];
                bytes.copy_from_slice(accumulator.u.to_bytes().as_ref());
                bytes
            },
        }
    }

    /// Parse this wire as an Eq/Vesta accumulator.
    pub fn to_eq(&self) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
        self.validate_shape()?;
        let xi = self
            .round_challenges
            .iter()
            .map(|bytes| {
                Option::<Fp>::from(Fp::from_repr((*bytes).into()))
                    .ok_or_else(|| "Kagemusha Eq accumulator scalar is non-canonical".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let u = Option::<EqAffine>::from(EqAffine::from_bytes(&self.folded_generator.into()))
            .ok_or_else(|| "Kagemusha Eq accumulator point is non-canonical".to_owned())?;
        Ok(IpaAccumulator::new(xi, u))
    }

    /// Parse this wire as an Ep/Pallas accumulator.
    pub fn to_ep(&self) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
        self.validate_shape()?;
        let xi = self
            .round_challenges
            .iter()
            .map(|bytes| {
                Option::<Fq>::from(Fq::from_repr((*bytes).into()))
                    .ok_or_else(|| "Kagemusha Ep accumulator scalar is non-canonical".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let u = Option::<EpAffine>::from(EpAffine::from_bytes(&self.folded_generator.into()))
            .ok_or_else(|| "Kagemusha Ep accumulator point is non-canonical".to_owned())?;
        Ok(IpaAccumulator::new(xi, u))
    }

    fn validate_shape(&self) -> Result<(), String> {
        if self.version != KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1
            || self.round_challenges.len() != KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1
        {
            return Err("Kagemusha IPA accumulator wire shape mismatch".to_owned());
        }
        Ok(())
    }
}

/// Opaque fold proof appended to one ordinary augmented Halo2 proof.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaIpaAccumulationProofV1 {
    /// Wire layout version.
    pub version: u16,
    /// Empty for an initialization step; otherwise the exact BGH19 fold proof.
    pub bytes: Vec<u8>,
}

impl KagemushaIpaAccumulationProofV1 {
    /// Construct the initialization marker, where the current opening is the
    /// only outstanding accumulator.
    #[must_use]
    pub fn initialization() -> Self {
        Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            bytes: Vec::new(),
        }
    }

    /// Validate whether this wire matches the presence of a parent claim.
    pub fn validate(&self, has_parent: bool) -> Result<(), String> {
        if self.version != KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1
            || (has_parent && self.bytes.len() != KAGEMUSHA_IPA_ACCUMULATION_PROOF_BYTES_V1)
            || (!has_parent && !self.bytes.is_empty())
        {
            return Err("Kagemusha IPA accumulation proof shape mismatch".to_owned());
        }
        Ok(())
    }
}

fn eq_keys(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
) -> (IpaProvingKey<EqAffine>, IpaDecidingKey<EqAffine>) {
    use halo2_proofs::poly::commitment::ParamsProver as _;

    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let h = hash_to_curve(&[2]).to_affine();
    let s = Some(hash_to_curve(&[1]).to_affine());
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        h,
        s,
    );
    (
        IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), h, s),
        IpaDecidingKey::new(svk, params.get_g().to_vec()),
    )
}

fn ep_keys(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
) -> (IpaProvingKey<EpAffine>, IpaDecidingKey<EpAffine>) {
    use halo2_proofs::poly::commitment::ParamsProver as _;

    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let h = hash_to_curve(&[2]).to_affine();
    let s = Some(hash_to_curve(&[1]).to_affine());
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        h,
        s,
    );
    (
        IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), h, s),
        IpaDecidingKey::new(svk, params.get_g().to_vec()),
    )
}

/// Fold the current Eq opening with the parent Eq accumulator.
pub fn fold_eq_accumulators(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    current: IpaAccumulator<EqAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EqAffine, NativeLoader>>,
) -> Result<
    (
        KagemushaIpaAccumulationProofV1,
        IpaAccumulator<EqAffine, NativeLoader>,
    ),
    String,
> {
    let Some(parent) = parent else {
        return Ok((KagemushaIpaAccumulationProofV1::initialization(), current));
    };
    let (proving_key, _) = eq_keys(params);
    let inputs = [current, parent];
    let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let accumulated = <EqAccumulation as AccumulationSchemeProver<EqAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rand_core_06::OsRng,
    )
    .map_err(|error| format!("failed to create Kagemusha Eq accumulation proof: {error:?}"))?;
    let proof = KagemushaIpaAccumulationProofV1 {
        version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
        bytes: transcript.finalize(),
    };
    proof.validate(true)?;
    Ok((proof, accumulated))
}

/// Fold the current Ep opening with the parent Ep accumulator.
pub fn fold_ep_accumulators(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    current: IpaAccumulator<EpAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EpAffine, NativeLoader>>,
) -> Result<
    (
        KagemushaIpaAccumulationProofV1,
        IpaAccumulator<EpAffine, NativeLoader>,
    ),
    String,
> {
    let Some(parent) = parent else {
        return Ok((KagemushaIpaAccumulationProofV1::initialization(), current));
    };
    let (proving_key, _) = ep_keys(params);
    let inputs = [current, parent];
    let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let accumulated = <EpAccumulation as AccumulationSchemeProver<EpAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rand_core_06::OsRng,
    )
    .map_err(|error| format!("failed to create Kagemusha Ep accumulation proof: {error:?}"))?;
    let proof = KagemushaIpaAccumulationProofV1 {
        version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
        bytes: transcript.finalize(),
    };
    proof.validate(true)?;
    Ok((proof, accumulated))
}

/// Verify an Eq fold and terminally decide its single resulting claim.
pub fn verify_and_decide_eq_accumulation(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    current: IpaAccumulator<EqAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EqAffine, NativeLoader>>,
    proof: &KagemushaIpaAccumulationProofV1,
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
    proof.validate(parent.is_some())?;
    let (_, deciding_key) = eq_keys(params);
    let accumulated = if let Some(parent) = parent {
        let inputs = [current, parent];
        let cursor = std::io::Cursor::new(proof.bytes.clone());
        let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
        let parsed = <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
        .map_err(|error| format!("failed to parse Kagemusha Eq accumulation proof: {error:?}"))?;
        let accumulated = <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
        .map_err(|error| format!("failed to verify Kagemusha Eq accumulation proof: {error:?}"))?;
        let cursor = transcript.finalize();
        if cursor.position() != proof.bytes.len() as u64 {
            return Err("Kagemusha Eq accumulation proof has trailing bytes".to_owned());
        }
        accumulated
    } else {
        current
    };
    <EqAccumulation as AccumulationDecider<EqAffine, NativeLoader>>::decide(
        &deciding_key,
        accumulated.clone(),
    )
    .map_err(|error| format!("Kagemusha Eq accumulated opening decision failed: {error:?}"))?;
    Ok(accumulated)
}

/// Verify an Ep fold and terminally decide its single resulting claim.
pub fn verify_and_decide_ep_accumulation(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    current: IpaAccumulator<EpAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EpAffine, NativeLoader>>,
    proof: &KagemushaIpaAccumulationProofV1,
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
    proof.validate(parent.is_some())?;
    let (_, deciding_key) = ep_keys(params);
    let accumulated = if let Some(parent) = parent {
        let inputs = [current, parent];
        let cursor = std::io::Cursor::new(proof.bytes.clone());
        let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
        let parsed = <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
        .map_err(|error| format!("failed to parse Kagemusha Ep accumulation proof: {error:?}"))?;
        let accumulated = <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
        .map_err(|error| format!("failed to verify Kagemusha Ep accumulation proof: {error:?}"))?;
        let cursor = transcript.finalize();
        if cursor.position() != proof.bytes.len() as u64 {
            return Err("Kagemusha Ep accumulation proof has trailing bytes".to_owned());
        }
        accumulated
    } else {
        current
    };
    <EpAccumulation as AccumulationDecider<EpAffine, NativeLoader>>::decide(
        &deciding_key,
        accumulated.clone(),
    )
    .map_err(|error| format!("Kagemusha Ep accumulated opening decision failed: {error:?}"))?;
    Ok(accumulated)
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        halo2curves::{CurveAffine as _, group::Curve as _},
        poly::ipa::commitment::ParamsIPA,
    };
    use snark_verifier::pcs::ipa::h_coeffs;

    use super::*;

    fn eq_accumulator(
        params: &ParamsIPA<EqAffine>,
        seed: u64,
    ) -> IpaAccumulator<EqAffine, NativeLoader> {
        use halo2_proofs::poly::commitment::ParamsProver as _;
        let xi = (0..KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1)
            .map(|round| Fp::from(seed + round as u64 + 1))
            .collect::<Vec<_>>();
        let coefficients = h_coeffs(&xi, Fp::ONE);
        let u = params
            .get_g()
            .iter()
            .zip(coefficients)
            .fold(Eq::identity(), |sum, (base, coefficient)| {
                sum + *base * coefficient
            })
            .to_affine();
        IpaAccumulator::new(xi, u)
    }

    fn ep_accumulator(
        params: &ParamsIPA<EpAffine>,
        seed: u64,
    ) -> IpaAccumulator<EpAffine, NativeLoader> {
        use halo2_proofs::poly::commitment::ParamsProver as _;
        let xi = (0..KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1)
            .map(|round| Fq::from(seed + round as u64 + 1))
            .collect::<Vec<_>>();
        let coefficients = h_coeffs(&xi, Fq::ONE);
        let u = params
            .get_g()
            .iter()
            .zip(coefficients)
            .fold(Ep::identity(), |sum, (base, coefficient)| {
                sum + *base * coefficient
            })
            .to_affine();
        IpaAccumulator::new(xi, u)
    }

    #[test]
    fn accumulator_wire_is_canonical_for_both_pasta_parities() {
        let eq_params = ParamsIPA::<EqAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let ep_params = ParamsIPA::<EpAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let eq = eq_accumulator(&eq_params, 7);
        let ep = ep_accumulator(&ep_params, 11);
        assert_eq!(
            KagemushaIpaAccumulatorWireV1::from_eq(&eq)
                .to_eq()
                .unwrap()
                .xi,
            eq.xi
        );
        assert_eq!(
            KagemushaIpaAccumulatorWireV1::from_ep(&ep)
                .to_ep()
                .unwrap()
                .xi,
            ep.xi
        );

        let mut noncanonical = KagemushaIpaAccumulatorWireV1::from_eq(&eq);
        noncanonical.round_challenges[0] = [0xFF; 32];
        assert!(noncanonical.to_eq().is_err());
    }

    #[test]
    fn eq_and_ep_accumulation_fold_and_reject_substitution() {
        let eq_params = ParamsIPA::<EqAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let eq_current = eq_accumulator(&eq_params, 3);
        let eq_parent = eq_accumulator(&eq_params, 19);
        let (eq_proof, eq_expected) =
            fold_eq_accumulators(&eq_params, eq_current.clone(), Some(eq_parent.clone())).unwrap();
        let eq_actual = verify_and_decide_eq_accumulation(
            &eq_params,
            eq_current.clone(),
            Some(eq_parent),
            &eq_proof,
        )
        .unwrap();
        assert_eq!(eq_actual.xi, eq_expected.xi);
        assert_eq!(eq_actual.u, eq_expected.u);
        let mut tampered = eq_proof;
        tampered.bytes[0] ^= 1;
        assert!(
            verify_and_decide_eq_accumulation(
                &eq_params,
                eq_current,
                Some(eq_accumulator(&eq_params, 19)),
                &tampered,
            )
            .is_err()
        );

        let ep_params = ParamsIPA::<EpAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let ep_current = ep_accumulator(&ep_params, 5);
        let ep_parent = ep_accumulator(&ep_params, 23);
        let (ep_proof, ep_expected) =
            fold_ep_accumulators(&ep_params, ep_current.clone(), Some(ep_parent.clone())).unwrap();
        let ep_actual =
            verify_and_decide_ep_accumulation(&ep_params, ep_current, Some(ep_parent), &ep_proof)
                .unwrap();
        assert_eq!(ep_actual.xi, ep_expected.xi);
        assert_eq!(ep_actual.u, ep_expected.u);
    }
}
