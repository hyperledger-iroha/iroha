//! Closed admission of the distinct ordinary pre-debit MintAuthorization113 family.
//! This authenticates complete historical proof operands. It supplies no account debit, current
//! FI/effect clock, global pending-head reservation, finalized Mint source, or funded State.
use super::{
    DigestV1, KagemushaAuthenticatedRecursiveVerifierV1, KagemushaEpAccumulatorV1,
    KagemushaEqAccumulatorV1, decide_kagemusha_ep_accumulator_v1,
    decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    native_backend::{
        OrdinaryMintMaterialV1, verify_ep_succinct_protocol, verify_eq_succinct_protocol,
    },
    ordinary_mint_public::{ordinary_mint_public_column_v1, ordinary_mint_public_data_v1},
};
use crate::kagemusha_v1_state::KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};

/// Genuine both-parity proof admission under an actual independently installed ordinary release.
/// No clone, decoder, raw-instance-vector constructor, or hardware-token conversion exists.
/// Full account consent, current FI/clock, DATA predecessor exclusivity and finalized debit are
/// separately required by the Node/Core effect owner.
pub struct KagemushaVerifiedOrdinaryMintAuthorizationV1 {
    request: KagemushaOrdinaryTopUpRequestV1,
    request_original: Vec<u8>,
    request_original_sha256: DigestV1,
    authorization_original_digest: DigestV1,
    credential_original: Vec<u8>,
    selected_integrity_original: Option<Vec<u8>>,
    preparation_clock_original: Vec<u8>,
}
impl KagemushaVerifiedOrdinaryMintAuthorizationV1 {
    /// Exact complete dedicated pre-debit authorization which actually passed both proofs.
    #[must_use]
    pub fn authorization(&self) -> &KagemushaOrdinaryMintAuthorizationV1 {
        &self.request.authorization
    }
    /// Same complete unsigned request original; account consent and subsequent decision are separate.
    #[must_use]
    pub fn request_original(&self) -> &[u8] {
        &self.request_original
    }
    /// Raw SHA256 of that exact unsigned original, excluded from the later Core decision.
    #[must_use]
    pub const fn request_original_sha256(&self) -> DigestV1 {
        self.request_original_sha256
    }
    /// Sole model-owned complete authorization original identity, including both actual proofs.
    #[must_use]
    pub const fn authorization_original_digest(&self) -> DigestV1 {
        self.authorization_original_digest
    }
    /// Complete admitted C data; borrowing it cannot create another verified credential owner.
    #[must_use]
    pub fn credential_original(&self) -> &[u8] {
        &self.credential_original
    }
    /// Exact selected historical Integrity original, without current refresh authority.
    #[must_use]
    pub fn selected_integrity_original(&self) -> Option<&[u8]> {
        self.selected_integrity_original.as_deref()
    }
    /// Actual authenticated complete four-observation preparation original; no elapsed-clock grant.
    #[must_use]
    pub fn preparation_clock_original(&self) -> &[u8] {
        &self.preparation_clock_original
    }
}

