//! Real PIPA-R proofs checked by the circuit and the independent native path.

use super::*;
use iroha_pasta::{Ep, Eq, PastaAffine, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::{Column, Instance, InstanceType},
    frontend::{Circuit, SimpleFloorPlanner, Value},
    keys::{KeygenConfigV2, keygen_pk_v2},
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::bytes::{
    element::{decode_le_element, le_message_segments},
    tape::{BytesChip, BytesConfig},
};

#[derive(Clone)]
struct Square;
impl<F: PastaField> Circuit<F> for Square {
    type Config = (GlueConfig, RunningSumConfig, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let cols = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, cols, constants);
        let col = meta.advice_column();
        let range = RunningSumConfig::configure(meta, col, LimbBits::new(3).unwrap());
        let instance = meta.instance_column(1);
        meta.enable_equality(instance);
        (glue, range, instance)
    }
    fn synthesize(
        &self,
        (glue, range, instance): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(glue);
        let mut range = RunningSumChip::new(range);
        range.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "square and range",
            |mut region| {
                let value = glue.witness(&mut region, Value::known(F::from(3)))?;
                range.range_check(&mut region, &value, 3)?;
                glue.mul(&mut region, &value, &value)
            },
        )?;
        layouter.constrain_instance(out.cell(), instance, 0)
    }
}

#[derive(Clone)]
struct Program<C: PastaCurve> {
    plan: VerifierPlan<C>,
    key: VerifyingKey<C>,
    proof: Vec<u8>,
    actual_length: u32,
    instance: C::ScalarExt,
    hard: bool,
    known: bool,
}
#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    verifier: VerifierConfig<C>,
    bytes: BytesConfig,
    output: Column<Instance>,
}
impl<C: PastaCurve> Program<C> {
    fn witness<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn expected(&self) -> Vec<C::Base> {
        let native = accumulate_generator(
            &self.plan.params,
            &self.plan.binding,
            &self.key,
            &[vec![self.instance]],
            &self.proof,
            MemoryBudget::new(0),
        );
        let valid = native.is_ok() && self.actual_length as usize == self.plan.proof_length();
        let (g, challenges) = if valid {
            let claim = native.unwrap();
            (*claim.g(), claim.challenges().to_vec())
        } else {
            (
                self.plan.dummy,
                vec![C::ScalarExt::ONE; self.plan.params.k() as usize],
            )
        };
        let (x, y) = g.coordinates().unwrap();
        let mut out = vec![
            C::Base::from(u64::from(valid)),
            self.key.kagemusha_digest(&self.plan.binding).unwrap(),
            x,
            y,
        ];
        for u in challenges {
            let limbs = u.to_canonical_limbs();
            out.extend([
                C::Base::from_u128(u128::from(limbs[0]) | (u128::from(limbs[1]) << 64)),
                C::Base::from_u128(u128::from(limbs[2]) | (u128::from(limbs[3]) << 64)),
            ]);
        }
        out
    }
}
impl<C: PastaCurve> Circuit<C::Base> for Program<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn params(&self) -> usize {
        4 + 2 * self.plan.params.k() as usize
    }
    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        Self::configure_with_params(meta, 16)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<C::Base>, outputs: usize) -> Self::Config {
        let verifier = VerifierConfig::configure(meta);
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let output = meta.instance_column(outputs);
        meta.enable_equality(output);
        Config {
            verifier,
            bytes,
            output,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::<C>::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "PIPA-R circuit verifier",
            |mut region| {
                let key = chip.constant_key(&mut region, &self.plan, &self.key)?;
                let input: Vec<_> = self.proof.iter().map(|byte| self.witness(*byte)).collect();
                let segments = vec![16; input.len() / 16];
                let run = bytes.run(
                    &mut region,
                    &input,
                    &segments,
                    &le_message_segments(0, input.len() / 32),
                )?;
                let mut proof = Vec::new();
                for index in 0..input.len() / 32 {
                    proof.push(decode_le_element(
                        &mut chip.uint(),
                        &mut region,
                        &run,
                        32 * index,
                    )?);
                }
                let scalar = self.instance.to_canonical_limbs();
                let lo = u128::from(scalar[0]) | (u128::from(scalar[1]) << 64);
                let hi = u128::from(scalar[2]) | (u128::from(scalar[3]) << 64);
                let lo = chip.uint().assign::<128>(&mut region, self.witness(lo))?;
                let hi = chip.uint().assign::<127>(&mut region, self.witness(hi))?;
                let instance =
                    ScalarCells::<C>::from_limbs(&mut chip.uint(), &mut region, &lo, &hi)?;
                let length = chip
                    .uint()
                    .assign::<32>(&mut region, self.witness(u128::from(self.actual_length)))?;
                let output = chip.verify(
                    &mut region,
                    &self.plan,
                    &key,
                    &[vec![instance]],
                    &proof,
                    &length,
                    if self.hard {
                        VerificationMode::Hard
                    } else {
                        VerificationMode::Soft
                    },
                )?;
                let mut outputs = vec![
                    output.valid.cell(),
                    output.key_digest.cell(),
                    output.claim.g().x().cell(),
                    output.claim.g().y().cell(),
                ];
                for value in output.claim.challenges() {
                    outputs.extend([value.lo().cell(), value.hi().cell()]);
                }
                Ok(outputs)
            },
        )?;
        for (index, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.output, index)?;
        }
        Ok(())
    }
}

