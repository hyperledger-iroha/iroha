//! Finality-bound public observations for frozen validator committee preparation.

use super::*;
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, global_threshold_beacon_roster_hash_v1,
        validate_global_threshold_beacon_session_v1,
    },
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    block::consensus_v2::finality::V2FinalityArtifact,
    nexus::{
        ValidatorCandidateKeysV1, ValidatorCommitteeSelectionStatusV1, ValidatorCommitteeStatusV1,
    },
};
use mv::storage::StorageReadOnly;

/// Optional exact target epoch; omission selects the next scheduling epoch.
#[derive(Debug, Default, crate::json_macros::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct CommitteeStatusQuery {
    pub(super) target_epoch: Option<u64>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Query(iroha_data_model::ValidationFail::InternalError(
        message.into(),
    ))
}

fn unavailable() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::NotFound,
    ))
}

fn finality_at(state: &impl StateReadOnly, height: u64) -> Result<V2FinalityArtifact, Error> {
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(unavailable)?;
    let block = state.block_by_height(index).ok_or_else(unavailable)?;
    let artifact = state
        .kura()
        .v2_finality_artifact(height)
        .map_err(|error| {
            invalid(format!(
                "invalid durable committee finality at {height}: {error}"
            ))
        })?
        .ok_or_else(unavailable)?;
    if !block.has_results()
        || artifact.block_hash != block.hash()
        || artifact.height_context.network_id != *state.network_id()
    {
        return Err(invalid("committee finality differs from committed State"));
    }
    artifact
        .validate_for_header(&block.header())
        .map_err(|error| invalid(format!("committee finality/header mismatch: {error}")))?;
    Ok(artifact)
}

pub(super) fn validate_selection(
    selection: &ValidatorCommitteeSelectionStatusV1,
    latest: &V2FinalityArtifact,
    target_epoch: u64,
) -> Result<(), Error> {
    selection.transition.validate().map_err(invalid)?;
    let preparation = &selection.transition.preparation;
    let selecting = &selection.selecting_finality;
    if preparation.target_epoch != target_epoch
        || preparation.network_id != latest.height_context.network_id
        || selecting.height_context.network_id != preparation.network_id
        || selecting.height != preparation.selection_height
        || selecting.height > latest.height
        || selecting.subject.parent_block_hash != Some(preparation.selection_anchor)
    {
        return Err(invalid("committee selection finality binding differs"));
    }
    let snapshot = selecting
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .ok_or_else(|| invalid("selecting finality lacks the frozen committee preparation"))?;
    if snapshot.committee_preparation.as_ref() != Some(preparation) {
        return Err(invalid(
            "committee preparation differs from selecting finality",
        ));
    }
    preparation
        .validate_against_preparing_authorization(&snapshot.kagemusha_mint_finality_authorization)
        .map_err(invalid)?;
    Ok(())
}

