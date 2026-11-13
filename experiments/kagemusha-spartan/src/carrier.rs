//! Strict, measured transport of the research pair, not a production Payment.
//!
//! The trusted external profile selects the circuits and verification keys; the
//! wire never selects a circuit. This baseline carries every ordinary `io` word,
//! including the experiment's seeded filler. Late accumulator claims are already
//! in `Proof::to_bytes`; uniform pairs have no additional IPA claims. `d` and
//! `rho` are reconstructed from the first-phase transcripts. R is transmitted.
//! Decoding validates syntax and the expected profile, not proof soundness;
//! consumers must still run both linked cryptographic verifiers against the
//! independently authenticated profile and reconstructed external inputs.
//! The compact projection checks every ordinary word against explicitly supplied
//! external inputs. Its caller must authenticate those sources. TODO: compose
//! their actual encoding with the full monetary envelope before reporting a
//! Payment size. `payment_wire` composes the complete future Send envelope.

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve};

use crate::fs::{Scalar, Transcript};
use crate::ipa::F;
use crate::r1cs::{Circuit, Instance, Shape};
use crate::spartan::Proof;

const VERSION: u8 = 1;
// A parser resource limit, deliberately above the proposed 12,000-byte Payment
// target so an oversized actual result can be measured rather than suppressed.
const MAX_RESEARCH_BYTES: usize = 65_536;
const MAX_IO_WORDS: usize = 256;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.carrier.Layout.v1")]
struct Layout {
    sx: u32,
    row_bits: u32,
    col_bits: u32,
    k: u32,
    rows: u32,
    phase1: u32,
    late: u32,
    io: u32,
}

impl Layout {
    fn new<S: Scalar>(shape: &Shape, circuit: &dyn Circuit<S>, io: usize) -> Self {
        Self {
            sx: shape.sx as u32,
            row_bits: shape.row_bits as u32,
            col_bits: shape.col_bits as u32,
            k: shape.k as u32,
            rows: (shape.rows() - circuit.fixed_rows(shape)) as u32,
            phase1: circuit.phase1_rows(shape) as u32,
            late: circuit.late_slots().len() as u32,
            io: io as u32,
        }
    }

    fn check(&self, shape: &Shape) -> Result<(), String> {
        if !(1..=5).contains(&shape.k)
            || shape.row_bits > 16
            || shape.col_bits > 24
            || shape.sx > 30
            || self.sx as usize != shape.sx
            || self.row_bits as usize != shape.row_bits
            || self.col_bits as usize != shape.col_bits
            || self.k as usize != shape.k
            || self.rows as usize > shape.rows()
            || self.phase1 > self.rows
            || self.io == 0
            || self.io as usize > MAX_IO_WORDS
            || self.late as usize > MAX_IO_WORDS
        {
            return Err("invalid trusted research layout".into());
        }
        Ok(())
    }
}

/// Trusted fixture context, supplied independently of untrusted carrier bytes.
/// `id` must identify the caller's exact authorized scheme/VK, not just geometry.
/// The measurement hook uses an in-memory run-specific fixture context, which
/// is not a production authenticated parameter distribution mechanism.
#[derive(Clone)]
pub(crate) struct TrustedProfile {
    id: [u8; 32],
    p_shape: Shape,
    s_shape: Shape,
    p: Layout,
    s: Layout,
    lean: bool,
}

