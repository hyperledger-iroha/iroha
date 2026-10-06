//! Fixed ordinary CommitVote BLS source program, split into constrained leaves.
//!
//! The 1,084 fixed cursors cover canonical encoding, both input subgroup checks,
//! the exact W3f hash-to-G2, two Miller pairings and final exponentiation. Each
//! leaf commits its complete live state and immutable message/key/signature
//! context. The initial and terminal state commitments are deterministic.
//!
//! These circuits are not finality capabilities. An owner must authenticate all
//! fixed source keys, compose the entire contiguous program, decide both carried
//! IPA claims, and link the key/message to the authorized quorum and block.
mod plan;
mod state;
pub use plan::BlsLeafPlan;
pub use state::BlsStateWitness;

use super::continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native};
use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, Word,
    bls12_381::{
        curve::{G1AffineWitness, G1Value, G2AffineWitness, G2Value},
        extension::Fp12,
        field::Bls381Chip,
        hash_to_field::hash_to_field,
        native,
        pairing::miller_program::MillerPairState,
    },
    sha256::{Sha256Chip, Sha256Config},
};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};
use plan::Step;
use state::{StateCells, cells_g1, cells_g2, native_g1, native_g2};

/// Exact context domain for the ordinary native BLS key/signature and message.
pub const BLS_CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwblsc1");
/// Domain for complete state openings at a fixed program boundary.
pub const BLS_STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwblss1");

/// Raw immutable native BLS inputs. Coordinates remain untrusted witness data;
/// the start leaf binds them to the canonical nonidentity compressed encodings.
#[derive(Clone, Debug)]
pub struct BlsContextWitness {
    /// Exact ordinary Sumeragi signing preimage, without the W3f signing prefix.
    pub message: [u8; 165],
    /// Native compressed G1 aggregate public key.
    pub public_key: [u8; 48],
    /// Native compressed G2 aggregate signature.
    pub signature: [u8; 96],
    /// Untrusted affine opening of the compressed key.
    pub key_point: G1AffineWitness,
    /// Untrusted affine opening of the compressed signature.
    pub signature_point: G2AffineWitness,
}
impl BlsContextWitness {
    fn bytes(&self) -> Vec<u8> {
        self.message
            .iter()
            .chain(&self.public_key)
            .chain(&self.signature)
            .copied()
            .collect()
    }
    /// Deterministic context commitment; this does not verify the signature.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            Fp::from(BlsLeafPlan::PROGRAM_ID),
            Fp::from(165),
            Fp::from(48),
            Fp::from(96),
        ];
        for chunk in self.bytes().chunks(31) {
            words.push(chunk.iter().rev().fold(Fp::ZERO, |acc, byte| {
                acc * Fp::from(256) + Fp::from(u64::from(*byte))
            }));
        }
        hash_with_domain(BLS_CONTEXT_DOMAIN, &words)
    }
}
/// Deterministic initial or terminal state digest for a terminal source owner.
/// No witness registers are admitted at either boundary.
pub fn boundary_digest_native(context: Fp, terminal: bool) -> Fp {
    hash_with_domain(
        BLS_STATE_DOMAIN,
        &[
            Fp::from(BlsLeafPlan::PROGRAM_ID),
            context,
            Fp::from(u64::from(if terminal { BlsLeafPlan::LENGTH } else { 0 })),
            Fp::from(if terminal { 11 } else { 0 }),
        ],
    )
}
/// Recompute the exact BLS context from source-linked message, key and signature
/// bytes. Terminal owners use this to bind the complete proof to consensus cells.
/// # Errors
/// Layout errors; every input is constrained to an eight-bit byte.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    message: &[Word<Fp>; 165],
    key: &[Word<Fp>; 48],
    signature: &[Word<Fp>; 96],
) -> Result<Word<Fp>, Error> {
    let bytes = message
        .iter()
        .chain(key)
        .chain(signature)
        .collect::<Vec<_>>();
    let mut words = Vec::new();
    for value in [BlsLeafPlan::PROGRAM_ID, 165, 48, 96] {
        words.push(chip.uint().glue().constant(region, Fp::from(value))?);
    }
    for chunk in bytes.chunks(31) {
        let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
        for byte in chunk.iter().rev() {
            chip.uint().range_check::<8>(region, byte)?;
            packed = chip.uint().glue().linear(
                region,
                &[(Fp::from(256), &packed), (Fp::ONE, *byte)],
                Fp::ZERO,
            )?;
        }
        words.push(packed);
    }
    chip.hash_words(region, BLS_CONTEXT_DOMAIN, &words)
}
/// Constrain the unique initial or terminal state for a complete source proof.
/// The terminal flag is fixed by the circuit, never selected by a witness.
/// # Errors
/// Layout errors.
pub fn boundary_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    terminal: bool,
) -> Result<Word<Fp>, Error> {
    let program = chip
        .uint()
        .glue()
        .constant(region, Fp::from(BlsLeafPlan::PROGRAM_ID))?;
    let cursor = chip.uint().glue().constant(
        region,
        Fp::from(u64::from(if terminal { BlsLeafPlan::LENGTH } else { 0 })),
    )?;
    let tag = chip
        .uint()
        .glue()
        .constant(region, Fp::from(if terminal { 11 } else { 0 }))?;
    chip.hash_words(
        region,
        BLS_STATE_DOMAIN,
        &[program, context.clone(), cursor, tag],
    )
}
fn native_state(context: &BlsContextWitness, cursor: u32, state: &BlsStateWitness) -> Fp {
    let mut words = vec![
        Fp::from(BlsLeafPlan::PROGRAM_ID),
        context.digest(),
        Fp::from(u64::from(cursor)),
        Fp::from(state.tag()),
    ];
    if !matches!(state, BlsStateWitness::Empty | BlsStateWitness::Done) {
        native_g1(&mut words, &context.key_point);
        native_g2(&mut words, &context.signature_point);
        words.extend(state.words());
    }
    hash_with_domain(BLS_STATE_DOMAIN, &words)
}

