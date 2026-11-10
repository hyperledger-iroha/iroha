//! Native-suffix Unload stage carriage (owner go of 2026-11-06).
//!
//! A wallet whose latest head is not folded may Unload by disclosing its released suffix: the
//! genuine anchor Ω, every later Receive (or earlier suffix Unload) step in full, and the Unload
//! itself. Validators verify all of it natively before any payout. The ledger carries a suffix
//! in three stages appended to the bound `KagemushaWalletLedgerActionV1`:
//!
//! - Begin: the [`KagemushaWalletSuffixHeaderV1`] and [`KagemushaWalletSuffixAnchorV1`] frames;
//! - Append: one [`KagemushaWalletSuffixChunkV1`] of complete [`KagemushaWalletSuffixStepV1`]
//!   frames. Appends are independent of each other and may execute in any order;
//! - Finalize: the [`KagemushaWalletSuffixUnloadV1`] frame with the hardware receipt τ, whose
//!   `proof_digest` is the header's authority digest
//!   ([`kagemusha_wallet_suffix_authority_digest_v1`]).
//!
//! The header commits to the exact digest of every chunk, so a stage can only be the chunk the
//! wallet selected. Each chunk names the lineage-adjusted public fields before its first step
//! (`start`); the ledger proves those claims at Finalize by chaining every chunk from the
//! anchor Ω to the header's tip.
//!
//! Every value here is DATA. Decoding or validating one grants nothing: the ledger verifies
//! every proof, signature and opening and binds every identity before a payout. Encodings of
//! existing KAGEMUSHA objects are unchanged; they are embedded as their existing types.

mod native;
pub use native::*;
mod monetary;
pub use monetary::*;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use super::kagemusha_wallet_v1::{
    KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1, KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_INDEXED_EMPTY_OPENING_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_INDEXED_LEAF_OPENING_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, KAGEMUSHA_WALLET_SIGMA_PROOF_CAP_V1,
    KAGEMUSHA_WALLET_VERSION_V1, KagemushaWalletCertificateSetV1, KagemushaWalletCredentialV1,
    KagemushaWalletEffectV1, KagemushaWalletLineagePublicV1, KagemushaWalletLineageV1,
    KagemushaWalletReceiptSignerV1, KagemushaWalletReceiptV1, KagemushaWalletSchemeV1,
    KagemushaWalletSignerRoleV1, KagemushaWalletStateCommitmentV1, KagemushaWalletStateV1,
    KagemushaWalletStatementV1, KagemushaWalletStepProofV1, KagemushaWalletUnloadChargeV1,
    KagemushaWalletValidationErrorV1, decode_frame_v1, encode_frame_v1,
    kagemusha_wallet_account_digest_v1, kagemusha_wallet_is_canonical_field_v1,
    kagemusha_wallet_poseidon_bytes_v1, kagemusha_wallet_unload_account_payout_v1,
};
use crate::account::AccountId;

#[cfg(test)]
#[path = "kagemusha_wallet_suffix_unload_v1_tests.rs"]
mod tests;

type SuffixResult<T> = Result<T, KagemushaWalletValidationErrorV1>;

// ---------------------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------------------

/// The native suffix method: every step is verified natively by validators.
pub const KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1: u16 = 1;
/// Maximum complete canonical header frame.
pub const KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1: usize = 49_152;
/// Maximum complete canonical anchor frame.
pub const KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1: usize = 24_576;
/// Maximum complete canonical frame of one semantic step.
pub const KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1: usize = 65_536;
/// Most complete steps in one chunk. This bounds one Append's verification time inside the
/// block execution budget; the proof-call quota alone would admit 42 Receive steps.
pub const KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1: usize = 8;
/// Maximum chunk stream bytes: `LE32(len) || step frame` per step.
pub const KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 * (4 + KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1);
/// Maximum complete canonical chunk frame: its stream plus the fixed fields and framing.
pub const KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1: usize =
    KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1 + 4_096;
/// Most chunks (Appends) of one suffix.
pub const KAGEMUSHA_WALLET_SUFFIX_CHUNKS_MAX_V1: usize = 512;
/// Most steps of one suffix.
pub const KAGEMUSHA_WALLET_SUFFIX_STEPS_MAX_V1: usize = 4_096;
/// Maximum complete canonical Finalize frame.
pub const KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1: usize = 16_384;
/// Smallest stream entry: the length prefix and at least one frame byte.
const STREAM_ENTRY_MIN_BYTES: usize = 5;

/// Exact low-leaf opening transcript of an indexed-map update.
const LOW_LEAF_OPENING_BYTES: usize = KAGEMUSHA_WALLET_INDEXED_LEAF_OPENING_TRANSCRIPT_BYTES_V1;
/// Exact empty-slot opening transcript of an indexed-map insertion.
const EMPTY_SLOT_OPENING_BYTES: usize = KAGEMUSHA_WALLET_INDEXED_EMPTY_OPENING_TRANSCRIPT_BYTES_V1;

// ---------------------------------------------------------------------------------------
// Digests
// ---------------------------------------------------------------------------------------

/// Poseidon `P_bytes` domain of the suffix authority digest over the canonical header frame.
pub const KAGEMUSHA_WALLET_SUFFIX_AUTHORITY_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgwsufa1");
/// Prefix of the SHA-256 suffix digests ([`kagemusha_wallet_suffix_digest_v1`]).
pub const KAGEMUSHA_WALLET_SUFFIX_DIGEST_PREFIX_V1: &[u8] =
    b"iroha:kagemusha:wallet:suffix-unload:v1:";

