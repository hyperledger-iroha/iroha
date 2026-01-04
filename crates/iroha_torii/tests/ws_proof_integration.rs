//! Integration (ignored) test: start a WS endpoint, subscribe, and assert Proof JSON frames.
#![cfg(feature = "app_api")]

#[path = "common/proof_events.rs"]
mod proof_events;

use std::io::ErrorKind;

use axum::{Router, routing::get};
use futures_util::{SinkExt as _, StreamExt as _};
use proof_events::ProofEventFixture;
use tokio::net::TcpListener;

#[cfg(feature = "ws_integration_tests")]
#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn ws_proof_json_integration() {
    // Setup broadcast sender and WS route
    let events: iroha_core::EventsSender = tokio::sync::broadcast::channel(16).0;
    let app = Router::new().route(
        "/ws",
        get({
            let events = events.clone();
            move |ws: axum::extract::ws::WebSocketUpgrade| async move {
                ws.on_upgrade(move |ws| async move {
                    let _ = iroha_torii::handle_events_stream(events, ws).await;
                })
            }
        }),
    );
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => return,
        Err(e) => panic!("tcp bind failed: {e}"),
    };
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    // Connect client
    let (mut ws_stream, _resp) =
        match tokio_tungstenite::connect_async(format!("ws://{addr}/ws")).await {
            Ok(pair) => pair,
            Err(tokio_tungstenite::tungstenite::Error::Io(io_err))
                if io_err.kind() == ErrorKind::PermissionDenied =>
            {
                return;
            }
            Err(e) => panic!("ws connect failed: {e}"),
        };

    // Send EventSubscriptionRequest (Any + proof backend filter)
    let sub = iroha_data_model::events::stream::EventSubscriptionRequest {
        filters: vec![iroha_data_model::events::EventFilterBox::Data(
            iroha_data_model::prelude::DataEventFilter::Any,
        )],
        proof_backend: Some(vec!["halo2/ipa".into()]),
        proof_call_hash: None,
        proof_envelope_hash: None,
    };
    let sub_bytes = <_ as norito::codec::Encode>::encode(&sub);
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            sub_bytes.into(),
        ))
        .await
        .unwrap();

    // Send events (one rejected backend; one verified matching backend)
    let ev_bad = ProofEventFixture::new("groth16", [0x20; 32])
        .without_vk()
        .verified();
    events
        .send(ev_bad)
        .expect("events stream subscriber to be ready for non-matching backend");

    let ev_ok = ProofEventFixture::new("halo2/ipa", [0x21; 32])
        .with_vk("vk", [0x55; 32])
        .with_envelope_hash(Some([0x20; 32]))
        .verified();
    events
        .send(ev_ok)
        .expect("events stream subscriber to be ready for matching backend");

    // Read frames until we get a Text JSON (the first one might be filtered internally)
    let mut got_json = None;
    while let Some(msg) = ws_stream.next().await {
        if let tokio_tungstenite::tungstenite::Message::Text(s) = msg.unwrap() {
            got_json = Some(s);
            break;
        }
    }
    let s = got_json.expect("text json message");
    let v: norito::json::Value = norito::json::from_str(&s).expect("json parse");
    assert_eq!(
        v.get("event").and_then(|x| x.as_str()),
        Some("ProofVerified")
    );
    assert_eq!(v.get("backend").and_then(|x| x.as_str()), Some("halo2/ipa"));
}
