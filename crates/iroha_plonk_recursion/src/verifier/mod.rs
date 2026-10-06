//! Descriptor-fixed PIPA-R succinct verifier circuit.
//!
//! This interpreter follows the engine's transcript schedule, expression
//! programs and constraint table. Its output remains an undecided generator
//! claim. The owning relation must authorize the key digest, register the
//! output in its obligation ledger and eventually decide that ledger.
//! Soft verification constrains an exact verdict and selects a fixed valid
//! dummy claim on failure; hard verification additionally asserts the verdict.

mod expressions;
mod multiopen;
pub(crate) mod scalar;
#[cfg(test)]
mod tests;

use crate::{
    codec::{ScalarCells, decode_point_soft, decode_scalar_soft},
    transcript::{Domain, TranscriptChip},
};
use ff::{Field, PrimeField};
use group::prime::PrimeCurveAffine;
use iroha_pasta::PastaCurve;
use iroha_plonk::{
    DescriptorBinding, Protocol, VerifyError, VerifyingKey,
    cs::{ConstraintSystem, DescriptorRule, InstanceModeV1, ProofSuffixV1, TranscriptV2},
    frontend::{Error, Layouter, Region, Value},
    pcs::ipa::PinnedParams,
    protocol::{Challenge, CommonInput, ProofMessage, TranscriptStep},
    transcript::TranscriptRepr,
};
use iroha_plonk_gadgets::{
    Bit, GlueChip, GlueConfig, Uint, Word,
    bytes::{element::LeElement, tape::BytesConfig},
    ecc::{AssignedPoint, EccChip, EccConfig, NonIdentityPoint, ScalarLimbs},
    ff::{FfChip, FfConfig},
    poseidon::{Pow5Columns, RoundConstantColumns},
    pow5_fq::{DuplexChip, DuplexConfig},
    range::{LimbBits, RunningSumChip, RunningSumConfig, u128::UintChip},
};
use scalar::{Arithmetic, Scalar};

/// Whether proof failures become a constrained zero or an unsatisfied circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationMode {
    /// Exposes the complete validity bit; every proof byte pattern is satisfiable.
    Soft,
    /// Requires that complete validity bit to equal one.
    Hard,
}

/// Validated circuit-fixed protocol tables and pinned parameter points.
#[derive(Clone, Debug)]
pub struct VerifierPlan<C: PastaCurve> {
    binding: DescriptorBinding,
    protocol: Protocol,
    params: PinnedParams<C>,
    dummy: C::AffineExt,
}
impl<C: PastaCurve> VerifierPlan<C> {
    /// Builds the immutable program; only V2 PIPA-R Direct+suffix is admitted.
    ///
    /// # Errors
    /// Rejects a different profile or parameter curve/k and malformed tables.
    pub fn new(binding: DescriptorBinding, params: PinnedParams<C>) -> Result<Self, VerifyError> {
        let d = binding.descriptor();
        if !binding.is_v2()
            || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
            || d.instance_mode != InstanceModeV1::Direct
            || d.proof_suffix != ProofSuffixV1::FoldedGenerator
        {
            return Err(VerifyError::Descriptor(
                DescriptorRule::TranscriptProfile.into(),
            ));
        }
        if params.curve() != d.curve || params.k() != u32::from(d.k) {
            return Err(VerifyError::ParamsMismatch);
        }
        let protocol = Protocol::new(d)?;
        let dummy = params
            .params()
            .g()
            .iter()
            .fold(C::identity(), |sum, point| sum + point.to_curve())
            .to_affine();
        if bool::from(dummy.is_identity()) {
            return Err(VerifyError::ParamsMismatch);
        }
        Ok(Self {
            binding,
            protocol,
            params,
            dummy,
        })
    }
    /// Exact proof-byte count of this program.
    #[must_use]
    pub fn proof_length(&self) -> usize {
        self.protocol.proof_length()
    }
    /// The descriptor binding used by every table and key digest.
    #[must_use]
    pub const fn binding(&self) -> &DescriptorBinding {
        &self.binding
    }
    /// The pinned source generator prefix used by this program.
    #[must_use]
    pub const fn params(&self) -> &PinnedParams<C> {
        &self.params
    }
}

