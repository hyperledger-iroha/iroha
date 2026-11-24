//! Native authority for the complete future recursive backend.
//!
//! The immutable four- or five-kind program set and its
//! commitment-generator/table profile are pinned by a manifest signed by an
//! Artifact-role key that the installed scheme root certified; installation
//! recomputes every pin. Every accepted output is verified against those
//! installed programs, with both native matrix accumulators and both inherited
//! generator accumulators decided (`verify_pair`), and every proof operation
//! takes fresh OS entropy. Custody remains owned by the canonical Advance
//! provider.
use super::*;
use crate::{
    carrier, certificate, credential, deferral,
    fs::{ProofEntropy, Scalar},
    full,
    ipa::Gens,
    payment_wire,
    r1cs::{Circuit, Instance, Witness, WrittenWitness},
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
#[cfg(test)]
#[path = "archive_selector_signed_tests.rs"]
mod archive_selector_signed_tests;
#[cfg(test)]
#[path = "engine_head_reuse_tests.rs"]
mod head_reuse_tests;

#[path = "native_credential.rs"]
mod issued_credential;
pub use issued_credential::{CredentialIssuerParameters, credential_signing_message_for_body};
#[path = "credit_status_wire.rs"]
mod credit_status_wire;
#[path = "open_request_wire.rs"]
mod open_request_wire;
#[path = "native_request.rs"]
mod send_request;
#[path = "native_transfers.rs"]
mod transfers;
pub use credit_status_wire::VerifiedCreditStatus;
pub(super) use credit_status_wire::{MatrixQueryLayout, admit_common_program_geometry};
pub use open_request_wire::{OpenRequestPeer, decode_open_request_peer, encode_open_request_peer};
pub(super) use transfers::archive_sent::PreparedArchive;
pub use transfers::{
    SpartanWalletMap, SpartanWalletMaps, wallet_receive_replay_key, wallet_send_replay_key,
};

#[path = "native_activation.rs"]
mod activation;
pub use activation::{
    BootstrapActivationPackage, MAX_ACTIVATION_CLAIM_BYTES, VerifiedActivationClaim,
};

#[path = "native_unload.rs"]
mod unload_claim;
#[cfg(test)]
pub(crate) use unload_claim::tests::test_unload_claim;
pub use unload_claim::{MAX_UNLOAD_CLAIM_BYTES, UnloadDestinations, VerifiedUnloadClaim};

#[path = "native_epoch.rs"]
mod selected_epoch;

/// Both traces belong to this finalize call. The head came from the exact head
/// verifier and the incoming trace came from strict decoding of the exact
/// Payment that the native verifier accepted under these generators.
pub(crate) struct VerifiedReceiveFolds<'a> {
    generators: &'a full::PairGens,
    pairs: &'a [full::TracedPair; 2],
}

type FoldProof<'a, C> = (&'a spartan::Proof<C>, &'a crate::verifier::Trace<C>);

/// A single curve's generators, head and Payment from the same verified Receive.
pub(crate) struct VerifiedCurveInputs<'a, C: iroha_pasta::PastaCurve> {
    generators: &'a Gens<C>,
    proofs: [FoldProof<'a, C>; 2],
}

impl<'a, C: iroha_pasta::PastaCurve> VerifiedCurveInputs<'a, C> {
    pub(crate) fn into_parts(self) -> (&'a Gens<C>, [FoldProof<'a, C>; 2]) {
        (self.generators, self.proofs)
    }
}

impl<'a> VerifiedReceiveFolds<'a> {
    fn bind(
        parameters: &'a AuthenticatedParameters,
        payment: &VerifiedIncomingPayment,
        original: &[u8],
        pairs: &'a [full::TracedPair],
    ) -> Result<Self> {
        payment.require_parameters(parameters)?;
        let pairs: &[full::TracedPair; 2] = pairs
            .try_into()
            .map_err(|_| Error::Malformed("verified Receive fold proof count"))?;
        let [head, incoming] = pairs;
        if original != payment.exact_bytes()
            || head.incoming_original.is_some()
            || incoming.kind != full::SEND
            || incoming.incoming_original.as_deref() != Some(payment.exact_pair())
        {
            return Err(Error::AuthorityMismatch("verified Receive fold sources"));
        }
        Ok(Self {
            generators: &parameters.inner.generators,
            pairs,
        })
    }

    pub(crate) fn sources(&self) -> (&'a full::PairGens, &'a [full::TracedPair]) {
        (self.generators, self.pairs)
    }

    pub(crate) fn primary(&self) -> VerifiedCurveInputs<'a, Eq> {
        let [head, incoming] = self.pairs;
        VerifiedCurveInputs {
            generators: &self.generators.p,
            proofs: [
                (&head.proof_p, &head.tr_p),
                (&incoming.proof_p, &incoming.tr_p),
            ],
        }
    }

    pub(crate) fn secondary(&self) -> VerifiedCurveInputs<'a, Ep> {
        let [head, incoming] = self.pairs;
        VerifiedCurveInputs {
            generators: &self.generators.s,
            proofs: [
                (&head.proof_s, &head.tr_s),
                (&incoming.proof_s, &incoming.tr_s),
            ],
        }
    }
}

#[derive(Clone, Default)]
pub struct NativeEngine {
    // Shared by clones until the protected finality reader changes.
    selection_owner: Arc<()>,
    verified_payment: Arc<std::sync::Mutex<Option<AcceptedPayment>>>,
    verified_output: Arc<std::sync::Mutex<Option<AcceptedOutput>>>,
    selected_epoch: Option<SumeragiCommitVerifierV1>,
    time: Option<Arc<dyn TimeSource>>,
    debit: Option<Arc<dyn DebitAuthorizationSource>>,
    #[cfg(test)]
    isolated_unload_confirmation: bool,
}
impl sealed::Engine for NativeEngine {}

/// One positive verifier result per owner. A fresh engine always verifies independently.
struct AcceptedPayment {
    installation: Arc<()>,
    payment: VerifiedIncomingPayment,
}

/// The last completion output this owner's exact verifier accepted, with the
/// head it gave. Finalization verifies the output it built, and the staged
/// completion check of the same dispatch then presents the same bytes. Another
/// owner (a fresh engine, or one that selected another protected epoch),
/// installation, credential or output is verified in full, and so is every
/// other part of a completion record.
struct AcceptedOutput {
    owner: Arc<()>,
    installation: Arc<()>,
    output: Vec<u8>,
    head: VerifiedWalletHead,
}

/// The last wallet head an owner's exact verifier accepted under one installed
/// parameter instance, with its decoded proof pair. The cache is a field of that
/// `AuthenticatedParameters` instance, so it lives exactly as long as the
/// installation: dropping or replacing the installation drops the entry and the
/// circuits its pair shares with the instance's programs, and no engine holds
/// either. Head verification is a pure function of the exact head bytes and the
/// installed parameters, so the same owner, credential and byte-identical head
/// under the same instance give the same verdict and pair. Any other owner (a
/// fresh engine after a restart, or one that selected another protected epoch),
/// instance, credential or head is verified in full. The entry is never
/// mutated: a hit clones it, and a refusal never replaces it.
#[derive(Default)]
pub(crate) struct HeadCache {
    entry: std::sync::Mutex<Option<AcceptedHead>>,
    /// Full head decodes under this instance; tests count misses per instance.
    #[cfg(test)]
    decodes: std::sync::atomic::AtomicUsize,
}

struct AcceptedHead {
    owner: Arc<()>,
    head: VerifiedWalletHead,
    pair: full::UniformPair,
}

/// A reconstruction accepted the exact predecessor bytes and reproduced the capsule.
/// Keep the decoded proof only until this owner finalizes that same preparation.
pub(super) struct ResumedSelection {
    owner: Arc<()>,
    installation: Arc<()>,
    original: Arc<[u8]>,
    predecessor: Option<full::UniformPair>,
}

impl ResumedSelection {
    fn matches(&self, engine: &NativeEngine, parameters: &AuthenticatedParameters) -> bool {
        Arc::ptr_eq(&self.owner, &engine.selection_owner)
            && Arc::ptr_eq(&self.installation, &parameters.installation)
    }
}

pub(crate) struct Program {
    pub primary: Arc<UniformCircuit<Fp>>,
    pub secondary: Arc<UniformCircuit<Fq>>,
}
pub(crate) struct ParameterSet {
    pub programs: Box<[Program]>,
    // The immutable mounted programs passed the digest comparison with these
    // manifest pins. Each private operation circuit is still hashed afresh.
    program_digests: Box<[[[u8; 32]; 2]]>,
    pub generators: full::PairGens,
    pub scheme: [u8; 32],
    pub relation: [u8; 32],
    pub root: p256::Point,
    pub finality: SumeragiCommitVerifierV1,
}
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Parameters.v4")]
pub struct ParameterManifest {
    /// Exactly `PARAMETER_MANIFEST_VERSION`.
    pub version: u16,
    pub scheme: [u8; 32],
    /// The parameter identity: proof profiles carry it, and heads, capsules
    /// and completions are scoped by it.
    pub relation: [u8; 32],
    /// Canonical program digests, Receive, Send, Bootstrap, Control.
    pub primary: [[u8; 32]; 4],
    pub secondary: [[u8; 32]; 4],
    pub dimensions_p: [u32; 2],
    pub dimensions_s: [u32; 2],
    /// Commitment generators (research/CONSTRUCTION.md): the namespace, the
    /// hash-to-curve suite with its index and prefix rule, each side's
    /// capacity (log2: P's Vesta set, S's Pallas set) and the digest of each
    /// exact derived set, `g[0..N]`, `h` and `u`.
    pub generator_domain: String,
    pub hash_to_curve: String,
    pub generator_capacity_bits: [u32; 2],
    pub generator_digests: [[u8; 32]; 2],
    /// The payment key-table construction, column order included, and its
    /// known answer: the certified commitment (`x`, `y` little-endian) of the
    /// P-256 base point's tables.
    pub table_construction: String,
    pub table_vector: [[u8; 32]; 2],
    pub finality_network: String,
    pub finality_chain: String,
    /// The exact Artifact-role signer certificate original (body ‖ signature,
    /// 172 bytes) issued by the installed scheme root; its key signs the
    /// manifest. The root itself signs only certificates.
    pub signer_certificate: Vec<u8>,
}

/// Version 4 uses two fresh full-width LogUp challenges and four recursive
/// pairs with 64 ordinary inputs. Control's last two inputs select its mode.
pub const PARAMETER_MANIFEST_VERSION: u16 = 4;

/// The retired v1 manifest layout, kept only so that installation refuses it
/// by a typed error instead of a framing error, and so isolated fixtures can
/// re-issue a retired bundle's exact scope (`reissue_fixture_manifest`).
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Parameters.v1")]
pub(crate) struct RetiredManifestV1 {
    pub(crate) version: u16,
    pub(crate) scheme: [u8; 32],
    pub(crate) relation: [u8; 32],
    pub(crate) primary: [[u8; 32]; 4],
    pub(crate) secondary: [[u8; 32]; 4],
    pub(crate) dimensions_p: [u32; 2],
    pub(crate) dimensions_s: [u32; 2],
    pub(crate) finality_network: String,
    pub(crate) finality_chain: String,
}

/// Hash-to-curve suite and generator rule the installed parameters name. The
/// implementation is `iroha_pasta`'s `hash_to_curve`; changing either changes
/// every commitment and so the relation.
pub const HASH_TO_CURVE_SUITE: &str = "iroha_pasta hash_to_curve: BLAKE2b-512 XMD (DST prefix || '-' || curve ID || '_XMD:BLAKE2b_SSWU_RO_'), two field elements, simplified SWU on the 3-isogenous curve, sum, 3-isogeny; g[i] = H(domain, LE64 i) for i < N, h = H(domain, LE64 N), u = H(domain, LE64 N+1); a prefix keeps its parent's h and u";
/// Payment key-table construction the issuer and every wallet recompute.
pub const TABLE_CONSTRUCTION: &str = "P-256 comb m=16: B_k = 2^(16k) Q (k < 16); T_t[e] = sum_(j<8) (2 bit_j(e) - 1) B_(8t+j) (t < 2, e < 256); row r = 256 t + e holds 7 cells: r, x limbs of 88, 88, 80 bits, y limbs of 88, 88, 80 bits; C = sum cell_c(r) g[512 c + r] over the first 3584 Vesta generators, column-major, unblinded; encoded x || y little-endian";

/// Exact generator and table profile of two derived generator sets.
pub(crate) struct GeneratorProfile {
    pub capacity_bits: [u32; 2],
    pub digests: [[u8; 32]; 2],
    pub table_vector: [[u8; 32]; 2],
}

