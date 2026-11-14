//! Native authority for the complete future recursive backend.
//!
//! The immutable program set is pinned by a root-signed manifest. Every output
//! is verified against that program, both native matrix accumulators are checked,
//! and every proof operation takes fresh OS entropy. Custody remains owned by
//! the canonical Advance provider.
use super::*;
use crate::{
    carrier, certificate, credential, deferral,
    fs::{ProofEntropy, Scalar},
    full,
    ipa::Gens,
    payment_wire,
    r1cs::{Circuit, Instance, Witness},
    spartan::{self, Link},
    uniform::UniformCircuit,
};
use ff::{Field, PrimeField};
use iroha_data_model::{
    isi::kagemusha_wallet::load_finality::KagemushaWalletLoadReceiptV1,
    kagemusha::KagemushaWalletLoadFinalityV1, sumeragi_finality::SumeragiCommitVerifierV1,
};
use iroha_pasta::{Ep, Eq, Fq};

#[cfg(test)]
#[path = "engine_adversarial_tests.rs"]
mod adversarial_tests;

#[derive(Default)]
pub struct NativeEngine {
    time: Option<Arc<dyn TimeSource>>,
}
impl sealed::Engine for NativeEngine {}

pub(crate) struct Program {
    pub primary: Arc<UniformCircuit<Fp>>,
    pub secondary: Arc<UniformCircuit<Fq>>,
}
pub(crate) struct ParameterSet {
    pub programs: [Program; 4],
    pub generators: full::PairGens,
    pub scheme: [u8; 32],
    pub relation: [u8; 32],
    pub root: p256::Point,
    pub finality: SumeragiCommitVerifierV1,
}
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Parameters.v1")]
pub struct ParameterManifest {
    pub version: u16,
    pub scheme: [u8; 32],
    pub relation: [u8; 32],
    pub primary: [[u8; 32]; 4],
    pub secondary: [[u8; 32]; 4],
    pub dimensions_p: [u32; 2],
    pub dimensions_s: [u32; 2],
    pub finality_network: String,
    pub finality_chain: String,
}

