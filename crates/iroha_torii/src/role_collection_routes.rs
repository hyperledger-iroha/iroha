//! Authenticated bounded role reads, including authorization at the authoritative peer.
use super::*;
use routing::collection_sources::CollectionTarget;
use std::net::SocketAddr;

const ROLES: &str = "/v1/roles";
const ACCOUNT_ROLES: &str = "/v1/accounts/{account_id}/roles";

pub(super) fn mount(builder: &mut RouterBuilder, app: SharedAppState, max_body_bytes: usize) {
    use route_catalog::application_api as routes;
    builder.route(
        &routes::ROLES_GET,
        catalog_get(roles_get)
            .authenticated_in_handler(HandlerAuthentication::OptionalCanonicalAccountSignature),
    );
    builder.route(
        &routes::ROLES_QUERY_POST,
        catalog_post(roles_query)
            .optionally_authenticated_canonical_account_body(app.clone(), max_body_bytes),
    );
    builder.route(
        &routes::ACCOUNTS_BY_ACCOUNT_ID_ROLES_GET,
        catalog_get(account_roles_get)
            .authenticated_in_handler(HandlerAuthentication::OptionalCanonicalAccountSignature),
    );
    builder.route(
        &routes::ACCOUNTS_BY_ACCOUNT_ID_ROLES_QUERY_POST,
        catalog_post(account_roles_query)
            .optionally_authenticated_canonical_account_body(app, max_body_bytes),
    );
}

fn signed_caller(caller: &ToriiAccountReadVisibility) -> Result<&AccountId, Error> {
    caller.caller().ok_or_else(|| Error::AppUnauthorized {
        code: "canonical_authentication_required",
        message: "canonical account request authentication is required for role reads".into(),
    })
}

fn require_account_read(
    app: &AppState,
    caller: &AccountId,
    target: &AccountId,
) -> Result<(), Error> {
    let view = app.state.view();
    let exact: Permission = iroha_executor_data_model::permission::query::CanReadAccountData {
        account: target.clone(),
    }
    .into();
    let global: Permission = CanReadAllLedgerData.into();
    if caller == target
        || torii_account_has_permission(view.world(), caller, &exact)
        || torii_account_has_permission(view.world(), caller, &global)
    {
        Ok(())
    } else {
        Err(Error::AppForbidden {
            code: "role_read_denied",
            message: "the signed caller lacks exact-account or whole-ledger read permission".into(),
        })
    }
}

async fn rate_limit(
    app: &SharedAppState,
    headers: &HeaderMap,
    remote: SocketAddr,
    endpoint: &str,
) -> Result<(), Error> {
    if !limits::is_allowed_by_cidr(headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets) {
        check_access_enforced(app, headers, Some(remote.ip()), endpoint, true).await?;
    }
    Ok(())
}

