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
/// Version of the degree-parameterized accumulated-opening wire.
///
/// V4 is intentionally a distinct wire.  A V1 value can never be accepted by
/// a V4 parser merely because the authenticated degree happens to be 12.
pub const KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4: u16 = 5;
/// Return the exact V4 public-instance limb count for one authenticated IPA
/// round count.
pub fn kagemusha_ipa_accumulator_instance_limbs_v4(round_count: u32) -> Result<usize, String> {
    if round_count == 0 {
        return Err("Kagemusha V4 IPA round count must be non-zero".to_owned());
    }
    usize::try_from(
        round_count
            .checked_mul(2)
            .and_then(|value| value.checked_add(4))
            .ok_or_else(|| "Kagemusha V4 IPA accumulator chunk count overflows".to_owned())?,
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
fn catch_native_verifier_panic<T>(label: &str, verify: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(verify))
        .map_err(|_| format!("Kagemusha V4 {label} rejected an invalid native verifier relation"))
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
    pub fn instance_limbs(&self, authenticated_round_count: u32) -> Result<Vec<u128>, String> {
        self.validate_shape(authenticated_round_count)?;
        let expected = kagemusha_ipa_accumulator_instance_limbs_v4(authenticated_round_count)?;
        let mut limbs = Vec::with_capacity(expected);
        limbs.push(u128::from(self.version));
        limbs.push(u128::from(self.round_count));
        for bytes in self
            .round_challenges
            .iter()
            .chain(std::iter::once(&self.folded_generator))
        {
            limbs.extend(bytes.chunks_exact(16).map(|chunk| {
                u128::from_le_bytes(chunk.try_into().expect("32-byte value has exact chunks"))
            }));
        }
        if limbs.len() != expected {
            return Err("Kagemusha V4 IPA accumulator encoded length mismatch".to_owned());
        }
        Ok(limbs)
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
fn eq_proving_key(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
) -> IpaProvingKey<EqAffine> {
    use halo2_proofs::poly::commitment::ParamsProver as _;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let h = hash_to_curve(&[2]).to_affine();
    let s = Some(hash_to_curve(&[1]).to_affine());
    #[cfg(test)]
    record_key_construction(KeyConstruction::EqProving);
    IpaProvingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g().to_vec(),
        h,
        s,
    )
}
fn eq_deciding_key(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EqAffine>,
) -> IpaDecidingKey<EqAffine> {
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
    #[cfg(test)]
    record_key_construction(KeyConstruction::EqDeciding);
    IpaDecidingKey::new(svk, params.get_g().to_vec())
}
fn ep_proving_key(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
) -> IpaProvingKey<EpAffine> {
    use halo2_proofs::poly::commitment::ParamsProver as _;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let h = hash_to_curve(&[2]).to_affine();
    let s = Some(hash_to_curve(&[1]).to_affine());
    #[cfg(test)]
    record_key_construction(KeyConstruction::EpProving);
    IpaProvingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g().to_vec(),
        h,
        s,
    )
}
fn ep_deciding_key(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<EpAffine>,
) -> IpaDecidingKey<EpAffine> {
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
    #[cfg(test)]
    record_key_construction(KeyConstruction::EpDeciding);
    IpaDecidingKey::new(svk, params.get_g().to_vec())
}
#[cfg(test)]
#[derive(Clone, Copy)]
enum KeyConstruction {
    EqProving = 0,
    EqDeciding = 1,
    EpProving = 2,
    EpDeciding = 3,
}
#[cfg(test)]
std::thread_local! {
    static KEY_CONSTRUCTION_COUNTS: std::cell::Cell<[usize; 4]> = const {
        std::cell::Cell::new([0; 4])
    };
}
#[cfg(test)]
fn record_key_construction(kind: KeyConstruction) {
    KEY_CONSTRUCTION_COUNTS.with(|counts| {
        let mut values = counts.get();
        values[kind as usize] += 1;
        counts.set(values);
    });
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
    let proving_key = eq_proving_key(params);
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
    let proving_key = ep_proving_key(params);
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
    let deciding_key = eq_deciding_key(params);
    let accumulated = if let Some(parent) = parent {
        let inputs = [current, parent];
        let cursor = std::io::Cursor::new(proof.bytes.clone());
        let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
        let parsed = catch_native_verifier_panic("Eq accumulation proof parse", || {
            <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                deciding_key.as_ref(),
                &inputs,
                &mut transcript,
            )
        })?
        .map_err(|error| {
            format!("failed to parse Kagemusha V4 Eq accumulation proof: {error:?}")
        })?;
        let accumulated =
            catch_native_verifier_panic("Eq accumulation proof verification", || {
                <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::verify(
                    deciding_key.as_ref(),
                    &inputs,
                    &parsed,
                )
            })?
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
    catch_native_verifier_panic("Eq accumulated decision", || {
        <EqAccumulation as AccumulationDecider<EqAffine, NativeLoader>>::decide(
            &deciding_key,
            accumulated.clone(),
        )
    })?
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
    let deciding_key = ep_deciding_key(params);
    let accumulated = if let Some(parent) = parent {
        let inputs = [current, parent];
        let cursor = std::io::Cursor::new(proof.bytes.clone());
        let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
        let parsed = catch_native_verifier_panic("Ep accumulation proof parse", || {
            <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
                deciding_key.as_ref(),
                &inputs,
                &mut transcript,
            )
        })?
        .map_err(|error| {
            format!("failed to parse Kagemusha V4 Ep accumulation proof: {error:?}")
        })?;
        let accumulated =
            catch_native_verifier_panic("Ep accumulation proof verification", || {
                <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::verify(
                    deciding_key.as_ref(),
                    &inputs,
                    &parsed,
                )
            })?
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
    catch_native_verifier_panic("Ep accumulated decision", || {
        <EpAccumulation as AccumulationDecider<EpAffine, NativeLoader>>::decide(
            &deciding_key,
            accumulated.clone(),
        )
    })?
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
    fn ipa_h_coefficients<F: ff::Field>(challenges: &[F], scalar: F) -> Vec<F> {
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
        let round_count = usize::try_from(params.k()).expect("test degree fits usize");
        let xi = (0..round_count)
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
        let round_count = usize::try_from(params.k()).expect("test degree fits usize");
        let xi = (0..round_count)
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
    fn reset_key_construction_counts() {
        KEY_CONSTRUCTION_COUNTS.with(|counts| counts.set([0; 4]));
    }
    fn key_construction_counts() -> [usize; 4] {
        KEY_CONSTRUCTION_COUNTS.with(std::cell::Cell::get)
    }
    #[test]
    fn parity_key_builders_construct_consistent_requested_keys() {
        const K: u32 = 4;
        let eq_params = ParamsIPA::<EqAffine>::new(K);
        let eq_proving_key = eq_proving_key(&eq_params);
        assert_eq!(eq_proving_key.domain.k, K as usize);
        assert_eq!(eq_proving_key.g.as_slice(), eq_params.get_g());
        let eq_proving_svk = eq_proving_key.svk();
        drop(eq_proving_key);
        let eq_deciding_key = eq_deciding_key(&eq_params);
        let eq_deciding_svk = eq_deciding_key.as_ref();
        assert_eq!(eq_deciding_svk.domain.k, eq_proving_svk.domain.k);
        assert_eq!(eq_deciding_svk.g, eq_proving_svk.g);
        assert_eq!(eq_deciding_svk.h, eq_proving_svk.h);
        assert_eq!(eq_deciding_svk.s, eq_proving_svk.s);
        let ep_params = ParamsIPA::<EpAffine>::new(K);
        let ep_proving_key = ep_proving_key(&ep_params);
        assert_eq!(ep_proving_key.domain.k, K as usize);
        assert_eq!(ep_proving_key.g.as_slice(), ep_params.get_g());
        let ep_proving_svk = ep_proving_key.svk();
        drop(ep_proving_key);
        let ep_deciding_key = ep_deciding_key(&ep_params);
        let ep_deciding_svk = ep_deciding_key.as_ref();
        assert_eq!(ep_deciding_svk.domain.k, ep_proving_svk.domain.k);
        assert_eq!(ep_deciding_svk.g, ep_proving_svk.g);
        assert_eq!(ep_deciding_svk.h, ep_proving_svk.h);
        assert_eq!(ep_deciding_svk.s, ep_proving_svk.s);
    }
    #[test]
    fn v4_dynamic_sizes_and_headers_are_exact() {
        assert_eq!(kagemusha_ipa_accumulator_instance_limbs_v4(12).unwrap(), 28);
        assert_eq!(
            kagemusha_ipa_accumulation_proof_bytes_v4(12).unwrap(),
            1_024
        );
        assert_eq!(kagemusha_ipa_accumulator_instance_limbs_v4(20).unwrap(), 44);
        assert_eq!(
            kagemusha_ipa_accumulation_proof_bytes_v4(20).unwrap(),
            1_536
        );
        assert!(kagemusha_ipa_accumulator_instance_limbs_v4(0).is_err());
        assert!(kagemusha_ipa_accumulator_instance_limbs_v4(u32::MAX).is_err());
        assert!(kagemusha_ipa_accumulation_proof_bytes_v4(u32::MAX).is_err());
        let proof = KagemushaIpaAccumulationProofV4::from_fold_bytes(
            20,
            vec![0; kagemusha_ipa_accumulation_proof_bytes_v4(20).unwrap()],
        )
        .unwrap();
        assert!(proof.validate_fixed_transcript(20).is_ok());
        assert!(proof.validate_fixed_transcript(12).is_err());
        let cross_version = KagemushaIpaAccumulationProofV4 {
            version: 1,
            ..proof
        };
        assert!(cross_version.validate_fixed_transcript(20).is_err());
    }
    #[test]
    fn v4_eq_wire_and_fold_reject_substitution() {
        const K: u32 = 12;
        let params = ParamsIPA::<EqAffine>::new(K);
        let current = eq_accumulator(&params, 3);
        let parent = eq_accumulator(&params, 19);
        let wire = KagemushaIpaAccumulatorWireV4::from_eq(&current, K).unwrap();
        assert_eq!(wire.to_eq(K).unwrap().xi, current.xi);
        assert_eq!(wire.instance_limbs(K).unwrap().len(), 28);
        let mut noncanonical = wire;
        noncanonical.round_challenges[0] = [0xFF; 32];
        assert!(noncanonical.to_eq(K).is_err());
        let (proof, expected) =
            fold_eq_accumulators_v4(&params, K, current.clone(), Some(parent.clone())).unwrap();
        let actual = verify_and_decide_eq_accumulation_v4(
            &params,
            K,
            current.clone(),
            Some(parent.clone()),
            &proof,
        )
        .unwrap();
        assert_eq!(actual.xi, expected.xi);
        assert_eq!(actual.u, expected.u);
        let mut tampered = proof;
        tampered.bytes[0] ^= 1;
        assert!(
            verify_and_decide_eq_accumulation_v4(&params, K, current, Some(parent), &tampered,)
                .is_err()
        );
    }
    #[test]
    fn v4_fold_and_decision_construct_only_their_required_key_material() {
        const K: u32 = 4;
        let eq_params = ParamsIPA::<EqAffine>::new(K);
        let eq_current = eq_accumulator(&eq_params, 3);
        let eq_parent = eq_accumulator(&eq_params, 19);
        reset_key_construction_counts();
        let (eq_proof, eq_expected) =
            fold_eq_accumulators_v4(&eq_params, K, eq_current.clone(), Some(eq_parent.clone()))
                .unwrap();
        assert_eq!(key_construction_counts(), [1, 0, 0, 0]);
        reset_key_construction_counts();
        let eq_actual = verify_and_decide_eq_accumulation_v4(
            &eq_params,
            K,
            eq_current,
            Some(eq_parent),
            &eq_proof,
        )
        .unwrap();
        assert_eq!(key_construction_counts(), [0, 1, 0, 0]);
        assert_eq!(eq_actual.xi, eq_expected.xi);
        assert_eq!(eq_actual.u, eq_expected.u);
        let ep_params = ParamsIPA::<EpAffine>::new(K);
        let ep_current = ep_accumulator(&ep_params, 7);
        let ep_parent = ep_accumulator(&ep_params, 23);
        reset_key_construction_counts();
        let (ep_proof, ep_expected) =
            fold_ep_accumulators_v4(&ep_params, K, ep_current.clone(), Some(ep_parent.clone()))
                .unwrap();
        assert_eq!(key_construction_counts(), [0, 0, 1, 0]);
        reset_key_construction_counts();
        let ep_actual = verify_and_decide_ep_accumulation_v4(
            &ep_params,
            K,
            ep_current,
            Some(ep_parent),
            &ep_proof,
        )
        .unwrap();
        assert_eq!(key_construction_counts(), [0, 0, 0, 1]);
        assert_eq!(ep_actual.xi, ep_expected.xi);
        assert_eq!(ep_actual.u, ep_expected.u);
    }
}
