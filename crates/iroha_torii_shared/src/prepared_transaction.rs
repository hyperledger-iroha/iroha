//! Stable cross-SDK signing transcript for Taira prepared transactions.

use iroha_crypto::Hash;

/// Domain prepended to every V1 prepared-transaction signing transcript.
pub const PREPARED_TRANSACTION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:taira:prepared-transaction:v1\0";
/// Schema identifier carried inside every V1 transcript.
pub const PREPARED_TRANSACTION_SIGNATURE_TRANSCRIPT_SCHEMA_V1: &str =
    "iroha.taira.prepared-signature-transcript.v1";

/// Borrowed public-reset mutation binding fields committed by a transcript.
#[derive(Clone, Copy, Debug)]
pub struct PreparedMutationBindingRefV1<'a> {
    /// Exact binding schema.
    pub schema: &'a str,
    /// Lowercase SHA-256 of the admitted reset authorization.
    pub authorization_sha256: &'a str,
    /// Exact authorization nonce.
    pub authorization_nonce: &'a str,
    /// Exact operation kind.
    pub kind: &'a str,
    /// Exact reset phase.
    pub phase: &'a str,
    /// Lowercase mutation idempotency digest.
    pub idempotency_key: &'a str,
    /// Absolute execution deadline in Unix milliseconds.
    pub execution_expires_at_unix_ms: u64,
}

/// Borrowed fields signed for one prepared onboarding transaction.
#[derive(Clone, Copy, Debug)]
pub struct OnboardingPreparedSignatureFieldsV1<'a> {
    /// Exact envelope schema.
    pub envelope_schema: &'a str,
    /// Exact public-reset mutation binding.
    pub binding: PreparedMutationBindingRefV1<'a>,
    /// Lowercase signed receipt/body hash.
    pub semantic_hash_hex: &'a str,
    /// Canonical target account.
    pub account_id: &'a str,
    /// Canonical target alias.
    pub alias: &'a str,
    /// Canonical disposition spelling (`create`, `repair`, or `no_op`).
    pub disposition: &'a str,
    /// Lowercase exact transaction hash.
    pub transaction_hash_hex: &'a str,
    /// Lowercase SHA-256 of the fixed-V1 transaction wire.
    pub signed_transaction_wire_sha256: &'a str,
    /// Exact fixed-V1 `SignedTransaction` wire bytes.
    pub signed_transaction_wire: &'a [u8],
}

/// Borrowed fields signed for an authenticated onboarding result that still requires live proof.
#[derive(Clone, Copy, Debug)]
pub struct OnboardingProofRequiredSignatureFieldsV1<'a> {
    /// Exact proof-required result schema.
    pub envelope_schema: &'a str,
    /// Exact public-reset mutation binding.
    pub binding: PreparedMutationBindingRefV1<'a>,
    /// Exact nonterminal outcome, always `ProofRequired`.
    pub outcome: &'a str,
    /// Exact live-state proof required before a coordinator may terminalize the result.
    pub proof_kind: &'a str,
    /// Lowercase signed receipt/body hash.
    pub semantic_hash_hex: &'a str,
    /// Canonical target account.
    pub account_id: &'a str,
    /// Canonical target alias.
    pub alias: &'a str,
    /// Canonical disposition spelling, currently `no_op`.
    pub disposition: &'a str,
}

