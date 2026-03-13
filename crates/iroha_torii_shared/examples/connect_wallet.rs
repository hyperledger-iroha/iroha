//! Minimal Iroha Connect wallet-side example.
//!
//! Usage (demo):
//!   cargo run -p `iroha_torii_shared` --example `connect_wallet` -- \
//!     --node <http://127.0.0.1:8080> --sid <base64> --token <`token_wallet`>
//!
//! Notes: This demo uses placeholder ECDH keys for both sides for simplicity.

#[cfg(feature = "connect")]
use anyhow::Context;
#[cfg(feature = "connect")]
use base64::Engine as _;
#[cfg(feature = "connect")]
use blake2::Blake2bVar;
#[cfg(feature = "connect")]
use blake2::digest::{Update, VariableOutput};
#[cfg(feature = "connect")]
use futures_util::{SinkExt, StreamExt as _};
#[cfg(feature = "connect")]
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
#[cfg(feature = "connect")]
use iroha_crypto::{Algorithm, KeyGenOption, Signature};
#[cfg(feature = "connect")]
use iroha_torii_shared::connect as proto;
#[cfg(feature = "connect")]
use iroha_torii_shared::connect_sdk as sdk;
#[cfg(feature = "connect")]
use norito::codec::{DecodeAll as _, Encode as _};
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
#[cfg(feature = "connect")]
use tokio_tungstenite::tungstenite::{Bytes, Message};

#[cfg(feature = "connect")]
type WalletWebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(feature = "connect")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let _flag_node = args.next();
    let node = args
        .next()
        .unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let _flag_sid = args.next();
    let sid_b64 = args.next().context("--sid <base64>")?;
    let _flag_token = args.next();
    let token = args.next().context("--token <token_wallet>")?;
    // Optional: action after decrypting request: ok | reject | close
    let action = args.next().unwrap_or_else(|| "ok".into());

    let sid_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(sid_b64.as_bytes())
        .context("sid b64")?;
    if sid_bytes.len() != 32 {
        anyhow::bail!("sid must be 32 bytes");
    }
    let mut sid = [0u8; 32];
    sid.copy_from_slice(&sid_bytes);

    // Connect WS as wallet
    let ws_url = format!(
        "{}/v2/connect/ws?sid={}&role=wallet",
        node.replace("http", "ws"),
        sid_b64
    );
    let mut request = ws_url.into_client_request()?;
    request
        .headers_mut()
        .insert(AUTHORIZATION, format!("Bearer {token}").parse()?);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(request).await?;
    eprintln!("wallet: connected WS");

    // Derive deterministic demo ephemerals from sid so app/wallet interoperate
    let mut app_seed = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).unwrap();
    b2.update(b"connect:demo:app|");
    b2.update(&sid);
    b2.finalize_variable(&mut app_seed).unwrap();
    let mut wallet_seed = [0u8; 32];
    let mut b3 = Blake2bVar::new(32).unwrap();
    b3.update(b"connect:demo:wallet|");
    b3.update(&sid);
    b3.finalize_variable(&mut wallet_seed).unwrap();
    let x = X25519Sha256::new();
    let (app_pk, _app_sk) = x.keypair(KeyGenOption::UseSeed(app_seed.to_vec()));
    let (_wallet_pk, wallet_sk) = x.keypair(KeyGenOption::UseSeed(wallet_seed.to_vec()));
    let app_pk_bytes: [u8; 32] = *app_pk.as_bytes();
    let (k_app, k_wallet) = sdk::x25519_derive_keys(&wallet_sk.to_bytes(), &app_pk_bytes, &sid)
        .expect("x25519 derive keys");

    // Read one frame (expect SignRequestTx/Raw)
    let msg = ws.next().await.context("ws recv")??;
    let Message::Binary(bin) = msg else {
        anyhow::bail!("expected binary frame");
    };
    let mut cursor = bin.as_ref();
    let frame = proto::ConnectFrameV1::decode_all(&mut cursor).context("decode frame")?;
    let env = match &frame.kind {
        proto::FrameKind::Ciphertext(_) => {
            sdk::open_envelope_v1(&k_app, &frame).map_err(|e| anyhow::anyhow!(e))?
        }
        _ => anyhow::bail!("expected ciphertext"),
    };
    log_wallet_payload(&env);

    send_wallet_action(&mut ws, &k_wallet, &sid, action.as_str()).await?;
    Ok(())
}

