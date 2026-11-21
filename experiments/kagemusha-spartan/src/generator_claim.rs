//! Private, committed-mask restructuring of exact generator claims.
//!
//! Four ordered claims are batched before a fresh multilinear evaluation is
//! chosen. All private inputs and masks are committed through D before the
//! batching challenge. The private IPA removes its accumulated H mask exactly;
//! only its fresh (G, u) claim is exported. There is no blinding scalar in the
//! exported claim, and native verification must decide that claim with an MSM.
//!
//! Each private source is a tensor, `s(u) = s(u[..D]) (x) s(u[D..])`. Its
//! constant-time sums over the `2^D` public generator index blocks determine its
//! claimed point and every inner-product round message before round `D`, so those
//! rounds need no full-width secret MSM. The transcript and witness are unchanged.
use crate::{
    fs::{ProofEntropy, Scalar, SpikeRng, Transcript},
    ipa::{Aff, Claim, F, Gens, s_vector},
};
use ff::{Field, PrimeField};
use group::{Curve, Group};
use iroha_pasta::msm::{MemoryBudget, msm_public};
use iroha_pasta::poseidon::PoseidonField;
use iroha_pasta::{PastaAffine, PastaCurve};
use zeroize::{Zeroize, Zeroizing};

pub const SOURCE_COUNT: usize = 4;
pub const IO_START: usize = 160;
/// Leading rounds whose L/R are combined from private source block points.
const BLOCK_DEPTH: usize = 4;
const INPUT_DOMAIN: &[u8] = b"generator-restructure/private-input/v1";
const FOLD_DOMAIN: &[u8] = b"generator-restructure/fold/v1";

/// Private witness: never serialize this structure into a Payment or proof.
#[derive(Clone)]
pub struct Fold<C: PastaCurve> {
    pub sources: Vec<Claim<C>>,
    pub masks: Vec<F<C>>,
    pub d: Aff<C>,
    pub lr: Vec<(Aff<C>, Aff<C>)>,
    pub c: F<C>,
    pub output: Claim<C>,
}

impl<C: PastaCurve> Drop for Fold<C> {
    fn drop(&mut self) {
        for source in &mut self.sources {
            source.g.zeroize();
            source.us.zeroize();
        }
        self.masks.zeroize();
        self.d.zeroize();
        for (l, r) in &mut self.lr {
            l.zeroize();
            r.zeroize();
        }
        self.c.zeroize();
        self.output.g.zeroize();
        self.output.us.zeroize();
    }
}

/// Stable offsets in the cross-field, range-checked shared limb stream.
#[derive(Clone, Debug)]
pub struct Layout {
    pub k: usize,
    pub sources: usize,
    pub masks: usize,
    pub digest: usize,
    pub challenges: usize,
    pub c: usize,
    pub gamma: usize,
    pub kz: usize,
    pub cbz: usize,
    pub adjusted_c: usize,
    pub output: usize,
    pub zero: usize,
    pub len: usize,
}
impl Layout {
    pub fn new(k: usize) -> Self {
        let sources = 2;
        let masks = sources + SOURCE_COUNT * (4 + 2 * k);
        let digest = masks + 2 * (1 + 2 * k);
        let challenges = digest + 2;
        let c = challenges + 2 * k + 2;
        let gamma = c + 2;
        let kz = gamma + 2;
        let cbz = kz + 2;
        let adjusted_c = cbz + 2;
        let output = adjusted_c + 2;
        Self {
            k,
            sources,
            masks,
            digest,
            challenges,
            c,
            gamma,
            kz,
            cbz,
            adjusted_c,
            output,
            zero: output + 4 + 2 * k,
            len: output + 5 + 2 * k,
        }
    }
    pub fn source(&self, j: usize) -> usize {
        self.sources + j * (4 + 2 * self.k)
    }
    pub fn r(&self, j: usize) -> usize {
        self.challenges + 1 + j
    }
    pub fn zeta(&self) -> usize {
        self.challenges + 1 + self.k
    }
    pub fn u(&self, j: usize) -> usize {
        self.challenges + 2 + self.k + j
    }
}

