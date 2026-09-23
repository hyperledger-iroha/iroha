//! Exact DA proof and bounded-page response binding regressions.
use super::*;
use iroha_data_model::da::commitment::{DaCommitmentKey, DaCommitmentLocation};
use iroha_data_model::sorafs::pin_registry::ManifestDigest;
use iroha_model_base::topology::LaneId;

#[tokio::test]
async fn commitment_proof_checks_every_selector_and_location() {
    let proof = sample_da_commitment_proof();
    let query = commitment_query(&proof);
    for mutate in [
        |p: &mut DaCommitmentProof| p.commitment.manifest_hash = ManifestDigest::new([9; 32]),
        |p: &mut DaCommitmentProof| p.commitment.lane_id = LaneId::new(99),
        |p: &mut DaCommitmentProof| p.commitment.epoch += 1,
        |p: &mut DaCommitmentProof| p.commitment.sequence += 1,
        |p: &mut DaCommitmentProof| p.location.block_height = 0,
        |p: &mut DaCommitmentProof| p.location.index_in_bundle = p.bundle_len,
        |p: &mut DaCommitmentProof| p.bundle_len = 0,
    ] {
        let mut altered = proof.clone();
        mutate(&mut altered);
        let (client, requests, _) = attach(
            move |_| {
                Ok(json_response(&Some(DaCommitmentProofResponse {
                    policies: sample_da_proof_policy_bundle(),
                    proof: altered.clone(),
                })))
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .da()
                .prove_commitment(&query)
                .await,
            Err(Error::ResponseBinding { .. })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn pin_proof_checks_selectors_and_both_network_identities() {
    let network = client_with_base_url(base_url()).network_id;
    let mut proof = sample_da_pin_intent_proof(network);
    proof.intent.alias = Some("named".into());
    let query = pin_query(&proof);
    for mutate in [
        |p: &mut DaPinIntentProof| p.intent.manifest_hash = ManifestDigest::new([9; 32]),
        |p: &mut DaPinIntentProof| {
            p.intent.storage_ticket = iroha_data_model::da::types::StorageTicketId::new([9; 32])
        },
        |p: &mut DaPinIntentProof| p.intent.alias = None,
        |p: &mut DaPinIntentProof| p.intent.lane_id = LaneId::new(99),
        |p: &mut DaPinIntentProof| p.intent.epoch += 1,
        |p: &mut DaPinIntentProof| p.intent.sequence += 1,
        |p: &mut DaPinIntentProof| {
            p.intent.authorization.network_id =
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"wrong")))
        },
        |p: &mut DaPinIntentProof| {
            p.intent.pin_scope_authorization.scope.network_id =
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"wrong")))
        },
        |p: &mut DaPinIntentProof| p.location.block_height = 0,
        |p: &mut DaPinIntentProof| p.location.index_in_bundle = p.bundle_len,
    ] {
        let mut altered = proof.clone();
        mutate(&mut altered);
        let (client, _, _) = attach(
            move |_| Ok(json_response(&Some(altered.clone()))),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .da()
                .prove_pin_intent(&query)
                .await,
            Err(Error::ResponseBinding { .. })
        ));
    }
}

fn snapshot() -> DaListSnapshot {
    DaListSnapshot {
        block_height: 20,
        block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(b"snapshot"))),
    }
}

async fn commitments_response(
    query: &DaCommitmentListRequest,
    response: DaCommitmentListResponse,
) -> crate::Result<DaCommitmentListResponse> {
    let (client, _, _) = attach(
        move |_| Ok(json_response(&response)),
        Duration::ZERO,
        Duration::ZERO,
    );
    client.da().commitments(query).await
}