// Fallback stub when `tokio-tungstenite` `connect` feature is not enabled.
#[cfg(not(feature = "connect"))]
fn main() {
    eprintln!("connect_wallet example requires `tokio-tungstenite` with `connect` feature");
}

#[cfg(feature = "connect")]
fn log_wallet_payload(env: &proto::EnvelopeV1) {
    match &env.payload {
        proto::ConnectPayloadV1::SignRequestTx { tx_bytes } => {
            eprintln!(
                "wallet: SignRequestTx len={} at seq {}",
                tx_bytes.len(),
                env.seq
            );
        }
        proto::ConnectPayloadV1::SignRequestRaw { domain_tag, bytes } => {
            eprintln!(
                "wallet: SignRequestRaw tag={} len={} at seq {}",
                domain_tag,
                bytes.len(),
                env.seq
            );
        }
        proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Close {
            who,
            code,
            reason,
            retryable,
        }) => {
            eprintln!(
                "wallet: got encrypted Close who={:?} code={} retryable={} reason='{}' at seq {}",
                who, code, retryable, reason, env.seq
            );
        }
        proto::ConnectPayloadV1::Control(proto::ControlAfterKeyV1::Reject {
            code,
            code_id,
            reason,
        }) => {
            eprintln!(
                "wallet: got encrypted Reject code={} id='{}' reason='{}' at seq {}",
                code, code_id, reason, env.seq
            );
        }
        other => {
            eprintln!("wallet: unexpected payload: {other:?}");
        }
    }
}

#[cfg(feature = "connect")]
async fn send_wallet_action(
    ws: &mut WalletWebSocket,
    k_wallet: &[u8; 32],
    sid: &[u8; 32],
    action: &str,
) -> anyhow::Result<()> {
    match action {
        "ok" => {
            let reply = proto::ConnectPayloadV1::SignResultOk {
                signature: proto::WalletSignatureV1::new(
                    Algorithm::Ed25519,
                    Signature::from_bytes(&[0xDE; 64]),
                ),
            };
            let frame = sdk::seal_envelope_v1(k_wallet, sid, proto::Dir::WalletToApp, 1, reply);
            ws.send(Message::Binary(Bytes::from(frame.encode())))
                .await?;
            eprintln!("wallet: sent SignResultOk");

            let close = sdk::encrypt_close_v1(
                k_wallet,
                sid,
                proto::Dir::WalletToApp,
                2,
                proto::Role::Wallet,
                1000,
                "done".into(),
                false,
            );
            ws.send(Message::Binary(Bytes::from(close.encode())))
                .await?;
            eprintln!("wallet: sent encrypted Close");
        }
        "reject" => {
            let rej = sdk::encrypt_reject_v1(
                k_wallet,
                sid,
                proto::Dir::WalletToApp,
                1,
                401,
                "UNAUTHORIZED".into(),
                "user denied".into(),
            );
            ws.send(Message::Binary(Bytes::from(rej.encode()))).await?;
            eprintln!("wallet: sent encrypted Reject");
        }
        "close" => {
            let close = sdk::encrypt_close_v1(
                k_wallet,
                sid,
                proto::Dir::WalletToApp,
                1,
                proto::Role::Wallet,
                1000,
                "done".into(),
                false,
            );
            ws.send(Message::Binary(Bytes::from(close.encode())))
                .await?;
            eprintln!("wallet: sent encrypted Close");
        }
        other => {
            eprintln!("wallet: unknown action '{other}', doing nothing");
        }
    }
    Ok(())
}