impl AuthenticatedParameters {
    /// Exact lower bound from unavoidable simultaneous dense prover vectors.
    /// This excludes symbolic planning, retained programs, allocator and worker
    /// overhead; passing it is necessary, not evidence of device feasibility.
    pub fn minimum_proving_memory_bytes(&self, kind: usize) -> Result<u64> {
        let program = self
            .inner
            .programs
            .get(kind)
            .ok_or(Error::Malformed("wallet kind"))?;
        let count = |bits: usize| 1u64.checked_shl(bits as u32).ok_or(Error::ResourceLimit);
        let wp = count(program.primary.shape.sw())?;
        let ws = count(program.secondary.shape.sw())?;
        let cp = count(program.primary.shape.sx)?;
        let cs = count(program.secondary.shape.sx)?;
        let phase = |w: u64, other: u64, constraints: u64| {
            w.checked_mul(4)
                .and_then(|n| n.checked_add(other))
                .and_then(|n| constraints.checked_mul(3).and_then(|c| n.checked_add(c)))
                .and_then(|n| n.checked_mul(32))
                .ok_or(Error::ResourceLimit)
        };
        let prefix = wp
            .checked_mul(2)
            .and_then(|n| n.checked_add(ws))
            .and_then(|n| n.checked_mul(32))
            .ok_or(Error::ResourceLimit)?;
        Ok(prefix.max(phase(wp, 0, cp)?).max(phase(ws, wp, cs)?))
    }
    /// Authenticate all eight exact public programs under Native's independently
    /// installed scheme root and checkpoint-rooted finality reader. SDK bridges
    /// obtain these two trusted inputs from their native installation owner;
    /// foreign requests supply only the bundle and signed manifest originals.
    pub fn install(
        program_bundle: &[u8],
        manifest_bytes: &[u8],
        signature: [u8; 64],
        installed_scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<Self> {
        installed_scheme
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let root = super::point(installed_scheme.scheme_root_key.as_ref())?;
        // Authenticate the small manifest before allocating its large program bundle.
        let manifest = authenticate_manifest(manifest_bytes, &signature, &root)?;
        if manifest.scheme != installed_scheme.scheme_id() {
            return Err(Error::AuthorityMismatch("installed native scheme"));
        }
        let bundle = ProgramBundle::decode(program_bundle)?;
        let programs = (0..4)
            .map(|i| {
                Ok(Program {
                    primary: Arc::new(
                        UniformCircuit::import_program(&bundle.primary[i], true)
                            .map_err(Error::Rejected)?,
                    ),
                    secondary: Arc::new(
                        UniformCircuit::import_program(&bundle.secondary[i], false)
                            .map_err(Error::Rejected)?,
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| Error::Malformed("four monetary programs"))?;
        ParameterSet::authenticate(programs, manifest_bytes, signature, root, finality)
    }
    /// Canonical public programs only; this never exports private witness arrays.
    pub fn export_program_bundle(&self) -> Result<Vec<u8>> {
        let primary = self
            .inner
            .programs
            .iter()
            .map(|p| p.primary.export_program().map_err(Error::Rejected))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| Error::Malformed("primary program count"))?;
        let secondary = self
            .inner
            .programs
            .iter()
            .map(|p| p.secondary.export_program().map_err(Error::Rejected))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| Error::Malformed("secondary program count"))?;
        ProgramBundle {
            version: 1,
            primary,
            secondary,
        }
        .encode()
    }
}

impl ParameterSet {
    /// The compiler supplies real programs; the independent installation supplies
    /// its trusted scheme root and already authenticated native genesis reader.
    pub(crate) fn authenticate(
        programs: [Program; 4],
        manifest_bytes: &[u8],
        signature: [u8; 64],
        root: p256::Point,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<AuthenticatedParameters> {
        let manifest = authenticate_manifest(manifest_bytes, &signature, &root)?;
        if manifest.finality_network != finality.network().to_string()
            || manifest.finality_chain != finality.chain_id()
        {
            return Err(Error::AuthorityMismatch("parameter ledger root"));
        }
        for i in 0..4 {
            if programs[i].primary.program_digest() != manifest.primary[i]
                || programs[i].secondary.program_digest() != manifest.secondary[i]
            {
                return Err(Error::AuthorityMismatch("exact compiled monetary program"));
            }
        }
        let pcols = programs[0].primary.shape.cols();
        let scols = programs[0].secondary.shape.cols();
        for p in &programs {
            if p.primary.shape.cols() != pcols
                || p.secondary.shape.cols() != scols
                || p.primary.io_len != 64
                || p.secondary.io_len != 62
            {
                return Err(Error::Malformed("parameter program geometry"));
            }
        }
        let gp = Gens::<Eq>::new(pcols);
        let gs = Gens::<Ep>::new(scols);
        let keys_p = programs
            .iter()
            .map(|x| spartan::fixed_commitments(&x.primary.shape, &gp, x.primary.as_ref()))
            .collect();
        let keys_s = programs
            .iter()
            .map(|x| spartan::fixed_commitments(&x.secondary.shape, &gs, x.secondary.as_ref()))
            .collect();
        let dimensions = [manifest.dimensions_p, manifest.dimensions_s];
        if dimensions.iter().flatten().any(|&n| !(1..=30).contains(&n)) {
            return Err(Error::Malformed("parameter accumulator dimensions"));
        }
        let dp = (dimensions[0][0] as usize, dimensions[0][1] as usize);
        let ds = (dimensions[1][0] as usize, dimensions[1][1] as usize);
        let defer_p = deferral::MatPoly::with_min_vec(
            programs
                .iter()
                .map(|x| full::uniform_kind(x.primary.clone()))
                .collect(),
            dp.0,
            dp.1,
        );
        let defer_s = deferral::MatPoly::with_min_vec(
            programs
                .iter()
                .map(|x| full::uniform_kind(x.secondary.clone()))
                .collect(),
            ds.0,
            ds.1,
        );
        let generators = full::PairGens {
            p: gp,
            s: gs,
            defer_p,
            defer_s,
            keys_p: Some(keys_p),
            keys_s: Some(keys_s),
        };
        Ok(AuthenticatedParameters {
            id: manifest.relation,
            inner: Self {
                programs,
                generators,
                scheme: manifest.scheme,
                relation: manifest.relation,
                root,
                finality,
            },
        })
    }
    fn profile(&self, kind: usize) -> Result<carrier::TrustedProfile> {
        let p = self
            .programs
            .get(kind)
            .ok_or(Error::Malformed("wallet kind"))?;
        carrier::TrustedProfile::from_parameters(
            self.relation,
            p.primary.shape,
            p.primary.as_ref(),
            p.secondary.shape,
            p.secondary.as_ref(),
            true,
        )
        .map_err(Error::Rejected)
    }
    fn context(&self) -> bootstrap::Context {
        bootstrap::Context {
            root: certificate::RootContext {
                scheme: self.scheme,
                root: self.root.clone(),
            },
            relation: self.relation,
        }
    }
}

pub(super) fn authenticate_manifest(
    bytes: &[u8],
    signature: &[u8; 64],
    root: &p256::Point,
) -> Result<ParameterManifest> {
    let manifest: ParameterManifest = super::decode(bytes)?;
    if manifest.version != 1
        || super::encode(&manifest)? != bytes
        || manifest.relation == [0; 32]
        || manifest.scheme == [0; 32]
    {
        return Err(Error::Malformed("parameter manifest"));
    }
    let m = crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), bytes);
    verify_signature(root, m, signature)?;
    Ok(manifest)
}

fn verify_signature(q: &p256::Point, m: Fp, sig: &[u8; 64]) -> Result<()> {
    let r = BigUint::from_bytes_be(&sig[..32]);
    let s = BigUint::from_bytes_be(&sig[32..]);
    if q.x >= p256::p() || q.y >= p256::p() || !p256::on_curve(q) || s > p256::n() >> 1 {
        return Err(Error::Malformed("canonical signature/key"));
    }
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(m.to_repr().as_ref()));
    if !p256::verify(q, &z, &p256::Signature { r, s }) {
        return Err(Error::Rejected("P-256 signature".into()));
    }
    Ok(())
}
fn credential_bytes(c: &Credential) -> Vec<u8> {
    [c.body.as_slice(), c.signature.as_slice()].concat()
}
fn certificate_bytes(c: &certificate::Certificate) -> Vec<u8> {
    [c.body.as_slice(), c.signature.as_slice()].concat()
}
fn read_credential(b: &[u8]) -> Result<Credential> {
    if b.len() != 596 {
        return Err(Error::Malformed("credential length"));
    }
    Ok(Credential {
        body: b[..532].try_into().unwrap(),
        signature: b[532..].try_into().unwrap(),
    })
}
fn read_certificate(b: &[u8]) -> Result<certificate::Certificate> {
    if b.len() != 172 {
        return Err(Error::Malformed("certificate length"));
    }
    Ok(certificate::Certificate {
        body: b[..108].try_into().unwrap(),
        signature: b[108..].try_into().unwrap(),
    })
}
fn exact_credential(a: &Credential, b: &Credential) -> bool {
    a.body == b.body && a.signature == b.signature
}
fn scope(parameters: &AuthenticatedParameters, id: &[u8; 32]) -> Result<()> {
    if id != parameters.id() {
        Err(Error::AuthorityMismatch("parameter identity"))
    } else {
        Ok(())
    }
}
fn identity(x: &[Fp]) -> Result<[u8; 32]> {
    if x.len() != 2 {
        return Err(Error::Malformed("identity limbs"));
    }
    let mut b = [0; 32];
    for i in 0..2 {
        let r = x[i].to_repr();
        if r[16..].iter().any(|x| *x != 0) {
            return Err(Error::Malformed("identity overflow"));
        }
        b[16 * i..16 * i + 16].copy_from_slice(&r[..16]);
    }
    Ok(b)
}

fn authenticate_credential(
    parameters: &ParameterSet,
    c: &Credential,
    cert: &certificate::Certificate,
) -> Result<()> {
    if c.body[..2] != 1u16.to_le_bytes()
        || c.body[2..34] != parameters.scheme
        || cert.body[..2] != 1u16.to_le_bytes()
        || cert.body[2..34] != parameters.scheme
        || cert.body[34] != 1
        || c.body[500..532] != certificate::certificate_digest(cert).to_repr()
    {
        return Err(Error::AuthorityMismatch("credential enrollment chain"));
    }
    verify_signature(
        &parameters.root,
        certificate::message(cert),
        &cert.signature,
    )?;
    let issuer = point(&cert.body[35..100])?;
    verify_signature(&issuer, credential::message(c), &c.signature)?;
    let q = point(&c.body[130..195])?;
    let table = full::wallet_table_commitment(&q, &parameters.generators.p);
    let certified: [Fp; 4] = std::array::from_fn(|i| {
        certificate::fp(&BigUint::from_bytes_le(&c.body[195 + 16 * i..211 + 16 * i]))
    });
    if spartan::limbs_of_point::<Eq>(&table) != certified {
        return Err(Error::AuthorityMismatch("certified wallet table"));
    }
    Ok(())
}
fn profile_matches(parameters: &ParameterSet, p: &[Fp; 64], c: &Credential) -> Result<()> {
    use crate::bootstrap_context as b;
    if p[0] != Fp::ONE
        || identity(&p[b::CONTEXT_IO + b::SCHEME..b::CONTEXT_IO + b::SCHEME + 2])?
            != parameters.scheme
        || identity(&p[b::CONTEXT_IO + b::RELATION..b::CONTEXT_IO + b::RELATION + 2])?
            != parameters.relation
        || identity(&p[b::CONTEXT_IO + b::WALLET..b::CONTEXT_IO + b::WALLET + 2])? != c.body[66..98]
        || p[b::CONTEXT_IO + b::CREDENTIAL] != credential::digest(c)
    {
        return Err(Error::AuthorityMismatch("wallet public credential context"));
    }
    Ok(())
}
fn projection(profile: &[Fp; 64], kind: usize) -> Result<carrier::ProjectedInputs> {
    let b = crate::bootstrap_context::CONTEXT_IO;
    let l = crate::bootstrap_context::LINEAGE_IO;
    let out = carrier::ProjectedInputs::from_authenticated_fields(
        profile[full::HEAD_IO],
        profile[full::STATEMENT_IO],
        profile[full::RECEIPT_IO],
        profile[b..b + 13].try_into().unwrap(),
        profile[l..l + 6].try_into().unwrap(),
        profile[full::TABLE_IO..full::TABLE_IO + 4]
            .try_into()
            .unwrap(),
        kind,
    )
    .map_err(Error::Rejected)?;
    if out.primary().io != profile {
        return Err(Error::AuthorityMismatch("complete public projection"));
    }
    Ok(out)
}
fn verify_pair(
    parameters: &ParameterSet,
    kind: usize,
    d: carrier::Decoded,
) -> Result<full::UniformPair> {
    let p = parameters
        .programs
        .get(kind)
        .ok_or(Error::Malformed("wallet kind"))?;
    let r = d.r;
    let rlc = move |_: &[u8; 32]| r;
    let vp = spartan::verify_linked_circuit(
        &p.primary.shape,
        &parameters.generators.p,
        &d.ip,
        &d.pp.to_bytes(),
        &Link::First { rho: d.rho, r: d.r },
        p.primary.as_ref(),
    )
    .map_err(|_| Error::Rejected("primary linked proof".into()))?;
    let vs = spartan::verify_linked_circuit(
        &p.secondary.shape,
        &parameters.generators.s,
        &d.is,
        &d.ps.to_bytes(),
        &Link::Second { d: d.d, rlc: &rlc },
        p.secondary.as_ref(),
    )
    .map_err(|_| Error::Rejected("secondary linked proof".into()))?;
    if vp != d.d || vs != d.rho {
        return Err(Error::Rejected("pair transcript binding".into()));
    }
    let np = parameters.generators.defer_p.n();
    let ns = parameters.generators.defer_s.n();
    if d.pp.late.len() < np + 1 || d.ps.late.len() < ns + 1 {
        return Err(Error::Malformed("matrix accumulator output length"));
    }
    let ap = deferral::claim_of(&d.pp.late[..np + 1]);
    let as_ = deferral::claim_of(&d.ps.late[..ns + 1]);
    if ap.point.len() != parameters.generators.defer_p.n()
        || as_.point.len() != parameters.generators.defer_s.n()
        || parameters.generators.defer_p.eval(&ap.point) != ap.value
        || parameters.generators.defer_s.eval(&as_.point) != as_.value
    {
        return Err(Error::Rejected("native matrix accumulators".into()));
    }
    let u = full::UniformPair {
        cp: p.primary.clone(),
        ip: d.ip,
        pp: d.pp,
        cs: p.secondary.clone(),
        is: d.is,
        ps: d.ps,
        d: d.d,
        rho: d.rho,
        r: d.r,
        kind,
    };
    let traced = full::trace_uniform(u.clone(), &parameters.generators);
    let inherited_p = full::inherited_generator_claim(&traced.tr_p, &parameters.generators.p)
        .map_err(Error::Malformed)?;
    let inherited_s = full::inherited_generator_claim(&traced.tr_s, &parameters.generators.s)
        .map_err(Error::Malformed)?;
    if !crate::generator_claim::decide(&parameters.generators.p, &inherited_p)
        || !crate::generator_claim::decide(&parameters.generators.s, &inherited_s)
    {
        return Err(Error::Rejected(
            "native inherited generator accumulators".into(),
        ));
    }
    Ok(u)
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Head.v1")]
struct HeadFrame {
    version: u16,
    kind: u8,
    parameters: [u8; 32],
    profile: Vec<[u8; 32]>,
    pair: Vec<u8>,
    credential: Vec<u8>,
    certificate: Vec<u8>,
}
fn encode_head(
    parameters: &AuthenticatedParameters,
    u: &full::UniformPair,
    c: &Credential,
    cert: &certificate::Certificate,
) -> Result<Vec<u8>> {
    let profile: [Fp; 64] =
        u.ip.io
            .as_slice()
            .try_into()
            .map_err(|_| Error::Malformed("head profile"))?;
    let proj = projection(&profile, u.kind)?;
    let trusted = parameters.inner.profile(u.kind)?;
    let pair = carrier::encode_projected_pair(&trusted, &u.ip, &u.is, &u.pp, &u.ps, u.r, &proj)
        .map_err(Error::Rejected)?;
    super::encode(&HeadFrame {
        version: 1,
        kind: u.kind as u8,
        parameters: parameters.id,
        profile: profile.map(|x| x.to_repr()).to_vec(),
        pair,
        credential: credential_bytes(c),
        certificate: certificate_bytes(cert),
    })
}
fn decode_head(
    bytes: &[u8],
    parameters: &AuthenticatedParameters,
) -> Result<(
    HeadFrame,
    [Fp; 64],
    Credential,
    certificate::Certificate,
    full::UniformPair,
)> {
    let f: HeadFrame = super::decode(bytes)?;
    if f.version != 1 || f.kind > 3 || f.profile.len() != 64 || super::encode(&f)? != bytes {
        return Err(Error::Malformed("canonical head"));
    }
    scope(parameters, &f.parameters)?;
    let profile = f
        .profile
        .iter()
        .map(field)
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .unwrap();
    let c = read_credential(&f.credential)?;
    let cert = read_certificate(&f.certificate)?;
    authenticate_credential(&parameters.inner, &c, &cert)?;
    profile_matches(&parameters.inner, &profile, &c)?;
    let proj = projection(&profile, f.kind as usize)?;
    let trusted = parameters.inner.profile(f.kind as usize)?;
    let decoded =
        carrier::decode_projected_pair(&trusted, &f.pair, &proj).map_err(Error::Rejected)?;
    let pair = verify_pair(&parameters.inner, f.kind as usize, decoded)?;
    Ok((f, profile, c, cert, pair))
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Recovery.v1")]
struct RecoveryFrame {
    version: u16,
    witness: Vec<u8>,
    predecessor: Vec<u8>,
    credential: Vec<u8>,
    certificate: Vec<u8>,
    profile: Vec<[u8; 32]>,
    enrollment: Option<[u8; 32]>,
    previous_capsule: [u8; 32],
}
impl Drop for RecoveryFrame {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.witness);
    }
}
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Output.v1")]
struct OutputFrame {
    version: u16,
    head: Vec<u8>,
    payment: Vec<u8>,
}
pub(super) fn retained_output_part(bytes: &[u8], part: u32) -> Result<Vec<u8>> {
    let out: OutputFrame = super::decode(bytes)?;
    if out.version != 1 || super::encode(&out)? != bytes {
        return Err(Error::Malformed("retained output"));
    }
    match part {
        0 => Ok(out.head),
        1 => Ok(out.payment),
        _ => Err(Error::Malformed("retained output selector")),
    }
}
fn receipt_mut(w: &mut ExactOperationWitness) -> &mut bootstrap::Receipt {
    match w {
        ExactOperationWitness::Bootstrap(x) => &mut x.input.receipt,
        ExactOperationWitness::Load { step, .. } => &mut step.receipt,
        ExactOperationWitness::Send(x) => &mut x.receipt,
        ExactOperationWitness::Receive { step, .. } => &mut step.receipt,
    }
}
fn terms(w: &ExactOperationWitness) -> (&Credential, &[Fp; 26], &bootstrap::Receipt, u8) {
    match w {
        ExactOperationWitness::Bootstrap(x) => {
            (&x.input.credential, &x.input.statement, &x.input.receipt, 1)
        }
        ExactOperationWitness::Load { step, .. } => {
            (&step.credential, &step.statement, &step.receipt, 2)
        }
        ExactOperationWitness::Send(x) => (&x.credential, &x.statement, &x.receipt, 3),
        ExactOperationWitness::Receive { step, .. } => {
            (&step.credential, &step.statement, &step.receipt, 4)
        }
    }
}
fn kind(w: &ExactOperationWitness) -> usize {
    match w {
        ExactOperationWitness::Bootstrap(_) => full::BOOTSTRAP,
        ExactOperationWitness::Load { .. } => full::LOAD,
        ExactOperationWitness::Send(_) => full::SEND,
        ExactOperationWitness::Receive { .. } => full::RECEIVE,
    }
}
fn freeze_witness(
    mut w: ExactOperationWitness,
    head: &VerifiedWalletHead,
) -> Result<(ExactOperationWitness, [u8; 32])> {
    let r = receipt_mut(&mut w);
    r.signature = [0; 64];
    r.body[242..306].fill(0);
    let mut bytes = zeroize::Zeroizing::new(super::witness_codec::encode_witness(&w)?);
    bytes.extend_from_slice(&head.profile[full::HEAD_IO].to_repr());
    bytes.extend_from_slice(&head.custody_digest);
    let digest = crate::payment::byte_digest(u64::from_le_bytes(*b"kgwprep1"), &bytes).to_repr();
    receipt_mut(&mut w).body[242..274].copy_from_slice(&digest);
    Ok((w, digest))
}
fn build<S: Scalar>(
    w: full::Wiring<S>,
    target: &UniformCircuit<S>,
    entropy: &ProofEntropy,
    purpose: &[u8],
) -> (UniformCircuit<S>, Instance<S>, Witness<S>) {
    let (a, p, n, t, c) = w.finish();
    let seed = zeroize::Zeroizing::new(entropy.stream(purpose).scalar::<Fp>().to_repr());
    UniformCircuit::build_with_copies_at(
        target.label,
        a,
        p,
        n,
        t,
        c,
        target.shape.col_bits,
        target.io_len,
        seed.as_ref(),
        Some(target.layout()),
    )
}
struct PrivateCircuit<S: Scalar + zeroize::Zeroize>(UniformCircuit<S>);
impl<S: Scalar + zeroize::Zeroize> std::ops::Deref for PrivateCircuit<S> {
    type Target = UniformCircuit<S>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S: Scalar + zeroize::Zeroize> Drop for PrivateCircuit<S> {
    fn drop(&mut self) {
        self.0.zeroize_witness_values();
    }
}
fn check_constraints(
    w: full::Wiring<Fp>,
    columns: usize,
    source: Option<&dyn TimeSource>,
) -> Result<()> {
    let (a, p, n, t, c) = w.finish();
    let entropy = ProofEntropy::fresh().map_err(|_| Error::Unavailable("OS proof entropy"))?;
    let seed = zeroize::Zeroizing::new(
        entropy
            .stream(b"preparation witness")
            .scalar::<Fp>()
            .to_repr(),
    );
    let (c, i, wit) = UniformCircuit::build_with_copies(
        "native preparation",
        a,
        p,
        n,
        t,
        c,
        columns,
        192,
        seed.as_ref(),
    );
    let c = PrivateCircuit(c);
    if let Some(source) = source {
        let required = c.preparation_memory_bytes();
        let available = source.available_memory_bytes()?;
        if available < required {
            return Err(Error::NotEnoughMemory {
                required,
                available,
            });
        }
    }
    let mut wit = zeroize::Zeroizing::new(wit);
    let mut rng = entropy.stream(b"preparation relation check");
    let ch = [rng.scalar(), rng.scalar()];
    let rel = c.instantiate(&c.shape, &ch, Some(&mut wit.w));
    let z = zeroize::Zeroizing::new(crate::r1cs::z_vector(&c.shape, &i, &wit));
    let (a, b, products) = rel.abc(&c.shape, &z);
    let a = zeroize::Zeroizing::new(a);
    let b = zeroize::Zeroizing::new(b);
    let products = zeroize::Zeroizing::new(products);
    if a.iter()
        .zip(b.iter())
        .zip(products.iter())
        .all(|((a, b), c)| *a * b == *c)
    {
        Ok(())
    } else {
        Err(Error::Rejected(
            "native monetary preparation constraints".into(),
        ))
    }
}
impl NativeEngine {
    fn require_stage_memory(&self, workspace: u64, cold_commitments: u64) -> Result<()> {
        if let Some(source) = &self.time {
            let required = workspace
                .checked_add(cold_commitments)
                .ok_or(Error::ResourceLimit)?
                .checked_add(
                    u64::try_from(iroha_pasta::msm::PROCESS_MSM_SCRATCH_BYTES)
                        .map_err(|_| Error::ResourceLimit)?,
                )
                .ok_or(Error::ResourceLimit)?;
            let available = source.available_memory_bytes()?;
            if available < required {
                return Err(Error::NotEnoughMemory {
                    required,
                    available,
                });
            }
        }
        Ok(())
    }
    fn require_minimum_memory(
        &self,
        parameters: &AuthenticatedParameters,
        kind: usize,
    ) -> Result<()> {
        if let Some(source) = &self.time {
            let required = parameters.minimum_proving_memory_bytes(kind)?;
            let available = source.available_memory_bytes()?;
            if available < required {
                return Err(Error::NotEnoughMemory {
                    required,
                    available,
                });
            }
        }
        Ok(())
    }
    fn incoming_candidate(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<IncomingCandidate> {
        let p = &parameters.inner;
        let trusted = p.profile(full::SEND)?;
        let d = payment_wire::decode_candidate(
            &payment_wire::Profile {
                pair: &trusted,
                scheme: p.scheme,
                relation: p.relation,
            },
            bytes,
        )
        .map_err(Error::Rejected)?;
        let profile: [Fp; 64] = d
            .projection
            .primary()
            .io
            .as_slice()
            .try_into()
            .map_err(|_| Error::Malformed("candidate profile"))?;
        let decoded = carrier::decode_projected_candidate(&trusted, &d.pair_bytes, &d.projection)
            .map_err(Error::Rejected)?;
        // Parsing and the exact full verifier derive this private verdict. No
        // foreign Boolean can skip proof checks or turn a malformed candidate into credit.
        let valid = match self.verify_payment(bytes, parameters) {
            Ok(_) => true,
            Err(
                e @ (Error::ResourceLimit | Error::NotEnoughMemory { .. } | Error::Unavailable(_)),
            ) => return Err(e),
            Err(_) => false,
        };
        let program = &p.programs[full::SEND];
        let pair = full::UniformPair {
            cp: program.primary.clone(),
            ip: decoded.ip,
            pp: decoded.pp,
            cs: program.secondary.clone(),
            is: decoded.is,
            ps: decoded.ps,
            d: decoded.d,
            rho: decoded.rho,
            r: decoded.r,
            kind: full::SEND,
        };
        Ok(IncomingCandidate {
            profile,
            proof_digest: d.digests.proof,
            valid,
            originals: d.originals,
            pair,
            pair_bytes: d.pair_bytes,
        })
    }
    /// Authenticate exact future credential and issuer-certificate originals under
    /// the installed scheme root and the actual certified table generators.
    pub fn authenticate_credential_originals(
        &self,
        credential: &[u8],
        certificate: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<Credential> {
        let credential = read_credential(credential)?;
        let certificate = read_certificate(certificate)?;
        authenticate_credential(&parameters.inner, &credential, &certificate)?;
        Ok(credential)
    }
    /// Bind the canonical local custody/OS clock adapter. Controls-off operations
    /// use `Default` and never acquire a network or clock dependency.
    pub fn with_time_source(time: Arc<dyn TimeSource>) -> Self {
        Self { time: Some(time) }
    }
    pub(super) fn check_send_time(
        &self,
        witness: &ExactOperationWitness,
        fresh: bool,
    ) -> Result<()> {
        let ExactOperationWitness::Send(step) = witness else {
            return Ok(());
        };
        let uint = |v: Fp| -> Result<u64> {
            let b = v.to_repr();
            if b[8..].iter().any(|&x| x != 0) {
                return Err(Error::Malformed("Send time integer"));
            }
            Ok(u64::from_le_bytes(b[..8].try_into().unwrap()))
        };
        let controls = uint(step.before_core[22])?;
        if controls > 3 {
            return Err(Error::Malformed("Send controls"));
        }
        let floor = uint(step.before_core[28])?;
        let accepted = u64::from_le_bytes(step.request.body[346..354].try_into().unwrap());
        if controls == 0 {
            if step.lower != floor.max(accepted) || step.upper != step.lower {
                return Err(Error::AuthorityMismatch("controls-off Send floor"));
            }
            return Ok(());
        }
        let source = self
            .time
            .as_ref()
            .ok_or(Error::Unavailable("protected same-boot time anchor"))?;
        let wallet = step.credential.body[66..98].try_into().unwrap();
        let (anchor, now) = if fresh {
            source.observe(&wallet)?
        } else {
            source.prepared(&wallet, &step.receipt.body[114..146].try_into().unwrap())?
        };
        if anchor.anchor.body.scheme_id != step.credential.body[2..34]
            || anchor.anchor.body.wallet_id != wallet
            || anchor.anchor.time_anchor_digest() != step.before_rest[5].to_repr()
        {
            return Err(Error::AuthorityMismatch("committed protected time anchor"));
        }
        let mut interval = anchor
            .interval_at(&now, uint(step.before_core[25])?)
            .map_err(|e| Error::Rejected(e.to_string()))?;
        interval.lower_ms = interval.lower_ms.max(floor).max(accepted);
        interval.upper_ms = interval.upper_ms.max(interval.lower_ms);
        if !fresh {
            if step.lower != interval.lower_ms || step.upper != interval.upper_ms {
                return Err(Error::AuthorityMismatch("observed Send time interval"));
            }
            return Ok(());
        }
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let before = crate::monetary_state::append_state(
            &mut w,
            family,
            &step.before_core,
            &step.before_rest,
        );
        let request = crate::request::append_completed(&mut w, family, &step.request);
        let controls = crate::send_controls::append_send_controls(
            &mut w,
            family,
            &before,
            &request,
            interval.lower_ms,
            interval.upper_ms,
            &step.quota,
        );
        if w.arrays[controls.local].values[controls.new_usage_root][0] != step.after_core[21] {
            return Err(Error::AuthorityMismatch(
                "Send crossed quota charge boundary",
            ));
        }
        check_constraints(w, 13, self.time.as_deref())
    }
    /// Start from an authenticated enrollment original and the provider's protected
    /// marker. No funded state or synthetic recursive predecessor is introduced.
    pub fn enrollment_origin(
        &self,
        input: &BootstrapWitness,
        marker: [u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedWalletHead> {
        if marker == [0; 32]
            || input.scheme != parameters.inner.scheme
            || input.relation != parameters.inner.relation
            || input.root != parameters.inner.root
        {
            return Err(Error::AuthorityMismatch("enrollment installation"));
        }
        authenticate_credential(
            &parameters.inner,
            &input.input.credential,
            &input.input.certificate,
        )?;
        let mut profile = [Fp::ZERO; 64];
        profile[0] = Fp::ONE;
        Ok(VerifiedWalletHead {
            parameters_id: parameters.id,
            profile,
            pair: Arc::from([]),
            credential: input.input.credential.clone(),
            certificate: input.input.certificate.clone(),
            custody_digest: [0; 32],
            enrollment_marker: Some(marker),
        })
    }
    /// This metadata is checked again against the provider's exclusively held
    /// marker before Selected. A recursive public head does not contain custody.
    pub fn bind_released_custody(
        &self,
        mut head: VerifiedWalletHead,
        marker: &iroha_data_model::kagemusha::KagemushaWalletMarkerV1,
    ) -> Result<VerifiedWalletHead> {
        use iroha_data_model::kagemusha::KagemushaWalletMarkerStateV1;
        marker
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let KagemushaWalletMarkerStateV1::Head {
            head: state,
            capsule_digest,
            ..
        } = &marker.state
        else {
            return Err(Error::AuthorityMismatch("released head marker"));
        };
        if state.value != head.profile[full::HEAD_IO].to_repr()
            || marker.scheme_id != head.credential.body[2..34]
            || marker.asset_digest != head.credential.body[34..66]
            || marker.wallet_id != head.credential.body[66..98]
            || marker.payment_key.as_ref() != &head.credential.body[130..195]
        {
            return Err(Error::AuthorityMismatch("released custody marker"));
        }
        head.custody_digest = *capsule_digest;
        Ok(head)
    }
    fn check_preparation(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        let p = &parameters.inner;
        let (c, _, _, _) = terms(witness);
        if !exact_credential(c, &head.credential) {
            return Err(Error::AuthorityMismatch("prepared current credential"));
        }
        authenticate_credential(p, c, &head.certificate)?;
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let predecessor = full::native_predecessor(&mut w, family, head);
        match witness {
            ExactOperationWitness::Bootstrap(input) => {
                if input.scheme != p.scheme
                    || input.relation != p.relation
                    || input.root != p.root
                    || head.enrollment_marker.is_none()
                    || !head.pair.is_empty()
                {
                    return Err(Error::AuthorityMismatch("bootstrap enrollment origin"));
                }
                let prepared = bootstrap::prepare_unsigned(&p.context(), &input.input)
                    .map_err(Error::Malformed)?;
                return check_constraints(
                    prepared.wiring,
                    p.programs[full::BOOTSTRAP].primary.shape.col_bits,
                    self.time.as_deref(),
                );
            }
            ExactOperationWitness::Load {
                step,
                receipt_original,
                finality_original,
            } => {
                let receipt = KagemushaWalletLoadReceiptV1::decode_canonical(receipt_original)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
                if receipt
                    .transcript()
                    .map_err(|e| Error::Rejected(e.to_string()))?
                    != step.ordinary
                {
                    return Err(Error::AuthorityMismatch("exact Load original"));
                }
                let finality = KagemushaWalletLoadFinalityV1::decode_canonical(finality_original)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
                finality
                    .verify_with(&mut p.finality.clone(), &receipt)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
                crate::load::append_preparation(&mut w, family, step, &predecessor)
                    .map_err(Error::Malformed)?;
            }
            ExactOperationWitness::Send(step) => {
                crate::send::append_preparation(&mut w, family, step, &predecessor)
                    .map_err(Error::Malformed)?;
            }
            ExactOperationWitness::Receive {
                step,
                incoming_payment,
            } => {
                let candidate = self.incoming_candidate(incoming_payment, parameters)?;
                let verified = &candidate.originals;
                if step.request.body != verified.request.body
                    || step.request.signature != verified.request.signature
                    || !exact_credential(&step.payer, &verified.payer)
                    || step.incoming_statement != verified.statement
                    || step.incoming_receipt.body != verified.receipt.body
                    || step.incoming_receipt.signature != verified.receipt.signature
                {
                    return Err(Error::AuthorityMismatch("exact incoming Payment originals"));
                }
                let incoming = full::native_incoming_candidate(&mut w, family, &candidate);
                crate::receive::append_preparation(&mut w, family, step, &predecessor, &incoming)
                    .map_err(Error::Malformed)?;
            }
        }
        check_constraints(
            w,
            p.programs[kind(witness)].primary.shape.col_bits,
            self.time.as_deref(),
        )
    }
    fn prepare_retained(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        scope(parameters, &head.parameters_id)?;
        if !matches!(witness, ExactOperationWitness::Bootstrap(_)) {
            if head.custody_digest == [0; 32] || head.enrollment_marker.is_some() {
                return Err(Error::AuthorityMismatch("released predecessor custody"));
            }
            let checked = self.verify_head(&head.pair, &head.credential, parameters)?;
            if checked.profile != head.profile {
                return Err(Error::AuthorityMismatch("retained predecessor"));
            }
        }
        let (mut frozen, intent) = freeze_witness(witness.clone(), head)?;
        let (c, _, r, tag) = terms(&frozen);
        let recovery = super::encode(&RecoveryFrame {
            version: 1,
            witness: super::witness_codec::encode_witness(&frozen)?,
            predecessor: head.pair.to_vec(),
            credential: credential_bytes(&head.credential),
            certificate: certificate_bytes(&head.certificate),
            profile: head.profile.map(|x| x.to_repr()).to_vec(),
            enrollment: head.enrollment_marker,
            previous_capsule: head.custody_digest,
        })?;
        let capsule = RecoveryCapsule {
            frame: CapsuleFrame {
                version: 1,
                parameters_id: parameters.id,
                scheme: parameters.inner.scheme,
                asset: c.body[34..66].try_into().unwrap(),
                wallet: c.body[66..98].try_into().unwrap(),
                payment_key: c.body[130..195].to_vec(),
                kind: tag,
                sequence: u128::from_le_bytes(r.body[98..114].try_into().unwrap()),
                predecessor: r.body[146..178].try_into().unwrap(),
                predecessor_capsule_digest: head.custody_digest,
                successor: r.body[178..210].try_into().unwrap(),
                operation_id: r.body[114..146].try_into().unwrap(),
                proof_digest: intent,
                enrollment_marker: head.enrollment_marker,
                receipt_body: r.body.to_vec(),
                recovery,
            },
        };
        let digest = RecoveryCapsule::frame_digest(&capsule.encode()?);
        receipt_mut(&mut frozen).body[274..306].copy_from_slice(&digest);
        self.check_preparation(&frozen, head, parameters)?;
        let body = receipt_mut(&mut frozen).body;
        Ok(PreparedOperation {
            parameters_id: parameters.id,
            capsule,
            witness: Arc::new(frozen),
            receipt_body: body,
            predecessor: head.clone(),
        })
    }
    fn output_head(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
        c: &Credential,
    ) -> Result<(VerifiedWalletHead, Vec<u8>)> {
        let out: OutputFrame = super::decode(bytes)?;
        if out.version != 1 || super::encode(&out)? != bytes {
            return Err(Error::Malformed("completion output"));
        }
        let head = self.verify_head(&out.head, c, parameters)?;
        if !out.payment.is_empty() {
            let incoming = self.verify_payment(&out.payment, parameters)?;
            if incoming.profile != head.profile {
                return Err(Error::AuthorityMismatch("released Payment head"));
            }
        }
        Ok((head, out.payment))
    }
}

impl Engine for NativeEngine {
    fn before_selection(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        let kind = match capsule.kind() {
            1 => full::BOOTSTRAP,
            2 => full::LOAD,
            3 => full::SEND,
            4 => full::RECEIVE,
            _ => return Err(Error::Malformed("capsule kind")),
        };
        self.require_minimum_memory(parameters, kind)?;
        let prepared = self.resume(capsule, parameters)?;
        self.check_send_time(&prepared.witness, true)
    }

    fn verify_head(
        &self,
        bytes: &[u8],
        credential: &Credential,
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedWalletHead> {
        let (_, profile, c, cert, _) = decode_head(bytes, parameters)?;
        if !exact_credential(&c, credential) {
            return Err(Error::AuthorityMismatch("requested head credential"));
        }
        Ok(VerifiedWalletHead {
            parameters_id: parameters.id,
            profile,
            pair: bytes.into(),
            credential: c,
            certificate: cert,
            custody_digest: [0; 32],
            enrollment_marker: None,
        })
    }
    fn verify_payment(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedIncomingPayment> {
        let trusted = parameters.inner.profile(full::SEND)?;
        let profile = payment_wire::Profile {
            pair: &trusted,
            scheme: parameters.inner.scheme,
            relation: parameters.inner.relation,
        };
        let d = payment_wire::decode(&profile, bytes).map_err(Error::Rejected)?;
        authenticate_credential(
            &parameters.inner,
            &d.originals.payer,
            &d.originals.certificate,
        )?;
        let profile: [Fp; 64] = d
            .projection
            .primary()
            .io
            .as_slice()
            .try_into()
            .map_err(|_| Error::Malformed("Payment profile"))?;
        profile_matches(&parameters.inner, &profile, &d.originals.payer)?;
        verify_pair(&parameters.inner, full::SEND, d.pair)?;
        Ok(VerifiedIncomingPayment {
            parameters_id: parameters.id,
            bytes: bytes.into(),
            profile,
            digest: d.digests.payment,
            request: d.originals.request,
            payer: d.originals.payer,
            statement: d.originals.statement,
            receipt: d.originals.receipt,
            pair: d.pair_bytes.into(),
        })
    }
    fn prepare(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        self.require_minimum_memory(parameters, kind(witness))?;
        self.check_send_time(witness, false)?;
        self.prepare_retained(witness, head, parameters)
    }
    fn resume(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        scope(parameters, &capsule.frame.parameters_id)?;
        capsule.validate()?;
        let r: RecoveryFrame = super::decode(&capsule.frame.recovery)?;
        if r.version != 1 || r.profile.len() != 64 || super::encode(&r)? != capsule.frame.recovery {
            return Err(Error::Malformed("canonical recovery payload"));
        }
        let witness = super::witness_codec::decode_witness(&r.witness)?;
        let c = read_credential(&r.credential)?;
        let cert = read_certificate(&r.certificate)?;
        let profile: [Fp; 64] = r
            .profile
            .iter()
            .map(field)
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .unwrap();
        let mut head = if let ExactOperationWitness::Bootstrap(b) = &witness {
            self.enrollment_origin(
                b,
                r.enrollment.ok_or(Error::Malformed("enrollment marker"))?,
                parameters,
            )?
        } else {
            self.verify_head(&r.predecessor, &c, parameters)?
        };
        if head.profile != profile
            || !exact_credential(&head.credential, &c)
            || certificate_bytes(&head.certificate) != certificate_bytes(&cert)
        {
            return Err(Error::AuthorityMismatch("recovered predecessor"));
        }
        head.custody_digest = r.previous_capsule;
        let prepared = self.prepare_retained(&witness, &head, parameters)?;
        if prepared.capsule.encode()? != capsule.encode()? {
            return Err(Error::AuthorityMismatch("recovered frozen capsule"));
        }
        Ok(prepared)
    }
    fn finalize(
        &self,
        prepared: PreparedOperation,
        receipt: &HardwareAdvanceReceipt,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation> {
        scope(parameters, &prepared.parameters_id)?;
        let actual = HardwareAdvanceReceipt::verify_selected(
            &prepared,
            &receipt.receipt.body,
            receipt.receipt.signature,
        )?;
        if actual.capsule_digest != receipt.capsule_digest
            || actual.operation_id != receipt.operation_id
        {
            return Err(Error::AuthorityMismatch("hardware selected receipt"));
        }
        let mut witness = prepared.witness.as_ref().clone();
        *receipt_mut(&mut witness) = receipt.receipt.clone();
        let p = &parameters.inner;
        let k = kind(&witness);
        let mut pairs = Vec::new();
        if k != full::BOOTSTRAP {
            let (_, _, _, _, u) = decode_head(&prepared.predecessor.pair, parameters)?;
            pairs.push(full::trace_uniform(u, &p.generators));
        }
        if let ExactOperationWitness::Receive {
            incoming_payment, ..
        } = &witness
        {
            let candidate = self.incoming_candidate(incoming_payment, parameters)?;
            pairs.push(full::trace_incoming_candidate(
                candidate.pair,
                &p.generators,
                candidate.pair_bytes,
            ));
        }
        let entropy = ProofEntropy::fresh().map_err(|_| Error::Unavailable("OS proof entropy"))?;
        let folds = if k == full::BOOTSTRAP {
            None
        } else {
            Some(
                full::GeneratorFolds::prepare(&p.generators, &pairs, &entropy)
                    .map_err(Error::Malformed)?,
            )
        };
        let (wp, ws) = match &witness {
            ExactOperationWitness::Bootstrap(input) => (
                full::p_boot_from_inputs(&p.generators, &p.context(), &input.input)
                    .map_err(Error::Malformed)?
                    .0,
                full::s_boot(&p.generators).0,
            ),
            ExactOperationWitness::Load { step, .. } => (
                full::p_operation(
                    full::OperationWitness::Load(step),
                    &p.generators,
                    &pairs,
                    folds.as_ref().unwrap(),
                )
                .map_err(Error::Malformed)?
                .0,
                full::s_operation(k, &p.generators, &pairs, folds.as_ref().unwrap()).0,
            ),
            ExactOperationWitness::Send(step) => (
                full::p_operation(
                    full::OperationWitness::Send(step),
                    &p.generators,
                    &pairs,
                    folds.as_ref().unwrap(),
                )
                .map_err(Error::Malformed)?
                .0,
                full::s_operation(k, &p.generators, &pairs, folds.as_ref().unwrap()).0,
            ),
            ExactOperationWitness::Receive { step, .. } => (
                full::p_operation(
                    full::OperationWitness::Receive(step),
                    &p.generators,
                    &pairs,
                    folds.as_ref().unwrap(),
                )
                .map_err(Error::Malformed)?
                .0,
                full::s_operation(k, &p.generators, &pairs, folds.as_ref().unwrap()).0,
            ),
        };
        let (cp, ip, wp) = build(
            wp,
            p.programs[k].primary.as_ref(),
            &entropy,
            b"primary witness",
        );
        let (cs, is, ws) = build(
            ws,
            p.programs[k].secondary.as_ref(),
            &entropy,
            b"secondary witness",
        );
        let cp = PrivateCircuit(cp);
        let cs = PrivateCircuit(cs);
        let wp = zeroize::Zeroizing::new(wp);
        let ws = zeroize::Zeroizing::new(ws);
        if cp.program_digest() != p.programs[k].primary.program_digest()
            || cs.program_digest() != p.programs[k].secondary.program_digest()
        {
            return Err(Error::AuthorityMismatch("operation compiled program"));
        }
        // Caller witnesses and both builders are already resident. Admit the
        // actual next stage, then resample OS headroom before each full proof.
        // S is dropped before P, so its released storage is available to P.
        // These accounted workspaces exclude opaque closure/allocator overhead.
        self.require_stage_memory(
            cp.phase_memory_accounting().phase1_bytes,
            p.generators.p.commitment_cache_bytes(&cp.shape, false),
        )?;
        let d = spartan::prepare_linked_phase1(
            &cp.shape,
            &p.generators.p,
            &ip,
            &wp,
            &mut entropy.stream(b"primary"),
            &*cp,
        );
        let rlc = |_: &[u8; 32]| [0; 32];
        self.require_stage_memory(
            cs.phase_memory_accounting().additional_peak_bytes,
            p.generators.s.commitment_cache_bytes(&cs.shape, true),
        )?;
        let (ps, rho, r) = spartan::prove_linked_circuit(
            &cs.shape,
            &p.generators.s,
            &is,
            &ws,
            &mut entropy.stream(b"secondary"),
            &Link::Second { d, rlc: &rlc },
            &*cs,
        );
        // Verification uses the independently authenticated public programs.
        // Retaining the secondary's private arrays/plans through the primary
        // proof only increased memory; the completion needs its proof and IO.
        drop(ws);
        drop(cs);
        self.require_stage_memory(
            cp.phase_memory_accounting().additional_peak_bytes,
            p.generators.p.commitment_cache_bytes(&cp.shape, true),
        )?;
        let (pp, d2, _) = spartan::prove_linked_circuit(
            &cp.shape,
            &p.generators.p,
            &ip,
            &wp,
            &mut entropy.stream(b"primary"),
            &Link::First { rho, r },
            &*cp,
        );
        if d != d2 {
            return Err(Error::Rejected("primary preparation replay".into()));
        }
        drop(wp);
        drop(cp);
        let u = verify_pair(
            p,
            k,
            carrier::Decoded {
                ip,
                is,
                pp,
                ps,
                r,
                d,
                rho,
            },
        )?;
        let (c, _, _, _) = terms(&witness);
        let cert = &prepared.predecessor.certificate;
        let head_bytes = encode_head(parameters, &u, c, cert)?;
        let payment = if let ExactOperationWitness::Send(step) = &witness {
            let trusted = p.profile(full::SEND)?;
            let profile = payment_wire::Profile {
                pair: &trusted,
                scheme: p.scheme,
                relation: p.relation,
            };
            let epoch = u.ip.io[33].to_repr();
            if epoch[8..].iter().any(|x| *x != 0) {
                return Err(Error::Malformed("policy epoch"));
            }
            payment_wire::encode(
                &profile,
                &payment_wire::Originals {
                    payer: c.clone(),
                    certificate: cert.clone(),
                    request: step.request.clone(),
                    statement: step.statement,
                    receipt: step.receipt.clone(),
                    policy_epoch: u64::from_le_bytes(epoch[..8].try_into().unwrap()),
                    pending_root: u.ip.io[36],
                    credit_root: u.ip.io[37],
                },
                &u.ip,
                &u.is,
                &u.pp,
                &u.ps,
                u.r,
            )
            .map_err(Error::Rejected)?
        } else {
            Vec::new()
        };
        let output = super::encode(&OutputFrame {
            version: 1,
            head: head_bytes,
            payment,
        })?;
        let mut head = self.output_head(&output, parameters, c)?.0;
        head.custody_digest = actual.capsule_digest;
        let completion = CompletionRecord {
            frame: CompletionFrame {
                version: 1,
                parameters_id: parameters.id,
                wallet: prepared.capsule.frame.wallet,
                operation_id: actual.operation_id,
                capsule_digest: actual.capsule_digest,
                receipt: [
                    receipt.receipt.body.as_slice(),
                    receipt.receipt.signature.as_slice(),
                ]
                .concat(),
                output,
            },
        };
        completion.validate()?;
        Ok(CompletedOperation { completion, head })
    }
    fn verify_completion(
        &self,
        capsule: &RecoveryCapsule,
        record: &CompletionRecord,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation> {
        scope(parameters, &record.frame.parameters_id)?;
        record.validate()?;
        let prepared = self.resume(capsule, parameters)?;
        let receipt = HardwareAdvanceReceipt::verify_selected(
            &prepared,
            &record.frame.receipt[..338],
            record.frame.receipt[338..].try_into().unwrap(),
        )?;
        if receipt.capsule_digest != record.frame.capsule_digest
            || receipt.operation_id != record.frame.operation_id
        {
            return Err(Error::AuthorityMismatch("retained completion"));
        }
        let c = terms(&prepared.witness).0;
        let (mut head, payment) = self.output_head(&record.frame.output, parameters, c)?;
        if head.profile[full::HEAD_IO].to_repr() != capsule.frame.successor
            || head.profile[full::STATEMENT_IO]
                != crate::statement::digest(terms(&prepared.witness).1)
            || head.profile[full::RECEIPT_IO] != crate::receipt::digest(&receipt.receipt)
            || (!payment.is_empty()) != (capsule.frame.kind == 3)
        {
            return Err(Error::AuthorityMismatch(
                "completion exact selected operation",
            ));
        }
        head.custody_digest = receipt.capsule_digest;
        Ok(CompletedOperation {
            completion: record.clone(),
            head,
        })
    }
}

/// The full chain driver calls the actual sealed native API after its program
/// fixed point. Test software signatures model only the isolated fixture keys.
#[cfg(test)]
pub(crate) fn test_full_chain_native(
    chain: &crate::chain_fixture::Chain,
    pairs: &[full::UniformPair; 4],
    receiver_boot: &full::UniformPair,
    dimensions: ((usize, usize), (usize, usize)),
    artifacts: &std::path::Path,
) -> std::result::Result<(), String> {
    test_chain_native(chain, pairs, receiver_boot, dimensions, artifacts, true)
}

/// Candidate measurement only; the selected layout must also pass the complete
/// native containment test, including every independent invalid-payment case.
#[cfg(test)]
pub(crate) fn test_measure_chain_native(
    chain: &crate::chain_fixture::Chain,
    pairs: &[full::UniformPair; 4],
    receiver_boot: &full::UniformPair,
    dimensions: ((usize, usize), (usize, usize)),
    artifacts: &std::path::Path,
) -> std::result::Result<(), String> {
    test_chain_native(chain, pairs, receiver_boot, dimensions, artifacts, false)
}

#[cfg(test)]
fn test_chain_native(
    chain: &crate::chain_fixture::Chain,
    pairs: &[full::UniformPair; 4],
    receiver_boot: &full::UniformPair,
    dimensions: ((usize, usize), (usize, usize)),
    artifacts: &std::path::Path,
    complete_validation: bool,
) -> std::result::Result<(), String> {
    fn run(
        chain: &crate::chain_fixture::Chain,
        pairs: &[full::UniformPair; 4],
        receiver_boot: &full::UniformPair,
        dimensions: ((usize, usize), (usize, usize)),
        artifacts: &std::path::Path,
        complete_validation: bool,
    ) -> Result<()> {
        use iroha_data_model::sumeragi_finality::{
            SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture,
        };
        let fixture = NativeFinalityFixture::new_with_explicit_parameters();
        let finality = SumeragiCommitVerifierV1::new(&fixture.verifier())
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let programs: [Program; 4] = std::array::from_fn(|i| Program {
            primary: pairs[i].cp.clone(),
            secondary: pairs[i].cs.clone(),
        });
        let manifest = ParameterManifest {
            version: 1,
            scheme: chain.sender_context.root.scheme,
            relation: chain.sender_context.relation,
            primary: std::array::from_fn(|i| programs[i].primary.program_digest()),
            secondary: std::array::from_fn(|i| programs[i].secondary.program_digest()),
            dimensions_p: [dimensions.0.0 as u32, dimensions.0.1 as u32],
            dimensions_s: [dimensions.1.0 as u32, dimensions.1.1 as u32],
            finality_network: finality.network().to_string(),
            finality_chain: finality.chain_id().to_string(),
        };
        let manifest = super::encode(&manifest)?;
        let signature = bootstrap::sign(
            crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &manifest),
            &BigUint::from_bytes_be(&[0x11; 32]),
            "native test parameter manifest",
        );
        let parameters = ParameterSet::authenticate(
            programs,
            &manifest,
            signature,
            chain.sender_context.root.root.clone(),
            finality.clone(),
        )?;
        adversarial_tests::reject_wrong_program_digests(&parameters, &manifest)?;
        let bundle_bytes = parameters.export_program_bundle()?;
        {
            let path = artifacts;
            std::fs::create_dir_all(path).map_err(|e| Error::Rejected(e.to_string()))?;
            for (name, bytes) in [
                ("program-bundle.norito", bundle_bytes.as_slice()),
                ("parameter-manifest.norito", manifest.as_slice()),
                ("parameter-signature.bin", signature.as_slice()),
            ] {
                std::fs::write(path.join(name), bytes)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
            }
        }
        let bundle = ProgramBundle::decode(&bundle_bytes)?;
        let imported: [Program; 4] = (0..4)
            .map(|i| {
                Ok(Program {
                    primary: Arc::new(
                        UniformCircuit::import_program(&bundle.primary[i], true)
                            .map_err(Error::Rejected)?,
                    ),
                    secondary: Arc::new(
                        UniformCircuit::import_program(&bundle.secondary[i], false)
                            .map_err(Error::Rejected)?,
                    ),
                })
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| Error::Malformed("test programs"))?;
        let parameters = ParameterSet::authenticate(
            imported,
            &manifest,
            signature,
            chain.sender_context.root.root.clone(),
            finality,
        )?;
        let engine = NativeEngine::default();
        for i in [full::BOOTSTRAP, full::LOAD, full::SEND] {
            let bytes = encode_head(
                &parameters,
                &pairs[i],
                &chain.send.credential,
                &chain.sender_bootstrap.certificate,
            )?;
            engine.verify_head(&bytes, &chain.send.credential, &parameters)?;
        }
        let receive_head = encode_head(
            &parameters,
            &pairs[full::RECEIVE],
            &chain.receiver_bootstrap.credential,
            &chain.receiver_bootstrap.certificate,
        )?;
        engine.verify_head(
            &receive_head,
            &chain.receiver_bootstrap.credential,
            &parameters,
        )?;
        let sender_head = encode_head(
            &parameters,
            &pairs[full::LOAD],
            &chain.send.credential,
            &chain.sender_bootstrap.certificate,
        )?;
        let mut sender = engine.verify_head(&sender_head, &chain.send.credential, &parameters)?;
        // This unit fixture has no durable provider; the provider integration binds
        // its actual marker before producing the identical sealed predecessor.
        sender.custody_digest = [17; 32];
        let mut native_send = chain.send.clone();
        // Controls-off preparation derives one deterministic Core lower/upper.
        native_send.upper = native_send.lower;
        native_send.statement[25] = Fp::from(native_send.lower);
        native_send.receipt.body[210..242]
            .copy_from_slice(&crate::statement::digest(&native_send.statement).to_repr());
        let operation = ExactOperationWitness::Send(Box::new(native_send.clone()));
        let prepared = engine.prepare(&operation, &sender, &parameters)?;
        let capsule = prepared.capsule().clone();
        let frozen = capsule.encode()?;
        let restored = engine.resume(&capsule, &parameters)?;
        if restored.capsule().encode()? != frozen {
            return Err(Error::AuthorityMismatch("native exact resume"));
        }
        let signed = bootstrap::sign(
            super::field(&prepared.signing_message())?,
            &chain.sender_key.d,
            "native Send fixture hardware boundary",
        );
        let receipt =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signed)?;
        eprintln!("native full-chain: finalizing Send through actual Engine");
        let completed = engine.finalize(prepared, &receipt, &parameters)?;
        let exact = completed.completion().encode()?;
        let verified = engine.verify_completion(&capsule, completed.completion(), &parameters)?;
        if verified.completion().encode()? != exact {
            return Err(Error::AuthorityMismatch("native retained Send bytes"));
        }
        let payment = completed.completion().output_part(1)?;
        let received = engine.verify_payment(&payment, &parameters)?;
        let mut changed = payment.clone();
        let at = changed.len() / 2;
        changed[at] ^= 1;
        if engine.verify_payment(&changed, &parameters).is_ok() {
            return Err(Error::AuthorityMismatch("native Payment tamper test"));
        }
        let boot = encode_head(
            &parameters,
            receiver_boot,
            &chain.receiver_bootstrap.credential,
            &chain.receiver_bootstrap.certificate,
        )?;
        let mut receiver =
            engine.verify_head(&boot, &chain.receiver_bootstrap.credential, &parameters)?;
        receiver.custody_digest = [19; 32];
        let mut actual_send = native_send;
        actual_send.receipt = received.receipt().clone();
        let witness = crate::receive_fixture::from_send(
            &chain.receiver_bootstrap,
            &chain.receiver_key,
            &actual_send,
            received.exact_pair(),
            *receiver.profile(),
        )
        .map_err(Error::Malformed)?;
        let prepared = engine.prepare(
            &ExactOperationWitness::Receive {
                step: Box::new(witness),
                incoming_payment: payment.clone().into(),
            },
            &receiver,
            &parameters,
        )?;
        let capsule = prepared.capsule().clone();
        let signed = bootstrap::sign(
            super::field(&prepared.signing_message())?,
            &chain.receiver_key.d,
            "native Receive fixture hardware boundary",
        );
        let receipt =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signed)?;
        eprintln!("native full-chain: finalizing Receive through actual Engine");
        let completed = engine.finalize(prepared, &receipt, &parameters)?;
        engine.verify_completion(&capsule, completed.completion(), &parameters)?;
        if !completed.completion().output_part(1)?.is_empty() {
            return Err(Error::AuthorityMismatch("Receive has no outbound Payment"));
        }
        // Same fixed profile, noncanonical R: its exact bytes enter the total
        // parser and the authenticated Receive relation must derive Burn.
        let trusted = parameters.inner.profile(full::SEND)?;
        let payment_profile = payment_wire::Profile {
            pair: &trusted,
            scheme: parameters.inner.scheme,
            relation: parameters.inner.relation,
        };
        let mut raw = received.exact_pair().to_vec();
        raw[42..74].fill(255);
        let malformed = payment_wire::test_replace_pair(&payment_profile, &payment, raw.clone())
            .map_err(Error::Rejected)?;
        let candidate = engine.incoming_candidate(&malformed, &parameters)?;
        if candidate.valid || candidate.pair_bytes != raw {
            return Err(Error::AuthorityMismatch(
                "native malformed candidate exact originals",
            ));
        }
        let witness = crate::receive_fixture::from_send_with_validity(
            &chain.receiver_bootstrap,
            &chain.receiver_key,
            &actual_send,
            &raw,
            *receiver.profile(),
            false,
        )
        .map_err(Error::Malformed)?;
        let prepared = engine.prepare(
            &ExactOperationWitness::Receive {
                step: Box::new(witness),
                incoming_payment: malformed.clone().into(),
            },
            &receiver,
            &parameters,
        )?;
        let capsule = prepared.capsule().clone();
        let resumed = engine.resume(&capsule, &parameters)?;
        let ExactOperationWitness::Receive {
            incoming_payment, ..
        } = resumed.witness.as_ref()
        else {
            return Err(Error::AuthorityMismatch("native Burn resume kind"));
        };
        if incoming_payment.as_ref() != malformed.as_slice() {
            return Err(Error::AuthorityMismatch(
                "native Burn retains malformed originals",
            ));
        }
        let signed = bootstrap::sign(
            super::field(&prepared.signing_message())?,
            &chain.receiver_key.d,
            "native Burn fixture hardware boundary",
        );
        let receipt =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signed)?;
        eprintln!("native full-chain: finalizing malformed-R Burn through actual Engine");
        let burned = engine.finalize(prepared, &receipt, &parameters)?;
        let verified = engine.verify_completion(&capsule, burned.completion(), &parameters)?;
        if verified.head.profile
            [crate::bootstrap_context::LINEAGE_IO + crate::bootstrap_context::BURNED_TOTAL]
            != Fp::from(10)
        {
            return Err(Error::AuthorityMismatch(
                "native malformed Payment must burn ten",
            ));
        }
        if complete_validation {
            adversarial_tests::burn_and_successors(
                &engine,
                &parameters,
                chain,
                &receiver,
                &sender,
                &actual_send,
                &payment,
            )?;
            eprintln!("native full-chain: sealed prepare/finalize/verify/resume/retention passed");
        } else {
            eprintln!(
                "native candidate measurement: positive Send/Receive and malformed-R Burn passed; full containment gate not run"
            );
        }
        Ok(())
    }
    run(
        chain,
        pairs,
        receiver_boot,
        dimensions,
        artifacts,
        complete_validation,
    )
    .map_err(|e| e.to_string())
}
