//! Canonical PQ-MASP note relation.
//!
//! The transparent proof establishes note membership, stable nullifiers, ownership by the ML-DSA
//! key committed in the public statement, output commitments, and checked value conservation.
//! ML-DSA verification and the ML-KEM/XChaCha output codec are separate native proof-wire checks;
//! neither is represented by a caller-selectable backend tag.
use iroha_data_model::{
    asset::AssetDefinitionId,
    privacy::{
        PQ_MASP_MAX_INPUTS_V1, PQ_MASP_MAX_OUTPUTS_V1, PqMaspStarkStatementV1,
        PrivacyAuthorizationKeyDigestV1, PrivacyCommitmentV1, PrivacyNamespaceScopeV1,
        PrivacyNamespaceV1, PrivacyNoteEncryptionKeyDigestV1, PrivacyNullifierV1, PrivacyPoolIdV1,
        PrivacyPoolNamespaceV1, PrivacyProtocolIdV1, PrivacyRecipientIdV1, PrivacyRootV1,
    },
};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, fmt};
use thiserror::Error;
use zeroize::Zeroize;
/// Exact depth of the validator-owned PQ note tree.
pub const PQ_MASP_TREE_DEPTH_V1: usize = 32;
/// Maximum consumed notes in the sole compiled relation.
pub const PQ_MASP_INPUT_BOUND_V1: usize = PQ_MASP_MAX_INPUTS_V1 as usize;
/// Maximum created notes in the sole compiled relation.
pub const PQ_MASP_OUTPUT_BOUND_V1: usize = PQ_MASP_MAX_OUTPUTS_V1 as usize;
pub(super) const HASH_FRAME_DOMAIN_V1: &[u8] = b"iroha:privacy:pq-masp:hash-frame:v1";
pub(super) const NULLIFIER_KEY_DOMAIN_V1: &[u8] = b"iroha:privacy:pq-masp:nullifier-key:v1";
pub(super) const NOTE_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:privacy:pq-masp:note-commitment:v1";
pub(super) const NOTE_NULLIFIER_DOMAIN_V1: &[u8] = b"iroha:privacy:pq-masp:nullifier:v1";
pub(super) const NOTE_ENCRYPTION_KEYS_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pq-masp:note-encryption-keys:v1";
pub(super) const ACCUMULATOR_LEAF_DOMAIN_V1: &[u8] =
    b"iroha.privacy.proof-managed-note-tree.leaf.v1";