/// Roles of the SHA-256 suffix digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KagemushaWalletSuffixDigestRoleV1 {
    /// Canonical anchor frame; the header's `anchor_digest`.
    Anchor,
    /// Canonical chunk frame; one entry of the header's `chunk_digests`.
    Chunk,
    /// Canonical Finalize frame.
    Unload,
    /// Payout source: authority digest followed by the Finalize frame digest.
    PayoutSource,
    /// Installed `VerifierPackV1` original bytes; the header's `verifier_definition`.
    VerifierDefinition,
    /// Canonical exact recovery-map opening pair selected by the header.
    RecoveryWitness,
}

impl KagemushaWalletSuffixDigestRoleV1 {
    /// Exact ASCII role label hashed after [`KAGEMUSHA_WALLET_SUFFIX_DIGEST_PREFIX_V1`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Chunk => "chunk",
            Self::Unload => "unload",
            Self::PayoutSource => "payout-source",
            Self::VerifierDefinition => "verifier-definition",
            Self::RecoveryWitness => "recovery-witness",
        }
    }
}

/// `Hs(role, body) = SHA-256(prefix || role || 0x00 || LE64(len(body)) || body)`.
#[must_use]
pub fn kagemusha_wallet_suffix_digest_v1(
    role: KagemushaWalletSuffixDigestRoleV1,
    body: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_WALLET_SUFFIX_DIGEST_PREFIX_V1);
    hasher.update(role.as_str().as_bytes());
    hasher.update([0]);
    hasher.update((body.len() as u64).to_le_bytes());
    hasher.update(body);
    hasher.finalize().into()
}

/// Suffix authority digest `P_bytes(kgwsufa1, header frame)`: a nonzero canonical σ-field
/// value. It is the Unload receipt's opaque `proof_digest`, so τ binds the complete header.
///
/// # Errors
/// Rejects the (negligible) zero output instead of weakening the receipt rule.
pub fn kagemusha_wallet_suffix_authority_digest_v1(header_frame: &[u8]) -> SuffixResult<[u8; 32]> {
    let digest = kagemusha_wallet_poseidon_bytes_v1(
        KAGEMUSHA_WALLET_SUFFIX_AUTHORITY_DOMAIN_V1,
        header_frame,
    );
    if digest == [0; 32] || !kagemusha_wallet_is_canonical_field_v1(&digest) {
        return Err(invalid("suffix.authority"));
    }
    Ok(digest)
}

/// Verifier-definition identity of the installed verifier pack original.
#[must_use]
pub fn kagemusha_wallet_suffix_verifier_definition_v1(pack_original: &[u8]) -> [u8; 32] {
    kagemusha_wallet_suffix_digest_v1(
        KagemushaWalletSuffixDigestRoleV1::VerifierDefinition,
        pack_original,
    )
}

/// Payout source recorded in the unchanged Unload payout row:
/// `Hs("payout-source", authority || Hs("unload", Finalize frame))`. It identifies the exact
/// suffix claim, so an exact retry is idempotent and any other claim on the nullifier
/// conflicts. `unload_digest` is the [`KagemushaWalletSuffixDigestRoleV1::Unload`] digest.
#[must_use]
pub fn kagemusha_wallet_suffix_payout_source_v1(
    authority: &[u8; 32],
    unload_digest: &[u8; 32],
) -> [u8; 32] {
    let mut body = [0; 64];
    body[..32].copy_from_slice(authority);
    body[32..].copy_from_slice(unload_digest);
    kagemusha_wallet_suffix_digest_v1(KagemushaWalletSuffixDigestRoleV1::PayoutSource, &body)
}

/// Commit to the exact selected recovery-map insertion pair.
///
/// The preimage is the canonical Norito `Vec<Vec<u8>>` frame containing the low-leaf
/// opening followed by the empty-slot opening; the digest uses the `recovery-witness` role.
///
/// # Errors
/// Rejects a missing, additional, wrongly sized or unencodable opening.
pub fn kagemusha_wallet_suffix_recovery_witness_digest_v1(
    openings: &[Vec<u8>],
) -> SuffixResult<[u8; 32]> {
    require_insert_openings("suffix.recovery_witness", openings)?;
    let original = norito::encode_canonical(&openings.to_vec())
        .map_err(|_| invalid("suffix.recovery_witness"))?;
    Ok(kagemusha_wallet_suffix_digest_v1(
        KagemushaWalletSuffixDigestRoleV1::RecoveryWitness,
        &original,
    ))
}

// ---------------------------------------------------------------------------------------
// Small shared checks
// ---------------------------------------------------------------------------------------

fn invalid(field: &'static str) -> KagemushaWalletValidationErrorV1 {
    KagemushaWalletValidationErrorV1::InvalidField { field }
}

fn require_version(field: &'static str, version: u16) -> SuffixResult<()> {
    if version == KAGEMUSHA_WALLET_VERSION_V1 {
        Ok(())
    } else {
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { field, version })
    }
}

fn require_nonzero(field: &'static str, digest: &[u8; 32]) -> SuffixResult<()> {
    if *digest == [0; 32] {
        Err(invalid(field))
    } else {
        Ok(())
    }
}

fn require_scheme(field: &'static str, actual: &[u8; 32], expected: &[u8; 32]) -> SuffixResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch { field })
    }
}

fn require_len(field: &'static str, bytes: &[u8], exact: usize) -> SuffixResult<()> {
    if bytes.len() == exact {
        Ok(())
    } else {
        Err(invalid(field))
    }
}

fn count(field: &'static str, value: u128) -> SuffixResult<usize> {
    usize::try_from(value).map_err(|_| invalid(field))
}

/// Exactly the indexed-map update pair: occupied low leaf, then intermediate empty slot.
fn require_insert_openings(field: &'static str, openings: &[Vec<u8>]) -> SuffixResult<()> {
    match openings {
        [low, empty]
            if low.len() == LOW_LEAF_OPENING_BYTES && empty.len() == EMPTY_SLOT_OPENING_BYTES =>
        {
            Ok(())
        }
        _ => Err(invalid(field)),
    }
}

