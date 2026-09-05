//! Public, bounded, read-only native race discovery. Transactions use the standard signed pipeline.
use crate::{Error, SharedAppState};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::Hash;
use mv::storage::StorageReadOnly;
use norito::json;
/// Cursor for a bounded stable race-id ordered page.
#[derive(Default, Debug, crate::json_macros::JsonDeserialize)]
pub(crate) struct RaceListParams {
    /// Exclusive canonical race hash cursor.
    pub cursor: Option<String>,
}
/// The shipped protocol and verifier qualification, bound to the exact chain.
pub(crate) fn capabilities(app: &SharedAppState) -> Result<Response, Error> {
    let view = app.state.view();
    Ok(crate::utils::JsonBody(json::json!({
        "version":1,
        "network_id":view.network_id(),
        "native_races":true,
        "enabled":true,
        "proof_qualified":iroha_core::execution_proofs::race_profile_is_qualified_v1(),
        "profile_id":iroha_core::execution_proofs::race_profile_id_v1(),
        "rules_hash":iroha_core::execution_proofs::race_rules_hash_v1(),
        "max_racers":8,
        "ticks_per_second":30,
        "input_batch_ticks":6,
        "max_ticks":5400,
        "wallet_funds_required_only_for_entry":true
    }))
    .into_response())
}
/// Read one exact race; this is node evidence until the caller authenticates ledger finality.
pub(crate) fn get(app: &SharedAppState, id: &str) -> Result<Response, Error> {
    let id: Hash = id
        .parse()
        .map_err(|_| crate::routing::conversion_error("invalid race hash".into()))?;
    let view = app.state.view();
    let Some(race) = view.world().races().get(&id) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    Ok(crate::utils::JsonBody(race.clone()).into_response())
}
/// Read at most 32 summaries without loading historical input arrays into the response.
pub(crate) fn list(app: &SharedAppState, params: RaceListParams) -> Result<Response, Error> {
    let cursor = params
        .cursor
        .map(|s| {
            s.parse::<Hash>()
                .map_err(|_| crate::routing::conversion_error("invalid race cursor".into()))
        })
        .transpose()?;
    let view = app.state.view();
    let mut rows = Vec::new();
    let mut next = None;
    let mut has_more = false;
    for (id, race) in view
        .world()
        .races()
        .range((
            cursor.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded),
            std::ops::Bound::Unbounded,
        ))
        .take(33)
    {
        if rows.len() == 32 {
            has_more = true;
            break;
        }
        rows.push(json::json!({"race_id":id,"rules":race.rules,"stake":race.stake,"asset_definition":race.asset_definition,"participants":race.participants,"phase":race.phase,"revision":race.revision,"deadline_height":race.deadline_height,"result":race.result}));
        next = Some(*id);
    }
    Ok(crate::utils::JsonBody(
        json::json!({"version":1,"network_id":view.network_id(),"items":rows,"next_cursor":if has_more {next} else {None}}),
    )
    .into_response())
}