fn generator_set_digest<C: iroha_pasta::PastaCurve>(gens: &Gens<C>) -> Result<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    h.update(b"kagemusha-spartan/generators/v1\0");
    h.update(&(gens.g.len() as u64).to_le_bytes());
    for point in gens.g.iter().chain([&gens.h, &gens.u]) {
        let (x, y): (C::Base, C::Base) = Option::from(iroha_pasta::PastaAffine::coordinates(point))
            .ok_or(Error::Malformed("generator at infinity"))?;
        h.update(x.to_repr().as_ref());
        h.update(y.to_repr().as_ref());
    }
    Ok(*h.finalize().as_bytes())
}

fn point_bytes(point: &crate::ipa::Aff<Eq>) -> Result<[[u8; 32]; 2]> {
    let (x, y): (Fq, Fq) = Option::from(iroha_pasta::PastaAffine::coordinates(point))
        .ok_or(Error::Malformed("table commitment at infinity"))?;
    Ok([x.to_repr(), y.to_repr()])
}

/// Measure the profile of exact generator sets; installation recomputes it.
pub(crate) fn generator_profile(p: &Gens<Eq>, s: &Gens<Ep>) -> Result<GeneratorProfile> {
    let bits = |n: usize, least: usize| -> Result<u32> {
        if !n.is_power_of_two() || n < least {
            return Err(Error::Malformed("generator capacity"));
        }
        Ok(n.trailing_zeros())
    };
    Ok(GeneratorProfile {
        capacity_bits: [bits(p.g.len(), full_table_bases())?, bits(s.g.len(), 1)?],
        digests: [generator_set_digest(p)?, generator_set_digest(s)?],
        table_vector: point_bytes(&full::wallet_table_commitment(&p256::generator(), p))?,
    })
}

/// Generators the payment key-table commitment uses (7 cells × 512 rows).
fn full_table_bases() -> usize {
    crate::nonnative::WALLET_CELLS * crate::nonnative::WALLET_ROWS
}

/// An unsigned manifest for exact programs on exact generator sets.
/// `programs[kind]` is the (primary, secondary) canonical digest pair.
pub(crate) fn manifest_for(
    scheme: [u8; 32],
    relation: [u8; 32],
    programs: [[[u8; 32]; 2]; 4],
    dimensions: ((usize, usize), (usize, usize)),
    generators: (&Gens<Eq>, &Gens<Ep>),
    finality: (&str, &str),
    signer: &certificate::Certificate,
) -> Result<ParameterManifest> {
    let profile = generator_profile(generators.0, generators.1)?;
    let dims = |d: (usize, usize)| -> Result<[u32; 2]> {
        Ok([
            u32::try_from(d.0).map_err(|_| Error::ResourceLimit)?,
            u32::try_from(d.1).map_err(|_| Error::ResourceLimit)?,
        ])
    };
    Ok(ParameterManifest {
        version: PARAMETER_MANIFEST_VERSION,
        scheme,
        relation,
        primary: programs.map(|p| p[0]),
        secondary: programs.map(|p| p[1]),
        dimensions_p: dims(dimensions.0)?,
        dimensions_s: dims(dimensions.1)?,
        generator_domain: crate::ipa::GENERATOR_DOMAIN.into(),
        hash_to_curve: HASH_TO_CURVE_SUITE.into(),
        generator_capacity_bits: profile.capacity_bits,
        generator_digests: profile.digests,
        table_construction: TABLE_CONSTRUCTION.into(),
        table_vector: profile.table_vector,
        finality_network: finality.0.into(),
        finality_chain: finality.1.into(),
        signer_certificate: certificate_bytes(signer),
    })
}

/// The generator and table profile the manifest names must be exactly the
/// implementation's, at the programs' column capacities. Returns the derived
/// generator sets every verifier, prover and credential intake then uses.
pub(crate) fn authenticate_profile(
    m: &ParameterManifest,
    column_bits: [usize; 2],
) -> Result<(Gens<Eq>, Gens<Ep>)> {
    if m.generator_domain != crate::ipa::GENERATOR_DOMAIN {
        return Err(Error::AuthorityMismatch("generator namespace"));
    }
    if m.hash_to_curve != HASH_TO_CURVE_SUITE {
        return Err(Error::AuthorityMismatch("hash-to-curve suite"));
    }
    if m.table_construction != TABLE_CONSTRUCTION {
        return Err(Error::AuthorityMismatch("payment table construction"));
    }
    if m.generator_capacity_bits.map(|b| b as usize) != column_bits
        || (1usize << column_bits[0]) < full_table_bases()
    {
        return Err(Error::AuthorityMismatch("generator capacity"));
    }
    let p = Gens::<Eq>::new(1 << column_bits[0]);
    let s = Gens::<Ep>::new(1 << column_bits[1]);
    let profile = generator_profile(&p, &s)?;
    if profile.digests != m.generator_digests {
        return Err(Error::AuthorityMismatch("derived generator sets"));
    }
    if profile.table_vector != m.table_vector {
        return Err(Error::AuthorityMismatch("payment table known answer"));
    }
    Ok((p, s))
}

/// Isolated fixtures only: re-issue a retired v1 manifest in the current
/// format for its exact program bundle. The retired manifest must carry the
/// root's own v1 signature (`retired_signature`, under the root of
/// `root_secret`) and pin exactly the bundle's eight programs (each imported
/// and degree-checked as installation does). The result keeps its scheme,
/// relation, dimensions and finality scope and adds the generator/table
/// profile at the programs' capacities and a root-issued Artifact-role
/// certificate for `artifact_secret`. Returns the manifest bytes and their
/// Artifact-role signature, ready for `AuthenticatedParameters::install`.
#[cfg(any(test, feature = "device-fixture"))]
pub fn reissue_fixture_manifest(
    bundle: &[u8],
    retired_manifest: &[u8],
    retired_signature: [u8; 64],
    root_secret: &[u8; 32],
    artifact_secret: &[u8; 32],
) -> Result<(Vec<u8>, [u8; 64])> {
    let retired: RetiredManifestV1 = super::decode(retired_manifest)?;
    if retired.version != 1 || super::encode(&retired)? != retired_manifest {
        return Err(Error::Malformed("retired v1 parameter manifest"));
    }
    let root_secret = BigUint::from_bytes_be(root_secret);
    let root = p256::mul(&p256::generator(), &root_secret)
        .ok_or(Error::Malformed("fixture root secret"))?;
    verify_signature(
        &root,
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), retired_manifest),
        &retired_signature,
    )?;
    let bundle = ProgramBundle::decode(bundle)?;
    let mut pins = [[[0; 32]; 2]; 4];
    let mut columns = [0; 2];
    for kind in 0..4 {
        let primary = UniformCircuit::<Fp>::import_program(&bundle.primary[kind], true)
            .map_err(Error::Rejected)?;
        let secondary = UniformCircuit::<Fq>::import_program(&bundle.secondary[kind], false)
            .map_err(Error::Rejected)?;
        pins[kind] = [primary.program_digest(), secondary.program_digest()];
        if pins[kind] != [retired.primary[kind], retired.secondary[kind]] {
            return Err(Error::AuthorityMismatch("retired manifest program pins"));
        }
        columns = [primary.shape.cols(), secondary.shape.cols()];
    }
    let artifact_secret = BigUint::from_bytes_be(artifact_secret);
    let artifact_key = p256::mul(&p256::generator(), &artifact_secret)
        .ok_or(Error::Malformed("fixture artifact secret"))?;
    let signer = certificate::issue_fixture(
        &retired.scheme,
        certificate::Role::Artifact as u8,
        &artifact_key,
        5,
        &root_secret,
    );
    let (gp, gs) = (Gens::<Eq>::new(columns[0]), Gens::<Ep>::new(columns[1]));
    let manifest = manifest_for(
        retired.scheme,
        retired.relation,
        pins,
        (
            (
                retired.dimensions_p[0] as usize,
                retired.dimensions_p[1] as usize,
            ),
            (
                retired.dimensions_s[0] as usize,
                retired.dimensions_s[1] as usize,
            ),
        ),
        (&gp, &gs),
        (&retired.finality_network, &retired.finality_chain),
        &signer,
    )?;
    let bytes = super::encode(&manifest)?;
    let signature = bootstrap::sign(
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &bytes),
        &artifact_secret,
        "fixture parameter manifest",
    );
    Ok((bytes, signature))
}

impl AuthenticatedParameters {
    /// Require the ArchiveSent selector from this wallet's authenticated installation.
    pub fn require_archive_sent_program(&self) -> Result<()> {
        if self.inner.programs.len() == crate::deferral::MAX_KINDS {
            Ok(())
        } else {
            Err(Error::AuthorityMismatch(ARCHIVE_SENT_UNSIGNED))
        }
    }

