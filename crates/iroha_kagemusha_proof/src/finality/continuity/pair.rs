//! Shared exact evidence, native preparation and cell assignment for two sources.

use super::*;
use iroha_pasta::Fq;
use iroha_plonk::frontend::Value;
use iroha_plonk_gadgets::bytes::element::LeElement;
use iroha_plonk_recursion::{AccumulatorT, FOLD_WITNESS_BYTES, FoldConfig, create_fold};

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

pub(super) fn native_binding(endpoints: &[Fp; 6], claim: &AccumulatorT<Ep>) -> Result<Fp, Error> {
    let (x, y) = Option::<(Fp, Fp)>::from(claim.g().coordinates()).ok_or(Error::Synthesis)?;
    let mut words = endpoints.to_vec();
    words.extend([Fp::from(16), x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    Ok(hash_with_domain(SOURCE_BINDING_DOMAIN, &words))
}
pub(super) fn native_vesta(claim: &AccumulatorT<Eq>) -> Result<Vec<Fp>, Error> {
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

/// Encode the complete source frame while retaining both child Vesta claims.
pub fn frame_native(
    endpoints: [Fp; 6],
    pallas: &AccumulatorT<Ep>,
    children: [&AccumulatorT<Eq>; 2],
    vesta_params: &PinnedParams<Eq>,
    budget: iroha_pasta::msm::MemoryBudget,
) -> Result<[Fp; 69], Error> {
    let mut public = vec![native_binding(&endpoints, pallas)?, Fp::from(16)];
    public.extend(native_vesta(children[0])?);
    public.extend(native_vesta(children[1])?);
    let trivial = AccumulatorT::trivial(vesta_params, budget).map_err(|_| Error::Synthesis)?;
    let filler = native_vesta(&trivial)?;
    public.extend_from_slice(&filler);
    public.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    public.extend_from_slice(&filler[..4]);
    public.try_into().map_err(|_| Error::Synthesis)
}

/// Shared evidence for two source-qualified children; relation owners must
/// separately bind their exact endpoints before exporting a source checkpoint.
#[derive(Clone, Debug)]
pub struct SourcePairWitness {
    pub(crate) plan: SourcePairPlan,
    pub(crate) children: [SourceNodeEvidence; 2],
    pub(crate) fold: [u8; FOLD_WITNESS_BYTES],
    pub(crate) pallas: AccumulatorT<Ep>,
    pub(crate) known: bool,
}
impl SourcePairWitness {
    /// Build only the witnessless layout for importing an installed merge key.
    /// Fixed source-qualified child keys determine every verifier and length;
    /// no child proof or live operation is needed to mount original artifacts.
    /// This circuit cannot produce a proof until replaced by [`Self::prepare`].
    /// # Errors
    /// Invalid pinned filler encodings.
    pub fn for_source(plan: SourcePairPlan) -> Result<Self, Error> {
        let pallas = AccumulatorT::new(
            decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).map_err(|_| Error::Synthesis)?,
            [Fq::ONE; 16],
        )
        .map_err(|_| Error::Synthesis)?;
        let children = [
            plan.children[0].blank_evidence()?,
            plan.children[1].blank_evidence()?,
        ];
        Ok(Self {
            plan,
            children,
            fold: [0; FOLD_WITNESS_BYTES],
            pallas,
            known: false,
        })
    }

    /// Verify original child proofs, decide all child claims, and prepare the
    /// exact four-obligation fold. No native verdict becomes a circuit input.
    /// # Errors
    /// Bad endpoints, proof/key/claim mismatch, invalid fold or resource refusal.
    pub fn prepare(
        plan: SourcePairPlan,
        children: [SourceNodeEvidence; 2],
        vesta_params: &PinnedParams<Eq>,
        salt: Fp,
        fold_config: &FoldConfig,
    ) -> Result<Self, Error> {
        if vesta_params.k() != 16 {
            return Err(Error::Synthesis);
        }
        let cancellation = fold_config.cancellation.as_ref();
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let budget = fold_config.kernel_budget;
        let mut claims = Vec::with_capacity(4);
        for (source, child) in plan.children.iter().zip(&children) {
            let opening =
                source.verify_native_cancellable(child, vesta_params, budget, cancellation)?;
            claims.extend([child.pallas.as_input(), opening]);
        }
        let (fold, pallas) = create_fold(&plan.params, &claims, salt.to_repr(), fold_config)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Synthesis
                }
            })?;
        pallas
            .decide_cancellable(&plan.params, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Synthesis
                }
            })?;
        Ok(Self {
            plan,
            children,
            fold: fold.to_bytes(),
            pallas,
            known: true,
        })
    }
    pub(crate) fn frame(
        &self,
        endpoints: [Fp; 6],
        vesta_params: &PinnedParams<Eq>,
        budget: iroha_pasta::msm::MemoryBudget,
    ) -> Result<[Fp; 69], Error> {
        frame_native(
            endpoints,
            &self.pallas,
            [&self.children[0].vesta, &self.children[1].vesta],
            vesta_params,
            budget,
        )
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.children.each_ref().map(|child| child.vesta.clone())
    }
    pub(crate) fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    pub(crate) fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
    pub(crate) fn bytes<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [u8; N],
    ) -> Result<[Word<Fp>; N], Error> {
        self.words(chip, region, values.map(|v| Fp::from(u64::from(v))))
    }
    pub(crate) fn assign(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourcePairCells, Error> {
        assign_evidence(chip, region, &self.children, &self.fold, self.known)
    }
}