fn example<C: PastaCurve>() -> Program<C> {
    let params = PinnedParams::<C>::derive(6).unwrap();
    let key = keygen_pk_v2(
        &params,
        &Square,
        &KeygenConfigV2::pipa_r(vec![InstanceType::Bits(4)]),
    )
    .unwrap();
    let instances = [vec![C::ScalarExt::from(9)]];
    let witness = Witness::from_circuit(&key, &Square, &instances).unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        witness,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .unwrap()
    .proof;
    let actual_length = proof.len().try_into().unwrap();
    Program {
        plan: VerifierPlan::new(key.binding().clone(), params).unwrap(),
        key: key.vk().clone(),
        proof,
        actual_length,
        instance: C::ScalarExt::from(9),
        hard: false,
        known: true,
    }
}
fn check<C: PastaCurve>(program: &Program<C>) {
    let report = check_circuit(program, 16, &[program.expected()], CheckMode::default())
        .expect("total synthesis");
    assert!(report.is_satisfied(), "{report:?}");
}
#[test]
fn actual_pipa_r_proof_matches_native_succinct_on_both_curves() {
    check(&example::<Ep>());
    check(&example::<Eq>());
}

fn corruptions<C: PastaCurve>() {
    let honest = example::<C>();
    let mut hard = honest.clone();
    hard.hard = true;
    check(&hard);
    let mut cases = Vec::new();
    for delta in [-1_i64, 1] {
        let mut malformed = honest.clone();
        malformed.actual_length =
            u32::try_from(i64::from(malformed.actual_length) + delta).unwrap();
        cases.push(malformed);
    }
    for value in [10, 16] {
        let mut wrong = honest.clone();
        wrong.instance = C::ScalarExt::from(value);
        cases.push(wrong);
    }
    let mut identity = honest.clone();
    identity.proof[..32].fill(0);
    cases.push(identity.clone());
    let mut noncanonical = honest.clone();
    noncanonical.proof[..32].fill(0xff);
    cases.push(noncanonical);
    // Every wire word participates in the same constrained relation; mutate
    // valid/canonical messages as well as decoding failures, including suffix.
    for index in (0..honest.proof.len()).step_by(32) {
        let mut forged = honest.clone();
        forged.proof[index] ^= 0x40;
        cases.push(forged);
    }
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(case.expected()[0], C::Base::ZERO, "corpus case {index}");
        check(case);
    }
    identity.hard = true;
    let report = check_circuit(&identity, 16, &[identity.expected()], CheckMode::default())
        .expect("hard still total synthesis");
    assert!(!report.is_satisfied(), "hard rejection is constrained");
    identity.hard = false;
    let mut forged_bit = identity.expected();
    forged_bit[0] = C::Base::ONE;
    assert!(
        !check_circuit(&identity, 16, &[forged_bit], CheckMode::default())
            .unwrap()
            .is_satisfied()
    );
}