impl TrustedProfile {
    pub(crate) fn id(&self) -> &[u8; 32] {
        &self.id
    }
    /// Parameters must be authenticated independently. The ID identifies the
    /// exact authorized scheme/VKs, not a choice supplied by a Payment.
    pub(crate) fn from_parameters(
        id: [u8; 32],
        p_shape: Shape,
        primary: &dyn Circuit<Fp>,
        s_shape: Shape,
        secondary: &dyn Circuit<Fq>,
        lean: bool,
    ) -> Result<Self, String> {
        let profile = Self {
            id,
            p_shape,
            s_shape,
            p: Layout::new(&p_shape, primary, 64),
            s: Layout::new(&s_shape, secondary, 62),
            lean,
        };
        profile.p.check(&p_shape)?;
        profile.s.check(&s_shape)?;
        Ok(profile)
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.carrier.Wire.v1")]
struct Wire {
    version: u8,
    profile: Vec<u8>,
    lean: bool,
    io_source: u8,
    p_layout: Layout,
    s_layout: Layout,
    r: Vec<u8>,
    p_io: Vec<u8>,
    s_io: Vec<u8>,
    p_proof: Vec<u8>,
    s_proof: Vec<u8>,
}

pub(crate) struct Decoded {
    pub(crate) ip: Instance<Fp>,
    pub(crate) is: Instance<Fq>,
    pub(crate) pp: Proof<Eq>,
    pub(crate) ps: Proof<Ep>,
    pub(crate) r: [u8; 32],
    pub(crate) d: [u8; 32],
    pub(crate) rho: [u8; 32],
}

/// Early inputs reconstructed from the separately authenticated monetary
/// envelope and credential. Construction here does not authenticate those
/// sources: the caller must verify their bindings and count their bytes.
/// Every unused slot is fixed to zero, never recovered from a fixture seed.
pub struct ProjectedInputs {
    primary: Instance<Fp>,
    secondary: Instance<Fq>,
}

impl ProjectedInputs {
    pub(crate) fn primary(&self) -> &Instance<Fp> {
        &self.primary
    }
    pub(crate) fn secondary(&self) -> &Instance<Fq> {
        &self.secondary
    }
    /// Reconstruct the agreed compact context ABI. The head and dynamic fields
    /// come from Ω public inputs; the asset and table commitment come from its
    /// verified credential; the kind comes from the selected operation.
    pub fn from_authenticated_fields(
        head: Fp,
        statement_digest: Fp,
        receipt_digest: Fp,
        context: [Fp; crate::bootstrap_context::FIELDS],
        lineage: [Fp; crate::bootstrap_context::LINEAGE_FIELDS],
        table: [Fp; 4],
        kind: usize,
    ) -> Result<Self, String> {
        if kind > crate::full::LOAD || crate::spartan::point_from_limbs::<Eq>(&table).is_none() {
            return Err("invalid projected kind or table commitment".into());
        }
        let mut p = vec![Fp::ZERO; 64];
        let mut s = vec![Fq::ZERO; 62];
        p[0] = Fp::ONE;
        s[0] = Fq::ONE;
        p[crate::full::HEAD_IO] = head;
        p[crate::full::STATEMENT_IO] = statement_digest;
        p[crate::full::RECEIPT_IO] = receipt_digest;
        let start = crate::bootstrap_context::CONTEXT_IO;
        p[start..start + context.len()].copy_from_slice(&context);
        let start = crate::bootstrap_context::LINEAGE_IO;
        p[start..start + lineage.len()].copy_from_slice(&lineage);
        p[crate::full::TABLE_IO..crate::full::TABLE_IO + 4].copy_from_slice(&table);
        let (a, b) = crate::deferral::kind_bits(kind);
        p[crate::full::KIND_IO] = Fp::from(a as u64);
        p[crate::full::KIND_IO + 1] = Fp::from(b as u64);
        s[crate::full::KIND_IO] = Fq::from(a as u64);
        s[crate::full::KIND_IO + 1] = Fq::from(b as u64);
        Ok(Self {
            primary: Instance { io: p },
            secondary: Instance { io: s },
        })
    }
}

fn io_bytes<S: Scalar>(io: &[S]) -> Vec<u8> {
    io.iter().flat_map(|x| x.to_repr()).collect()
}

fn io_decode<S: Scalar>(bytes: &[u8], count: u32) -> Result<Instance<S>, String> {
    if bytes.len() != count as usize * 32 || count as usize > MAX_IO_WORDS {
        return Err("ordinary io length".into());
    }
    let io = bytes
        .chunks_exact(32)
        .map(|b| {
            Option::<S>::from(S::from_repr(b.try_into().expect("32-byte chunk")))
                .ok_or_else(|| "noncanonical ordinary io scalar".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if io.first() != Some(&S::ONE) {
        return Err("ordinary io[0] must be one".into());
    }
    Ok(Instance { io })
}

fn encode(wire: &Wire) -> Result<Vec<u8>, String> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    norito::to_bytes(wire).map_err(|e| format!("Norito encode: {e}"))
}

fn phase1<C: PastaCurve>(
    shape: &Shape,
    layout: &Layout,
    inst: &Instance<F<C>>,
    proof: &Proof<C>,
) -> Transcript<C> {
    let mut transcript = Transcript::<C>::new(shape.label.as_bytes());
    crate::spartan::header(&mut transcript, shape, &inst.io);
    for row in &proof.rows[..layout.phase1 as usize] {
        transcript.point(b"row", row);
    }
    transcript
}

fn decode(profile: &TrustedProfile, bytes: &[u8]) -> Result<Decoded, String> {
    decode_with_projection(profile, bytes, None)
}

pub(crate) fn decode_with_projection(
    profile: &TrustedProfile,
    bytes: &[u8],
    projection: Option<&ProjectedInputs>,
) -> Result<Decoded, String> {
    profile.p.check(&profile.p_shape)?;
    profile.s.check(&profile.s_shape)?;
    if bytes.len() > MAX_RESEARCH_BYTES {
        return Err("research carrier resource limit".into());
    }
    let view = norito::core::from_bytes_view(bytes).map_err(|e| format!("Norito frame: {e}"))?;
    if view.flags() != norito::core::V1_LAYOUT_FLAGS {
        return Err("research carrier requires fixed v1 Norito layout".into());
    }
    let wire: Wire = norito::decode_from_bytes_with_limits(
        bytes,
        norito::DecodeLimits::new(
            MAX_RESEARCH_BYTES,
            MAX_RESEARCH_BYTES,
            4 * MAX_RESEARCH_BYTES,
            8 * MAX_RESEARCH_BYTES,
            16,
        ),
    )
    .map_err(|e| format!("Norito decode: {e}"))?;
    if encode(&wire)? != bytes {
        return Err("noncanonical research carrier".into());
    }
    if wire.version != VERSION
        || wire.profile != profile.id
        || wire.lean != profile.lean
        || wire.p_layout != profile.p
        || wire.s_layout != profile.s
    {
        return Err("research carrier/profile mismatch".into());
    }
    let r: [u8; 32] = wire.r.as_slice().try_into().map_err(|_| "link R length")?;
    if Option::<Fq>::from(Fq::from_repr(r)).is_none() {
        return Err("noncanonical link R".into());
    }
    let (ip, is) = match (wire.io_source, projection) {
        (0, _) => (
            io_decode::<Fp>(&wire.p_io, profile.p.io)?,
            io_decode::<Fq>(&wire.s_io, profile.s.io)?,
        ),
        (1, Some(inputs))
            if wire.p_io.is_empty()
                && wire.s_io.is_empty()
                && profile.p.io == 64
                && profile.s.io == 62 =>
        {
            (inputs.primary.clone(), inputs.secondary.clone())
        }
        _ => return Err("missing or invalid externally authenticated io projection".into()),
    };
    let pp = Proof::<Eq>::from_bytes_rows(
        &profile.p_shape,
        profile.p.rows as usize,
        profile.p.late as usize,
        &wire.p_proof,
    )
    .ok_or("primary proof encoding")?;
    let ps = Proof::<Ep>::from_bytes_rows(
        &profile.s_shape,
        profile.s.rows as usize,
        profile.s.late as usize,
        &wire.s_proof,
    )
    .ok_or("secondary proof encoding")?;
    if pp.to_bytes() != wire.p_proof || ps.to_bytes() != wire.s_proof {
        return Err("noncanonical proof encoding".into());
    }
    let mut tp = phase1(&profile.p_shape, &profile.p, &ip, &pp);
    let _ = tp.challenge_full(b"pair/d");
    let d = tp.last_word().to_repr();
    let mut ts = phase1(&profile.s_shape, &profile.s, &is, &ps);
    ts.word_limbs(b"pair/d", &d);
    let _ = ts.challenge_full(b"pair/rho");
    let rho = ts.last_word().to_repr();
    Ok(Decoded {
        ip,
        is,
        pp,
        ps,
        r,
        d,
        rho,
    })
}

fn wire_of(
    profile: &TrustedProfile,
    ip: &Instance<Fp>,
    is: &Instance<Fq>,
    pp: &Proof<Eq>,
    ps: &Proof<Ep>,
    r: [u8; 32],
) -> Wire {
    Wire {
        version: VERSION,
        profile: profile.id.to_vec(),
        lean: profile.lean,
        io_source: 0,
        p_layout: profile.p.clone(),
        s_layout: profile.s.clone(),
        r: r.to_vec(),
        p_io: io_bytes(&ip.io),
        s_io: io_bytes(&is.io),
        p_proof: pp.to_bytes(),
        s_proof: ps.to_bytes(),
    }
}

/// Only omit early inputs after every actual word matches the externally
/// reconstructed instance. This refuses current seeded filler and unbound
/// synthetic monetary fields; it does not zero them to force a size result.
fn project_wire(wire: &Wire, inputs: &ProjectedInputs) -> Result<Wire, String> {
    if wire.io_source != 0
        || wire.p_layout.io != 64
        || wire.s_layout.io != 62
        || wire.p_io != io_bytes(&inputs.primary.io)
        || wire.s_io != io_bytes(&inputs.secondary.io)
    {
        return Err("actual ordinary io does not match authenticated projection".into());
    }
    let mut out = wire.clone();
    out.io_source = 1;
    out.p_io.clear();
    out.s_io.clear();
    Ok(out)
}

/// Encode all actual proof bytes, omitting ordinary IO only after exact equality
/// with the independently reconstructed full monetary envelope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_projected_pair(
    profile: &TrustedProfile,
    ip: &Instance<Fp>,
    is: &Instance<Fq>,
    pp: &Proof<Eq>,
    ps: &Proof<Ep>,
    r: [u8; 32],
    inputs: &ProjectedInputs,
) -> Result<Vec<u8>, String> {
    // This comparison is mandatory even though the dedicated compact pair
    // never serializes the ordinary IO words.
    project_wire(&wire_of(profile, ip, is, pp, ps, r), inputs)?;
    let (bytes, _) = canonical_projected_template(profile, &pp.to_bytes(), &ps.to_bytes(), r)?;
    let decoded = decode_projected_pair(profile, &bytes, inputs)?;
    if decoded.pp.to_bytes() != pp.to_bytes() || decoded.ps.to_bytes() != ps.to_bytes() {
        return Err("projected proof roundtrip mismatch".into());
    }
    Ok(bytes)
}

const PROJECTED_MAGIC: &[u8; 8] = b"KGSPPR1\0";
pub(crate) const PROJECTED_HEADER: usize = 74;

/// Exact fixed offsets for the full raw-byte commitment relation. No search
/// through potentially repeated proof bytes is used to discover these ranges.
#[derive(Clone, Debug)]
pub(crate) struct ProjectedRanges {
    pub(crate) r: std::ops::Range<usize>,
    pub(crate) primary: std::ops::Range<usize>,
    pub(crate) secondary: std::ops::Range<usize>,
    pub(crate) total: usize,
}

fn projected_ranges(profile: &TrustedProfile) -> Result<ProjectedRanges, String> {
    profile.p.check(&profile.p_shape)?;
    profile.s.check(&profile.s_shape)?;
    let count = |shape: &Shape, layout: &Layout| {
        shape.proof_bytes() - 32 * (shape.rows() - layout.rows as usize) + 32 * layout.late as usize
    };
    let p_end = PROJECTED_HEADER + count(&profile.p_shape, &profile.p);
    let total = p_end + count(&profile.s_shape, &profile.s);
    if profile.p.io != 64 || profile.s.io != 62 || total > MAX_RESEARCH_BYTES {
        return Err("invalid compact pair trusted layout".into());
    }
    Ok(ProjectedRanges {
        r: 42..74,
        primary: 74..p_end,
        secondary: p_end..total,
        total,
    })
}

fn projected_prefix(profile: &TrustedProfile) -> Vec<u8> {
    let mut bytes = PROJECTED_MAGIC.to_vec();
    bytes.extend_from_slice(&[u8::from(profile.lean), 0]);
    bytes.extend_from_slice(&profile.id);
    bytes
}

/// Profile-bounded original bytes for the total in-circuit parser. This view
/// authenticates no points, scalars, transcript or acceptance predicate.
pub(crate) struct RawProjectedPair<'a> {
    pub(crate) r: [u8; 32],
    pub(crate) primary: &'a [u8],
    pub(crate) secondary: &'a [u8],
    pub(crate) ranges: ProjectedRanges,
}

pub(crate) fn frame_projected_pair<'a>(
    profile: &TrustedProfile,
    bytes: &'a [u8],
) -> Result<RawProjectedPair<'a>, String> {
    let ranges = projected_ranges(profile)?;
    if bytes.len() != ranges.total || bytes[..42] != projected_prefix(profile) {
        return Err("compact pair profile/header/length mismatch".into());
    }
    Ok(RawProjectedPair {
        r: bytes[ranges.r.clone()].try_into().unwrap(),
        primary: &bytes[ranges.primary.clone()],
        secondary: &bytes[ranges.secondary.clone()],
        ranges,
    })
}

