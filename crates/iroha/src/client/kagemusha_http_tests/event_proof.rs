//! Original asynchronous and blocking event-path transport boundaries.

use super::*;
use iroha_crypto::{HashOf, MerkleProof};
use iroha_data_model::events::EventBox;

const EVENT_OP: &str = "kagemusha.wallet.load_event_proof.read";
const EVENT_MAX: usize = 8_192;

fn proof() -> MerkleProof<EventBox> {
    MerkleProof::from_audit_path(
        0,
        vec![Some(HashOf::from_untyped_unchecked(Hash::new(
            b"event sibling DATA",
        )))],
    )
}
fn reply(proof: &MerkleProof<EventBox>) -> Response<Vec<u8>> {
    Response::builder()
        .status(200)
        .header("content-type", "application/x-norito")
        .body(norito::encode_canonical(proof).unwrap())
        .unwrap()
}

#[tokio::test]
async fn event_path_signs_exact_account_network_route_and_preserves_original_data() {
    let initial = client_with_base_url(base_url());
    let expected = proof();
    let response = reply(&expected);
    let (client, requests, _) = attach(
        &initial,
        move |_| Ok(response.clone()),
        Duration::ZERO,
        Duration::ZERO,
    );
    let value = original(&client.account);
    for preference in [
        WireFormatPreference::JsonOnly,
        WireFormatPreference::NoritoOnly,
    ] {
        let mut builder = client.to_builder();
        builder.wire_format_preference = preference;
        let selected = builder.build().unwrap();
        let actual = selected
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await
            .unwrap();
        assert_eq!(actual, expected);
    }
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert_eq!(request.method, Method::GET);
        assert_eq!(
            request.url.path(),
            format!(
                "/v1/kagemusha/{}/wallets/{}/loads/{}/event-proof",
                hex::encode(value.scheme_id),
                hex::encode(value.wallet_id),
                hex::encode(REQUEST)
            )
        );
        assert!(request.url.query().is_none() && request.body.is_empty());
        assert_eq!(request.max_response_bytes, EVENT_MAX);
        assert_eq!(header(request, "accept"), "application/x-norito");
        assert_eq!(
            header(request, "x-iroha-account"),
            client.account.to_canonical_hex().unwrap()
        );
        assert!(
            !request
                .headers
                .iter()
                .any(|(name, _)| name.as_str() == "content-type")
        );
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
        let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign event network",
        )));
        let message = Client::exact_network_request_message(
            &foreign,
            &request.method,
            &request.url,
            &[],
            timestamp,
            nonce,
        )
        .unwrap();
        assert!(
            signature
                .verify(client.key_pair.public_key(), &message)
                .is_err()
        );
        let mut altered = request.url.clone();
        altered.set_path("/v1/kagemusha/foreign/event-proof");
        let message = Client::exact_network_request_message(
            &client.network_id,
            &request.method,
            &altered,
            &[],
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
    assert_ne!(
        header(&requests[0], "x-iroha-nonce"),
        header(&requests[1], "x-iroha-nonce")
    );
}

