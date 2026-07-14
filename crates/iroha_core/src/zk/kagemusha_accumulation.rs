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
/// Fixed number of field-neutral `u32` limbs used to expose one accumulator.
///
/// The first two limbs are the wire version and round count. They are followed
/// by every canonical 32-byte challenge and the canonical compressed point.
pub const KAGEMUSHA_IPA_ACCUMULATOR_INSTANCE_LIMBS_V1: usize =
    2 + (KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1 + 1) * 8;
/// Exact size of a non-ZK BGH19 accumulation proof at degree 12.
pub const KAGEMUSHA_IPA_ACCUMULATION_PROOF_BYTES_V1: usize =
    (8 + 2 * KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1) * 32;
/// Version of the degree-parameterized accumulated-opening wire.
///
/// V4 is intentionally a distinct wire.  A V1 value can never be accepted by
/// a V4 parser merely because the authenticated degree happens to be 12.
pub const KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4: u16 = 4;

/// Return the exact V4 public-instance limb count for one authenticated IPA
/// round count.
pub fn kagemusha_ipa_accumulator_instance_limbs_v4(round_count: u32) -> Result<usize, String> {
    if round_count == 0 {
        return Err("Kagemusha V4 IPA round count must be non-zero".to_owned());
    }
    let scalar_and_point_count = round_count
        .checked_add(1)
        .ok_or_else(|| "Kagemusha V4 IPA round count overflows".to_owned())?;
    let encoded_values = scalar_and_point_count
        .checked_mul(8)
        .ok_or_else(|| "Kagemusha V4 IPA accumulator limb count overflows".to_owned())?;
    usize::try_from(
        encoded_values
            .checked_add(2)
            .ok_or_else(|| "Kagemusha V4 IPA accumulator limb count overflows".to_owned())?,
    )
    .map_err(|_| "Kagemusha V4 IPA accumulator limb count does not fit usize".to_owned())
}

/// Return the exact non-ZK BGH19 transcript length for an authenticated V4
/// IPA round count.
pub fn kagemusha_ipa_accumulation_proof_bytes_v4(round_count: u32) -> Result<usize, String> {
    if round_count == 0 {
        return Err("Kagemusha V4 IPA round count must be non-zero".to_owned());
    }
    let field_elements = round_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(8))
        .ok_or_else(|| "Kagemusha V4 IPA fold length overflows".to_owned())?;
    usize::try_from(
        field_elements
            .checked_mul(32)
            .ok_or_else(|| "Kagemusha V4 IPA fold length overflows".to_owned())?,
    )
    .map_err(|_| "Kagemusha V4 IPA fold length does not fit usize".to_owned())
}

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

    /// Encode the accumulator as a fixed field-neutral public-instance vector.
    pub fn instance_limbs(&self) -> Result<Vec<u32>, String> {
        self.validate_shape()?;
        let mut limbs = Vec::with_capacity(KAGEMUSHA_IPA_ACCUMULATOR_INSTANCE_LIMBS_V1);
        limbs.push(u32::from(self.version));
        limbs.push(
            u32::try_from(self.round_challenges.len())
                .map_err(|_| "Kagemusha IPA round count does not fit u32".to_owned())?,
        );
        for bytes in self
            .round_challenges
            .iter()
            .chain(std::iter::once(&self.folded_generator))
        {
            limbs.extend(bytes.chunks_exact(4).map(|chunk| {
                u32::from_le_bytes(chunk.try_into().expect("32-byte value has exact limbs"))
            }));
        }
        debug_assert_eq!(limbs.len(), KAGEMUSHA_IPA_ACCUMULATOR_INSTANCE_LIMBS_V1);
        Ok(limbs)
    }

    /// Decode the exact public-instance representation without field reduction.
    pub fn from_instance_limbs(limbs: &[u32]) -> Result<Self, String> {
        if limbs.len() != KAGEMUSHA_IPA_ACCUMULATOR_INSTANCE_LIMBS_V1
            || limbs[0] != u32::from(KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1)
            || limbs[1]
                != u32::try_from(KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1)
                    .expect("fixed Kagemusha IPA round count fits u32")
        {
            return Err("Kagemusha IPA accumulator instance shape mismatch".to_owned());
        }
        let values = limbs[2..]
            .chunks_exact(8)
            .map(|value_limbs| {
                let mut bytes = [0_u8; 32];
                for (target, limb) in bytes.chunks_exact_mut(4).zip(value_limbs) {
                    target.copy_from_slice(&limb.to_le_bytes());
                }
                bytes
            })
            .collect::<Vec<_>>();
        let (folded_generator, round_challenges) = values
            .split_last()
            .ok_or_else(|| "Kagemusha IPA accumulator instance is empty".to_owned())?;
        let wire = Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            round_challenges: round_challenges.to_vec(),
            folded_generator: *folded_generator,
        };
        wire.validate_shape()?;
        Ok(wire)
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

