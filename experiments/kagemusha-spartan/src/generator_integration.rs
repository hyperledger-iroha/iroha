//! Bind the private generator fold to exact recursive proof ports.
use super::*;
use crate::{
    generator_claim as gc,
    ipa::{Claim, F, Gens},
};
use group::{Curve, Group};
use iroha_pasta::{Ep, Eq, PastaAffine, PastaCurve};

/// One fresh private restructuring witness, shared by both proof halves.
pub struct GeneratorFolds {
    pub p: gc::Fold<Eq>,
    pub s: gc::Fold<Ep>,
    pub tp: gc::Trace<Eq>,
    pub ts: gc::Trace<Ep>,
}

fn coordinate_limbs<C: PastaCurve>(point: &crate::ipa::Aff<C>) -> [u128; 4] {
    let (x, y) = Option::from(point.coordinates()).unwrap_or((C::Base::ZERO, C::Base::ZERO));
    let x = x.to_repr();
    let y = y.to_repr();
    [
        u128::from_le_bytes(x[..16].try_into().unwrap()),
        u128::from_le_bytes(x[16..].try_into().unwrap()),
        u128::from_le_bytes(y[..16].try_into().unwrap()),
        u128::from_le_bytes(y[16..].try_into().unwrap()),
    ]
}

fn inherited_words<C: PastaCurve>(
    tr: &crate::verifier::Trace<C>,
    k: usize,
) -> Result<&[F<C>], &'static str> {
    tr.circuit
        .as_ref()
        .and_then(|c| c.io_full.get(gc::IO_START..gc::IO_START + 4 + k))
        .ok_or("missing inherited generator claim")
}

/// Decode the exact canonical inherited generator claim from authenticated IO.
pub fn inherited_generator_claim<C: PastaCurve>(
    tr: &crate::verifier::Trace<C>,
    gens: &Gens<C>,
) -> Result<Claim<C>, &'static str> {
    let k = gens.g.len().ilog2() as usize;
    let words = inherited_words(tr, k)?;
    let mut enc = [[0u8; 32]; 2];
    for i in 0..4 {
        let b = words[i].to_repr();
        if b[16..].iter().any(|b| *b != 0) {
            return Err("generator coordinate limb exceeds 128 bits");
        }
        enc[i / 2][(i % 2) * 16..(i % 2 + 1) * 16].copy_from_slice(&b[..16]);
    }
    let x =
        Option::<C::Base>::from(C::Base::from_repr(enc[0])).ok_or("noncanonical generator x")?;
    let y =
        Option::<C::Base>::from(C::Base::from_repr(enc[1])).ok_or("noncanonical generator y")?;
    let g = if x == C::Base::ZERO && y == C::Base::ZERO {
        C::identity().to_affine()
    } else {
        Option::from(crate::ipa::Aff::<C>::from_xy(x, y)).ok_or("off-curve inherited generator")?
    };
    Ok(Claim {
        g,
        us: words[4..].to_vec(),
    })
}

fn corrected<C: PastaCurve>(gens: &Gens<C>, us: Vec<F<C>>) -> Result<Claim<C>, &'static str> {
    if us.len() != gens.g.len().ilog2() as usize {
        return Err("generator claim dimension");
    }
    let g = iroha_pasta::msm::msm_secret::<C>(
        &crate::ipa::s_vector(&us),
        &gens.g,
        iroha_pasta::msm::MemoryBudget::DEFAULT,
    )
    .map_err(|_| "generator correction resource bound")?
    .to_affine();
    Ok(Claim { g, us })
}

fn sources<C: PastaCurve>(
    gens: &Gens<C>,
    proofs: &[(&crate::spartan::Proof<C>, &crate::verifier::Trace<C>)],
) -> Result<Vec<Claim<C>>, &'static str> {
    let mut out = Vec::with_capacity(4);
    for slot in 0..2 {
        let i = slot % proofs.len();
        let (proof, tr) = proofs[i];
        let own = Claim {
            g: proof.ipa.g_final,
            us: tr.us.clone(),
        };
        if i == 0 {
            let inherited = inherited_generator_claim(tr, gens)?;
            if !gc::decide(gens, &own) || !gc::decide(gens, &inherited) {
                return Err("wallet predecessor generator claim");
            }
            out.extend([own, inherited]);
        } else {
            out.push(corrected(gens, own.us)?);
            out.push(corrected(
                gens,
                inherited_words(tr, gens.g.len().ilog2() as usize)?[4..].to_vec(),
            )?);
        }
    }
    Ok(out)
}