/// Canonical fixed-layout frame around exact raw proof bytes. Structural byte
/// binding only: malformed points/scalars remain available to the total proof
/// validity relation and are not silently replaced. The strict decoder below
/// additionally requires canonical proof encodings.
pub(crate) fn canonical_projected_template(
    profile: &TrustedProfile,
    proof_p: &[u8],
    proof_s: &[u8],
    r: [u8; 32],
) -> Result<(Vec<u8>, ProjectedRanges), String> {
    let ranges = projected_ranges(profile)?;
    if proof_p.len() != ranges.primary.len() || proof_s.len() != ranges.secondary.len() {
        return Err("compact pair proof lengths differ from trusted profile".into());
    }
    let mut bytes = projected_prefix(profile);
    bytes.extend_from_slice(&r);
    bytes.extend_from_slice(proof_p);
    bytes.extend_from_slice(proof_s);
    debug_assert_eq!(bytes.len(), ranges.total);
    Ok((bytes, ranges))
}

/// The full future Payment admits only this fixed projected representation.
/// Raw ordinary IO Norito remains a separate research measurement format.
pub(crate) fn decode_projected_pair(
    profile: &TrustedProfile,
    bytes: &[u8],
    inputs: &ProjectedInputs,
) -> Result<Decoded, String> {
    let raw = frame_projected_pair(profile, bytes)?;
    let r = raw.r;
    if Option::<Fq>::from(Fq::from_repr(r)).is_none() {
        return Err("noncanonical link R".into());
    }
    let pp = Proof::<Eq>::from_bytes_rows(
        &profile.p_shape,
        profile.p.rows as usize,
        profile.p.late as usize,
        raw.primary,
    )
    .ok_or("compact primary proof encoding")?;
    let ps = Proof::<Ep>::from_bytes_rows(
        &profile.s_shape,
        profile.s.rows as usize,
        profile.s.late as usize,
        raw.secondary,
    )
    .ok_or("compact secondary proof encoding")?;
    if pp.to_bytes() != raw.primary || ps.to_bytes() != raw.secondary {
        return Err("noncanonical compact proof encoding".into());
    }
    Ok(projected_transcripts(profile, inputs, pp, ps, r))
}