/// Exactly the certificates `required` names, each with its role, under `scheme_id`.
fn require_exact_certificates(
    certificates: &KagemushaWalletCertificateSetV1,
    scheme_id: &[u8; 32],
    required: &[([u8; 32], KagemushaWalletSignerRoleV1)],
) -> SuffixResult<()> {
    certificates.validate()?;
    let mut expected: Vec<[u8; 32]> = required.iter().map(|(digest, _)| *digest).collect();
    expected.sort_unstable();
    expected.dedup();
    if certificates.digests() != expected {
        return Err(invalid("suffix.certificates.set"));
    }
    for (digest, role) in required {
        let certificate = certificates.certificate(digest, *role)?;
        require_scheme(
            "suffix.certificate.scheme_id",
            &certificate.body.scheme_id,
            scheme_id,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------------------

/// Immutable description of one suffix Unload, carried by Begin. Its canonical frame is the
/// input of the authority digest that the Unload receipt τ binds.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixHeaderV1"
)]
#[repr(align(16))]
pub struct KagemushaWalletSuffixHeaderV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Exactly [`KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1`].
    pub method: u16,
    /// Network of the scheme.
    pub network_id: [u8; 32],
    /// Scheme.
    pub scheme_id: [u8; 32],
    /// Wallet incarnation.
    pub wallet_id: [u8; 32],
    /// [`kagemusha_wallet_suffix_verifier_definition_v1`] of the installed verifier pack.
    pub verifier_definition: [u8; 32],
    /// Signed artifact manifest digest of the installed verifier pack.
    pub artifact_manifest: [u8; 32],
    /// Native custody identity of the released source capsule (DATA for the ledger).
    pub source_capsule: [u8; 32],
    /// Native request identity (DATA for the ledger).
    pub request_id: [u8; 32],
    /// [`KagemushaWalletSuffixDigestRoleV1::Anchor`] digest of the anchor frame.
    pub anchor_digest: [u8; 32],
    /// Sequence of the anchor state (the genuine Ω head).
    pub anchor_sequence: u128,
    /// Steps after the anchor; the tip is `anchor_sequence + step_count`.
    pub step_count: u128,
    /// Appends; equal to `chunk_digests.len()`.
    pub chunk_count: u128,
    /// Total stream bytes of every chunk's `data`.
    pub stream_bytes: u128,
    /// [`KagemushaWalletSuffixDigestRoleV1::Chunk`] digest of each chunk frame, in order.
    pub chunk_digests: Vec<[u8; 32]>,
    /// State commitment after the last step: the Unload predecessor.
    pub tip: KagemushaWalletStateCommitmentV1,
    /// Sequence of the tip state.
    pub tip_sequence: u128,
    /// Lineage-adjusted public fields at the tip.
    pub tip_adjusted: KagemushaWalletLineagePublicV1,
    /// The Unload statement; its predecessor is the tip.
    pub statement: KagemushaWalletStatementV1,
    /// The Unload σ.
    pub sigma: KagemushaWalletStepProofV1,
    /// Native recovery-witness selection identity (DATA for the ledger).
    pub recovery_witness_digest: [u8; 32],
    /// Paid account; its digest is the credential's account digest.
    pub account: AccountId,
    /// Online charge: present exactly when the Unload effect names a charge quote.
    pub charge: KagemushaWalletUnloadChargeV1,
}

/// Payout DATA of a shape-validated header. This alone grants no monetary authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(align(16))]
pub struct KagemushaWalletSuffixPayoutTermsV1 {
    /// Unload nullifier; the ledger pays it exactly once.
    pub nullifier: [u8; 32],
    /// Redemption ordinal.
    pub redeem_ordinal: u128,
    /// Net offline value subtracted from the wallet: the reserve liability released.
    pub amount: u128,
    /// Online charge paid to the charge quote's beneficiary.
    pub online_charge: u128,
    /// Account payout `amount - online_charge`.
    pub account_payout: u128,
    /// Charge quote digest; zero without one.
    pub charge_quote: [u8; 32],
    /// Account digest of the charge beneficiary; zero without a charge quote.
    pub beneficiary_account_digest: [u8; 32],
}