/// A key represented by constrained finite points and its base-field repr.
/// Authorization of the resulting key digest belongs to the owning relation.
#[derive(Clone, Debug)]
pub struct VerifierKeyCells<C: PastaCurve> {
    /// PIPA-R's base-field VK representation.
    pub representation: Word<C::Base>,
    /// Fixed commitments, in the descriptor's wire order.
    pub fixed: Vec<NonIdentityPoint<C::Base>>,
    /// Permutation commitments, in equality-column order.
    pub permutation: Vec<NonIdentityPoint<C::Base>>,
}

/// A constrained source-k generator claim, never an acceptance by itself.
#[must_use = "register this generator claim in the obligation ledger"]
#[derive(Clone, Debug)]
pub struct GeneratorClaimCells<C: PastaCurve> {
    pub(crate) k: u32,
    pub(crate) g: NonIdentityPoint<C::Base>,
    pub(crate) challenges: Vec<ScalarCells<C>>,
}
impl<C: PastaCurve> GeneratorClaimCells<C> {
    /// Source polynomial logarithmic length.
    #[must_use]
    pub const fn k(&self) -> u32 {
        self.k
    }
    /// Claimed folded generator.
    #[must_use]
    pub const fn g(&self) -> &NonIdentityPoint<C::Base> {
        &self.g
    }
    /// Nonzero source challenges, without normalization padding.
    #[must_use]
    pub fn challenges(&self) -> &[ScalarCells<C>] {
        &self.challenges
    }
}

/// Exact proof verdict, key digest, and an undecided claim (a fixed dummy on failure).
#[must_use = "bind the key digest and register the generator claim"]
#[derive(Clone, Debug)]
pub struct SuccinctOutput<C: PastaCurve> {
    /// Exact native-equivalent succinct verdict, including all decoding checks.
    pub valid: Bit<C::Base>,
    /// `kgwvkey1` digest for authorization by the owning relation.
    pub key_digest: Word<C::Base>,
    /// The proof claim, or `(sum g_i, [1; k])` if the verdict is zero.
    pub claim: GeneratorClaimCells<C>,
}

/// Columns for one interpreter lane. Consumers may schedule multiple proofs
/// sequentially by creating one chip and preserving its row cursors.
#[derive(Clone, Debug)]
pub struct VerifierConfig<C: PastaCurve> {
    glue: GlueConfig,
    range: RunningSumConfig,
    ff: FfConfig,
    ecc: EccConfig<C>,
    ecc_start_row: usize,
    duplex: DuplexConfig<C::Base>,
}
impl<C: PastaCurve> VerifierConfig<C> {
    /// Configures deterministic complete curve, foreign-field and transcript gates.
    /// The foreign-field range table requires a circuit with k at least 16.
    pub fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self {
        Self::configure_lanes(meta, None).0
    }
    /// Reuses two ECC columns for a fixed byte-tape prefix. The consumer must
    /// assign all byte runs inside `byte_rows` before using the ECC lane; the
    /// interpreter starts curve operations after that reserved prefix.
    ///
    /// The row budget is circuit metadata, never a witness-dependent length.
    pub fn configure_with_byte_tape(
        meta: &mut ConstraintSystem<C::Base>,
        byte_rows: usize,
    ) -> (Self, BytesConfig) {
        let (config, bytes) = Self::configure_lanes(meta, Some(byte_rows));
        (config, bytes.expect("requested byte tape is configured"))
    }
    fn configure_lanes(
        meta: &mut ConstraintSystem<C::Base>,
        byte_rows: Option<usize>,
    ) -> (Self, Option<BytesConfig>) {
        let glue_cols = core::array::from_fn(|_| meta.advice_column());
        let constant = meta.fixed_column();
        let glue = GlueConfig::configure(meta, glue_cols, constant);
        let range_col = meta.advice_column();
        let range = RunningSumConfig::configure(
            meta,
            range_col,
            LimbBits::new(15).expect("fixed limb width"),
        );
        let ff_cols = core::array::from_fn(|_| meta.advice_column());
        let ff = FfConfig::configure(meta, ff_cols, &[Arithmetic::<C>::modulus()]);
        let ecc_cols = core::array::from_fn(|_| meta.advice_column());
        let ecc = EccConfig::configure(meta, ecc_cols);
        let bytes = byte_rows.map(|_| BytesConfig::configure(meta, ecc_cols[0], ecc_cols[1]));
        let columns = Pow5Columns {
            state: core::array::from_fn(|_| meta.advice_column()),
            aux: meta.advice_column(),
        };
        let constants = RoundConstantColumns::allocate(meta);
        let duplex = DuplexConfig::configure(meta, columns, constants, &[]);
        (
            Self {
                glue,
                range,
                ff,
                ecc,
                ecc_start_row: byte_rows.unwrap_or(0),
                duplex,
            },
            bytes,
        )
    }
}