/// Assign original evidence; the owning relation separately authenticates it.
pub fn assign_evidence(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    children: &[SourceNodeEvidence; 2],
    fold: &[u8; FOLD_WITNESS_BYTES],
    known: bool,
) -> Result<SourcePairCells, Error> {
    EvidenceAssignment {
        children,
        fold,
        known,
    }
    .assign(chip, region)
}
struct EvidenceAssignment<'a> {
    children: &'a [SourceNodeEvidence; 2],
    fold: &'a [u8; FOLD_WITNESS_BYTES],
    known: bool,
}
impl EvidenceAssignment<'_> {
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
    fn assign(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourcePairCells, Error> {
        let mut endpoints = Vec::new();
        let mut pallas = Vec::new();
        let mut vesta = Vec::new();
        let mut proofs = Vec::new();
        for child in self.children {
            let words = chip
                .uint()
                .glue()
                .witnesses(region, &child.endpoints.map(|x| self.value(x)))?;
            endpoints.push(SourceEndpoints::from_words(
                chip,
                region,
                &words.try_into().map_err(|_| Error::Synthesis)?,
            )?);
            let point = chip.witness_point(region, self.value(Ep::from(*child.pallas.g())))?;
            let challenges = child
                .pallas
                .challenges()
                .iter()
                .map(|u| self.scalar(chip, region, *u))
                .collect::<Result<Vec<_>, _>>()?;
            pallas.push(FoldInputCells::from_normalized(
                chip,
                region,
                16,
                point,
                challenges.try_into().map_err(|_| Error::Synthesis)?,
            )?);
            let (x, y) =
                Option::<(Fq, Fq)>::from(child.vesta.g().coordinates()).ok_or(Error::Synthesis)?;
            let coordinates = [self.scalar(chip, region, x)?, self.scalar(chip, region, y)?];
            let challenges = chip
                .uint()
                .glue()
                .witnesses(region, &child.vesta.challenges().map(|x| self.value(x)))?;
            vesta.push(VestaClaimCells::constrain(
                chip,
                region,
                16,
                coordinates,
                challenges.try_into().map_err(|_| Error::Synthesis)?,
            )?);
            proofs.push(self.proof(chip, region, &child.proof)?);
        }
        let fold = self.proof(chip, region, self.fold)?;
        Ok(SourcePairCells {
            endpoints: endpoints.try_into().map_err(|_| Error::Synthesis)?,
            pallas: pallas.try_into().map_err(|_| Error::Synthesis)?,
            vesta: vesta.try_into().map_err(|_| Error::Synthesis)?,
            proofs: proofs.try_into().map_err(|_| Error::Synthesis)?,
            fold,
        })
    }
}

pub struct SourcePairCells {
    pub(crate) endpoints: [SourceEndpoints; 2],
    pallas: [FoldInputCells<Ep>; 2],
    vesta: [VestaClaimCells; 2],
    proofs: [ProofMessageCells; 2],
    pub(crate) fold: ProofMessageCells,
}
impl SourcePairCells {
    pub(crate) fn children(&self) -> [SourceChild<'_>; 2] {
        core::array::from_fn(|i| SourceChild {
            endpoints: &self.endpoints[i],
            pallas: &self.pallas[i],
            vesta: &self.vesta[i],
            proof: &self.proofs[i],
        })
    }
}
