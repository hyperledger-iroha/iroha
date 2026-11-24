//! Native future-backend authority and immutable custody-frame contracts.
//!
//! Cryptographic authority is constructed only by the child engine. Durable
//! selection, hardware signing, redundant copies, anchors and exact retry are
//! delegated to the canonical generic Advance provider; this module does not
//! implement a second journal or expose a caller-supplied acceptance Boolean.

use std::sync::Arc;
use zeroize::Zeroizing;

use ff::PrimeField;
use iroha_pasta::Fp;
use num_bigint::BigUint;

pub use crate::credential::Credential;
use crate::{bootstrap, p256, request::CompletedRequest};

#[path = "engine.rs"]
pub(crate) mod engine;
pub use engine::{
    BootstrapActivationPackage, CredentialIssuerParameters, GovernedManifest,
    MAX_ACTIVATION_CLAIM_BYTES, MAX_UNLOAD_CLAIM_BYTES, NativeEngine, OpenRequestPeer,
    PARAMETER_MANIFEST_VERSION, ParameterManifest, SpartanWalletMap, SpartanWalletMaps,
    UnloadDestinations, VerifiedActivationClaim, VerifiedCreditStatus, VerifiedUnloadClaim,
    credential_signing_message_for_body, decode_open_request_peer, encode_open_request_peer,
    wallet_receive_replay_key, wallet_send_replay_key,
};
#[cfg(test)]
pub(crate) use engine::{test_full_chain_native, test_measure_chain_native};
#[cfg(any(test, feature = "device-fixture"))]
#[path = "native_device_fixture.rs"]
pub mod device_fixture;
#[path = "native_sent_delivery.rs"]
mod sent_delivery;
#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
pub use sent_delivery::VerifiedSentDelivery;
#[path = "native_funding.rs"]
mod funding;
#[path = "native_open_request.rs"]
mod open_request;
#[path = "native_owned_unload.rs"]
mod owned_unload;
#[path = "native_witness.rs"]
mod witness_codec;

/// Matrix accumulator coordinates of the compiled four-operation producer profile.
pub(crate) const PRODUCER_MATRIX_DIMENSIONS: ((usize, usize), (usize, usize)) =
    ((24, 25), (24, 24));

