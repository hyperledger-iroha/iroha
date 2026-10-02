//! Data-only exact canonical capacity planning for a genuine installed ordinary Mint profile.
//! The private neutral layout specimen is never accepted as an approval/proof or Native grant.
use super::{
    KagemushaAuthenticatedRecursiveVerifierV1, deferred_parent::ordinary_ipa_proof_profile_v1,
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
};
use iroha_data_model::kagemusha::*;

/// Exact finite request-slot byte allowance derived from loaded protocols and same retained C.
/// This number carries no authorization, key custody, current time or financial authority.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaOrdinaryMintRequestByteBudgetV1 {
    maximum_original_bytes: usize,
}
impl KagemushaOrdinaryMintRequestByteBudgetV1 {
    /// Maximum whole canonical request for the same context and supported actual platform grammar.
    #[must_use]
    pub const fn maximum_original_bytes(self) -> usize {
        self.maximum_original_bytes
    }
}
/// Compute before reserving the Native request slot or invoking the platform. Exact current
/// Eq/Ep proof lengths come from the actual authenticated protocols; histories are canonical.
/// Maximum evidence size comes from the maintained full Android DER/Apple assertion grammar.
/// Full signed C/FI/clock/service carriers require their separate actual bounded custody slots.
/// # Errors
/// Refuses wrong actual profile/credential, overflow or a genuine protocol which does not fit
/// the complete supported request envelope. It never truncates proof/platform originals.
pub fn ordinary_mint_request_byte_budget_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    context: &KagemushaOrdinaryMintAuthorizationContextV1,
    actual_credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
) -> Result<KagemushaOrdinaryMintRequestByteBudgetV1, String> {
    context.validate_against_credential(actual_credential)?;
    if norito::encode_canonical(&context.lineage.owner.account_id)
        .map_err(|e| e.to_string())?
        .len()
        > 4096
    {
        return Err(
            "ordinary Mint account original exceeds the authenticated relation capacity".into(),
        );
    }
    let m = verifier.ordinary_mint_material()?;
    if context.release_id != m.release_id
        || context.suite_id != m.suite_id
        || context.vk_digest != m.vk_set_digest
        || context.artifact_manifest_digest != m.artifact_manifest_digest
    {
        return Err("ordinary Mint request budget selects another loaded release".into());
    }
    let eq = ordinary_ipa_proof_profile_v1(m.eq_protocol)?.byte_len;
    let ep = ordinary_ipa_proof_profile_v1(m.ep_protocol)?.byte_len;
    let evidence = match actual_credential.subject().platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: vec![0; 72],
            }
        }
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                raw_assertion: vec![0; KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1],
            }
        }
        _ => return Err("ordinary Mint budget actual platform differs".into()),
    };
    // Reject before any protocol-sized allocation. Metadata is encoded exactly below.
    if eq == 0
        || ep == 0
        || eq > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
        || ep > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
        || eq
            .checked_add(ep)
            .is_none_or(|n| n > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1)
        || eq
            .checked_add(ep)
            .and_then(|n| n.checked_add(2 * KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1))
            .is_none_or(|n| n > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1)
    {
        return Err(
            "actual ordinary Mint proofs cannot fit supported complete request capacity".into(),
        );
    }
    let statement = KagemushaOrdinaryMintAuthorizationStatementV1 {
        version: 1,
        context: context.clone(),
        issuance_commitment: context.issuance_commitment()?,
        credit_id: context.credit_id()?,
        ciphertext_digest: [1; 32],
    };
    let approval = KagemushaOrdinaryMintApprovalV1 {
        challenge: KagemushaOrdinaryMintApprovalChallengeV1 {
            version: 1,
            operation_id: context.operation_id,
            nonce: [1; 32],
            credential_digest: context.recipient_app_credential_digest,
            statement_digest: statement.binding_digest()?,
            clock_context_digest: context.clock_context.binding_digest()?,
            financial_control_original_sha256: context.financial_control_original_sha256,
            issued_at_ms: 0,
            expires_at_ms: 0,
        },
        evidence,
    };
    let eh = initial_kagemusha_eq_accumulator_v1(m.eq_parameters).map_err(|e| e.to_string())?;
    let ph = initial_kagemusha_ep_accumulator_v1(m.ep_parameters).map_err(|e| e.to_string())?;
    let request = KagemushaOrdinaryTopUpRequestV1 {
        version: 1,
        encrypted_credit: vec![0; 384],
        authorization: KagemushaOrdinaryMintAuthorizationV1 {
            version: 1,
            statement,
            approval,
            proof: KagemushaOrdinaryMintPairedProofV1 {
                version: 1,
                eq_protocol_digest: m.eq_protocol_digest,
                ep_protocol_digest: m.ep_protocol_digest,
                statement_digest: [1; 32],
                approval_original_digest: [1; 32],
                eq_proof: vec![0; eq],
                ep_proof: vec![0; ep],
                eq_history: eh.as_bytes().to_vec(),
                ep_history: ph.as_bytes().to_vec(),
            },
        },
    };
    // Only the sole codec's data layout is measured. Deliberately invalid neutral witnesses
    // cannot pass the actual approval/proof verifier and leave this function as a numeric size.
    let length = norito::encode_canonical(&request)
        .map_err(|e| e.to_string())?
        .len();
    if length > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1 {
        return Err(format!(
            "actual ordinary Mint profile/context needs {length} canonical bytes; supported maximum is {}",
            KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1
        ));
    }
    Ok(KagemushaOrdinaryMintRequestByteBudgetV1 {
        maximum_original_bytes: length,
    })
}