impl KagemushaWalletSuffixHeaderV1 {
    /// Validate the header's self-contained shape and bindings. No proof or signature is
    /// checked here.
    ///
    /// # Errors
    /// Rejects another version or method, zero identities, a partition outside the bounds, a
    /// zero chunk digest, inconsistent sequences or stream length, a tip that is not the
    /// adjusted head, a statement that is not an Unload of the tip under the tip's lineage
    /// fields, an empty or oversized σ, and a charge that does not match the effect.
    pub fn validate(&self) -> SuffixResult<()> {
        require_version("suffix_header.version", self.version)?;
        if self.method != KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1 {
            return Err(invalid("suffix_header.method"));
        }
        for (field, digest) in [
            ("suffix_header.network_id", &self.network_id),
            ("suffix_header.scheme_id", &self.scheme_id),
            ("suffix_header.wallet_id", &self.wallet_id),
            (
                "suffix_header.verifier_definition",
                &self.verifier_definition,
            ),
            ("suffix_header.artifact_manifest", &self.artifact_manifest),
            ("suffix_header.source_capsule", &self.source_capsule),
            ("suffix_header.request_id", &self.request_id),
            ("suffix_header.anchor_digest", &self.anchor_digest),
            (
                "suffix_header.recovery_witness_digest",
                &self.recovery_witness_digest,
            ),
        ] {
            require_nonzero(field, digest)?;
        }
        let steps = count("suffix_header.step_count", self.step_count)?;
        let chunks = count("suffix_header.chunk_count", self.chunk_count)?;
        if chunks == 0
            || chunks > KAGEMUSHA_WALLET_SUFFIX_CHUNKS_MAX_V1
            || chunks != self.chunk_digests.len()
            || steps < chunks
            || steps > KAGEMUSHA_WALLET_SUFFIX_STEPS_MAX_V1
            || steps > chunks * KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1
        {
            return Err(invalid("suffix_header.partition"));
        }
        if self.chunk_digests.contains(&[0; 32]) {
            return Err(invalid("suffix_header.chunk_digests"));
        }
        let stream = count("suffix_header.stream_bytes", self.stream_bytes)?;
        if stream < steps * STREAM_ENTRY_MIN_BYTES
            || stream > chunks * KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1
        {
            return Err(invalid("suffix_header.stream_bytes"));
        }
        if self.anchor_sequence.checked_add(self.step_count) != Some(self.tip_sequence) {
            return Err(invalid("suffix_header.tip_sequence"));
        }
        if !self.tip.is_complete() {
            return Err(invalid("suffix_header.tip"));
        }
        let adjusted = &self.tip_adjusted;
        adjusted.validate()?;
        if adjusted.head != self.tip {
            return Err(invalid("suffix_header.tip_adjusted.head"));
        }
        require_scheme(
            "suffix_header.tip_adjusted.scheme_id",
            &adjusted.scheme_id,
            &self.scheme_id,
        )?;
        if adjusted.wallet_id != self.wallet_id {
            return Err(invalid("suffix_header.tip_adjusted.wallet_id"));
        }
        let statement = &self.statement;
        // Exactly the ordinary Unload consumer rule against Ω(tip): head, credential, burned
        // total, pending root, controls, lifecycle and the wallet's nullifier.
        statement.validate_against_lineage(adjusted)?;
        if !matches!(statement.effect, KagemushaWalletEffectV1::Unload { .. }) {
            return Err(invalid("suffix_header.statement.effect"));
        }
        if self.tip_sequence.checked_add(1) != Some(statement.sequence) {
            return Err(invalid("suffix_header.statement.sequence"));
        }
        self.sigma.validate()?;
        if self.sigma.bytes.len() > KAGEMUSHA_WALLET_SIGMA_PROOF_CAP_V1 {
            return Err(invalid("suffix_header.sigma"));
        }
        self.payout_terms().map(|_| ())
    }

    /// Payout terms named by the Unload effect and the charge, checked like an ordinary Unload
    /// claim: a quote is present exactly when the effect names one, binds the effect's terms
    /// and wallet, and names the carried beneficiary. Signatures are checked by
    /// [`KagemushaWalletSuffixUnloadV1::verify_terms`].
    ///
    /// # Errors
    /// Rejects a non-Unload effect, a mismatched charge, and an online charge above the amount.
    pub fn payout_terms(&self) -> SuffixResult<KagemushaWalletSuffixPayoutTermsV1> {
        let effect = self.statement.effect;
        let KagemushaWalletEffectV1::Unload {
            nullifier,
            redeem_ordinal,
            amount,
            online_charge,
            charge_quote,
        } = effect
        else {
            return Err(invalid("suffix_header.statement.effect"));
        };
        let beneficiary_account_digest = match (&self.charge, charge_quote == [0; 32]) {
            (KagemushaWalletUnloadChargeV1::None, true) => [0; 32],
            (KagemushaWalletUnloadChargeV1::None, false)
            | (KagemushaWalletUnloadChargeV1::Quoted { .. }, true) => {
                return Err(invalid("suffix_header.charge"));
            }
            (KagemushaWalletUnloadChargeV1::Quoted { quote, beneficiary }, false) => {
                quote.validate()?;
                require_scheme(
                    "suffix_header.charge.scheme_id",
                    &quote.body.scheme_id,
                    &self.scheme_id,
                )?;
                if quote.body.asset_digest != self.statement.asset_digest {
                    return Err(invalid("suffix_header.charge.asset_digest"));
                }
                quote.require_unload_effect(&effect, &self.wallet_id)?;
                let digest = kagemusha_wallet_account_digest_v1(beneficiary)?;
                if digest != quote.body.beneficiary_account_digest {
                    return Err(invalid("suffix_header.charge.beneficiary"));
                }
                digest
            }
        };
        Ok(KagemushaWalletSuffixPayoutTermsV1 {
            nullifier,
            redeem_ordinal,
            amount,
            online_charge,
            account_payout: kagemusha_wallet_unload_account_payout_v1(amount, online_charge)?,
            charge_quote,
            beneficiary_account_digest,
        })
    }

    /// Validate and encode the bounded canonical frame.
    ///
    /// # Errors
    /// Rejects an invalid header or an oversized frame.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1)
    }

    /// Decode one canonical header frame for `expected_scheme_id`.
    ///
    /// # Errors
    /// Rejects, in order, an oversized or noncanonical frame, another scheme, and what
    /// [`Self::validate`] rejects.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> SuffixResult<Self> {
        let header: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1)?;
        require_scheme(
            "suffix_header.scheme_id",
            &header.scheme_id,
            expected_scheme_id,
        )?;
        header.validate()?;
        Ok(header)
    }

    /// Ordinal of the Finalize stage: `chunk_count + 1`.
    ///
    /// # Errors
    /// Rejects an overflowing count.
    pub fn finalize_ordinal(&self) -> SuffixResult<u128> {
        self.chunk_count.checked_add(1).ok_or(
            KagemushaWalletValidationErrorV1::ArithmeticOverflow {
                field: "suffix_header.chunk_count",
            },
        )
    }

    /// Expected chunk digest of Append `ordinal` (1-based).
    #[must_use]
    pub fn chunk_digest(&self, ordinal: u128) -> Option<[u8; 32]> {
        let index = usize::try_from(ordinal.checked_sub(1)?).ok()?;
        self.chunk_digests.get(index).copied()
    }
}

// ---------------------------------------------------------------------------------------
// Anchor
// ---------------------------------------------------------------------------------------

