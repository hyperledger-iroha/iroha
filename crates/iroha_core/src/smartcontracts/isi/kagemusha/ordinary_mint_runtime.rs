//! Distinct actual ordinary Mint113 dispatch under the installed governed release family.
//! The host cannot promote an OEM authorization or decoded C/clock original into this proof.
use super::*;
use iroha_core_zk::{
    kagemusha_v1_recursion::{
        KagemushaMintAuthorizationFamilyV1, KagemushaVerifiedOrdinaryMintAuthorizationV1,
        verify_ordinary_mint_authorization_v1,
    },
    kagemusha_v1_state::KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryTopUpRequestV1, KagemushaVerifiedOrdinaryAppCredentialV1,
    KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
};

pub(super) fn authorize_selected_monetary_family(
    verifier: &mut KagemushaAuthenticatedRecursiveVerifierV1,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    family: KagemushaMintAuthorizationFamilyV1,
) -> Result<(), String> {
    match family {
        KagemushaMintAuthorizationFamilyV1::OrdinaryPreDebit113 => {
            verifier.authorize_ordinary_monetary_release(release)
        }
        KagemushaMintAuthorizationFamilyV1::RecursiveHardware84 => {
            verifier.authorize_monetary_release(release)
        }
    }
    .map_err(|e| format!("failed to authorize the selected KAGEMUSHA monetary family: {e}"))
}

pub(super) fn verify(
    owner: &AuthenticatedKagemushaV1RuntimeVerifier,
    raw: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryMintAuthorizationV1, String> {
    let runtime = selected_runtime(owner, raw)?;
    // The maintained verifier reconstructs all113 semantics from these same authentic full
    // originals and decides both current IPA proofs and complete carried histories.
    verify_ordinary_mint_authorization_v1(&runtime.verifier, raw, credential, lease, clock)
}

pub(super) fn selected_runtime<'a>(
    owner: &'a AuthenticatedKagemushaV1RuntimeVerifier,
    raw: &[u8],
) -> Result<&'a AuthenticatedKagemushaV1ReleaseRuntime, String> {
    let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(raw)?;
    let context = &request.authorization.statement.context;
    let runtime = owner.runtime_for_new_top_up(context.release_id)?;
    require_production_release_purpose_v1(runtime.purpose)?;
    if context.lineage.owner.runtime.network_id != runtime.network_id
        || runtime.release_id != context.release_id
    {
        return Err("ordinary Mint selected release/network differs".into());
    }
    Ok(runtime)
}
// Retained historical request admission is distinct from the enabled-new-topup selector.
// A retired release may authenticate its immutable committed source; no new debit is lent.
pub(super) fn verify_retained(
    owner: &AuthenticatedKagemushaV1RuntimeVerifier,
    raw: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryMintAuthorizationV1, String> {
    let runtime = selected_retained_runtime(owner, raw)?;
    verify_ordinary_mint_authorization_v1(&runtime.verifier, raw, credential, lease, clock)
}
pub(super) fn selected_retained_runtime<'a>(
    owner: &'a AuthenticatedKagemushaV1RuntimeVerifier,
    raw: &[u8],
) -> Result<&'a AuthenticatedKagemushaV1ReleaseRuntime, String> {
    let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(raw)?;
    let context = &request.authorization.statement.context;
    let runtime = owner.runtime_for_terminal_verification(context.release_id)?;
    require_production_release_purpose_v1(runtime.purpose)?;
    if context.lineage.owner.runtime.network_id != runtime.network_id
        || runtime.release_id != context.release_id
    {
        return Err("retained ordinary Mint selected release/network differs".into());
    }
    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_public_request_cannot_install_its_own_ordinary_release() {
        let fixture =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let owner = AuthenticatedKagemushaV1RuntimeVerifier {
            releases: BTreeMap::new(),
            lifecycle: KagemushaVerifierReleaseLifecycleV1::default(),
        };
        let raw = fixture.request.canonical_bytes().unwrap();
        // Genuine public signatures, explicitly inert proofs: no accepting cap is constructed.
        assert!(selected_runtime(&owner, &raw).is_err());
        assert!(selected_runtime(&owner, &[]).is_err());
        let mut trailing = raw;
        trailing.push(0);
        assert!(selected_runtime(&owner, &trailing).is_err());
    }
}