/// Authenticate the complete sole Mint request against genuine independently held C/PI/clock.
/// The signed C enrollment counter minimum is public. Native's later journal floor remains a
/// separate privately loaned proving/capture operand and is never certified from this request.
///
/// # Errors
/// Refuses missing ordinary113 family, exact release/key/profile/original changes, invalid
/// platform equations, overdue historical Integrity, unsupported proof sizes, any changed
/// semantic public operand, noncanonical history, or either failed IPA proof/terminal decision.
pub fn verify_ordinary_mint_authorization_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    full_request_original: &[u8],
    actual_credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    actual_selected_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    actual_preparation_clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryMintAuthorizationV1, String> {
    let material = verifier.ordinary_mint_material()?;
    let release = verifier.monetary_release()?;
    let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(full_request_original)?;
    let authorization = &request.authorization;
    let context = &authorization.statement.context;
    context.validate_against_credential(actual_credential)?;
    let enabled = release
        .enabled_profile(context.app_credential_profile_id)
        .ok_or("ordinary Mint app profile is not independently release enabled")?;
    if !enabled.hardware_profile.platform_class.is_ordinary_app()
        || enabled.hardware_profile.platform_class != actual_credential.subject().platform_class
        || enabled.policy_epoch != context.policy_epoch
        || enabled.suite_id != context.suite_id
        || enabled.vk_digest != context.vk_digest
        || context.release_id != material.release_id
        || context.suite_id != material.suite_id
        || context.vk_digest != material.vk_set_digest
        || context.artifact_manifest_digest != material.artifact_manifest_digest
        || context.lineage.owner.runtime.network_id != release.network_id()
    {
        return Err("ordinary Mint release/profile/runtime originals differ".into());
    }
    actual_preparation_clock
        .recheck_cash_context(&context.clock_context)
        .map_err(|e| e.to_string())?;
    let challenge = &authorization.approval.challenge;
    if context.clock_context.lower_at_ms < challenge.issued_at_ms
        || context.clock_context.upper_at_ms >= challenge.expires_at_ms
        || challenge.expires_at_ms > actual_credential.subject().expires_at_ms
        || challenge.issued_at_ms < actual_credential.subject().issued_at_ms
    {
        return Err("ordinary Mint original approval/admission interval differs".into());
    }
    for now in [
        context.clock_context.lower_at_ms,
        context.clock_context.upper_at_ms,
    ] {
        match actual_selected_lease {
            Some(lease) => actual_credential.recheck_with_integrity_lease(lease, now)?,
            None => actual_credential.recheck_at_trusted_time(now)?,
        }
    }
    let minimum = match actual_credential.subject().platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => None,
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            Some(actual_credential.subject().app_attest_counter_floor)
        }
        _ => return Err("ordinary Mint credential class differs".into()),
    };
    authorization
        .approval
        .authenticate_platform_equation(actual_credential, minimum)?;
    // Strictly recover public mathematical data from these *same* authentic originals only.
    // This decoder does not create a credential/lease capability.
    let credential =
        KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(actual_credential.original())?;
    let lease = actual_selected_lease
        .map(|lease| {
            let value: KagemushaPlayIntegrityRefreshLeaseV1 = norito::decode_canonical_with_limits(
                lease.original(),
                norito::canonical_decode_limits(lease.original().len()),
            )
            .map_err(|e| e.to_string())?;
            if value.canonical_bytes()? != lease.original()
                || value.canonical_digest()? != lease.digest()
            {
                return Err("ordinary Mint selected Integrity original differs".to_owned());
            }
            Ok(value)
        })
        .transpose()?;
    let public = ordinary_mint_public_data_v1(
        &authorization.statement,
        &authorization.approval,
        &credential,
        lease.as_ref(),
        material.provider_policy_root,
    )?;
    verify_proof(authorization, &public, &material)?;
    let digest = authorization.binding_digest()?;
    Ok(KagemushaVerifiedOrdinaryMintAuthorizationV1 {
        request,
        request_original: full_request_original.to_vec(),
        request_original_sha256: Sha256::digest(full_request_original).into(),
        authorization_original_digest: digest,
        credential_original: actual_credential.original().to_vec(),
        selected_integrity_original: actual_selected_lease.map(|l| l.original().to_vec()),
        preparation_clock_original: actual_preparation_clock.original().to_vec(),
    })
}
fn verify_proof(
    authorization: &KagemushaOrdinaryMintAuthorizationV1,
    public: &super::ordinary_mint_public::OrdinaryMintPublicDataV1,
    m: &OrdinaryMintMaterialV1<'_>,
) -> Result<(), String> {
    let p = &authorization.proof;
    let eq_len = ordinary_ipa_proof_profile_v1(m.eq_protocol)?.byte_len;
    let ep_len = ordinary_ipa_proof_profile_v1(m.ep_protocol)?.byte_len;
    if p.eq_protocol_digest != m.eq_protocol_digest
        || p.ep_protocol_digest != m.ep_protocol_digest
        || p.eq_proof.len() != eq_len
        || p.ep_proof.len() != ep_len
        || eq_len == 0
        || ep_len == 0
        || eq_len
            .checked_add(ep_len)
            .is_none_or(|n| n > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1)
    {
        return Err("ordinary Mint exact protocol/proof lengths differ".into());
    }
    let eq_history =
        initial_kagemusha_eq_accumulator_v1(m.eq_parameters).map_err(|e| e.to_string())?;
    let ep_history =
        initial_kagemusha_ep_accumulator_v1(m.ep_parameters).map_err(|e| e.to_string())?;
    if p.eq_history.as_slice() != eq_history.as_bytes()
        || p.ep_history.as_slice() != ep_history.as_bytes()
    {
        return Err(
            "ordinary Mint leaf history is not the exact canonical initial accumulator".into(),
        );
    }
    let e = ordinary_mint_public_column_v1::<Fp>(public, eq_history.as_bytes());
    let pcol = ordinary_mint_public_column_v1::<Fq>(public, ep_history.as_bytes());
    let eq = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
        m.eq_parameters,
        m.eq_protocol,
        &p.eq_proof,
        &e,
    )?)
    .map_err(|e| e.to_string())?;
    let ep = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
        m.ep_parameters,
        m.ep_protocol,
        &p.ep_proof,
        &pcol,
    )?)
    .map_err(|e| e.to_string())?;
    decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eq).map_err(|e| e.to_string())?;
    decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &ep).map_err(|e| e.to_string())?;
    decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eq_history).map_err(|e| e.to_string())?;
    decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &ep_history).map_err(|e| e.to_string())
}
