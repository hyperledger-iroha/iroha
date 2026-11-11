//! Additive suffix monetary recovery frames; old capsule and completion encodings are unchanged.
//! A decoded frame supplies exact DATA. Only the verified Native owner may select it for Advance.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KagemushaWalletMarkerStateV1, KagemushaWalletOutputDescriptorV1, KagemushaWalletReceiptSignerV1,
};

/// Maximum complete suffix recovery body.
pub const SUFFIX_RECOVERY_BODY_MAX_BYTES_V1: usize = 65_536;
/// Maximum exact suffix capsule or completion frame.
pub const SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1: usize = 69_632;

fn private_digest(role: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_WALLET_SUFFIX_DIGEST_PREFIX_V1);
    hash.update(role);
    hash.update([0]);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn tag(version: u16, discriminator: u16, expected: u16) -> SuffixResult<()> {
    require_version("suffix recovery version", version)?;
    if discriminator != expected {
        return Err(invalid("suffix recovery discriminator"));
    }
    Ok(())
}

/// Receipt-free exact suffix Unload body, frozen before the independent capsule opening.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[repr(align(16))]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixRecoveryBodyV1"
)]
pub struct SuffixRecoveryBodyV1 {
    /// Exactly one.
    pub version: u16,
    /// Exactly ten.
    pub discriminator: u16,
    /// Admitted scheme.
    pub scheme_id: [u8; 32],
    /// Wallet incarnation.
    pub wallet_id: [u8; 32],
    /// Identity derived from the exact Unload statement.
    pub operation_id: [u8; 32],
    /// Selected predecessor capsule.
    pub predecessor_capsule_digest: [u8; 32],
    /// Exact successor state, including independent native nonce.
    pub successor_state: KagemushaWalletStateV1,
    /// Exact Unload statement.
    pub statement: KagemushaWalletStatementV1,
    /// Exact installed Unload sigma.
    pub step_proof: KagemushaWalletStepProofV1,
    /// Exact immutable canonical suffix authority header.
    pub authority_header: Vec<u8>,
    /// P_bytes over that exact header.
    pub authority_digest: [u8; 32],
    /// Selected source-pins index root.
    pub pinned_sources_root: [u8; 32],
    /// Exact recovery-map low and empty-slot openings.
    pub map_openings: Vec<Vec<u8>>,
    /// Unchanged receipt-free output descriptor, bound to the suffix digest.
    pub output: KagemushaWalletOutputDescriptorV1,
}
impl SuffixRecoveryBodyV1 {
    /// Check the exact canonical body and all noncryptographic cross-object bindings.
    /// # Errors
    /// Incorrect identity, header, statement, proof, opening, output or state commitment.
    #[expect(
        clippy::suspicious_operation_groupings,
        reason = "The header's source capsule is bound to this body's predecessor capsule digest; the body has no same-named field."
    )]
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.version, self.discriminator, 10)?;
        for value in [
            self.scheme_id,
            self.wallet_id,
            self.operation_id,
            self.predecessor_capsule_digest,
            self.authority_digest,
            self.pinned_sources_root,
        ] {
            require_nonzero("suffix recovery identity", &value)?;
        }
        self.successor_state.validate()?;
        self.statement.validate()?;
        require_insert_openings("suffix recovery openings", &self.map_openings)?;
        let header = KagemushaWalletSuffixHeaderV1::decode_canonical(
            &self.authority_header,
            &self.scheme_id,
        )?;
        if header.wallet_id != self.wallet_id
            || header.source_capsule != self.predecessor_capsule_digest
            || header.statement != self.statement
            || header.sigma != self.step_proof
            || self.statement.operation_id(&self.wallet_id)? != self.operation_id
            || self.successor_state.commitment()? != self.statement.successor
            || self.successor_state.core.scheme_id != self.scheme_id
            || self.successor_state.core.wallet_id != self.wallet_id
            || kagemusha_wallet_suffix_authority_digest_v1(&self.authority_header)?
                != self.authority_digest
            || self.output
                != KagemushaWalletOutputDescriptorV1::for_transition(
                    &self.statement,
                    &self.authority_digest,
                    &[0; 32],
                )?
        {
            return Err(invalid("suffix recovery binding"));
        }
        // Shared model owner authenticates the exact selected recovery-witness original.
        if kagemusha_wallet_suffix_recovery_witness_digest_v1(&self.map_openings)?
            != header.recovery_witness_digest
        {
            return Err(invalid("suffix recovery witness digest"));
        }
        Ok(())
    }
    /// Encode the validated, bounded body.
    /// # Errors
    /// Invalid body or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_RECOVERY_BODY_MAX_BYTES_V1)
    }
    /// Decode one exact canonical body under its admitted scheme.
    /// # Errors
    /// Wrong schema/scheme, invalid body, or resource refusal.
    pub fn decode_canonical(bytes: &[u8], scheme: &[u8; 32]) -> SuffixResult<Self> {
        let body: Self = decode_frame_v1(bytes, SUFFIX_RECOVERY_BODY_MAX_BYTES_V1)?;
        require_scheme("suffix recovery scheme", &body.scheme_id, scheme)?;
        body.validate()?;
        Ok(body)
    }
    /// Freeze all body bytes before independently sampling the capsule opening.
    /// # Errors
    /// Invalid body or resource refusal.
    pub fn freeze(self) -> SuffixResult<SuffixFrozenRecoveryBodyV1> {
        let frame = self.to_canonical_bytes()?;
        let digest = private_digest(b"capsule-body", &frame);
        Ok(SuffixFrozenRecoveryBodyV1 {
            body: self,
            frame,
            digest,
        })
    }
}