    /// Early admission threshold derived from the signed program shapes.
    /// The streamed prover accounts for its actual circuit and scratch at each stage.
    /// This threshold measures available headroom separately from process peak memory.
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
        // Preserve the admission allowance for the dense witness and the larger of
        // the outer and inner round workspaces. Streaming and copy-on-write change
        // the allocations made for a particular program.
        let phase = |w: u64, other: u64, constraints: u64| {
            let outer = w
                .checked_mul(2)
                .and_then(|n| constraints.checked_mul(3).and_then(|c| n.checked_add(c)));
            let inner = w.checked_mul(3).and_then(|n| n.checked_add(constraints));
            outer
                .zip(inner)
                .map(|(o, i)| o.max(i))
                .and_then(|n| n.checked_add(other))
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
    /// Prepare the canonical public manifest and the 32-byte message the Artifact
    /// key signs with ECDSA P-256/SHA-256. Requires the complete original bundle
    /// from the compiled producer and its root-certified Artifact signer.
    /// This offline operation does not produce an authenticated installation.
    pub fn prepare_manifest_for_signing(
        program_bundle: &[u8],
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        artifact_certificate_transcript: &[u8],
        finality: SumeragiCommitVerifierV1,
    ) -> Result<(Vec<u8>, [u8; 32])> {
        if program_bundle.len() > super::MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        if norito::core::Header::read(program_bundle).is_ok_and(|header| {
            header.schema == norito::schema::identity::frame_hash::<super::ProgramBundleV2>()
        }) {
            return super::built_in_five_programs()
                .ok_or(Error::Unavailable("qualified five-kind compiler profile"))?
                .prepare_manifest_for_signing(
                    program_bundle,
                    scheme,
                    &finality,
                    artifact_certificate_transcript,
                );
        }
        scheme
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        if finality.network().as_bytes() != &scheme.network_id {
            return Err(Error::AuthorityMismatch("parameter ledger root"));
        }
        let scheme_id = scheme.scheme_id();
        let signer = read_certificate(artifact_certificate_transcript)?;
        certificate::authenticate_signer(
            &signer,
            &scheme_id,
            &super::point(scheme.scheme_root_key.as_ref())?,
            certificate::Role::Artifact,
        )
        .map_err(Error::AuthorityMismatch)?;

        let (pinned_p, pinned_s) = super::built_in_producer_programs()
            .ok_or(Error::Unavailable("compiled producer profile"))?;
        let bundle = ProgramBundle::decode(program_bundle)?;
        let (dp, ds) = super::PRODUCER_MATRIX_DIMENSIONS;
        // Import one tape at a time. Only pins checked against this binary can
        // select the size of either regenerated generator set.
        fn columns<S: Scalar>(
            originals: &[Vec<u8>; 4],
            pins: &[[u8; 32]; 4],
            primary: bool,
            dimensions: (usize, usize),
        ) -> Result<usize> {
            let mut columns = None;
            for (bytes, digest) in originals.iter().zip(pins) {
                let tape =
                    UniformCircuit::<S>::import_program(bytes, primary).map_err(Error::Rejected)?;
                if tape.program_digest() != *digest {
                    return Err(Error::AuthorityMismatch("compiled producer program"));
                }
                tape.check_declared_degrees().map_err(Error::Rejected)?;
                if tape.shape.sx > dimensions.0
                    || tape.shape.sz() > dimensions.1
                    || columns
                        .replace(tape.shape.cols())
                        .is_some_and(|n| n != tape.shape.cols())
                {
                    return Err(Error::Malformed("compiled producer geometry"));
                }
            }
            columns.ok_or(Error::Malformed("empty producer program set"))
        }
        let p = columns::<Fp>(&bundle.primary, &pinned_p, true, dp)?;
        let s = columns::<Fq>(&bundle.secondary, &pinned_s, false, ds)?;
        let manifest = manifest_for(
            scheme_id,
            scheme.relation_id,
            std::array::from_fn(|i| [pinned_p[i], pinned_s[i]]),
            (dp, ds),
            (&Gens::<Eq>::new(p), &Gens::<Ep>::new(s)),
            (&finality.network().to_string(), finality.chain_id()),
            &signer,
        )?;
        let bytes = super::encode(&manifest)?;
        let message = manifest_signing_message(&bytes).to_repr();
        Ok((bytes, message))
    }
    /// Authenticate the small signed manifest against the registered scheme,
    /// relation and genesis identity. The returned digest covers the exact
    /// manifest bytes. Program installation separately verifies all eight pins.
    pub fn authenticated_manifest(
        manifest_bytes: &[u8],
        signature: &[u8; 64],
        installed_scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
    ) -> Result<(ParameterManifest, [u8; 32])> {
        installed_scheme
            .validate()
            .map_err(|e| Error::Rejected(e.to_string()))?;
        let root = super::point(installed_scheme.scheme_root_key.as_ref())?;
        let (manifest, digest) = authenticate_manifest(manifest_bytes, signature, &root)?;
        let network: iroha_data_model::prelude::NetworkId = manifest
            .finality_network
            .parse()
            .map_err(|_| Error::Malformed("parameter finality network"))?;
        if manifest.scheme != installed_scheme.scheme_id()
            || manifest.relation != installed_scheme.relation_id
            || network.as_bytes() != &installed_scheme.network_id
            || network.to_string() != manifest.finality_network
        {
            return Err(Error::AuthorityMismatch("installed native scheme"));
        }
        Ok((manifest, digest))
    }
    /// Check every public program against its independently authenticated manifest.
    /// The offline carrier publisher releases each imported tape before the next
    /// curve so it does not need to retain the complete prover and generator sets.
    pub fn verify_program_originals(bundle: &[u8], manifest: &ParameterManifest) -> Result<()> {
        let bundle = ProgramBundle::decode(bundle)?;
        for (index, (primary, secondary)) in
            bundle.primary.into_iter().zip(bundle.secondary).enumerate()
        {
            let check_primary =
                UniformCircuit::<Fp>::import_program(&primary, true).map_err(Error::Rejected)?;
            if check_primary.program_digest() != manifest.primary[index] {
                return Err(Error::AuthorityMismatch("primary program original"));
            }
            drop(check_primary);
            let check_secondary =
                UniformCircuit::<Fq>::import_program(&secondary, false).map_err(Error::Rejected)?;
            if check_secondary.program_digest() != manifest.secondary[index] {
                return Err(Error::AuthorityMismatch("secondary program original"));
            }
        }
        Ok(())
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
        let (manifest, manifest_digest) =
            Self::authenticated_manifest(manifest_bytes, &signature, installed_scheme)?;
        if manifest.finality_network != finality.network().to_string()
            || manifest.finality_chain != finality.chain_id()
        {
            return Err(Error::AuthorityMismatch("parameter ledger root"));
        }
        let root = super::point(installed_scheme.scheme_root_key.as_ref())?;
        let bundle = ProgramBundle::decode(program_bundle)?;
        // Release each pair of encoded programs after import. Generator derivation
        // and fixed-key construction need only the authenticated program tapes.
        let programs = import_programs(bundle.primary, bundle.secondary)?;
        ParameterSet::from_authenticated_manifest(
            programs,
            manifest,
            manifest_digest,
            root,
            finality,
        )
    }
    /// Select the typed V5 verifier. The header alone grants no authority.
    pub fn is_five_manifest(bytes: &[u8]) -> bool {
        norito::core::Header::read(bytes).is_ok_and(|header| {
            header.schema
                == norito::core::schema_hash_for_name(
                    "iroha.research.kagemusha_spartan.native.Parameters.v5",
                )
        })
    }

    /// Verify the signed header selected by the World record and this build.
    pub fn governed_manifest(
        bytes: &[u8],
        signature: &[u8; 64],
        selected: &[u8; 32],
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
    ) -> Result<GovernedManifest> {
        if Self::is_five_manifest(bytes) {
            let header = super::ProgramProfileV2::authenticate_manifest_header(
                bytes, signature, selected, scheme,
            )?;
            let supported = super::built_in_five_programs()
                .ok_or(Error::Unavailable("qualified five-kind compiler profile"))?;
            if header.programs != supported {
                return Err(Error::AuthorityMismatch(
                    "five-kind compiled producer profile",
                ));
            }
            Ok(GovernedManifest::Five(header))
        } else {
            let (manifest, digest) = Self::authenticated_manifest(bytes, signature, scheme)?;
            if &digest != selected {
                return Err(Error::AuthorityMismatch("governed manifest digest"));
            }
            Ok(GovernedManifest::Four(manifest))
        }
    }

    /// Install with the independent manifest selection from the governed World record.
    pub fn install_governed(
        bundle: &[u8],
        manifest: &[u8],
        signature: [u8; 64],
        selected: &[u8; 32],
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<Self> {
        let header = Self::governed_manifest(manifest, &signature, selected, scheme)?;
        match header {
            GovernedManifest::Four(_) => {
                Self::install(bundle, manifest, signature, scheme, finality)
            }
            GovernedManifest::Five(header) => Self::install_five(
                bundle,
                manifest,
                signature,
                selected,
                &header.programs,
                scheme,
                finality,
            ),
        }
    }

    /// Mount the exact five-kind generation selected by the program publisher and
    /// authenticated under the registered scheme's Artifact key and local finality.
    pub fn install_five(
        bundle: &[u8],
        manifest: &[u8],
        signature: [u8; 64],
        selected_digest: &[u8; 32],
        producer: &super::ProgramProfileV2,
        scheme: &iroha_data_model::kagemusha::KagemushaWalletSchemeV1,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<Self> {
        if producer
            .primary
            .iter()
            .chain(&producer.secondary)
            .any(|pin| *pin == [0; 32])
        {
            return Err(Error::AuthorityMismatch("missing five-kind producer pin"));
        }
        let (programs, generators) = producer.import_signed_programs(
            bundle,
            manifest,
            &signature,
            selected_digest,
            scheme,
            &finality,
        )?;
        ParameterSet::mount(
            programs,
            [&producer.primary, &producer.secondary],
            scheme.scheme_id(),
            scheme.relation_id,
            *selected_digest,
            producer.dimensions,
            generators,
            super::point(scheme.scheme_root_key.as_ref())?,
            finality,
        )
    }

    /// Canonical verifier programs of this installation.
    pub fn export_program_bundle(&self) -> Result<Vec<u8>> {
        let mut primary = Vec::with_capacity(self.inner.programs.len());
        let mut secondary = Vec::with_capacity(self.inner.programs.len());
        for program in &self.inner.programs {
            primary.push(program.primary.export_program().map_err(Error::Rejected)?);
            secondary.push(
                program
                    .secondary
                    .export_program()
                    .map_err(Error::Rejected)?,
            );
        }
        if primary.len() == crate::deferral::MAX_KINDS {
            super::ProgramBundleV2 {
                version: 2,
                primary: primary.try_into().unwrap(),
                secondary: secondary.try_into().unwrap(),
            }
            .encode()
        } else {
            ProgramBundle {
                version: 1,
                primary: primary
                    .try_into()
                    .map_err(|_| Error::Malformed("installed program count"))?,
                secondary: secondary.try_into().unwrap(),
            }
            .encode()
        }
    }
}

/// Signed program metadata selected for validation or installation by a governed record.
pub enum GovernedManifest {
    Four(ParameterManifest),
    Five(super::AuthenticatedManifestHeaderV5),
}

impl GovernedManifest {
    pub fn finality_identity(&self) -> (&str, &str) {
        match self {
            Self::Four(m) => (&m.finality_network, &m.finality_chain),
            Self::Five(m) => (&m.finality_network, &m.finality_chain),
        }
    }

    pub fn verify_program_originals(&self, bundle: &[u8]) -> Result<()> {
        match self {
            Self::Four(m) => AuthenticatedParameters::verify_program_originals(bundle, m),
            Self::Five(m) => m.programs.verify_compiled_originals(bundle, m).map(drop),
        }
    }
}

fn import_programs<const N: usize>(
    primary: [Vec<u8>; N],
    secondary: [Vec<u8>; N],
) -> Result<[Program; N]> {
    primary
        .into_iter()
        .zip(secondary)
        .map(|(p, s)| {
            Ok(Program {
                primary: Arc::new(
                    UniformCircuit::import_program(&p, true).map_err(Error::Rejected)?,
                ),
                secondary: Arc::new(
                    UniformCircuit::import_program(&s, false).map_err(Error::Rejected)?,
                ),
            })
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| Error::Malformed("installed program count"))
}

impl ParameterSet {
    /// The compiler supplies real programs; the independent installation supplies
    /// its trusted scheme root and already authenticated native genesis reader.
    #[cfg(test)]
    pub(crate) fn authenticate(
        programs: [Program; 4],
        manifest_bytes: &[u8],
        signature: [u8; 64],
        root: p256::Point,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<AuthenticatedParameters> {
        let (manifest, manifest_digest) = authenticate_manifest(manifest_bytes, &signature, &root)?;
        Self::from_authenticated_manifest(programs, manifest, manifest_digest, root, finality)
    }
    fn from_authenticated_manifest(
        programs: [Program; 4],
        manifest: ParameterManifest,
        manifest_digest: [u8; 32],
        root: p256::Point,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<AuthenticatedParameters> {
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
        let pbits = programs[0].primary.shape.col_bits;
        let sbits = programs[0].secondary.shape.col_bits;
        for p in &programs {
            if p.primary.shape.col_bits != pbits
                || p.secondary.shape.col_bits != sbits
                || p.primary.io_len != 64
                || p.secondary.io_len != 64
            {
                return Err(Error::Malformed("parameter program geometry"));
            }
            // However a program was obtained, its matrix tape must have the
            // degrees every fold and bound over it assumes.
            p.primary
                .check_declared_degrees()
                .and(p.secondary.check_declared_degrees())
                .map_err(|e| Error::Rejected(format!("declared matrix degrees: {e}")))?;
        }
        // Refuse an otherwise signed generation that could only discover its
        // outgoing Payment is oversized after a hardware-selected debit.
        let send = &programs[full::SEND];
        let transport = carrier::TrustedProfile::from_parameters(
            manifest.relation,
            send.primary.shape,
            send.primary.as_ref(),
            send.secondary.shape,
            send.secondary.as_ref(),
            true,
        )
        .map_err(Error::Rejected)?;
        payment_wire::admit_send_profile(&transport).map_err(Error::Rejected)?;
        // Generators exactly as the signed profile names them, never derived
        // from program geometry alone.
        let (gp, gs) = authenticate_profile(&manifest, [pbits, sbits])?;
        Self::mount(
            Vec::from(programs).into_boxed_slice(),
            [&manifest.primary, &manifest.secondary],
            manifest.scheme,
            manifest.relation,
            manifest_digest,
            [manifest.dimensions_p, manifest.dimensions_s],
            (gp, gs),
            root,
            finality,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn mount<const KINDS: usize>(
        programs: Box<[Program]>,
        pins: [&[[u8; 32]; KINDS]; 2],
        scheme: [u8; 32],
        relation: [u8; 32],
        manifest_digest: [u8; 32],
        dimensions: [[u32; 2]; 2],
        gens: (Gens<Eq>, Gens<Ep>),
        root: p256::Point,
        finality: SumeragiCommitVerifierV1,
    ) -> Result<AuthenticatedParameters> {
        if programs.len() != KINDS {
            return Err(Error::Malformed("installed program count"));
        }
        let program_digests = pins[0]
            .iter()
            .zip(pins[1])
            .map(|(&primary, &secondary)| [primary, secondary])
            .collect();
        let (gp, gs) = gens;
        let keys_p = programs
            .iter()
            .map(|x| spartan::fixed_commitments(&x.primary.shape, &gp, x.primary.as_ref()))
            .collect();
        let keys_s = programs
            .iter()
            .map(|x| spartan::fixed_commitments(&x.secondary.shape, &gs, x.secondary.as_ref()))
            .collect();
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
        if (defer_p.x, defer_p.y) != dp || (defer_s.x, defer_s.y) != ds {
            return Err(Error::Malformed("parameter accumulator dimensions"));
        }
        let generators = full::PairGens {
            p: gp,
            s: gs,
            defer_p,
            defer_s,
            keys_p: Some(keys_p),
            keys_s: Some(keys_s),
        };
        let authenticated = AuthenticatedParameters {
            installation: Arc::new(()),
            verified_head: HeadCache::default(),
            id: relation,
            manifest_digest,
            confirmation: ConfirmationScope::Installed,
            inner: Arc::new(Self {
                programs,
                program_digests,
                generators,
                scheme,
                relation,
                root,
                finality,
            }),
        };
        let receive = authenticated.inner.profile(full::RECEIVE)?;
        credit_status_wire::admit_profile(&receive, &authenticated.inner.generators)
            .map_err(Error::Rejected)?;
        Ok(authenticated)
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

pub(super) fn manifest_signing_message(bytes: &[u8]) -> Fp {
    crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), bytes)
}

pub(super) fn authenticate_manifest(
    bytes: &[u8],
    signature: &[u8; 64],
    root: &p256::Point,
) -> Result<(ParameterManifest, [u8; 32])> {
    let manifest: ParameterManifest = match super::decode(bytes) {
        Ok(manifest) => manifest,
        Err(e) => {
            if super::decode::<RetiredManifestV1>(bytes).is_ok() {
                return Err(Error::AuthorityMismatch(
                    "retired v1 parameter manifest: no generator/table profile or Artifact-role signer",
                ));
            }
            return Err(e);
        }
    };
    if manifest.version != PARAMETER_MANIFEST_VERSION
        || super::encode(&manifest)? != bytes
        || manifest.relation == [0; 32]
        || manifest.scheme == [0; 32]
    {
        return Err(Error::Malformed("parameter manifest"));
    }
    // The root certifies an Artifact-role key for this scheme; that key, not
    // the root, signs artifacts (proposal §2.3, wire §3.1).
    let signer = read_certificate(&manifest.signer_certificate)?;
    let key = certificate::authenticate_signer(
        &signer,
        &manifest.scheme,
        root,
        certificate::Role::Artifact,
    )
    .map_err(Error::AuthorityMismatch)?;
    let m = manifest_signing_message(bytes);
    verify_signature(&key, m, signature)?;
    Ok((manifest, m.to_repr()))
}

pub(super) fn verify_signature(q: &p256::Point, m: Fp, sig: &[u8; 64]) -> Result<()> {
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
pub(super) fn read_certificate(b: &[u8]) -> Result<certificate::Certificate> {
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
    authenticate_credential_under(
        &parameters.scheme,
        &parameters.root,
        &parameters.generators.p,
        c,
        cert,
    )
}
/// Credential intake under the installed scope: an Enrollment-role issuer
/// certificate from the scheme root, the issuer's signature, and the certified
/// payment key-table commitment recomputed from the certified payment key on
/// the authenticated generators (`authenticate_profile`). A correctly signed
/// credential whose table commitment does not recompute is refused.
pub(super) fn authenticate_credential_under(
    scheme: &[u8; 32],
    root: &p256::Point,
    table_generators: &Gens<Eq>,
    c: &Credential,
    cert: &certificate::Certificate,
) -> Result<()> {
    if c.body[..2] != 1u16.to_le_bytes()
        || c.body[2..34] != scheme[..]
        || c.body[500..532] != certificate::certificate_digest(cert).to_repr()
    {
        return Err(Error::AuthorityMismatch("credential enrollment chain"));
    }
    let issuer =
        certificate::authenticate_signer(cert, scheme, root, certificate::Role::Enrollment)
            .map_err(Error::AuthorityMismatch)?;
    verify_signature(&issuer, credential::message(c), &c.signature)?;
    certified_table(table_generators, c)
}
/// The certified payment key-table commitment (credential bytes 195..259,
/// `x || y` little-endian) must be exactly `TABLE_CONSTRUCTION` of the
/// certified payment key on these generators.
fn payment_key_table(generators: &Gens<Eq>, key: &[u8]) -> Result<[[u8; 32]; 2]> {
    if generators.g.len() < full_table_bases() {
        return Err(Error::Malformed("payment table generators"));
    }
    let q = point(key)?;
    point_bytes(&full::wallet_table_commitment(&q, generators))
}
fn certified_table(generators: &Gens<Eq>, c: &Credential) -> Result<()> {
    let table = payment_key_table(generators, &c.body[130..195])?;
    if c.body[195..227] != table[0] || c.body[227..259] != table[1] {
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
        profile[crate::control::MODE_IO..crate::control::MODE_IO + 2]
            .try_into()
            .unwrap(),
    )
    .map_err(Error::Rejected)?;
    if out.primary().io != profile {
        return Err(Error::AuthorityMismatch("complete public projection"));
    }
    Ok(out)
}
fn assemble_pair(
    parameters: &ParameterSet,
    kind: usize,
    d: carrier::Decoded,
) -> Result<full::UniformPair> {
    let p = parameters
        .programs
        .get(kind)
        .ok_or(Error::Malformed("wallet kind"))?;
    Ok(full::UniformPair {
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
    })
}

fn verify_pair(
    parameters: &ParameterSet,
    kind: usize,
    decoded: carrier::Decoded,
) -> Result<full::UniformPair> {
    let u = assemble_pair(parameters, kind, decoded)?;
    let r = u.r;
    let rlc = move |_: &[u8; 32]| r;
    let vp = spartan::verify_linked_circuit(
        &u.cp.shape,
        &parameters.generators.p,
        &u.ip,
        &u.pp.to_bytes(),
        &Link::First { rho: u.rho, r },
        u.cp.as_ref(),
    )
    .map_err(|_| Error::Rejected("primary linked proof".into()))?;
    let vs = spartan::verify_linked_circuit(
        &u.cs.shape,
        &parameters.generators.s,
        &u.is,
        &u.ps.to_bytes(),
        &Link::Second { d: u.d, rlc: &rlc },
        u.cs.as_ref(),
    )
    .map_err(|_| Error::Rejected("secondary linked proof".into()))?;
    if vp != u.d || vs != u.rho {
        return Err(Error::Rejected("pair transcript binding".into()));
    }
    decide_accumulators(&parameters.generators, &u)?;
    Ok(u)
}

/// One deferred matrix accumulator, the first `n + 1` late outputs of a proof:
/// its point must have the polynomial's `n` coordinates and its value must be
/// the installed combined matrix polynomial's exact value there.
pub(crate) fn matrix_accumulator<S: Scalar>(poly: &deferral::MatPoly<S>, late: &[S]) -> bool {
    let n = poly.n();
    if late.len() < n + 1 {
        return false;
    }
    let claim = deferral::claim_of(&late[..n + 1]);
    claim.point.len() == n && poly.eval(&claim.point) == claim.value
}

/// The decisions a linked verifier's success does not imply: both deferred
/// matrix accumulators and both inherited generator accumulators. (Each linked
/// verification already decided its own IPA generator claim.) Template pairs
/// that a compiler warm-up builds from toy predecessors verify as linked
/// proofs under the final programs but carry false accumulators; every native
/// accepting boundary reaches this through `verify_pair` and refuses them.
pub(crate) fn decide_accumulators(
    generators: &full::PairGens,
    pair: &full::UniformPair,
) -> Result<()> {
    if !matrix_accumulator(&generators.defer_p, &pair.pp.late) {
        return Err(Error::Rejected("native P matrix accumulator".into()));
    }
    if !matrix_accumulator(&generators.defer_s, &pair.ps.late) {
        return Err(Error::Rejected("native S matrix accumulator".into()));
    }
    let traced = full::trace_uniform(pair.clone(), generators);
    let inherited_p =
        full::inherited_generator_claim(&traced.tr_p, &generators.p).map_err(Error::Malformed)?;
    if !crate::generator_claim::decide(&generators.p, &inherited_p) {
        return Err(Error::Rejected(
            "native P inherited generator accumulator".into(),
        ));
    }
    let inherited_s =
        full::inherited_generator_claim(&traced.tr_s, &generators.s).map_err(Error::Malformed)?;
    if !crate::generator_claim::decide(&generators.s, &inherited_s) {
        return Err(Error::Rejected(
            "native S inherited generator accumulator".into(),
        ));
    }
    Ok(())
}

/// Whether a native refusal is one of `decide_accumulators`'s.
#[cfg(test)]
pub(crate) fn is_accumulator_refusal(e: &Error) -> bool {
    matches!(e, Error::Rejected(m) if m.starts_with("native ") && m.ends_with(" accumulator"))
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
    // A head's kind must be one of the installed programs: the signed v4
    // generation's four, a five-program generation's ArchiveSent as well.
    if f.version != 1
        || usize::from(f.kind) >= parameters.inner.programs.len()
        || f.profile.len() != 64
        || super::encode(&f)? != bytes
    {
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
    // Version 1 carries a Payment (or no artifact). Version 2 is exclusive to a completed Unload claim.
    payment: Vec<u8>,
}
pub(super) fn retained_output_part(bytes: &[u8], part: u32) -> Result<Vec<u8>> {
    let out: OutputFrame = super::decode(bytes)?;
    if !matches!(out.version, 1 | 2) || super::encode(&out)? != bytes {
        return Err(Error::Malformed("retained output"));
    }
    match part {
        0 => Ok(out.head),
        1 if out.version == 1 => Ok(out.payment),
        1 if out.version == 2 => Ok(Vec::new()),
        3 if out.version == 2 => Ok(out.payment),
        _ => Err(Error::Malformed("retained output selector")),
    }
}
/// The signed eight-program generation (Receive, Send, Bootstrap, Control on both curves)
/// has no ArchiveSent selector. Only a newly signed generation carrying an ArchiveSent
/// program may admit that kind; until then every path that needs its program refuses here.
pub(crate) const ARCHIVE_SENT_UNSIGNED: &str = "signed generation has no ArchiveSent program";

fn receipt_mut(w: &mut ExactOperationWitness) -> &mut bootstrap::Receipt {
    match w {
        ExactOperationWitness::Bootstrap(x) => &mut x.input.receipt,
        ExactOperationWitness::Load { step, .. } => &mut step.receipt,
        ExactOperationWitness::RefreshTimeAnchor(x)
        | ExactOperationWitness::Unload { step: x, .. } => &mut x.receipt,
        ExactOperationWitness::Send(x) => &mut x.receipt,
        ExactOperationWitness::Receive { step, .. } => &mut step.receipt,
        ExactOperationWitness::ArchiveSent { step, .. } => &mut step.receipt,
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
        ExactOperationWitness::RefreshTimeAnchor(x) => (&x.credential, &x.statement, &x.receipt, 7),
        ExactOperationWitness::Unload { step: x, .. } => {
            (&x.credential, &x.statement, &x.receipt, 6)
        }
        ExactOperationWitness::Send(x) => (&x.credential, &x.statement, &x.receipt, 3),
        ExactOperationWitness::Receive { step, .. } => {
            (&step.credential, &step.statement, &step.receipt, 4)
        }
        ExactOperationWitness::ArchiveSent { step, .. } => {
            (&step.credential, &step.statement, &step.receipt, 5)
        }
    }
}
/// The installed program that proves this witness. Memory admission, the preparation
/// relation check, proving and completion verification all select it here.
fn kind(w: &ExactOperationWitness, installed: usize) -> Result<usize> {
    program_for_tag(terms(w).3, installed)
}

fn program_for_tag(tag: u8, installed: usize) -> Result<usize> {
    match tag {
        1 => Ok(full::BOOTSTRAP),
        2 | 6 | 7 => Ok(full::CONTROL),
        3 => Ok(full::SEND),
        4 => Ok(full::RECEIVE),
        5 if installed == crate::deferral::MAX_KINDS => Ok(full::ARCHIVE_SENT),
        5 => Err(Error::AuthorityMismatch(ARCHIVE_SENT_UNSIGNED)),
        _ => Err(Error::Malformed("capsule kind")),
    }
}
/// The kind's name in measurement lines.
fn kind_name(k: usize) -> &'static str {
    match k {
        full::RECEIVE => "Receive",
        full::SEND => "Send",
        full::BOOTSTRAP => "Bootstrap",
        full::ARCHIVE_SENT => "ArchiveSent",
        _ => "Control",
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
type PrivateRows<S> = (
    UniformCircuit<S>,
    Instance<S>,
    Witness<S>,
    Vec<crate::r1cs::RowFill>,
);

/// Builds the private circuit and witness of `wiring`. `prepare` runs on the
/// wiring's arrays first: the primary shares equal public template
/// coefficients there ([`share_wiring_coefficients`]); others pass
/// [`keep_wiring`]. The circuit is the same either way.
#[allow(clippy::too_many_arguments)]
fn build_private_rows<S: Scalar + zeroize::Zeroize>(
    wiring: full::Wiring<S>,
    label: &'static str,
    column_bits: usize,
    group: usize,
    io_len: usize,
    seed: &[u8],
    layout: Option<crate::uniform::Layout>,
    prepare: fn(&'static str, &mut [crate::uniform::Array<S>]),
) -> Result<PrivateRows<S>> {
    let build = match group {
        3 => UniformCircuit::<S>::build_rows_grouped_with_release::<3>,
        4 => UniformCircuit::<S>::build_rows_grouped_with_release::<4>,
        _ => return Err(Error::Malformed("selected sumcheck grouping")),
    };
    let (mut arrays, pi, counts, tables, copies) = wiring.finish();
    prepare(label, &mut arrays);
    Ok(build(
        label,
        arrays,
        pi,
        counts,
        tables,
        copies,
        column_bits,
        io_len,
        seed,
        layout,
        zeroize::Zeroize::zeroize,
    ))
}

/// A wiring left as it is before its build.
fn keep_wiring<S>(_label: &'static str, _arrays: &mut [crate::uniform::Array<S>]) {}

/// Let the wiring's equal public template coefficients share one allocation
/// before the build. The build is a native operation's heap peak: the arrays'
/// private values, the placed witness and the new circuit's structures are
/// live together, and the duplicate buffers were otherwise shared only after
/// it ([`UniformCircuit::share_template_coefficients`]).
fn share_wiring_coefficients<S: Scalar + std::hash::Hash>(
    label: &'static str,
    arrays: &mut [crate::uniform::Array<S>],
) {
    let before = crate::prof::memory_enabled().then(crate::prof::heap_in_use);
    crate::uniform::share_array_coefficients(arrays);
    if let Some(before) = before {
        println!(
            "wiring coefficient sharing {label}: heap in use {:.0} -> {:.0} MB",
            before as f64 / 1e6,
            crate::prof::heap_in_use() as f64 / 1e6
        );
    }
}

fn build<S: Scalar + zeroize::Zeroize>(
    w: full::Wiring<S>,
    target: &UniformCircuit<S>,
    entropy: &ProofEntropy,
    purpose: &[u8],
) -> Result<PrivateRows<S>> {
    build_prepared(w, target, entropy, purpose, keep_wiring)
}

/// [`build`] with a step on the wiring's arrays before the build.
fn build_prepared<S: Scalar + zeroize::Zeroize>(
    w: full::Wiring<S>,
    target: &UniformCircuit<S>,
    entropy: &ProofEntropy,
    purpose: &[u8],
    prepare: fn(&'static str, &mut [crate::uniform::Array<S>]),
) -> Result<PrivateRows<S>> {
    let seed = zeroize::Zeroizing::new(entropy.stream(purpose).scalar::<Fp>().to_repr());
    build_private_rows(
        w,
        target.label,
        target.shape.col_bits,
        target.shape.k,
        target.io_len,
        seed.as_ref(),
        Some(target.layout()),
        prepare,
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
impl<S: Scalar + zeroize::Zeroize> PrivateCircuit<S> {
    /// The built circuit without its builder's private values: provers and
    /// the relation check read only the witness.
    fn new(mut c: UniformCircuit<S>) -> Self {
        c.release_private_values();
        PrivateCircuit(c)
    }
}
/// Whether a private circuit compiles to its authenticated program (the
/// digest the manifest pins).
fn same_program<S: Scalar + zeroize::Zeroize>(
    private: &UniformCircuit<S>,
    program: &UniformCircuit<S>,
    installed_digest: &[u8; 32],
) -> Result<()> {
    if private.program_digest() == *installed_digest {
        return Ok(());
    }
    if crate::prof::enabled() {
        println!(
            "native program mismatch {}:\n  private  {}\n  program  {}",
            private.label,
            private.digest_summary(),
            program.digest_summary()
        );
    }
    Err(Error::AuthorityMismatch("operation compiled program"))
}
fn check_constraints(
    w: full::Wiring<Fp>,
    columns: usize,
    group: usize,
    source: Option<&dyn TimeSource>,
) -> Result<()> {
    use rayon::prelude::*;
    let stage = crate::prof::Stage::start();
    let entropy = ProofEntropy::fresh().map_err(|_| Error::Unavailable("OS proof entropy"))?;
    let seed = zeroize::Zeroizing::new(
        entropy
            .stream(b"preparation witness")
            .scalar::<Fp>()
            .to_repr(),
    );
    let (c, i, wit, rows) = build_private_rows(
        w,
        "native preparation",
        columns,
        group,
        192,
        seed.as_ref(),
        None,
        keep_wiring,
    )?;
    let c = PrivateCircuit::new(c);
    let wit = WrittenWitness::new(wit, rows);
    stage.mark("build");
    if crate::prof::memory_enabled() {
        println!(
            "native preparation accounting sx {} sw {}: preparation {:.0} MB, retained {:.0} MB",
            c.shape.sx,
            c.shape.sw(),
            c.preparation_memory_bytes() as f64 / 1e6,
            c.proving_memory_accounting().retained_bytes as f64 / 1e6
        );
    }
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
    let mut rng = entropy.stream(b"preparation relation check");
    let ch = [rng.scalar(), rng.scalar()];
    // The second phase writes its own copies of the rows it fills; the
    // caller's rows are read in place, and rows it never wrote not at all.
    let mut rows = crate::kern::Rows::with_fill(&wit.w, c.shape.cols(), wit.rows());
    let rel = c.instantiate_rows(&c.shape, &ch, &mut rows);
    stage.mark("instantiate");
    // A * B = C chunk by chunk of constraint rows: no z copy and no whole
    // Az, Bz, Cz tables.
    let zv = crate::kern::ZView {
        rows: &rows,
        tail: &i.io,
    };
    assert!(
        rel.abc_ranges(&c.shape, &zv, &[], [false; 3], &mut Default::default()),
        "a uniform relation streams its rows"
    );
    let total = 1usize << c.shape.sx;
    let holds = (0..total.div_ceil(crate::uniform::PREPARATION_CHECK_ROWS))
        .into_par_iter()
        .all(|task| {
            let range = task * crate::uniform::PREPARATION_CHECK_ROWS
                ..((task + 1) * crate::uniform::PREPARATION_CHECK_ROWS).min(total);
            let mut out: [Vec<Fp>; 3] = std::array::from_fn(|_| vec![Fp::ZERO; range.len()]);
            rel.abc_ranges(&c.shape, &zv, &[range], [true; 3], &mut out);
            let ok = out[0]
                .iter()
                .zip(&out[1])
                .zip(&out[2])
                .all(|((a, b), c)| *a * b == *c);
            out.iter_mut()
                .for_each(|v| v.iter_mut().for_each(zeroize::Zeroize::zeroize));
            ok
        });
    stage.mark("check");
    drop(rel);
    rows.zeroize_owned();
    drop(rows);
    drop((wit, c));
    stage.mark("drop");
    stage.finish("preparation check");
    if holds {
        Ok(())
    } else {
        Err(Error::Rejected(
            "native monetary preparation constraints".into(),
        ))
    }
}
impl NativeEngine {
    pub(super) fn funding_commit_reader<'a>(
        &'a self,
        parameters: &'a AuthenticatedParameters,
    ) -> &'a SumeragiCommitVerifierV1 {
        self.selected_epoch
            .as_ref()
            .unwrap_or(&parameters.inner.finality)
    }

    /// Admits a proving stage: its accounted `workspace` plus the process MSM
    /// scratch ceiling must fit the memory now available. Private commitments
    /// use the secret Pippenger MSMs, whose digit and bucket scratch is
    /// admitted within that ceiling; no commitment table is built, so none is
    /// charged.
    fn require_stage_memory(&self, workspace: u64) -> Result<()> {
        if let Some(source) = &self.time {
            let required = workspace
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
    /// Reuse an Execute caller's reconstructed witness only while this installation
    /// and its protected finality reader still match. The provider checks the exact
    /// capsule and every live control again at the irreversible boundary.
    pub fn retain_reconstruction(
        &self,
        prepared: PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Option<PreparedOperation> {
        self.resumed_here(&prepared, parameters).then_some(prepared)
    }

    fn resumed_here(
        &self,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> bool {
        prepared
            .resumed_under
            .as_ref()
            .is_some_and(|resume| resume.matches(self, parameters))
    }

    fn accepted_head(
        &self,
        bytes: &[u8],
        credential: &Credential,
        parameters: &AuthenticatedParameters,
    ) -> Option<(VerifiedWalletHead, full::UniformPair)> {
        let held = parameters.verified_head.entry.lock().ok()?;
        let accepted = held.as_ref()?;
        (Arc::ptr_eq(&accepted.owner, &self.selection_owner)
            && accepted.head.require_parameters(parameters).is_ok()
            && exact_credential(&accepted.head.credential, credential)
            && accepted.head.pair[..] == *bytes)
            .then(|| (accepted.head.clone(), accepted.pair.clone()))
    }

    fn verified_head_and_pair(
        &self,
        bytes: &[u8],
        credential: &Credential,
        parameters: &AuthenticatedParameters,
    ) -> Result<(VerifiedWalletHead, full::UniformPair)> {
        if let Some(accepted) = self.accepted_head(bytes, credential, parameters) {
            return Ok(accepted);
        }
        #[cfg(test)]
        parameters
            .verified_head
            .decodes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (_, profile, c, cert, pair) = decode_head(bytes, parameters)?;
        if !exact_credential(&c, credential) {
            return Err(Error::AuthorityMismatch("requested head credential"));
        }
        let head = VerifiedWalletHead {
            parameters_id: parameters.id,
            manifest_digest: parameters.manifest_digest,
            profile,
            pair: bytes.into(),
            credential: c,
            certificate: cert,
            custody_digest: [0; 32],
            enrollment_marker: None,
        };
        if let Ok(mut held) = parameters.verified_head.entry.lock() {
            *held = Some(AcceptedHead {
                owner: Arc::clone(&self.selection_owner),
                head: head.clone(),
                pair: pair.clone(),
            });
        }
        Ok((head, pair))
    }

    fn predecessor_for_proof(
        &self,
        prepared: &mut PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Result<full::UniformPair> {
        let original = &prepared.predecessor.pair;
        if let Some(resume) = prepared.resumed_under.as_mut()
            && resume.matches(self, parameters)
            && Arc::ptr_eq(&resume.original, original)
            && let Some(pair) = resume.predecessor.take()
        {
            return Ok(pair);
        }
        decode_head(original, parameters).map(|(_, _, _, _, pair)| pair)
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
    fn accepted_payment(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Option<VerifiedIncomingPayment> {
        if let Ok(held) = self.verified_payment.lock()
            && let Some(accepted) = held.as_ref()
            && Arc::ptr_eq(&accepted.installation, &parameters.installation)
            && accepted.payment.require_parameters(parameters).is_ok()
            && accepted.payment.exact_bytes() == bytes
        {
            return Some(accepted.payment.clone());
        }
        None
    }

    fn payment_candidate(
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<(carrier::TrustedProfile, payment_wire::Candidate)> {
        let trusted = parameters.inner.profile(full::SEND)?;
        let profile = payment_wire::Profile {
            pair: &trusted,
            scheme: parameters.inner.scheme,
            relation: parameters.inner.relation,
        };
        let candidate = payment_wire::decode_candidate(&profile, bytes).map_err(Error::Rejected)?;
        Ok((trusted, candidate))
    }

    fn verify_payment_candidate(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
        trusted: &carrier::TrustedProfile,
        d: &payment_wire::Candidate,
    ) -> Result<VerifiedIncomingPayment> {
        let pair = carrier::decode_projected_pair(trusted, &d.pair_bytes, &d.projection)
            .map_err(Error::Rejected)?;
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
        verify_pair(&parameters.inner, full::SEND, pair)?;
        let payment = VerifiedIncomingPayment {
            parameters_id: parameters.id,
            manifest_digest: parameters.manifest_digest,
            bytes: bytes.into(),
            profile,
            digest: d.digests.payment,
            request: d.originals.request.clone(),
            payer: d.originals.payer.clone(),
            statement: d.originals.statement,
            receipt: d.originals.receipt.clone(),
            pair: d.pair_bytes.as_slice().into(),
        };
        if let Ok(mut held) = self.verified_payment.lock() {
            *held = Some(AcceptedPayment {
                installation: Arc::clone(&parameters.installation),
                payment: payment.clone(),
            });
        }
        Ok(payment)
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
        // Only the native verifier can issue this capability. Monetary admission
        // also checks the receiver, its Request, and its consumed-credit map.
        let verified = match self.verify_payment(bytes, parameters) {
            Ok(payment) => Some(payment),
            Err(
                e @ (Error::ResourceLimit | Error::NotEnoughMemory { .. } | Error::Unavailable(_)),
            ) => {
                return Err(e);
            }
            Err(_) => None,
        };
        let decoded = if let Some(payment) = verified.as_ref() {
            if payment.exact_pair() != d.pair_bytes.as_slice() {
                return Err(Error::AuthorityMismatch("verified Receive proof bytes"));
            }
            carrier::decode_projected_pair(&trusted, payment.exact_pair(), &d.projection)
        } else {
            carrier::decode_projected_candidate(&trusted, &d.pair_bytes, &d.projection)
        }
        .map_err(Error::Rejected)?;
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
            payment_digest: d.digests.payment,
            profile,
            proof_digest: d.digests.proof,
            verified,
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
    /// Authenticate an exact fee schedule original (body ‖ signature, 255
    /// bytes) and its RegulatoryPolicy-role signer certificate (172 bytes)
    /// under the installed scheme root, for the wallet's asset. Policy
    /// installation must pass this before a schedule's digest may be held;
    /// Send opens the held schedule only by that digest. Returns the digest.
    pub fn authenticate_fee_schedule_originals(
        &self,
        schedule: &[u8],
        certificate: &[u8],
        asset: &[u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<Fp> {
        use crate::fee_schedule::{BODY_BYTES, SignedFeeSchedule};
        if schedule.len() != BODY_BYTES + 64 {
            return Err(Error::Malformed("fee schedule length"));
        }
        let input = SignedFeeSchedule {
            body: schedule[..BODY_BYTES].try_into().unwrap(),
            signature: schedule[BODY_BYTES..].try_into().unwrap(),
        };
        let signer = read_certificate(certificate)?;
        crate::fee_schedule::authenticate(
            &input,
            &signer,
            &parameters.inner.scheme,
            asset,
            &parameters.inner.root,
        )
        .map_err(Error::AuthorityMismatch)
    }
    /// The payment key signs after selection. Its signature cannot establish prior
    /// user presence for a Send or Unload. A protected exact-review authorization
    /// must be available at both debit boundaries before an installed wallet can spend.
    fn require_debit_confirmation(&self, tag: u8, scope: ConfirmationScope) -> Result<()> {
        match (tag, scope) {
            #[cfg(any(test, feature = "device-fixture"))]
            (3, ConfirmationScope::FixtureSend) => Ok(()),
            (3, _) => Err(Error::AuthorityMismatch(
                "protected Send review confirmation unavailable",
            )),
            #[cfg(test)]
            (6, ConfirmationScope::FixtureSend) if self.isolated_unload_confirmation => Ok(()),
            (6, _) => Err(Error::AuthorityMismatch(
                "protected Unload review confirmation unavailable",
            )),
            // ArchiveSent proves delivery of an existing debit and moves no value.
            (1 | 2 | 4 | 5 | 7, _) => Ok(()),
            _ => Err(Error::Malformed("capsule kind")),
        }
    }

    #[cfg(test)]
    pub(crate) fn isolated_unload_fixture() -> Self {
        Self {
            isolated_unload_confirmation: true,
            ..Self::default()
        }
    }

    /// Install the stock wallet's protected custody adapter. The engine verifies the
    /// actual hardware signature and complete review independently at both boundaries.
    pub fn with_protected_custody(
        time: Arc<dyn TimeSource>,
        debit: Arc<dyn DebitAuthorizationSource>,
    ) -> Self {
        Self {
            time: Some(time),
            debit: Some(debit),
            ..Self::default()
        }
    }

    fn authorize_debit(
        &self,
        boundary: DebitBoundary,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        if let (ConfirmationScope::Installed, 3 | 6, Some(custody)) = (
            parameters.confirmation,
            prepared.capsule().kind(),
            &self.debit,
        ) {
            return custody
                .evidence(boundary, prepared, parameters)?
                .verify(prepared, parameters);
        }
        self.require_debit_confirmation(prepared.capsule().kind(), parameters.confirmation)
    }

    /// Bind the canonical local custody/OS clock adapter. Controls-off operations
    /// use `Default` and never acquire a network or clock dependency.
    pub fn with_time_source(time: Arc<dyn TimeSource>) -> Self {
        Self {
            time: Some(time),
            ..Self::default()
        }
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
        let operation_id = step.receipt.body[114..146].try_into().unwrap();
        let (anchor, now) = if fresh {
            source.observe(&wallet, &operation_id)?
        } else {
            source.prepared(&wallet, &operation_id)?
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
        check_constraints(w, 13, 3, self.time.as_deref())
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
        input.input.credential.enrollment_fields()?;
        authenticate_credential(
            &parameters.inner,
            &input.input.credential,
            &input.input.certificate,
        )?;
        let mut profile = [Fp::ZERO; 64];
        profile[0] = Fp::ONE;
        Ok(VerifiedWalletHead {
            parameters_id: parameters.id,
            manifest_digest: parameters.manifest_digest,
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
    ) -> Result<Option<Box<PreparedReceive>>> {
        let p = &parameters.inner;
        let (c, _, _, _) = terms(witness);
        if !exact_credential(c, &head.credential) {
            return Err(Error::AuthorityMismatch("prepared current credential"));
        }
        authenticate_credential(p, c, &head.certificate)?;
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let predecessor = full::native_predecessor(&mut w, family, head);
        let mut receive = None;
        match witness {
            ExactOperationWitness::Bootstrap(input) => {
                let marker = head
                    .enrollment_marker
                    .ok_or(Error::AuthorityMismatch("bootstrap enrollment origin"))?;
                if input.scheme != p.scheme
                    || input.relation != p.relation
                    || input.root != p.root
                    || input.input.statement[19..21] != super::funding::limbs(&marker)
                    || !head.pair.is_empty()
                {
                    return Err(Error::AuthorityMismatch("bootstrap enrollment origin"));
                }
                let prepared = bootstrap::prepare_unsigned(&p.context(), &input.input)
                    .map_err(Error::Malformed)?;
                return check_constraints(
                    prepared.wiring,
                    p.programs[full::BOOTSTRAP].primary.shape.col_bits,
                    p.programs[full::BOOTSTRAP].primary.shape.k,
                    self.time.as_deref(),
                )
                .map(|()| None);
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
                    .verify_with(
                        &mut self.funding_commit_reader(parameters).clone(),
                        &receipt,
                    )
                    .map_err(|e| Error::Rejected(e.to_string()))?;
                crate::control::append_preparation(
                    &mut w,
                    family,
                    &p.root,
                    &crate::control::Witness::load((**step).clone()),
                    &predecessor,
                )
                .map_err(Error::Malformed)?;
            }
            ExactOperationWitness::Unload { step, destinations } => {
                if step.mode != crate::control::Mode::Unload {
                    return Err(Error::AuthorityMismatch("Unload mode"));
                }
                destinations.verify(&step.credential, &step.statement, &p.scheme, &p.root)?;
                crate::control::append_preparation(&mut w, family, &p.root, step, &predecessor)
                    .map_err(Error::Malformed)?;
            }
            ExactOperationWitness::RefreshTimeAnchor(step) => {
                if step.mode != crate::control::Mode::RefreshTimeAnchor {
                    return Err(Error::AuthorityMismatch("RefreshTimeAnchor mode"));
                }
                let wallet = c.body[66..98].try_into().unwrap();
                let anchor =
                    crate::time_anchor::authenticate(&step.authority, &p.scheme, &wallet, &p.root)
                        .map_err(Error::AuthorityMismatch)?;
                let (expected_core, expected_rest) = crate::time_anchor::refresh(
                    &step.before_core,
                    &step.before_rest,
                    &anchor,
                    step.after_core[29],
                )
                .map_err(Error::AuthorityMismatch)?;
                if expected_core != step.after_core || expected_rest != step.after_rest {
                    return Err(Error::AuthorityMismatch("signed TimeAnchor successor"));
                }
                crate::control::append_preparation(&mut w, family, &p.root, step, &predecessor)
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
                let relation = crate::receive::append_preparation(
                    &mut w,
                    family,
                    step,
                    &predecessor,
                    &incoming,
                )
                .map_err(Error::Malformed)?;
                // The constrained relation conjoins the incoming proof, signed objects, issuer/payer/receiver
                // scope, Request signature and consumed-credit nonmembership. A retained first credit-map
                // entry still owns its digest even if a historic Burn left the consumed map unchanged.
                let can_credit = relation.valid().value(&w) == Fp::ONE
                    && step.maps.credit_search.leaf.key != step.statement[17];
                receive = Some(Box::new(PreparedReceive {
                    verified: candidate.verified,
                    pair: candidate.pair,
                    pair_bytes: candidate.pair_bytes,
                    can_credit,
                }));
            }
            ExactOperationWitness::ArchiveSent { step, .. } => {
                let prior = crate::monetary_state::append_state(
                    &mut w,
                    family,
                    &step.before_core,
                    &step.before_rest,
                );
                let current =
                    crate::operation_auth::append_current(&mut w, family, c, &prior, &predecessor)
                        .map_err(Error::Malformed)?;
                let statement = crate::statement::append_statement(
                    &mut w,
                    family,
                    crate::statement::Kind::Archive,
                    &step.statement,
                );
                crate::receipt::append_preparation(
                    &mut w,
                    family,
                    &step.receipt.body,
                    &current,
                    &statement,
                    crate::statement::Kind::Archive,
                    crate::receipt::PaymentDigest::Absent,
                )
                .map_err(Error::Malformed)?;
            }
        }
        let shape = p.programs[kind(witness, p.programs.len())?].primary.shape;
        check_constraints(w, shape.col_bits, shape.k, self.time.as_deref())?;
        Ok(receive)
    }
    // The private head already carries the installation that accepted its proof
    // or enrollment. Cold recovery reconstructs it before restoring custody.
    fn prepare_retained(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        head.require_parameters(parameters)?;
        let stage = crate::prof::Stage::start();
        if !matches!(witness, ExactOperationWitness::Bootstrap(_)) {
            if head.custody_digest == [0; 32] || head.enrollment_marker.is_some() {
                return Err(Error::AuthorityMismatch("released predecessor custody"));
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
                version: RECOVERY_CAPSULE_VERSION,
                parameters_id: parameters.id,
                manifest_digest: parameters.manifest_digest,
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
        stage.mark("head");
        let archive = if matches!(&frozen, ExactOperationWitness::ArchiveSent { .. }) {
            program_for_tag(5, parameters.inner.programs.len())?;
            Some(self.archive_proving_inputs(&frozen, head, parameters)?)
        } else {
            None
        };
        let receive = self.check_preparation(&frozen, head, parameters)?;
        stage.mark("preparation");
        stage.finish(&format!(
            "prepare {}",
            kind_name(kind(&frozen, parameters.inner.programs.len())?)
        ));
        let body = receipt_mut(&mut frozen).body;
        Ok(PreparedOperation {
            resumed_under: None,
            manifest_digest: parameters.manifest_digest,
            parameters_id: parameters.id,
            capsule,
            witness: Arc::new(frozen),
            receipt_body: body,
            receive,
            archive,
            predecessor: head.clone(),
        })
    }
    fn output_head(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
        c: &Credential,
    ) -> Result<(VerifiedWalletHead, OutputFrame)> {
        let out: OutputFrame = super::decode(bytes)?;
        if !matches!(out.version, 1 | 2) || super::encode(&out)? != bytes {
            return Err(Error::Malformed("completion output"));
        }
        if let Ok(held) = self.verified_output.lock()
            && let Some(accepted) = held.as_ref()
            && Arc::ptr_eq(&accepted.owner, &self.selection_owner)
            && Arc::ptr_eq(&accepted.installation, &parameters.installation)
            && accepted.head.require_parameters(parameters).is_ok()
            && exact_credential(&accepted.head.credential, c)
            && accepted.output == bytes
        {
            return Ok((accepted.head.clone(), out));
        }
        let head = self.verify_head(&out.head, c, parameters)?;
        if out.version == 1 && !out.payment.is_empty() {
            let incoming = self.verify_payment(&out.payment, parameters)?;
            if incoming.profile != head.profile {
                return Err(Error::AuthorityMismatch("released Payment head"));
            }
        }
        if let Ok(mut held) = self.verified_output.lock() {
            *held = Some(AcceptedOutput {
                owner: Arc::clone(&self.selection_owner),
                installation: Arc::clone(&parameters.installation),
                output: bytes.to_vec(),
                head: head.clone(),
            });
        }
        Ok((head, out))
    }
}

impl Engine for NativeEngine {
    fn require_selected_debit(
        &self,
        prepared: &PreparedOperation,
        checkpoint: [u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        if prepared.manifest_digest != parameters.manifest_digest {
            return Err(Error::AuthorityMismatch("prepared producer manifest"));
        }
        self.authorize_debit(
            DebitBoundary::AnchoredSelection(checkpoint),
            prepared,
            parameters,
        )
    }

    fn prepare_selection(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        let kind = program_for_tag(capsule.kind(), parameters.inner.programs.len())?;
        self.require_minimum_memory(parameters, kind)?;
        let prepared = self.resume(capsule, parameters)?;
        self.recheck_selection(&prepared, parameters)?;
        Ok(prepared)
    }

    fn recheck_selection(
        &self,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        if !self.resumed_here(prepared, parameters) {
            return Err(Error::AuthorityMismatch("resumed selection authority"));
        }
        self.require_minimum_memory(
            parameters,
            kind(&prepared.witness, parameters.inner.programs.len())?,
        )?;
        prepared.require_receive_admission()?;
        // A resumed preparation must bind its frozen interval to the operation's actual
        // capture before fresh policy checking and selection. Selected recovery bypasses both.
        self.check_send_time(&prepared.witness, false)?;
        self.check_send_time(&prepared.witness, true)?;
        self.authorize_debit(DebitBoundary::BeforeSelection, prepared, parameters)
    }

    fn verify_head(
        &self,
        bytes: &[u8],
        credential: &Credential,
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedWalletHead> {
        self.verified_head_and_pair(bytes, credential, parameters)
            .map(|(head, _)| head)
    }
    fn verify_payment(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedIncomingPayment> {
        if let Some(payment) = self.accepted_payment(bytes, parameters) {
            return Ok(payment);
        }
        let (trusted, candidate) = Self::payment_candidate(bytes, parameters)?;
        self.verify_payment_candidate(bytes, parameters, &trusted, &candidate)
    }
    fn prepare(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        if let ExactOperationWitness::ArchiveSent { step, .. } = witness {
            if parameters.inner.programs.len() != crate::deferral::MAX_KINDS {
                self.check_archive_sent(witness, head, parameters)?;
                return Err(Error::AuthorityMismatch(ARCHIVE_SENT_UNSIGNED));
            }
            if step.receipt.body[242..306] != [0; 64] || step.receipt.signature != [0; 64] {
                return Err(Error::AuthorityMismatch("ArchiveSent fresh receipt"));
            }
        }
        self.require_minimum_memory(parameters, kind(witness, parameters.inner.programs.len())?)?;
        self.check_send_time(witness, false)?;
        self.authenticate_send_request(witness, parameters)?;
        let prepared = self.prepare_retained(witness, head, parameters)?;
        prepared.require_receive_admission()?;
        Ok(prepared)
    }
    fn resume(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation> {
        scope(parameters, &capsule.frame.parameters_id)?;
        if capsule.frame.manifest_digest != parameters.manifest_digest {
            return Err(Error::AuthorityMismatch("recovery producer manifest"));
        }
        capsule.validate()?;
        program_for_tag(capsule.frame.kind, parameters.inner.programs.len())?;
        let r: RecoveryFrame = super::decode(&capsule.frame.recovery)?;
        if r.version != 1 || r.profile.len() != 64 || super::encode(&r)? != capsule.frame.recovery {
            return Err(Error::Malformed("canonical recovery payload"));
        }
        let witness = super::witness_codec::decode_witness(&r.witness)?;
        self.authenticate_send_request(&witness, parameters)?;
        let c = read_credential(&r.credential)?;
        let cert = read_certificate(&r.certificate)?;
        let profile: [Fp; 64] = r
            .profile
            .iter()
            .map(field)
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .unwrap();
        let (mut head, predecessor) = if let ExactOperationWitness::Bootstrap(b) = &witness {
            let origin = self.enrollment_origin(
                b,
                r.enrollment.ok_or(Error::Malformed("enrollment marker"))?,
                parameters,
            )?;
            (origin, None)
        } else {
            let (head, pair) = self.verified_head_and_pair(&r.predecessor, &c, parameters)?;
            (head, Some(pair))
        };
        if head.profile != profile
            || !exact_credential(&head.credential, &c)
            || certificate_bytes(&head.certificate) != certificate_bytes(&cert)
        {
            return Err(Error::AuthorityMismatch("recovered predecessor"));
        }
        head.custody_digest = r.previous_capsule;
        let mut prepared = self.prepare_retained(&witness, &head, parameters)?;
        if prepared.capsule.encode()? != capsule.encode()? {
            return Err(Error::AuthorityMismatch("recovered frozen capsule"));
        }
        prepared.resumed_under = Some(Box::new(ResumedSelection {
            owner: Arc::clone(&self.selection_owner),
            installation: Arc::clone(&parameters.installation),
            original: Arc::clone(&head.pair),
            predecessor,
        }));
        Ok(prepared)
    }
    fn finalize_selected(
        &self,
        mut prepared: PreparedOperation,
        receipt: &HardwareAdvanceReceipt,
        parameters: &AuthenticatedParameters,
    ) -> Result<(PreparedOperation, CompletedOperation)> {
        scope(parameters, &prepared.parameters_id)?;
        if prepared.manifest_digest != parameters.manifest_digest {
            return Err(Error::AuthorityMismatch("prepared producer manifest"));
        }
        // A selected capsule can resume in a colder, smaller process than the
        // one that prepared it. Check again before cloning its private witness.
        self.require_minimum_memory(
            parameters,
            kind(&prepared.witness, parameters.inner.programs.len())?,
        )?;
        self.authorize_debit(DebitBoundary::AfterSelection, &prepared, parameters)?;
        let stage = crate::prof::Stage::start();
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
        let k = kind(&witness, parameters.inner.programs.len())?;
        let mut pairs = Vec::new();
        if k != full::BOOTSTRAP {
            let u = self.predecessor_for_proof(&mut prepared, parameters)?;
            pairs.push(full::trace_uniform(u, &p.generators));
        }
        stage.mark("predecessor trace");
        if k == full::RECEIVE {
            // Preparation and cold resume derive this pair and its verdict
            // from the same frozen Payment before checking its constraints.
            let incoming = prepared
                .receive
                .as_ref()
                .ok_or(Error::Malformed("missing prepared Receive"))?;
            pairs.push(full::trace_incoming_candidate(
                incoming.pair.clone(),
                &p.generators,
                incoming.pair_bytes.clone(),
            ));
            stage.mark("incoming trace");
        }
        if k == full::ARCHIVE_SENT {
            let incoming = prepared
                .archive
                .as_ref()
                .ok_or(Error::Malformed("missing prepared ArchiveSent"))?;
            pairs.push(full::trace_incoming_candidate(
                incoming.pair.clone(),
                &p.generators,
                incoming.pair_bytes.clone(),
            ));
        }
        let entropy = ProofEntropy::fresh().map_err(|_| Error::Unavailable("OS proof entropy"))?;
        let verified_payment = prepared
            .receive
            .as_ref()
            .and_then(|receive| receive.verified.as_ref());
        let folds = match (&witness, verified_payment) {
            (ExactOperationWitness::Bootstrap(_), _) => None,
            (
                ExactOperationWitness::Receive {
                    incoming_payment, ..
                },
                Some(payment),
            ) => {
                let sources =
                    VerifiedReceiveFolds::bind(parameters, payment, incoming_payment, &pairs)?;
                Some(
                    full::GeneratorFolds::prepare_verified_receive(&sources, &entropy)
                        .map_err(Error::Malformed)?,
                )
            }
            _ => Some(
                full::GeneratorFolds::prepare(&p.generators, &pairs, &entropy)
                    .map_err(Error::Malformed)?,
            ),
        };
        stage.mark("folds");
        let build_control = |control: &crate::control::Witness| -> Result<_> {
            Ok((
                full::p_operation(
                    full::OperationWitness::Control {
                        root: &p.root,
                        witness: control,
                    },
                    &p.generators,
                    &pairs,
                    folds.as_ref().expect("Control has predecessor folds"),
                )
                .map_err(Error::Malformed)?
                .0,
                Some(control.mode),
            ))
        };
        let (wp, control_mode) = match &witness {
            ExactOperationWitness::Bootstrap(input) => (
                full::p_boot_from_inputs(&p.generators, &p.context(), &input.input)
                    .map_err(Error::Malformed)?
                    .0,
                None,
            ),
            ExactOperationWitness::Load { step, .. } => {
                build_control(&crate::control::Witness::load((**step).clone()))?
            }
            ExactOperationWitness::RefreshTimeAnchor(step)
            | ExactOperationWitness::Unload { step, .. } => build_control(step)?,
            ExactOperationWitness::Send(step) => (
                full::p_operation(
                    full::OperationWitness::Send(step),
                    &p.generators,
                    &pairs,
                    folds.as_ref().unwrap(),
                )
                .map_err(Error::Malformed)?
                .0,
                None,
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
                None,
            ),
            ExactOperationWitness::ArchiveSent { step, .. } => (
                prepared
                    .archive
                    .as_ref()
                    .ok_or(Error::Malformed("missing prepared ArchiveSent"))?
                    .primary(
                        step,
                        &prepared.predecessor,
                        &p.generators,
                        &pairs,
                        folds.as_ref().expect("ArchiveSent folds"),
                    )?,
                None,
            ),
        };
        stage.mark("primary wiring");
        // The primary's templates are shared before its build (its build was
        // this operation's heap peak) and again after it, for the rows the
        // build adds. The secondary's are shared the same way below.
        let (mut cp, ip, wp, rows_p) = build_prepared(
            wp,
            p.programs[k].primary.as_ref(),
            &entropy,
            b"primary witness",
            share_wiring_coefficients,
        )?;
        cp.share_template_coefficients();
        // TODO: build the primary's constraint templates after the secondary
        // proof. The first-phase commitments need only the witness, the row
        // bounds and the public support; the templates (about 97 MB for the
        // Receive primary) are first read by `instantiate_rows` in the primary
        // proof. The installed program cannot lend them: an imported program
        // holds only its compact tape. Splitting `build_rows_grouped_with_release`
        // into a layout and witness pass and a later template pass (from the
        // wiring's structure, retained without its private values) would take
        // them out of the first phase and the secondary proof; the primary
        // build's own peak falls only if the template pass leaves it too.
        if crate::prof::enabled() {
            println!(
                "native circuits {} P: {}",
                kind_name(k),
                cp.template_census()
            );
        }
        let cp = PrivateCircuit::new(cp);
        let wp = WrittenWitness::new(wp, rows_p);
        stage.mark("primary build");
        // Take the primary prefix: its first-phase row commitments are kept
        // for the primary proof below, which regenerates the masks and blinds
        // from the same stream and does not commit those rows again.
        //
        // The secondary's wiring and build read only the folds, the traced
        // pairs and the public generators, never the primary witness, and
        // draw from their own entropy stream. They run alongside the
        // commitments, whose MSM kernel keeps the pool busy: the build is
        // mostly single-threaded. The secondary's arrays then coexist
        // with the primary's first phase, below the later proof peaks, where
        // both circuits are resident anyway.
        self.require_stage_memory(cp.first_phase_memory_bytes())?;
        let (primary, secondary) = rayon::join(
            || {
                spartan::retain_linked_witness(
                    &cp.shape,
                    &p.generators.p,
                    &ip,
                    spartan::ProverWitness::Owned(wp),
                    &mut entropy.stream(b"primary"),
                    &*cp,
                )
            },
            || {
                let ws = match folds.as_ref() {
                    Some(folds) if k == full::ARCHIVE_SENT => {
                        full::s_archive_operation(&p.generators, &pairs, folds).0
                    }
                    Some(folds) => {
                        full::s_operation(k, control_mode, &p.generators, &pairs, folds).0
                    }
                    None => full::s_boot(&p.generators).0,
                };
                build_prepared(
                    ws,
                    p.programs[k].secondary.as_ref(),
                    &entropy,
                    b"secondary witness",
                    share_wiring_coefficients,
                )
            },
        );
        let d = primary.digest();
        stage.mark("phase1");
        drop(folds);
        drop(pairs);
        let (mut cs, is, ws, rows_s) = secondary?;
        // Both circuits are live through the secondary proof: share the
        // secondary's equal template coefficients too.
        cs.share_template_coefficients();
        let cs = PrivateCircuit::new(cs);
        let ws = WrittenWitness::new(ws, rows_s);
        stage.mark("build");
        // Each private circuit must compile to its authenticated program.
        // The secondary's is small and checked now; the primary's is checked
        // once its proof is made and its witness and prover tables are gone,
        // before anything is verified or released.
        same_program(&cs, &p.programs[k].secondary, &p.program_digests[k][1])?;
        // The primary wiring and both builds are freed: return their pages
        // before the secondary proof's peak (Apple; a no-op elsewhere).
        crate::prof::release_free_memory();
        stage.mark("digest");
        let rlc = |_: &[u8; 32]| [0; 32];
        self.require_stage_memory(cs.phase_memory_accounting().additional_peak_bytes)?;
        let (ps, rho, r) = spartan::prove_linked_witness(
            &cs.shape,
            &p.generators.s,
            &is,
            spartan::ProverWitness::Owned(ws),
            &mut entropy.stream(b"secondary"),
            &Link::Second { d, rlc: &rlc },
            &*cs,
        );
        // Verification uses the independently authenticated public programs.
        // Retaining the secondary's private arrays/plans through the primary
        // proof only increased memory; the completion needs its proof and IO.
        drop(cs);
        crate::prof::release_free_memory();
        stage.mark("s-proof");
        self.require_stage_memory(cp.phase_memory_accounting().additional_peak_bytes)?;
        let (pp, d2, _) = primary.prove_first(rho, r);
        if d != d2 {
            return Err(Error::Rejected("primary preparation replay".into()));
        }
        stage.mark("p-proof");
        // The primary's witness and prover tables are gone: return their pages
        // before the program compile allocates its own (Apple; a no-op
        // elsewhere).
        crate::prof::release_free_memory();
        same_program(&cp, &p.programs[k].primary, &p.program_digests[k][0])?;
        drop(cp);
        crate::prof::release_free_memory();
        stage.mark("p-digest");
        if crate::prof::enabled() {
            println!(
                "native proofs {}: P {} S {}",
                kind_name(k),
                blake3::hash(&pp.to_bytes()).to_hex(),
                blake3::hash(&ps.to_bytes()).to_hex()
            );
        }
        // Verify the canonical head and outgoing artifact together in output_head
        // before constructing the completion. No local proof grants authority here.
        let u = assemble_pair(
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
        let (version, artifact) =
            if let ExactOperationWitness::Unload { step, destinations } = &witness {
                let head = self.verify_head(&head_bytes, c, parameters)?;
                let receipt = [
                    step.receipt.body.as_slice(),
                    step.receipt.signature.as_slice(),
                ]
                .concat();
                let claim = self.unload_claim(
                    &head,
                    &step.statement.map(|x| x.to_repr()),
                    &receipt,
                    destinations.as_ref().clone(),
                    parameters,
                )?;
                (2, claim)
            } else {
                (1, payment)
            };
        let output = super::encode(&OutputFrame {
            version,
            head: head_bytes,
            payment: artifact,
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
        stage.mark("output");
        stage.finish(&format!("finalize {}", kind_name(k)));
        Ok((prepared, CompletedOperation { completion, head }))
    }
    fn verify_staged_completion(
        &self,
        prepared: &PreparedOperation,
        record: &CompletionRecord,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation> {
        scope(parameters, &record.frame.parameters_id)?;
        record.validate()?;
        let reconstructed;
        let prepared = if self.resumed_here(prepared, parameters) {
            prepared
        } else {
            reconstructed = self.resume(prepared.capsule(), parameters)?;
            &reconstructed
        };
        let capsule = prepared.capsule();
        if record.frame.wallet != capsule.frame.wallet {
            return Err(Error::AuthorityMismatch("retained completion wallet"));
        }
        let receipt = HardwareAdvanceReceipt::verify_selected(
            prepared,
            &record.frame.receipt[..338],
            record.frame.receipt[338..].try_into().unwrap(),
        )?;
        if receipt.capsule_digest != record.frame.capsule_digest
            || receipt.operation_id != record.frame.operation_id
        {
            return Err(Error::AuthorityMismatch("retained completion"));
        }
        let c = terms(&prepared.witness).0;
        let (mut head, output) = self.output_head(&record.frame.output, parameters, c)?;
        let exact_artifact = if let ExactOperationWitness::Unload { step, destinations } =
            prepared.witness.as_ref()
        {
            output.version == 2
                && self.unload_claim(
                    &head,
                    &step.statement.map(|x| x.to_repr()),
                    &record.frame.receipt,
                    destinations.as_ref().clone(),
                    parameters,
                )? == output.payment
        } else {
            output.version == 1 && (!output.payment.is_empty()) == (capsule.frame.kind == 3)
        };
        let installed = parameters.inner.programs.len();
        let expected_kind = program_for_tag(capsule.frame.kind, installed)?;
        let expected_corner = crate::deferral::kind_corner(installed, expected_kind)
            .map_err(|_| Error::Malformed("completion program selector"))?
            .map(Fp::from);
        let actual_corner =
            [full::KIND_IO, full::KIND_IO + 1, full::KIND_C2_IO].map(|word| head.profile[word]);
        if actual_corner != expected_corner
            || head.profile[full::HEAD_IO].to_repr() != capsule.frame.successor
            || head.profile[full::STATEMENT_IO]
                != crate::statement::digest(terms(&prepared.witness).1)
            || head.profile[full::RECEIPT_IO] != crate::receipt::digest(&receipt.receipt)
            || !exact_artifact
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
    fn verify_completion(
        &self,
        capsule: &RecoveryCapsule,
        record: &CompletionRecord,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation> {
        self.verify_released_operation(capsule, record, parameters)
            .map(|(_, completed)| completed)
    }
}

impl NativeEngine {
    /// Verify the released proof and signed hardware receipt, retaining the
    /// same checked private opening for subsequent wallet projections.
    pub fn verify_released_operation(
        &self,
        capsule: &RecoveryCapsule,
        record: &CompletionRecord,
        parameters: &AuthenticatedParameters,
    ) -> Result<(PreparedOperation, CompletedOperation)> {
        // Refuse malformed or foreign records before the cold preparation.
        scope(parameters, &record.frame.parameters_id)?;
        record.validate()?;
        let stage = crate::prof::Stage::start();
        let prepared = self.resume(capsule, parameters)?;
        stage.mark("resume");
        let completed = self.verify_staged_completion(&prepared, record, parameters)?;
        stage.mark("output");
        stage.finish(&format!(
            "completion {}",
            kind_name(kind(&prepared.witness, parameters.inner.programs.len())?)
        ));
        Ok((prepared, completed))
    }
}

/// The fixture scheme root's test secret (`bootstrap::fixture`).
#[cfg(test)]
pub(crate) const TEST_ROOT_SECRET: [u8; 32] = [0x11; 32];
/// The test Artifact-role signer's secret; the root certifies its key.
#[cfg(test)]
pub(crate) const TEST_ARTIFACT_SECRET: [u8; 32] = [0x15; 32];
/// The root-issued Artifact-role certificate of `TEST_ARTIFACT_SECRET`.
#[cfg(test)]
pub(crate) fn test_artifact_certificate(scheme: &[u8; 32]) -> certificate::Certificate {
    let key = p256::mul(
        &p256::generator(),
        &BigUint::from_bytes_be(&TEST_ARTIFACT_SECRET),
    )
    .expect("test artifact key");
    certificate::issue_fixture(
        scheme,
        certificate::Role::Artifact as u8,
        &key,
        5,
        &BigUint::from_bytes_be(&TEST_ROOT_SECRET),
    )
}
/// The test Artifact-role signature over exact manifest bytes.
#[cfg(test)]
pub(crate) fn test_sign_manifest(bytes: &[u8]) -> [u8; 64] {
    bootstrap::sign(
        crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), bytes),
        &BigUint::from_bytes_be(&TEST_ARTIFACT_SECRET),
        "native test parameter manifest",
    )
}

/// The full chain driver calls the actual sealed native API after its program
/// fixed point. Test software signatures model only the isolated fixture keys.
/// `templates` are construction artifacts over the final programs, each with
/// whether its accumulators hold under them (decided by the caller).
#[cfg(test)]
pub(crate) fn test_full_chain_native(
    chain: &crate::chain_fixture::Chain,
    pairs: &[full::UniformPair; 4],
    receiver_boot: &full::UniformPair,
    templates: &[(full::UniformPair, bool)],
    dimensions: ((usize, usize), (usize, usize)),
    artifacts: &std::path::Path,
) -> std::result::Result<(), String> {
    test_chain_native(
        chain,
        pairs,
        receiver_boot,
        templates,
        dimensions,
        artifacts,
        true,
    )
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
    test_chain_native(
        chain,
        pairs,
        receiver_boot,
        &[],
        dimensions,
        artifacts,
        false,
    )
}

#[cfg(test)]
#[test]
fn installed_debits_require_protected_confirmation() {
    let engine = NativeEngine::default();
    for kind in [3, 6] {
        assert!(matches!(
            engine.require_debit_confirmation(kind, ConfirmationScope::Installed),
            Err(Error::AuthorityMismatch(_))
        ));
        assert!(
            NativeEngine::isolated_unload_fixture()
                .require_debit_confirmation(kind, ConfirmationScope::Installed)
                .is_err()
        );
    }
    for kind in [0, 8, 255] {
        for scope in [ConfirmationScope::Installed, ConfirmationScope::FixtureSend] {
            assert!(matches!(
                engine.require_debit_confirmation(kind, scope),
                Err(Error::Malformed("capsule kind"))
            ));
        }
    }
    for kind in [1, 2, 4, 5, 7] {
        for scope in [ConfirmationScope::Installed, ConfirmationScope::FixtureSend] {
            engine.require_debit_confirmation(kind, scope).unwrap();
        }
    }
    engine
        .require_debit_confirmation(3, ConfirmationScope::FixtureSend)
        .unwrap();
    assert!(
        engine
            .require_debit_confirmation(6, ConfirmationScope::FixtureSend)
            .is_err()
    );
}

#[cfg(test)]
fn test_chain_native(
    chain: &crate::chain_fixture::Chain,
    pairs: &[full::UniformPair; 4],
    receiver_boot: &full::UniformPair,
    templates: &[(full::UniformPair, bool)],
    dimensions: ((usize, usize), (usize, usize)),
    artifacts: &std::path::Path,
    complete_validation: bool,
) -> std::result::Result<(), String> {
    fn run(
        chain: &crate::chain_fixture::Chain,
        pairs: &[full::UniformPair; 4],
        receiver_boot: &full::UniformPair,
        templates: &[(full::UniformPair, bool)],
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
        // The signed profile names exactly the generator sets every verifier,
        // prover and credential intake derives at the programs' capacities.
        let (gp, gs) = (
            Gens::<Eq>::new(programs[0].primary.shape.cols()),
            Gens::<Ep>::new(programs[0].secondary.shape.cols()),
        );
        let manifest = manifest_for(
            chain.sender_context.root.scheme,
            chain.sender_context.relation,
            std::array::from_fn(|i| {
                [
                    programs[i].primary.program_digest(),
                    programs[i].secondary.program_digest(),
                ]
            }),
            dimensions,
            (&gp, &gs),
            (&finality.network().to_string(), finality.chain_id()),
            &test_artifact_certificate(&chain.sender_context.root.scheme),
        )?;
        drop((gp, gs));
        let manifest = super::encode(&manifest)?;
        let signature = test_sign_manifest(&manifest);
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
        let mut parameters = ParameterSet::authenticate(
            imported,
            &manifest,
            signature,
            chain.sender_context.root.root.clone(),
            finality,
        )?;
        let engine = NativeEngine::default();
        for i in [full::BOOTSTRAP, full::CONTROL, full::SEND] {
            let bytes = encode_head(
                &parameters,
                &pairs[i],
                &chain.send.credential,
                &chain.sender_bootstrap.certificate,
            )?;
            engine.verify_head(&bytes, &chain.send.credential, &parameters)?;
        }
        adversarial_tests::reject_template_artifacts(&engine, &parameters, chain, templates)?;
        adversarial_tests::reject_wrong_certified_table(
            &engine,
            &parameters,
            chain,
            &pairs[full::CONTROL],
        )?;
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
            &pairs[full::CONTROL],
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
        // The restored capsule and genuine payment-key receipt still carry no user presence.
        for result in [
            engine.before_selection(&capsule, &parameters),
            engine.finalize(restored, &receipt, &parameters).map(|_| ()),
        ] {
            if !matches!(
                result,
                Err(Error::AuthorityMismatch(
                    "protected Send review confirmation unavailable"
                ))
            ) {
                return Err(Error::Rejected(
                    "installed Send confirmation boundary did not hold".into(),
                ));
            }
        }
        parameters.confirmation = ConfirmationScope::FixtureSend;
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
        if candidate.valid() || candidate.pair_bytes != raw {
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
        let invalid = ExactOperationWitness::Receive {
            step: Box::new(witness),
            incoming_payment: malformed.clone().into(),
        };
        assert!(matches!(
            engine.prepare(&invalid, &receiver, &parameters),
            Err(Error::Rejected(_))
        ));
        // Model a historical, already-selected capsule. Recovery must finish the defensive Burn;
        // neither a fresh Advance nor a restored unselected plan may select that branch.
        let prepared = engine.prepare_retained(&invalid, &receiver, &parameters)?;
        let capsule = prepared.capsule().clone();
        assert!(matches!(
            engine.before_selection(&capsule, &parameters),
            Err(Error::Rejected(_))
        ));
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
        let burned = engine.finalize(resumed, &receipt, &parameters)?;
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
        templates,
        dimensions,
        artifacts,
        complete_validation,
    )
    .map_err(|e| e.to_string())
}
