//! Native admission transitions and finalized readers over a certified four-validator chain.

use super::test_fixture::{
    NOW, ProviderAdmissionTestFixtureV1 as Fixture, key, raw, sign_envelope,
};
use super::*;
use iroha_data_model::sorafs::capacity::ProviderId;
use iroha_data_model::sorafs::provider_admission::history::AdmissionHistoryPathV1;
use sorafs_manifest::provider_admission::compute_envelope_digest;
#[test]
fn finalized_admission_expiry_cannot_be_reopened_by_a_lagging_local_clock() {
    let mut fixture = Fixture::new();
    fixture.envelope.retention_epoch = NOW + 3;
    sign_envelope(&mut fixture.envelope, &fixture.policy, &fixture.signer);
    fixture.admit();
    let provider = fixture.provider();
    assert!(
        native::read_finalized_provider_admission_v1(&fixture.state.view(), provider, NOW - 1)
            .is_err(),
        "finalized time cannot replace the local issuance lower bound"
    );
    fixture.commit(|_| {}, true);
    assert!(
        native::read_finalized_provider_admission_v1(&fixture.state.view(), provider, NOW + 1)
            .unwrap()
            .is_some(),
        "both clocks remain before admission expiry"
    );
    assert!(
        native::read_finalized_provider_admission_v1(&fixture.state.view(), provider, NOW + 3)
            .is_err(),
        "local expiry still closes admission before committed time reaches it"
    );
    fixture.commit(|_| {}, true);
    assert!(
        native::read_finalized_provider_admission_v1(&fixture.state.view(), provider, NOW + 1)
            .is_err(),
        "exact finalized expiry cannot be rolled back by local time"
    );
}
#[test]
fn finalized_admission_revocation_and_tombstones_share_one_native_history() {
    let mut f = Fixture::new();
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), f.provider(), NOW + 1)
            .unwrap()
            .is_none(),
        "no council and no admission before the first enactment"
    );
    f.admit();
    let provider = f.provider();
    assert_eq!(
        native::retained_provider_count_v1(&f.state.view()).unwrap(),
        1
    );
    let record = native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 2)
        .unwrap()
        .unwrap();
    assert_eq!(record.envelope(), &f.envelope);
    let revoke = f.revocation();
    f.commit(
        |tx| {
            assert!(apply(Action::Revoke(native::encode(&revoke).unwrap()), tx).unwrap());
        },
        true,
    );
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 3)
            .unwrap()
            .is_none()
    );
    assert!(native::retains_provider_identity_v1(&f.state.view(), provider).unwrap());
    let original = f.envelope.clone();
    f.commit(
        |tx| {
            assert!(apply(Action::Admit(native::encode(&original).unwrap()), tx).is_err());
            assert!(
                governed_head(
                    tx.world(),
                    &Action::Admit(native::encode(&original).unwrap())
                )
                .unwrap()
                .is_some()
            );
        },
        true,
    );
}
#[test]
fn renewal_requires_exact_predecessor_and_follows_governed_council_rotation() {
    let mut f = Fixture::new();
    f.admit();
    let old_digest = compute_envelope_digest(&f.envelope).unwrap();
    let mut policy = f.policy.clone();
    policy.revision += 1;
    policy.predecessor_policy_digest = Some(f.policy.canonical_digest().unwrap());
    let new_signer = key(32);
    policy.trusted_signers = vec![raw(&new_signer)];
    let mut next = f.envelope.clone();
    next.admission_revision += 1;
    next.expected_current_event_digest = Some(old_digest);
    next.retention_epoch += 60;
    sign_envelope(&mut next, &policy, &new_signer);
    let renewal = ProviderAdmissionRenewalV1 {
        version: sorafs_manifest::provider_admission::PROVIDER_ADMISSION_RENEWAL_VERSION_V1,
        provider_id: next.proposal.provider_id,
        previous_envelope_digest: old_digest,
        envelope_digest: compute_envelope_digest(&next).unwrap(),
        envelope: next.clone(),
        notes: None,
    };
    let provider = f.provider();
    f.commit(
        |tx| {
            assert!(
                apply(
                    Action::ConfigureCouncil(native::encode(&policy).unwrap()),
                    tx
                )
                .unwrap()
            );
            let mut wrong = renewal.clone();
            wrong.previous_envelope_digest[0] ^= 1;
            assert!(apply(Action::Renew(native::encode(&wrong).unwrap()), tx).is_err());
            assert!(apply(Action::Renew(native::encode(&renewal).unwrap()), tx).unwrap());
            assert!(apply(Action::Renew(native::encode(&renewal).unwrap()), tx).is_err());
        },
        true,
    );
    assert_eq!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 3)
            .unwrap()
            .unwrap()
            .envelope(),
        &next
    );
}
#[test]
fn native_head_needs_durable_qc_and_exact_owner_and_retained_predecessor() {
    let mut f = Fixture::new();
    f.admit();
    let provider = f.provider();
    // A tip whose local CommitQC does not verify is not a finalized cut.
    f.commit(|_| {}, false);
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 3).is_err()
    );
    f.commit(|_| {}, true);
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 3)
            .unwrap()
            .is_some()
    );
    f.commit(
        |tx| {
            tx.world.provider_owners.remove(provider);
        },
        true,
    );
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 4).is_err()
    );
    f.commit(
        |tx| {
            tx.world.smart_contract_state.remove(native::path(
                Some(provider),
                AdmissionHistoryPathV1::Revision(1),
            ));
            assert!(native::read_head(tx.world(), Some(provider)).is_err());
        },
        true,
    );
}
#[test]
fn initial_policy_and_signed_claim_substitution_are_rejected_before_any_write() {
    let mut f = Fixture::new();
    let policy = f.policy.clone();
    let envelope = f.envelope.clone();
    f.commit(
        |tx| {
            let mut skipped = policy.clone();
            skipped.revision = 2;
            skipped.predecessor_policy_digest = Some([1; 32]);
            assert!(
                apply(
                    Action::ConfigureCouncil(native::encode(&skipped).unwrap()),
                    tx
                )
                .is_err()
            );
            assert!(native::read_head(tx.world(), None).unwrap().is_none());
            apply(
                Action::ConfigureCouncil(native::encode(&policy).unwrap()),
                tx,
            )
            .unwrap();
            let mut substituted = envelope.clone();
            substituted.policy_digest[0] ^= 1;
            assert!(apply(Action::Admit(native::encode(&substituted).unwrap()), tx).is_err());
            assert!(
                native::read_head(
                    tx.world(),
                    Some(ProviderId::new(envelope.proposal.provider_id))
                )
                .unwrap()
                .is_none()
            );
        },
        true,
    );
}