#[test]
fn soft_verifier_is_total_and_exact_for_every_message_on_pallas() {
    corruptions::<Ep>();
}
#[test]
fn soft_verifier_is_total_and_exact_for_every_message_on_vesta() {
    corruptions::<Eq>();
}

#[derive(Clone)]
struct ArithmeticProgram<C: PastaCurve> {
    values: Vec<C::ScalarExt>,
    known: bool,
}
impl<C: PastaCurve> Circuit<C::Base> for ArithmeticProgram<C> {
    type Config = (VerifierConfig<C>, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn params(&self) -> usize {
        self.values.len() * 5
    }
    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        Self::configure_with_params(meta, 0)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<C::Base>, len: usize) -> Self::Config {
        let config = VerifierConfig::configure(meta);
        let output = meta.instance_column(len);
        meta.enable_equality(output);
        (config, output)
    }
    fn synthesize(
        &self,
        (config, output): Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::<C>::new(config);
        chip.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "exact arithmetic bridge and total inverse",
            |mut region| {
                let mut cells = Vec::new();
                for scalar in &self.values {
                    let words = scalar.to_canonical_limbs();
                    let lo = u128::from(words[0]) | (u128::from(words[1]) << 64);
                    let hi = u128::from(words[2]) | (u128::from(words[3]) << 64);
                    let witness = |value| {
                        if self.known {
                            Value::known(value)
                        } else {
                            Value::unknown()
                        }
                    };
                    let lo = chip.uint().assign::<128>(&mut region, witness(lo))?;
                    let hi = chip.uint().assign::<127>(&mut region, witness(hi))?;
                    let original =
                        ScalarCells::from_limbs(&mut chip.uint(), &mut region, &lo, &hi)?;
                    let imported = chip.import(&mut region, &original)?;
                    let result = chip.export(&mut region, &imported)?;
                    let (inverse, nonzero) = chip.arithmetic.inverse(
                        &mut UintChip::new(&mut chip.glue, &mut chip.range),
                        &mut region,
                        &imported,
                    )?;
                    let inverse = chip.export(&mut region, &inverse)?;
                    cells.extend([
                        result.lo().cell(),
                        result.hi().cell(),
                        inverse.lo().cell(),
                        inverse.hi().cell(),
                        nonzero.cell(),
                    ]);
                }
                Ok(cells)
            },
        )?;
        for (index, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, output, index)?;
        }
        Ok(())
    }
}
fn arithmetic<C: PastaCurve>() {
    let mut values = vec![C::ScalarExt::ZERO, C::ScalarExt::ONE, -C::ScalarExt::ONE];
    if let Some(modulus) = Option::<C::ScalarExt>::from(C::ScalarExt::from_repr(
        iroha_plonk::cs::CurveV1::Pallas.base_modulus(),
    )) {
        values.push(modulus);
    }
    let mut expected = Vec::new();
    for value in &values {
        let inverse = value.invert().unwrap_or(C::ScalarExt::ONE);
        for scalar in [*value, inverse] {
            let words = scalar.to_canonical_limbs();
            expected.extend([
                C::Base::from_u128(u128::from(words[0]) | (u128::from(words[1]) << 64)),
                C::Base::from_u128(u128::from(words[2]) | (u128::from(words[3]) << 64)),
            ]);
        }
        expected.push(C::Base::from(u64::from(!bool::from(value.is_zero()))));
    }
    let program = ArithmeticProgram::<C> {
        values,
        known: true,
    };
    let report = check_circuit(&program, 16, &[expected.clone()], CheckMode::default()).unwrap();
    assert!(report.is_satisfied(), "{report:?}");
    // A base-residue alias cannot stand in for the canonical S6 output.
    expected[0] += C::Base::ONE;
    assert!(
        !check_circuit(&program, 16, &[expected], CheckMode::default())
            .unwrap()
            .is_satisfied()
    );
}
#[test]
fn scalar_bridge_is_integer_injective_and_zero_inverse_is_total() {
    arithmetic::<Ep>();
    arithmetic::<Eq>();
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    WrongModulus,
    Equality,
    Nonzero,
    Inverse,
    ScalarBits,
    Transcript,
    Export,
}