/// Published independently of any incoming signature, profile or program bytes.
/// Populated by the compiler after all ten monetary tapes reach a fixed point.
const FIVE_KIND_PRODUCER: Option<FiveKindProducer> = Some(FiveKindProducer {
    programs: ProgramProfileV2 {
        primary: [
            [
                0xa6, 0x60, 0x67, 0x59, 0x20, 0xe0, 0xbd, 0x04, 0x41, 0xac, 0x41, 0xb4, 0x70, 0x68,
                0x14, 0xd1, 0x65, 0xed, 0x7e, 0x57, 0xa0, 0xc9, 0x85, 0x70, 0x83, 0x8f, 0xa3, 0x32,
                0x4a, 0x09, 0x96, 0x47,
            ],
            [
                0x2f, 0xa1, 0x9f, 0x13, 0x2a, 0xc6, 0xc4, 0x57, 0x24, 0x74, 0xeb, 0x5e, 0x3e, 0x00,
                0x1e, 0xb6, 0x76, 0x55, 0x63, 0x77, 0x18, 0x27, 0x49, 0xb4, 0xb7, 0x9a, 0x6d, 0xb5,
                0x25, 0xfd, 0xd8, 0xab,
            ],
            [
                0x23, 0x0c, 0x04, 0x95, 0x42, 0x7a, 0xea, 0x5d, 0x8a, 0xa1, 0xc6, 0x84, 0xe3, 0x5a,
                0x8e, 0x32, 0xac, 0x93, 0x64, 0x0d, 0x1e, 0x4c, 0x94, 0xc8, 0x9d, 0x2e, 0xca, 0x3e,
                0x08, 0x7a, 0x7b, 0x7f,
            ],
            [
                0xd2, 0x66, 0x00, 0xac, 0xd5, 0xea, 0x0c, 0x14, 0x50, 0x5b, 0x5d, 0xb0, 0x9b, 0xc6,
                0x91, 0x5e, 0xfe, 0xfe, 0x33, 0x3d, 0xa3, 0xac, 0x4a, 0x77, 0x57, 0x9a, 0xfe, 0x1c,
                0x07, 0x65, 0x50, 0x99,
            ],
            [
                0x1a, 0xe9, 0x5c, 0x13, 0x4d, 0xea, 0x2f, 0xb3, 0x04, 0x87, 0xa7, 0x2d, 0xba, 0x05,
                0x49, 0xb9, 0x36, 0x73, 0x5b, 0x74, 0x5d, 0x10, 0x57, 0x06, 0x7b, 0x72, 0x2e, 0xbf,
                0x5f, 0x20, 0xa0, 0x17,
            ],
        ],
        secondary: [
            [
                0x0a, 0x29, 0xd7, 0xd3, 0xc4, 0xa5, 0xea, 0x39, 0x09, 0xd1, 0x3a, 0xc2, 0x2d, 0x25,
                0x37, 0x0d, 0x91, 0x93, 0xfd, 0x86, 0x5a, 0x88, 0x6e, 0x8f, 0xbe, 0x2a, 0x55, 0x74,
                0xde, 0x8b, 0xe3, 0xb5,
            ],
            [
                0xed, 0xf8, 0x1a, 0xdc, 0x26, 0x7a, 0x3a, 0x8a, 0x66, 0xf5, 0x8d, 0xf3, 0x03, 0xb3,
                0xe2, 0x31, 0xa3, 0xfe, 0x36, 0xb3, 0xc4, 0x31, 0x52, 0x2b, 0x12, 0xd4, 0xda, 0x33,
                0x38, 0xfe, 0x17, 0x04,
            ],
            [
                0xb9, 0xb2, 0x40, 0x75, 0x2e, 0x15, 0xac, 0x55, 0x6e, 0xd6, 0x59, 0x4c, 0x38, 0x47,
                0xcb, 0xb5, 0xbb, 0x26, 0x89, 0x5d, 0xa9, 0x34, 0xfd, 0x00, 0x1d, 0x82, 0x3e, 0xeb,
                0x68, 0x59, 0xf7, 0xb0,
            ],
            [
                0xe2, 0x0b, 0xa0, 0xd3, 0xcb, 0x71, 0xcb, 0x85, 0xa4, 0x52, 0x0f, 0x7f, 0xec, 0xde,
                0xf6, 0xc0, 0xef, 0xf4, 0xb5, 0xe2, 0xe9, 0x0d, 0xf0, 0x36, 0x89, 0x0e, 0x76, 0xf0,
                0xe7, 0x05, 0xae, 0x96,
            ],
            [
                0xbb, 0x85, 0x4e, 0xef, 0xfb, 0x08, 0x28, 0x09, 0x2d, 0x35, 0x95, 0x6d, 0xd7, 0x6c,
                0x4c, 0xe1, 0x9e, 0x01, 0xef, 0xad, 0xae, 0x3f, 0xfa, 0x6c, 0x50, 0x89, 0xa7, 0x3a,
                0x9c, 0xf9, 0x24, 0xa3,
            ],
        ],
        dimensions: [[24, 25], [24, 24]],
    },
    generator_capacity_bits: [18, 17],
    grouping: [program_bundle::SumcheckGroup::Four; 2],
});

struct FiveKindProducer {
    programs: ProgramProfileV2,
    generator_capacity_bits: [usize; 2],
    grouping: [program_bundle::SumcheckGroup; 2],
}

/// Five-kind program pins supported by this native producer build.
pub fn built_in_five_programs() -> Option<ProgramProfileV2> {
    FIVE_KIND_PRODUCER
        .as_ref()
        .map(|source| source.programs.clone())
}