/// Borrowed fields signed for one prepared faucet transaction.
#[derive(Clone, Copy, Debug)]
pub struct FaucetPreparedSignatureFieldsV1<'a> {
    /// Exact envelope schema.
    pub envelope_schema: &'a str,
    /// Exact public-reset mutation binding.
    pub binding: PreparedMutationBindingRefV1<'a>,
    /// Canonical claim account.
    pub claim_account_id: &'a str,
    /// Optional proof-of-work anchor height.
    pub claim_pow_anchor_height: Option<u64>,
    /// Optional canonical lowercase proof-of-work nonce.
    pub claim_pow_nonce_hex: Option<&'a str>,
    /// Lowercase domain-separated claim hash.
    pub semantic_hash_hex: &'a str,
    /// Canonical result account.
    pub account_id: &'a str,
    /// Canonical faucet asset definition.
    pub asset_definition_id: &'a str,
    /// Canonical destination asset.
    pub asset_id: &'a str,
    /// Canonical quantity text.
    pub amount: &'a str,
    /// Lowercase exact transaction hash.
    pub transaction_hash_hex: &'a str,
    /// Lowercase SHA-256 of the fixed-V1 transaction wire.
    pub signed_transaction_wire_sha256: &'a str,
    /// Exact fixed-V1 `SignedTransaction` wire bytes.
    pub signed_transaction_wire: &'a [u8],
}

/// Build the exact V1 onboarding-prepared signature transcript.
#[must_use]
pub fn onboarding_prepared_signature_transcript_v1(
    fields: OnboardingPreparedSignatureFieldsV1<'_>,
) -> Vec<u8> {
    let mut transcript = base_transcript(fields.envelope_schema, "onboarding", fields.binding);
    append_field(
        &mut transcript,
        b"semantic_hash_hex",
        fields.semantic_hash_hex.as_bytes(),
    );
    append_field(&mut transcript, b"account_id", fields.account_id.as_bytes());
    append_field(&mut transcript, b"alias", fields.alias.as_bytes());
    append_field(
        &mut transcript,
        b"disposition",
        fields.disposition.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"transaction_hash_hex",
        fields.transaction_hash_hex.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"signed_transaction_wire_sha256",
        fields.signed_transaction_wire_sha256.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"signed_transaction_wire",
        fields.signed_transaction_wire,
    );
    transcript
}

/// Build the exact V1 onboarding-proof-required signature transcript.
#[must_use]
pub fn onboarding_proof_required_signature_transcript_v1(
    fields: OnboardingProofRequiredSignatureFieldsV1<'_>,
) -> Vec<u8> {
    let mut transcript = base_transcript(fields.envelope_schema, "onboarding", fields.binding);
    append_field(&mut transcript, b"outcome", fields.outcome.as_bytes());
    append_field(&mut transcript, b"proof_kind", fields.proof_kind.as_bytes());
    append_field(
        &mut transcript,
        b"semantic_hash_hex",
        fields.semantic_hash_hex.as_bytes(),
    );
    append_field(&mut transcript, b"account_id", fields.account_id.as_bytes());
    append_field(&mut transcript, b"alias", fields.alias.as_bytes());
    append_field(
        &mut transcript,
        b"disposition",
        fields.disposition.as_bytes(),
    );
    transcript
}

/// Build the exact V1 faucet-prepared signature transcript.
#[must_use]
pub fn faucet_prepared_signature_transcript_v1(
    fields: FaucetPreparedSignatureFieldsV1<'_>,
) -> Vec<u8> {
    let mut transcript = base_transcript(fields.envelope_schema, "faucet", fields.binding);
    append_field(
        &mut transcript,
        b"claim.account_id",
        fields.claim_account_id.as_bytes(),
    );
    let anchor = fields
        .claim_pow_anchor_height
        .map_or_else(|| "none".to_owned(), |value| format!("some:{value}"));
    append_field(
        &mut transcript,
        b"claim.pow_anchor_height",
        anchor.as_bytes(),
    );
    let nonce = fields
        .claim_pow_nonce_hex
        .map_or_else(|| "none".to_owned(), |value| format!("some:{value}"));
    append_field(&mut transcript, b"claim.pow_nonce_hex", nonce.as_bytes());
    append_field(
        &mut transcript,
        b"semantic_hash_hex",
        fields.semantic_hash_hex.as_bytes(),
    );
    append_field(&mut transcript, b"account_id", fields.account_id.as_bytes());
    append_field(
        &mut transcript,
        b"asset_definition_id",
        fields.asset_definition_id.as_bytes(),
    );
    append_field(&mut transcript, b"asset_id", fields.asset_id.as_bytes());
    append_field(&mut transcript, b"amount", fields.amount.as_bytes());
    append_field(
        &mut transcript,
        b"transaction_hash_hex",
        fields.transaction_hash_hex.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"signed_transaction_wire_sha256",
        fields.signed_transaction_wire_sha256.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"signed_transaction_wire",
        fields.signed_transaction_wire,
    );
    transcript
}

