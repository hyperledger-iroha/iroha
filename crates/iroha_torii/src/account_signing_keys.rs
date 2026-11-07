//! Public read of an account's off-chain signing key (`specs/account_signing_keys.md`).
//!
//! A verifier of an account's off-chain signatures, such as an Iroha Connect dApp, reads the
//! key and its generation here and checks a signature over the off-chain envelope against it.
//! A key a requester presents proves nothing; only this committed record does. An account
//! without an active signing key answers `account_signing_key_not_found`.

use super::*;

const ROUTE: &str = "/v1/accounts/{account_id}/signing-key";

pub async fn read(
    State(app): State<SharedAppState>,
    axum::extract::Path(account_literal): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<AxResponse, Error> {
    let (account, _canonical) = routing::parse_account_path_segment_with_state(
        app.state.as_ref(),
        &account_literal,
        &routing::MaybeTelemetry::disabled(),
        ROUTE,
    )?;
    let _principal = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        ROUTE,
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response =
        routing::run_admitted_blocking(admission, "account signing key worker failed", move || {
            let signing_key = signing_key_of(&state.world_view(), &account)?;
            if matches!(format, crate::utils::ResponseFormat::Norito) {
                Ok(crate::NoritoBody(signing_key).into_response())
            } else {
                crate::routing::pretty_json_response(&signing_key)
            }
        })
        .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}

/// The active signing key of `account`, or the stable not-found error.
fn signing_key_of(
    world: &impl iroha_core::state::WorldReadOnly,
    account: &iroha_data_model::account::AccountId,
) -> Result<iroha_data_model::account::signing_key::AccountSigningKeyV1, Error> {
    iroha_core::smartcontracts::isi::account_signing_key::account_signing_key(world, account)
        .ok_or_else(|| Error::AppNotFound {
            code: "account_signing_key_not_found",
            message: "the account has no active off-chain signing key".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountId, signing_key::AccountSigningKeyV1},
        isi::account_signing_key::SetAccountSigningKeyV1,
        prelude::BlockHeader,
    };

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture key")
    }

    #[test]
    fn reads_only_an_active_committed_signing_key() {
        use iroha_core::smartcontracts::Execute as _;
        let account = AccountId::new(key(1).public_key().clone());
        let world = World::with([], [Account::new(account.clone()).build(&account)], []);
        // The tables exist only on a root whose change set activated the addition.
        iroha_core::runtime::activate_additions_for_testing(&world, "iroha.account.signing_key.v1")
            .expect("activate the account signing key addition");
        let state = iroha_core::state::State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        assert!(matches!(
            signing_key_of(&state.world_view(), &account),
            Err(Error::AppNotFound {
                code: "account_signing_key_not_found",
                ..
            })
        ));
        let mut block = state.block(BlockHeader::new(
            std::num::NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ));
        let mut tx = block.transaction();
        SetAccountSigningKeyV1::install(account.clone(), key(2).public_key().clone())
            .execute(&account, &mut tx)
            .unwrap();
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
        assert_eq!(
            signing_key_of(&state.world_view(), &account).unwrap(),
            AccountSigningKeyV1 {
                public_key: key(2).public_key().clone(),
                generation: 1,
            }
        );
    }
}
