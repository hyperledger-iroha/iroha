// Exact-network account authentication shared by expensive ZK tooling routes.
#[cfg(feature = "app_api")]
fn require_zk_ivm_derive_authority(
    request_authority: &iroha_data_model::account::AccountId,
    verified: &crate::app_auth::VerifiedCanonicalRequest,
) -> Result<(), Error> {
    if request_authority == &verified.account {
        return Ok(());
    }
    Err(Error::Query(
        iroha_data_model::ValidationFail::NotPermitted(
            "authenticated account must match the IVM derive request authority".to_owned(),
        ),
    ))
}
macro_rules! mount_authenticated_zk_compute_routes {
    ($builder:ident, $app_state:ident, $proof_body_limit:ident) => {
        #[cfg(feature = "zk-verify-batch")]
        $builder.route(
            &route_catalog::runtime_governance::ZK_VERIFY_BATCH,
            catalog_post(handler_zk_verify_batch)
                .authenticated_canonical_account_proof_body($app_state.clone(), $proof_body_limit),
        );
        #[cfg(feature = "app_api")]
        $builder.route(
            &route_catalog::runtime_governance::ZK_IVM_DERIVE,
            catalog_post(handler_zk_ivm_derive)
                .authenticated_canonical_account_proof_body($app_state.clone(), $proof_body_limit),
        );
    };
}
