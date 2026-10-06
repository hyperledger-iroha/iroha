//! Native preparation and actual circuit for a two-child source merge.

use super::*;
use iroha_pasta::Fq;
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value},
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::bytes::element::LeElement;
use iroha_plonk_recursion::{
    AccumulatorT, FOLD_WITNESS_BYTES, FoldConfig, FoldInput, create_fold, verifier::VerifierConfig,
};

/// Original source-node proof and its complete two-curve carried obligations.
/// This is untrusted evidence until its installed source key and terminal rules
/// are checked. It is never an ordinary validator or monetary authority token.
#[derive(Clone, Debug)]
pub struct SourceNodeEvidence {
    /// Exact `[program, context, start, end, before, after]` endpoint opening.
    pub endpoints: [Fp; 6],
    /// Original internal wrapper proof under the installed child key.
    pub proof: Vec<u8>,
    /// Complete Pallas claim committed by the wrapper digest.
    pub pallas: AccumulatorT<Ep>,
    /// Complete Vesta claim from the wrapper's public columns.
    pub vesta: AccumulatorT<Eq>,
}

fn native_binding(endpoints: &[Fp; 6], claim: &AccumulatorT<Ep>) -> Result<Fp, Error> {
    let (x, y) = Option::<(Fp, Fp)>::from(claim.g().coordinates()).ok_or(Error::Synthesis)?;
    let mut words = endpoints.to_vec();
    words.extend([Fp::from(16), x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    Ok(hash_with_domain(SOURCE_BINDING_DOMAIN, &words))
}
fn native_vesta(claim: &AccumulatorT<Eq>) -> Result<Vec<Fp>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(claim.g().coordinates()).ok_or(Error::Synthesis)?;
    let mut words = [x, y]
        .into_iter()
        .flat_map(|v| foreign_limbs(&v).map(Fp::from_u128))
        .collect::<Vec<_>>();
    words.extend_from_slice(claim.challenges());
    Ok(words)
}
impl SourceNodeEvidence {
    /// Exact native internal-wrapper public columns. This only encodes the
    /// supplied evidence; callers must authenticate the source key and decide it.
    /// # Errors
    /// Identity point or non-injective cross-field public value.
    pub fn instances(&self) -> Result<Vec<Vec<Fq>>, Error> {
        let digest = native_binding(&self.endpoints, &self.pallas)?;
        let cross = |v: &Fp| Option::<Fq>::from(Fq::from_repr(v.to_repr())).ok_or(Error::Synthesis);
        let (x, y) =
            Option::<(Fq, Fq)>::from(self.vesta.g().coordinates()).ok_or(Error::Synthesis)?;
        let challenges = self
            .vesta
            .challenges()
            .iter()
            .map(cross)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(vec![vec![cross(&digest)?], vec![x, y], challenges])
    }
}

/// Fixed verifier lanes and public69 source frame of an actual merge circuit.
#[derive(Clone, Debug)]
pub struct SourceMergeConfig {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}

/// Actual source merge under two circuit-fixed complete child wrapper keys.
/// Its constructor verifies and fully decides both originals before preparing
/// witnesses; the circuit independently verifies their equations and continuity.
#[derive(Clone, Debug)]
pub struct SourceMergeCircuit {
    plan: SourceMergePlan,
    children: [SourceNodeEvidence; 2],
    fold: [u8; FOLD_WITNESS_BYTES],
    pallas: AccumulatorT<Ep>,
    endpoints: [Fp; 6],
    public: [Fp; 69],
    known: bool,
}
impl SourceMergeCircuit {
    /// Verify original child proofs, decide all child claims, and prepare the
    /// exact four-obligation fold. No native verdict becomes a circuit input.
    /// # Errors
    /// Bad endpoints, proof/key/claim mismatch, invalid fold or resource refusal.
    pub fn prepare(
        plan: SourceMergePlan,
        children: [SourceNodeEvidence; 2],
        vesta_params: &PinnedParams<Eq>,
        salt: Fp,
        fold_config: &FoldConfig,
    ) -> Result<Self, Error> {
        let a = children[0].endpoints;
        let b = children[1].endpoints;
        for e in [a, b] {
            let start = to_u128(&e[2]).ok_or(Error::Synthesis)?;
            let end = to_u128(&e[3]).ok_or(Error::Synthesis)?;
            if e[0] == Fp::ZERO || start >= end || end > u128::from(u32::MAX) {
                return Err(Error::Synthesis);
            }
        }
        if a[0] != b[0] || a[1] != b[1] || a[3] != b[2] || a[5] != b[4] {
            return Err(Error::Synthesis);
        }
        let budget = fold_config.kernel_budget;
        let mut claims = Vec::with_capacity(4);
        for ((verifier, key), child) in plan.children.iter().zip(&children) {
            if child.proof.len() != verifier.proof_length() {
                return Err(Error::Synthesis);
            }
            let instances = child.instances()?;
            verify_full(
                verifier.params(),
                verifier.binding(),
                key,
                &instances,
                &child.proof,
                budget,
            )
            .map_err(|_| Error::Synthesis)?;
            child
                .pallas
                .decide(&plan.params, budget)
                .map_err(|_| Error::Synthesis)?;
            child
                .vesta
                .decide(vesta_params, budget)
                .map_err(|_| Error::Synthesis)?;
            let opening = accumulate_generator(
                verifier.params(),
                verifier.binding(),
                key,
                &instances,
                &child.proof,
                budget,
            )
            .map_err(|_| Error::Synthesis)?;
            let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
                .map_err(|_| Error::Synthesis)?;
            opening
                .decide(&plan.params, budget)
                .map_err(|_| Error::Synthesis)?;
            claims.extend([child.pallas.as_input(), opening]);
        }
        let (fold, pallas) = create_fold(&plan.params, &claims, salt.to_repr(), fold_config)
            .map_err(|_| Error::Synthesis)?;
        pallas
            .decide(&plan.params, budget)
            .map_err(|_| Error::Synthesis)?;
        let endpoints = [a[0], a[1], a[2], b[3], a[4], b[5]];
        let mut public = vec![native_binding(&endpoints, &pallas)?, Fp::from(16)];
        public.extend(native_vesta(&children[0].vesta)?);
        public.extend(native_vesta(&children[1].vesta)?);
        let trivial = AccumulatorT::trivial(vesta_params, budget).map_err(|_| Error::Synthesis)?;
        let filler = native_vesta(&trivial)?;
        public.extend_from_slice(&filler);
        public.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
        public.extend_from_slice(&filler[..4]);
        Ok(Self {
            plan,
            children,
            fold: fold.to_bytes(),
            pallas,
            endpoints,
            public: public.try_into().map_err(|_| Error::Synthesis)?,
            known: true,
        })
    }
    /// Independently computed complete public frame for proving and native checks.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact joined endpoint opening to retain after wrapping this source proof.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Complete folded Pallas claim which the wrapper digest commits.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pallas
    }
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn scalar(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Fq,
    ) -> Result<ScalarCells<Ep>, Error> {
        let [lo, hi] = foreign_limbs(&value);
        let lo = chip.uint().assign::<128>(region, self.value(lo))?;
        let hi = chip.uint().assign::<127>(region, self.value(hi))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
    }
    fn proof(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        bytes: &[u8],
    ) -> Result<ProofMessageCells, Error> {
        if bytes.is_empty() || !bytes.len().is_multiple_of(32) {
            return Err(Error::Synthesis);
        }
        let length = u32::try_from(bytes.len()).map_err(|_| Error::BoundsFailure)?;
        let length = chip
            .uint()
            .assign::<32>(region, self.value(u128::from(length)))?;
        let messages = bytes
            .chunks_exact(32)
            .map(|chunk| {
                let bytes: [u8; 32] = chunk.try_into().map_err(|_| Error::Synthesis)?;
                LeElement::assign(&mut chip.uint(), region, self.value(bytes))
            })
            .collect::<Result<Vec<_>, _>>()?;
        ProofMessageCells::from_messages(messages, length)
    }
}
impl Circuit<Fp> for SourceMergeCircuit {
    type Config = SourceMergeConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed source merge profile");
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        SourceMergeConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "source interval merge",
            |mut region| {
                let mut endpoints = Vec::new();
                let mut pallas = Vec::new();
                let mut vesta = Vec::new();
                let mut proofs = Vec::new();
                for child in &self.children {
                    let words = chip
                        .uint()
                        .glue()
                        .witnesses(&mut region, &child.endpoints.map(|x| self.value(x)))?;
                    endpoints.push(SourceEndpoints::from_words(
                        &mut chip,
                        &mut region,
                        &words.try_into().map_err(|_| Error::Synthesis)?,
                    )?);
                    let point =
                        chip.witness_point(&mut region, self.value(Ep::from(*child.pallas.g())))?;
                    let challenges = child
                        .pallas
                        .challenges()
                        .iter()
                        .map(|u| self.scalar(&mut chip, &mut region, *u))
                        .collect::<Result<Vec<_>, _>>()?;
                    pallas.push(FoldInputCells::from_normalized(
                        &mut chip,
                        &mut region,
                        16,
                        point,
                        challenges.try_into().map_err(|_| Error::Synthesis)?,
                    )?);
                    let (x, y) = Option::<(Fq, Fq)>::from(child.vesta.g().coordinates())
                        .ok_or(Error::Synthesis)?;
                    let coordinates = [
                        self.scalar(&mut chip, &mut region, x)?,
                        self.scalar(&mut chip, &mut region, y)?,
                    ];
                    let challenges = chip.uint().glue().witnesses(
                        &mut region,
                        &child.vesta.challenges().map(|x| self.value(x)),
                    )?;
                    vesta.push(VestaClaimCells::constrain(
                        &mut chip,
                        &mut region,
                        16,
                        coordinates,
                        challenges.try_into().map_err(|_| Error::Synthesis)?,
                    )?);
                    proofs.push(self.proof(&mut chip, &mut region, &child.proof)?);
                }
                let children = core::array::from_fn(|i| SourceChild {
                    endpoints: &endpoints[i],
                    pallas: &pallas[i],
                    vesta: &vesta[i],
                    proof: &proofs[i],
                });
                let fold = self.proof(&mut chip, &mut region, &self.fold)?;
                self.plan
                    .merge(&mut chip, &mut region, children, &fold)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