#[derive(Clone)]
pub struct Trace<C: PastaCurve> {
    pub relation: [u8; 32],
    pub layout: Layout,
    pub shared: Vec<u128>,
    pub input_header: ([C::Base; 3], usize),
    pub fold_header: ([C::Base; 3], usize),
    pub digest: C::Base,
    /// mu, r[0..k], zeta, u[0..k], with the native squeeze's freshness.
    pub challenges: Vec<(Vec<bool>, bool)>,
    pub mu: F<C>,
    pub point: Vec<F<C>>,
    pub zeta: F<C>,
    pub gamma: F<C>,
    pub value: F<C>,
    pub b_final: F<C>,
    pub commitment: C,
}

impl<C: PastaCurve> Drop for Trace<C> {
    fn drop(&mut self) {
        self.shared.zeroize();
        self.digest.zeroize();
        self.challenges.zeroize();
        self.mu.zeroize();
        self.point.zeroize();
        self.zeta.zeroize();
        self.gamma.zeroize();
        self.value.zeroize();
        self.b_final.zeroize();
        self.commitment.zeroize();
    }
}

pub(crate) fn header<C: PastaCurve>(input: bool, k: usize) -> Transcript<C> {
    let mut t = Transcript::new(if input { INPUT_DOMAIN } else { FOLD_DOMAIN });
    t.bytes(b"slots", &(SOURCE_COUNT as u64).to_le_bytes());
    t.bytes(b"rounds", &(k as u64).to_le_bytes());
    // Complete the constant header; callers replay its exact pending state.
    t
}
fn limbs(bytes: [u8; 32]) -> [u128; 2] {
    [
        u128::from_le_bytes(bytes[..16].try_into().unwrap()),
        u128::from_le_bytes(bytes[16..].try_into().unwrap()),
    ]
}
fn push_point<C: PastaCurve>(out: &mut Vec<u128>, p: Aff<C>) {
    let (x, y) = Option::from(p.coordinates()).unwrap_or((C::Base::ZERO, C::Base::ZERO));
    out.extend(limbs(x.to_repr()));
    out.extend(limbs(y.to_repr()));
}
fn absorb_limbs<C: PastaCurve>(t: &mut Transcript<C>, s: F<C>) {
    for x in limbs(s.to_repr()) {
        t.absorb(C::Base::from_u128(x));
    }
}
fn input_digest<C: PastaCurve>(
    relation: &[u8; 32],
    sources: &[Claim<C>],
    masks: &[F<C>],
) -> C::Base {
    let k = sources[0].us.len();
    let mut t = header::<C>(true, k);
    for x in limbs(*relation) {
        t.absorb(C::Base::from_u128(x));
    }
    for s in sources {
        t.point(b"source", &s.g);
        for u in &s.us {
            absorb_limbs(&mut t, *u);
        }
    }
    for m in masks {
        absorb_limbs(&mut t, *m);
    }
    let _ = t.challenge_full(b"input-digest");
    t.last_word()
}
fn d_open<C: PastaCurve>(gens: &Gens<C>, digest: C::Base, rho: F<C>) -> C {
    let h = limbs(digest.to_repr());
    C::from(gens.g[0]) * F::<C>::from_u128(h[0])
        + C::from(gens.g[1]) * F::<C>::from_u128(h[1])
        + C::from(gens.h) * rho
}
fn challenge<C: PastaCurve>(
    t: &mut Transcript<C>,
    out: &mut Vec<(Vec<bool>, bool)>,
    endo: bool,
) -> F<C> {
    let fresh = !t.has_spare();
    let bits = t.challenge_bits();
    let v = if endo {
        crate::fs::endo_scalar(&bits)
    } else {
        crate::fs::raw_scalar(&bits)
    };
    out.push((bits, fresh));
    v
}
fn closed<S: Field>(us: &[S], point: &[S]) -> S {
    us.iter()
        .zip(point)
        .map(|(u, r)| S::ONE - r + *r * u)
        .product()
}
fn inner<S: Field>(a: &[S], b: &[S]) -> S {
    a.iter().zip(b).map(|(a, b)| *a * b).sum()
}
/// The private coefficients' sum over the installed generators, through the
/// constant-time secret MSM (`ipa::secret_msm`).
fn msm<C: PastaCurve>(s: &[F<C>], g: &[Aff<C>]) -> C {
    crate::ipa::secret_msm::<C>(s, g)
}