#[tokio::test]
async fn event_path_refuses_zero_scope_and_witness_before_dispatch() {
    let (client, requests, _) = attach(
        &client_with_base_url(base_url()),
        |_| unreachable!(),
        Duration::ZERO,
        Duration::ZERO,
    );
    let value = original(&client.account);
    for (scheme, wallet, request) in [
        ([0; 32], value.wallet_id, REQUEST),
        (value.scheme_id, [0; 32], REQUEST),
        (value.scheme_id, value.wallet_id, [0; 32]),
    ] {
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .kagemusha()
                .load_event_proof(&scheme, &wallet, &request)
                .await,
            Err(Error::InvalidRequest {
                operation: EVENT_OP,
                ..
            })
        ));
    }
    let mut builder = client.to_builder();
    builder
        .headers
        .insert("x-IrOhA-WiTnEsS".into(), "forbidden".into());
    let client = builder.build().unwrap();
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await,
        Err(Error::InvalidRequest {
            operation: EVENT_OP,
            ..
        })
    ));
    assert!(requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn event_path_refuses_noncanonical_media_frames_and_excess_depth() {
    let initial = client_with_base_url(base_url());
    let value = original(&initial.account);
    let mut replies = Vec::new();
    for types in [
        vec![],
        vec!["application/json"],
        vec!["application/x-norito", "application/x-norito"],
        vec!["application/x-norito, application/json"],
    ] {
        let mut response = reply(&proof());
        response.headers_mut().remove("content-type");
        for media in types {
            response
                .headers_mut()
                .append("content-type", media.parse().unwrap());
        }
        replies.push(response);
    }
    for bytes in [
        vec![],
        vec![0xff],
        [norito::encode_canonical(&proof()).unwrap(), vec![0]].concat(),
    ] {
        let mut response = reply(&proof());
        *response.body_mut() = bytes;
        replies.push(response);
    }
    replies.push(reply(&MerkleProof::from_audit_path(0, vec![None; 33])));
    for response in replies {
        let (client, requests, _) = attach(
            &initial,
            move |_| Ok(response.clone()),
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .kagemusha()
                .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
                .await,
            Err(Error::Decode {
                operation: EVENT_OP,
                ..
            } | Error::CanonicalDecode {
                operation: EVENT_OP,
                ..
            })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn event_path_preserves_status_transport_and_capacity_without_retry() {
    let initial = client_with_base_url(base_url());
    let value = original(&initial.account);
    for status in [401, 403, 404, 429, 503] {
        let (client, requests, _) = attach(
            &initial,
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .header("retry-after", "7")
                    .body(b"unavailable-path".to_vec())
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        let error = client
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await
            .unwrap_err();
        assert!(
            matches!(error, Error::Http { operation: EVENT_OP, status: actual, body, .. } if actual == status && body == b"unavailable-path")
        );
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    let (client, requests, _) = attach(
        &initial,
        |_| Err(std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into()),
        Duration::ZERO,
        Duration::ZERO,
    );
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await,
        Err(Error::Transport {
            operation: EVENT_OP,
            kind: TransportErrorKind::Io(std::io::ErrorKind::ConnectionRefused),
            ..
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
    for status in [200, 503] {
        let (client, requests, _) = attach(
            &initial,
            move |_| {
                Ok(Response::builder()
                    .status(status)
                    .body(vec![0; EVENT_MAX + 1])
                    .unwrap())
            },
            Duration::ZERO,
            Duration::ZERO,
        );
        assert!(matches!(
            client
                .account_client()
                .unwrap()
                .kagemusha()
                .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
                .await,
            Err(Error::ResponseTooLarge {
                maximum: EVENT_MAX,
                ..
            })
        ));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn event_path_obeys_deadline_and_cancels_pending_dispatch() {
    let initial = client_with_base_url(base_url());
    let value = original(&initial.account);
    let response = reply(&proof());
    let (client, requests, completed) = attach(
        &initial,
        move |_| Ok(response.clone()),
        Duration::from_secs(60),
        Duration::from_millis(10),
    );
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await,
        Err(Error::Timeout {
            operation: EVENT_OP
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
    let client =
        client.with_request_deadline(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    assert!(matches!(
        client
            .account_client()
            .unwrap()
            .kagemusha()
            .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
            .await,
        Err(Error::Timeout {
            operation: EVENT_OP
        })
    ));
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn blocking_event_path_reuses_runtime_and_refuses_nested_async_entry() {
    let initial = client_with_base_url(base_url());
    let value = original(&initial.account);
    let response = reply(&proof());
    let (client, requests, _) = attach(
        &initial,
        move |_| Ok(response.clone()),
        Duration::ZERO,
        Duration::ZERO,
    );
    let account = blocking::AccountClient::from_client(client.account_client().unwrap()).unwrap();
    for current in [account.clone(), account.clone()] {
        assert_eq!(
            current
                .kagemusha()
                .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST)
                .unwrap(),
            proof()
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert!(matches!(
            account
                .kagemusha()
                .load_event_proof(&value.scheme_id, &value.wallet_id, &REQUEST),
            Err(Error::Blocking(_))
        ));
        drop(account);
    });
    assert_eq!(requests.lock().unwrap().len(), 2);
}