/// Native compiler digests in Receive, Send, Bootstrap and Control order.
pub fn built_in_producer_programs() -> Option<([[u8; 32]; 4], [[u8; 32]; 4])> {
    // v4 p18-s17. Each canonical program was byte-identical across compiler passes 4 and 5.
    Some((
        [
            [
                0x3c, 0xad, 0x0e, 0x02, 0x19, 0x04, 0xa8, 0xd0, 0xd9, 0x60, 0x66, 0x2e, 0xac, 0xb3,
                0x95, 0x59, 0xe4, 0x8f, 0x8c, 0xfa, 0x48, 0xa2, 0x36, 0xf0, 0x33, 0x56, 0x7f, 0x30,
                0x73, 0xc2, 0x75, 0xed,
            ],
            [
                0x6f, 0x9c, 0x3c, 0x33, 0x06, 0x21, 0x1d, 0xd1, 0xea, 0x33, 0x56, 0x36, 0x41, 0xa0,
                0x86, 0xe6, 0xbf, 0x3d, 0x1d, 0xf4, 0xdb, 0xe1, 0xa3, 0xee, 0x05, 0x3f, 0x9e, 0x5e,
                0xcc, 0x05, 0xfa, 0xed,
            ],
            [
                0x95, 0x6d, 0x00, 0x7c, 0x57, 0xb1, 0xa7, 0x85, 0x0d, 0x0c, 0x21, 0x4e, 0x31, 0x7e,
                0x93, 0x09, 0x4b, 0xe7, 0x57, 0xe6, 0x89, 0x2e, 0xb7, 0x14, 0x42, 0xd3, 0x6b, 0x1f,
                0x59, 0xc5, 0x67, 0x4c,
            ],
            [
                0xb7, 0xca, 0xae, 0xc2, 0xc0, 0xf9, 0xe5, 0x04, 0xed, 0x68, 0x64, 0x9d, 0x35, 0xd9,
                0x7e, 0x15, 0x20, 0x7c, 0xdd, 0x03, 0x10, 0x6b, 0xac, 0x54, 0xab, 0x34, 0x30, 0x62,
                0x84, 0x3c, 0x63, 0xa4,
            ],
        ],
        [
            [
                0xd4, 0x50, 0xda, 0x3d, 0x4c, 0x75, 0x6b, 0x2a, 0xe4, 0x69, 0xe6, 0x22, 0x1d, 0xc2,
                0xbb, 0x2a, 0x31, 0x1b, 0x72, 0x27, 0x62, 0x70, 0x59, 0xa3, 0x25, 0x15, 0xa2, 0x5f,
                0x86, 0x6a, 0x3d, 0x40,
            ],
            [
                0x35, 0x1b, 0xbc, 0xab, 0xba, 0x5e, 0x1e, 0x3a, 0xcd, 0xcb, 0xe5, 0x03, 0x59, 0xe2,
                0x8f, 0xac, 0x01, 0x61, 0x8c, 0xd4, 0xff, 0x69, 0xee, 0x2b, 0x1f, 0x7b, 0xaf, 0xb1,
                0x15, 0x82, 0x3e, 0x97,
            ],
            [
                0x21, 0x31, 0xd9, 0x27, 0xe6, 0x03, 0x50, 0xeb, 0xd3, 0x75, 0x35, 0x01, 0x35, 0x47,
                0xb7, 0x8d, 0x43, 0x13, 0xfa, 0xac, 0x72, 0xb7, 0x26, 0x2d, 0xb3, 0x58, 0x51, 0xd5,
                0x04, 0x7a, 0xd9, 0x43,
            ],
            [
                0xb0, 0x85, 0x78, 0xf9, 0x69, 0x7f, 0x40, 0x16, 0xb3, 0xf7, 0x0d, 0xe9, 0x30, 0x03,
                0xa9, 0xef, 0x38, 0x46, 0x42, 0x16, 0x3a, 0x36, 0x33, 0x13, 0x7e, 0x3b, 0x44, 0xe0,
                0x62, 0x39, 0x54, 0x0f,
            ],
        ],
    ))
}

