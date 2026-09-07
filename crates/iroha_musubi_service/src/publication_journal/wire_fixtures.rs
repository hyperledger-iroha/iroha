//! Pre-extraction durable journal frames, variant tags and snapshot digests.

use super::*;
use crate::{MusubiSeedIngressStageRequestV1, tests::wire_fixtures::record};

pub(crate) fn check_identities(fixtures: &[norito::json::Value]) {
    use crate::tests::wire_fixtures::check_identity;
    check_identity::<DurableMusubiPublicationServiceJournalLimitsV1>(fixtures);
    check_identity::<DurablePublicationOperationRecordV1>(fixtures);
    check_identity::<DurablePublicationResultStateV1>(fixtures);
    check_identity::<DurablePublicationResultRecordV1>(fixtures);
    check_identity::<DurablePublicationAuthorizationRecordV1>(fixtures);
    check_identity::<DurablePublicationJournalStateV1>(fixtures);
    check_identity::<DurablePublicationJournalEnvelopeV1>(fixtures);
    check_identity::<Vec<DurablePublicationOperationRecordV1>>(fixtures);
    check_identity::<Vec<DurablePublicationResultRecordV1>>(fixtures);
    check_identity::<Vec<DurablePublicationAuthorizationRecordV1>>(fixtures);
}

pub(crate) fn records(seed: &MusubiSeedIngressStageRequestV1) -> Vec<norito::json::Value> {
    let deployment = MusubiPublicationServiceJournalBindingV1 {
        network_id: seed.binding.network_id,
        ingress_broker: seed.binding.ingress_broker.clone(),
        seed_provider: seed.binding.seed_provider,
    };
    let binding = MusubiPublicationOperationBindingV1 {
        operation_id: seed.operation_id,
        network_id: seed.binding.network_id,
        publisher: seed.binding.publisher.clone(),
        archive_id: seed.binding.archive_id,
        car_body_digest: seed.binding.car_body_digest,
        car_body_length: seed.binding.car_body_length,
    };
    let key = MusubiPublicationIdempotencyKeyV1 {
        operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
        operation_id: seed.operation_id,
        target: [0; 32],
    };
    let limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
        8,
        32,
        17 * 1024 * 1024,
        18 * 1024 * 1024,
    )
    .unwrap();
    let request_digest = [0x23; 32];
    let attempt = MusubiPublicationJournalAttemptV1 {
        key,
        binding: binding.clone(),
        request_digest,
        authorization_digest: [0x24; 32],
        authorization_expires_at_ms: 20_000,
    };
    let mut journal = InMemoryMusubiPublicationServiceJournalV1::new(
        deployment.clone(),
        limits.max_operations_usize(),
        limits.max_authorizations_usize(),
    )
    .unwrap();
    assert_eq!(
        journal.begin(&attempt, 1_000).unwrap(),
        MusubiPublicationJournalBeginV1::Execute
    );
    let pending = state_from_journal(&journal, &deployment, limits, 3).unwrap();
    let response = norito::encode_canonical(&42_u64).unwrap();
    journal.commit(key, request_digest, &response).unwrap();
    let complete = state_from_journal(&journal, &deployment, limits, 4).unwrap();
    let restored = journal_from_state(&complete, &deployment, limits).unwrap();
    assert_eq!(
        state_from_journal(&restored, &deployment, limits, 4).unwrap(),
        complete
    );
    let envelope = DurablePublicationJournalEnvelopeV1::new(complete.clone()).unwrap();
    envelope.validate_digest().unwrap();
    let variants = [
        (
            "pending",
            InMemoryPublicationResultV1::Pending(request_digest),
        ),
        (
            "aborted",
            InMemoryPublicationResultV1::Aborted(request_digest),
        ),
        (
            "refreshing",
            InMemoryPublicationResultV1::Refreshing {
                request_digest,
                previous_response: response.clone(),
            },
        ),
        (
            "complete",
            InMemoryPublicationResultV1::Complete {
                request_digest,
                response,
            },
        ),
    ];
    let mut records = vec![
        record("journal.limits", &limits),
        record("journal.deployment", &deployment),
        record("journal.operation_binding", &binding),
        record("journal.idempotency_key", &key),
        record("journal.operation_record", &complete.operations[0]),
        record("journal.authorization_record", &complete.authorizations[0]),
        record("journal.operation_records", &complete.operations),
        record("journal.authorization_records", &complete.authorizations),
        record("journal.result_records", &complete.results),
        record("journal.pending_state", &pending),
        record("journal.complete_state", &complete),
        record("journal.envelope", &envelope),
        norito::json!({
            "specimen": "journal.digests",
            "pending_state_digest": (hex::encode(pending.digest().unwrap())),
            "complete_state_digest": (hex::encode(complete.digest().unwrap())),
        }),
    ];
    for (label, result) in variants {
        journal.results.insert(key, result);
        let snapshot = state_from_journal(&journal, &deployment, limits, 5).unwrap();
        let restored = journal_from_state(&snapshot, &deployment, limits).unwrap();
        assert_eq!(
            state_from_journal(&restored, &deployment, limits, 5).unwrap(),
            snapshot
        );
        records.push(record(
            &format!("journal.{label}"),
            &snapshot.results[0].state,
        ));
        records.push(record(
            &format!("journal.{label}_record"),
            &snapshot.results[0],
        ));
        records.push(record(&format!("journal.{label}_snapshot"), &snapshot));
    }
    records
}