/// Compute the Iroha BLAKE2b-256 digest signed for a prepared transcript.
#[must_use]
pub fn prepared_signature_digest_v1(transcript: &[u8]) -> Hash {
    Hash::new(transcript)
}

fn base_transcript(
    envelope_schema: &str,
    operation: &str,
    binding: PreparedMutationBindingRefV1<'_>,
) -> Vec<u8> {
    let mut transcript = Vec::new();
    append_frame(&mut transcript, PREPARED_TRANSACTION_SIGNATURE_DOMAIN_V1);
    append_field(
        &mut transcript,
        b"transcript_schema",
        PREPARED_TRANSACTION_SIGNATURE_TRANSCRIPT_SCHEMA_V1.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"envelope_schema",
        envelope_schema.as_bytes(),
    );
    append_field(&mut transcript, b"operation", operation.as_bytes());
    append_field(
        &mut transcript,
        b"binding.schema",
        binding.schema.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"binding.authorization_sha256",
        binding.authorization_sha256.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"binding.authorization_nonce",
        binding.authorization_nonce.as_bytes(),
    );
    append_field(&mut transcript, b"binding.kind", binding.kind.as_bytes());
    append_field(&mut transcript, b"binding.phase", binding.phase.as_bytes());
    append_field(
        &mut transcript,
        b"binding.idempotency_key",
        binding.idempotency_key.as_bytes(),
    );
    append_field(
        &mut transcript,
        b"binding.execution_expires_at_unix_ms",
        binding.execution_expires_at_unix_ms.to_string().as_bytes(),
    );
    transcript
}

fn append_field(transcript: &mut Vec<u8>, label: &[u8], value: &[u8]) {
    append_frame(transcript, label);
    append_frame(transcript, value);
}

fn append_frame(transcript: &mut Vec<u8>, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).expect("prepared transcript frame length fits u64");
    transcript.extend_from_slice(&length.to_be_bytes());
    transcript.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_frames_are_unambiguous_and_domain_first() {
        let binding = PreparedMutationBindingRefV1 {
            schema: "binding",
            authorization_sha256: "11",
            authorization_nonce: "nonce",
            kind: "onboarding",
            phase: "pre_edge",
            idempotency_key: "22",
            execution_expires_at_unix_ms: 42,
        };
        let transcript = onboarding_proof_required_signature_transcript_v1(
            OnboardingProofRequiredSignatureFieldsV1 {
                envelope_schema: "proof-required",
                binding,
                outcome: "ProofRequired",
                proof_kind: "account_alias_current_state",
                semantic_hash_hex: "33",
                account_id: "account",
                alias: "alias",
                disposition: "no_op",
            },
        );
        let domain_length = u64::from_be_bytes(
            transcript[..8]
                .try_into()
                .expect("domain length prefix is eight bytes"),
        );
        assert_eq!(
            domain_length,
            u64::try_from(PREPARED_TRANSACTION_SIGNATURE_DOMAIN_V1.len())
                .expect("domain length fits u64")
        );
        assert_eq!(
            &transcript[8..8 + PREPARED_TRANSACTION_SIGNATURE_DOMAIN_V1.len()],
            PREPARED_TRANSACTION_SIGNATURE_DOMAIN_V1
        );
        assert_ne!(
            prepared_signature_digest_v1(&transcript),
            prepared_signature_digest_v1(&transcript[..transcript.len() - 1])
        );
    }
}
