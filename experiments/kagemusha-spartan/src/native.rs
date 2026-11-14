//! Native future-backend authority and immutable custody-frame contracts.
//!
//! Cryptographic authority is constructed only by the child engine. Durable
//! selection, hardware signing, redundant copies, anchors and exact retry are
//! delegated to the canonical generic Advance provider; this module does not
//! implement a second journal or expose a caller-supplied acceptance Boolean.

use std::sync::Arc;

use ff::PrimeField;
use iroha_pasta::Fp;
use num_bigint::BigUint;

pub use crate::credential::Credential;
use crate::{bootstrap, p256, request::CompletedRequest};

#[path = "engine.rs"]
pub(crate) mod engine;
pub use engine::{NativeEngine, ParameterManifest};
#[cfg(test)]
pub(crate) use engine::{test_full_chain_native, test_measure_chain_native};
#[cfg(any(test, feature = "device-fixture"))]
#[path = "native_device_fixture.rs"]
pub mod device_fixture;
#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
#[path = "native_witness.rs"]
mod witness_codec;

pub const FUTURE_ARTIFACT_VERSION: u16 = 1;
pub const MAX_RECOVERY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
pub const MAX_CUSTODY_FRAME_BYTES: usize = MAX_RECOVERY_BYTES + 16 * 1024;
pub const MAX_PROGRAM_BUNDLE_BYTES: usize = 256 * 1024 * 1024;

/// Four complete public verifier programs per curve, ordered Receive, Send,
/// Bootstrap, Load. Encoding or decoding this object grants no authority.
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ProgramBundle.v1")]
pub struct ProgramBundle {
    pub version: u16,
    pub primary: [Vec<u8>; 4],
    pub secondary: [Vec<u8>; 4],
}
impl ProgramBundle {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.version != 1
            || self
                .primary
                .iter()
                .chain(&self.secondary)
                .any(Vec::is_empty)
        {
            return Err(Error::Malformed("program bundle"));
        }
        let total = self
            .primary
            .iter()
            .chain(&self.secondary)
            .try_fold(0usize, |n, p| n.checked_add(p.len()))
            .ok_or(Error::ResourceLimit)?;
        if total > MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
        let bytes = norito::to_bytes(self).map_err(|e| Error::Rejected(e.to_string()))?;
        if bytes.len() > MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_PROGRAM_BUNDLE_BYTES {
            return Err(Error::ResourceLimit);
        }
        let limits = norito::DecodeLimits::new(
            MAX_PROGRAM_BUNDLE_BYTES,
            MAX_PROGRAM_BUNDLE_BYTES,
            2 * MAX_PROGRAM_BUNDLE_BYTES,
            4 * MAX_PROGRAM_BUNDLE_BYTES,
            16,
        );
        // Use the object-specific archive bound without changing Norito's
        // process default; the payload still inherits stricter caller budgets.
        let bundle: Self = norito::core::with_decode_limits(limits, || {
            let payload = norito::canonical_frame_payload_with_archive_limit::<Self>(
                bytes,
                norito::core::V1_LAYOUT_FLAGS,
                MAX_PROGRAM_BUNDLE_BYTES as u64,
            )?;
            let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
            let (value, used) = norito::core::decode_field_canonical(payload)?;
            if used != payload.len() {
                return Err(norito::Error::LengthMismatch);
            }
            Ok(value)
        })
        .map_err(|_| Error::Malformed("bounded program bundle"))?;
        if bundle.encode()? != bytes {
            return Err(Error::Malformed("canonical program bundle"));
        }
        Ok(bundle)
    }
}

