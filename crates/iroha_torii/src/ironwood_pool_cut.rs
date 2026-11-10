//! Current-cut Ironwood pool originals under certified-original or full-ledger read authority.
//! Complete World hashes authenticate absence but expose unrelated row identities and permit
//! low-entropy value dictionaries. Selected data space grants cannot authorize this response.
//! A certified-original reader may name any payer: it reads that payer's public Global balance
//! row, and the runner reading a cut is not the per-run account paying for the proof.
//! TODO: replace this full snapshot with genuinely scoped keyed-State inclusion/absence proofs
//! before offering restricted bank one-call context reads.
use crate::*;
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::AccountId,
    privacy::PrivacyPoolIdV1,
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_JOURNAL_BYTES, NativeFinalityLimits,
    },
    sumeragi_finality::MAX_WORLD_STATE_SNAPSHOT_BYTES_V1,
};
type SelectedPools = (PrivacyPoolIdV1, AccountId);
const ROUTE: &str = "/v1/privacy/ironwood/pool-cut/{height}";
fn unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "ironwood_pool_cut_unavailable",
        message: "The exact current certified Ironwood pool context is unavailable.".into(),
    }
}
fn height_selector(value: &str) -> Result<u64, Error> {
    value
        .parse::<u64>()
        .ok()
        .filter(|h| *h >= 2 && h.to_string() == value)
        .ok_or_else(|| {
            conversion_error("Ironwood pool cut requires canonical non-genesis height".into())
        })
}
fn selector(body: &[u8], _budget: &AllocationBudget) -> Result<SelectedPools, Error> {
    if body.is_empty() || body.len() > 4096 {
        return Err(crate::native_projection_response::capacity());
    }
    let value: norito::json::Value = norito::json::from_slice(body)
        .map_err(|_| conversion_error("malformed Ironwood cut selector".into()))?;
    let invalid =
        || conversion_error("Ironwood cut requires exact pool_id_hex and payer_account_id".into());
    let object = value
        .as_object()
        .filter(|o| o.len() == 2)
        .ok_or_else(invalid)?;
    let pool = object
        .get("pool_id_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(invalid)?;
    if pool.len() != 64
        || !pool
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    let pool: [u8; 32] = hex::decode(pool)
        .map_err(|_| invalid())?
        .try_into()
        .map_err(|_| invalid())?;
    if pool == [0; 32] {
        return Err(invalid());
    }
    let payer = AccountId::parse_encoded(
        object
            .get("payer_account_id")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    Ok((PrivacyPoolIdV1::new(pool), payer))
}
fn require_permission(
    app: &SharedAppState,
    account: &AccountId,
    _selectors: &SelectedPools,
) -> Result<(), Error> {
    // The selected typed values do not reduce the full hash-list disclosure. Reuse the
    // canonical permission matchers at every current authorization check.
    let owner = crate::history_producer::HistoryProducerOwner::authentication_read(app)?;
    owner.scope(|| {
        app.state
            .try_with_authorization_view(owner.allocation_context(), |view| {
                use iroha_core::state::{StorageReadOnly as _, WorldReadOnly as _};
                let world = view.world();
                let allowed = world.accounts().get(account).is_some()
                    && (crate::stream_control::can_read_all(world, account)?
                        || crate::stream_control::can_read_certified_originals(world, account)?);
                if !allowed {
                    return Err(Error::Query(
                        iroha_data_model::ValidationFail::NotPermitted(
                            "Ironwood pool cut full World snapshot requires certified-original or all-ledger read permission".into(),
                        ),
                    ));
                }
                Ok(())
            })
            .map_err(crate::app_auth::authorization_view_error)?
    })
}
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
            "Ironwood pool cut requires exact signed request target".into(),
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
            code: "ironwood_pool_cut_authentication_required",
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
    let selectors = selector(&body, &budget)?;
    require_permission(&app, &verified.account, &selectors)?;
    let state = app.state.clone();
    let maximum = app
        .torii_proxy_max_response_bytes
        .min(MAX_WORLD_STATE_SNAPSHOT_BYTES_V1);
    let original_selectors = selectors.clone();
    let response =
        routing::run_admitted_blocking(admission, "Ironwood pool cut worker failed", move || {
            let _body = (body, body_charge);
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
            let view = state.view();
            let tip = norito::core::with_decode_limits_scope(decode_limits, || {
                crate::native_projection_response::current_global_tip(
                    &view,
                    height,
                    limits,
                    unavailable,
                )
            })?;
            drop(view);
            let mut body = state
                .with_native_ironwood_pool_cut_v1(
                    &tip,
                    original_selectors.0,
                    &original_selectors.1,
                    &budget,
                    |proof| {
                        crate::native_projection_response::encode(&proof, format, maximum, &budget)
                            .map_err(|_| "Ironwood pool cut output unavailable".to_owned())
                    },
                )
                .map_err(|_| unavailable())?;
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
    require_permission(&app, &verified.account, &selectors)?;
    let mut response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await?;
    require_permission(&app, &verified.account, &selectors)?;
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
    fn ironwood_pool_cut_selector_is_bounded_and_exact() {
        let budget = AllocationBudget::new(4096);
        let payer = iroha_test_samples::ALICE_ID.to_string();
        let valid = norito::json::to_json(
            &norito::json!({"pool_id_hex": (hex::encode([1; 32])), "payer_account_id": payer}),
        )
        .unwrap();
        assert_eq!(
            selector(valid.as_bytes(), &budget).unwrap().0,
            PrivacyPoolIdV1::new([1; 32])
        );
        for bad in [
            b"{}".as_slice(),
            br#"{"pool_id_hex":"00","payer_account_id":"bad"}"#,
            &[b'x'; 4097],
        ] {
            assert!(selector(bad, &budget).is_err());
        }
        assert_eq!(height_selector("2").unwrap(), 2);
        for bad in ["0", "1", "02", "+2"] {
            assert!(height_selector(bad).is_err());
        }
        assert_eq!(
            ROUTE,
            iroha_torii_shared::route_catalog::runtime_governance::PRIVACY_IRONWOOD_POOL_CUT.path()
        );
    }
    #[test]
    fn ironwood_pool_cut_requires_reader_permission_and_current_revocation() {
        use iroha_core::state::World;
        use iroha_data_model::{Account, Registrable};
        use iroha_executor_data_model::permission::query::CanReadCertifiedOriginals;
        let owner = iroha_test_samples::ALICE_ID.clone();
        let world = World::with([], [Account::new(owner.clone()).build(&owner)], []);
        let app = crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(world);
        let ds = iroha_model_base::topology::DataSpaceId::new(7);
        let key = (PrivacyPoolIdV1::new([9; 32]), owner.clone());
        let change = |permission: iroha_data_model::permission::Permission, grant| {
            let mut block = app.state.block(iroha_data_model::block::BlockHeader::new(
                std::num::NonZeroU64::new(1).unwrap(),
                None,
                None,
                0,
                0,
            ));
            let mut tx = block.transaction();
            if grant {
                tx.world_mut_for_testing()
                    .add_account_permission(&owner, permission);
            } else {
                assert!(
                    tx.world_mut_for_testing()
                        .remove_account_permission(&owner, &permission)
                );
            }
            tx.apply();
            block.commit_world_overlay_for_testing().unwrap();
        };
        assert!(require_permission(&app, &owner, &key).is_err());
        change(
            CanReadRestrictedDataspace {
                dataspace: iroha_model_base::topology::DataSpaceId::new(8),
            }
            .into(),
            true,
        );
        assert!(
            require_permission(&app, &owner, &key).is_err(),
            "foreign dataspace grant grants no selected-context read"
        );
        change(CanReadRestrictedDataspace { dataspace: ds }.into(), true);
        assert!(
            require_permission(&app, &owner, &key).is_err(),
            "even all selected dataspace grants cannot disclose unrelated complete World hashes"
        );
        for reader in [
            iroha_data_model::permission::Permission::from(CanReadAllLedgerData),
            CanReadCertifiedOriginals.into(),
        ] {
            change(reader.clone(), true);
            require_permission(&app, &owner, &key).unwrap();
            change(reader, false);
            assert!(
                require_permission(&app, &owner, &key).is_err(),
                "a retained request cannot hide current revocation"
            );
        }
    }
}