async fn roles_get(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<SocketAddr>,
) -> Result<Response, Error> {
    let caller = torii_one_shot_caller_from_headers(&app, &headers, &method, &uri, ROLES).await?;
    rate_limit(&app, &headers, remote, ROLES).await?;
    read_roles(&app, &caller, list_query_from_query_string(uri.query())?).await
}
async fn roles_query(
    State(app): State<SharedAppState>,
    Extension(caller): Extension<ToriiAccountReadVisibility>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<SocketAddr>,
    crate::JsonOnly(body): crate::JsonOnly<norito::json::Value>,
) -> Result<Response, Error> {
    rate_limit(&app, &headers, remote, ROLES).await?;
    read_roles(&app, &caller, list_query_from_json(body)?).await
}
async fn read_roles(
    app: &SharedAppState,
    caller: &ToriiAccountReadVisibility,
    query: ListQuery,
) -> Result<Response, Error> {
    torii_global_reader_visibility(app, caller, ROLES)?;
    forward_visible_collection_read(
        app,
        caller.caller(),
        ToriiReadEndpointV1::RolesQuery,
        CollectionTarget::Roles,
        Vec::new(),
        query,
    )
    .await
}
async fn account_roles_get(
    State(app): State<SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<SocketAddr>,
    AxPath(account): AxPath<String>,
) -> Result<Response, Error> {
    let caller =
        torii_one_shot_caller_from_headers(&app, &headers, &method, &uri, ACCOUNT_ROLES).await?;
    rate_limit(&app, &headers, remote, ACCOUNT_ROLES).await?;
    read_account_roles(
        &app,
        &caller,
        account,
        list_query_from_query_string(uri.query())?,
    )
    .await
}
async fn account_roles_query(
    State(app): State<SharedAppState>,
    Extension(caller): Extension<ToriiAccountReadVisibility>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<SocketAddr>,
    AxPath(account): AxPath<String>,
    crate::JsonOnly(body): crate::JsonOnly<norito::json::Value>,
) -> Result<Response, Error> {
    rate_limit(&app, &headers, remote, ACCOUNT_ROLES).await?;
    read_account_roles(&app, &caller, account, list_query_from_json(body)?).await
}
async fn read_account_roles(
    app: &SharedAppState,
    caller: &ToriiAccountReadVisibility,
    literal: String,
    query: ListQuery,
) -> Result<Response, Error> {
    let caller = signed_caller(caller)?;
    let (account, canonical) = routing::parse_account_path_segment_with_state(
        &app.state,
        &literal,
        &app.telemetry_handle(),
        ACCOUNT_ROLES,
    )?;
    require_account_read(app, caller, &account)?;
    let (_, body, reservation) = prepare_collection_forward(
        app,
        &CollectionTarget::AccountRoles(canonical.clone()),
        query,
    )?;
    let routes = match torii_target_account_routes(app, &account) {
        Ok(routes) => routes,
        Err(response) => return Ok(response),
    };
    let Some(route) = collection_execution_route(app, &routes) else {
        return Ok(torii_proxy_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "route_unavailable",
            "account-role read has no authoritative target route",
        ));
    };
    let scope = ToriiFanoutRouteScopeV1::TargetAccount {
        account_id: canonical.clone(),
        caller_account_id: Some(caller.to_string()),
    };
    Ok(execute_collection_on_route(
        app,
        route,
        scope,
        ToriiReadEndpointV1::AccountRolesQuery,
        vec![canonical],
        body,
        reservation,
    )
    .await)
}

