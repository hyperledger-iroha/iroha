//! Durable ingress for single-route ordinary transactions.
//!
//! The leader samples these transactions from its local queue. An exact-height
//! threshold-key lifecycle certificate additionally authenticates its own
//! frozen-roster quorum before entering that queue.
use super::*;
use iroha_data_model::isi::consensus_keys::{
    ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleCertificateV1,
};

fn certificate(transaction: &TransactionEntrypoint) -> Option<&ThresholdKeyLifecycleCertificateV1> {
    let TransactionEntrypoint::External(transaction) = transaction else {
        return None;
    };
    if transaction.admission_intent() != TransactionAdmissionIntent::Ordinary
        || transaction.attachments().is_some()
        || transaction.multisig_signatures().is_some()
        || !transaction.metadata().is_empty()
    {
        return None;
    }
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        transaction.instructions()
    else {
        return None;
    };
    if instructions.len() != 1 {
        return None;
    }
    instructions[0]
        .as_any()
        .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
        .map(|instruction| &instruction.certificate)
}

pub(super) fn authenticate(
    app: &AppState,
    transaction: &TransactionEntrypoint,
    routing_plan: &RoutingPlan,
) -> Result<(), String> {
    let TransactionEntrypoint::External(signed) = transaction else {
        return Err("ordinary ingress requires one external signed transaction".to_owned());
    };
    if signed.admission_intent() != TransactionAdmissionIntent::Ordinary {
        return Err("ordinary ingress requires a signature-bound ordinary intent".to_owned());
    }
    if !matches!(routing_plan, RoutingPlan::Single(_)) {
        return Err(
            "multi-route transactions require signature-bound QueuePlanSynced admission".to_owned(),
        );
    }
    if !signed
        .instructions()
        .explicit_instructions()
        .any(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
                .is_some()
        })
    {
        return Ok(());
    }
    let certificate = certificate(transaction)
        .ok_or_else(|| "lifecycle instruction must be one exact certificate".to_owned())?;
    let global_route = resolve_torii_route_for_dataspace_id(app, DataSpaceId::UNIVERSAL)
        .map_err(|error| format!("lifecycle global route is unavailable: {error}"))?;
    if routing_plan != &RoutingPlan::single(global_route) {
        return Err(
            "lifecycle certificate requires the exact single global control route".to_owned(),
        );
    }
    let context = app
        .queue
        .plan_admission_context_with_state(&app.state, routing_plan)
        .map_err(|error| format!("lifecycle route authority is unavailable: {error}"))?;
    if context.proposal_height != certificate.effective_height
        || context.route_incarnations.len() != 1
    {
        return Err("lifecycle route does not bind the certified next height".to_owned());
    }
    let (parent_hash, roster) = app
        .state
        .verify_next_height_threshold_key_lifecycle_certificate_v1(certificate)?;
    if context.predecessor_block_hash != Some(parent_hash) {
        return Err("lifecycle route differs from the authenticated parent".to_owned());
    }
    let route = &context.route_incarnations[0];
    let local = app
        .local_peer_id
        .as_ref()
        .ok_or_else(|| "lifecycle ingress has no local validator identity".to_owned())?;
    if !roster.contains(local)
        || !route.validator_set.contains(local)
        || route
            .validator_set
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            != roster.iter().collect::<std::collections::BTreeSet<_>>()
    {
        return Err(
            "lifecycle ingress does not own the authenticated global control route".to_owned(),
        );
    }
    Ok(())
}

pub(super) async fn submit(
    app: SharedAppState,
    accepted: iroha_core::tx::AcceptedTransaction<'static>,
    routing_plan: RoutingPlan,
    minimal_response: bool,
    format: ResponseFormat,
) -> Response {
    let entrypoint_hash = accepted.entrypoint().hash();
    let signed_hash = signed_transaction_hash_for_entrypoint(accepted.entrypoint());
    let permit =
        match try_acquire_transaction_ingress_compute(&app.transaction_ingress_compute_inflight) {
            Ok(permit) => permit,
            Err(error) => return error.into_response(),
        };
    let worker_app = app.clone();
    let result = run_transaction_ingress_compute_job(
        permit,
        "ordinary_transaction_admission_worker_failed",
        move || {
            authenticate(&worker_app, accepted.entrypoint(), &routing_plan).map_err(|message| {
                Error::Query(iroha_data_model::ValidationFail::NotPermitted(message))
            })?;
            routing::push_accepted_transaction_for_ingress_with_routing_plan_strict_durable(
                worker_app.queue.clone(),
                worker_app.state.clone(),
                accepted,
                routing_plan,
            )
        },
    )
    .await;
    match result {
        Ok((route, _permit)) => transaction_submission_response(
            &app,
            entrypoint_hash,
            signed_hash,
            route,
            "local",
            minimal_response,
            format,
        ),
        Err(error) => error.into_response(),
    }
}