/// A stateful interpreter whose separate chips retain their row cursors.
#[derive(Debug)]
pub struct VerifierChip<C: PastaCurve> {
    pub(crate) glue: GlueChip<C::Base>,
    pub(crate) range: RunningSumChip<C::Base>,
    pub(crate) arithmetic: Arithmetic<C>,
    pub(crate) ecc: EccChip<C>,
    pub(crate) duplex: Option<DuplexChip<C::Base>>,
}

#[derive(Clone)]
struct Read<C: PastaCurve> {
    scalars: Vec<(ProofMessage, Scalar<C>)>,
    points: Vec<(ProofMessage, NonIdentityPoint<C::Base>)>,
    challenges: Vec<(Challenge, Scalar<C>)>,
    round_cells: Vec<ScalarCells<C>>,
    suffix: Option<NonIdentityPoint<C::Base>>,
}
impl<C: PastaCurve> Read<C> {
    fn scalar(&self, id: ProofMessage) -> Result<Scalar<C>, Error> {
        self.scalars
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value.clone())
            .ok_or(Error::Synthesis)
    }
    fn point(&self, id: ProofMessage) -> Result<AssignedPoint<C::Base>, Error> {
        self.points
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value.point().clone())
            .ok_or(Error::Synthesis)
    }
    fn challenge(&self, id: Challenge) -> Result<Scalar<C>, Error> {
        self.challenges
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, value)| value.clone())
            .ok_or(Error::Synthesis)
    }
}