fn load(
    state: &impl StateReadOnly,
    target_epoch: Option<u64>,
) -> Result<ValidatorCommitteeStatusV1, Error> {
    let height = u64::try_from(state.height()).map_err(|_| invalid("committee height overflow"))?;
    let latest_finality = finality_at(state, height)?;
    let current = latest_finality
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .map_or(
            &latest_finality
                .height_context
                .kagemusha_mint_finality_authorization,
            |snapshot| &snapshot.kagemusha_mint_finality_authorization,
        );
    let target_epoch = match target_epoch {
        Some(epoch) => epoch,
        None => current
            .epoch
            .checked_add(1)
            .ok_or_else(|| invalid("committee target epoch overflow"))?,
    };
    let selected = state
        .world()
        .validator_committee_transitions()
        .get(&target_epoch)
        .cloned()
        .map(|transition| {
            let selecting_finality = finality_at(state, transition.preparation.selection_height)?;
            let selection = ValidatorCommitteeSelectionStatusV1 {
                transition,
                selecting_finality,
            };
            validate_selection(&selection, &latest_finality, target_epoch)?;
            Ok::<_, Error>(selection)
        })
        .transpose()?;
    if selected.is_none()
        && latest_finality
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.committee_preparation.as_ref())
            .is_some_and(|preparation| preparation.target_epoch == target_epoch)
    {
        return Err(invalid(
            "finalized committee preparation is absent from committed State",
        ));
    }
    let network_id = *state.network_id();
    // A status read reports only the frozen attempt's bounded set of candidates.
    // Before an election exists there is no selected roster to enumerate.
    let mut candidate_keys = Vec::new();
    if let Some(selection) = &selected {
        let generation = selection.transition.preparation.authority_generation;
        for seat in &selection.transition.preparation.roster {
            let key = ValidatorCandidateKeysV1::key_id(network_id, generation, &seat.validator);
            let Some(candidate) = state.world().validator_candidate_keys().get(&key) else {
                continue;
            };
            if candidate.network_id != network_id
                || candidate.generation != generation
                || candidate.keys.validator != seat.validator
            {
                return Err(invalid("candidate publication storage binding differs"));
            }
            candidate.validate().map_err(invalid)?;
            candidate_keys.push(candidate.clone());
        }
    }
    let pending_beacon_session = selected
        .as_ref()
        .map(|selection| {
            let preparation = &selection.transition.preparation;
            let session_id = preparation.beacon_session_id().map_err(invalid)?;
            state
                .world()
                .global_beacon_key_sessions()
                .get(&session_id)
                .map(|record| {
                    let peers = preparation
                        .roster
                        .iter()
                        .map(|seat| seat.validator.clone())
                        .collect::<Vec<_>>();
                    if usize::from(record.session.committee_size) != peers.len()
                        || record.session.adaptive_dkg.session.start_height
                            <= preparation.selection_height
                        || record.session.adaptive_dkg.finalized_at_height
                            >= preparation.first_height - 1
                    {
                        return Err(invalid(
                            "prepared committee beacon differs from the frozen roster or interval",
                        ));
                    }
                    let binding = GlobalThresholdBeaconSessionBindingV1 {
                        network_id,
                        session_id,
                        roster_hash: global_threshold_beacon_roster_hash_v1(&peers),
                        transcript_hash: record.session.transcript_hash,
                    };
                    validate_global_threshold_beacon_session_v1(record.session.clone(), &binding)
                        .map_err(|error| {
                        invalid(format!("invalid prepared committee beacon: {error}"))
                    })?;
                    if selection
                        .transition
                        .credentials
                        .as_ref()
                        .is_some_and(|credentials| {
                            credentials.beacon.session_id != session_id
                                || credentials.beacon.transcript_hash
                                    != record.session.transcript_hash
                        })
                    {
                        return Err(invalid(
                            "prepared committee beacon differs from fixed credentials",
                        ));
                    }
                    Ok(record.session.clone())
                })
                .transpose()
        })
        .transpose()?
        .flatten();
    if pending_beacon_session.is_none()
        && selected
            .as_ref()
            .is_some_and(|selection| selection.transition.credentials.is_some())
    {
        return Err(invalid(
            "fixed committee credentials lack their committed beacon transcript",
        ));
    }
    Ok(ValidatorCommitteeStatusV1 {
        network_id,
        target_epoch,
        latest_finality,
        selected,
        candidate_keys,
        pending_beacon_session,
    })
}

/// Serve the exact selected preparation and its immutable finality attachments.
pub(super) async fn handler_validator_committee_status(
    State(app): State<SharedAppState>,
    crate::NoritoQuery(query): crate::NoritoQuery<CommitteeStatusQuery>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    validate_api_token(app.as_ref(), &headers)?;
    let format = match negotiate_heavy_query_response_format(&headers) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        iroha_torii_shared::uri::NEXUS_VALIDATOR_COMMITTEE,
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let state = app.state.clone();
    let response =
        routing::run_admitted_blocking(admission, "committee status worker failed", move || {
            let view = state.view();
            let payload = load(&view, query.target_epoch)?;
            Ok(crate::utils::respond_with_format(payload, format))
        })
        .await?;
    proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        "v1/nexus/validator-committee",
        response,
        true,
    )
    .await
}