/// Immutable receipt-free body awaiting exactly one independently sampled opening.
pub struct SuffixFrozenRecoveryBodyV1 {
    body: SuffixRecoveryBodyV1,
    frame: Vec<u8>,
    digest: [u8; 32],
}
impl SuffixFrozenRecoveryBodyV1 {
    /// Seal this exact body using the unchanged randomized capsule commitment primitive.
    /// # Errors
    /// Noncanonical opening, invalid compiled generators, zero identifier, or frame bound.
    pub fn seal(self, opening: [u8; 32]) -> SuffixResult<SuffixRecoveryCapsuleV1> {
        let opening = zeroize::Zeroizing::new(opening);
        let commitment =
            crate::kagemusha::kagemusha_wallet_v1::kagemusha_wallet_capsule_commitment_for_body_v1(
                &self.digest,
                &opening,
            )?;
        let wire = SuffixRecoveryCapsuleFrameV1 {
            version: 1,
            discriminator: 11,
            body_frame: self.frame,
            opening: *opening,
        };
        let frame = encode_frame_v1(&wire, SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1)?;
        let custody_digest = private_digest(b"capsule", &frame);
        Ok(SuffixRecoveryCapsuleV1 {
            body: Box::new(self.body),
            wire,
            frame,
            custody_digest,
            commitment,
        })
    }
}
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixRecoveryCapsuleFrameV1"
)]
struct SuffixRecoveryCapsuleFrameV1 {
    version: u16,
    discriminator: u16,
    body_frame: Vec<u8>,
    opening: [u8; 32],
}
impl Drop for SuffixRecoveryCapsuleFrameV1 {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.body_frame);
        zeroize::Zeroize::zeroize(&mut self.opening);
    }
}
/// Validated immutable suffix capsule with separate private custody and public receipt identities.
#[derive(Clone, PartialEq, Eq)]
pub struct SuffixRecoveryCapsuleV1 {
    body: Box<SuffixRecoveryBodyV1>,
    wire: SuffixRecoveryCapsuleFrameV1,
    frame: Vec<u8>,
    custody_digest: [u8; 32],
    commitment: [u8; 32],
}
impl core::fmt::Debug for SuffixRecoveryCapsuleV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SuffixRecoveryCapsuleV1")
            .finish_non_exhaustive()
    }
}
impl Drop for SuffixRecoveryCapsuleV1 {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.frame);
    }
}
impl SuffixRecoveryCapsuleV1 {
    /// Borrow the immutable validated recovery body.
    #[must_use]
    pub fn body(&self) -> &SuffixRecoveryBodyV1 {
        &self.body
    }
    /// Exact selected private frame; never re-encode a retry.
    #[must_use]
    pub fn as_canonical_bytes(&self) -> &[u8] {
        &self.frame
    }
    /// Copy the exact selected private frame.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.frame.clone()
    }
    /// Private custody identity of the exact frame.
    #[must_use]
    pub const fn custody_digest(&self) -> [u8; 32] {
        self.custody_digest
    }
    /// Randomized public identifier bound by the unchanged receipt transcript.
    #[must_use]
    pub const fn receipt_commitment(&self) -> [u8; 32] {
        self.commitment
    }
    /// Fixed head state selected by Advance.
    #[must_use]
    pub fn head_marker_state(&self) -> KagemushaWalletMarkerStateV1 {
        KagemushaWalletMarkerStateV1::Head {
            sequence: self.body.statement.sequence,
            operation_id: self.body.operation_id,
            head: self.body.statement.successor,
            capsule_digest: self.custody_digest,
            predecessor_capsule_digest: self.body.predecessor_capsule_digest,
        }
    }
    /// Exact public schema used for declared-schema custody dispatch.
    #[must_use]
    pub fn frame_schema() -> [u8; 16] {
        norito::schema::identity::frame_hash::<SuffixRecoveryCapsuleFrameV1>()
    }
    /// Restore this schema only; failure never falls back to ordinary capsule decoding.
    /// # Errors
    /// Wrong schema, body/original mismatch, invalid opening or frame bound.
    pub fn decode_canonical(bytes: &[u8], scheme: &[u8; 32]) -> SuffixResult<Self> {
        let wire: SuffixRecoveryCapsuleFrameV1 =
            decode_frame_v1(bytes, SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1)?;
        tag(wire.version, wire.discriminator, 11)?;
        let body = SuffixRecoveryBodyV1::decode_canonical(&wire.body_frame, scheme)?;
        let capsule = body.freeze()?.seal(wire.opening)?;
        if capsule.as_canonical_bytes() != bytes {
            return Err(invalid("suffix capsule original"));
        }
        Ok(capsule)
    }
    /// Private frame digest used by the unchanged generic provider envelope.
    #[must_use]
    pub fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        private_digest(b"capsule", bytes)
    }
}