/// Degree-parameterized field-neutral IPA accumulator.
///
/// `round_count` is redundant with the challenge vector on purpose: it is an
/// authenticated shape commitment, not an inferred serialization detail.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaIpaAccumulatorWireV4 {
    /// Exact V4 wire version.
    pub version: u16,
    /// Authenticated IPA round count (equal to the circuit/Params degree).
    pub round_count: u32,
    /// Ordered canonical IPA round challenges.
    pub round_challenges: Vec<[u8; 32]>,
    /// Canonical compressed accumulated generator.
    pub folded_generator: [u8; 32],
}

impl KagemushaIpaAccumulatorWireV4 {
    /// Encode an Eq/Vesta accumulator under an explicit authenticated degree.
    pub fn from_eq(
        accumulator: &IpaAccumulator<EqAffine, NativeLoader>,
        round_count: u32,
    ) -> Result<Self, String> {
        let wire = Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count,
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
        };
        wire.validate_shape(round_count)?;
        wire.to_eq(round_count)?;
        Ok(wire)
    }

    /// Encode an Ep/Pallas accumulator under an explicit authenticated degree.
    pub fn from_ep(
        accumulator: &IpaAccumulator<EpAffine, NativeLoader>,
        round_count: u32,
    ) -> Result<Self, String> {
        let wire = Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count,
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
        };
        wire.validate_shape(round_count)?;
        wire.to_ep(round_count)?;
        Ok(wire)
    }

    /// Parse this wire as Eq/Vesta without reducing any scalar bytes.
    pub fn to_eq(
        &self,
        authenticated_round_count: u32,
    ) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
        use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

        self.validate_shape(authenticated_round_count)?;
        let xi = self
            .round_challenges
            .iter()
            .map(|bytes| {
                Option::<Fp>::from(Fp::from_repr((*bytes).into()))
                    .ok_or_else(|| "Kagemusha V4 Eq accumulator scalar is non-canonical".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let u = Option::<EqAffine>::from(EqAffine::from_bytes(&self.folded_generator.into()))
            .ok_or_else(|| "Kagemusha V4 Eq accumulator point is non-canonical".to_owned())?;
        if bool::from(u.is_identity()) {
            return Err("Kagemusha V4 Eq accumulator point is identity".to_owned());
        }
        Ok(IpaAccumulator::new(xi, u))
    }

    /// Parse this wire as Ep/Pallas without reducing any scalar bytes.
    pub fn to_ep(
        &self,
        authenticated_round_count: u32,
    ) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
        use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

        self.validate_shape(authenticated_round_count)?;
        let xi = self
            .round_challenges
            .iter()
            .map(|bytes| {
                Option::<Fq>::from(Fq::from_repr((*bytes).into()))
                    .ok_or_else(|| "Kagemusha V4 Ep accumulator scalar is non-canonical".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let u = Option::<EpAffine>::from(EpAffine::from_bytes(&self.folded_generator.into()))
            .ok_or_else(|| "Kagemusha V4 Ep accumulator point is non-canonical".to_owned())?;
        if bool::from(u.is_identity()) {
            return Err("Kagemusha V4 Ep accumulator point is identity".to_owned());
        }
        Ok(IpaAccumulator::new(xi, u))
    }

    /// Encode this accumulator as the exact dynamic V4 public-instance vector.
    pub fn instance_limbs(&self, authenticated_round_count: u32) -> Result<Vec<u32>, String> {
        self.validate_shape(authenticated_round_count)?;
        let expected = kagemusha_ipa_accumulator_instance_limbs_v4(authenticated_round_count)?;
        let mut limbs = Vec::with_capacity(expected);
        limbs.push(u32::from(self.version));
        limbs.push(self.round_count);
        for bytes in self
            .round_challenges
            .iter()
            .chain(std::iter::once(&self.folded_generator))
        {
            limbs.extend(bytes.chunks_exact(4).map(|chunk| {
                u32::from_le_bytes(chunk.try_into().expect("32-byte value has exact limbs"))
            }));
        }
        if limbs.len() != expected {
            return Err("Kagemusha V4 IPA accumulator encoded length mismatch".to_owned());
        }
        Ok(limbs)
    }

    /// Decode the exact dynamic V4 instance representation.
    pub fn from_instance_limbs(
        limbs: &[u32],
        authenticated_round_count: u32,
    ) -> Result<Self, String> {
        let expected = kagemusha_ipa_accumulator_instance_limbs_v4(authenticated_round_count)?;
        if limbs.len() != expected
            || limbs.first().copied() != Some(u32::from(KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4))
            || limbs.get(1).copied() != Some(authenticated_round_count)
        {
            return Err("Kagemusha V4 IPA accumulator instance shape mismatch".to_owned());
        }
        let values = limbs[2..]
            .chunks_exact(8)
            .map(|value_limbs| {
                let mut bytes = [0_u8; 32];
                for (target, limb) in bytes.chunks_exact_mut(4).zip(value_limbs) {
                    target.copy_from_slice(&limb.to_le_bytes());
                }
                bytes
            })
            .collect::<Vec<_>>();
        let (folded_generator, round_challenges) = values
            .split_last()
            .ok_or_else(|| "Kagemusha V4 IPA accumulator instance is empty".to_owned())?;
        let wire = Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count: authenticated_round_count,
            round_challenges: round_challenges.to_vec(),
            folded_generator: *folded_generator,
        };
        wire.validate_shape(authenticated_round_count)?;
        Ok(wire)
    }

    /// Validate only the authenticated V4 wire shape.
    pub fn validate_shape(&self, authenticated_round_count: u32) -> Result<(), String> {
        kagemusha_ipa_accumulator_instance_limbs_v4(authenticated_round_count)?;
        if self.version != KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4
            || self.round_count != authenticated_round_count
            || usize::try_from(authenticated_round_count).ok() != Some(self.round_challenges.len())
        {
            return Err("Kagemusha V4 IPA accumulator wire shape mismatch".to_owned());
        }
        Ok(())
    }
}

/// Degree-parameterized opaque BGH19 fold transcript.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaIpaAccumulationProofV4 {
    /// Exact V4 wire version.
    pub version: u16,
    /// Authenticated IPA round count.
    pub round_count: u32,
    /// Empty only for a native initialization marker; fixed-shape recursive
    /// witnesses use [`Self::validate_fixed_transcript`] and require all bytes.
    pub bytes: Vec<u8>,
}

impl KagemushaIpaAccumulationProofV4 {
    /// Construct the native initialization marker for an explicit degree.
    pub fn initialization(round_count: u32) -> Result<Self, String> {
        kagemusha_ipa_accumulation_proof_bytes_v4(round_count)?;
        Ok(Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count,
            bytes: Vec::new(),
        })
    }

    /// Construct and validate a complete fold transcript.
    pub fn from_fold_bytes(round_count: u32, bytes: Vec<u8>) -> Result<Self, String> {
        let proof = Self {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count,
            bytes,
        };
        proof.validate_fixed_transcript(round_count)?;
        Ok(proof)
    }

    /// Validate the native optional-parent representation.
    pub fn validate(&self, authenticated_round_count: u32, has_parent: bool) -> Result<(), String> {
        self.validate_header(authenticated_round_count)?;
        let expected = kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)?;
        if (has_parent && self.bytes.len() != expected) || (!has_parent && !self.bytes.is_empty()) {
            return Err("Kagemusha V4 IPA accumulation proof shape mismatch".to_owned());
        }
        Ok(())
    }

    /// Validate the always-present transcript required by a fixed-shape Step
    /// witness, including disabled/bootstrap fold stages.
    pub fn validate_fixed_transcript(&self, authenticated_round_count: u32) -> Result<(), String> {
        self.validate_header(authenticated_round_count)?;
        if self.bytes.len() != kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)?
        {
            return Err("Kagemusha V4 fixed IPA fold transcript shape mismatch".to_owned());
        }
        Ok(())
    }

    fn validate_header(&self, authenticated_round_count: u32) -> Result<(), String> {
        if self.version != KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4
            || self.round_count != authenticated_round_count
        {
            return Err("Kagemusha V4 IPA accumulation proof header mismatch".to_owned());
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

/// Fold Eq accumulators under an explicit authenticated V4 degree.
pub fn fold_eq_accumulators_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    authenticated_round_count: u32,
    current: IpaAccumulator<EqAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EqAffine, NativeLoader>>,
) -> Result<
    (
        KagemushaIpaAccumulationProofV4,
        IpaAccumulator<EqAffine, NativeLoader>,
    ),
    String,
> {
    kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)?;
    if params.k() != authenticated_round_count
        || usize::try_from(authenticated_round_count).ok() != Some(current.xi.len())
        || parent.as_ref().is_some_and(|value| {
            usize::try_from(authenticated_round_count).ok() != Some(value.xi.len())
        })
    {
        return Err("Kagemusha V4 Eq fold degree mismatch".to_owned());
    }
    let Some(parent) = parent else {
        return Ok((
            KagemushaIpaAccumulationProofV4::initialization(authenticated_round_count)?,
            current,
        ));
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
    .map_err(|error| format!("failed to create Kagemusha V4 Eq accumulation proof: {error:?}"))?;
    let proof = KagemushaIpaAccumulationProofV4::from_fold_bytes(
        authenticated_round_count,
        transcript.finalize(),
    )?;
    Ok((proof, accumulated))
}

/// Fold Ep accumulators under an explicit authenticated V4 degree.
pub fn fold_ep_accumulators_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    authenticated_round_count: u32,
    current: IpaAccumulator<EpAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EpAffine, NativeLoader>>,
) -> Result<
    (
        KagemushaIpaAccumulationProofV4,
        IpaAccumulator<EpAffine, NativeLoader>,
    ),
    String,