/// The exact claim decision, also required for the fresh exported claim.
pub fn decide<C: PastaCurve>(gens: &Gens<C>, claim: &Claim<C>) -> bool {
    if claim.us.len() >= usize::BITS as usize {
        return false;
    }
    let n = 1usize << claim.us.len();
    n <= gens.g.len()
        && msm_public::<C>(&s_vector(&claim.us), &gens.g[..n], MemoryBudget::DEFAULT)
            .is_ok_and(|g| g == C::from(claim.g))
}

/// A private source's block points: entry `j` is `<s(us[depth..]), G_j>` for the
/// `j`-th of the `2^depth` equal public index blocks `G_j` of the generator prefix.
type Blocks<C> = Zeroizing<Vec<Aff<C>>>;

/// Evaluate a private lineage challenge vector's tail on every public generator
/// block, one constant-time secret MSM of the same private weights per block.
fn source_blocks<C: PastaCurve>(
    gens: &Gens<C>,
    us: &[F<C>],
    depth: usize,
) -> Result<Blocks<C>, &'static str> {
    if us.is_empty() || us.len() >= usize::BITS as usize || depth > us.len() {
        return Err("generator source dimension");
    }
    let bases = gens
        .g
        .get(..1usize << us.len())
        .ok_or("generator source dimension")?;
    let weights = Zeroizing::new(s_vector(&us[depth..]));
    let mut sums = Zeroizing::new(Vec::with_capacity(1 << depth));
    for block in bases.chunks_exact(weights.len()) {
        sums.push(
            iroha_pasta::msm::msm_secret_ct_cancellable::<C>(
                &weights,
                block,
                MemoryBudget::DEFAULT,
                &iroha_pasta::msm::SharedMemoryBudget::process_default(),
                None,
            )
            .map_err(|_| "generator source resource bound")?,
        );
    }
    let mut blocks = Zeroizing::new(vec![Aff::<C>::default(); sums.len()]);
    C::batch_normalize(&sums, &mut blocks[..]);
    Ok(blocks)
}

/// The source point `<s(us), G>`: the blocks weighted by the private head tensor.
fn recombine<C: PastaCurve>(blocks: &[Aff<C>], us: &[F<C>]) -> Aff<C> {
    let head = Zeroizing::new(s_vector(&us[..blocks.len().ilog2() as usize]));
    msm::<C>(&head, blocks).to_affine()
}

fn check_private_sources<C: PastaCurve>(
    gens: &Gens<C>,
    k: usize,
    sources: &[Claim<C>],
    depth: usize,
) -> Result<Vec<Blocks<C>>, &'static str> {
    let mut checked = Vec::with_capacity(sources.len());
    for claim in sources {
        if claim.us.len() != k {
            return Err("wallet predecessor generator claim");
        }
        let blocks = source_blocks(gens, &claim.us, depth)?;
        if recombine::<C>(&blocks, &claim.us) != claim.g {
            return Err("wallet predecessor generator claim");
        }
        checked.push(blocks);
    }
    Ok(checked)
}

/// Derive an incoming source point locally from its block points.
fn corrected_source<C: PastaCurve>(
    gens: &Gens<C>,
    k: usize,
    us: &[F<C>],
    depth: usize,
    blocks: &mut Vec<Blocks<C>>,
) -> Result<Claim<C>, &'static str> {
    if us.len() != k {
        return Err("generator source dimension");
    }
    let source = source_blocks(gens, us, depth)?;
    let g = recombine::<C>(&source, us);
    blocks.push(source);
    Ok(Claim { g, us: us.to_vec() })
}