/// The genuine folded anchor: the receiver's current credential and enrollment certificates,
/// its genuine Ω and the full anchor state opening.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixAnchorV1"
)]
pub struct KagemushaWalletSuffixAnchorV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Receiver credential every suffix step runs under.
    pub credential: KagemushaWalletCredentialV1,
    /// The credential's enrollment certificates.
    pub certificates: KagemushaWalletCertificateSetV1,
    /// Genuine self-verified Ω of the anchor head.
    pub lineage: KagemushaWalletLineageV1,
    /// Full anchor state opening of Ω's head.
    pub state: KagemushaWalletStateV1,
}

impl KagemushaWalletSuffixAnchorV1 {
    /// Validate the anchor's shape.
    ///
    /// # Errors
    /// Rejects another version and an invalid credential, certificate set, lineage or state.
    pub fn validate(&self) -> SuffixResult<()> {
        require_version("suffix_anchor.version", self.version)?;
        self.credential.validate()?;
        self.certificates.validate()?;
        self.lineage.validate()?;
        self.state.validate()?;
        Ok(())
    }

    /// Bind the anchor to `header`: scheme, wallet, credential, account and anchor sequence,
    /// and the anchor state opening of Ω's head.
    ///
    /// # Errors
    /// Rejects any differing identity, account or sequence, and an opening of another head.
    pub fn require_header(&self, header: &KagemushaWalletSuffixHeaderV1) -> SuffixResult<()> {
        self.validate()?;
        let body = &self.credential.body;
        require_scheme(
            "suffix_anchor.credential.scheme_id",
            &body.scheme_id,
            &header.scheme_id,
        )?;
        let public = &self.lineage.public;
        require_scheme(
            "suffix_anchor.lineage.scheme_id",
            &public.scheme_id,
            &header.scheme_id,
        )?;
        for (field, matches) in [
            (
                "suffix_anchor.credential.wallet_id",
                body.wallet_id == header.wallet_id,
            ),
            (
                "suffix_anchor.lineage.wallet_id",
                public.wallet_id == header.wallet_id,
            ),
            (
                "suffix_anchor.credential.asset_digest",
                body.asset_digest == header.statement.asset_digest,
            ),
            (
                "suffix_anchor.credential",
                self.credential.credential_digest() == header.statement.credential_digest,
            ),
            (
                "suffix_anchor.lineage.credential_digest",
                public.credential_digest == header.statement.credential_digest,
            ),
            (
                "suffix_anchor.lineage.relation_id",
                public.relation_id == header.statement.relation_id,
            ),
            (
                "suffix_anchor.state.sequence",
                self.state.core.sequence == header.anchor_sequence,
            ),
            (
                "suffix_anchor.state.wallet_id",
                self.state.core.wallet_id == header.wallet_id,
            ),
        ] {
            if !matches {
                return Err(invalid(field));
            }
        }
        if kagemusha_wallet_account_digest_v1(&header.account)? != body.account_digest {
            return Err(invalid("suffix_header.account"));
        }
        if self.state.commitment()? != public.head {
            return Err(invalid("suffix_anchor.state"));
        }
        Ok(())
    }

    /// Verify the credential's enrollment certificate set and issuer signature under
    /// `scheme`. The genuine Ω and the state opening are the native verifier's.
    ///
    /// # Errors
    /// Rejects certificates or a credential signature that do not verify under `scheme`.
    pub fn verify_signatures(&self, scheme: &KagemushaWalletSchemeV1) -> SuffixResult<()> {
        self.certificates.verify(scheme)?;
        let issuer = self.certificates.certificate(
            &self.credential.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )?;
        self.credential.verify(scheme, issuer)
    }

    /// Validate and encode the bounded canonical frame.
    ///
    /// # Errors
    /// Rejects an invalid anchor or an oversized frame.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1)
    }

    /// Decode one canonical anchor frame for `expected_scheme_id`.
    ///
    /// # Errors
    /// Rejects an oversized or noncanonical frame, another scheme and an invalid anchor.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> SuffixResult<Self> {
        let anchor: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1)?;
        require_scheme(
            "suffix_anchor.credential.scheme_id",
            &anchor.credential.body.scheme_id,
            expected_scheme_id,
        )?;
        anchor.validate()?;
        Ok(anchor)
    }
}

// ---------------------------------------------------------------------------------------
// Chunk and steps
// ---------------------------------------------------------------------------------------

/// One Append: complete steps after a claimed start. Chunks are independent of each other.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixChunkV1"
)]
#[repr(align(16))]
pub struct KagemushaWalletSuffixChunkV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Zero-based chunk index; the Append ordinal is `chunk_index + 1`.
    pub chunk_index: u128,
    /// Stream bytes of all earlier chunks.
    pub stream_offset: u128,
    /// Lineage-adjusted public fields before the first step (claimed; proven at Finalize).
    pub start: KagemushaWalletLineagePublicV1,
    /// `LE32(len) || canonical step frame`, repeated, for 1 to
    /// [`KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1`] complete steps.
    pub data: Vec<u8>,
}

impl KagemushaWalletSuffixChunkV1 {
    /// Validate the chunk's shape, including its stream framing.
    ///
    /// # Errors
    /// Rejects another version, an invalid start and a malformed stream.
    pub fn validate(&self) -> SuffixResult<()> {
        require_version("suffix_chunk.version", self.version)?;
        self.start.validate()?;
        self.step_frames().map(|_| ())
    }