#[derive(Debug)]
pub enum Error {
    Malformed(&'static str),
    ResourceLimit,
    NotEnoughMemory { required: u64, available: u64 },
    AuthorityMismatch(&'static str),
    Rejected(String),
    Unavailable(&'static str),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(s) => write!(f, "malformed {s}"),
            Self::ResourceLimit => f.write_str("future backend resource limit"),
            Self::NotEnoughMemory {
                required,
                available,
            } => write!(
                f,
                "future prover memory admission estimates {required} bytes; {available} available"
            ),
            Self::AuthorityMismatch(s) => write!(f, "authority mismatch: {s}"),
            Self::Rejected(s) => write!(f, "proof rejected: {s}"),
            Self::Unavailable(s) => write!(f, "future backend unavailable: {s}"),
        }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Native-only result of total fixed-profile parsing and actual verification.
/// A false verdict may authorize only the Receive relation's Burn outcome.
pub(crate) struct IncomingCandidate {
    profile: [Fp; 64],
    proof_digest: Fp,
    valid: bool,
    originals: crate::payment_wire::Originals,
    pair: crate::full::UniformPair,
    pair_bytes: Vec<u8>,
}
impl IncomingCandidate {
    pub(crate) fn profile(&self) -> &[Fp; 64] {
        &self.profile
    }
    pub(crate) fn proof_digest(&self) -> Fp {
        self.proof_digest
    }
    pub(crate) fn valid(&self) -> bool {
        self.valid
    }
}

/// Native custody's protected same-boot anchor and a fresh platform observation.
/// Implementations read these from local custody and the OS; peer wire data must
/// never supply this interface.
pub trait TimeSource: Send + Sync {
    /// Current memory available to the app, obtained from the same actual OS platform.
    fn available_memory_bytes(&self) -> Result<u64>;
    /// Observation captured by Native when it froze this operation's choices.
    fn prepared(
        &self,
        wallet: &[u8; 32],
        operation_id: &[u8; 32],
    ) -> Result<(
        iroha_data_model::kagemusha::KagemushaWalletAnchoredTimeV1,
        iroha_data_model::kagemusha::KagemushaWalletMonotonicReadingV1,
    )>;
    fn observe(
        &self,
        wallet: &[u8; 32],
    ) -> Result<(
        iroha_data_model::kagemusha::KagemushaWalletAnchoredTimeV1,
        iroha_data_model::kagemusha::KagemushaWalletMonotonicReadingV1,
    )>;
}

/// Parameter admission owns the actual fixed keys, trusted scope and compiled
/// matrix polynomials. A byte identifier or a decoded carrier is insufficient.
pub struct AuthenticatedParameters {
    id: [u8; 32],
    inner: engine::ParameterSet,
}
impl AuthenticatedParameters {
    pub fn id(&self) -> &[u8; 32] {
        &self.id
    }
}

#[derive(Clone)]
pub struct BootstrapWitness {
    pub scheme: [u8; 32],
    pub root: p256::Point,
    pub relation: [u8; 32],
    pub input: bootstrap::Inputs,
}
#[derive(Clone)]
pub enum ExactOperationWitness {
    Bootstrap(Box<BootstrapWitness>),
    Load {
        step: Box<crate::load::Witness>,
        receipt_original: Arc<[u8]>,
        finality_original: Arc<[u8]>,
    },
    Send(Box<crate::send::Witness>),
    Receive {
        step: Box<crate::receive::Witness>,
        /// Exact bounded fixed-profile wire bytes. Native derives the complete
        /// verification verdict; the Receive relation constrains valid credit or
        /// Burn, retaining malformed cryptographic words without rewriting them.
        incoming_payment: Arc<[u8]>,
    },
}
impl ExactOperationWitness {
    pub fn credential(&self) -> &Credential {
        match self {
            Self::Bootstrap(w) => &w.input.credential,
            Self::Load { step, .. } => &step.credential,
            Self::Send(w) => &w.credential,
            Self::Receive { step, .. } => &step.credential,
        }
    }
    /// Decode bounded canonical private witness data, without granting authority.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        witness_codec::decode_witness(bytes)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        witness_codec::encode_witness(self)
    }
}
impl Drop for ExactOperationWitness {
    fn drop(&mut self) {
        witness_codec::wipe(self);
    }
}

/// Accepted recursive head under the exact authenticated parameter set and
/// credential. An enrollment origin is separately authenticated by the engine;
/// callers cannot construct a zero-head substitute.
#[derive(Clone)]
pub struct VerifiedWalletHead {
    parameters_id: [u8; 32],
    profile: [Fp; 64],
    pair: Arc<[u8]>,
    credential: Credential,
    certificate: crate::certificate::Certificate,
    custody_digest: [u8; 32],
    enrollment_marker: Option<[u8; 32]>,
}
impl VerifiedWalletHead {
    pub fn profile(&self) -> &[Fp; 64] {
        &self.profile
    }
    pub fn exact_pair(&self) -> &[u8] {
        &self.pair
    }
    pub fn credential(&self) -> &Credential {
        &self.credential
    }
    pub fn parameters_id(&self) -> &[u8; 32] {
        &self.parameters_id
    }
    pub fn enrollment_marker(&self) -> Option<[u8; 32]> {
        self.enrollment_marker
    }
}

/// A full original Payment whose envelope, object joins, linked proof pair and
/// both accumulated claims have passed the native engine. This is a read-only
/// capability; consumed-credit admission is still specific to the receiver.
#[derive(Clone)]
pub struct VerifiedIncomingPayment {
    parameters_id: [u8; 32],
    bytes: Arc<[u8]>,
    profile: [Fp; 64],
    digest: Fp,
    request: CompletedRequest,
    payer: Credential,
    statement: [Fp; 26],
    receipt: bootstrap::Receipt,
    pair: Arc<[u8]>,
}
impl VerifiedIncomingPayment {
    pub fn exact_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> Fp {
        self.digest
    }
    pub fn digest_bytes(&self) -> [u8; 32] {
        self.digest.to_repr()
    }
    pub fn profile(&self) -> &[Fp; 64] {
        &self.profile
    }
    pub fn parameters_id(&self) -> &[u8; 32] {
        &self.parameters_id
    }
    pub fn request(&self) -> &CompletedRequest {
        &self.request
    }
    pub fn payer(&self) -> &Credential {
        &self.payer
    }
    pub fn statement(&self) -> &[Fp; 26] {
        &self.statement
    }
    pub fn receipt(&self) -> &bootstrap::Receipt {
        &self.receipt
    }
    pub fn exact_pair(&self) -> &[u8] {
        &self.pair
    }
}

/// Preparation is not publication. Only the engine can construct it after
/// checking every monetary/admission predicate under authenticated inputs.
pub struct PreparedOperation {
    parameters_id: [u8; 32],
    capsule: RecoveryCapsule,
    witness: Arc<ExactOperationWitness>,
    receipt_body: [u8; 338],
    predecessor: VerifiedWalletHead,
}
impl PreparedOperation {
    pub fn capsule_digest(&self) -> &[u8; 32] {
        self.receipt_body[274..306]
            .try_into()
            .expect("fixed selected capsule digest")
    }
    pub fn credential(&self) -> &Credential {
        self.witness.credential()
    }
    /// Fixed 291-byte financial review projection, version 1. All terms come
    /// from this sealed checked operation and bind its exact frozen capsule.
    pub fn review_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(291);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.push(self.capsule.kind());
        out.extend_from_slice(self.capsule.scheme());
        out.extend_from_slice(self.capsule.wallet());
        out.extend_from_slice(self.capsule.operation_id());
        out.extend_from_slice(&RecoveryCapsule::frame_digest(&self.capsule.encode()?));
        let (amount, fee, receiver, account) = match self.witness.as_ref() {
            ExactOperationWitness::Send(w) => (
                &w.request.body[242..258],
                &w.request.body[290..306],
                &w.request.body[130..162],
                &w.request.body[162..194],
            ),
            ExactOperationWitness::Receive { step: w, .. } => (
                &w.request.body[242..258],
                &w.request.body[290..306],
                &w.request.body[130..162],
                &w.request.body[162..194],
            ),
            _ => (
                &[0u8; 16][..],
                &[0u8; 16][..],
                &[0u8; 32][..],
                &[0u8; 32][..],
            ),
        };
        out.extend_from_slice(amount);
        out.extend_from_slice(fee);
        out.extend_from_slice(receiver);
        out.extend_from_slice(account);
        out.extend_from_slice(self.capsule.predecessor());
        out.extend_from_slice(self.capsule.successor());
        Ok(out)
    }
    pub fn capsule(&self) -> &RecoveryCapsule {
        &self.capsule
    }
    pub fn receipt_body(&self) -> &[u8] {
        &self.receipt_body
    }
    /// Exact 32-byte P_bytes receipt message. Hardware performs SHA-256/ECDSA.
    pub fn signing_message(&self) -> [u8; 32] {
        crate::receipt::message(&bootstrap::Receipt {
            body: self.receipt_body,
            signature: [0; 64],
        })
        .to_repr()
    }
    pub fn parameters_id(&self) -> &[u8; 32] {
        &self.parameters_id
    }
}

/// The canonical provider obtains the actual hardware signature only after its
/// Selected marker is durable. Native independently verifies the exact body,
/// canonical low-S signature and the preparation's already authenticated key.
pub struct HardwareAdvanceReceipt {
    receipt: bootstrap::Receipt,
    operation_id: [u8; 32],
    capsule_digest: [u8; 32],
}
impl HardwareAdvanceReceipt {
    pub fn receipt(&self) -> &bootstrap::Receipt {
        &self.receipt
    }
    pub fn capsule_digest(&self) -> &[u8; 32] {
        &self.capsule_digest
    }
    pub fn operation_id(&self) -> &[u8; 32] {
        &self.operation_id
    }
    pub fn verify_selected(
        prepared: &PreparedOperation,
        body: &[u8],
        signature: [u8; 64],
    ) -> Result<Self> {
        if body != prepared.receipt_body() || body.len() != 338 {
            return Err(Error::AuthorityMismatch("selected receipt body"));
        }
        let receipt = bootstrap::Receipt {
            body: body
                .try_into()
                .map_err(|_| Error::Malformed("receipt length"))?,
            signature,
        };
        let q = point(&prepared.capsule.frame.payment_key)?;
        let r = BigUint::from_bytes_be(&signature[..32]);
        let s = BigUint::from_bytes_be(&signature[32..]);
        if s > p256::n() >> 1 {
            return Err(Error::Malformed("receipt high-S signature"));
        }
        let message = crate::receipt::message(&receipt).to_repr();
        let z = BigUint::from_bytes_be(&p256::sha256_one_block(message.as_ref()));
        if !p256::verify(&q, &z, &p256::Signature { r, s }) {
            return Err(Error::Rejected("hardware receipt signature".into()));
        }
        Ok(Self {
            receipt,
            operation_id: prepared.capsule.frame.operation_id,
            capsule_digest: RecoveryCapsule::frame_digest(&prepared.capsule.encode()?),
        })
    }
}

/// Final proof and output are verified before returning this capability. The
/// canonical provider persists its exact completion before releasing bytes.
pub struct CompletedOperation {
    completion: CompletionRecord,
    head: VerifiedWalletHead,
}
impl CompletedOperation {
    pub fn completion(&self) -> &CompletionRecord {
        &self.completion
    }
    pub fn head(&self) -> &VerifiedWalletHead {
        &self.head
    }
    pub fn exact_output(&self) -> &[u8] {
        &self.completion.frame.output
    }
}

mod sealed {
    pub trait Engine {}
}
pub trait Engine: sealed::Engine {
    /// Fresh time-dependent controls immediately before irreversible selection.
    /// Recovery of an already selected operation does not rerun this check.
    fn before_selection(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<()>;
    fn prepare(
        &self,
        witness: &ExactOperationWitness,
        head: &VerifiedWalletHead,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation>;
    fn finalize(
        &self,
        prepared: PreparedOperation,
        receipt: &HardwareAdvanceReceipt,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation>;
    fn verify_payment(
        &self,
        bytes: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedIncomingPayment>;
    fn verify_head(
        &self,
        pair: &[u8],
        credential: &Credential,
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedWalletHead>;
    /// Reconstruct and revalidate a decoded retained capsule after a restart.
    /// Decoding a frame alone never recreates preparation authority.
    fn resume(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation>;
    /// Verify retained exact completion bytes; recovery never regenerates a
    /// randomized proof or selects a replacement signature for a released head.
    fn verify_completion(
        &self,
        capsule: &RecoveryCapsule,
        record: &CompletionRecord,
        parameters: &AuthenticatedParameters,
    ) -> Result<CompletedOperation>;
}

#[derive(Clone, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Capsule.v1")]
struct CapsuleFrame {
    version: u16,
    parameters_id: [u8; 32],
    scheme: [u8; 32],
    asset: [u8; 32],
    wallet: [u8; 32],
    payment_key: Vec<u8>,
    kind: u8,
    sequence: u128,
    predecessor: [u8; 32],
    predecessor_capsule_digest: [u8; 32],
    successor: [u8; 32],
    operation_id: [u8; 32],
    proof_digest: [u8; 32],
    enrollment_marker: Option<[u8; 32]>,
    receipt_body: Vec<u8>,
    recovery: Vec<u8>,
}
impl Drop for CapsuleFrame {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.recovery);
    }
}
#[derive(Clone)]
pub struct RecoveryCapsule {
    frame: CapsuleFrame,
}
impl RecoveryCapsule {
    pub fn scheme(&self) -> &[u8; 32] {
        &self.frame.scheme
    }
    pub fn wallet(&self) -> &[u8; 32] {
        &self.frame.wallet
    }
    pub fn asset(&self) -> &[u8; 32] {
        &self.frame.asset
    }
    pub fn payment_key(&self) -> &[u8] {
        &self.frame.payment_key
    }
    pub fn predecessor_capsule_digest(&self) -> &[u8; 32] {
        &self.frame.predecessor_capsule_digest
    }
    pub fn operation_id(&self) -> &[u8; 32] {
        &self.frame.operation_id
    }
    pub fn predecessor(&self) -> &[u8; 32] {
        &self.frame.predecessor
    }
    pub fn successor(&self) -> &[u8; 32] {
        &self.frame.successor
    }
    pub fn proof_digest(&self) -> &[u8; 32] {
        &self.frame.proof_digest
    }
    pub fn sequence(&self) -> u128 {
        self.frame.sequence
    }
    pub fn kind(&self) -> u8 {
        self.frame.kind
    }
    pub fn enrollment_marker(&self) -> Option<[u8; 32]> {
        self.frame.enrollment_marker
    }
    pub fn parameters_id(&self) -> &[u8; 32] {
        &self.frame.parameters_id
    }
    pub fn receipt_body(&self) -> &[u8] {
        &self.frame.receipt_body
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(&self.frame)
    }
    /// Structural frame validation under the provider marker's scheme. Resume
    /// must still recreate a PreparedOperation through the sealed engine.
    pub fn decode(bytes: &[u8], scheme: &[u8; 32]) -> Result<Self> {
        let frame = decode::<CapsuleFrame>(bytes)?;
        let capsule = Self { frame };
        capsule.validate()?;
        if capsule.scheme() != scheme {
            return Err(Error::AuthorityMismatch("capsule scheme"));
        }
        if capsule.encode()? != bytes {
            return Err(Error::Malformed("capsule canonical frame"));
        }
        Ok(capsule)
    }
    fn validate(&self) -> Result<()> {
        let f = &self.frame;
        if f.version != FUTURE_ARTIFACT_VERSION
            || ![1, 2, 3, 4].contains(&f.kind)
            || f.receipt_body.len() != 338
            || f.recovery.is_empty()
            || f.recovery.len() > MAX_RECOVERY_BYTES
        {
            return Err(Error::Malformed("future recovery capsule"));
        }
        point(&f.payment_key)?;
        for v in [
            &f.scheme,
            &f.asset,
            &f.wallet,
            &f.parameters_id,
            &f.operation_id,
            &f.successor,
            &f.proof_digest,
        ] {
            if v.iter().all(|b| *b == 0) {
                return Err(Error::Malformed("zero capsule identity"));
            }
        }
        let r = &f.receipt_body;
        for (actual, expected) in [
            (&r[..2], 1u16.to_le_bytes().as_slice()),
            (&r[2..34], f.scheme.as_slice()),
            (&r[34..66], f.wallet.as_slice()),
            (&r[98..114], f.sequence.to_le_bytes().as_slice()),
            (&r[114..146], f.operation_id.as_slice()),
            (&r[146..178], f.predecessor.as_slice()),
            (&r[178..210], f.successor.as_slice()),
            (&r[242..274], f.proof_digest.as_slice()),
        ] {
            if actual != expected {
                return Err(Error::AuthorityMismatch("capsule receipt field"));
            }
        }
        for v in [
            &f.predecessor,
            &f.successor,
            &f.operation_id,
            &f.proof_digest,
        ] {
            field(v)?;
        }
        if (f.kind == 1) != f.enrollment_marker.is_some() {
            return Err(Error::Malformed("Bootstrap enrollment binding"));
        }
        Ok(())
    }
    pub fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        custody_digest(b"spartan-future-capsule-v1", bytes)
    }
}

#[derive(Clone, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Completion.v1")]
struct CompletionFrame {
    version: u16,
    parameters_id: [u8; 32],
    wallet: [u8; 32],
    operation_id: [u8; 32],
    capsule_digest: [u8; 32],
    receipt: Vec<u8>,
    output: Vec<u8>,
}
#[derive(Clone)]
pub struct CompletionRecord {
    frame: CompletionFrame,
}
impl CompletionRecord {
    /// Exact retained public output: 0 complete head, 1 Payment, 2 this record.
    /// Decoding output does not itself grant proof authority.
    pub fn output_part(&self, part: u32) -> Result<Vec<u8>> {
        if part == 2 {
            return self.encode();
        }
        engine::retained_output_part(&self.frame.output, part)
    }
    pub fn operation_id(&self) -> &[u8; 32] {
        &self.frame.operation_id
    }
    pub fn capsule_digest(&self) -> &[u8; 32] {
        &self.frame.capsule_digest
    }
    pub fn wallet(&self) -> &[u8; 32] {
        &self.frame.wallet
    }
    pub fn exact_receipt(&self) -> &[u8] {
        &self.frame.receipt
    }
    pub fn exact_output(&self) -> &[u8] {
        &self.frame.output
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(&self.frame)
    }
    pub fn decode(bytes: &[u8], wallet: &[u8; 32]) -> Result<Self> {
        let record = Self {
            frame: decode::<CompletionFrame>(bytes)?,
        };
        record.validate()?;
        if record.wallet() != wallet {
            return Err(Error::AuthorityMismatch("completion wallet"));
        }
        if record.encode()? != bytes {
            return Err(Error::Malformed("completion canonical frame"));
        }
        Ok(record)
    }
    fn validate(&self) -> Result<()> {
        let f = &self.frame;
        if f.version != FUTURE_ARTIFACT_VERSION
            || f.receipt.len() != 402
            || f.output.is_empty()
            || f.output.len() > MAX_OUTPUT_BYTES
        {
            return Err(Error::Malformed("future completion"));
        }
        if f.receipt[34..66] != f.wallet || f.receipt[114..146] != f.operation_id {
            return Err(Error::AuthorityMismatch("completion receipt"));
        }
        for v in [
            &f.parameters_id,
            &f.wallet,
            &f.operation_id,
            &f.capsule_digest,
        ] {
            if v.iter().all(|b| *b == 0) {
                return Err(Error::Malformed("zero completion identity"));
            }
        }
        Ok(())
    }
    pub fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        custody_digest(b"spartan-future-completion-v1", bytes)
    }
}
fn custody_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    *h.finalize().as_bytes()
}
fn point(bytes: &[u8]) -> Result<p256::Point> {
    if bytes.len() != 65 || bytes[0] != 4 {
        return Err(Error::Malformed("payment key"));
    }
    let q = p256::Point {
        x: BigUint::from_bytes_be(&bytes[1..33]),
        y: BigUint::from_bytes_be(&bytes[33..]),
    };
    if q.x >= p256::p() || q.y >= p256::p() || !p256::on_curve(&q) {
        return Err(Error::Malformed("payment key curve"));
    }
    Ok(q)
}
fn field(bytes: &[u8; 32]) -> Result<Fp> {
    Option::<Fp>::from(Fp::from_repr(*bytes)).ok_or(Error::Malformed("canonical field"))
}
fn encode<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    let bytes = norito::to_bytes(value).map_err(|e| Error::Rejected(e.to_string()))?;
    if bytes.len() > MAX_CUSTODY_FRAME_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(bytes)
}
fn decode<T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>>(
    bytes: &[u8],
) -> Result<T> {
    if bytes.len() > MAX_CUSTODY_FRAME_BYTES {
        return Err(Error::ResourceLimit);
    }
    let view =
        norito::core::from_bytes_view(bytes).map_err(|_| Error::Malformed("Norito frame"))?;
    if view.flags() != norito::core::V1_LAYOUT_FLAGS {
        return Err(Error::Malformed("Norito flags"));
    }
    norito::decode_from_bytes_with_limits(
        bytes,
        norito::DecodeLimits::new(
            MAX_CUSTODY_FRAME_BYTES,
            MAX_CUSTODY_FRAME_BYTES,
            4 * MAX_CUSTODY_FRAME_BYTES,
            8 * MAX_CUSTODY_FRAME_BYTES,
            16,
        ),
    )
    .map_err(|_| Error::Malformed("bounded Norito custody frame"))
}