> {
    kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)?;
    if params.k() != authenticated_round_count
        || usize::try_from(authenticated_round_count).ok() != Some(current.xi.len())
        || parent.as_ref().is_some_and(|value| {
            usize::try_from(authenticated_round_count).ok() != Some(value.xi.len())
        })
    {
        return Err("Kagemusha V4 Ep fold degree mismatch".to_owned());
    }
    let Some(parent) = parent else {
        return Ok((
            KagemushaIpaAccumulationProofV4::initialization(authenticated_round_count)?,
            current,
        ));
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
    .map_err(|error| format!("failed to create Kagemusha V4 Ep accumulation proof: {error:?}"))?;
    let proof = KagemushaIpaAccumulationProofV4::from_fold_bytes(
        authenticated_round_count,
        transcript.finalize(),
    )?;
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

/// Verify and terminally decide an Eq fold under the authenticated V4 degree.
pub fn verify_and_decide_eq_accumulation_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
    authenticated_round_count: u32,
    current: IpaAccumulator<EqAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EqAffine, NativeLoader>>,
    proof: &KagemushaIpaAccumulationProofV4,
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, String> {
    if params.k() != authenticated_round_count
        || usize::try_from(authenticated_round_count).ok() != Some(current.xi.len())
        || parent.as_ref().is_some_and(|value| {
            usize::try_from(authenticated_round_count).ok() != Some(value.xi.len())
        })
    {
        return Err("Kagemusha V4 Eq decision degree mismatch".to_owned());
    }
    proof.validate(authenticated_round_count, parent.is_some())?;
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
        .map_err(|error| {
            format!("failed to parse Kagemusha V4 Eq accumulation proof: {error:?}")
        })?;
        let accumulated = <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
        .map_err(|error| {
            format!("failed to verify Kagemusha V4 Eq accumulation proof: {error:?}")
        })?;
        let cursor = transcript.finalize();
        if cursor.position()
            != u64::try_from(proof.bytes.len())
                .map_err(|_| "Kagemusha V4 Eq fold length does not fit u64".to_owned())?
        {
            return Err("Kagemusha V4 Eq accumulation proof has trailing bytes".to_owned());
        }
        accumulated
    } else {
        current
    };
    <EqAccumulation as AccumulationDecider<EqAffine, NativeLoader>>::decide(
        &deciding_key,
        accumulated.clone(),
    )
    .map_err(|error| format!("Kagemusha V4 Eq accumulated decision failed: {error:?}"))?;
    Ok(accumulated)
}