/// Construct the four sources from the public operation shape. A single-parent
/// operation repeats its two checked slots. Receive derives both incoming points
/// locally, including when its total verifier must record a Burn.
pub(crate) fn prove_predecessors<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    parents: [Claim<C>; 2],
    incoming: Option<[&[F<C>]; 2]>,
    entropy: &ProofEntropy,
    purpose: &[u8],
) -> Result<(Fold<C>, Trace<C>), &'static str>
where
    F<C>: Scalar,
{
    if !gens.g.len().is_power_of_two() || gens.g.len() < 2 {
        return Err("generator source dimension");
    }
    let k = gens.g.len().ilog2() as usize;
    let depth = BLOCK_DEPTH.min(k);
    let mut blocks = check_private_sources(gens, k, &parents, depth)?;
    let [own, inherited] = parents;
    let [peer, peer_inherited] = match incoming {
        Some([own, inherited]) => [
            corrected_source(gens, k, own, depth, &mut blocks)?,
            corrected_source(gens, k, inherited, depth, &mut blocks)?,
        ],
        None => {
            blocks.extend_from_within(..2);
            [own.clone(), inherited.clone()]
        }
    };
    prove_checked_with_rng(
        gens,
        relation,
        vec![own, inherited, peer, peer_inherited],
        &blocks,
        depth,
        &mut entropy.stream(purpose),
    )
}

/// Fresh operation entropy is mandatory at production call sites.
pub fn prove<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    entropy: &ProofEntropy,
    purpose: &[u8],
) -> Result<(Fold<C>, Trace<C>), &'static str>
where
    F<C>: Scalar,
{
    prove_with_rng(gens, relation, sources, &mut entropy.stream(purpose))
}
pub(crate) fn prove_with_rng<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    rng: &mut SpikeRng,
) -> Result<(Fold<C>, Trace<C>), &'static str>
where
    F<C>: Scalar,
{
    prove_with_rng_at(gens, relation, sources, BLOCK_DEPTH, rng)
}
fn prove_with_rng_at<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    depth: usize,
    rng: &mut SpikeRng,
) -> Result<(Fold<C>, Trace<C>), &'static str>
where
    F<C>: Scalar,
{
    if sources.len() != SOURCE_COUNT {
        return Err("generator source count");
    }
    let k = sources[0].us.len();
    if k == 0 || k >= usize::BITS as usize || (1usize << k) > gens.g.len() {
        return Err("generator source dimension");
    }
    let depth = depth.min(k);
    let blocks = check_private_sources(gens, k, &sources, depth)?;
    prove_checked_with_rng(gens, relation, sources, &blocks, depth, rng)
}

/// Add one private source's weighted tensor. A row index selects the product
/// of the high challenge bits, and a column index selects the low bits.
fn add_weighted_source<S: Field + Zeroize>(coeff: &mut [S], us: &[S], weight: S) {
    use rayon::prelude::*;
    let (high, low) = us.split_at(us.len() / 2);
    let high = Zeroizing::new(s_vector(high));
    let low = Zeroizing::new(s_vector(low));
    assert_eq!(coeff.len(), high.len() * low.len());
    coeff
        .par_chunks_mut(low.len())
        .zip(high.par_iter())
        .for_each(|(row, prefix)| {
            let scale = Zeroizing::new(weight * prefix);
            for (value, suffix) in row.iter_mut().zip(low.iter()) {
                *value += *scale * suffix;
            }
        });
}

/// The folding state before a leading round `r`, with `c` the earlier round
/// challenges. Source `i` folds to `weights[i] s(us_i[r..])`, the generators to
/// `sum_beta c^beta G_beta` over the `2^r` leading blocks, and `b` to
/// `b_scale eq(point[r..])`.
struct Leading<'a, C: PastaCurve> {
    sources: &'a [Claim<C>],
    blocks: &'a [Blocks<C>],
    weights: &'a [F<C>],
    b_scale: F<C>,
    point: &'a [F<C>],
    zeta: F<C>,
}