pub const FUTURE_ARTIFACT_VERSION: u16 = 1;
const RECOVERY_CAPSULE_VERSION: u16 = 2;
pub const MAX_RECOVERY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
pub const MAX_CUSTODY_FRAME_BYTES: usize = MAX_RECOVERY_BYTES + 16 * 1024;
/// Maximum complete compact program bundle. The wallet carrier applies its own
/// aggregate limit across the bundle, signed manifest and remaining originals.
pub const MAX_PROGRAM_BUNDLE_BYTES: usize = 10 * 1024 * 1024;

/// Four complete public verifier programs per curve, ordered Receive, Send,
/// Bootstrap, Control. Encoding or decoding this object grants no authority.
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ProgramBundle.v1")]
pub struct ProgramBundle {
    pub version: u16,
    pub primary: [Vec<u8>; 4],
    pub secondary: [Vec<u8>; 4],
}
#[path = "program_bundle.rs"]
mod program_bundle;
pub use program_bundle::{ProgramBundleGeometry, ProgramBundleV2, ProgramTransportSizes};
#[path = "successor_manifest.rs"]
mod successor_manifest;
pub use successor_manifest::{AuthenticatedManifestHeaderV5, ProgramProfileV2};

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
    payment_digest: Fp,
    profile: [Fp; 64],
    proof_digest: Fp,
    verified: Option<VerifiedIncomingPayment>,
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
        self.verified.is_some()
    }
}

#[path = "native_debit.rs"]
mod debit;
pub use debit::{DebitApprovalEvidence, DebitAuthorizationSource, DebitBoundary};

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
    /// Fresh OS reading against the same direct anchor used for this preparation.
    fn observe(
        &self,
        wallet: &[u8; 32],
        operation_id: &[u8; 32],
    ) -> Result<(
        iroha_data_model::kagemusha::KagemushaWalletAnchoredTimeV1,
        iroha_data_model::kagemusha::KagemushaWalletMonotonicReadingV1,
    )>;
}

/// Parameter admission owns the actual fixed keys, trusted scope and compiled
/// matrix polynomials. A byte identifier or a decoded carrier is insufficient.
pub struct AuthenticatedParameters {
    // Local identity of this successful installation, including its trusted authorities.
    installation: Arc<()>,
    id: [u8; 32],
    manifest_digest: [u8; 32],
    confirmation: ConfirmationScope,
    inner: Arc<engine::ParameterSet>,
    // Verified-head reuse, bound to this exact installed instance.
    verified_head: engine::HeadCache,
}
/// Production installation never gains debit authority from a prepared review alone.
#[derive(Clone, Copy)]
enum ConfirmationScope {
    Installed,
    /// Only constructed by the isolated public-issuer fixture or crate unit tests.
    #[cfg(any(test, feature = "device-fixture"))]
    FixtureSend,
}

impl AuthenticatedParameters {
    /// Clone the compact verifier selected when the signed scheme was installed.
    pub fn genesis_commit_verifier(
        &self,
    ) -> iroha_data_model::sumeragi_finality::SumeragiCommitVerifierV1 {
        self.inner.finality.clone()
    }