/// One fixed compiled leaf. No public constructor accepts an arbitrary opcode.
#[derive(Clone, Debug)]
pub struct BlsLeafCircuit {
    plan: BlsLeafPlan,
    context: BlsContextWitness,
    before: BlsStateWitness,
    after: BlsStateWitness,
    known: bool,
}
impl BlsLeafCircuit {
    /// Construct a witness for an exact fixed cursor. Claimed output states are
    /// used only to construct public instances; synthesis computes the output
    /// from the constrained input, so a false output does not satisfy the proof.
    /// # Errors
    /// Wrong phase shape for this fixed cursor.
    pub fn new(
        plan: BlsLeafPlan,
        context: BlsContextWitness,
        before: BlsStateWitness,
        after: BlsStateWitness,
    ) -> Result<Self, Error> {
        if before.tag() != plan.before_tag() || after.tag() != plan.after_tag() {
            return Err(Error::Synthesis);
        }
        Ok(Self {
            plan,
            context,
            before,
            after,
            known: true,
        })
    }
    /// Fixed program cursor used to qualify this circuit's verification key.
    pub const fn plan(&self) -> BlsLeafPlan {
        self.plan
    }
    /// Exact six endpoint openings, for composition under a qualified key.
    pub fn endpoints(&self) -> [Fp; 6] {
        [
            Fp::from(BlsLeafPlan::PROGRAM_ID),
            self.context.digest(),
            Fp::from(u64::from(self.plan.cursor())),
            Fp::from(u64::from(self.plan.cursor()) + 1),
            native_state(&self.context, self.plan.cursor(), &self.before),
            native_state(&self.context, self.plan.cursor() + 1, &self.after),
        ]
    }
    /// Exact A69 internal wrapper frame, carrying explicit trivial leaf claims.
    /// # Errors
    /// Invalid internal pinned generator encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
}
/// Fixed source leaf layout. SHA lanes occur only in the hash-to-field leaf.
#[derive(Clone, Debug)]
pub struct BlsLeafConfig {
    verifier: VerifierConfig<Ep>,
    sha: Option<Sha256Config>,
    public: Column<Instance>,
    plan: BlsLeafPlan,
}
impl Circuit<Fp> for BlsLeafCircuit {
    type Config = BlsLeafConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BlsLeafPlan;
    fn without_witnesses(&self) -> Self {
        let mut out = self.clone();
        out.known = false;
        out
    }
    fn params(&self) -> Self::Params {
        self.plan
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        Self::configure_with_params(meta, BlsLeafPlan::at(0).expect("fixed first leaf"))
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, plan: Self::Params) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed three-bank source profile");
        let sha = if plan.needs_sha() {
            let advice = core::array::from_fn(|_| meta.advice_column());
            let constants = meta.fixed_column();
            Some(Sha256Config::configure(meta, advice, constants))
        } else {
            None
        };
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        BlsLeafConfig {
            verifier,
            sha,
            public,
            plan,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        if config.plan != self.plan {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let mut sha = config.sha.as_ref().map(Sha256Chip::new);
        if let Some(sha) = sha.as_mut() {
            sha.load_table(&mut layouter)?;
        }
        let frame = layouter.assign_region(
            || "fixed BLS source leaf",
            |mut region| {
                let mut bytes = Vec::with_capacity(309);
                for byte in self.context.bytes() {
                    let value = if self.known {
                        Value::known(u128::from(byte))
                    } else {
                        Value::unknown()
                    };
                    bytes.push(chip.uint().assign::<8>(&mut region, value)?.word().clone());
                }
                let context = context_digest_cells(
                    &mut chip,
                    &mut region,
                    bytes[..165].try_into().map_err(|_| Error::Synthesis)?,
                    bytes[165..213].try_into().map_err(|_| Error::Synthesis)?,
                    bytes[213..].try_into().map_err(|_| Error::Synthesis)?,
                )?;
                let key = chip.bls381().assign_g1(
                    &mut region,
                    if self.known {
                        Value::known(self.context.key_point)
                    } else {
                        Value::unknown()
                    },
                )?;
                let signature = chip.bls381().assign_g2(
                    &mut region,
                    if self.known {
                        Value::known(self.context.signature_point)
                    } else {
                        Value::unknown()
                    },
                )?;
                let before = self
                    .before
                    .assign(&mut chip.bls381(), &mut region, self.known)?;
                let before_digest = state_digest(
                    &mut chip,
                    &mut region,
                    &context,
                    self.plan.cursor(),
                    self.plan.before_tag(),
                    &key,
                    &signature,
                    &before,
                )?;
                let after = transition(
                    &mut chip.bls381(),
                    sha.as_mut(),
                    &mut region,
                    self.plan.step(),
                    &bytes,
                    &key,
                    &signature,
                    &before,
                )?;
                let after_digest = state_digest(
                    &mut chip,
                    &mut region,
                    &context,
                    self.plan.cursor() + 1,
                    self.plan.after_tag(),
                    &key,
                    &signature,
                    &after,
                )?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(BlsLeafPlan::PROGRAM_ID),
                    &context,
                    self.plan.cursor(),
                    self.plan.cursor() + 1,
                    &before_digest,
                    &after_digest,
                )?;
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (row, word) in frame.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
fn state_digest(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: u32,
    tag: u64,
    key: &G1Value<Fp>,
    signature: &G2Value<Fp>,
    state: &StateCells,
) -> Result<Word<Fp>, Error> {
    let program = chip
        .uint()
        .glue()
        .constant(region, Fp::from(BlsLeafPlan::PROGRAM_ID))?;
    let cursor = chip
        .uint()
        .glue()
        .constant(region, Fp::from(u64::from(cursor)))?;
    let tag_word = chip.uint().glue().constant(region, Fp::from(tag))?;
    let mut words = vec![program, context.clone(), cursor, tag_word];
    if !matches!(state, StateCells::Empty | StateCells::Done) {
        cells_g1(&mut words, key);
        cells_g2(&mut words, signature);
        words.extend(state.words());
    }
    chip.hash_words(region, BLS_STATE_DOMAIN, &words)
}
const ZERO12: Fp12 = [[[native::ZERO; 2]; 3]; 2];
const ONE12: Fp12 = {
    let mut out = ZERO12;
    out[0][0][0] = native::ONE;
    out
};
#[allow(clippy::too_many_arguments)]
fn transition(
    field: &mut Bls381Chip<'_, Fp>,
    sha: Option<&mut Sha256Chip<Fp>>,
    region: &mut Region<'_, Fp>,
    step: Step,
    bytes: &[Word<Fp>],
    key: &G1Value<Fp>,
    signature: &G2Value<Fp>,
    before: &StateCells,
) -> Result<StateCells, Error> {
    Ok(match (step, before) {
        (Step::Start, StateCells::Empty) => {
            let encoded_key = field.compressed_g1(region, key)?;
            let encoded_signature = field.compressed_g2(region, signature)?;
            for (a, b) in encoded_key
                .iter()
                .chain(&encoded_signature)
                .zip(&bytes[165..])
            {
                GlueChip::assert_equal(region, a, b)?;
            }
            let identity = field.identity_g1(region)?;
            StateCells::Key([key.clone(), identity.clone(), identity])
        }
        (Step::G1(index), StateCells::Key(regs)) => {
            StateCells::Key(field.g1_subgroup_step(region, regs, index)?)
        }
        (Step::StartSignature, StateCells::Key(_)) => {
            let identity = field.identity_g2(region)?;
            StateCells::Signature(core::array::from_fn(|index| {
                if index == 0 {
                    signature.clone()
                } else {
                    identity.clone()
                }
            }))
        }
        (Step::G2(index), StateCells::Signature(regs)) => {
            StateCells::Signature(field.g2_subgroup_step(region, regs, index)?)
        }
        (Step::HashFields, StateCells::Signature(_)) => StateCells::Fields(hash_to_field(
            field,
            sha.ok_or(Error::Synthesis)?,
            region,
            &bytes[..165],
        )?),
        (Step::Swu0, StateCells::Fields(values)) => StateCells::SwuFirst {
            point: field.map_to_swu_g2(region, &values[0])?,
            second: values[1].clone(),
        },
        (Step::Iso0, StateCells::SwuFirst { point, second }) => StateCells::First {
            point: field.isogeny_to_g2(region, point)?,
            second: second.clone(),
        },
        (Step::Swu1, StateCells::First { point, second }) => StateCells::SwuSecond {
            first: point.clone(),
            point: field.map_to_swu_g2(region, second)?,
        },
        (Step::Iso1, StateCells::SwuSecond { first, point }) => {
            StateCells::Points([first.clone(), field.isogeny_to_g2(region, point)?])
        }
        (Step::StartCofactor, StateCells::Points(points)) => {
            let sum = field.add_g2(region, &points[0], &points[1])?;
            let identity = field.identity_g2(region)?;
            StateCells::Cofactor(core::array::from_fn(|index| {
                if index == 0 {
                    sum.clone()
                } else {
                    identity.clone()
                }
            }))
        }
        (Step::Cofactor(index), StateCells::Cofactor(regs)) => {
            StateCells::Cofactor(field.g2_cofactor_step(region, regs, index)?)
        }
        (Step::StartMiller, StateCells::Cofactor(regs)) => {
            let h = regs[4].clone();
            let points = [
                field.start_miller_g2(region, &h)?,
                field.start_miller_g2(region, signature)?,
            ];
            let zero = field.constant_fp2(region, [native::ZERO; 2])?;
            // Coefficient assignments are bound to explicit constant zero.
            let line = field.assign_miller_line(region, Value::known([[native::ZERO; 2]; 3]))?;
            for c in line.coefficients() {
                field.assert_equal_fp2(region, c, &zero)?;
            }
            StateCells::Miller {
                message_point: h,
                state: MillerPairState::from_parts(
                    points,
                    [line.clone(), line],
                    field.constant_fp12(region, ONE12)?,
                ),
            }
        }
        (
            Step::Miller(index),
            StateCells::Miller {
                message_point,
                state,
            },
        ) => {
            let negative_generator = negative_generator(field, region)?;
            StateCells::Miller {
                message_point: message_point.clone(),
                state: field.miller_pair_step(
                    region,
                    state,
                    &[key.clone(), negative_generator],
                    &[message_point.clone(), signature.clone()],
                    index,
                )?,
            }
        }
        (Step::StartFinal, StateCells::Miller { state, .. }) => {
            let zero = field.constant_fp12(region, ZERO12)?;
            StateCells::Final(core::array::from_fn(|index| {
                if index == 0 {
                    state.accumulator().clone()
                } else {
                    zero.clone()
                }
            }))
        }
        (Step::Final(index), StateCells::Final(regs)) => {
            StateCells::Final(field.final_exponent_step(region, regs, index)?)
        }
        (Step::Finish, StateCells::Final(regs)) => {
            let one = field.constant_fp12(region, ONE12)?;
            field.assert_equal_fp12(region, &regs[0], &one)?;
            StateCells::Done
        }
        _ => return Err(Error::Synthesis),
    })
}
fn negative_generator(
    field: &mut Bls381Chip<'_, Fp>,
    region: &mut Region<'_, Fp>,
) -> Result<G1Value<Fp>, Error> {
    let point = field.assign_g1(region, Value::known(G1_GENERATOR))?;
    let x = field.constant(region, G1_GENERATOR.x)?;
    let y = field.constant(region, G1_GENERATOR.y)?;
    field.assert_equal(region, point.x(), &x)?;
    field.assert_equal(region, point.y(), &y)?;
    field.assert_nonidentity_g1(region, &point)?;
    field.neg_g1(region, &point)
}
const G1_GENERATOR: G1AffineWitness = G1AffineWitness {
    x: [
        0xfb3af00adb22c6bb,
        0x6c55e83ff97a1aef,
        0xa14e3a3f171bac58,
        0xc3688c4f9774b905,
        0x2695638c4fa9ac0f,
        0x17f1d3a73197d794,
    ],
    y: [
        0x0caa232946c5e7e1,
        0xd03cc744a2888ae4,
        0x00db18cb2c04b3ed,
        0xfcf5e095d5d00af6,
        0xa09e30ed741d8ae4,
        0x08b3f481e3aaa0f1,
    ],
    infinity: false,
};

#[cfg(test)]
mod tests;
