//! Exact table/key lookups at the current certified Global State root.
use crate::*;
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::AccountId,
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_JOURNAL_BYTES, NativeFinalityLimits,
    },
    sumeragi_finality::{
        MAX_STATE_PROOF_KEY_BYTES_V1, MAX_STATE_PROOF_ROWS_V1, MAX_STATE_ROWS_FRAME_BYTES_V1,
    },
};
use iroha_executor_data_model::permission::query::CanReadStateRow;

pub(super) const MAX_REQUEST_BYTES: usize = 64 * 1024;
const ROUTE: &str = iroha_torii_shared::route_catalog::core::LEDGER_STATE_ROWS_POST.path();

fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "state_rows_unavailable",
        message: "The selected certified State rows are unavailable.".into(),
    }
}

fn height_selector(value: &str) -> Result<u64, Error> {
    value
        .parse::<u64>()
        .ok()
        .filter(|h| *h >= 2 && h.to_string() == value)
        .ok_or_else(|| {
            conversion_error("State row proof requires canonical non-genesis height".into())
        })
}

#[derive(norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Selection {
    rows: Vec<CanReadStateRow>,
}
fn selectors(body: &[u8]) -> Result<Vec<CanReadStateRow>, Error> {
    let invalid =
        || conversion_error("expected exact State rows in ascending (table, key) order".into());
    let limits = norito::DecodeLimits::new(
        MAX_STATE_PROOF_ROWS_V1,
        MAX_STATE_PROOF_KEY_BYTES_V1 * 2,
        MAX_STATE_PROOF_ROWS_V1 * 3 + 1,
        4 * MAX_REQUEST_BYTES,
        4,
    );
    norito::json::preflight_slice(
        body,
        norito::json::JsonPreflightLimits::from_decode_limits(MAX_REQUEST_BYTES, limits),
    )
    .map_err(|e| {
        if e.resource_kind().is_some() {
            crate::native_projection_response::capacity()
        } else {
            invalid()
        }
    })?;
    let selection: Selection =
        norito::core::with_decode_limits_scope(limits, || norito::json::from_slice(body))
            .map_err(|_| invalid())?;
    if selection.rows.is_empty()
        || selection.rows.len() > MAX_STATE_PROOF_ROWS_V1
        || !selection.rows.iter().all(CanReadStateRow::is_valid)
        || selection
            .rows
            .windows(2)
            .any(|pair| (&pair[0].table, &pair[0].key_hex) >= (&pair[1].table, &pair[1].key_hex))
    {
        return Err(invalid());
    }
    Ok(selection.rows)
}

fn require_permission(
    app: &SharedAppState,
    account: &AccountId,
    grants: &[Permission],
) -> Result<(), Error> {
    let owner = crate::history_producer::HistoryProducerOwner::authentication_read(app)?;
    let forbidden = || {
        Error::Query(iroha_data_model::ValidationFail::NotPermitted(
            "Current read authority for every selected State row is required".into(),
        ))
    };
    owner.scope(|| {
        app.state
            .try_with_authorization_view(owner.allocation_context(), |view| {
                use iroha_core::state::{StorageReadOnly as _, WorldReadOnly as _};
                let world = view.world();
                if world.accounts().get(account).is_none() {
                    return Err(forbidden());
                }
                if !crate::stream_control::can_read_all(world, account)? {
                    if !iroha_core::runtime::state_row_permission::active(world.runtime_selection())
                        || !grants
                            .iter()
                            .all(|p| world.account_contains_inherent_permission(account, p))
                    {
                        return Err(forbidden());
                    }
                }
                Ok(())
            })
            .map_err(crate::app_auth::authorization_view_error)?
    })
}

