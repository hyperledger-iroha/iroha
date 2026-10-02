//! Independent current World purpose and exact signed pre-debit decision admission.
//!
//! This closed evidence cannot debit or emit credit. The ordinary Mint mathematical result and
//! actual protected pooled-reserve mutation are separate mandatory consumers. No OEM proof type,
//! hardware credential/counter, decoded policy or read-only FI status is promoted here.

use super::ordinary_mint_clock::KagemushaWorldOrdinaryMintSignedClockV1;
use super::ordinary_mint_permission::{
    KagemushaWorldOrdinaryMintIssuerPurposeV1, admit_ordinary_mint_issuer_purpose_v1,
};
use crate::state::StateTransaction;
use iroha_crypto::Signature;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KagemushaOrdinaryIncomingSelectionV1, KagemushaOrdinaryTopUpRequestV1,
        KagemushaSignedOrdinaryMintDebitDecisionV1,
    },
};
use iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint;
use sha2::{Digest as _, Sha256};

/// Closed current World issuer-purpose and exact request/consent/decision equation evidence.
/// The genuine ordinary proof and one-shot reserve debit are not constructed by this module.
pub struct KagemushaWorldOrdinaryMintDebitDecisionV1 {
    purpose: KagemushaWorldOrdinaryMintIssuerPurposeV1,
    current_clock: KagemushaWorldOrdinaryMintSignedClockV1,
    request_original_sha256: [u8; 32],
    decision_original: Vec<u8>,
    decision: KagemushaSignedOrdinaryMintDebitDecisionV1,
}
impl KagemushaWorldOrdinaryMintDebitDecisionV1 {
    /// Exact whole unsigned original request SHA; future issuer decision/finality is excluded.
    #[must_use]
    pub fn request_original_sha256(&self) -> [u8; 32] {
        self.request_original_sha256
    }
    /// Borrow the exact independently World-admitted signed decision data.
    #[must_use]
    pub fn decision(&self) -> &KagemushaSignedOrdinaryMintDebitDecisionV1 {
        &self.decision
    }
    /// Borrow the exact complete signed original for actual Node immutable execution records.
    #[must_use]
    pub fn decision_original(&self) -> &[u8] {
        &self.decision_original
    }
    /// Recheck the same currently admitted decision before actual separate reserve mutation.
    /// # Errors
    /// Refuses revoked purpose, changed full unsigned request, original expiry or authority drift.
    pub fn recheck(
        &self,
        transaction: &StateTransaction<'_, '_>,
        request: &KagemushaOrdinaryTopUpRequestV1,
        authority: &AccountId,
    ) -> Result<(), String> {
        self.purpose.recheck(transaction)?;
        if self.decision.subject.data_incarnation_digest
            != self
                .purpose
                .lineage_data_authority()
                .data_incarnation_digest
        {
            return Err("ordinary Mint decision substituted the governed DATA incarnation".into());
        }
        if authority
            != &request
                .authorization
                .statement
                .context
                .lineage
                .owner
                .account_id
            || self.request_original_sha256
                != <[u8; 32]>::from(Sha256::digest(request.canonical_bytes()?))
            || self.decision.canonical_bytes()? != self.decision_original
        {
            return Err("ordinary Mint retained decision/request/payer was substituted".into());
        }
        self.decision.verify_for_request(
            request,
            &self.decision.subject.selection,
            self.purpose.issuer_policy(),
        )?;
        self.current_clock.require_current_execution_cut(
            transaction,
            &self.decision.subject.decision_clock_context,
            self.decision.subject.authority_height,
            self.decision.subject.authority_context_id,
            self.decision.subject.world_root,
        )?;
        // Ledger execution time is consensus data. It is not relabeled as Native elapsed time:
        // the issuing Core separately captured the original live clock before signing.
        let now = transaction.block_unix_timestamp_ms();
        if now < self.decision.subject.issued_at_ms || now >= self.decision.subject.expires_at_ms {
            return Err(
                "ordinary Mint issuer decision is outside its original effect window".into(),
            );
        }
        Ok(())
    }
}

/// Authenticate a separate pre-debit decision under actual current World and payer consent.
///
/// `authority` is the actual signed transaction authority supplied by the native instruction
/// dispatcher. No returned value can invoke the protected reserve debit without separately
/// admitted ordinary Mint proof, exact current pool plan and immutable operation conflict checks.
/// # Errors
/// Refuses wrong signed payer, account consent, full request/selector, current World purpose or
/// immutable issuer window. An expired preparation FI original is never substituted or renewed.
pub fn admit_ordinary_mint_debit_decision_v1(
    transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    request: &KagemushaOrdinaryTopUpRequestV1,
    selection: &KagemushaOrdinaryIncomingSelectionV1,
    account_consent: &Signature,
    issuer_purpose: &CanAuthorizeKagemushaOrdinaryMint,
    signed: &KagemushaSignedOrdinaryMintDebitDecisionV1,
    current_clock: KagemushaWorldOrdinaryMintSignedClockV1,
) -> Result<KagemushaWorldOrdinaryMintDebitDecisionV1, String> {
    if authority
        != &request
            .authorization
            .statement
            .context
            .lineage
            .owner
            .account_id
    {
        return Err("ordinary Mint signed transaction authority is not the payer".into());
    }
    request.verify_account_signature(account_consent)?;
    selection.validate_against_topup(request)?;
    let purpose = admit_ordinary_mint_issuer_purpose_v1(transaction, issuer_purpose)?;
    signed.verify_for_request(request, selection, purpose.issuer_policy())?;
    let value = KagemushaWorldOrdinaryMintDebitDecisionV1 {
        purpose,
        current_clock,
        request_original_sha256: Sha256::digest(request.canonical_bytes()?).into(),
        decision_original: signed.canonical_bytes()?,
        decision: signed.clone(),
    };
    value.recheck(transaction, request, authority)?;
    Ok(value)
}