    /// The exact step frames of `data`, in order, without decoding them.
    ///
    /// # Errors
    /// Rejects an empty or oversized stream, a truncated entry, a zero-length or oversized
    /// frame, and more than [`KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1`] entries.
    pub fn step_frames(&self) -> SuffixResult<Vec<&[u8]>> {
        if self.data.is_empty() || self.data.len() > KAGEMUSHA_WALLET_SUFFIX_CHUNK_DATA_MAX_BYTES_V1
        {
            return Err(invalid("suffix_chunk.data"));
        }
        let mut frames = Vec::new();
        let mut rest = self.data.as_slice();
        while !rest.is_empty() {
            if frames.len() == KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 || rest.len() < 4 {
                return Err(invalid("suffix_chunk.data"));
            }
            let (prefix, tail) = rest.split_at(4);
            let length = u32::from_le_bytes(prefix.try_into().expect("four bytes")) as usize;
            if length == 0
                || length > KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1
                || length > tail.len()
            {
                return Err(invalid("suffix_chunk.data"));
            }
            let (frame, next) = tail.split_at(length);
            frames.push(frame);
            rest = next;
        }
        Ok(frames)
    }

    /// Decode every step frame for `scheme_id`.
    ///
    /// # Errors
    /// Rejects what [`Self::step_frames`] rejects and any step frame that
    /// [`KagemushaWalletSuffixStepV1::decode_canonical`] rejects.
    pub fn decode_steps(
        &self,
        scheme_id: &[u8; 32],
    ) -> SuffixResult<Vec<KagemushaWalletSuffixStepV1>> {
        self.step_frames()?
            .into_iter()
            .map(|frame| KagemushaWalletSuffixStepV1::decode_canonical(frame, scheme_id))
            .collect()
    }

    /// Build the stream of `frames` (`LE32(len) || frame` each).
    ///
    /// # Errors
    /// Rejects an empty list, too many steps and an empty or oversized frame.
    pub fn stream(frames: &[Vec<u8>]) -> SuffixResult<Vec<u8>> {
        if frames.is_empty() || frames.len() > KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1 {
            return Err(invalid("suffix_chunk.data"));
        }
        let mut data = Vec::with_capacity(frames.iter().map(|frame| frame.len() + 4).sum());
        for frame in frames {
            if frame.is_empty() || frame.len() > KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1 {
                return Err(invalid("suffix_chunk.data"));
            }
            let length = u32::try_from(frame.len()).map_err(|_| invalid("suffix_chunk.data"))?;
            data.extend_from_slice(&length.to_le_bytes());
            data.extend_from_slice(frame);
        }
        Ok(data)
    }

    /// Validate and encode the bounded canonical frame.
    ///
    /// # Errors
    /// Rejects an invalid chunk or an oversized frame.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1)
    }

    /// Decode one canonical chunk frame for `expected_scheme_id`. Steps stay undecoded.
    ///
    /// # Errors
    /// Rejects an oversized or noncanonical frame, another scheme and an invalid chunk.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> SuffixResult<Self> {
        let chunk: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1)?;
        require_scheme(
            "suffix_chunk.start.scheme_id",
            &chunk.start.scheme_id,
            expected_scheme_id,
        )?;
        chunk.validate()?;
        Ok(chunk)
    }
}

/// Role of one Receive original, in the required ascending order.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixOriginalRoleV1"
)]
pub enum KagemushaWalletSuffixOriginalRoleV1 {
    /// The unchanged full canonical `KagemushaWalletRequestV1`.
    #[codec(index = 1)]
    Request,
    /// The exact canonical Payment.
    #[codec(index = 2)]
    Payment,
    /// The payer's issuer-authenticated credential.
    #[codec(index = 3)]
    PayerCredential,
    /// The payer's exact certificate set.
    #[codec(index = 4)]
    PayerCertificates,
}

impl KagemushaWalletSuffixOriginalRoleV1 {
    /// Every role, in the required order.
    pub const ALL: [Self; 4] = [
        Self::Request,
        Self::Payment,
        Self::PayerCredential,
        Self::PayerCertificates,
    ];

    /// Maximum original bytes inherited from the current owners.
    #[must_use]
    pub const fn max_bytes(self) -> usize {
        match self {
            Self::Request | Self::Payment | Self::PayerCertificates => {
                KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
            }
            Self::PayerCredential => KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
        }
    }
}

/// One exact original of a Receive step.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixOriginalV1"
)]
pub struct KagemushaWalletSuffixOriginalV1 {
    /// Role.
    pub role: KagemushaWalletSuffixOriginalRoleV1,
    /// Exact original canonical bytes.
    pub bytes: Vec<u8>,
}

/// Lineage credit-tree witness of one Receive.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixCreditWitnessV1"
)]
pub enum KagemushaWalletSuffixCreditWitnessV1 {
    /// The credit digest is already present; its first value is preserved.
    #[codec(index = 1)]
    Present {
        /// Membership opening transcript (1,124 bytes).
        membership: Vec<u8>,
    },
    /// The credit digest is inserted.
    #[codec(index = 2)]
    Inserted {
        /// Low-leaf opening transcript (1,124 bytes).
        low: Vec<u8>,
        /// Empty-slot opening transcript (1,028 bytes).
        empty: Vec<u8>,
    },
}

/// Exact signed charge originals of a charged Unload inside a suffix.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixChargeOriginalsV1"
)]
pub struct KagemushaWalletSuffixChargeOriginalsV1 {
    /// Canonical RegulatoryPolicy-signed `KagemushaWalletChargeQuoteV1` frame.
    pub quote: Vec<u8>,
    /// Canonical certificate-set frame containing the quote's signer.
    pub certificate_set: Vec<u8>,
}