impl GeneratorFolds {
    /// Native callers supply one fresh entropy owner for this retained operation.
    pub fn prepare(
        gens: &PairGens,
        pairs: &[TracedPair],
        entropy: &crate::fs::ProofEntropy,
    ) -> Result<Self, &'static str> {
        if !(1..=2).contains(&pairs.len()) {
            return Err("generator fold proof count");
        }
        let mut relation = [0; 32];
        for i in 0..2 {
            let r = pairs[0].inst_p.io
                [crate::bootstrap_context::CONTEXT_IO + crate::bootstrap_context::RELATION + i]
                .to_repr();
            if r[16..].iter().any(|b| *b != 0) {
                return Err("relation identifier limb");
            }
            relation[16 * i..16 * (i + 1)].copy_from_slice(&r[..16]);
        }
        let ps = pairs
            .iter()
            .map(|p| (&p.proof_p, &p.tr_p))
            .collect::<Vec<_>>();
        let ss = pairs
            .iter()
            .map(|p| (&p.proof_s, &p.tr_s))
            .collect::<Vec<_>>();
        let (p, tp) = gc::prove(
            &gens.p,
            relation,
            sources(&gens.p, &ps)?,
            entropy,
            b"primary inherited generator fold",
        );
        let (s, ts) = gc::prove(
            &gens.s,
            relation,
            sources(&gens.s, &ss)?,
            entropy,
            b"secondary inherited generator fold",
        );
        Ok(Self { p, s, tp, ts })
    }
    pub(super) fn append_shared(&self, values: &mut Vec<u128>) -> (usize, usize) {
        let p = values.len();
        values.extend_from_slice(&self.tp.shared);
        let s = values.len();
        values.extend_from_slice(&self.ts.shared);
        (p, s)
    }
}

fn equal<S: Scalar>(w: &mut Wiring<S>, family: usize, a: VerifierCell, b: VerifierCell) {
    w.push(Array {
        name: "exact inherited generator source",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: vec![[
            plus(&a.linear(), &scaled(&b.linear(), -S::ONE)),
            constant(vec![S::ONE]),
            vec![],
        ]],
    });
}
pub(super) fn require_true<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    port: (usize, usize, usize),
) {
    let (a, c, i) = port;
    w.push(Array {
        name: "hard predecessor generator equality",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: vec![[
            plus(&link(a, c, vec![Some(i)], S::ONE), &constant(vec![-S::ONE])),
            constant(vec![S::ONE]),
            vec![],
        ]],
    });
}

fn expose<S: Scalar>(w: &mut Wiring<S>, family: usize, sources: &[VerifierCell]) {
    let n = sources.len().next_power_of_two();
    assert_eq!(gc::IO_START % n, 0);
    let io = w.push(Array {
        name: "inherited generator claim public IO",
        family,
        rho: n.ilog2() as usize,
        r_count: sources.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![vec![S::ZERO; n]],
    });
    w.io_arrays.push((io, gc::IO_START));
    for (i, source) in sources.iter().enumerate() {
        equal(
            w,
            family,
            *source,
            VerifierCell {
                array: io,
                cell: 0,
                instance: i,
            },
        );
        w.late_io
            .push((gc::IO_START + i, source.array, source.cell, source.instance));
    }
}

/// Bootstrap starts with the exact public claim (g[0], all-zero u).
pub(super) fn expose_empty<C: PastaCurve>(w: &mut Wiring<F<C>>, family: usize, gens: &Gens<C>)
where
    F<C>: Scalar,
{
    let mut values = coordinate_limbs::<C>(&gens.g[0])
        .map(F::<C>::from_u128)
        .to_vec();
    values.resize(4 + gens.g.len().ilog2() as usize, F::<C>::ZERO);
    let a = w.push(Array {
        name: "Bootstrap exact empty generator claim",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![255; values.len()],
        rows: values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                [
                    plus(&own(i, 1), &constant(vec![-*v])),
                    constant(vec![F::<C>::ONE]),
                    vec![],
                ]
            })
            .collect(),
        values: values.iter().map(|v| vec![*v]).collect(),
    });
    expose(
        w,
        family,
        &(0..values.len())
            .map(|cell| VerifierCell {
                array: a,
                cell,
                instance: 0,
            })
            .collect::<Vec<_>>(),
    );
}