/// Serve a signed batch of exact row reads from one certified Global execution.
///
/// # Errors
/// Rejects malformed selections, invalid signatures, insufficient grants, unavailable
/// certified rows, and admission or egress refusals.
pub async fn handler(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<String>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let height = height_selector(&height)?;
    if method != Method::POST
        || uri.query().is_some()
        || uri.path() != ROUTE.replace("{height}", &height.to_string())
    {
        return Err(conversion_error(
            "State row proof requires exact signed request target".into(),
        ));
    }
    let auth = crate::history_producer::HistoryProducerOwner::authentication_read(&app)?;
    let verified = auth
        .scope(|| {
            crate::app_auth::verify_canonical_network_request(
                &app.state,
                app.state.network_id_ref(),
                &headers,
                crate::app_auth::CanonicalRequestParts {
                    method: &method,
                    uri: &uri,
                    body: &body,
                },
                None,
                auth.allocation_context(),
            )
        })
        .map_err(crate::app_auth::canonical_authentication_error)?
        .ok_or_else(|| Error::AppUnauthorized {
            code: "state_rows_authentication_required",
            message: "canonical signed account authentication is required".into(),
        })?;
    let principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let rate_key = rate_limit_key(&headers, Some(remote.ip()), ROUTE, principal);
    rate_limit_requests_with_cost(&app, &rate_key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let bytes = app.query_fanout_working_set_bytes;
    let capacity = crate::native_projection_response::capacity;
    let memory = crate::acquire_query_pool_credit(&app, bytes)
        .await
        .ok_or_else(capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    let budget = AllocationBudget::new(bytes);
    let body_charge = budget
        .try_reserve_bytes(body.len())
        .map_err(|_| capacity())?;

    // The HTTP future retains the selected permissions through its final revocation check.
    let _selection_charge = budget
        .try_reserve_bytes(
            4 * body.len() + MAX_STATE_PROOF_ROWS_V1 * std::mem::size_of::<CanReadStateRow>(),
        )
        .map_err(|_| capacity())?;
    let selectors = selectors(&body)?;
    drop(body);
    drop(body_charge);
    let grants: Vec<_> = selectors
        .iter()
        .map(crate::torii_permission_target)
        .collect::<Result<_, _>>()?;
    require_permission(&app, &verified.account, &grants)?;
    // A detached blocking worker keeps its lookup keys funded even if the caller disconnects.
    let key_bytes = selectors
        .iter()
        .map(|row| {
            std::mem::size_of::<(String, Vec<u8>)>() + row.table.len() + row.key_hex.len() / 2
        })
        .sum();
    let key_charge = budget
        .try_reserve_bytes(key_bytes)
        .map_err(|_| capacity())?;
    let keys: Vec<_> = selectors
        .iter()
        .map(|r| {
            hex::decode(&r.key_hex)
                .map(|k| (r.table.clone(), k))
                .map_err(|_| conversion_error("invalid State key".into()))
        })
        .collect::<Result<_, _>>()?;
    let state = app.state.clone();
    let maximum = app
        .torii_proxy_max_response_bytes
        .min(MAX_STATE_ROWS_FRAME_BYTES_V1);
    let response =
        routing::run_admitted_blocking(admission, "State row proof worker failed", move || {
            let _keys_charge = key_charge;
            let unit = bytes / 8;
            let limits = NativeFinalityLimits {
                block_bytes: unit.min(NATIVE_FINALITY_MAX_BLOCK_BYTES),
                journal_bytes: (unit * 2).min(NATIVE_FINALITY_MAX_JOURNAL_BYTES),
                block_count: 8,
                allocated_bytes: unit * 4,
            };
            let decode_limits = limits.decode_limits().map_err(|_| capacity())?;
            let _source = budget
                .try_reserve_bytes(limits.journal_bytes + limits.allocated_bytes)
                .map_err(|_| capacity())?;
            let view = state
                .try_view_once()
                .map_err(crate::app_auth::authorization_view_error)?;
            let tip = norito::core::with_decode_limits_scope(decode_limits, || {
                crate::native_projection_response::current_global_tip(
                    &view,
                    height,
                    limits,
                    unavailable,
                )
            })?;
            drop(view);

            let proof = state
                .keyed_state_rows(&tip, &keys, &budget)
                .map_err(|_| unavailable())?;
            let mut body =
                crate::native_projection_response::encode(&*proof, format, maximum, &budget)?;
            body.memory = Some(memory);
            let mut response = AxResponse::new(Body::from(Bytes::from_owner(body)));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static(match format {
                    ResponseFormat::Norito => "application/x-norito",
                    ResponseFormat::Json => "application/json",
                }),
            );
            Ok(response)
        })
        .await?;
    require_permission(&app, &verified.account, &grants)?;
    let mut response = proof_response_with_exact_egress_bounded(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
        maximum,
    )
    .await?;
    require_permission(&app, &verified.account, &grants)?;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_admits_only_bounded_ordered_unique_exact_keys() {
        let selected = selectors(br#"{"rows":[{"table":"state.runtime","key_hex":""},{"table":"world.assets","key_hex":"00ab"}]}"#).unwrap();
        assert_eq!(selected.len(), 2);
        for invalid in [
            br#"{"rows":[]}"#.as_slice(),
            br#"{"rows":[],"rows":[]}"#,
            br#"{"rows":[{"table":"world.assets","key_hex":"A0"}]}"#,
            br#"{"rows":[{"table":"world.assets","key_hex":"0"}]}"#,
            br#"{"rows":[{"table":"world.assets","key_hex":"00","extra":0}]}"#,
            br#"{"rows":[{"table":"world.a","key_hex":"01"},{"table":"world.a","key_hex":"01"}]}"#,
            br#"{"rows":[{"table":"world.b","key_hex":"01"},{"table":"world.a","key_hex":"00"}]}"#,
        ] {
            assert!(selectors(invalid).is_err());
        }
        let overflow = format!(
            r#"{{"rows":[{}]}}"#,
            (0..=MAX_STATE_PROOF_ROWS_V1)
                .map(|n| format!(r#"{{"table":"world.assets","key_hex":"{n:02x}"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(selectors(overflow.as_bytes()).is_err());
        assert_eq!(height_selector("2").unwrap(), 2);
        for bad in ["0", "1", "02", "+2", ""] {
            assert!(height_selector(bad).is_err());
        }
    }

    #[test]
    fn a_direct_grant_for_every_selected_row_is_required_and_revocable() {
        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        use iroha_data_model::{
            Account, Registrable,
            isi::{Grant, Revoke},
        };
        let issuer = iroha_test_samples::ALICE_ID.clone();
        let owner = iroha_test_samples::BOB_ID.clone();
        let mut world = World::with(
            [],
            [
                Account::new(issuer.clone()).build(&issuer),
                Account::new(owner.clone()).build(&issuer),
            ],
            [],
        );
        iroha_core::runtime::keyed_state::install_for_test(&mut world);
        let mut cfg = TestChainConfig::new(world, 1000);
        cfg.genesis_instructions
            .push(Grant::account_permission(CanReadAllLedgerData, issuer.clone()).into());
        let mut chain = CertifiedTestChain::start(cfg).unwrap();
        let mut app = crate::mk_app_state_for_tests();
        Arc::get_mut(&mut app).unwrap().state = Arc::clone(chain.state());
        let selectors = selectors(br#"{"rows":[{"table":"world.assets","key_hex":"01"},{"table":"world.assets","key_hex":"02"}]}"#).unwrap();
        let grants = selectors
            .iter()
            .map(crate::torii_permission_target)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut change = |permission: Permission, grant: bool| {
            let instruction = if grant {
                Grant::account_permission(permission, owner.clone()).into()
            } else {
                Revoke::account_permission(permission, owner.clone()).into()
            };
            let now = chain
                .committed(chain.height())
                .block()
                .header()
                .creation_time_ms
                + 1;
            let tx = chain.sign(&iroha_test_samples::ALICE_KEYPAIR, [instruction], now);
            assert_eq!(chain.commit(vec![tx]), vec![true]);
        };
        assert!(require_permission(&app, &owner, &grants).is_err());
        change(
            iroha_executor_data_model::permission::query::CanReadCertifiedOriginals.into(),
            true,
        );
        assert!(require_permission(&app, &owner, &grants).is_err());
        change(grants[0].clone(), true);
        assert!(require_permission(&app, &owner, &grants[..1]).is_ok());
        assert!(require_permission(&app, &owner, &grants).is_err());
        change(grants[1].clone(), true);
        assert!(require_permission(&app, &owner, &grants).is_ok());
        change(grants[1].clone(), false);
        assert!(require_permission(&app, &owner, &grants).is_err());
        assert!(require_permission(&app, &issuer, &grants).is_ok());
    }
}
