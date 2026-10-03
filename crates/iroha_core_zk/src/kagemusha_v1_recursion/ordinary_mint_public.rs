//! Sole ordinary Mint expected-public contract; no offered instance-vector authority.
//! Exact canonical request preprocessing is public and independent of witness/key generation.
use super::{DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1};
use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128};
use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};
pub(crate) const ORDINARY_MINT_PUBLIC_PREFIX_V1: usize = 79;
const DIGEST_COUNT: usize = 34;
const SCALAR_COUNT: usize = 11;
// Scalar9 is the signed C enrollment minimum only, never a current Native journal floor.
// Native proving separately lends its genuine original previous floor to the platform equation.
// Sole ordered public contract. These offsets are consumed by the closed ordinary Mint verifier.
#[derive(Clone, Copy)]
pub(crate) struct OrdinaryMintPublicDataV1 {
    pub(crate) digests: [DigestV1; DIGEST_COUNT],
    pub(crate) scalars: [u128; SCALAR_COUNT],
}
fn digest_framed(domain: &[u8], raw: &[u8]) -> DigestV1 {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((raw.len() as u64).to_le_bytes());
    h.update(raw);
    h.finalize().into()
}
/// Pure complete expected-public reconstruction. It neither authenticates C/PI/time nor proof.
/// The closed verifier must independently admit those operands before using this fixed contract.
pub(crate) fn ordinary_mint_public_data_v1(
    s: &KagemushaOrdinaryMintAuthorizationStatementV1,
    a: &KagemushaOrdinaryMintApprovalV1,
    c: &KagemushaOrdinaryAppCredentialV1,
    lease: Option<&KagemushaPlayIntegrityRefreshLeaseV1>,
    provider_root: DigestV1,
) -> Result<OrdinaryMintPublicDataV1, String> {
    s.validate_shape()?;
    a.validate_shape()?;
    a.challenge.validate_against_statement(s)?;
    let x = &s.context;
    let o = &x.lineage.owner;
    let rt = &o.runtime;
    let cd = c.canonical_digest()?;
    if cd != x.recipient_app_credential_digest || provider_root == [0; 32] {
        return Err("ordinary Mint original C/provider differs".into());
    }
    let apple = c.subject.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest;
    if apple
        != matches!(
            a.evidence,
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { .. }
        )
    {
        return Err("ordinary Mint actual platform/counter scope differs".into());
    }
    let evidence = match &a.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => signature_der,
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => raw_assertion,
    };
    let canonical_account = norito::encode_canonical(&o.account_id).map_err(|e| e.to_string())?;
    // These account hashes are sole-verifier public preprocessing. Keep the original
    // canonical-byte ceiling here; neither a supplied digest nor a circuit witness can
    // substitute a different account for either domain-separated identity.
    if canonical_account.len() > 4096 {
        return Err("ordinary Mint account original capacity differs".into());
    }
    let account_binding = kagemusha_ordinary_app_account_binding_v1(&o.account_id);
    if account_binding != c.subject.account_binding {
        return Err("ordinary Mint public account differs from original credential".into());
    }
    let digests = [
        s.binding_digest()?,
        x.binding_digest()?,
        cd,
        a.binding_digest()?,
        Sha256::digest(evidence).into(),
        lease
            .map(|l| l.canonical_digest())
            .transpose()?
            .unwrap_or([0; 32]),
        x.operation_id,
        x.release_id,
        x.suite_id,
        x.vk_digest,
        x.artifact_manifest_digest,
        *rt.network_id.as_bytes(),
        kagemusha_asset_identity_digest_v1(&rt.asset).map_err(|e| e.to_string())?,
        *rt.asset_incarnation.as_bytes(),
        kagemusha_liability_pool_id_v1(&rt.network_id, &rt.asset, rt.asset_incarnation)
            .map_err(|e| e.to_string())?,
        o.lane_id,
        account_binding,
        digest_framed(b"iroha:kagemusha:v1:account-identity\0", &canonical_account),
        x.lineage.financial_epoch_id,
        x.lineage.financial_authority_commitment,
        x.app_credential_profile_id,
        x.recipient_credential_commitment,
        x.credit_commitment,
        x.recipient_one_time_key,
        s.credit_id,
        s.ciphertext_digest,
        x.clock_context.binding_digest()?,
        x.financial_control_original_sha256,
        a.challenge.nonce,
        x.clock_context.request_nonce,
        x.clock_context.signed_observations_original_digest,
        x.predecessor.state_commitment,
        x.predecessor.state_original_sha256,
        provider_root,
    ];
    let scalars = [
        1,
        x.amount,
        u128::from(rt.scale),
        u128::from(x.policy_epoch),
        x.predecessor.logical_sequence,
        u128::from(x.clock_context.lower_at_ms),
        u128::from(x.clock_context.upper_at_ms),
        u128::from(a.challenge.issued_at_ms),
        u128::from(a.challenge.expires_at_ms),
        u128::from(c.subject.app_attest_counter_floor),
        u128::from(apple),
    ];
    Ok(OrdinaryMintPublicDataV1 { digests, scalars })
}

/// Convert the independently reconstructed semantic contract and exact canonical empty history.
/// This data helper alone does not verify a proof or authenticate any source.
pub(crate) fn ordinary_mint_public_column_v1<F: KagemushaPoseidonFieldV1>(
    data: &OrdinaryMintPublicDataV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<F> {
    let mut public = data
        .digests
        .into_iter()
        .flat_map(digest_limbs::<F>)
        .collect::<Vec<_>>();
    public.extend(data.scalars.into_iter().map(from_u128::<F>));
    public.extend(
        history
            .chunks_exact(16)
            .map(|b| from_u128::<F>(u128::from_le_bytes(b.try_into().expect("history16")))),
    );
    public
}