pub(super) const ACCUMULATOR_NODE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.proof-managed-note-tree.node.v1";
/// Complete relation descriptor committed by the compiled engine manifest.
pub(crate) const PQ_MASP_ENGINE_DESCRIPTOR_V1: &[u8] = b"pq-masp-stark-v0:native-rust:first-release:inputs=1..2:outputs=1..2:values=u128-checked:membership=sha256-depth32-exact-ledger-domains:nullifier=stable-note-secret+rho+commitment+pool:ownership=statement-mldsa65-key-digest:authorization=outer-mldsa65-over-canonical-statement-digest+inner-proof-digest:producer=typed-redacted-witness+relation-and-key-preflight+rand0.9-trycrypto-fixed64-reservoir-zeroize-poison-error-or-unwind-policy-v1+block1-stark-replay+block2-independent-health-sha256-authorization-hedge+block3plus-stark+self-verify:encryption=mlkem768+xchacha20poly1305+internal-fixed64-health-sha256-seed:successor=validator-derived-only:legacy=unrepresentable";
/// Exact SHA-256 framing consumed by the native oracle and AIR.
pub(crate) const PQ_MASP_HASH_PROFILE_DESCRIPTOR_V1: &[u8] = b"sha256:frame-domain-len-u16be-field-count-u16be-field-len-u64be:nullifier-key+note-commitment+stable-nullifier+ordered-recipient-and-encapsulation-digest:proof-managed-leaf-and-level-node-exact-v1";
/// Plaintext committed by one PQ-MASP note.
#[derive(Clone, PartialEq, Eq)]
pub struct PqMaspNotePlaintextV1 {
    /// Atomic value.
    pub(crate) value: u128,
    /// ML-DSA-65 public-key digest authorized to spend this note.
    pub(crate) authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    /// ML-KEM-768 recipient key digest used for wallet note discovery.
    pub(crate) recipient_key_digest: PrivacyRecipientIdV1,
    /// Digest of the secret used to derive the stable nullifier.
    pub(crate) nullifier_key_digest: [u8; 32],
    /// Unique note nonce.
    pub(crate) rho: [u8; 32],
    /// Commitment blinding.
    pub(crate) blinding: [u8; 32],
    /// Wallet-defined payload digest.
    pub(crate) memo_digest: [u8; 32],
}
impl PqMaspNotePlaintextV1 {
    /// Construct one canonical nonzero PQ-MASP note plaintext.
    ///
    /// `memo_digest` may be zero because an empty wallet memo is valid.
    ///
    /// # Errors
    ///
    /// Rejects zero value, key digests, nonce, or blinding.
    pub fn new(
        value: u128,
        authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
        recipient_key_digest: PrivacyRecipientIdV1,
        nullifier_key_digest: [u8; 32],
        rho: [u8; 32],
        blinding: [u8; 32],
        memo_digest: [u8; 32],
    ) -> Result<Self, PqMaspRelationErrorV1> {
        let note = Self {
            value,
            authorization_key_digest,
            recipient_key_digest,
            nullifier_key_digest,
            rho,
            blinding,
            memo_digest,
        };
        validate_note_v1(&note)?;
        Ok(note)
    }
    /// Return the atomic value.
    #[must_use]
    pub const fn value(&self) -> u128 {
        self.value
    }
    /// Return the committed ML-DSA authorization-key digest.
    #[must_use]
    pub const fn authorization_key_digest(&self) -> PrivacyAuthorizationKeyDigestV1 {
        self.authorization_key_digest
    }
    /// Return the committed ML-KEM recipient identifier.
    #[must_use]
    pub const fn recipient_key_digest(&self) -> PrivacyRecipientIdV1 {
        self.recipient_key_digest
    }
    /// Return the digest opening expected from the nullifier secret.
    #[must_use]
    pub const fn nullifier_key_digest(&self) -> &[u8; 32] {
        &self.nullifier_key_digest
    }
    /// Return the unique note nonce.
    #[must_use]
    pub const fn rho(&self) -> &[u8; 32] {
        &self.rho
    }
    /// Return the commitment blinding.
    #[must_use]
    pub const fn blinding(&self) -> &[u8; 32] {
        &self.blinding
    }
    /// Return the wallet-defined memo digest.
    #[must_use]
    pub const fn memo_digest(&self) -> &[u8; 32] {
        &self.memo_digest
    }
}
impl fmt::Debug for PqMaspNotePlaintextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PqMaspNotePlaintextV1(<redacted>)")
    }
}
impl Drop for PqMaspNotePlaintextV1 {
    fn drop(&mut self) {
        self.value = 0;
        self.authorization_key_digest = PrivacyAuthorizationKeyDigestV1::new([0; 32]);
        self.recipient_key_digest = PrivacyRecipientIdV1::new([0; 32]);
        self.nullifier_key_digest.zeroize();
        self.rho.zeroize();
        self.blinding.zeroize();
        self.memo_digest.zeroize();
    }
}
/// Wallet-local consumed-note witness.
#[derive(Clone, PartialEq, Eq)]
pub struct PqMaspInputWitnessV1 {
    /// Committed note plaintext.
    pub(crate) note: PqMaspNotePlaintextV1,
    /// Secret preimage of `note.nullifier_key_digest`.
    pub(crate) nullifier_secret: [u8; 32],
    /// Zero-based position in the append-only tree.
    pub(crate) leaf_position: u32,
    /// Exact depth-32 sibling path.
    pub(crate) authentication_path: [[u8; 32]; PQ_MASP_TREE_DEPTH_V1],
}
impl PqMaspInputWitnessV1 {
    /// Construct one typed consumed-note witness.
    ///
    /// # Errors
    ///
    /// Rejects malformed note material, a zero or mismatched nullifier secret,
    /// and reserved-zero authentication siblings.
    pub fn new(
        note: PqMaspNotePlaintextV1,
        nullifier_secret: [u8; 32],
        leaf_position: u32,
        authentication_path: [[u8; 32]; PQ_MASP_TREE_DEPTH_V1],
    ) -> Result<Self, PqMaspRelationErrorV1> {
        validate_note_v1(&note)?;
        if is_zero(&nullifier_secret) || authentication_path.iter().any(|sibling| is_zero(sibling))
        {
            return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
        }
        if derive_pq_masp_nullifier_key_digest_v1(&nullifier_secret)? != note.nullifier_key_digest {
            return Err(PqMaspRelationErrorV1::NullifierKeyMismatch);
        }
        Ok(Self {
            note,
            nullifier_secret,
            leaf_position,
            authentication_path,
        })
    }
    /// Borrow the committed plaintext.
    #[must_use]
    pub const fn note(&self) -> &PqMaspNotePlaintextV1 {
        &self.note
    }
    /// Return the zero-based leaf position.
    #[must_use]
    pub const fn leaf_position(&self) -> u32 {
        self.leaf_position
    }
    /// Borrow the exact depth-32 authentication path.
    #[must_use]
    pub const fn authentication_path(&self) -> &[[u8; 32]; PQ_MASP_TREE_DEPTH_V1] {
        &self.authentication_path
    }
    /// Derive the public commitment opened by this input witness.
    pub fn commitment_v1(
        &self,
        statement: &PqMaspStarkStatementV1,
    ) -> Result<PrivacyCommitmentV1, PqMaspRelationErrorV1> {
        derive_pq_masp_note_commitment_v1(statement, &self.note)
    }
    /// Derive the stable public nullifier for this input and pool.
    ///
    /// The nullifier secret remains encapsulated by the redacted witness and
    /// is never returned to wallet adapters.
    pub fn nullifier_v1(
        &self,
        statement: &PqMaspStarkStatementV1,
    ) -> Result<PrivacyNullifierV1, PqMaspRelationErrorV1> {
        let commitment = self.commitment_v1(statement)?;
        derive_pq_masp_nullifier_v1(
            statement,
            &self.nullifier_secret,
            self.note.rho(),
            commitment,
        )
    }
}
impl fmt::Debug for PqMaspInputWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PqMaspInputWitnessV1(<redacted>)")
    }
}
impl Drop for PqMaspInputWitnessV1 {
    fn drop(&mut self) {
        self.nullifier_secret.zeroize();
        self.authentication_path.zeroize();
    }
}
/// Wallet-local created-note witness.
#[derive(Clone, PartialEq, Eq)]
pub struct PqMaspOutputWitnessV1 {
    /// Committed note plaintext.
    pub(crate) note: PqMaspNotePlaintextV1,
}
impl PqMaspOutputWitnessV1 {
    /// Construct one typed created-note witness.
    ///
    /// # Errors
    ///
    /// Rejects malformed note material.
    pub fn new(note: PqMaspNotePlaintextV1) -> Result<Self, PqMaspRelationErrorV1> {
        validate_note_v1(&note)?;
        Ok(Self { note })
    }
    /// Borrow the committed plaintext.
    #[must_use]
    pub const fn note(&self) -> &PqMaspNotePlaintextV1 {
        &self.note
    }
}
impl fmt::Debug for PqMaspOutputWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PqMaspOutputWitnessV1(<redacted>)")
    }
}
/// Complete bounded PQ-MASP witness.
#[derive(Clone, PartialEq, Eq)]
pub struct PqMaspWitnessV1 {
    /// Consumed notes in public-nullifier order.
    pub(crate) inputs: Vec<PqMaspInputWitnessV1>,
    /// Created notes in public-commitment order.
    pub(crate) outputs: Vec<PqMaspOutputWitnessV1>,
}
impl PqMaspWitnessV1 {
    /// Construct one exact first-release PQ-MASP witness.
    ///
    /// # Errors
    ///
    /// Rejects input/output cardinality outside the closed one-to-two bounds
    /// before any proof allocation.
    pub fn new(
        inputs: Vec<PqMaspInputWitnessV1>,
        outputs: Vec<PqMaspOutputWitnessV1>,
    ) -> Result<Self, PqMaspRelationErrorV1> {
        if inputs.is_empty()
            || inputs.len() > PQ_MASP_INPUT_BOUND_V1
            || outputs.is_empty()
            || outputs.len() > PQ_MASP_OUTPUT_BOUND_V1
        {
            return Err(PqMaspRelationErrorV1::WitnessShape);
        }
        Ok(Self { inputs, outputs })
    }
    /// Borrow consumed notes in public-nullifier order.
    #[must_use]
    pub fn inputs(&self) -> &[PqMaspInputWitnessV1] {
        &self.inputs
    }
    /// Borrow created notes in public-commitment order.
    #[must_use]
    pub fn outputs(&self) -> &[PqMaspOutputWitnessV1] {
        &self.outputs
    }
}
impl fmt::Debug for PqMaspWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PqMaspWitnessV1")
            .field("input_count", &self.inputs.len())
            .field("output_count", &self.outputs.len())
            .finish_non_exhaustive()
    }
}
/// Semantic role of one SHA-256 invocation in the compiled STARK schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PqMaspSha256RoleV1 {
    NullifierKey { input: u8 },
    InputCommitment { input: u8 },
    Nullifier { input: u8 },
    AccumulatorLeaf { input: u8 },
    AccumulatorNode { input: u8, level: u8 },
    OutputCommitment { output: u8 },
    EncryptionKeySet,
}
/// One exact SHA-256 invocation consumed by the STARK witness compiler.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct PqMaspSha256InvocationV1 {
    pub(super) role: PqMaspSha256RoleV1,
    pub(super) preimage: Vec<u8>,
    pub(super) digest: [u8; 32],
}
impl fmt::Debug for PqMaspSha256InvocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PqMaspSha256InvocationV1")
            .field("role", &self.role)
            .field("preimage", &"<redacted>")
            .finish_non_exhaustive()
    }
}
impl Drop for PqMaspSha256InvocationV1 {
    fn drop(&mut self) {
        self.preimage.zeroize();
        self.digest.zeroize();
    }
}
/// Fully checked relation material retained only by the prover.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ValidatedPqMaspRelationV1 {
    pub(super) invocations: Vec<PqMaspSha256InvocationV1>,
    pub(super) input_sum: u128,
    pub(super) output_sum: u128,
}
impl fmt::Debug for ValidatedPqMaspRelationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedPqMaspRelationV1")
            .field("invocation_count", &self.invocations.len())
            .field("private_values", &"<redacted>")
            .finish_non_exhaustive()
    }
}
/// Native relation or witness failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PqMaspRelationErrorV1 {
    /// The public statement is not the sole canonical first-release shape.
    #[error("PQ-MASP public statement is invalid")]
    InvalidStatement,
    /// Witness cardinality differs from the public statement.
    #[error("PQ-MASP witness shape does not match the statement")]
    WitnessShape,
    /// A required witness component is the reserved zero value.
    #[error("PQ-MASP witness contains a reserved zero value")]
    ZeroWitnessComponent,
    /// The input note does not commit the statement-authorized ML-DSA key.
    #[error("PQ-MASP input authorization key does not match the statement")]
    AuthorizationKeyMismatch,
    /// The nullifier secret does not open its committed digest.
    #[error("PQ-MASP nullifier key opening is invalid")]
    NullifierKeyMismatch,
    /// A note commitment differs from the public ordered commitment.
    #[error("PQ-MASP note commitment relation is invalid")]
    CommitmentMismatch,
    /// A public nullifier differs from its stable note nullifier.
    #[error("PQ-MASP nullifier relation is invalid")]
    NullifierMismatch,
    /// An input authentication path does not reach the admitted anchor.
    #[error("PQ-MASP accumulator membership is invalid")]
    Membership,
    /// An output recipient differs from its aligned encrypted-output recipient.
    #[error("PQ-MASP encrypted-output recipient binding is invalid")]
    RecipientMismatch,
    /// Duplicate private spend or output material was supplied.
    #[error("PQ-MASP witness contains duplicate spend or output material")]
    Duplicate,
    /// Checked value arithmetic overflowed.
    #[error("PQ-MASP value arithmetic overflow")]
    ValueOverflow,
    /// Hidden input and output values are not conserved.
    #[error("PQ-MASP values are not conserved")]
    ValueConservation,
    /// Canonical Norito/hash framing failed.
    #[error("PQ-MASP canonical encoding failed")]
    Encoding,
    /// A bounded allocation failed.
    #[error("PQ-MASP bounded allocation failed")]
    AllocationFailure,
}
fn is_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}
fn frame_preimage_v1(domain: &[u8], fields: &[&[u8]]) -> Result<Vec<u8>, PqMaspRelationErrorV1> {
    let domain_len = u16::try_from(domain.len()).map_err(|_| PqMaspRelationErrorV1::Encoding)?;
    let field_count = u16::try_from(fields.len()).map_err(|_| PqMaspRelationErrorV1::Encoding)?;
    let capacity = HASH_FRAME_DOMAIN_V1
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_add(domain.len()))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| {
            fields.iter().try_fold(value, |length, field| {
                length.checked_add(8)?.checked_add(field.len())
            })
        })
        .ok_or(PqMaspRelationErrorV1::Encoding)?;
    let mut preimage = Vec::new();
    preimage
        .try_reserve_exact(capacity)
        .map_err(|_| PqMaspRelationErrorV1::AllocationFailure)?;
    preimage.extend_from_slice(HASH_FRAME_DOMAIN_V1);
    preimage.extend_from_slice(&domain_len.to_be_bytes());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(&field_count.to_be_bytes());
    for field in fields {
        preimage.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| PqMaspRelationErrorV1::Encoding)?
                .to_be_bytes(),
        );
        preimage.extend_from_slice(field);
    }
    Ok(preimage)
}
fn sha256_invocation_v1(
    role: PqMaspSha256RoleV1,
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    let preimage = frame_preimage_v1(domain, fields)?;
    let digest = Sha256::digest(&preimage).into();
    Ok(PqMaspSha256InvocationV1 {
        role,
        preimage,
        digest,
    })
}
fn note_commitment_invocation_v1(
    role: PqMaspSha256RoleV1,
    statement: &PqMaspStarkStatementV1,
    note: &PqMaspNotePlaintextV1,
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    note_commitment_invocation_for_pool_v1(
        role,
        &statement.asset_definition_id,
        statement.pool_id,
        note,
    )
}
fn note_commitment_invocation_for_pool_v1(
    role: PqMaspSha256RoleV1,
    asset_definition_id: &AssetDefinitionId,
    pool_id: PrivacyPoolIdV1,
    note: &PqMaspNotePlaintextV1,
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    validate_note_v1(note)?;
    let asset =
        norito::to_bytes(asset_definition_id).map_err(|_| PqMaspRelationErrorV1::Encoding)?;
    sha256_invocation_v1(
        role,
        NOTE_COMMITMENT_DOMAIN_V1,
        &[
            &asset,
            pool_id.as_bytes(),
            &note.value.to_be_bytes(),
            note.authorization_key_digest.as_bytes(),
            note.recipient_key_digest.as_bytes(),
            &note.nullifier_key_digest,
            &note.rho,
            &note.blinding,
            &note.memo_digest,
        ],
    )
}
fn wallet_namespace_v1(pool_id: PrivacyPoolIdV1) -> PrivacyNamespaceV1 {
    PrivacyNamespaceV1::new(
        PrivacyProtocolIdV1::PqMaspStarkV0,
        PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 { pool_id }),
    )
}
pub(super) fn namespace_v1(statement: &PqMaspStarkStatementV1) -> PrivacyNamespaceV1 {
    wallet_namespace_v1(statement.pool_id)
}
fn validate_note_v1(note: &PqMaspNotePlaintextV1) -> Result<(), PqMaspRelationErrorV1> {
    if note.value == 0
        || note.authorization_key_digest.is_zero()
        || note.recipient_key_digest.is_zero()
        || is_zero(&note.nullifier_key_digest)
        || is_zero(&note.rho)
        || is_zero(&note.blinding)
    {
        return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
    }
    Ok(())
}
/// Derive the digest committed by a note for its nullifier secret.
pub fn derive_pq_masp_nullifier_key_digest_v1(
    nullifier_secret: &[u8; 32],
) -> Result<[u8; 32], PqMaspRelationErrorV1> {
    if is_zero(nullifier_secret) {
        return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
    }
    Ok(sha256_invocation_v1(
        PqMaspSha256RoleV1::NullifierKey { input: 0 },
        NULLIFIER_KEY_DOMAIN_V1,
        &[nullifier_secret],
    )?
    .digest)
}
/// Derive the sole canonical PQ note commitment.
pub fn derive_pq_masp_note_commitment_v1(
    statement: &PqMaspStarkStatementV1,
    note: &PqMaspNotePlaintextV1,
) -> Result<PrivacyCommitmentV1, PqMaspRelationErrorV1> {
    Ok(PrivacyCommitmentV1::new(
        note_commitment_invocation_v1(
            PqMaspSha256RoleV1::OutputCommitment { output: 0 },
            statement,
            note,
        )?
        .digest,
    ))
}
/// Derive a stable nullifier for a committed note.
///
/// Transaction, action, anchor, and epoch data are intentionally absent. Including any of them
/// would let the same note derive a fresh nullifier for a second spend.
pub fn derive_pq_masp_nullifier_v1(
    statement: &PqMaspStarkStatementV1,
    nullifier_secret: &[u8; 32],
    rho: &[u8; 32],
    commitment: PrivacyCommitmentV1,
) -> Result<PrivacyNullifierV1, PqMaspRelationErrorV1> {
    if is_zero(nullifier_secret) || is_zero(rho) || commitment.is_zero() {
        return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
    }
    Ok(PrivacyNullifierV1::new(
        sha256_invocation_v1(
            PqMaspSha256RoleV1::Nullifier { input: 0 },
            NOTE_NULLIFIER_DOMAIN_V1,
            &[
                nullifier_secret,
                rho,
                commitment.as_bytes(),
                statement.pool_id.as_bytes(),
            ],
        )?
        .digest,
    ))
}
/// Commit the exact ordered ML-KEM recipient-key digests.
pub fn derive_pq_masp_note_encryption_keys_digest_v1(
    statement: &PqMaspStarkStatementV1,
) -> Result<PrivacyNoteEncryptionKeyDigestV1, PqMaspRelationErrorV1> {
    if statement.encrypted_outputs.is_empty() {
        return Err(PqMaspRelationErrorV1::InvalidStatement);
    }
    let invocation = note_encryption_keys_invocation_v1(statement)?;
    Ok(PrivacyNoteEncryptionKeyDigestV1::new(invocation.digest))
}
fn note_encryption_keys_invocation_v1(
    statement: &PqMaspStarkStatementV1,
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    let field_count = statement
        .encrypted_outputs
        .len()
        .checked_mul(2)
        .ok_or(PqMaspRelationErrorV1::Encoding)?;
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| PqMaspRelationErrorV1::AllocationFailure)?;
    for output in &statement.encrypted_outputs {
        fields.push(output.recipient.as_bytes().as_slice());
        fields.push(output.ephemeral_public_key.as_bytes().as_slice());
    }
    sha256_invocation_v1(
        PqMaspSha256RoleV1::EncryptionKeySet,
        NOTE_ENCRYPTION_KEYS_DOMAIN_V1,
        &fields,
    )
}
pub(super) fn accumulator_leaf_invocation_v1(
    statement: &PqMaspStarkStatementV1,
    input: u8,
    commitment: PrivacyCommitmentV1,
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    accumulator_leaf_invocation_for_pool_v1(statement.pool_id, input, commitment)
}
fn accumulator_leaf_invocation_for_pool_v1(
    pool_id: PrivacyPoolIdV1,
    input: u8,
    commitment: PrivacyCommitmentV1,
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    let namespace = norito::to_bytes(&wallet_namespace_v1(pool_id))
        .map_err(|_| PqMaspRelationErrorV1::Encoding)?;
    let mut preimage = Vec::new();
    preimage
        .try_reserve_exact(ACCUMULATOR_LEAF_DOMAIN_V1.len() + 8 + namespace.len() + 32)
        .map_err(|_| PqMaspRelationErrorV1::AllocationFailure)?;
    preimage.extend_from_slice(ACCUMULATOR_LEAF_DOMAIN_V1);
    preimage.extend_from_slice(
        &u64::try_from(namespace.len())
            .map_err(|_| PqMaspRelationErrorV1::Encoding)?
            .to_be_bytes(),
    );
    preimage.extend_from_slice(&namespace);
    preimage.extend_from_slice(commitment.as_bytes());
    Ok(PqMaspSha256InvocationV1 {
        role: PqMaspSha256RoleV1::AccumulatorLeaf { input },
        digest: Sha256::digest(&preimage).into(),
        preimage,
    })
}
pub(super) fn accumulator_node_invocation_v1(
    input: u8,
    level: u8,
    left: &[u8; 32],
    right: &[u8; 32],
) -> Result<PqMaspSha256InvocationV1, PqMaspRelationErrorV1> {
    let mut preimage = Vec::new();
    preimage
        .try_reserve_exact(ACCUMULATOR_NODE_DOMAIN_V1.len() + 1 + 64)
        .map_err(|_| PqMaspRelationErrorV1::AllocationFailure)?;
    preimage.extend_from_slice(ACCUMULATOR_NODE_DOMAIN_V1);
    preimage.push(level);
    preimage.extend_from_slice(left);
    preimage.extend_from_slice(right);
    Ok(PqMaspSha256InvocationV1 {
        role: PqMaspSha256RoleV1::AccumulatorNode { input, level },
        digest: Sha256::digest(&preimage).into(),
        preimage,
    })
}