/// Bind own IPA challenges and inherited IO; soft equality compares the exact
/// original coordinate limbs against the corrected, natively decided source.
pub(super) fn scalar_bind<C: PastaCurve>(
    w: &mut Wiring<F<C>>,
    family: usize,
    shared: usize,
    base: usize,
    fold: &gc::Fold<C>,
    tr: &gc::Trace<C>,
    scalars: &[ScalarIdx<F<C>>],
    soft: bool,
) -> Option<VerifierCell>
where
    F<C>: Scalar,
{
    let ports = gc::append_scalar(w, family, shared, base, fold, tr);
    let mut validity = None;
    for slot in 0..2 {
        let i = slot % scalars.len();
        let sc = &scalars[i];
        let leaves = (1usize << w.arrays[sc.io].rho) / 2;
        for j in 0..tr.layout.k {
            for (source, target) in [
                (
                    ports.sources[2 * slot][j],
                    VerifierCell {
                        array: sc.side + crate::scalar::S_BITS,
                        cell: crate::scalar::U,
                        instance: j * sc.q_pad,
                    },
                ),
                (
                    ports.sources[2 * slot + 1][j],
                    VerifierCell {
                        array: sc.io,
                        cell: 0,
                        instance: leaves + gc::IO_START + 4 + j,
                    },
                ),
            ] {
                equal(
                    w,
                    family,
                    VerifierCell {
                        array: source.0,
                        cell: source.1,
                        instance: source.2,
                    },
                    target,
                );
            }
        }
        let source = base + tr.layout.source(2 * slot + 1);
        let mut conjunction = None;
        for limb in 0..4 {
            let original = VerifierCell {
                array: sc.io,
                cell: 0,
                instance: leaves + gc::IO_START + limb,
            };
            let corrected = VerifierCell {
                array: shared,
                cell: 0,
                instance: source + limb,
            };
            if soft && i == 1 {
                let bit = corrected_claim_validity(w, family, original, corrected);
                conjunction = Some(match conjunction {
                    None => bit,
                    Some(prev) => and(w, family, prev, bit),
                });
            } else {
                equal(w, family, original, corrected)
            }
        }
        if conjunction.is_some() {
            validity = conjunction;
        }
    }
    let mut out = (0..4)
        .map(|i| VerifierCell {
            array: shared,
            cell: 0,
            instance: base + tr.layout.output + i,
        })
        .collect::<Vec<_>>();
    out.extend(
        ports
            .output
            .into_iter()
            .map(|(array, cell, instance)| VerifierCell {
                array,
                cell,
                instance,
            }),
    );
    expose(w, family, &out);
    validity
}

fn and<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    a: VerifierCell,
    b: VerifierCell,
) -> VerifierCell {
    let value =
        w.arrays[a.array].values[a.cell][a.instance] * w.arrays[b.array].values[b.cell][b.instance];
    let array = w.push(Array {
        name: "generator equality conjunction",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![1],
        values: vec![vec![value]],
        rows: vec![[a.linear(), b.linear(), own(0, 1)]],
    });
    VerifierCell {
        array,
        cell: 0,
        instance: 0,
    }
}

pub(super) fn bind_relation(
    w: &mut Wiring<Fp>,
    family: usize,
    shared: usize,
    starts: (usize, usize),
    sc: &ScalarIdx<Fp>,
) {
    let leaves = (1usize << w.arrays[sc.io].rho) / 2;
    for start in [starts.0, starts.1] {
        for j in 0..2 {
            equal(
                w,
                family,
                VerifierCell {
                    array: shared,
                    cell: 0,
                    instance: start + j,
                },
                VerifierCell {
                    array: sc.io,
                    cell: 0,
                    instance: leaves
                        + crate::bootstrap_context::CONTEXT_IO
                        + crate::bootstrap_context::RELATION
                        + j,
                },
            );
        }
    }
}
