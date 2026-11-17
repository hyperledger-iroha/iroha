//! Exact signed State-row transport and response-binding regressions.

use super::*;
use crate::{
    client::{
        capability_test_support::AsyncOnlyTransport,
        evidence_http_tests::{base_url, client_with_base_url},
    },
    http::{Response, TransportRequest},
};
use base64::Engine as _;
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::sumeragi_finality::KeyedStateRowV1;
use std::{
    sync::{Arc, Mutex, atomic::AtomicUsize},
    time::Duration,
};

fn position() -> StateKeyV1 {
    StateKeyV1 {
        table: "world.runtime_selection".into(),
        key: vec![],
    }
}

fn original() -> KeyedStateRowsV1 {
    KeyedStateRowsV1 {
        height: 2,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"transport-only")),
        schema_hash: [1; 32],
        rows: vec![KeyedStateRowV1 {
            position: position(),
            value: None,
            // Only transport structure: native verification independently checks the root.
            witness: norito::json::from_str(r#"{"terminal":null,"steps":[]}"#).unwrap(),
        }],
    }
}

fn attach(original: &KeyedStateRowsV1) -> (Client, Arc<Mutex<Vec<TransportRequest>>>) {
    let response = Response::builder()
        .status(200)
        .header("content-type", "application/x-norito")
        .body(norito::encode_canonical(original).unwrap())
        .unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(move |_| Ok(response.clone())),
        requests: requests.clone(),
        completed: Arc::new(AtomicUsize::new(0)),
        delay: Duration::ZERO,
    });
    let client = client_with_base_url(base_url())
        .to_builder()
        .http_transport(transport)
        .build()
        .unwrap();
    (client, requests)
}

fn header<'a>(request: &'a TransportRequest, name: &str) -> &'a str {
    let values: Vec<_> = request
        .headers
        .iter()
        .filter(|(key, _)| key.as_str() == name)
        .collect();
    assert_eq!(values.len(), 1);
    values[0].1.to_str().unwrap()
}

#[tokio::test]
async fn state_rows_signs_exact_json_post_and_preserves_absence_for_native_verification() {
    let expected = original();
    let (client, requests) = attach(&expected);
    let actual = client
        .account_client()
        .unwrap()
        .state_rows(2, &[position()])
        .await
        .unwrap();
    assert_eq!(
        norito::encode_canonical(&actual).unwrap(),
        norito::encode_canonical(&expected).unwrap()
    );
    assert!(actual.rows[0].value.is_none());
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.url.path(), "/v1/ledger/state-proof/2");
    assert!(request.url.query().is_none());
    assert_eq!(
        request.body,
        br#"{"rows":[{"table":"world.runtime_selection","key_hex":""}]}"#
    );
    assert_eq!(request.max_response_bytes, MAX_STATE_ROWS_FRAME_BYTES_V1);
    assert_eq!(header(request, "content-type"), "application/json");
    assert_eq!(header(request, "accept"), "application/x-norito");
    let signature = Signature::try_from_bytes(
        &base64::engine::general_purpose::STANDARD
            .decode(header(request, "x-iroha-signature"))
            .unwrap(),
    )
    .unwrap();
    let timestamp = header(request, "x-iroha-timestamp-ms").parse().unwrap();
    let nonce = header(request, "x-iroha-nonce");
    let message = Client::exact_network_request_message(
        &client.network_id,
        &request.method,
        &request.url,
        &request.body,
        timestamp,
        nonce,
    )
    .unwrap();
    signature
        .verify(client.key_pair.public_key(), &message)
        .unwrap();
    let message = Client::exact_network_request_message(
        &client.network_id,
        &request.method,
        &request.url,
        b"{}",
        timestamp,
        nonce,
    )
    .unwrap();
    assert!(
        signature
            .verify(client.key_pair.public_key(), &message)
            .is_err()
    );
}

#[tokio::test]
async fn state_rows_refuses_wrong_height_key_set_and_json_fallback() {
    let (client, _) = attach(&original());
    let account = client.account_client().unwrap();
    assert!(matches!(
        account.state_rows(3, &[position()]).await,
        Err(Error::ResponseBinding { .. })
    ));
    let other = StateKeyV1 {
        table: "world.assets".into(),
        key: vec![1],
    };
    assert!(matches!(
        account.state_rows(2, &[other]).await,
        Err(Error::ResponseBinding { .. })
    ));
    // The response is a typed canonical native projection, never an ad-hoc JSON fallback.
    let (client, _) = attach(&original());
    let transport = Arc::new(AsyncOnlyTransport {
        responder: Box::new(|_| {
            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(b"{}".to_vec())
                .unwrap())
        }),
        requests: Arc::new(Mutex::new(Vec::new())),
        completed: Arc::new(AtomicUsize::new(0)),
        delay: Duration::ZERO,
    });
    let client = client
        .to_builder()
        .http_transport(transport)
        .build()
        .unwrap();
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .state_rows(2, &[position()])
            .await,
        Err(Error::Decode { .. })
    ));
}

#[test]
fn selections_refuse_bad_bounds_tables_order_and_duplicates() {
    assert!(selection_body(1, &[position()]).is_err());
    assert!(selection_body(2, &[]).is_err());
    assert!(selection_body(2, &[position(), position()]).is_err());
    assert!(
        selection_body(
            2,
            &[StateKeyV1 {
                table: "world/invalid".into(),
                key: vec![]
            }]
        )
        .is_err()
    );
    assert!(
        selection_body(
            2,
            &[StateKeyV1 {
                table: "world.assets".into(),
                key: vec![0; 4097]
            }]
        )
        .is_err()
    );
    let rows: Vec<_> = (0..32)
        .map(|i| StateKeyV1 {
            table: "world.assets".into(),
            key: vec![i; 4096],
        })
        .collect();
    assert!(selection_body(2, &rows).is_err());
}