#[tokio::test]
async fn commitment_pages_reject_regressions_and_preserve_filtered_continuations() {
    let record = sample_da_commitment_with_location();
    let key = DaCommitmentKey::from_record(&record.commitment);
    let mut after = key;
    after.sequence -= 1;
    let cursor = DaCommitmentListCursor {
        snapshot: snapshot(),
        after,
    };
    let query = DaCommitmentListRequest {
        limit: NonZeroU64::new(1),
        cursor: Some(cursor),
    };
    let next = DaCommitmentListCursor {
        snapshot: snapshot(),
        after: key,
    };
    let valid = DaCommitmentListResponse {
        policies: sample_da_proof_policy_bundle(),
        commitments: vec![record],
        next_cursor: Some(next),
    };
    commitments_response(&query, valid.clone()).await.unwrap();
    let mut filtered = valid.clone();
    filtered.commitments.clear();
    commitments_response(&query, filtered).await.unwrap();
    for mutate in [
        |r: &mut DaCommitmentListResponse| r.commitments.push(r.commitments[0].clone()),
        |r: &mut DaCommitmentListResponse| r.commitments[0].commitment.sequence -= 1,
        |r: &mut DaCommitmentListResponse| r.commitments[0].location.block_height = 21,
        |r: &mut DaCommitmentListResponse| r.next_cursor.as_mut().unwrap().after.sequence -= 1,
        |r: &mut DaCommitmentListResponse| {
            r.next_cursor.as_mut().unwrap().snapshot.block_hash = None
        },
        |r: &mut DaCommitmentListResponse| {
            r.next_cursor.as_mut().unwrap().snapshot.block_height += 1
        },
    ] {
        let mut altered = valid.clone();
        mutate(&mut altered);
        assert!(matches!(
            commitments_response(&query, altered).await,
            Err(Error::ResponseBinding { .. })
        ));
    }
    let query = DaCommitmentListRequest {
        limit: NonZeroU64::new(2),
        cursor: None,
    };
    let mut duplicate = valid;
    duplicate.next_cursor = None;
    duplicate.commitments.push(duplicate.commitments[0].clone());
    assert!(matches!(
        commitments_response(&query, duplicate).await,
        Err(Error::ResponseBinding {
            field: "page_order",
            ..
        })
    ));
}

async fn pin_response(
    query: &DaPinIntentListRequest,
    response: DaPinIntentListResponse,
) -> crate::Result<DaPinIntentListResponse> {
    let (client, _, _) = attach(
        move |_| Ok(json_response(&response)),
        Duration::ZERO,
        Duration::ZERO,
    );
    client.da().pin_intents(query).await
}

#[tokio::test]
async fn pin_pages_bind_network_snapshot_limit_and_strict_block_order() {
    let network = client_with_base_url(base_url()).network_id;
    let record = sample_da_pin_intent_with_location(network);
    let cursor = DaPinIntentListCursor {
        snapshot: snapshot(),
        after: DaCommitmentLocation {
            block_height: 9,
            index_in_bundle: 0,
        },
    };
    let query = DaPinIntentListRequest {
        limit: NonZeroU64::new(1),
        cursor: Some(cursor),
    };
    let next = DaPinIntentListCursor {
        snapshot: snapshot(),
        after: record.location,
    };
    let valid = DaPinIntentListResponse {
        intents: vec![record],
        next_cursor: Some(next),
    };
    pin_response(&query, valid.clone()).await.unwrap();
    let mut filtered = valid.clone();
    filtered.intents.clear();
    pin_response(&query, filtered).await.unwrap();
    for mutate in [
        |r: &mut DaPinIntentListResponse| r.intents.push(r.intents[0].clone()),
        |r: &mut DaPinIntentListResponse| r.intents[0].location.block_height = 9,
        |r: &mut DaPinIntentListResponse| r.intents[0].location.block_height = 21,
        |r: &mut DaPinIntentListResponse| r.next_cursor.as_mut().unwrap().after.block_height = 9,
        |r: &mut DaPinIntentListResponse| {
            r.next_cursor.as_mut().unwrap().snapshot.block_hash = None
        },
        |r: &mut DaPinIntentListResponse| {
            r.next_cursor.as_mut().unwrap().snapshot.block_height += 1
        },
        |r: &mut DaPinIntentListResponse| {
            r.intents[0].intent.authorization.network_id =
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"wrong")))
        },
    ] {
        let mut altered = valid.clone();
        mutate(&mut altered);
        assert!(matches!(
            pin_response(&query, altered).await,
            Err(Error::ResponseBinding { .. })
        ));
    }
}

#[tokio::test]
async fn empty_pin_pages_reject_continuations_outside_the_snapshot() {
    let cursor = DaPinIntentListCursor {
        snapshot: snapshot(),
        after: DaCommitmentLocation {
            block_height: 9,
            index_in_bundle: 0,
        },
    };
    for input in [None, Some(cursor)] {
        let query = DaPinIntentListRequest {
            limit: None,
            cursor: input,
        };
        for block_height in [0, 21] {
            let response = DaPinIntentListResponse {
                intents: Vec::new(),
                next_cursor: Some(DaPinIntentListCursor {
                    after: DaCommitmentLocation {
                        block_height,
                        index_in_bundle: 0,
                    },
                    ..cursor
                }),
            };
            assert!(matches!(
                pin_response(&query, response).await,
                Err(Error::ResponseBinding {
                    field: "next_cursor",
                    ..
                })
            ));
        }
        pin_response(
            &query,
            DaPinIntentListResponse {
                intents: Vec::new(),
                next_cursor: Some(DaPinIntentListCursor {
                    after: DaCommitmentLocation {
                        block_height: 20,
                        index_in_bundle: 0,
                    },
                    ..cursor
                }),
            },
        )
        .await
        .unwrap();
    }
}