/// One leading round's exact L and R from the source block points, with the
/// same masks: `<coeff_lo, g_hi>` takes, per source, `weight c^beta s(tail)[delta]`
/// on the block with this round's bit set, and `<coeff_hi, g_lo>` takes the extra
/// factor `u_ir` on the block with it clear. Both inner products with `b` close
/// in `O(k)` because every folded vector is still a tensor.
fn leading_round<C: PastaCurve>(
    gens: &Gens<C>,
    state: &Leading<'_, C>,
    c: &[F<C>],
    masks: [F<C>; 2],
) -> (Aff<C>, Aff<C>) {
    let round = c.len();
    let depth = state.blocks[0].len().ilog2() as usize;
    let below = depth - round - 1;
    let prefix = Zeroizing::new(s_vector(c));
    let terms = state.sources.len() << (depth - 1);
    let mut scalars: [Zeroizing<Vec<F<C>>>; 2] =
        std::array::from_fn(|_| Zeroizing::new(Vec::with_capacity(terms + 2)));
    let mut bases: [Zeroizing<Vec<Aff<C>>>; 2] =
        std::array::from_fn(|_| Zeroizing::new(Vec::with_capacity(terms + 2)));
    let mut inner = Zeroizing::new([F::<C>::ZERO; 2]);
    let r = state.point[round];
    for ((source, blocks), weight) in state.sources.iter().zip(state.blocks).zip(state.weights) {
        let side = Zeroizing::new([*weight, *weight * source.us[round]]);
        let tail = Zeroizing::new(s_vector(&source.us[round + 1..depth]));
        let rest = Zeroizing::new(
            state.b_scale * closed(&source.us[round + 1..], &state.point[round + 1..]),
        );
        inner[0] += side[0] * r * *rest;
        inner[1] += side[1] * (F::<C>::ONE - r) * *rest;
        for (beta, c_beta) in prefix.iter().enumerate() {
            for (delta, t) in tail.iter().enumerate() {
                let low = (beta << (depth - round)) | delta;
                let scale = Zeroizing::new(*c_beta * t);
                for (j, index) in [low | (1 << below), low].into_iter().enumerate() {
                    scalars[j].push(side[j] * *scale);
                    bases[j].push(blocks[index]);
                }
            }
        }
    }
    for j in 0..2 {
        scalars[j].extend([inner[j] * state.zeta, masks[j]]);
        bases[j].extend([gens.u, gens.h]);
    }
    (
        msm::<C>(&scalars[0], &bases[0]).to_affine(),
        msm::<C>(&scalars[1], &bases[1]).to_affine(),
    )
}