/// Construct witnesses for the total fixed-profile parser. Canonical raw
/// proofs are retained even when their cryptographic verdict is false. If any
/// encoded point/scalar/R is malformed, use the unique all-zero proof and R.
/// No acceptance or parse-authority bit is returned: the circuit derives it
/// independently from the original bytes, which the caller must retain.
pub(crate) fn decode_projected_candidate(
    profile: &TrustedProfile,
    bytes: &[u8],
    inputs: &ProjectedInputs,
) -> Result<Decoded, String> {
    let raw = frame_projected_pair(profile, bytes)?;
    if let Ok(decoded) = decode_projected_pair(profile, bytes, inputs) {
        return Ok(decoded);
    }
    let pp = Proof::<Eq>::from_bytes_rows(
        &profile.p_shape,
        profile.p.rows as usize,
        profile.p.late as usize,
        &vec![0; raw.primary.len()],
    )
    .expect("zero points/scalars are a canonical fixed-profile proof encoding");
    let ps = Proof::<Ep>::from_bytes_rows(
        &profile.s_shape,
        profile.s.rows as usize,
        profile.s.late as usize,
        &vec![0; raw.secondary.len()],
    )
    .expect("zero points/scalars are a canonical fixed-profile proof encoding");
    Ok(projected_transcripts(profile, inputs, pp, ps, [0; 32]))
}