    /// Canonical kgwparm1 digest of the exact signed manifest, including every program pin.
    pub fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    /// Relation label carried in proof and custody frames.
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
/// Private payer state of one ArchiveSent step (wallet wire §3.3, tag 5). Every field other
/// than the successor nonce `after_core[29]` is recomputed by Native from the selected
/// predecessor, the exact retained originals and the two removal openings; a witness that
/// differs in any derived word is refused.
#[derive(Clone)]
pub struct ArchiveSentWitness {
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: Credential,
    pub statement: [Fp; 26],
    /// Unsigned receipt template: zero signature, proof and capsule digests.
    pub receipt: bootstrap::Receipt,
    /// Removal of the Payment's pending-outgoing leaf under the committed core root.
    pub core_removal: iroha_data_model::kagemusha::KagemushaWalletIndexedRemoveV1,
    /// The same removal under the predecessor head's lineage-adjusted pending root.
    pub adjusted_removal: iroha_data_model::kagemusha::KagemushaWalletIndexedRemoveV1,
}
#[derive(Clone)]
pub enum ExactOperationWitness {
    Bootstrap(Box<BootstrapWitness>),
    Load {
        step: Box<crate::load::Witness>,
        receipt_original: Arc<[u8]>,
        finality_original: Arc<[u8]>,
    },
    /// A signed TimeAnchor floor update. No local direct-exchange observation is created.
    RefreshTimeAnchor(Box<crate::control::Witness>),
    /// Future Control-Unload. Native authenticates both payout destinations before review.
    Unload {
        step: Box<crate::control::Witness>,
        destinations: Box<UnloadDestinations>,
    },
    Send(Box<crate::send::Witness>),
    Receive {
        step: Box<crate::receive::Witness>,
        /// Exact bounded fixed-profile wire bytes. Native derives the complete
        /// verification verdict; the Receive relation constrains valid credit or
        /// Burn, retaining malformed cryptographic words without rewriting them.
        incoming_payment: Arc<[u8]>,
    },
    /// Payer ArchiveSent of one delivered Send. The four exact originals are its fold
    /// witnesses: the released Send capsule and completion (historical payer credential,
    /// certificate and the exact Payment), the hosted open Request and the Credited
    /// CreditStatus. A five-kind installation reopens all four during preparation
    /// and recovery. A four-kind installation refuses before custody selection.
    ArchiveSent {
        step: Box<ArchiveSentWitness>,
        /// Contains the original private Send witness. Erase it when the last owner drops.
        send_capsule: Arc<Zeroizing<Vec<u8>>>,
        send_completion: Arc<[u8]>,
        open_request: Arc<[u8]>,
        credit_status: Arc<[u8]>,
    },
}
impl ExactOperationWitness {
    /// Counterparty fixed in the completed Request for a local policy check on a fresh operation.
    pub fn policy_counterparties(&self) -> [Option<[u8; 32]>; 2] {
        let first = self.counterparty_account();
        let second = match self {
            Self::Receive { step, .. } => Some(
                step.payer.body[98..130]
                    .try_into()
                    .expect("fixed credential account"),
            ),
            _ => None,
        };
        [first, second.filter(|value| Some(*value) != first)]
    }
    pub fn counterparty_account(&self) -> Option<[u8; 32]> {
        match self {
            Self::Send(step) => Some(
                step.request.body[162..194]
                    .try_into()
                    .expect("fixed receiver account"),
            ),
            Self::Receive { step, .. } => Some(
                step.request.body[98..130]
                    .try_into()
                    .expect("fixed payer account"),
            ),
            _ => None,
        }
    }