/// Kind of one suffix step.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixStepKindV1"
)]
pub enum KagemushaWalletSuffixStepKindV1 {
    /// A received Payment with its lineage credit witness.
    #[codec(index = 1)]
    Receive {
        /// Credit-tree witness.
        credit: KagemushaWalletSuffixCreditWitnessV1,
    },
    /// An earlier released Unload of this wallet (suffix or ordinary) whose debit the suffix
    /// carries. Its hardware receipt signs `proof_digest`.
    #[codec(index = 2)]
    PriorUnload {
        /// The exact nonzero `proof_digest` that Unload's receipt signs: an earlier
        /// suffix-authority digest or an ordinary package proof digest.
        proof_digest: [u8; 32],
        /// Exact signed charge originals; present exactly when its statement names a quote.
        charge: Option<KagemushaWalletSuffixChargeOriginalsV1>,
    },
}

/// One complete semantic step of the suffix.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixStepV1"
)]
pub struct KagemushaWalletSuffixStepV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Step kind.
    pub kind: KagemushaWalletSuffixStepKindV1,
    /// Full predecessor state opening.
    pub before: KagemushaWalletStateV1,
    /// Full successor state opening.
    pub after: KagemushaWalletStateV1,
    /// The step statement.
    pub statement: KagemushaWalletStatementV1,
    /// The step σ.
    pub sigma: KagemushaWalletStepProofV1,
    /// The step's original hardware receipt.
    pub receipt: KagemushaWalletReceiptV1,
    /// Exactly the indexed-map update pair (low leaf, empty slot): consumed credit for a
    /// Receive, recovery for a prior Unload.
    pub map_openings: Vec<Vec<u8>>,
    /// Receive: exactly one original of each role in role order. Prior Unload: none.
    pub originals: Vec<KagemushaWalletSuffixOriginalV1>,
}

impl KagemushaWalletSuffixStepV1 {
    /// Validate the step's shape. No proof, signature or opening is verified here.
    ///
    /// # Errors
    /// Rejects another version, invalid states, statement, σ or receipt, a statement that does
    /// not join `before` to `after` by sequence, wrong opening sizes, and missing, extra,
    /// unordered or oversized originals.
    pub fn validate(&self) -> SuffixResult<()> {
        require_version("suffix_step.version", self.version)?;
        self.before.validate()?;
        self.after.validate()?;
        self.statement.validate()?;
        self.sigma.validate()?;
        if self.sigma.bytes.len() > KAGEMUSHA_WALLET_SIGMA_PROOF_CAP_V1 {
            return Err(invalid("suffix_step.sigma"));
        }
        self.receipt.validate()?;
        if self.before.core.sequence.checked_add(1) != Some(self.after.core.sequence)
            || self.statement.sequence != self.after.core.sequence
        {
            return Err(invalid("suffix_step.sequence"));
        }
        require_insert_openings("suffix_step.map_openings", &self.map_openings)?;
        match &self.kind {
            KagemushaWalletSuffixStepKindV1::Receive { credit } => {
                if !matches!(
                    self.statement.effect,
                    KagemushaWalletEffectV1::Receive { .. }
                ) {
                    return Err(invalid("suffix_step.statement.effect"));
                }
                match credit {
                    KagemushaWalletSuffixCreditWitnessV1::Present { membership } => {
                        require_len("suffix_step.credit", membership, LOW_LEAF_OPENING_BYTES)?;
                    }
                    KagemushaWalletSuffixCreditWitnessV1::Inserted { low, empty } => {
                        require_len("suffix_step.credit", low, LOW_LEAF_OPENING_BYTES)?;
                        require_len("suffix_step.credit", empty, EMPTY_SLOT_OPENING_BYTES)?;
                    }
                }
                if self.originals.len() != KagemushaWalletSuffixOriginalRoleV1::ALL.len() {
                    return Err(invalid("suffix_step.originals"));
                }
                for (original, role) in self
                    .originals
                    .iter()
                    .zip(KagemushaWalletSuffixOriginalRoleV1::ALL)
                {
                    if original.role != role
                        || original.bytes.is_empty()
                        || original.bytes.len() > role.max_bytes()
                    {
                        return Err(invalid("suffix_step.originals"));
                    }
                }
            }
            KagemushaWalletSuffixStepKindV1::PriorUnload {
                proof_digest,
                charge,
            } => {
                let KagemushaWalletEffectV1::Unload { charge_quote, .. } = self.statement.effect
                else {
                    return Err(invalid("suffix_step.statement.effect"));
                };
                if *proof_digest == [0; 32]
                    || !kagemusha_wallet_is_canonical_field_v1(proof_digest)
                    || !self.originals.is_empty()
                    || charge.is_some() == (charge_quote == [0; 32])
                {
                    return Err(invalid("suffix_step.prior_unload"));
                }
                if let Some(charge) = charge
                    && (charge.quote.is_empty()
                        || charge.quote.len() > KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1
                        || charge.certificate_set.is_empty()
                        || charge.certificate_set.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)
                {
                    return Err(invalid("suffix_step.prior_unload.charge"));
                }
            }
        }
        Ok(())
    }

    /// The step's exact Receive original of `role`, if it is a Receive.
    #[must_use]
    pub fn original(&self, role: KagemushaWalletSuffixOriginalRoleV1) -> Option<&[u8]> {
        self.originals
            .iter()
            .find(|original| original.role == role)
            .map(|original| original.bytes.as_slice())
    }

    /// Validate and encode the bounded canonical frame.
    ///
    /// # Errors
    /// Rejects an invalid step or an oversized frame.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1)
    }

    /// Decode one canonical step frame for `expected_scheme_id`.
    ///
    /// # Errors
    /// Rejects an oversized or noncanonical frame, another scheme and an invalid step.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> SuffixResult<Self> {
        let step: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1)?;
        require_scheme(
            "suffix_step.statement.scheme_id",
            &step.statement.scheme_id,
            expected_scheme_id,
        )?;
        step.validate()?;
        Ok(step)
    }
}

// ---------------------------------------------------------------------------------------
// Finalize
// ---------------------------------------------------------------------------------------