fn projected_transcripts(
    profile: &TrustedProfile,
    inputs: &ProjectedInputs,
    pp: Proof<Eq>,
    ps: Proof<Ep>,
    r: [u8; 32],
) -> Decoded {
    let ip = inputs.primary.clone();
    let is = inputs.secondary.clone();
    let mut tp = phase1(&profile.p_shape, &profile.p, &ip, &pp);
    let _ = tp.challenge_full(b"pair/d");
    let d = tp.last_word().to_repr();
    let mut ts = phase1(&profile.s_shape, &profile.s, &is, &ps);
    ts.word_limbs(b"pair/d", &d);
    let _ = ts.challenge_full(b"pair/rho");
    let rho = ts.last_word().to_repr();
    Decoded {
        ip,
        is,
        pp,
        ps,
        r,
        d,
        rho,
    }
}

/// Roundtrip one actual proved pair and print all encoded components. No
/// field is silently reconstructed from a stand-in seed or omitted as filler.
/// The two generated circuits are trusted inputs to this measurement, rather
/// than circuit choices read from the untrusted envelope.
#[allow(clippy::too_many_arguments)]
pub fn measure_pair(
    label: &str,
    hop: usize,
    cp: &crate::uniform::UniformCircuit<Fp>,
    ip: &Instance<Fp>,
    cs: &crate::uniform::UniformCircuit<Fq>,
    is: &Instance<Fq>,
    pair: &crate::LinkedPair,
) -> Result<usize, String> {
    measure_pair_with_projection(label, hop, cp, ip, cs, is, pair, None)
}