fn prove_checked_with_rng<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    sources: Vec<Claim<C>>,
    blocks: &[Blocks<C>],
    depth: usize,
    rng: &mut SpikeRng,
) -> Result<(Fold<C>, Trace<C>), &'static str>
where
    F<C>: Scalar,
{
    let k = sources[0].us.len();
    let n = 1usize << k;
    let masks = (0..1 + 2 * k).map(|_| rng.scalar()).collect::<Vec<_>>();
    let digest = input_digest::<C>(&relation, &sources, &masks);
    let d = d_open(gens, digest, masks[0]).to_affine();
    let mut t = header::<C>(false, k);
    for h in limbs(relation) {
        t.absorb(C::Base::from_u128(h));
    }
    t.point(b"D", &d);
    let mut ch = vec![];
    let mu = challenge(&mut t, &mut ch, true);
    let mut commitment = C::identity();
    let mut weights = Zeroizing::new(Vec::with_capacity(sources.len()));
    let mut weight = F::<C>::ONE;
    for s in &sources {
        commitment += C::from(s.g) * weight;
        weights.push(weight);
        weight *= mu;
    }
    t.point(b"C-star", &commitment.to_affine());
    let point = (0..k)
        .map(|_| challenge(&mut t, &mut ch, false))
        .collect::<Vec<_>>();
    let zeta = challenge(&mut t, &mut ch, true);
    let mut g = Zeroizing::new(gens.g[..n].to_vec());
    let mut lr = vec![];
    let mut us = vec![];
    let mut b_scale = Zeroizing::new(F::<C>::ONE);
    for round in 0..depth {
        let state = Leading {
            sources: &sources,
            blocks,
            weights: &weights,
            b_scale: *b_scale,
            point: &point,
            zeta,
        };
        let (l, r) = leading_round(
            gens,
            &state,
            &us,
            [masks[1 + 2 * round], masks[2 + 2 * round]],
        );
        t.point(b"L", &l);
        t.point(b"R", &r);
        let challenge = challenge(&mut t, &mut ch, true);
        us.push(challenge);
        for (weight, s) in weights.iter_mut().zip(&sources) {
            *weight *= challenge + s.us[round];
        }
        *b_scale *= F::<C>::ONE - point[round] + challenge * point[round];
        let half = iroha_pasta::fold::fold_generators_vartime::<C>(&mut g, &challenge);
        g.truncate(half);
        lr.push((l, r));
    }
    // Materialize only the folded `n >> depth` vectors for the remaining rounds.
    let mut coeff = Zeroizing::new(vec![F::<C>::ZERO; n >> depth]);
    for (s, weight) in sources.iter().zip(weights.iter()) {
        add_weighted_source(&mut coeff, &s.us[depth..], *weight);
    }
    let mut b = Zeroizing::new(crate::grid::eq_table(&point[depth..]));
    for value in b.iter_mut() {
        *value *= *b_scale;
    }
    let u = C::from(gens.u) * zeta;
    for round in depth..k {
        let h = coeff.len() / 2;
        let l = (msm::<C>(&coeff[..h], &g[h..])
            + u * inner(&coeff[..h], &b[h..])
            + C::from(gens.h) * masks[1 + 2 * round])
            .to_affine();
        let r = (msm::<C>(&coeff[h..], &g[..h])
            + u * inner(&coeff[h..], &b[..h])
            + C::from(gens.h) * masks[2 + 2 * round])
            .to_affine();
        t.point(b"L", &l);
        t.point(b"R", &r);
        let challenge = challenge(&mut t, &mut ch, true);
        us.push(challenge);
        for j in 0..h {
            coeff[j] = challenge * coeff[j] + coeff[j + h];
            b[j] = b[j] + challenge * b[j + h];
        }
        // Only the installed generators and the exported output.us challenges
        // enter this fold. Private coefficients stay on the constant-time
        // secret MSM and the H masks use constant-time scalar multiplication.
        iroha_pasta::fold::fold_generators_vartime::<C>(&mut g, &challenge);
        coeff.truncate(h);
        b.truncate(h);
        g.truncate(h);
        lr.push((l, r));
    }
    let fold = Fold {
        sources,
        masks,
        d,
        lr,
        c: coeff[0],
        output: Claim { us, g: g[0] },
    };
    let trace = trace(gens, relation, &fold).expect("fresh restructuring verifies");
    Ok((fold, trace))
}