#[derive(Clone)]
struct AliasBoundary<C: PastaCurve> {
    integer: iroha_plonk_gadgets::ff::Nat,
    boundary: Boundary,
    marker: core::marker::PhantomData<C>,
}
impl<C: PastaCurve> Circuit<C::Base> for AliasBoundary<C> {
    type Config = VerifierConfig<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        VerifierConfig::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config);
        chip.load_tables(&mut layouter)?;
        layouter.assign_region(
            || "proper foreign aliases at semantic boundaries",
            |mut region| {
                let scalar = chip.arithmetic.ff.witness(
                    &mut region,
                    Arithmetic::<C>::modulus(),
                    Value::known(self.integer.low_words()),
                )?;
                match self.boundary {
                    Boundary::WrongModulus => {
                        let native = chip.glue.constant(&mut region, C::Base::ONE)?;
                        let certificate = iroha_plonk_gadgets::ff::CanonicalS6::from_native_word(
                            &mut chip.uint(), &mut region, &native,
                        )?;
                        let _ = ScalarCells::<C>::from_canonical(&mut chip.uint(), &mut region, certificate)?;
                    }
                    Boundary::Equality => {
                        let zero = chip.constant(&mut region, C::ScalarExt::ZERO)?;
                        let _ = chip.arithmetic.equal(
                            &mut UintChip::new(&mut chip.glue, &mut chip.range),
                            &mut region,
                            &scalar,
                            &zero,
                        )?;
                    }
                    Boundary::Nonzero => {
                        let _ = chip.nonzero(&mut region, &scalar)?;
                    }
                    Boundary::Inverse => {
                        let mut valid = chip.glue.boolean(&mut region, Value::known(true))?;
                        let _ = chip.inverse(&mut region, &scalar, &mut valid)?;
                    }
                    Boundary::ScalarBits => {
                        let point = chip.ecc.constant_point(&mut region, &C::generator())?;
                        let _ = chip.scale_point(&mut region, &point, &scalar)?;
                    }
                    Boundary::Transcript => {
                        let scalar = chip.export(&mut region, &scalar)?;
                        let mut transcript = TranscriptChip::from_duplex(
                            chip.duplex.take().ok_or(Error::Synthesis)?,
                            Domain::Proof,
                        )?;
                        transcript.common_scalar(&scalar);
                        let _ = transcript.squeeze_scalar::<C>(&mut chip.uint(), &mut region)?;
                    }
                    Boundary::Export => {
                        let _ = chip.export(&mut region, &scalar)?;
                    }
                }
                Ok(())
            },
        )
    }
}
fn alias_boundaries<C: PastaCurve>() {
    use iroha_plonk_gadgets::ff::Nat;
    for boundary in [
        Boundary::Equality,
        Boundary::Nonzero,
        Boundary::Inverse,
        Boundary::ScalarBits,
        Boundary::Transcript,
        Boundary::Export,
    ] {
        for (integer, accepted) in [
            (Nat::ZERO, true),
            (Nat::ONE, true),
            (Arithmetic::<C>::modulus().nat(), false),
            (
                Arithmetic::<C>::modulus().nat().wrapping_add(&Nat::ONE),
                false,
            ),
        ] {
            let circuit = AliasBoundary::<C> {
                integer,
                boundary,
                marker: core::marker::PhantomData,
            };
            let report = check_circuit(&circuit, 16, &[], CheckMode::Strict).unwrap();
            assert_eq!(report.is_satisfied(), accepted, "{boundary:?} {integer:?}");
        }
    }
}
#[test]
fn lazy_foreign_arithmetic_canonicalizes_every_semantic_boundary() {
    alias_boundaries::<Ep>();
    alias_boundaries::<Eq>();
}
#[test]
fn scalar_certificate_rejects_a_foreign_modulus_without_rebinding() {
    fn reject<C: PastaCurve>() {
        let circuit = AliasBoundary::<C> { integer: iroha_plonk_gadgets::ff::Nat::ONE,
            boundary: Boundary::WrongModulus, marker: core::marker::PhantomData };
        assert!(check_circuit(&circuit, 16, &[], CheckMode::Strict).is_err());
    }
    reject::<Ep>(); reject::<Eq>();
}

