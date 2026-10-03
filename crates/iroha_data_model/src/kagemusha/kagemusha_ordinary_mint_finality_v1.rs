//! Complete distinct ordinary top-up source from genuine native reserve execution/finality.
//! Decoding returns data; current World, ordinary proof and one-use incoming State admission
//! remain separate closed consumers. No OEM Mint authorization is reconstructed.
use super::{KagemushaOrdinaryTopUpRequestV1, KagemushaSignedOrdinaryMintDebitDecisionV1};
use crate::isi::kagemusha_v1::{
    KAGEMUSHA_OPERATION_RESULT_MAX_BYTES_V1, KagemushaFinalityTrustAnchorV1,
    KagemushaOperationFinalityV1, KagemushaOperationKindV1,
};
use norito::{Decode, Encode, JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};

/// Sole complete ordinary finalized-source canonical frame bound.
/// The existing native block/result budget is retained, with bounded request/decision framing.
pub const KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1: usize =
    KAGEMUSHA_OPERATION_RESULT_MAX_BYTES_V1 + 512 * 1024;

/// Purpose-bound actual debit intent digest, distinct from the acyclic unsigned request selector.
/// Both full canonical originals are retained in the finalized source, never replaced by hashes.
#[must_use]
pub fn kagemusha_ordinary_mint_applied_intent_digest_v1(
    request_original: &[u8],
    decision_original: &[u8],
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:ordinary-mint-applied-intent\0");
    h.update((request_original.len() as u64).to_le_bytes());
    h.update(request_original);
    h.update((decision_original.len() as u64).to_le_bytes());
    h.update(decision_original);
    h.finalize().into()
}

/// Full ordinary request, original signed effect decision and original certified reserve receipt.
/// This is portable data, with no conversion to a financial or finalized-debit authority.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryTopUpFinalizedOriginalV1")]
pub struct KagemushaOrdinaryTopUpFinalizedOriginalV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Complete accepted unsigned request including exact platform approval, paired proof and ciphertext.
    pub request_original: Vec<u8>,
    /// Complete independently signed current effect decision originally consumed by the debit.
    pub issuer_decision_original: Vec<u8>,
    /// Actual native quorum, original receipt write and neutral top-up membership proof.
    pub finality: KagemushaOperationFinalityV1,
}
impl KagemushaOrdinaryTopUpFinalizedOriginalV1 {
    /// Recheck full original data and exact original receipt binding, without claiming finality.
    /// # Errors
    /// Refuses wrong version, noncanonical originals, changed scope, effect time or receipt.
    pub fn validate_originals(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported ordinary finalized top-up version".into());
        }
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&self.request_original)?;
        let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &self.issuer_decision_original,
        )?;
        decision
            .subject
            .selection
            .validate_against_topup(&request)?;
        let context = &request.authorization.statement.context;
        let runtime = &context.lineage.owner.runtime;
        let receipt = &self.finality.reserve_receipt_witness.receipt;
        receipt.validate().map_err(|e| e.to_string())?;
        let statement = request
            .authorization
            .finalized_credit_statement(receipt.committed_at_ms)?;
        if receipt.kind != KagemushaOperationKindV1::TopUp
            || self.finality.network_id != runtime.network_id
            || receipt.operation_id != context.operation_id
            || receipt.request_digest
                != kagemusha_ordinary_mint_applied_intent_digest_v1(
                    &self.request_original,
                    &self.issuer_decision_original,
                )
            || receipt.mint_statement_digest
                != statement.canonical_digest().map_err(|e| e.to_string())?
            || receipt.network_id != runtime.network_id
            || receipt.asset != runtime.asset
            || receipt.asset_incarnation != runtime.asset_incarnation
            || receipt.scale != runtime.scale
            || receipt.amount != context.amount
            || decision.subject.release_id != context.release_id
            || receipt.committed_at_ms < decision.subject.issued_at_ms
            || receipt.committed_at_ms >= decision.subject.expires_at_ms
            || self.finality.top_up_membership_witness.is_none()
        {
            return Err("ordinary finalized top-up original/receipt differs".into());
        }
        Ok(())
    }
    /// Verify original consensus and membership under the independently selected checkpoint.
    /// This returns no proof/debit/current FI capability or receiver funding authority.
    /// # Errors
    /// Refuses invalid original data or a finality/receipt not certified by the selected prefix.
    pub fn validate_against(&self, anchor: &KagemushaFinalityTrustAnchorV1) -> Result<(), String> {
        self.validate_originals()?;
        self.finality
            .validate_against(anchor)
            .map_err(|e| e.to_string())
    }
    /// Sole bounded canonical complete original; no field fallback or compact replacement.
    /// # Errors
    /// Refuses invalid original data, encoding failure or the full frame budget.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_originals()?;
        let bytes = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if bytes.len() > KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 {
            return Err("ordinary finalized top-up exceeds full original budget".into());
        }
        Ok(bytes)
    }
    /// Decode exact complete data only, checking finite bound before allocation/decode.
    /// # Errors
    /// Refuses oversized, noncanonical, trailing or invalid original data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 {
            return Err("ordinary finalized top-up outside full original budget".into());
        }
        let value: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != bytes {
            return Err("ordinary finalized top-up is not sole canonical original".into());
        }
        Ok(value)
    }
}

