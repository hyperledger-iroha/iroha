//! Minimal Iroha Connect app-side example.
//!
//! - Creates a session via Torii POST /v2/connect/session
//! - Connects to WS with Authorization bearer token as app role
//! - Uses `connect_sdk` to seal a `SignRequestTx` payload and send as a frame
//! - Optional: send encrypted Close/Reject instead via --action close|reject
//!
//! Build (example only):
//!   cargo run -p `iroha_torii_shared` --example `connect_app` -- \
//!     --node <http://127.0.0.1:8080> --role app [--action ok|reject|close]

#[cfg(feature = "connect")]
use base64::Engine;
#[cfg(feature = "connect")]
use blake2::Blake2bVar;
#[cfg(feature = "connect")]
use blake2::digest::{Update, VariableOutput};
#[cfg(feature = "connect")]
use futures_util::{SinkExt, StreamExt as _};
#[cfg(feature = "connect")]
use iroha_crypto::KeyGenOption;
#[cfg(feature = "connect")]
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
#[cfg(feature = "connect")]
use iroha_torii_shared::connect as proto;
#[cfg(feature = "connect")]
use iroha_torii_shared::connect_sdk as sdk;
#[cfg(feature = "connect")]
use norito::codec::DecodeAll as _;
#[cfg(feature = "connect")]
use norito::codec::Encode as _;
#[cfg(feature = "connect")]
use norito::derive::JsonDeserialize;
#[cfg(feature = "connect")]
use norito::json;
#[cfg(feature = "connect")]
use reqwest::Client;
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::{Bytes, Message};

#[cfg(feature = "connect")]
#[derive(JsonDeserialize)]
struct SessionResp {
    sid: String,
    token_app: String,
    token_wallet: String,
}

#[cfg(feature = "connect")]
type AppWebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(feature = "connect")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    run_connect_app().await
}

#[cfg(feature = "connect")]
#[allow(clippy::future_not_send)]
async fn run_connect_app() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let _flag_node = args.next();
    let node = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let _flag_role = args.next();
    let role = args.next().unwrap_or_else(|| "app".into());
    let _flag_action = args.next();
    let action = args.next().unwrap_or_else(|| "ok".into());
    let client = Client::new();

    let SessionResp {
        sid,
        token_app,
        token_wallet,
        ..
    } = request_session(&client, &node).await?;
    println!("sid={sid} token_app={token_app} token_wallet={token_wallet}");

    let ws_url = format!(
        "{}/v2/connect/ws?sid={sid}&role={role}",
        node.replace("http", "ws")
    );
    let mut request = ws_url.into_client_request()?;
    request
        .headers_mut()
        .insert(AUTHORIZATION, format!("Bearer {token_app}").parse()?);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(request).await?;
    println!("WS connected");

    let sid_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(sid.as_bytes())
        .unwrap();
    let mut sid_arr = [0u8; 32];
    sid_arr.copy_from_slice(&sid_bytes);
    let mut app_seed = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).unwrap();
    b2.update(b"connect:demo:app|");
    b2.update(&sid_arr);
    b2.finalize_variable(&mut app_seed).unwrap();
    let mut wallet_seed = [0u8; 32];
    let mut b3 = Blake2bVar::new(32).unwrap();
    b3.update(b"connect:demo:wallet|");
    b3.update(&sid_arr);
    b3.finalize_variable(&mut wallet_seed).unwrap();
    let x = X25519Sha256::new();
    let (_app_pk, app_sk) = x.keypair(KeyGenOption::UseSeed(app_seed.to_vec()));
    let (wallet_pk, _wallet_sk) = x.keypair(KeyGenOption::UseSeed(wallet_seed.to_vec()));
    let wallet_pk_bytes: [u8; 32] = *wallet_pk.as_bytes();
    let (k_app, k_wallet) = sdk::x25519_derive_keys(&app_sk.to_bytes(), &wallet_pk_bytes, &sid_arr)
        .expect("x25519 derive keys");

    send_app_action(&mut ws, &k_app, &sid_arr, action.as_str()).await?;

    if let Some(Ok(Message::Binary(bin))) = ws.next().await {
        let mut cursor = bin.as_ref();
        if let Ok(frame) = proto::ConnectFrameV1::decode_all(&mut cursor)
            && let Ok(env) = sdk::open_envelope_current(&k_wallet, &frame)
        {
            log_app_response(&env);
        }
    }

    Ok(())
}

#[cfg(feature = "connect")]
async fn request_session(client: &Client, node: &str) -> anyhow::Result<SessionResp> {
    let url = format!("{node}/v2/connect/session");
    let body = client
        .post(url)
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let resp = json::from_slice(body.as_ref())?;
    Ok(resp)
}

#[cfg(feature = "connect")]
async fn send_app_action(
    ws: &mut AppWebSocket,
    k_app: &[u8; 32],
    sid: &[u8; 32],
    action: &str,
) -> anyhow::Result<()> {
    match action {
        "ok" => {
            let payload = proto::ConnectPayloadV1::SignRequestTx {
                tx_bytes: b"...".to_vec(),
            };
            let frame = sdk::seal_envelope_current(k_app, sid, proto::Dir::AppToWallet, 1, payload);
            ws.send(Message::Binary(Bytes::from(frame.encode())))
                .await?;
            println!("app: sent SignRequestTx");
        }
        "reject" => {
            let rej = sdk::encrypt_reject_current(
                k_app,
                sid,
                proto::Dir::AppToWallet,
                1,
                401,
                "UNAUTHORIZED".into(),
                "app rejected".into(),
            );
            ws.send(Message::Binary(Bytes::from(rej.encode()))).await?;
            println!("app: sent encrypted Reject");
        }
        "close" => {
            let close = sdk::encrypt_close_current(
                k_app,
                sid,
                proto::Dir::AppToWallet,
                1,
                proto::Role::App,
                1000,
                "done".into(),
                false,
            );
            ws.send(Message::Binary(Bytes::from(close.encode())))
                .await?;
            println!("app: sent encrypted Close");
        }
        other => {
            println!("app: unknown action '{other}', defaulting to ok");
            let payload = proto::ConnectPayloadV1::SignRequestTx {
                tx_bytes: b"...".to_vec(),
            };
            let frame = sdk::seal_envelope_current(k_app, sid, proto::Dir::AppToWallet, 1, payload);
            ws.send(Message::Binary(Bytes::from(frame.encode())))
                .await?;
        }
    }
    Ok(())
}

#[cfg(feature = "connect")]
fn log_app_response(env: &proto::EnvelopeV1) {
    match &env.payload {
        proto::ConnectPayloadV1::SignResultOk { signature } => {
            let sig_hex = hex::encode(signature.bytes());
            println!(
                "app: got SignResultOk algo={} sig={sig_hex}",
                signature.algorithm.as_static_str()
            );
        }
        proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close {
            who,
            code,
            reason,
            retryable,
        }) => {
            println!(
                "app: got encrypted Close who={who:?} code={code} retryable={retryable} reason='{reason}'"
            );
        }
        proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Reject {
            code,
            code_id,
            reason,
        }) => {
            println!("app: got encrypted Reject code={code} id='{code_id}' reason='{reason}'");
        }
        other => {
            println!("app: got unexpected payload: {other:?}");
        }
    }
}

// Fallback stub when `tokio-tungstenite` `connect` feature is not enabled.
#[cfg(not(feature = "connect"))]
fn main() {
    eprintln!("connect_app example requires `tokio-tungstenite` with `connect` feature");
}