#[derive(Clone)]
struct LazyArithmeticChain<C: PastaCurve>(core::marker::PhantomData<C>);
impl<C: PastaCurve> Circuit<C::Base> for LazyArithmeticChain<C> {
    type Config = (VerifierConfig<C>, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        let config = VerifierConfig::configure(meta);
        let public = meta.instance_column(5);
        meta.enable_equality(public);
        (config, public)
    }
    fn synthesize(
        &self,
        (config, public): Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config);
        chip.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "tracked bounds and semantic normalization",
            |mut region| {
                let mut value = chip.constant(&mut region, -C::ScalarExt::ONE)?;
                let one = chip.constant(&mut region, C::ScalarExt::ONE)?;
                // Repeated doubling reaches the envelope and forces the existing
                // structural reduction path; subtraction/negation introduce
                // noncanonical multiples of m before multiplication and export.
                for i in 0..160 {
                    value = chip.add(&mut region, &value, &value)?;
                    if i % 13 == 0 {
                        value = chip.sub(&mut region, &value, &one)?;
                        value = chip.neg(&mut region, &value)?;
                        value = chip.mul(&mut region, &value, &value)?;
                    }
                    assert!(iroha_plonk_gadgets::ff::within_envelope(&value.bounds()));
                }
                let zero = chip.sub(&mut region, &value, &value)?;
                let bit = chip.nonzero(&mut region, &zero)?;
                GlueChip::assert_constant(&mut region, bit.word(), C::Base::ZERO)?;
                let (inverse, nonzero) = chip.arithmetic.inverse(
                    &mut UintChip::new(&mut chip.glue, &mut chip.range),
                    &mut region,
                    &value,
                )?;
                let product = chip.mul(&mut region, &value, &inverse)?;
                let equal = chip.arithmetic.equal(
                    &mut UintChip::new(&mut chip.glue, &mut chip.range),
                    &mut region,
                    &product,
                    &one,
                )?;
                GlueChip::assert_constant(&mut region, equal.word(), C::Base::ONE)?;
                let value = chip.export(&mut region, &value)?;
                let inverse = chip.export(&mut region, &inverse)?;
                Ok([
                    value.lo().cell(),
                    value.hi().cell(),
                    inverse.lo().cell(),
                    inverse.hi().cell(),
                    nonzero.cell(),
                ])
            },
        )?;
        for (row, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, public, row)?;
        }
        Ok(())
    }
}
fn lazy_chain<C: PastaCurve>() {
    let mut value = -C::ScalarExt::ONE;
    for i in 0..160 {
        value = value + value;
        if i % 13 == 0 {
            value = -(value - C::ScalarExt::ONE);
            value = value.square();
        }
    }
    let inverse = Option::<C::ScalarExt>::from(value.invert()).unwrap();
    let mut expected = Vec::new();
    for scalar in [value, inverse] {
        let words = scalar.to_canonical_limbs();
        expected.extend([
            C::Base::from_u128(u128::from(words[0]) | (u128::from(words[1]) << 64)),
            C::Base::from_u128(u128::from(words[2]) | (u128::from(words[3]) << 64)),
        ]);
    }
    expected.push(C::Base::ONE);
    let circuit = LazyArithmeticChain::<C>(core::marker::PhantomData);
    let report = check_circuit(&circuit, 16, &[expected.clone()], CheckMode::Strict).unwrap();
    assert!(report.is_satisfied(), "{report:?}");
    for index in 0..expected.len() {
        let mut wrong = expected.clone();
        wrong[index] += C::Base::ONE;
        assert!(
            !check_circuit(&circuit, 16, &[wrong], CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
    }
}
#[test]
fn lazy_arithmetic_tracks_repeated_growth_and_preserves_modular_semantics() {
    lazy_chain::<Ep>();
    lazy_chain::<Eq>();
}