/// Complete data-frame ceiling, preserving the full maintained finality and neutral credit caps.
/// Canonical framing is explicit; no component is truncated to fit a transport default.
pub const KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1: usize =
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
        + super::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
        + 4096;

/// Full ordinary finalized debit and its actual neutral MintAuthority credit proof originals.
/// Decoding gives data only: independent release/finality/proof admission remains mandatory.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryFinalizedMintCreditOriginalV1"
)]
pub struct KagemushaOrdinaryFinalizedMintCreditOriginalV1 {
    /// First-release layout version.
    pub version: u16,
    /// Exact complete finalized debit original, including actual native receipt/membership.
    pub finalized_source_original: Vec<u8>,
    /// Exact canonical full neutral MintCredit, including both proofs and histories.
    pub mint_credit_original: Vec<u8>,
}
impl KagemushaOrdinaryFinalizedMintCreditOriginalV1 {
    /// Full original shape/equality checks only; no proof/effect capability is returned.
    /// # Errors
    /// Refuses unsupported, oversized, noncanonical or substituted complete operands.
    pub fn validate_originals(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("ordinary finalized credit version unsupported".into());
        }
        let finalized = KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(
            &self.finalized_source_original,
        )?;
        let credit =
            super::KagemushaMintCreditV1::decode_canonical_shape_exact(&self.mint_credit_original)
                .map_err(|e| e.to_string())?;
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&finalized.request_original)?;
        request.validate_finalized_credit(
            &credit,
            finalized
                .finality
                .reserve_receipt_witness
                .receipt
                .committed_at_ms,
        )?;
        if norito::encode_canonical(&credit).map_err(|e| e.to_string())?
            != self.mint_credit_original
        {
            return Err("ordinary finalized credit is not the complete sole original".into());
        }
        Ok(())
    }
    /// Encode the sole complete bounded data frame.
    /// # Errors
    /// Refuses invalid originals, encoding failure or full-frame overflow.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_originals()?;
        let bytes = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if bytes.len() > KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1 {
            return Err("ordinary finalized credit full frame exceeds its ceiling".into());
        }
        Ok(bytes)
    }
    /// Decode complete data with the finite ceiling charged before decode.
    /// # Errors
    /// Refuses missing, oversized, trailing, noncanonical or substituted originals.
    pub fn decode_canonical_exact(raw: &[u8]) -> Result<Self, String> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1 {
            return Err("ordinary finalized credit full frame outside its ceiling".into());
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != raw {
            return Err("ordinary finalized credit frame is not sole canonical original".into());
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_credit_frame_refuses_missing_components_and_over_budget_before_decode() {
        let mut data = KagemushaOrdinaryFinalizedMintCreditOriginalV1 {
            version: 1,
            finalized_source_original: Vec::new(),
            mint_credit_original: Vec::new(),
        };
        assert!(data.canonical_bytes().is_err());
        data.version = 2;
        assert!(data.validate_originals().is_err());
        assert!(
            KagemushaOrdinaryFinalizedMintCreditOriginalV1::decode_canonical_exact(&[]).is_err()
        );
        assert!(
            KagemushaOrdinaryFinalizedMintCreditOriginalV1::decode_canonical_exact(&vec![
                0;
                KAGEMUSHA_ORDINARY_FINALIZED_MINT_CREDIT_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        // Sole model codec refuses a canonical container whose inner full originals are invalid;
        // this test never fabricates a credit capability, finality or accepting verifier.
        let raw = norito::encode_canonical(&data).unwrap();
        assert!(
            KagemushaOrdinaryFinalizedMintCreditOriginalV1::decode_canonical_exact(&raw).is_err()
        );
    }
    #[test]
    fn actual_debit_intent_binds_both_complete_originals_and_their_lengths() {
        let a = kagemusha_ordinary_mint_applied_intent_digest_v1(b"ab", b"c");
        assert_ne!(
            a,
            kagemusha_ordinary_mint_applied_intent_digest_v1(b"a", b"bc")
        );
        assert_ne!(
            a,
            kagemusha_ordinary_mint_applied_intent_digest_v1(b"ab", b"changed issuer signature")
        );
        assert_ne!(a, <[u8; 32]>::from(Sha256::digest(b"ab")));
    }
    #[test]
    fn finalized_source_decoder_refuses_missing_trailing_and_over_budget_originals() {
        for raw in [
            Vec::new(),
            vec![0; 64],
            vec![0; KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 + 1],
        ] {
            assert!(
                KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(&raw).is_err()
            );
        }
    }
}