    pub fn credential(&self) -> &Credential {
        match self {
            Self::Bootstrap(w) => &w.input.credential,
            Self::Load { step, .. } => &step.credential,
            Self::RefreshTimeAnchor(w) | Self::Unload { step: w, .. } => &w.credential,
            Self::Send(w) => &w.credential,
            Self::Receive { step, .. } => &step.credential,
            Self::ArchiveSent { step, .. } => &step.credential,
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
    manifest_digest: [u8; 32],
    profile: [Fp; 64],
    pair: Arc<[u8]>,
    credential: Credential,
    certificate: crate::certificate::Certificate,
    custody_digest: [u8; 32],
    enrollment_marker: Option<[u8; 32]>,
}
impl VerifiedWalletHead {
    fn require_parameters(&self, parameters: &AuthenticatedParameters) -> Result<()> {
        if &self.parameters_id != parameters.id()
            || &self.manifest_digest != parameters.manifest_digest()
        {
            return Err(Error::AuthorityMismatch("verified head parameter manifest"));
        }
        Ok(())
    }

    pub fn profile(&self) -> &[Fp; 64] {
        &self.profile
    }
    /// The complete retained HeadFrame, including both original proofs and their authenticated public projection.
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
    manifest_digest: [u8; 32],
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
    fn require_parameters(&self, parameters: &AuthenticatedParameters) -> Result<()> {
        if self.parameters_id != parameters.id || self.manifest_digest != parameters.manifest_digest
        {
            return Err(Error::AuthorityMismatch(
                "verified Payment parameter manifest",
            ));
        }
        Ok(())
    }
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

/// Derived from the exact frozen Payment during preparation or cold resume.
/// This in-process witness is never serialized into a custody record.
struct PreparedReceive {
    verified: Option<VerifiedIncomingPayment>,
    pair: crate::full::UniformPair,
    pair_bytes: Vec<u8>,
    can_credit: bool,
}

/// Preparation is not publication. Only the engine can construct it after
/// checking every monetary/admission predicate under authenticated inputs.
pub struct PreparedOperation {
    // Present only after a cold reconstruction checked the captured predecessor and
    // reproduced the exact capsule. Neither identity is part of the wallet wire.
    resumed_under: Option<Box<engine::ResumedSelection>>,
    manifest_digest: [u8; 32],
    parameters_id: [u8; 32],
    capsule: RecoveryCapsule,
    witness: Arc<ExactOperationWitness>,
    receipt_body: [u8; 338],
    receive: Option<Box<PreparedReceive>>,
    archive: Option<Box<engine::PreparedArchive>>,
    predecessor: VerifiedWalletHead,
}
impl PreparedOperation {
    fn require_receive_admission(&self) -> Result<()> {
        if matches!(self.witness.as_ref(), ExactOperationWitness::Receive { .. })
            && !self
                .receive
                .as_ref()
                .is_some_and(|incoming| incoming.can_credit)
        {
            return Err(Error::Rejected(
                "incoming Payment cannot credit this receiver".into(),
            ));
        }
        Ok(())
    }
    /// Stable peer action identity of the exact admitted witness, independent of later heads.
    pub fn transfer_replay_key(&self) -> Result<Option<[u8; 32]>> {
        self.witness.transfer_replay_key()
    }

    /// Original complete Payment frozen into this Receive before its irreversible selection.
    pub fn received_payment_original(&self) -> Option<&[u8]> {
        match self.witness.as_ref() {
            ExactOperationWitness::Receive {
                incoming_payment, ..
            } => Some(incoming_payment),
            _ => None,
        }
    }
    /// Both the specialized Request account and the payer credential account, if they differ.
    pub fn policy_counterparties(&self) -> [Option<[u8; 32]>; 2] {
        self.witness.policy_counterparties()
    }

    /// Read the same completed Request counterparty from this immutable Native preparation.
    pub fn counterparty_account(&self) -> Option<[u8; 32]> {
        self.witness.counterparty_account()
    }

    fn successor_opening(&self) -> (&[Fp; 30], &[Fp; 6]) {
        match self.witness.as_ref() {
            ExactOperationWitness::Bootstrap(step) => (&step.input.core, &step.input.rest),
            ExactOperationWitness::Load { step, .. } => (&step.after_core, &step.after_rest),
            ExactOperationWitness::Send(step) => (&step.after_core, &step.after_rest),
            ExactOperationWitness::Receive { step, .. } => (&step.after_core, &step.after_rest),
            ExactOperationWitness::Unload { step, .. }
            | ExactOperationWitness::RefreshTimeAnchor(step) => {
                (&step.after_core, &step.after_rest)
            }
            ExactOperationWitness::ArchiveSent { step, .. } => (&step.after_core, &step.after_rest),
        }
    }

    /// Time policy and signed-anchor digest of this authenticated successor.
    /// A local clock observation is still required for a time-controlled Send.
    pub fn successor_time_policy(&self) -> Result<(u64, [u8; 32])> {
        let (core, rest) = self.successor_opening();
        let response = core[25].to_repr();
        if response[8..].iter().any(|byte| *byte != 0) {
            return Err(Error::Malformed("time anchor response limit"));
        }
        Ok((
            u64::from_le_bytes(response[..8].try_into().unwrap()),
            rest[5].to_repr(),
        ))
    }

    pub fn capsule_digest(&self) -> &[u8; 32] {
        self.receipt_body[274..306]
            .try_into()
            .expect("fixed selected capsule digest")
    }
    pub fn credential(&self) -> &Credential {
        self.witness.credential()
    }
    /// Version 1 financial review (291 bytes), or version 2 signed TimeAnchor
    /// review (243 bytes). Both bind the exact checked and frozen capsule.
    /// The signed issuer time and resulting floor do not assert OS clock custody.
    pub fn review_bytes(&self) -> Result<Vec<u8>> {
        let refresh = match self.witness.as_ref() {
            ExactOperationWitness::RefreshTimeAnchor(step) => Some(step),
            _ => None,
        };
        let mut out = Vec::with_capacity(if refresh.is_some() { 243 } else { 291 });
        let version: u16 = match self.witness.as_ref() {
            ExactOperationWitness::Unload { .. } => 3,
            ExactOperationWitness::RefreshTimeAnchor(_) => 2,
            _ => 1,
        };
        out.extend_from_slice(&version.to_le_bytes());
        out.push(self.capsule.kind());
        out.extend_from_slice(self.capsule.scheme());
        out.extend_from_slice(self.capsule.wallet());
        out.extend_from_slice(self.capsule.operation_id());
        out.extend_from_slice(&RecoveryCapsule::frame_digest(&self.capsule.encode()?));
        if let ExactOperationWitness::Unload { step, destinations } = self.witness.as_ref() {
            destinations.append_review(step, &self.capsule, &mut out)?;
            return Ok(out);
        }
        if let Some(step) = refresh {
            out.extend_from_slice(step.after_rest[5].to_repr().as_ref());
            out.extend_from_slice(
                &crate::time_anchor::issuer_time(&step.authority.anchor).to_le_bytes(),
            );
            let floor = step.after_core[28].to_repr();
            if floor[8..].iter().any(|&x| x != 0) {
                return Err(Error::Malformed("TimeAnchor review floor"));
            }
            out.extend_from_slice(&floor[..8]);
            out.extend_from_slice(self.capsule.predecessor());
            out.extend_from_slice(self.capsule.successor());
            return Ok(out);
        }
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
    fn released_core<'a>(&self, prepared: &'a PreparedOperation) -> Result<&'a [Fp; 30]> {
        let (core, rest) = prepared.successor_opening();
        let head = crate::monetary_state::commitment(core, rest);
        let signed = self.head.profile();
        if self.head.parameters_id != prepared.parameters_id
            || self.head.manifest_digest != prepared.manifest_digest
            || self.head.custody_digest != *prepared.capsule_digest()
            || signed[crate::full::HEAD_IO] != head
            || head.to_repr() != *prepared.capsule.successor()
            || self.completion.operation_id() != prepared.capsule.operation_id()
        {
            return Err(Error::AuthorityMismatch("released state opening"));
        }
        if funding::uint(core[11])? != prepared.capsule.sequence() {
            return Err(Error::AuthorityMismatch("released state sequence"));
        }
        Ok(core)
    }
    /// Next Load ordinal committed by this exact released wallet state.
    pub fn next_load_ordinal(&self, prepared: &PreparedOperation) -> Result<u128> {
        funding::uint(self.released_core(prepared)?[13])
    }
    /// Balance and lineage-adjusted burn committed by the released recursive head.
    pub fn balance(&self, prepared: &PreparedOperation) -> Result<(u128, u128, u128)> {
        let core = self.released_core(prepared)?;
        let gross = funding::uint(core[9])?;
        let burned = funding::uint(
            self.head.profile()
                [crate::bootstrap_context::LINEAGE_IO + crate::bootstrap_context::BURNED_TOTAL],
        )?;
        let spendable = gross
            .checked_sub(burned)
            .ok_or(Error::Malformed("released balance below lineage burn"))?;
        Ok((gross, burned, spendable))
    }
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
    /// Called exclusively by the transaction provider after anchoring a real Selected marker.
    fn require_selected_debit(
        &self,
        prepared: &PreparedOperation,
        checkpoint: [u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<()>;

    /// Fresh time-dependent controls immediately before irreversible selection.
    /// Recovery of an already selected operation does not rerun this check.
    fn before_selection(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        self.prepare_selection(capsule, parameters).map(drop)
    }

    /// Reconstruct the exact static witness, then perform every live check
    /// required for this selection. The provider may reuse this witness only
    /// within the same dispatch under its installed manifest.
    fn prepare_selection(
        &self,
        capsule: &RecoveryCapsule,
        parameters: &AuthenticatedParameters,
    ) -> Result<PreparedOperation>;
    /// Repeat all live controls with a witness this same owner has already
    /// reconstructed under the current installation and finality reader.
    fn recheck_selection(
        &self,
        prepared: &PreparedOperation,
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
    ) -> Result<CompletedOperation> {
        self.finalize_selected(prepared, receipt, parameters)
            .map(|(_, completed)| completed)
    }
    /// Return the checked witness to its owner for verifying the exact record
    /// read back from storage during this dispatch.
    fn finalize_selected(
        &self,
        prepared: PreparedOperation,
        receipt: &HardwareAdvanceReceipt,
        parameters: &AuthenticatedParameters,
    ) -> Result<(PreparedOperation, CompletedOperation)>;
    /// Authenticate the staged record against a preparation already checked
    /// under this installation. A cold owner must reconstruct that preparation.
    fn verify_staged_completion(
        &self,
        prepared: &PreparedOperation,
        record: &CompletionRecord,
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

#[derive(
    Clone, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.Capsule.v2")]
struct CapsuleFrame {
    version: u16,
    parameters_id: [u8; 32],
    manifest_digest: [u8; 32],
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
#[derive(Clone, PartialEq, Eq)]
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
        if Zeroizing::new(capsule.encode()?).as_slice() != bytes {
            return Err(Error::Malformed("capsule canonical frame"));
        }
        Ok(capsule)
    }
    fn validate(&self) -> Result<()> {
        let f = &self.frame;
        if f.version != RECOVERY_CAPSULE_VERSION
            || !(1..=7).contains(&f.kind)
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
            &f.manifest_digest,
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
        custody_digest(b"spartan-future-capsule-v2", bytes)
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
    /// Exact retained public output: 0 head, 1 Payment, 2 completion, 3 Unload claim.
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
/// Maximum complete IPM1 CreditStatus original, including its transport header.
pub const MAX_CREDIT_STATUS_ORIGINAL_BYTES: usize = 10_084;

/// Stable private reservation for one exact Credited original from a released Send.
pub fn wallet_archive_sent_replay_key(
    send_capsule_digest: &[u8; 32],
    wallet: &[u8; 32],
    credited: &[u8],
) -> Result<[u8; 32]> {
    if credited.is_empty() || credited.len() > MAX_CREDIT_STATUS_ORIGINAL_BYTES {
        return Err(Error::Malformed("ArchiveSent CreditStatus length"));
    }
    let mut identity = Vec::with_capacity(64 + credited.len());
    identity.extend_from_slice(send_capsule_digest);
    identity.extend_from_slice(wallet);
    identity.extend_from_slice(credited);
    Ok(custody_digest(b"spartan-archive-sent-action-v1", &identity))
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