/// Exact suffix completion, independently selected by the provider's Released marker.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SuffixCompletionRecordV1"
)]
pub struct SuffixCompletionRecordV1 {
    /// Exactly one.
    pub version: u16,
    /// Exactly twelve.
    pub discriminator: u16,
    /// Admitted wallet incarnation.
    pub wallet_id: [u8; 32],
    /// Exact derived operation identity.
    pub operation_id: [u8; 32],
    /// Private capsule custody identity.
    pub capsule_digest: [u8; 32],
    /// Original hardware receipt, never resigned after release.
    pub receipt: KagemushaWalletReceiptV1,
    /// Exact canonical Finalize semantic frame; receipt and openings are retained here.
    pub output: Vec<u8>,
}
impl SuffixCompletionRecordV1 {
    /// Validate canonical shape without inferring a completed monetary operation.
    /// # Errors
    /// Wrong identity, tag, output bound or receipt shape.
    pub fn validate(&self) -> SuffixResult<()> {
        tag(self.version, self.discriminator, 12)?;
        for value in [self.wallet_id, self.operation_id, self.capsule_digest] {
            require_nonzero("suffix completion identity", &value)?;
        }
        self.receipt.validate()?;
        if self.receipt.operation_id != self.operation_id
            || self.output.is_empty()
            || self.output.len() > KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1
        {
            return Err(invalid("suffix completion output"));
        }
        Ok(())
    }
    /// Verify receipt and every output binding against the immutable capsule and credential.
    /// # Errors
    /// Any changed receipt, source, scheme, output, openings, state or signature.
    pub fn verify(
        &self,
        credential: &KagemushaWalletCredentialV1,
        capsule: &SuffixRecoveryCapsuleV1,
    ) -> SuffixResult<()> {
        self.validate()?;
        let body = capsule.body();
        if self.wallet_id != body.wallet_id
            || self.operation_id != body.operation_id
            || self.capsule_digest != capsule.custody_digest()
            || self.receipt.capsule_commitment != capsule.receipt_commitment()
            || self.receipt.payment_digest != [0; 32]
        {
            return Err(invalid("suffix completion capsule binding"));
        }
        let header = KagemushaWalletSuffixHeaderV1::decode_canonical(
            &body.authority_header,
            &body.scheme_id,
        )?;
        let output =
            KagemushaWalletSuffixUnloadV1::decode_canonical(&self.output, &body.scheme_id)?;
        output.require_header(&header)?;
        if output.credential != *credential
            || output.receipt != self.receipt
            || output.after != body.successor_state
            || output.map_openings != body.map_openings
        {
            return Err(invalid("suffix completion exact output"));
        }
        let signer = KagemushaWalletReceiptSignerV1::from_credential(credential)?;
        self.receipt
            .verify(&signer, &body.statement, &body.authority_digest)?;
        Ok(())
    }
    /// Encode this complete bounded canonical record.
    /// # Errors
    /// Invalid record or resource refusal.
    pub fn to_canonical_bytes(&self) -> SuffixResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1)
    }
    /// Decode an exact canonical completion under the expected wallet.
    /// # Errors
    /// Wrong wallet, malformed record, schema mismatch or resource refusal.
    pub fn decode_canonical(bytes: &[u8], wallet: &[u8; 32]) -> SuffixResult<Self> {
        let value: Self = decode_frame_v1(bytes, SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1)?;
        if value.wallet_id != *wallet {
            return Err(invalid("suffix completion wallet"));
        }
        value.validate()?;
        Ok(value)
    }
    /// Private completion digest used by the generic provider's Released marker.
    #[must_use]
    pub fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        private_digest(b"completion", bytes)
    }
}