/// Verify and terminally decide an Ep fold under the authenticated V4 degree.
pub fn verify_and_decide_ep_accumulation_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
    authenticated_round_count: u32,
    current: IpaAccumulator<EpAffine, NativeLoader>,
    parent: Option<IpaAccumulator<EpAffine, NativeLoader>>,
    proof: &KagemushaIpaAccumulationProofV4,
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, String> {
    if params.k() != authenticated_round_count
        || usize::try_from(authenticated_round_count).ok() != Some(current.xi.len())
        || parent.as_ref().is_some_and(|value| {
            usize::try_from(authenticated_round_count).ok() != Some(value.xi.len())
        })
    {
        return Err("Kagemusha V4 Ep decision degree mismatch".to_owned());
    }
    proof.validate(authenticated_round_count, parent.is_some())?;
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
        .map_err(|error| {
            format!("failed to parse Kagemusha V4 Ep accumulation proof: {error:?}")
        })?;
        let accumulated = <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
        .map_err(|error| {
            format!("failed to verify Kagemusha V4 Ep accumulation proof: {error:?}")
        })?;
        let cursor = transcript.finalize();
        if cursor.position()
            != u64::try_from(proof.bytes.len())
                .map_err(|_| "Kagemusha V4 Ep fold length does not fit u64".to_owned())?
        {
            return Err("Kagemusha V4 Ep accumulation proof has trailing bytes".to_owned());
        }
        accumulated
    } else {
        current
    };
    <EpAccumulation as AccumulationDecider<EpAffine, NativeLoader>>::decide(
        &deciding_key,
        accumulated.clone(),
    )
    .map_err(|error| format!("Kagemusha V4 Ep accumulated decision failed: {error:?}"))?;
    Ok(accumulated)
}