/// Preflight every relation constraint determined by an owner-only PQ-MASP
/// wallet bundle before the worker advertises that bundle as ready.
///
/// ML-DSA authorization and encrypted-output construction still happen in the
/// native action builder, but their bundle-selected key digests are checked by
/// the caller before this relation preflight is invoked.
pub fn preflight_pq_masp_wallet_request_v1(
    asset_definition_id: &AssetDefinitionId,
    pool_id: PrivacyPoolIdV1,
    anchor: PrivacyRootV1,
    authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    inputs: &[PqMaspInputWitnessV1],
    outputs: &[&PqMaspOutputWitnessV1],
) -> Result<(), PqMaspRelationErrorV1> {
    if pool_id.is_zero() || anchor.is_zero() || authorization_key_digest.is_zero() {
        return Err(PqMaspRelationErrorV1::InvalidStatement);
    }
    if inputs.is_empty()
        || inputs.len() > PQ_MASP_INPUT_BOUND_V1
        || outputs.is_empty()
        || outputs.len() > PQ_MASP_OUTPUT_BOUND_V1
    {
        return Err(PqMaspRelationErrorV1::WitnessShape);
    }
    let mut commitments = BTreeSet::new();
    let mut nullifier_secrets = BTreeSet::new();
    let mut positions = BTreeSet::new();
    let mut input_sum = 0_u128;
    for (index, input) in inputs.iter().enumerate() {
        validate_note_v1(&input.note)?;
        if input.note.authorization_key_digest != authorization_key_digest {
            return Err(PqMaspRelationErrorV1::AuthorizationKeyMismatch);
        }
        if derive_pq_masp_nullifier_key_digest_v1(&input.nullifier_secret)?
            != input.note.nullifier_key_digest
        {
            return Err(PqMaspRelationErrorV1::NullifierKeyMismatch);
        }
        if !nullifier_secrets.insert(input.nullifier_secret)
            || !positions.insert(input.leaf_position)
        {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        let input_u8 = u8::try_from(index).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
        let commitment = PrivacyCommitmentV1::new(
            note_commitment_invocation_for_pool_v1(
                PqMaspSha256RoleV1::InputCommitment { input: input_u8 },
                asset_definition_id,
                pool_id,
                &input.note,
            )?
            .digest,
        );
        if !commitments.insert(commitment) {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        let mut current =
            accumulator_leaf_invocation_for_pool_v1(pool_id, input_u8, commitment)?.digest;
        for (level, sibling) in input.authentication_path.iter().enumerate() {
            if is_zero(sibling) {
                return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
            }
            let level_u8 = u8::try_from(level).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
            current = if input.leaf_position & (1_u32 << level_u8) == 0 {
                accumulator_node_invocation_v1(input_u8, level_u8, &current, sibling)?
            } else {
                accumulator_node_invocation_v1(input_u8, level_u8, sibling, &current)?
            }
            .digest;
        }
        if PrivacyRootV1::new(current) != anchor {
            return Err(PqMaspRelationErrorV1::Membership);
        }
        input_sum = input_sum
            .checked_add(input.note.value)
            .ok_or(PqMaspRelationErrorV1::ValueOverflow)?;
    }
    let mut output_sum = 0_u128;
    for (index, output) in outputs.iter().enumerate() {
        validate_note_v1(&output.note)?;
        let output_u8 = u8::try_from(index).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
        let commitment = PrivacyCommitmentV1::new(
            note_commitment_invocation_for_pool_v1(
                PqMaspSha256RoleV1::OutputCommitment { output: output_u8 },
                asset_definition_id,
                pool_id,
                &output.note,
            )?
            .digest,
        );
        if !commitments.insert(commitment) {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        output_sum = output_sum
            .checked_add(output.note.value)
            .ok_or(PqMaspRelationErrorV1::ValueOverflow)?;
    }
    if input_sum != output_sum {
        return Err(PqMaspRelationErrorV1::ValueConservation);
    }
    Ok(())
}

pub(super) fn validate_statement_v1(
    statement: &PqMaspStarkStatementV1,
) -> Result<(), PqMaspRelationErrorV1> {
    if statement.pool_id.is_zero()
        || statement.anchor.is_zero()
        || statement.anchor_epoch == 0
        || statement.authorization_epoch != statement.anchor_epoch
        || statement.authorization_key_digest.is_zero()
        || statement.note_encryption_key_digest.is_zero()
        || statement.nullifiers.is_empty()
        || statement.nullifiers.len() > PQ_MASP_INPUT_BOUND_V1
        || statement.output_commitments.is_empty()
        || statement.output_commitments.len() > PQ_MASP_OUTPUT_BOUND_V1
        || statement.encrypted_outputs.len() != statement.output_commitments.len()
        || derive_pq_masp_note_encryption_keys_digest_v1(statement)?
            != statement.note_encryption_key_digest
    {
        return Err(PqMaspRelationErrorV1::InvalidStatement);
    }
    let mut nullifiers = BTreeSet::new();
    if statement
        .nullifiers
        .iter()
        .any(|value| value.is_zero() || !nullifiers.insert(*value))
    {
        return Err(PqMaspRelationErrorV1::InvalidStatement);
    }
    let mut commitments = BTreeSet::new();
    if statement
        .output_commitments
        .iter()
        .any(|value| value.is_zero() || !commitments.insert(*value))
    {
        return Err(PqMaspRelationErrorV1::InvalidStatement);
    }
    for (encrypted, commitment) in statement
        .encrypted_outputs
        .iter()
        .zip(&statement.output_commitments)
    {
        if encrypted.commitment != *commitment
            || super::wire::validate_pq_masp_encrypted_output_v1(encrypted).is_err()
        {
            return Err(PqMaspRelationErrorV1::InvalidStatement);
        }
    }
    namespace_v1(statement)
        .validate()
        .map_err(|_| PqMaspRelationErrorV1::InvalidStatement)
}
fn checked_sum(mut values: impl Iterator<Item = u128>) -> Result<u128, PqMaspRelationErrorV1> {
    values.try_fold(0_u128, |sum, value| {
        sum.checked_add(value)
            .ok_or(PqMaspRelationErrorV1::ValueOverflow)
    })
}
/// Validate the complete native witness relation and compile its SHA schedule.
pub(crate) fn validate_pq_masp_relation_v1(
    statement: &PqMaspStarkStatementV1,
    witness: &PqMaspWitnessV1,
) -> Result<ValidatedPqMaspRelationV1, PqMaspRelationErrorV1> {
    validate_statement_v1(statement)?;
    if witness.inputs.len() != statement.nullifiers.len()
        || witness.outputs.len() != statement.output_commitments.len()
        || witness.inputs.is_empty()
        || witness.inputs.len() > PQ_MASP_INPUT_BOUND_V1
        || witness.outputs.is_empty()
        || witness.outputs.len() > PQ_MASP_OUTPUT_BOUND_V1
    {
        return Err(PqMaspRelationErrorV1::WitnessShape);
    }
    let mut invocations = Vec::new();
    invocations
        .try_reserve_exact(
            witness.inputs.len() * (4 + PQ_MASP_TREE_DEPTH_V1) + witness.outputs.len() + 1,
        )
        .map_err(|_| PqMaspRelationErrorV1::AllocationFailure)?;
    let mut spent_commitments = BTreeSet::new();
    let mut spent_nullifier_secrets = BTreeSet::new();
    for (index, (input_witness, expected_nullifier)) in
        witness.inputs.iter().zip(&statement.nullifiers).enumerate()
    {
        validate_note_v1(&input_witness.note)?;
        if is_zero(&input_witness.nullifier_secret) {
            return Err(PqMaspRelationErrorV1::ZeroWitnessComponent);
        }
        if input_witness.note.authorization_key_digest != statement.authorization_key_digest {
            return Err(PqMaspRelationErrorV1::AuthorizationKeyMismatch);
        }
        let input_index = u8::try_from(index).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
        let key_invocation = sha256_invocation_v1(
            PqMaspSha256RoleV1::NullifierKey { input: input_index },
            NULLIFIER_KEY_DOMAIN_V1,
            &[&input_witness.nullifier_secret],
        )?;
        if key_invocation.digest != input_witness.note.nullifier_key_digest {
            return Err(PqMaspRelationErrorV1::NullifierKeyMismatch);
        }
        if !spent_nullifier_secrets.insert(input_witness.nullifier_secret) {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        invocations.push(key_invocation);
        let commitment = derive_pq_masp_note_commitment_v1(statement, &input_witness.note)?;
        if !spent_commitments.insert(commitment) {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        invocations.push(note_commitment_invocation_v1(
            PqMaspSha256RoleV1::InputCommitment { input: input_index },
            statement,
            &input_witness.note,
        )?);
        let nullifier = derive_pq_masp_nullifier_v1(
            statement,
            &input_witness.nullifier_secret,
            &input_witness.note.rho,
            commitment,
        )?;
        if nullifier != *expected_nullifier {
            return Err(PqMaspRelationErrorV1::NullifierMismatch);
        }
        invocations.push(sha256_invocation_v1(
            PqMaspSha256RoleV1::Nullifier { input: input_index },
            NOTE_NULLIFIER_DOMAIN_V1,
            &[
                &input_witness.nullifier_secret,
                &input_witness.note.rho,
                commitment.as_bytes(),
                statement.pool_id.as_bytes(),
            ],
        )?);
        let leaf = accumulator_leaf_invocation_v1(statement, input_index, commitment)?;
        let mut current = leaf.digest;
        invocations.push(leaf);
        for (level, sibling) in input_witness.authentication_path.iter().enumerate() {
            let level = u8::try_from(level).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
            let (left, right) = if input_witness.leaf_position & (1_u32 << level) == 0 {
                (&current, sibling)
            } else {
                (sibling, &current)
            };
            let node = accumulator_node_invocation_v1(input_index, level, left, right)?;
            current = node.digest;
            invocations.push(node);
        }
        if PrivacyRootV1::new(current) != statement.anchor {
            return Err(PqMaspRelationErrorV1::Membership);
        }
    }
    let mut output_commitments = BTreeSet::new();
    for (index, ((output, expected_commitment), encrypted)) in witness
        .outputs
        .iter()
        .zip(&statement.output_commitments)
        .zip(&statement.encrypted_outputs)
        .enumerate()
    {
        validate_note_v1(&output.note)?;
        if output.note.recipient_key_digest != encrypted.recipient {
            return Err(PqMaspRelationErrorV1::RecipientMismatch);
        }
        let commitment = derive_pq_masp_note_commitment_v1(statement, &output.note)?;
        if commitment != *expected_commitment {
            return Err(PqMaspRelationErrorV1::CommitmentMismatch);
        }
        if !output_commitments.insert(commitment) || spent_commitments.contains(&commitment) {
            return Err(PqMaspRelationErrorV1::Duplicate);
        }
        let output = u8::try_from(index).map_err(|_| PqMaspRelationErrorV1::WitnessShape)?;
        invocations.push(note_commitment_invocation_v1(
            PqMaspSha256RoleV1::OutputCommitment { output },
            statement,
            &witness.outputs[index].note,
        )?);
    }
    invocations.push(note_encryption_keys_invocation_v1(statement)?);
    let input_sum = checked_sum(witness.inputs.iter().map(|input| input.note.value))?;
    let output_sum = checked_sum(witness.outputs.iter().map(|output| output.note.value))?;
    if input_sum != output_sum {
        return Err(PqMaspRelationErrorV1::ValueConservation);
    }
    Ok(ValidatedPqMaspRelationV1 {
        invocations,
        input_sum,
        output_sum,
    })
}
#[cfg(test)]
/// Canonical fixtures shared by the relation and extension-AIR suites.
pub(crate) mod tests {
    use super::*;
    use iroha_data_model::{
        asset::AssetDefinitionId,
        domain::DomainId,
        name::Name,
        privacy::{
            PqMaspStarkStatementV1, PrivacyAuthorizationKeyDigestV1, PrivacyCommitmentV1,
            PrivacyEncryptedOutputV1, PrivacyEngineManifestDigestV1,
            PrivacyNoteEncryptionKeyDigestV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
            PrivacyPoolIdV1, PrivacyPqAuthorizationProfileV1, PrivacyPqNoteEncryptionProfileV1,
            PrivacyRecipientIdV1, PrivacyRootV1, PrivacyStatementContextV1,
            PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
            PrivacyVerifierDigestV1,
        },
    };
    use std::str::FromStr as _;
    fn raw(byte: u8) -> [u8; 32] {
        [byte; 32]
    }
    fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xC2; 32]),
            )),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(raw(1)),
            parameter_id: PrivacyParameterIdV1::new(raw(2)),
            parameter_digest: PrivacyParameterDigestV1::new(raw(3)),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(4)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(5)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(6)),
        }
    }
    fn encrypted_output_shell(
        recipient: PrivacyRecipientIdV1,
        commitment: PrivacyCommitmentV1,
    ) -> PrivacyEncryptedOutputV1 {
        let mut ciphertext = vec![0xA5; super::super::wire::PQ_MASP_ENCRYPTED_OUTPUT_BYTES_V1];
        ciphertext[..4].copy_from_slice(super::super::wire::ENCRYPTED_OUTPUT_MAGIC_V1);
        let kem_end = 4 + super::super::wire::ML_KEM_768_CIPHERTEXT_BYTES_V1;
        let ephemeral_public_key =
            super::super::wire::derive_encapsulation_digest_v1(&ciphertext[4..kem_end])
                .expect("structurally valid ML-KEM ciphertext");
        PrivacyEncryptedOutputV1 {
            recipient,
            ephemeral_public_key,
            commitment,
            ciphertext,
        }
    }
    fn statement_shell() -> PqMaspStarkStatementV1 {
        let commitment = PrivacyCommitmentV1::new(raw(10));
        let recipient = PrivacyRecipientIdV1::new(raw(11));
        PqMaspStarkStatementV1 {
            context: context(),
            asset_definition_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("privacy", "universal").expect("domain"),
                Name::from_str("pq_note").expect("asset name"),
            ),
            pool_id: PrivacyPoolIdV1::new(raw(7)),
            anchor: PrivacyRootV1::new(raw(8)),
            anchor_epoch: 1,
            nullifiers: vec![PrivacyNullifierV1::new(raw(9))],
            output_commitments: vec![commitment],
            encrypted_outputs: vec![encrypted_output_shell(recipient, commitment)],
            authorization_profile: PrivacyPqAuthorizationProfileV1::MlDsa65,
            authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(13)),
            note_encryption_profile: PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305,
            note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1::new(raw(14)),
            authorization_epoch: 1,
        }
    }
    fn empty_authentication_path_v1() -> [[u8; 32]; PQ_MASP_TREE_DEPTH_V1] {
        const EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.privacy.proof-managed-note-tree.empty-leaf.v1";
        let mut path = [[0_u8; 32]; PQ_MASP_TREE_DEPTH_V1];
        let mut empty: [u8; 32] = Sha256::digest(EMPTY_LEAF_DOMAIN_V1).into();
        for (level, sibling) in path.iter_mut().enumerate() {
            *sibling = empty;
            let level = u8::try_from(level).expect("depth is 32");
            empty = accumulator_node_invocation_v1(0, level, &empty, &empty)
                .expect("empty node")
                .digest;
        }
        path
    }
    fn anchor_for_input_v1(
        statement: &PqMaspStarkStatementV1,
        commitment: PrivacyCommitmentV1,
        position: u32,
        path: &[[u8; 32]; PQ_MASP_TREE_DEPTH_V1],
    ) -> PrivacyRootV1 {
        let mut current = accumulator_leaf_invocation_v1(statement, 0, commitment)
            .expect("leaf")
            .digest;
        for (level, sibling) in path.iter().enumerate() {
            let level = u8::try_from(level).expect("depth is 32");
            let (left, right) = if position & (1_u32 << level) == 0 {
                (&current, sibling)
            } else {
                (sibling, &current)
            };
            current = accumulator_node_invocation_v1(0, level, left, right)
                .expect("node")
                .digest;
        }
        PrivacyRootV1::new(current)
    }
    /// Build the canonical one-input/one-output relation fixture.
    pub(crate) fn valid_fixture() -> (PqMaspStarkStatementV1, PqMaspWitnessV1) {
        let mut statement = statement_shell();
        let nullifier_secret = raw(15);
        let input_note = PqMaspNotePlaintextV1 {
            value: 70,
            authorization_key_digest: statement.authorization_key_digest,
            recipient_key_digest: PrivacyRecipientIdV1::new(raw(16)),
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&nullifier_secret)
                .expect("nullifier key digest"),
            rho: raw(17),
            blinding: raw(18),
            memo_digest: raw(19),
        };
        let output_note = PqMaspNotePlaintextV1 {
            value: 70,
            authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(20)),
            recipient_key_digest: statement.encrypted_outputs[0].recipient,
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&raw(21))
                .expect("output nullifier key digest"),
            rho: raw(22),
            blinding: raw(23),
            memo_digest: raw(24),
        };
        let input_commitment =
            derive_pq_masp_note_commitment_v1(&statement, &input_note).expect("input commitment");
        let authentication_path = empty_authentication_path_v1();
        statement.anchor =
            anchor_for_input_v1(&statement, input_commitment, 0, &authentication_path);
        statement.nullifiers[0] = derive_pq_masp_nullifier_v1(
            &statement,
            &nullifier_secret,
            &input_note.rho,
            input_commitment,
        )
        .expect("stable nullifier");
        statement.output_commitments[0] =
            derive_pq_masp_note_commitment_v1(&statement, &output_note).expect("output commitment");
        statement.encrypted_outputs[0].commitment = statement.output_commitments[0];
        statement.note_encryption_key_digest =
            derive_pq_masp_note_encryption_keys_digest_v1(&statement)
                .expect("ordered encryption keys digest");
        (
            statement,
            PqMaspWitnessV1 {
                inputs: vec![PqMaspInputWitnessV1 {
                    note: input_note,
                    nullifier_secret,
                    leaf_position: 0,
                    authentication_path,
                }],
                outputs: vec![PqMaspOutputWitnessV1 { note: output_note }],
            },
        )
    }
    /// Rebind the canonical one-input fixture to a real authorization key.
    pub(crate) fn valid_fixture_with_authorization_key_digest(
        authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    ) -> (PqMaspStarkStatementV1, PqMaspWitnessV1) {
        let (mut statement, mut witness) = valid_fixture();
        statement.authorization_key_digest = authorization_key_digest;
        witness.inputs[0].note.authorization_key_digest = authorization_key_digest;
        let input = &witness.inputs[0];
        let input_commitment =
            derive_pq_masp_note_commitment_v1(&statement, &input.note).expect("input commitment");
        statement.anchor = anchor_for_input_v1(
            &statement,
            input_commitment,
            input.leaf_position,
            &input.authentication_path,
        );
        statement.nullifiers[0] = derive_pq_masp_nullifier_v1(
            &statement,
            &input.nullifier_secret,
            &input.note.rho,
            input_commitment,
        )
        .expect("stable nullifier");
        (statement, witness)
    }
    /// Build the exact two-input/two-output boundary fixture.
    pub(crate) fn valid_two_by_two_fixture() -> (PqMaspStarkStatementV1, PqMaspWitnessV1) {
        let mut statement = statement_shell();
        statement.output_commitments.clear();
        statement.encrypted_outputs.clear();
        let first_secret = raw(50);
        let second_secret = raw(51);
        let first_input = PqMaspNotePlaintextV1 {
            value: 60,
            authorization_key_digest: statement.authorization_key_digest,
            recipient_key_digest: PrivacyRecipientIdV1::new(raw(52)),
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&first_secret)
                .expect("first nullifier key"),
            rho: raw(53),
            blinding: raw(54),
            memo_digest: raw(55),
        };
        let second_input = PqMaspNotePlaintextV1 {
            value: 40,
            authorization_key_digest: statement.authorization_key_digest,
            recipient_key_digest: PrivacyRecipientIdV1::new(raw(56)),
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&second_secret)
                .expect("second nullifier key"),
            rho: raw(57),
            blinding: raw(58),
            memo_digest: raw(59),
        };
        let first_input_commitment = derive_pq_masp_note_commitment_v1(&statement, &first_input)
            .expect("first input commitment");
        let second_input_commitment = derive_pq_masp_note_commitment_v1(&statement, &second_input)
            .expect("second input commitment");
        let first_leaf = accumulator_leaf_invocation_v1(&statement, 0, first_input_commitment)
            .expect("first leaf")
            .digest;
        let second_leaf = accumulator_leaf_invocation_v1(&statement, 1, second_input_commitment)
            .expect("second leaf")
            .digest;
        let mut first_path = empty_authentication_path_v1();
        let mut second_path = first_path;
        first_path[0] = second_leaf;
        second_path[0] = first_leaf;
        let first_anchor = anchor_for_input_v1(&statement, first_input_commitment, 0, &first_path);
        let second_anchor =
            anchor_for_input_v1(&statement, second_input_commitment, 1, &second_path);
        assert_eq!(first_anchor, second_anchor, "two-leaf paths share one root");
        statement.anchor = first_anchor;
        statement.nullifiers = vec![
            derive_pq_masp_nullifier_v1(
                &statement,
                &first_secret,
                &first_input.rho,
                first_input_commitment,
            )
            .expect("first nullifier"),
            derive_pq_masp_nullifier_v1(
                &statement,
                &second_secret,
                &second_input.rho,
                second_input_commitment,
            )
            .expect("second nullifier"),
        ];
        let first_output = PqMaspNotePlaintextV1 {
            value: 55,
            authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(60)),
            recipient_key_digest: PrivacyRecipientIdV1::new(raw(61)),
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&raw(62))
                .expect("first output nullifier key"),
            rho: raw(63),
            blinding: raw(64),
            memo_digest: raw(65),
        };
        let second_output = PqMaspNotePlaintextV1 {
            value: 45,
            authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(66)),
            recipient_key_digest: PrivacyRecipientIdV1::new(raw(67)),
            nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&raw(68))
                .expect("second output nullifier key"),
            rho: raw(69),
            blinding: raw(70),
            memo_digest: raw(71),
        };
        let first_output_commitment = derive_pq_masp_note_commitment_v1(&statement, &first_output)
            .expect("first output commitment");
        let second_output_commitment =
            derive_pq_masp_note_commitment_v1(&statement, &second_output)
                .expect("second output commitment");
        statement.output_commitments = vec![first_output_commitment, second_output_commitment];
        statement.encrypted_outputs = vec![
            encrypted_output_shell(first_output.recipient_key_digest, first_output_commitment),
            encrypted_output_shell(second_output.recipient_key_digest, second_output_commitment),
        ];
        statement.note_encryption_key_digest =
            derive_pq_masp_note_encryption_keys_digest_v1(&statement)
                .expect("ordered two-output key-set digest");
        (
            statement,
            PqMaspWitnessV1 {
                inputs: vec![
                    PqMaspInputWitnessV1 {
                        note: first_input,
                        nullifier_secret: first_secret,
                        leaf_position: 0,
                        authentication_path: first_path,
                    },
                    PqMaspInputWitnessV1 {
                        note: second_input,
                        nullifier_secret: second_secret,
                        leaf_position: 1,
                        authentication_path: second_path,
                    },
                ],
                outputs: vec![
                    PqMaspOutputWitnessV1 { note: first_output },
                    PqMaspOutputWitnessV1 {
                        note: second_output,
                    },
                ],
            },
        )
    }
    #[test]
    fn complete_relation_accepts_one_input_output_and_records_every_hash() {
        let (statement, witness) = valid_fixture();
        let validated =
            validate_pq_masp_relation_v1(&statement, &witness).expect("valid PQ-MASP relation");
        assert_eq!(validated.input_sum, 70);
        assert_eq!(validated.output_sum, 70);
        assert_eq!(validated.invocations.len(), 38);
        assert_eq!(
            validated
                .invocations
                .last()
                .expect("encryption-key invocation")
                .role,
            PqMaspSha256RoleV1::EncryptionKeySet
        );
    }
    #[test]
    fn complete_relation_accepts_the_exact_two_by_two_boundary() {
        let (statement, witness) = valid_two_by_two_fixture();
        let validated = validate_pq_masp_relation_v1(&statement, &witness)
            .expect("valid two-input/two-output relation");
        assert_eq!(validated.input_sum, 100);
        assert_eq!(validated.output_sum, 100);
        assert_eq!(validated.invocations.len(), 75);
        let mut wrong_position = witness.clone();
        wrong_position.inputs[1].leaf_position = 0;
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &wrong_position),
            Err(PqMaspRelationErrorV1::Membership)
        );
        let mut reordered_nullifiers = statement.clone();
        reordered_nullifiers.nullifiers.swap(0, 1);
        assert_eq!(
            validate_pq_masp_relation_v1(&reordered_nullifiers, &witness),
            Err(PqMaspRelationErrorV1::NullifierMismatch)
        );
        let mut reordered_outputs = statement.clone();
        reordered_outputs.output_commitments.swap(0, 1);
        reordered_outputs.encrypted_outputs.swap(0, 1);
        reordered_outputs.note_encryption_key_digest =
            derive_pq_masp_note_encryption_keys_digest_v1(&reordered_outputs)
                .expect("reordered key-set digest");
        assert_eq!(
            validate_pq_masp_relation_v1(&reordered_outputs, &witness),
            Err(PqMaspRelationErrorV1::RecipientMismatch)
        );
    }
    #[test]
    fn nullifier_is_stable_across_transaction_action_anchor_and_epoch() {
        let (statement, witness) = valid_fixture();
        let input = &witness.inputs[0];
        let commitment =
            derive_pq_masp_note_commitment_v1(&statement, &input.note).expect("commitment");
        let expected = derive_pq_masp_nullifier_v1(
            &statement,
            &input.nullifier_secret,
            &input.note.rho,
            commitment,
        )
        .expect("nullifier");
        let mut replay = statement.clone();
        replay.context.action_index = 1;
        replay.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(raw(90));
        replay.anchor = PrivacyRootV1::new(raw(91));
        replay.anchor_epoch = 99;
        replay.authorization_epoch = 99;
        assert_eq!(
            derive_pq_masp_nullifier_v1(
                &replay,
                &input.nullifier_secret,
                &input.note.rho,
                commitment,
            )
            .expect("replayed nullifier"),
            expected
        );
        replay.pool_id = PrivacyPoolIdV1::new(raw(92));
        assert_ne!(
            derive_pq_masp_nullifier_v1(
                &replay,
                &input.nullifier_secret,
                &input.note.rho,
                commitment,
            )
            .expect("cross-pool nullifier"),
            expected
        );
    }
    #[test]
    fn relation_rejects_every_private_binding_mutation() {
        let (statement, witness) = valid_fixture();
        let mut wrong_authority = witness.clone();
        wrong_authority.inputs[0].note.authorization_key_digest =
            PrivacyAuthorizationKeyDigestV1::new(raw(30));
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &wrong_authority),
            Err(PqMaspRelationErrorV1::AuthorizationKeyMismatch)
        );
        let mut wrong_secret = witness.clone();
        wrong_secret.inputs[0].nullifier_secret[0] ^= 1;
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &wrong_secret),
            Err(PqMaspRelationErrorV1::NullifierKeyMismatch)
        );
        let mut wrong_path = witness.clone();
        wrong_path.inputs[0].authentication_path[7][3] ^= 1;
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &wrong_path),
            Err(PqMaspRelationErrorV1::Membership)
        );
        let mut wrong_recipient = witness.clone();
        wrong_recipient.outputs[0].note.recipient_key_digest = PrivacyRecipientIdV1::new(raw(31));
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &wrong_recipient),
            Err(PqMaspRelationErrorV1::RecipientMismatch)
        );
        let mut inflation = witness.clone();
        inflation.outputs[0].note.value += 1;
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &inflation),
            Err(PqMaspRelationErrorV1::CommitmentMismatch)
        );
        let mut wrong_key_set = statement.clone();
        wrong_key_set.note_encryption_key_digest = PrivacyNoteEncryptionKeyDigestV1::new(raw(32));
        assert_eq!(
            validate_pq_masp_relation_v1(&wrong_key_set, &witness),
            Err(PqMaspRelationErrorV1::InvalidStatement)
        );
    }
    #[test]
    fn relation_rejects_shape_duplicates_and_checked_value_failures() {
        let (statement, witness) = valid_fixture();
        let mut empty = witness.clone();
        empty.inputs.clear();
        assert_eq!(
            validate_pq_masp_relation_v1(&statement, &empty),
            Err(PqMaspRelationErrorV1::WitnessShape)
        );
        let mut duplicate_statement = statement.clone();
        duplicate_statement.nullifiers.push(statement.nullifiers[0]);
        let mut duplicate_witness = witness.clone();
        duplicate_witness.inputs.push(witness.inputs[0].clone());
        assert_eq!(
            validate_pq_masp_relation_v1(&duplicate_statement, &duplicate_witness),
            Err(PqMaspRelationErrorV1::InvalidStatement)
        );
        let mut unequal_statement = statement.clone();
        let mut unequal = witness.clone();
        unequal.outputs[0].note.value = 69;
        unequal_statement.output_commitments[0] =
            derive_pq_masp_note_commitment_v1(&unequal_statement, &unequal.outputs[0].note)
                .expect("mutated output commitment");
        unequal_statement.encrypted_outputs[0].commitment = unequal_statement.output_commitments[0];
        assert_eq!(
            validate_pq_masp_relation_v1(&unequal_statement, &unequal),
            Err(PqMaspRelationErrorV1::ValueConservation)
        );
        let mut overflow = witness.clone();
        overflow.inputs[0].note.value = u128::MAX;
        let second = PqMaspInputWitnessV1 {
            note: PqMaspNotePlaintextV1 {
                value: 1,
                authorization_key_digest: statement.authorization_key_digest,
                recipient_key_digest: PrivacyRecipientIdV1::new(raw(40)),
                nullifier_key_digest: derive_pq_masp_nullifier_key_digest_v1(&raw(41))
                    .expect("second key"),
                rho: raw(42),
                blinding: raw(43),
                memo_digest: raw(44),
            },
            nullifier_secret: raw(41),
            leaf_position: 1,
            authentication_path: empty_authentication_path_v1(),
        };
        overflow.inputs.push(second);
        // Membership is checked before summation; use the direct checked-sum
        // oracle to pin the arithmetic boundary independently.
        assert_eq!(
            checked_sum(overflow.inputs.iter().map(|input| input.note.value)),
            Err(PqMaspRelationErrorV1::ValueOverflow)
        );
    }
    #[test]
    fn relation_rejects_zero_witness_components_and_cross_namespace_replay() {
        let (statement, witness) = valid_fixture();
        let mutations: [fn(&mut PqMaspWitnessV1); 13] = [
            |value| value.inputs[0].note.value = 0,
            |value| {
                value.inputs[0].note.authorization_key_digest =
                    PrivacyAuthorizationKeyDigestV1::new([0; 32]);
            },
            |value| {
                value.inputs[0].note.recipient_key_digest = PrivacyRecipientIdV1::new([0; 32]);
            },
            |value| value.inputs[0].note.nullifier_key_digest = [0; 32],
            |value| value.inputs[0].note.rho = [0; 32],
            |value| value.inputs[0].note.blinding = [0; 32],
            |value| value.inputs[0].nullifier_secret = [0; 32],
            |value| value.outputs[0].note.value = 0,
            |value| {
                value.outputs[0].note.authorization_key_digest =
                    PrivacyAuthorizationKeyDigestV1::new([0; 32]);
            },
            |value| {
                value.outputs[0].note.recipient_key_digest = PrivacyRecipientIdV1::new([0; 32]);
            },
            |value| value.outputs[0].note.nullifier_key_digest = [0; 32],
            |value| value.outputs[0].note.rho = [0; 32],
            |value| value.outputs[0].note.blinding = [0; 32],
        ];
        for mutate in mutations {
            let mut changed = witness.clone();
            mutate(&mut changed);
            assert_eq!(
                validate_pq_masp_relation_v1(&statement, &changed),
                Err(PqMaspRelationErrorV1::ZeroWitnessComponent)
            );
        }
        let mut other_pool = statement.clone();
        other_pool.pool_id = PrivacyPoolIdV1::new(raw(93));
        assert!(matches!(
            validate_pq_masp_relation_v1(&other_pool, &witness),
            Err(PqMaspRelationErrorV1::NullifierMismatch
                | PqMaspRelationErrorV1::CommitmentMismatch
                | PqMaspRelationErrorV1::Membership)
        ));
        let mut other_asset = statement.clone();
        other_asset.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("other_pq_note").expect("asset name"),
        );
        assert!(matches!(
            validate_pq_masp_relation_v1(&other_asset, &witness),
            Err(PqMaspRelationErrorV1::NullifierMismatch
                | PqMaspRelationErrorV1::CommitmentMismatch
                | PqMaspRelationErrorV1::Membership)
        ));
    }
    #[test]
    fn statement_caps_zero_values_and_encrypted_output_shapes_fail_closed() {
        let (statement, witness) = valid_fixture();
        let invalid: [fn(&mut PqMaspStarkStatementV1); 10] = [
            |value| value.pool_id = PrivacyPoolIdV1::new([0; 32]),
            |value| value.anchor = PrivacyRootV1::new([0; 32]),
            |value| value.anchor_epoch = 0,
            |value| value.authorization_epoch += 1,
            |value| {
                value.authorization_key_digest = PrivacyAuthorizationKeyDigestV1::new([0; 32]);
            },
            |value| {
                value.note_encryption_key_digest = PrivacyNoteEncryptionKeyDigestV1::new([0; 32]);
            },
            |value| value.nullifiers.clear(),
            |value| value.output_commitments.clear(),
            |value| value.encrypted_outputs.clear(),
            |value| {
                let _ = value.encrypted_outputs[0].ciphertext.pop();
            },
        ];
        for mutate in invalid {
            let mut changed = statement.clone();
            mutate(&mut changed);
            assert_eq!(
                validate_pq_masp_relation_v1(&changed, &witness),
                Err(PqMaspRelationErrorV1::InvalidStatement)
            );
        }
        let (mut over_input_cap, mut over_input_witness) = valid_two_by_two_fixture();
        let mut third = over_input_witness.inputs[0].clone();
        third.nullifier_secret = raw(94);
        third.note.nullifier_key_digest =
            derive_pq_masp_nullifier_key_digest_v1(&third.nullifier_secret)
                .expect("third nullifier key");
        third.note.rho = raw(95);
        over_input_cap
            .nullifiers
            .push(PrivacyNullifierV1::new(raw(96)));
        over_input_witness.inputs.push(third);
        assert_eq!(
            validate_pq_masp_relation_v1(&over_input_cap, &over_input_witness),
            Err(PqMaspRelationErrorV1::InvalidStatement)
        );
        let (mut duplicate_output, duplicate_output_witness) = valid_two_by_two_fixture();
        duplicate_output
            .output_commitments
            .push(duplicate_output.output_commitments[0]);
        duplicate_output
            .encrypted_outputs
            .push(duplicate_output.encrypted_outputs[0].clone());
        assert_eq!(
            validate_pq_masp_relation_v1(&duplicate_output, &duplicate_output_witness),
            Err(PqMaspRelationErrorV1::InvalidStatement)
        );
    }
}