impl<C: PastaCurve> VerifierChip<C> {
    /// Creates a lane starting at row zero.
    #[must_use]
    pub fn new(config: VerifierConfig<C>) -> Self {
        Self {
            glue: GlueChip::new(config.glue),
            range: RunningSumChip::new(config.range),
            arithmetic: Arithmetic::new(FfChip::new(config.ff)),
            ecc: EccChip::starting_at(&config.ecc, config.ecc_start_row),
            duplex: Some(DuplexChip::new(config.duplex)),
        }
    }
    /// Loads both fixed range tables.
    ///
    /// # Errors
    /// A table does not fit the configured circuit.
    pub fn load_tables(&self, layouter: &mut impl Layouter<C::Base>) -> Result<(), Error> {
        self.range.load_table(layouter)?;
        self.arithmetic.ff.load_table(layouter)
    }
    /// Exposes shared integer assignment for byte-linked caller inputs.
    pub fn uint(&mut self) -> UintChip<'_, C::Base> {
        UintChip::new(&mut self.glue, &mut self.range)
    }
    /// Assigns a witness key with finite curve commitments. The consuming
    /// relation must authorize the digest returned by [`Self::verify`].
    ///
    /// # Errors
    /// Layout failure. An identity commitment makes the circuit unsatisfied;
    /// descriptor-sized commitment counts are checked by [`Self::verify`].
    pub fn witness_key(
        &mut self,
        region: &mut Region<'_, C::Base>,
        representation: Value<C::Base>,
        fixed: &[Value<C>],
        permutation: &[Value<C>],
    ) -> Result<VerifierKeyCells<C>, Error> {
        let representation = self.glue.witness(region, representation)?;
        let mut points = |values: &[Value<C>]| {
            values
                .iter()
                .map(|value| self.ecc.witness_non_identity(region, *value))
                .collect::<Result<Vec<_>, Error>>()
        };
        Ok(VerifierKeyCells {
            representation,
            fixed: points(fixed)?,
            permutation: points(permutation)?,
        })
    }
    /// Assigns a finite witness point on this lane, for a corrected claim.
    ///
    /// # Errors
    /// Layout failure. Identity makes the circuit unsatisfied.
    pub fn witness_point(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: Value<C>,
    ) -> Result<NonIdentityPoint<C::Base>, Error> {
        self.ecc.witness_non_identity(region, value)
    }
    /// Pins a finite circuit constant on this lane, including explicit fillers.
    ///
    /// # Errors
    /// Layout failure. Identity makes the circuit unsatisfied.
    pub fn constant_point(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: &C,
    ) -> Result<NonIdentityPoint<C::Base>, Error> {
        let point = self.ecc.constant_point(region, value)?;
        EccChip::<C>::assert_non_identity(&mut self.glue, region, &point)
    }
    /// Checks caller-owned native coordinates as a finite curve point.
    ///
    /// # Errors
    /// Layout failure. Identity or off-curve coordinates are unsatisfiable.
    pub fn constrain_point(
        &mut self,
        region: &mut Region<'_, C::Base>,
        x: &Word<C::Base>,
        y: &Word<C::Base>,
    ) -> Result<NonIdentityPoint<C::Base>, Error> {
        self.ecc.constrain_non_identity(region, x, y)
    }
    /// Assigns a circuit-fixed key, checking its descriptor binding first.
    ///
    /// # Errors
    /// The key differs from this plan or a point is invalid.
    pub fn constant_key(
        &mut self,
        region: &mut Region<'_, C::Base>,
        plan: &VerifierPlan<C>,
        key: &VerifyingKey<C>,
    ) -> Result<VerifierKeyCells<C>, Error> {
        if key.descriptor_digest() != plan.binding.digest() {
            return Err(Error::Synthesis);
        }
        let TranscriptRepr::Base(repr) = *key.transcript_repr() else {
            return Err(Error::Synthesis);
        };
        let representation = self.glue.constant(region, repr)?;
        let mut points =
            |values: &[C::AffineExt]| -> Result<Vec<NonIdentityPoint<C::Base>>, Error> {
                values
                    .iter()
                    .map(|value| {
                        let point = self.ecc.constant_point(region, &value.to_curve())?;
                        EccChip::<C>::assert_non_identity(&mut self.glue, region, &point)
                    })
                    .collect()
            };
        Ok(VerifierKeyCells {
            representation,
            fixed: points(key.fixed_commitments())?,
            permutation: points(key.permutation_commitments())?,
        })
    }
    pub(crate) fn constant(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: C::ScalarExt,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.constant(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            value,
        )
    }
    pub(crate) fn import(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: &ScalarCells<C>,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.import(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            value,
        )
    }
    pub(crate) fn export(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: &Scalar<C>,
    ) -> Result<ScalarCells<C>, Error> {
        self.arithmetic.export(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            value,
        )
    }
    pub(crate) fn add(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.add(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
            b,
        )
    }
    pub(crate) fn sub(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.sub(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
            b,
        )
    }
    pub(crate) fn neg(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.neg(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
        )
    }
    pub(crate) fn mul(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        b: &Scalar<C>,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.mul(region, a, b)
    }
    pub(crate) fn pow(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        exponent: u64,
    ) -> Result<Scalar<C>, Error> {
        self.arithmetic.pow(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
            exponent,
        )
    }
    pub(crate) fn nonzero(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
    ) -> Result<Bit<C::Base>, Error> {
        self.arithmetic.nonzero(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
        )
    }
    pub(crate) fn combine(
        &mut self,
        region: &mut Region<'_, C::Base>,
        valid: &mut Bit<C::Base>,
        check: &Bit<C::Base>,
    ) -> Result<(), Error> {
        *valid = self.glue.and(region, valid, check)?;
        Ok(())
    }
    pub(crate) fn inverse(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &Scalar<C>,
        valid: &mut Bit<C::Base>,
    ) -> Result<Scalar<C>, Error> {
        let (inverse, nonzero) = self.arithmetic.inverse(
            &mut UintChip::new(&mut self.glue, &mut self.range),
            region,
            a,
        )?;
        self.combine(region, valid, &nonzero)?;
        Ok(inverse)
    }
    pub(crate) fn scale_point(
        &mut self,
        region: &mut Region<'_, C::Base>,
        point: &AssignedPoint<C::Base>,
        scalar: &Scalar<C>,
    ) -> Result<AssignedPoint<C::Base>, Error> {
        let scalar = self.export(region, scalar)?;
        self.ecc
            .mul(
                region,
                &mut self.range,
                ScalarLimbs::new(scalar.lo(), scalar.hi()),
                point,
            )
            .map(|(point, _)| point)
    }
}