#[test]
fn rollback_to_retained_head_is_rejected_and_full_history_preserves_emergency_revocation() {
    let mut f = Fixture::new();
    f.admit();
    let provider = f.provider();
    let revoke = f.revocation();
    f.commit(
        |tx| {
            // Exhausted ordinary history must never prevent a terminal revocation.
            tx.world.smart_contract_state.insert(
                native::path(None, AdmissionHistoryPathV1::HistoryBytes),
                native::encode(&PROVIDER_ADMISSION_HISTORY_MAX_BYTES_V1).unwrap(),
            );
            assert!(apply(Action::Revoke(native::encode(&revoke).unwrap()), tx).unwrap());
            let latest = tx
                .world
                .smart_contract_state
                .get(&native::path(Some(provider), AdmissionHistoryPathV1::Head))
                .unwrap()
                .clone();
            let original = tx
                .world
                .smart_contract_state
                .get(&native::path(
                    Some(provider),
                    AdmissionHistoryPathV1::Revision(1),
                ))
                .unwrap()
                .clone();
            tx.world.smart_contract_state.insert(
                native::path(Some(provider), AdmissionHistoryPathV1::Head),
                original,
            );
            assert!(native::read_head(tx.world(), Some(provider)).is_err());
            tx.world.smart_contract_state.insert(
                native::path(Some(provider), AdmissionHistoryPathV1::Head),
                latest,
            );
            assert!(
                native::read_head(tx.world(), Some(provider))
                    .unwrap()
                    .unwrap()
                    .revoked
            );
        },
        true,
    );
    assert!(
        native::read_finalized_provider_admission_v1(&f.state.view(), provider, NOW + 3)
            .unwrap()
            .is_none()
    );
}