/// Measure a real pair, optionally checking the agreed external IO projection.
/// Projected bytes exclude the separately transported Ω public inputs and
/// credential, which must be included by the complete monetary-envelope owner.
#[allow(clippy::too_many_arguments)]
pub fn measure_pair_with_projection(
    label: &str,
    hop: usize,
    cp: &crate::uniform::UniformCircuit<Fp>,
    ip: &Instance<Fp>,
    cs: &crate::uniform::UniformCircuit<Fq>,
    is: &Instance<Fq>,
    pair: &crate::LinkedPair,
    projection: Option<&ProjectedInputs>,
) -> Result<usize, String> {
    let profile = TrustedProfile {
        id: *blake3::hash(format!("research-carrier/in-memory-fixture/{label}/{hop}").as_bytes())
            .as_bytes(),
        p_shape: cp.shape,
        s_shape: cs.shape,
        p: Layout::new(&cp.shape, cp, ip.io.len()),
        s: Layout::new(&cs.shape, cs, is.io.len()),
        lean: crate::fs::lean_schedule(),
    };
    let wire = wire_of(&profile, ip, is, &pair.pp, &pair.ps, pair.r);
    let bytes = encode(&wire)?;
    let decoded = decode(&profile, &bytes)?;
    if decoded.ip.io != ip.io
        || decoded.is.io != is.io
        || decoded.r != pair.r
        || decoded.d != pair.d
        || decoded.rho != pair.rho
        || decoded.pp.to_bytes() != pair.pp.to_bytes()
        || decoded.ps.to_bytes() != pair.ps.to_bytes()
    {
        return Err("actual pair roundtrip mismatch".into());
    }
    let proofs = wire.p_proof.len() + wire.s_proof.len();
    let ordinary = wire.p_io.len() + wire.s_io.len();
    let framing = bytes.len() - proofs - ordinary - 32;
    println!(
        "  carrier {label}/{hop}: actual Norito pair envelope {} B = proofs {} + {} + ordinary io {} + {} + R 32 + profile/layout/framing {framing}; d/rho reconstructed, late claims included, extra IPA claims 0; roundtrip PASS; full monetary envelope not included",
        bytes.len(),
        wire.p_proof.len(),
        wire.s_proof.len(),
        wire.p_io.len(),
        wire.s_io.len()
    );
    if let Some(inputs) = projection {
        let projected = encode(&project_wire(&wire, inputs)?)?;
        let restored = decode_with_projection(&profile, &projected, Some(inputs))?;
        if restored.ip.io != ip.io
            || restored.is.io != is.io
            || restored.d != pair.d
            || restored.rho != pair.rho
            || restored.r != pair.r
            || restored.pp.to_bytes() != wire.p_proof
            || restored.ps.to_bytes() != wire.s_proof
        {
            return Err("projected actual pair roundtrip mismatch".into());
        }
        println!(
            "  carrier {label}/{hop}: externally reconstructed projection {} B, saved {} B of actual ordinary io; separately authenticated Ω/credential bytes NOT included; every original IO word matched",
            projected.len(),
            bytes.len() - projected.len()
        );
        return Ok(projected.len());
    }
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use group::{Curve, Group};

    fn proof<C: PastaCurve>(shape: &Shape) -> Proof<C> {
        let point = C::generator().to_affine();
        Proof {
            rows: vec![point; shape.rows()],
            late: vec![],
            outer: vec![point; shape.outer_blocks().len()],
            v: [F::<C>::ONE; 3],
            inner: vec![point; shape.inner_blocks().len()],
            ipa: crate::ipa::IpaProof {
                lr: vec![(point, point); shape.col_bits],
                g_final: point,
                a: point,
                z1: F::<C>::ONE,
                z2: F::<C>::ONE,
            },
        }
    }

    fn fixture() -> (TrustedProfile, Wire) {
        let p_shape = Shape {
            label: "carrier-test-p",
            sx: 5,
            row_bits: 2,
            col_bits: 6,
            k: 1,
        };
        let s_shape = Shape {
            label: "carrier-test-s",
            ..p_shape
        };
        let p = Layout::new::<Fp>(&p_shape, &crate::r1cs::ToyCircuit, 3);
        let s = Layout::new::<Fq>(&s_shape, &crate::r1cs::ToyCircuit, 2);
        let profile = TrustedProfile {
            id: [7; 32],
            p_shape,
            s_shape,
            p,
            s,
            lean: false,
        };
        let wire = wire_of(
            &profile,
            &Instance {
                io: vec![Fp::ONE, Fp::from(17), Fp::from(99)],
            },
            &Instance {
                io: vec![Fq::ONE, Fq::from(83)],
            },
            &proof::<Eq>(&p_shape),
            &proof::<Ep>(&s_shape),
            Fq::from(43).to_repr(),
        );
        (profile, wire)
    }

    fn projection() -> ProjectedInputs {
        ProjectedInputs::from_authenticated_fields(
            Fp::from(101),
            Fp::from(102),
            Fp::from(103),
            [Fp::from(7); crate::bootstrap_context::FIELDS],
            [Fp::from(9); crate::bootstrap_context::LINEAGE_FIELDS],
            crate::spartan::limbs_of_point::<Eq>(&Eq::generator().to_affine()),
            crate::full::SEND,
        )
        .unwrap()
    }

    #[test]
    fn projection_roundtrip_requires_every_actual_io_word_and_external_context() {
        let (mut profile, old) = fixture();
        profile.p.io = 64;
        profile.s.io = 62;
        let inputs = projection();
        let mut wire = old;
        wire.p_layout = profile.p.clone();
        wire.s_layout = profile.s.clone();
        wire.p_io = io_bytes(&inputs.primary.io);
        wire.s_io = io_bytes(&inputs.secondary.io);
        let raw = encode(&wire).unwrap();
        let projected = project_wire(&wire, &inputs).unwrap();
        let compact = encode(&projected).unwrap();
        assert_eq!(raw.len() - compact.len(), 4032);
        assert!(
            decode(&profile, &compact).is_err(),
            "external context is mandatory"
        );
        let decoded = decode_with_projection(&profile, &compact, Some(&inputs)).unwrap();
        assert_eq!(decoded.ip.io, inputs.primary.io);
        assert_eq!(decoded.is.io, inputs.secondary.io);
        for i in 0..126 {
            let mut changed = wire.clone();
            if i < 64 {
                changed.p_io[32 * i] ^= 1;
            } else {
                changed.s_io[32 * (i - 64)] ^= 1;
            }
            assert!(
                project_wire(&changed, &inputs).is_err(),
                "ordinary word {i}"
            );
        }
        let mut duplicate = projected.clone();
        duplicate.p_io = wire.p_io.clone();
        assert!(
            decode_with_projection(&profile, &encode(&duplicate).unwrap(), Some(&inputs)).is_err()
        );
        let mut unknown = projected;
        unknown.io_source = 2;
        assert!(
            decode_with_projection(&profile, &encode(&unknown).unwrap(), Some(&inputs)).is_err()
        );
    }

    #[test]
    fn projection_rejects_invalid_table_point_and_kind() {
        assert!(
            ProjectedInputs::from_authenticated_fields(
                Fp::ONE,
                Fp::ONE,
                Fp::ONE,
                [Fp::ZERO; crate::bootstrap_context::FIELDS],
                [Fp::ZERO; crate::bootstrap_context::LINEAGE_FIELDS],
                [Fp::ZERO; 4],
                crate::full::SEND
            )
            .is_err()
        );
        assert!(
            ProjectedInputs::from_authenticated_fields(
                Fp::ONE,
                Fp::ONE,
                Fp::ONE,
                [Fp::ZERO; crate::bootstrap_context::FIELDS],
                [Fp::ZERO; crate::bootstrap_context::LINEAGE_FIELDS],
                crate::spartan::limbs_of_point::<Eq>(&Eq::generator().to_affine()),
                4
            )
            .is_err()
        );
    }

    #[test]
    fn roundtrip_preserves_every_input_and_reconstructs_link_words() {
        let (profile, wire) = fixture();
        let bytes = encode(&wire).unwrap();
        let d = decode(&profile, &bytes).unwrap();
        assert_eq!(wire_of(&profile, &d.ip, &d.is, &d.pp, &d.ps, d.r), wire);
        assert_eq!(
            d.d,
            crate::spartan::phase1_digest(&profile.p_shape, &d.ip, &d.pp)
        );
        let mut changed = wire.clone();
        changed.s_io[32] ^= 1;
        assert_ne!(
            decode(&profile, &encode(&changed).unwrap()).unwrap().rho,
            d.rho
        );
    }

    #[test]
    fn proved_pair_roundtrip_still_verifies_and_changed_r_fails() {
        let p_shape = Shape {
            label: "carrier-real-p",
            sx: 10,
            row_bits: 2,
            col_bits: 8,
            k: 1,
        };
        let s_shape = Shape {
            label: "carrier-real-s",
            ..p_shape
        };
        let gp = crate::ipa::Gens::<Eq>::new(p_shape.cols());
        let gs = crate::ipa::Gens::<Ep>::new(s_shape.cols());
        let mut rng = crate::fs::SpikeRng::new(b"carrier real roundtrip");
        let (ip, wp) = crate::r1cs::sample(&p_shape, &mut rng);
        let (is, ws) = crate::r1cs::sample(&s_shape, &mut rng);
        let mut spec = crate::verifier::PairSpec {
            shape_p: p_shape,
            gens_p: &gp,
            inst_p: ip,
            wit_p: wp,
            claims_p: vec![],
            shape_s: s_shape,
            gens_s: &gs,
            inst_s: is,
            wit_s: ws,
            claims_s: vec![],
        };
        let inputs = projection();
        spec.inst_p = inputs.primary.clone();
        spec.inst_s = inputs.secondary.clone();
        let pair = crate::verifier::prove_pair(&spec, b"carrier real pair", &|rho| {
            crate::verifier::rlc_value(rho, Fq::from(19), &[11, 29]).to_repr()
        });
        let profile = TrustedProfile {
            id: [11; 32],
            p_shape,
            s_shape,
            p: Layout::new::<Fp>(&p_shape, &crate::r1cs::ToyCircuit, spec.inst_p.io.len()),
            s: Layout::new::<Fq>(&s_shape, &crate::r1cs::ToyCircuit, spec.inst_s.io.len()),
            lean: crate::fs::lean_schedule(),
        };
        let wire = wire_of(
            &profile,
            &spec.inst_p,
            &spec.inst_s,
            &pair.p,
            &pair.s,
            pair.r,
        );
        let bytes = encode(&wire).unwrap();
        let decoded = decode(&profile, &bytes).unwrap();
        assert_eq!((decoded.d, decoded.rho), (pair.d, pair.rho));
        spec.inst_p = decoded.ip;
        spec.inst_s = decoded.is;
        crate::verifier::verify_pair(&spec, &decoded.pp, &decoded.ps, decoded.r).unwrap();
        assert!(
            crate::verifier::verify_pair(
                &spec,
                &decoded.pp,
                &decoded.ps,
                (Fq::from_repr(decoded.r).unwrap() + Fq::ONE).to_repr()
            )
            .is_err()
        );
        let compact = encode(&project_wire(&wire, &inputs).unwrap()).unwrap();
        let projected = decode_with_projection(&profile, &compact, Some(&inputs)).unwrap();
        spec.inst_p = projected.ip;
        spec.inst_s = projected.is;
        crate::verifier::verify_pair(&spec, &projected.pp, &projected.ps, projected.r).unwrap();
        let mut wrong_context = projection();
        wrong_context.primary.io[crate::full::HEAD_IO] += Fp::ONE;
        let wrong = decode_with_projection(&profile, &compact, Some(&wrong_context)).unwrap();
        spec.inst_p = wrong.ip;
        spec.inst_s = wrong.is;
        assert!(
            crate::verifier::verify_pair(&spec, &wrong.pp, &wrong.ps, wrong.r).is_err(),
            "reconstructed context must still pass linked cryptographic verification"
        );
        println!(
            "actual toy linked-pair carrier: {} bytes, proofs {}+{}, ordinary io {}+{}, R 32; projected {} bytes excluding separately authenticated external context; not the fixed-point monetary pair",
            bytes.len(),
            wire.p_proof.len(),
            wire.s_proof.len(),
            wire.p_io.len(),
            wire.s_io.len(),
            compact.len()
        );
    }

    #[test]
    fn rejects_every_truncation_and_trailing_bytes() {
        let (profile, wire) = fixture();
        let bytes = encode(&wire).unwrap();
        for n in 0..bytes.len() {
            assert!(decode(&profile, &bytes[..n]).is_err(), "prefix {n}");
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&profile, &trailing).is_err());
    }

    #[test]
    fn rejects_profile_schedule_layout_and_version_mismatches() {
        let (profile, wire) = fixture();
        for which in 0..6 {
            let mut altered = wire.clone();
            match which {
                0 => altered.profile[0] ^= 1,
                1 => altered.version += 1,
                2 => altered.lean = !altered.lean,
                3 => altered.p_layout.sx += 1,
                4 => altered.s_layout.io += 1,
                _ => altered.p_layout.phase1 += 1,
            }
            assert!(decode(&profile, &encode(&altered).unwrap()).is_err());
        }
        let mut other = profile.clone();
        other.id[0] ^= 1;
        assert!(decode(&other, &encode(&wire).unwrap()).is_err());
    }

    #[test]
    fn rejects_noncanonical_scalars_points_lengths_and_one_slot() {
        let (profile, wire) = fixture();
        for which in 0..8 {
            let mut altered = wire.clone();
            match which {
                0 => altered.r = vec![255; 32],
                1 => altered.r.pop().map(|_| ()).unwrap(),
                2 => altered.p_io[32..64].fill(255),
                3 => altered.s_io[32..64].fill(255),
                4 => altered.p_io[0..32].fill(0),
                5 => {
                    altered.s_io.push(0);
                }
                6 => altered.p_proof[..32].fill(255),
                _ => {
                    altered.s_proof.pop();
                }
            }
            assert!(
                decode(&profile, &encode(&altered).unwrap()).is_err(),
                "mutation {which}"
            );
        }
    }

    #[test]
    fn rejects_corrupt_frame_and_resource_overflow() {
        let (profile, wire) = fixture();
        let mut bytes = encode(&wire).unwrap();
        bytes[0] ^= 1;
        assert!(decode(&profile, &bytes).is_err());
        let mut wrong_flags = encode(&wire).unwrap();
        wrong_flags[norito::core::Header::SIZE - 1] = 1;
        assert!(decode(&profile, &wrong_flags).is_err());
        let mut corrupt_payload = encode(&wire).unwrap();
        *corrupt_payload.last_mut().unwrap() ^= 1;
        assert!(decode(&profile, &corrupt_payload).is_err());
        assert!(decode(&profile, &vec![0; MAX_RESEARCH_BYTES + 1]).is_err());
        let mut invalid = profile;
        invalid.p.phase1 = invalid.p.rows + 1;
        assert!(decode(&invalid, &encode(&wire).unwrap()).is_err());
    }
}