/// The Unload of a suffix: its credential and exact certificates, successor state, hardware
/// receipt τ and recovery-map insertion openings.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::KagemushaWalletSuffixUnloadV1"
)]
pub struct KagemushaWalletSuffixUnloadV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Credential the Unload ran under.
    pub credential: KagemushaWalletCredentialV1,
    /// Exactly the credential's issuer certificate and, with a charge quote, the quote's
    /// RegulatoryPolicy-role signer certificate.
    pub certificates: KagemushaWalletCertificateSetV1,
    /// Full successor state opening.
    pub after: KagemushaWalletStateV1,
    /// Hardware receipt τ; its `proof_digest` is the header's authority digest.
    pub receipt: KagemushaWalletReceiptV1,
    /// Exactly the recovery-map insertion pair (low leaf, empty slot).
    pub map_openings: Vec<Vec<u8>>,
}

impl KagemushaWalletSuffixUnloadV1 {
    /// Validate the shape.
    ///
    /// # Errors
    /// Rejects another version, an invalid credential, set, state or receipt, and wrong
    /// opening sizes.
    pub fn validate(&self) -> SuffixResult<()> {
        require_version("suffix_unload.version", self.version)?;
        self.credential.validate()?;
        self.certificates.validate()?;
        self.after.validate()?;
        self.receipt.validate()?;
        require_insert_openings("suffix_unload.map_openings", &self.map_openings)
    }

    /// Bind the Unload to `header` without checking any signature: credential scheme, wallet,
    /// digest, asset and the wallet's nullifier (as an ordinary Unload), the paid account, the
    /// exact certificate set (issuer, plus the quote signer when charged) and the successor
    /// commitment. Returns the header's payout terms.
    ///
    /// # Errors
    /// Rejects another scheme, wallet, credential, account or certificate set and a successor
    /// of another commitment.
    pub fn require_header(
        &self,
        header: &KagemushaWalletSuffixHeaderV1,
    ) -> SuffixResult<KagemushaWalletSuffixPayoutTermsV1> {
        self.validate()?;
        let terms = header.payout_terms()?;
        let credential = &self.credential;
        let body = &credential.body;
        require_scheme(
            "suffix_unload.credential.scheme_id",
            &body.scheme_id,
            &header.scheme_id,
        )?;
        if body.wallet_id != header.wallet_id {
            return Err(invalid("suffix_unload.credential.wallet_id"));
        }
        header.statement.validate_for_credential(credential)?;
        if kagemusha_wallet_account_digest_v1(&header.account)? != body.account_digest {
            return Err(invalid("suffix_unload.account"));
        }
        require_exact_certificates(
            &self.certificates,
            &header.scheme_id,
            &self.required_certificates(header),
        )?;
        if self.after.commitment()? != header.statement.successor {
            return Err(invalid("suffix_unload.after"));
        }
        if kagemusha_wallet_suffix_recovery_witness_digest_v1(&self.map_openings)?
            != header.recovery_witness_digest
        {
            return Err(invalid("suffix_unload.recovery_witness_digest"));
        }
        Ok(terms)
    }

    fn required_certificates(
        &self,
        header: &KagemushaWalletSuffixHeaderV1,
    ) -> Vec<([u8; 32], KagemushaWalletSignerRoleV1)> {
        let mut required = vec![(
            self.credential.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )];
        if let Some(quote) = header.charge.quote() {
            required.push((
                quote.body.signer_certificate,
                KagemushaWalletSignerRoleV1::RegulatoryPolicy,
            ));
        }
        required
    }

    /// [`Self::require_header`], then every signature under `scheme`: the certificate set, the
    /// credential's issuer signature, the charge quote's signature, and the hardware receipt τ
    /// over the header's statement with `authority` as its `proof_digest`. Proofs and openings
    /// are the native verifier's.
    ///
    /// # Errors
    /// Rejects what [`Self::require_header`] rejects, another scheme or relation, and any
    /// signature that does not verify.
    pub fn verify_terms(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        header: &KagemushaWalletSuffixHeaderV1,
        authority: &[u8; 32],
    ) -> SuffixResult<KagemushaWalletSuffixPayoutTermsV1> {
        let terms = self.require_header(header)?;
        header.statement.validate_for_scheme(scheme)?;
        let body = &self.credential.body;
        self.certificates.verify(scheme)?;
        let issuer = self.certificates.certificate(
            &body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )?;
        self.credential.verify(scheme, issuer)?;
        if let Some(quote) = header.charge.quote() {
            let signer = self.certificates.certificate(
                &quote.body.signer_certificate,
                KagemushaWalletSignerRoleV1::RegulatoryPolicy,
            )?;
            quote.verify(scheme, signer)?;
        }
        let signer = KagemushaWalletReceiptSignerV1::from_credential(&self.credential)?;
        self.receipt.verify(&signer, &header.statement, authority)?;
        Ok(terms)
    }

    /// Validate and encode the bounded canonical frame.
    ///
    /// # Errors
    /// Rejects an invalid Unload or an oversized frame.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1)
    }

    /// Decode one canonical Finalize frame for `expected_scheme_id`.
    ///
    /// # Errors
    /// Rejects an oversized or noncanonical frame, another scheme and an invalid Unload.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> SuffixResult<Self> {
        let unload: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1)?;
        require_scheme(
            "suffix_unload.credential.scheme_id",
            &unload.credential.body.scheme_id,
            expected_scheme_id,
        )?;
        unload.validate()?;
        Ok(unload)
    }
}

// The header and chunk carry direct u128 fields; pin their 16-byte archived alignment so
// their frames keep the same eight padding bytes on 32-bit and 64-bit targets.
const _: () = {
    let header = norito::core::archived_payload_align::<KagemushaWalletSuffixHeaderV1>();
    let chunk = norito::core::archived_payload_align::<KagemushaWalletSuffixChunkV1>();
    assert!(header == 16 && chunk == 16);
};