impl<C: PastaCurve> VerifierChip<C> {
    /// Checks one fixed-size proof buffer and its constrained LE32 byte length.
    /// The caller must link these elements and `length` to the same payload.
    ///
    /// # Errors
    /// Returns layout errors or a structural mismatch in circuit metadata/key
    /// arrays. Arbitrary proof-message bytes are handled by constrained bits;
    /// hard mode makes rejected proofs unsatisfied instead of skipping work.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn verify(
        &mut self,
        region: &mut Region<'_, C::Base>,
        plan: &VerifierPlan<C>,
        key: &VerifierKeyCells<C>,
        instances: &[Vec<ScalarCells<C>>],
        proof: &[LeElement<C::Base>],
        length: &Uint<C::Base, 32>,
        mode: VerificationMode,
    ) -> Result<SuccinctOutput<C>, Error> {
        let descriptor = plan.binding.descriptor();
        let types = descriptor.instance_types.as_ref().ok_or(Error::Synthesis)?;
        if proof.len().checked_mul(32) != Some(plan.proof_length())
            || key.fixed.len() != descriptor.num_fixed_columns as usize
            || key.permutation.len() != descriptor.permutation.len()
            || instances.len() != types.len()
            || instances
                .iter()
                .zip(&descriptor.instance_lengths)
                .any(|(values, len)| values.len() != *len as usize)
        {
            return Err(Error::Synthesis);
        }
        let expected_len = self.glue.constant(
            region,
            C::Base::from(u64::try_from(plan.proof_length()).map_err(|_| Error::BoundsFailure)?),
        )?;
        let mut valid = self.glue.is_equal(region, length.word(), &expected_len)?;
        let mut instance_values = Vec::with_capacity(instances.len());
        for (column, ty) in instances.iter().zip(types) {
            let mut values = Vec::with_capacity(column.len());
            for value in column {
                let member = value.instance_type(&mut self.uint(), region, *ty)?;
                self.combine(region, &mut valid, &member)?;
                values.push(self.import(region, value)?);
            }
            instance_values.push(values);
        }
        let mut duplex = self.duplex.take().ok_or(Error::Synthesis)?;
        duplex.absorb_constant(C::Base::from(u64::from_le_bytes(*b"kgwvkey1")));
        let arity = 8 + 2 * (key.fixed.len() + key.permutation.len());
        duplex.absorb_constant(C::Base::from(
            u64::try_from(arity).map_err(|_| Error::BoundsFailure)?,
        ));
        for value in [
            1,
            match descriptor.curve {
                iroha_plonk::cs::CurveV1::Pallas => 0,
                iroha_plonk::cs::CurveV1::Vesta => 1,
            },
            u64::from(descriptor.k),
            u64::from(descriptor.num_fixed_columns),
            u64::try_from(key.permutation.len()).map_err(|_| Error::BoundsFailure)?,
        ] {
            duplex.absorb_constant(C::Base::from(value));
        }
        duplex.absorb(&key.representation);
        for chunk in plan.binding.digest().chunks_exact(16) {
            duplex.absorb_constant(C::Base::from_u128(u128::from_le_bytes(
                chunk.try_into().map_err(|_| Error::Synthesis)?,
            )));
        }
        for point in key.fixed.iter().chain(&key.permutation) {
            duplex.absorb(point.x());
            duplex.absorb(point.y());
        }
        let key_digest = duplex.squeeze_and_clear(region)?;
        let mut transcript = TranscriptChip::from_duplex(duplex, Domain::Proof)?;
        let mut read = Read {
            scalars: vec![],
            points: vec![],
            challenges: vec![],
            round_cells: vec![],
            suffix: None,
        };
        let mut cursor = proof.iter();
        for step in plan.protocol.transcript_schedule() {
            match *step {
                TranscriptStep::CommonBase(input) => match input {
                    CommonInput::TranscriptRepr => transcript.common_word(&key.representation),
                    CommonInput::FrameTag => {
                        transcript.common_constant(C::Base::from(u64::from_le_bytes(*b"pipainst")))
                    }
                    CommonInput::FrameColumns => {
                        transcript.common_constant(C::Base::from(types.len() as u64))
                    }
                    CommonInput::FrameLength { column } => transcript.common_constant(
                        C::Base::from(u64::from(descriptor.instance_lengths[column])),
                    ),
                    CommonInput::FrameType { column } => {
                        transcript.common_constant(C::Base::from(types[column].code()))
                    }
                    _ => return Err(Error::Synthesis),
                },
                TranscriptStep::Common(CommonInput::InstanceValue { column, row }) => {
                    transcript.common_scalar::<C>(&instances[column][row])
                }
                TranscriptStep::Common(_) => return Err(Error::Synthesis),
                TranscriptStep::Message(message) if message.is_point() => {
                    let decoded = decode_point_soft::<C>(
                        &mut UintChip::new(&mut self.glue, &mut self.range),
                        &mut self.ecc,
                        region,
                        cursor.next().ok_or(Error::Synthesis)?,
                    )?;
                    self.combine(region, &mut valid, &decoded.valid)?;
                    transcript.common_point(&decoded.value);
                    read.points.push((message, decoded.value));
                }
                TranscriptStep::Message(message) => {
                    let decoded = decode_scalar_soft::<C>(
                        &mut self.uint(),
                        region,
                        cursor.next().ok_or(Error::Synthesis)?,
                    )?;
                    self.combine(region, &mut valid, &decoded.valid)?;
                    transcript.common_scalar::<C>(&decoded.value);
                    read.scalars
                        .push((message, self.import(region, &decoded.value)?));
                }
                TranscriptStep::Squeeze(challenge) => {
                    let cells = transcript.squeeze_scalar::<C>(&mut self.uint(), region)?;
                    let scalar = self.import(region, &cells)?;
                    if matches!(challenge, Challenge::Round(_)) {
                        read.round_cells.push(cells);
                    }
                    read.challenges.push((challenge, scalar));
                }
                TranscriptStep::Suffix => {
                    let decoded = decode_point_soft::<C>(
                        &mut UintChip::new(&mut self.glue, &mut self.range),
                        &mut self.ecc,
                        region,
                        cursor.next().ok_or(Error::Synthesis)?,
                    )?;
                    self.combine(region, &mut valid, &decoded.valid)?;
                    read.suffix = Some(decoded.value);
                }
            }
        }
        if cursor.next().is_some() {
            return Err(Error::Synthesis);
        }
        self.duplex = Some(transcript.into_duplex());
        let (evaluations, xn) =
            self.constraint_evaluations(region, plan, &read, &instance_values, &mut valid)?;
        let equation =
            self.opening_equation(region, plan, key, &read, &evaluations, &xn, &mut valid)?;
        self.combine(region, &mut valid, &equation)?;
        if mode == VerificationMode::Hard {
            GlueChip::assert_constant(region, valid.word(), C::Base::ONE)?;
        }
        let suffix = read.suffix.ok_or(Error::Synthesis)?;
        let dummy = self.ecc.constant_point(region, &plan.dummy.to_curve())?;
        let selected =
            EccChip::<C>::select(&mut self.glue, region, &valid, suffix.point(), &dummy)?;
        let g = EccChip::<C>::assert_non_identity(&mut self.glue, region, &selected)?;
        let one = self.constant(region, C::ScalarExt::ONE)?;
        let mut challenges = Vec::with_capacity(read.round_cells.len());
        for cells in read.round_cells {
            let value = self.import(region, &cells)?;
            let selected = self.arithmetic.select(
                &mut UintChip::new(&mut self.glue, &mut self.range),
                region,
                &valid,
                &value,
                &one,
            )?;
            challenges.push(self.export(region, &selected)?);
        }
        Ok(SuccinctOutput {
            valid,
            key_digest,
            claim: GeneratorClaimCells {
                k: u32::from(descriptor.k),
                g,
                challenges,
            },
        })
    }
}
