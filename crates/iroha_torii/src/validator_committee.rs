//! Finality-bound public observations for frozen validator committee preparation.

use super::*;
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconSessionBindingV1, global_threshold_beacon_roster_hash_v1,
        validate_global_threshold_beacon_session_v1,
    },
    state::{StateReadOnly, WorldReadOnly},
    sumeragi::certified_chain::CertifiedChain,
    validator_committee_evidence::validate_validator_committee_selection_binding_v1,
};
use iroha_data_model::{
    nexus::{
        ValidatorCandidateKeysV1, ValidatorCommitteeSelectionStatusV1, ValidatorCommitteeStatusV1,
    },
    sumeragi::finality::{
        NATIVE_FINALITY_MAX_BLOCK_BYTES, NATIVE_FINALITY_MAX_BLOCK_COUNT,
        NATIVE_FINALITY_MAX_JOURNAL_BYTES, NativeFinalityArtifact, NativeFinalityLimits,
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

fn load(
    state: &impl StateReadOnly,
    target_epoch: Option<u64>,
    limits: NativeFinalityLimits,
) -> Result<ValidatorCommitteeStatusV1, Error> {
    let height = u64::try_from(state.height()).map_err(|_| invalid("committee height overflow"))?;
    limits.validate().map_err(invalid)?;
    if height < 2 {
        return Err(unavailable());
    }
    // The independently captured original execution tip authenticates recent
    // ancestry. Rechecking the entire genesis prefix for every status read makes
    // valid boundaries eventually exhaust even an otherwise idle request pool.
    // Keep one cumulative decode scope and source allowance for both attachments.
    let mut source_blocks_left = limits.block_count as u64;
    let mut source_bytes_left = limits.journal_bytes as u64;
    use iroha_data_model::query::error::QueryExecutionFail;
    let query_error = |error| Error::Query(iroha_data_model::ValidationFail::QueryFailed(error));
    let mut admit = |work, bytes| {
        if bytes > limits.block_bytes as u64 {
            return Err(QueryExecutionFail::GasBudgetExceeded);
        }
        let remaining_blocks = source_blocks_left
            .checked_sub(work)
            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
        let remaining_bytes = source_bytes_left
            .checked_sub(bytes)
            .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
        source_blocks_left = remaining_blocks;
        source_bytes_left = remaining_bytes;
        Ok(())
    };
    let reader =
        CertifiedChain::new_with_source_admission(state, &mut admit).map_err(query_error)?;
    let mut read = |height: u64| {
        let height = usize::try_from(height)
            .ok()
            .and_then(std::num::NonZeroUsize::new)
            .ok_or_else(unavailable)?;
        reader
            .certified_from_execution(height, &mut admit)
            .map_err(query_error)
    };
    let latest = read(height)?;
    let latest_finality =
        NativeFinalityArtifact::from_block(latest.block(), limits).map_err(invalid)?;
    let outcome = &latest.commitment().schedule;
    let current = outcome
        .boundary
        .as_ref()
        .map_or(&outcome.current, |boundary| &boundary.next);
    let target_epoch = match target_epoch {
        Some(epoch) => epoch,
        None => current
            .authorization
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
            let historical = (transition.preparation.selection_height != height)
                .then(|| read(transition.preparation.selection_height))
                .transpose()?;
            let selecting = historical.as_ref().unwrap_or(&latest);
            validate_validator_committee_selection_binding_v1(
                &transition,
                selecting,
                &latest,
                target_epoch,
            )
            .map_err(invalid)?;
            let selecting_finality =
                NativeFinalityArtifact::from_block(selecting.block(), limits).map_err(invalid)?;
            let selection = ValidatorCommitteeSelectionStatusV1 {
                transition,
                selecting_finality,
            };
            Ok::<_, Error>(selection)
        })
        .transpose()?;
    if selected.is_none()
        && outcome
            .boundary
            .as_ref()
            .and_then(|boundary| boundary.preparation.as_ref())
            .is_some_and(|preparation| preparation.target_epoch == target_epoch)
    {
        return Err(invalid(
            "certified committee preparation is absent from committed State",
        ));
    }
    let network_id = *state.network_id();
    // A status read reports only the frozen attempt's bounded set of candidates.
    // Before an election exists there is no selected roster to enumerate.
    let mut candidate_keys = Vec::new();
    if let Some(selection) = &selected {
        let generation = selection.transition.preparation.authority_generation;
        for seat in &selection.transition.preparation.committee {
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
                        .committee
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
        iroha_torii_shared::route_catalog::core::NEXUS_VALIDATOR_COMMITTEE_GET.path(),
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let reservation = match try_acquire_query_fanout_memory(&app) {
        Ok(reservation) => reservation,
        Err(response) => return Ok(response),
    };
    // Committee evidence reads complete certified blocks. The ordinary name/ID
    // query source bound is only 1 KiB and cannot describe this workload. Reserve
    // a complete working set before reading State and retain it through egress.
    let envelope = match QueryFanoutMemoryEnvelope::new(app.query_fanout_working_set_bytes, 0) {
        Ok(envelope) => envelope,
        Err(response) => {
            return Ok(hold_query_fanout_memory_in_response_body(
                response,
                reservation,
            ));
        }
    };
    let response_limit = envelope.final_body_bytes;
    let limits = NativeFinalityLimits {
        block_bytes: envelope
            .route_body_bytes
            .min(NATIVE_FINALITY_MAX_BLOCK_BYTES)
            .min(response_limit),
        journal_bytes: response_limit.min(NATIVE_FINALITY_MAX_JOURNAL_BYTES),
        block_count: NATIVE_FINALITY_MAX_BLOCK_COUNT,
        allocated_bytes: envelope.decode_allocated_bytes,
    };
    limits.validate().map_err(invalid)?;
    let state = app.state.clone();
    let worker_reservation = reservation.clone();
    let response =
        routing::run_admitted_blocking(admission, "committee status worker failed", move || {
            // A dropped HTTP future does not cancel a running blocking worker.
            let _reservation = worker_reservation;
            let view = state.view();
            let payload = norito::core::with_decode_limits_scope(
                limits.decode_limits().map_err(invalid)?,
                || load(&view, query.target_epoch, limits),
            )?;
            crate::utils::respond_with_format_bounded(payload, format, response_limit).map_err(
                |error| {
                    invalid(format!(
                        "committee response exceeds its configured bound: {error}"
                    ))
                },
            )
        })
        .await?;
    let response = proof_response_with_exact_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        "v1/nexus/validator-committee",
        response,
        true,
    )
    .await?;
    Ok(hold_query_fanout_memory_in_response_body(
        response,
        reservation,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::query::error::QueryExecutionFail;

    #[test]
    fn validator_committee_status_charges_genesis_to_the_shared_source_limits() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(iroha_core::state::World::new(), 1_000))
                .unwrap();
        while chain.height() < 4 {
            chain.commit(Vec::new());
        }
        let lengths =
            [1, 3, 4].map(|height| chain.committed(height).block().encode_wire().unwrap().len());
        let limits = NativeFinalityLimits {
            block_bytes: *lengths.iter().max().unwrap(),
            journal_bytes: lengths.iter().sum(),
            block_count: 3,
            allocated_bytes: 128 * 1024 * 1024,
        };
        let view = chain.state().view();
        assert!(load(&view, None, limits).is_ok());
        for refused in [
            NativeFinalityLimits {
                block_count: 2,
                ..limits
            },
            NativeFinalityLimits {
                journal_bytes: limits.journal_bytes - 1,
                ..limits
            },
        ] {
            assert!(matches!(
                load(&view, None, refused),
                Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    QueryExecutionFail::GasBudgetExceeded
                )))
            ));
        }
        assert!(
            load(&view, None, limits).is_ok(),
            "capacity refusal must preserve canonical storage for retry"
        );
    }
}