/// Recheck current grants on the authoritative peer; ingress cannot supply a broader scope.
pub(super) fn authorize_routed(
    app: &SharedAppState,
    request: &ToriiReadProxyRequestV1,
    route: RoutingDecision,
    target: &CollectionTarget,
    visibility: routing::DataspaceReadVisibility,
) -> Result<routing::DataspaceReadVisibility, Error> {
    if !matches!(
        target,
        CollectionTarget::Roles | CollectionTarget::AccountRoles(_)
    ) {
        return Ok(visibility);
    }
    let (scope_account, caller) = match &request.route_scope {
        ToriiFanoutRouteScopeV1::VisibleAccount {
            caller_account_id: Some(caller),
        } => (None, caller),
        ToriiFanoutRouteScopeV1::TargetAccount {
            account_id,
            caller_account_id: Some(caller),
        } => (Some(account_id), caller),
        _ => {
            return Err(Error::AppForbidden {
                code: "role_read_denied",
                message: "role reads require an authenticated caller scope".into(),
            });
        }
    };
    let caller = AccountId::parse_encoded(caller)
        .map_err(|error| routing::conversion_error(error.to_string()))?;
    match target {
        CollectionTarget::Roles => {
            torii_global_reader_visibility(app, &ToriiAccountReadVisibility::Signed(caller), ROLES)
        }
        CollectionTarget::AccountRoles(literal) => {
            if scope_account != Some(literal) {
                return Err(Error::AppForbidden {
                    code: "role_read_denied",
                    message: "account-role path and exact scope must match".into(),
                });
            }
            let account = AccountId::parse_encoded(literal)
                .map_err(|error| routing::conversion_error(error.to_string()))?;
            require_account_read(app, &caller, &account)?;
            Ok(visibility.with_exact_account(BTreeSet::from([route.dataspace_id]), account))
        }
        _ => unreachable!("non-role targets returned above"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::state::World;
    use iroha_data_model::{Account, Identifiable, Registrable, role::Role};

    #[tokio::test]
    async fn role_authorization_requires_live_exact_or_global_grants() {
        let account = |seed| {
            AccountId::new(
                iroha_crypto::KeyPair::try_from_seed(
                    vec![seed; 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .expect("role read authorization key")
                .public_key()
                .clone(),
            )
        };
        let owner = account(0xD1);
        let delegate = account(0xD2);
        let global = account(0xD3);
        let stranger = account(0xD4);
        let exact_role = Role::new("exact_reader".parse().unwrap(), delegate.clone())
            .add_permission(
                iroha_executor_data_model::permission::query::CanReadAccountData {
                    account: owner.clone(),
                },
            )
            .build(&owner);
        let global_role = Role::new("global_reader".parse().unwrap(), global.clone())
            .add_permission(CanReadAllLedgerData)
            .build(&owner);
        let mut world = World::with_assets_and_roles(
            [],
            [&owner, &delegate, &global, &stranger]
                .into_iter()
                .map(|id| Account::new(id.clone()).build(&owner)),
            [],
            [],
            [],
            [exact_role.clone(), global_role.clone()],
        );
        world.grant_role_for_tests(delegate.clone(), exact_role.id().clone());
        world.grant_role_for_tests(global.clone(), global_role.id().clone());
        let app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(world);
        assert!(signed_caller(&ToriiAccountReadVisibility::None).is_err());
        for caller in [&owner, &delegate, &global] {
            require_account_read(&app, caller, &owner).unwrap();
        }
        assert!(require_account_read(&app, &stranger, &owner).is_err());
        assert!(require_account_read(&app, &delegate, &stranger).is_err());
        assert!(
            torii_global_reader_visibility(
                &app,
                &ToriiAccountReadVisibility::Signed(delegate.clone()),
                ROLES
            )
            .is_err()
        );
        torii_global_reader_visibility(&app, &ToriiAccountReadVisibility::Signed(global), ROLES)
            .unwrap();
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let mut request = torii_read_request(
            ToriiReadEndpointV1::AccountRolesQuery,
            ToriiFanoutRouteScopeV1::TargetAccount {
                account_id: owner.to_string(),
                caller_account_id: Some(delegate.to_string()),
            },
            route,
            vec![owner.to_string()],
            None,
            Vec::new(),
        );
        let target = CollectionTarget::AccountRoles(owner.to_string());
        let visibility = routing::DataspaceReadVisibility::new(BTreeSet::new(), false);
        assert_eq!(
            authorize_routed(&app, &request, route, &target, visibility.clone())
                .unwrap()
                .exact_account_id(),
            Some(&owner)
        );
        request.route_scope = ToriiFanoutRouteScopeV1::TargetAccount {
            account_id: stranger.to_string(),
            caller_account_id: Some(delegate.to_string()),
        };
        assert!(authorize_routed(&app, &request, route, &target, visibility.clone()).is_err());
        request.route_scope = ToriiFanoutRouteScopeV1::AllDataspaces;
        assert!(
            authorize_routed(
                &app,
                &request,
                route,
                &target,
                routing::DataspaceReadVisibility::all()
            )
            .is_err()
        );
        request.route_scope = ToriiFanoutRouteScopeV1::TargetAccount {
            account_id: owner.to_string(),
            caller_account_id: Some(stranger.to_string()),
        };
        assert!(authorize_routed(&app, &request, route, &target, visibility).is_err());
    }
}