/// Reconstruct every transcript and equation. Does not decide the output MSM;
/// native callers must additionally call `decide` (or use `verify`).
pub fn trace<C: PastaCurve>(
    gens: &Gens<C>,
    relation: [u8; 32],
    fold: &Fold<C>,
) -> Option<Trace<C>> {
    let k = fold.lr.len();
    if k == 0
        || k >= usize::BITS as usize
        || (1usize << k) > gens.g.len()
        || fold.sources.len() != SOURCE_COUNT
        || fold.sources.iter().any(|s| s.us.len() != k)
        || fold.masks.len() != 1 + 2 * k
        || fold.output.us.len() != k
    {
        return None;
    }
    let digest = input_digest::<C>(&relation, &fold.sources, &fold.masks);
    if d_open(gens, digest, fold.masks[0]) != C::from(fold.d) {
        return None;
    }
    let input_header = header::<C>(true, k).snapshot();
    let fold_header = header::<C>(false, k).snapshot();
    let mut t = header::<C>(false, k);
    for h in limbs(relation) {
        t.absorb(C::Base::from_u128(h));
    }
    t.point(b"D", &fold.d);
    let mut challenges = vec![];
    let mu = challenge(&mut t, &mut challenges, true);
    let mut commitment = C::identity();
    let mut weight = F::<C>::ONE;
    for source in &fold.sources {
        commitment += C::from(source.g) * weight;
        weight *= mu;
    }
    t.point(b"C-star", &commitment.to_affine());
    let point = (0..k)
        .map(|_| challenge(&mut t, &mut challenges, false))
        .collect::<Vec<_>>();
    let zeta = challenge(&mut t, &mut challenges, true);
    let mut value = F::<C>::ZERO;
    weight = F::<C>::ONE;
    for source in &fold.sources {
        value += weight * closed(&source.us, &point);
        weight *= mu;
    }
    let mut folded = commitment + C::from(gens.u) * (value * zeta);
    let mut gamma = F::<C>::ZERO;
    let mut us = vec![];
    for (i, (l, r)) in fold.lr.iter().enumerate() {
        t.point(b"L", l);
        t.point(b"R", r);
        let u = challenge(&mut t, &mut challenges, true);
        us.push(u);
        folded = folded * u + C::from(*l) * u.square() + C::from(*r);
        gamma = gamma * u + fold.masks[1 + 2 * i] * u.square() + fold.masks[2 + 2 * i];
    }
    if us != fold.output.us {
        return None;
    }
    let b_final = closed(&us, &point);
    if folded
        != C::from(fold.output.g) * fold.c
            + C::from(gens.u) * (fold.c * b_final * zeta)
            + C::from(gens.h) * gamma
    {
        return None;
    }
    let layout = Layout::new(k);
    let mut shared = limbs(relation).to_vec();
    for source in &fold.sources {
        push_point::<C>(&mut shared, source.g);
        for u in &source.us {
            shared.extend(limbs(u.to_repr()));
        }
    }
    for mask in &fold.masks {
        shared.extend(limbs(mask.to_repr()));
    }
    shared.extend(limbs(digest.to_repr()));
    for (bits, _) in &challenges {
        shared.push(
            bits.iter()
                .enumerate()
                .fold(0, |n, (i, b)| n | ((*b as u128) << i)),
        );
    }
    let offset = F::<C>::from(2).pow_vartime([255]) + F::<C>::ONE;
    for s in [
        fold.c,
        gamma,
        value * zeta,
        fold.c * b_final * zeta,
        (fold.c - offset) * F::<C>::from(2).invert().unwrap(),
    ] {
        shared.extend(limbs(s.to_repr()));
    }
    push_point::<C>(&mut shared, fold.output.g);
    for u in &us {
        shared.extend(limbs(u.to_repr()));
    }
    shared.push(0);
    assert_eq!(shared.len(), layout.len);
    Some(Trace {
        relation,
        layout,
        shared,
        input_header,
        fold_header,
        digest,
        challenges,
        mu,
        point,
        zeta,
        gamma,
        value,
        b_final,
        commitment,
    })
}
pub fn verify<C: PastaCurve>(gens: &Gens<C>, relation: [u8; 32], fold: &Fold<C>) -> bool {
    trace(gens, relation, fold).is_some() && decide(gens, &fold.output)
}

#[path = "generator_claim_scalar.rs"]
mod scalar;
pub use scalar::{ScalarPorts, append_scalar};

#[cfg(test)]
#[path = "generator_claim_blocks_tests.rs"]
mod blocks_tests;
#[cfg(test)]
#[path = "generator_claim_tests.rs"]
mod tests;