#[cfg(test)]
mod tests {
    use ff::Field as _;
    use halo2_proofs::{
        halo2curves::group::{Curve as _, Group as _},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    use super::*;

    #[test]
    fn v4_dynamic_sizes_are_checked_and_cross_version() {
        assert_eq!(
            kagemusha_ipa_accumulator_instance_limbs_v4(12).unwrap(),
            KAGEMUSHA_IPA_ACCUMULATOR_INSTANCE_LIMBS_V1
        );
        assert_eq!(
            kagemusha_ipa_accumulation_proof_bytes_v4(12).unwrap(),
            KAGEMUSHA_IPA_ACCUMULATION_PROOF_BYTES_V1
        );
        assert_eq!(
            kagemusha_ipa_accumulator_instance_limbs_v4(20).unwrap(),
            170
        );
        assert_eq!(
            kagemusha_ipa_accumulation_proof_bytes_v4(20).unwrap(),
            1_536
        );
        assert!(kagemusha_ipa_accumulator_instance_limbs_v4(0).is_err());
        assert!(kagemusha_ipa_accumulator_instance_limbs_v4(u32::MAX).is_err());
        assert!(kagemusha_ipa_accumulation_proof_bytes_v4(u32::MAX).is_err());

        let v4_fold = KagemushaIpaAccumulationProofV4::from_fold_bytes(
            20,
            vec![0; kagemusha_ipa_accumulation_proof_bytes_v4(20).unwrap()],
        )
        .unwrap();
        assert!(v4_fold.validate_fixed_transcript(20).is_ok());
        assert!(v4_fold.validate_fixed_transcript(12).is_err());
        let cross_version = KagemushaIpaAccumulationProofV4 {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            ..v4_fold
        };
        assert!(cross_version.validate_fixed_transcript(20).is_err());
    }

    fn ipa_h_coefficients<F: ff::Field>(challenges: &[F], scalar: F) -> Vec<F> {
        // This is the BGH19 coefficient expansion used by the verifier: walk
        // challenges in reverse and duplicate each existing half scaled by
        // the next challenge.
        assert!(!challenges.is_empty());
        let mut coefficients = vec![F::ZERO; 1 << challenges.len()];
        coefficients[0] = scalar;
        for (len, challenge) in challenges
            .iter()
            .rev()
            .enumerate()
            .map(|(index, challenge)| (1 << index, challenge))
        {
            let (left, right) = coefficients.split_at_mut(len);
            let right = &mut right[..len];
            right.copy_from_slice(left);
            for coefficient in right {
                *coefficient *= challenge;
            }
        }
        coefficients
    }

    fn eq_accumulator(
        params: &ParamsIPA<EqAffine>,
        seed: u64,
    ) -> IpaAccumulator<EqAffine, NativeLoader> {
        use halo2_proofs::poly::commitment::ParamsProver as _;
        let xi = (0..KAGEMUSHA_IPA_ACCUMULATOR_ROUNDS_V1)
            .map(|round| Fp::from(seed + round as u64 + 1))
            .collect::<Vec<_>>();
        let coefficients = ipa_h_coefficients(&xi, Fp::ONE);
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
        let coefficients = ipa_h_coefficients(&xi, Fq::ONE);
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

        let eq_wire = KagemushaIpaAccumulatorWireV1::from_eq(&eq);
        assert_eq!(
            KagemushaIpaAccumulatorWireV1::from_instance_limbs(&eq_wire.instance_limbs().unwrap())
                .unwrap(),
            eq_wire
        );

        let mut noncanonical = KagemushaIpaAccumulatorWireV1::from_eq(&eq);
        noncanonical.round_challenges[0] = [0xFF; 32];
        assert!(noncanonical.to_eq().is_err());
    }

    #[test]
    fn accumulator_wire_rejects_every_shape_and_canonicality_substitution() {
        let eq_params = ParamsIPA::<EqAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let ep_params = ParamsIPA::<EpAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let eq_wire = KagemushaIpaAccumulatorWireV1::from_eq(&eq_accumulator(&eq_params, 31));
        let ep_wire = KagemushaIpaAccumulatorWireV1::from_ep(&ep_accumulator(&ep_params, 37));

        let mut wrong_version = eq_wire.clone();
        wrong_version.version += 1;
        assert!(wrong_version.to_eq().is_err());
        assert!(wrong_version.instance_limbs().is_err());

        let mut wrong_round_count = eq_wire.clone();
        wrong_round_count.round_challenges.pop();
        assert!(wrong_round_count.to_eq().is_err());
        assert!(wrong_round_count.instance_limbs().is_err());

        let mut noncanonical_eq_point = eq_wire.clone();
        noncanonical_eq_point.folded_generator = [0xFF; 32];
        assert!(noncanonical_eq_point.to_eq().is_err());

        let mut noncanonical_ep_scalar = ep_wire.clone();
        noncanonical_ep_scalar.round_challenges[0] = [0xFF; 32];
        assert!(noncanonical_ep_scalar.to_ep().is_err());
        let mut noncanonical_ep_point = ep_wire;
        noncanonical_ep_point.folded_generator = [0xFF; 32];
        assert!(noncanonical_ep_point.to_ep().is_err());

        let limbs = eq_wire.instance_limbs().unwrap();
        assert!(
            KagemushaIpaAccumulatorWireV1::from_instance_limbs(&limbs[..limbs.len() - 1]).is_err()
        );
        let mut wrong_instance_version = limbs.clone();
        wrong_instance_version[0] += 1;
        assert!(
            KagemushaIpaAccumulatorWireV1::from_instance_limbs(&wrong_instance_version).is_err()
        );
        let mut wrong_instance_round_count = limbs;
        wrong_instance_round_count[1] -= 1;
        assert!(
            KagemushaIpaAccumulatorWireV1::from_instance_limbs(&wrong_instance_round_count)
                .is_err()
        );
    }

    #[test]
    fn accumulation_proof_shape_distinguishes_initialization_from_parent_fold() {
        let initialization = KagemushaIpaAccumulationProofV1::initialization();
        assert!(initialization.validate(false).is_ok());
        assert!(initialization.validate(true).is_err());

        let mut wrong_version = initialization.clone();
        wrong_version.version += 1;
        assert!(wrong_version.validate(false).is_err());

        let mut unexpected_init_bytes = initialization;
        unexpected_init_bytes.bytes.push(0);
        assert!(unexpected_init_bytes.validate(false).is_err());

        let exact_fold = KagemushaIpaAccumulationProofV1 {
            version: KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V1,
            bytes: vec![0; KAGEMUSHA_IPA_ACCUMULATION_PROOF_BYTES_V1],
        };
        assert!(exact_fold.validate(true).is_ok());
        let mut truncated = exact_fold.clone();
        truncated.bytes.pop();
        assert!(truncated.validate(true).is_err());
        let mut trailing = exact_fold;
        trailing.bytes.push(0);
        assert!(trailing.validate(true).is_err());
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

        let (eq_proof, _) = fold_eq_accumulators(
            &eq_params,
            eq_accumulator(&eq_params, 3),
            Some(eq_accumulator(&eq_params, 19)),
        )
        .unwrap();
        assert!(
            verify_and_decide_eq_accumulation(
                &eq_params,
                eq_accumulator(&eq_params, 3),
                Some(eq_accumulator(&eq_params, 20)),
                &eq_proof,
            )
            .is_err(),
            "a different parent accumulator must invalidate the fold transcript"
        );

        let ep_params = ParamsIPA::<EpAffine>::new(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1,
        );
        let ep_current = ep_accumulator(&ep_params, 5);
        let ep_parent = ep_accumulator(&ep_params, 23);
        let (ep_proof, ep_expected) =
            fold_ep_accumulators(&ep_params, ep_current.clone(), Some(ep_parent.clone())).unwrap();
        let ep_actual = verify_and_decide_ep_accumulation(
            &ep_params,
            ep_current.clone(),
            Some(ep_parent),
            &ep_proof,
        )
        .unwrap();
        assert_eq!(ep_actual.xi, ep_expected.xi);
        assert_eq!(ep_actual.u, ep_expected.u);
        let mut tampered_ep = ep_proof;
        let tamper_index = tampered_ep.bytes.len() / 2;
        tampered_ep.bytes[tamper_index] ^= 1;
        assert!(
            verify_and_decide_ep_accumulation(
                &ep_params,
                ep_current,
                Some(ep_accumulator(&ep_params, 23)),
                &tampered_ep,
            )
            .is_err()
        );
    }
}
