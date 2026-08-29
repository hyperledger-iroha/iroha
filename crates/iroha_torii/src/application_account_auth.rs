// Exact principal binding for account-authenticated application drafts and receipts.
#[cfg(feature = "app_api")]
macro_rules! authenticated_application_query {
    ($handler:expr, $app_state:expr, $max_body_bytes:expr) => {
        catalog_post($handler).authenticated_canonical_account_body($app_state, $max_body_bytes)
    };
}
macro_rules! define_authenticated_application_query_mount {
    ($name:ident, $route:ident, $handler:ident) => {
        #[cfg(feature = "app_api")]
        fn $name(builder: &mut RouterBuilder, app_state: SharedAppState, max_body_bytes: usize) {
            builder.route(
                &route_catalog::application_api::$route,
                authenticated_application_query!($handler, app_state, max_body_bytes),
            );
        }
    };
}
define_authenticated_application_query_mount!(
    mount_account_transactions_query,
    ACCOUNTS_BY_ACCOUNT_ID_TRANSACTIONS_QUERY_POST,
    handler_account_transactions_query
);
define_authenticated_application_query_mount!(
    mount_account_assets_query,
    ACCOUNTS_BY_ACCOUNT_ID_ASSETS_QUERY_POST,
    handler_account_assets_query
);
define_authenticated_application_query_mount!(
    mount_domains_query,
    DOMAINS_QUERY_POST,
    handler_domains_query
);
define_authenticated_application_query_mount!(
    mount_accounts_query,
    ACCOUNTS_QUERY_POST,
    handler_accounts_query
);
define_authenticated_application_query_mount!(
    mount_transactions_query,
    TRANSACTIONS_QUERY_POST,
    handler_transactions_query
);
define_authenticated_application_query_mount!(
    mount_visible_transactions_query,
    TRANSACTIONS_VISIBLE_QUERY_POST,
    handler_transactions_visible_query
);
define_authenticated_application_query_mount!(
    mount_repo_agreements_query,
    REPO_AGREEMENTS_QUERY_POST,
    handler_repo_agreements_query
);
define_authenticated_application_query_mount!(
    mount_asset_definitions_query,
    ASSETS_DEFINITIONS_QUERY_POST,
    handler_assets_definitions_query
);
define_authenticated_application_query_mount!(
    mount_nfts_query,
    NFTS_QUERY_POST,
    handler_nfts_query
);
define_authenticated_application_query_mount!(
    mount_rwas_query,
    RWAS_QUERY_POST,
    handler_rwas_query
);
#[cfg(feature = "app_api")]
fn mount_signed_proof_query(builder: &mut RouterBuilder) {
    builder.route(
        &route_catalog::application_api::PROOFS_QUERY_POST,
        catalog_post(handler_proofs_query)
            .authenticated_in_handler(HandlerAuthentication::CanonicalSignedBody),
    );
}
#[cfg(feature = "app_api")]
macro_rules! mount_authenticated_asset_holder_routes {
    ($torii:expr, $builder:expr) => {{
        let max_body_bytes = usize::try_from($torii.transaction_max_content_len.get())
            .expect("transaction content limit should fit usize");
        $builder.route(
            &route_catalog::telemetry::ASSET_HOLDERS,
            catalog_get(handler_asset_holders)
                .authenticated_in_handler(HandlerAuthentication::OptionalCanonicalAccountSignature),
        );
        $builder.route(
            &route_catalog::telemetry::ASSET_HOLDERS_QUERY,
            authenticated_application_query!(
                handler_asset_holders_query,
                $builder.state().clone(),
                max_body_bytes
            ),
        );
    }};
}
#[cfg(feature = "app_api")]
fn add_authenticated_application_compute_routes(
    builder: &mut RouterBuilder,
    app_state: SharedAppState,
    max_body_bytes: usize,
) {
    builder.route(
        &route_catalog::application_api::SPACE_DIRECTORY_MANIFESTS_POST,
        catalog_post(handler_authenticated_space_directory_manifest_publish)
            .authenticated_canonical_account_body(app_state.clone(), max_body_bytes),
    );
    builder.route(
        &route_catalog::application_api::SPACE_DIRECTORY_MANIFESTS_REVOKE_POST,
        catalog_post(handler_authenticated_space_directory_manifest_revoke)
            .authenticated_canonical_account_body(app_state.clone(), max_body_bytes),
    );
    builder.route(
        &route_catalog::application_api::RAM_LFE_PROGRAMS_BY_PROGRAM_ID_EXECUTE_POST,
        catalog_post(handler_ram_lfe_execute)
            .authenticated_canonical_account_body(app_state.clone(), max_body_bytes),
    );
    builder.route(
        &route_catalog::application_api::RAM_LFE_RECEIPTS_VERIFY_POST,
        catalog_post(handler_ram_lfe_receipt_verify)
            .authenticated_canonical_account_body(app_state.clone(), max_body_bytes),
    );
    builder.route(
        &route_catalog::application_api::ACCOUNTS_BY_ACCOUNT_ID_IDENTIFIERS_CLAIM_RECEIPT_POST,
        catalog_post(handler_authenticated_identifier_claim_receipt)
            .authenticated_canonical_account_body(app_state.clone(), max_body_bytes),
    );
    builder.route(
        &route_catalog::application_api::IDENTIFIERS_RESOLVE_POST,
        catalog_post(handler_identifier_resolve)
            .authenticated_canonical_account_body(app_state, max_body_bytes),
    );
}
#[cfg(feature = "app_api")]
async fn handler_authenticated_space_directory_manifest_publish(
    State(app): State<SharedAppState>,
    axum::extract::Extension(verified): axum::extract::Extension<
        crate::app_auth::VerifiedCanonicalRequest,
    >,
    headers: axum::http::HeaderMap,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: crate::utils::extractors::NoritoJson<crate::routing::SpaceDirectoryManifestPublishDto>,
) -> Result<impl IntoResponse, Error> {
    require_runtime_governance_account(
        &request.0.authority,
        &verified.account,
        "space-directory manifest publication draft",
    )?;
    handler_space_directory_manifest_publish(State(app), headers, remote, request).await
}
#[cfg(feature = "app_api")]
async fn handler_authenticated_space_directory_manifest_revoke(
    State(app): State<SharedAppState>,
    axum::extract::Extension(verified): axum::extract::Extension<
        crate::app_auth::VerifiedCanonicalRequest,
    >,
    headers: axum::http::HeaderMap,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: crate::utils::extractors::NoritoJson<crate::routing::SpaceDirectoryManifestRevokeDto>,
) -> Result<impl IntoResponse, Error> {
    require_runtime_governance_account(
        &request.0.authority,
        &verified.account,
        "space-directory manifest revocation draft",
    )?;
    handler_space_directory_manifest_revoke(State(app), headers, remote, request).await
}
#[cfg(feature = "app_api")]
async fn handler_authenticated_identifier_claim_receipt(
    State(app): State<SharedAppState>,
    axum::extract::Extension(verified): axum::extract::Extension<
        crate::app_auth::VerifiedCanonicalRequest,
    >,
    headers: axum::http::HeaderMap,
    remote: axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxPath(account_literal): AxPath<String>,
    request: NoritoJson<routing::IdentifierResolveRequestDto>,
) -> Result<AxResponse, Error> {
    let account_id = parse_account_id_for_endpoint(
        &app,
        &account_literal,
        "/v1/accounts/{account_id}/identifiers/claim-receipt",
    )?;
    require_runtime_governance_account(&account_id, &verified.account, "identifier claim receipt")?;
    handler_identifier_claim_receipt(
        State(app),
        headers,
        remote,
        AxPath(account_literal),
        request,
    )
    .await
}
#[cfg(all(test, feature = "app_api"))]
mod application_account_auth_tests {
    use super::{Error, require_runtime_governance_account};
    use iroha_data_model::ValidationFail;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    #[test]
    fn application_authority_binding_rejects_substitution() {
        require_runtime_governance_account(
            &ALICE_ID,
            &ALICE_ID,
            "space-directory manifest publication draft",
        )
        .expect("the exact authenticated authority must be accepted");
        let error =
            require_runtime_governance_account(&BOB_ID, &ALICE_ID, "identifier claim receipt")
                .expect_err("another authority must be rejected");
        assert!(matches!(
            error,
            Error::Query(ValidationFail::NotPermitted(message))
                if message.contains("identifier claim receipt authority")
        ));
    }
}
