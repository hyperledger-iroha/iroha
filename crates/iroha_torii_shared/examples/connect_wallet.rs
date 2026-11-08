//! Minimal Iroha Connect wallet-side example.
//!
//! Usage (demo):
//!   cargo run -p `iroha_torii_shared` --example `connect_wallet` -- \
//!     --node <http://127.0.0.1:8080> --sid <base64url> \
//!     --network-id <hash:...#....> --app-pk <base64url> --nonce <base64url> \
//!     --token <`token_wallet`> --relay <`token_relay`> \
//!     [--action ok|reject|close]
//!
//! Supply both seeds at runtime; never persist them in source, shell history, or a
//! checked-in configuration file:
//!
//! - `IROHA_CONNECT_ACCOUNT_SEED_HEX=<32-byte-ed25519-seed>`: the account's transaction key.
//!   It signs only the transaction payloads of `SignRequestTx`, after decoding them.
//! - `IROHA_CONNECT_SIGNING_SEED_HEX=<32-byte-ed25519-seed>`: the account's separate
//!   off-chain signing key, registered with `SetAccountSigningKeyV1`, and
//!   `IROHA_CONNECT_SIGNING_KEY_GENERATION=<u32>` its registered generation. It signs
//!   `SignRequestRaw` inside the off-chain envelope, never raw bytes, so a dApp cannot obtain
//!   a transaction signature through a raw request.
#[cfg(feature = "connect")]
use anyhow::Context;
#[cfg(feature = "connect")]
use base64::Engine as _;
#[cfg(feature = "connect")]
use futures_util::{SinkExt, StreamExt as _};
#[cfg(feature = "connect")]
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
#[cfg(feature = "connect")]
use iroha_crypto::{Algorithm, KeyGenOption, KeyPair, Signature};
#[cfg(feature = "connect")]
use iroha_data_model::{NetworkId, account::AccountId};
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
    let node = required_arg(&mut args, "--node")?;
    let sid_b64 = required_arg(&mut args, "--sid")?;
    let network_id_literal = required_arg(&mut args, "--network-id")?;
    let network_id: NetworkId = network_id_literal.parse()?;
    if network_id.to_string() != network_id_literal {
        anyhow::bail!("--network-id must use the canonical checksummed spelling");
    }
    let app_pk_b64 = required_arg(&mut args, "--app-pk")?;
    let nonce_b64 = required_arg(&mut args, "--nonce")?;
    let token = required_arg(&mut args, "--token")?;
    let relay_token = required_arg(&mut args, "--relay")?;
    let action = match args.next() {
        Some(flag) if flag == "--action" => args
            .next()
            .ok_or_else(|| anyhow::anyhow!("--action requires ok, reject, or close"))?,
        Some(other) => anyhow::bail!("unexpected argument `{other}`; expected --action"),
        None => "ok".into(),
    };
    if let Some(other) = args.next() {
        anyhow::bail!("unexpected trailing argument `{other}`");
    }
    let sid = decode_canonical_base64url::<32>(&sid_b64, "sid")?;
    let app_pk_bytes = decode_canonical_base64url::<32>(&app_pk_b64, "app_pk")?;
    let nonce = decode_canonical_base64url::<16>(&nonce_b64, "nonce")?;
    if app_pk_bytes.iter().all(|byte| *byte == 0) || nonce.iter().all(|byte| *byte == 0) {
        anyhow::bail!("app_pk and nonce must not be all zero");
    }
    if sdk::derive_session_id(&network_id, &app_pk_bytes, &nonce) != sid {
        anyhow::bail!("sid is not bound to the exact NetworkId, app_pk, and nonce");
    }
    let account_seed_hex = std::env::var("IROHA_CONNECT_ACCOUNT_SEED_HEX")
        .context("set runtime-only IROHA_CONNECT_ACCOUNT_SEED_HEX")?;
    let account_seed = hex::decode(account_seed_hex)?;
    if account_seed.len() != 32 || account_seed.iter().all(|byte| *byte == 0) {
        anyhow::bail!("account seed must be exactly 32 nonzero bytes");
    }
    let account_key_pair = KeyPair::try_from_seed(account_seed, Algorithm::Ed25519)?;
    let signing_seed = hex::decode(
        std::env::var("IROHA_CONNECT_SIGNING_SEED_HEX")
            .context("set runtime-only IROHA_CONNECT_SIGNING_SEED_HEX")?,
    )?;
    if signing_seed.len() != 32 || signing_seed.iter().all(|byte| *byte == 0) {
        anyhow::bail!("signing seed must be exactly 32 nonzero bytes");
    }
    let signing_key_pair = KeyPair::try_from_seed(signing_seed, Algorithm::Ed25519)?;
    if signing_key_pair.public_key() == account_key_pair.public_key() {
        anyhow::bail!("the off-chain signing key must differ from the account's transaction key");
    }
    let signing_key_generation: u32 = std::env::var("IROHA_CONNECT_SIGNING_KEY_GENERATION")
        .context("set IROHA_CONNECT_SIGNING_KEY_GENERATION to the registered generation")?
        .parse()
        .context("IROHA_CONNECT_SIGNING_KEY_GENERATION must be a decimal u32")?;
    let account = AccountId::new(account_key_pair.public_key().clone());
    let account_id = account.to_string();
    // Connect WS as wallet
    let ws_url = format!(
        "{}/v1/connect/ws?sid={}&role=wallet",
        node.replace("http", "ws"),
        sid_b64
    );
    let mut request = ws_url.into_client_request()?;
    request
        .headers_mut()
        .insert(AUTHORIZATION, format!("Bearer {token}").parse()?);
    let (mut ws, _resp) = tokio_tungstenite::connect_async(request).await?;
    eprintln!("wallet: connected WS");
    let open_message = ws.next().await.context("Open frame")??;
    let Message::Binary(open_bytes) = open_message else {
        anyhow::bail!("expected binary Open frame");
    };
    let mut open_cursor = open_bytes.as_ref();
    let open_frame = proto::ConnectFrameV1::decode_all(&mut open_cursor).context("decode Open")?;
    if open_frame.sid != sid || open_frame.dir != proto::Dir::AppToWallet || open_frame.seq != 1 {
        anyhow::bail!("Open substituted the session, direction, or sequence");
    }
    let proto::FrameKind::Control(proto::ConnectControlV1::Open {
        app_pk,
        constraints,
        permissions,
        ..
    }) = open_frame.kind
    else {
        anyhow::bail!("expected one-shot Open control");
    };
    if app_pk != app_pk_bytes || constraints.network_id != network_id {
        anyhow::bail!("Open does not match the canonical invite identity");
    }
    let x = X25519Sha256::new();
    let (wallet_pk, wallet_secret_key) = x.try_keypair(KeyGenOption::Random)?;
    let wallet_pk_bytes: [u8; 32] = *wallet_pk.as_bytes();
    let relay_auth = sdk::relay_auth_hash(&sid, &relay_token);
    let approval_preimage = sdk::build_approve_preimage(
        &constraints,
        &sid,
        &app_pk_bytes,
        &wallet_pk_bytes,
        &account_id,
        permissions.as_ref(),
        None,
        &relay_auth,
    );
    let approval = proto::ConnectFrameV1 {
        sid,
        dir: proto::Dir::WalletToApp,
        seq: 1,
        kind: proto::FrameKind::Control(proto::ConnectControlV1::Approve {
            wallet_pk: wallet_pk_bytes,
            account_id,
            permissions,
            proof: None,
            sig_wallet: proto::WalletSignatureV1::new(
                Algorithm::Ed25519,
                Signature::try_new(account_key_pair.private_key(), &approval_preimage)?,
            ),
        }),
    };
    ws.send(Message::Binary(Bytes::from(approval.encode())))
        .await?;
    let (k_app, k_wallet) =
        sdk::x25519_derive_keys(&wallet_secret_key.to_bytes(), &app_pk_bytes, &sid)
            .expect("x25519 derive keys");
    // Read one frame (expect SignRequestTx/Raw)
    let msg = ws.next().await.context("ws recv")??;
    let Message::Binary(bin) = msg else {
        anyhow::bail!("expected binary frame");
    };
    let mut cursor = bin.as_ref();
    let frame = proto::ConnectFrameV1::decode_all(&mut cursor).context("decode frame")?;
    if frame.sid != sid || frame.dir != proto::Dir::AppToWallet || frame.seq != 2 {
        anyhow::bail!("sign request substituted the session, direction, or sequence");
    }
    let env = match &frame.kind {
        proto::FrameKind::Ciphertext(_) => {
            sdk::open_envelope_current(&k_app, &frame).map_err(|e| anyhow::anyhow!(e))?
        }
        _ => anyhow::bail!("expected ciphertext"),
    };
    log_wallet_payload(&env);
    let keys = WalletKeys {
        network_id,
        account,
        transaction_key: account_key_pair,
        signing_key: signing_key_pair,
        signing_key_generation,
    };
    send_wallet_action(&mut ws, &k_wallet, &sid, &env, &keys, action.as_str()).await?;
    Ok(())
}
#[cfg(feature = "connect")]
fn decode_canonical_base64url<const N: usize>(value: &str, field: &str) -> anyhow::Result<[u8; N]> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .with_context(|| format!("{field} must be canonical unpadded base64url"))?;
    if base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes) != value {
        anyhow::bail!("{field} must use its canonical unpadded base64url spelling");
    }
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("{field} must decode to exactly {N} bytes"))
}
#[cfg(feature = "connect")]
fn required_arg(
    args: &mut impl Iterator<Item = String>,
    expected_flag: &'static str,
) -> anyhow::Result<String> {
    let supplied_flag = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing required {expected_flag} argument"))?;
    if supplied_flag != expected_flag {
        anyhow::bail!("expected {expected_flag}, found `{supplied_flag}`");
    }
    args.next()
        .ok_or_else(|| anyhow::anyhow!("{expected_flag} requires a value"))
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
/// The keys of the account the example wallet acts for.
#[cfg(feature = "connect")]
struct WalletKeys {
    network_id: NetworkId,
    account: AccountId,
    /// Signs only decoded transaction payloads.
    transaction_key: KeyPair,
    /// Signs every off-chain request, inside the off-chain envelope.
    signing_key: KeyPair,
    signing_key_generation: u32,
}
/// Sign one Connect signing request with the key its kind calls for.
#[cfg(feature = "connect")]
fn sign_request(
    keys: &WalletKeys,
    payload: &proto::ConnectPayloadV1,
) -> anyhow::Result<proto::WalletSignatureV1> {
    match payload {
        proto::ConnectPayloadV1::SignRequestTx { tx_bytes } => {
            // Decode the payload and sign its hash, exactly as a transaction signature; refuse
            // a payload of another authority instead of signing bytes the wallet cannot read.
            let builder =
                iroha_data_model::transaction::TransactionBuilder::decode_payload(tx_bytes)
                    .context("SignRequestTx must carry a canonical transaction payload")?;
            let signed = builder
                .try_sign(keys.transaction_key.private_key())
                .context("the transaction is not this account's to sign")?;
            Ok(proto::WalletSignatureV1::new(
                Algorithm::Ed25519,
                Signature::from(signed.signature().0.clone()),
            ))
        }
        proto::ConnectPayloadV1::SignRequestRaw { domain_tag, bytes } => {
            // TODO: a real wallet shows the request and, for the Torii canonical-request tag,
            // checks that `bytes` are a canonical request before signing.
            Ok(sdk::sign_raw_request(
                &keys.signing_key,
                &keys.network_id,
                &keys.account,
                keys.signing_key_generation,
                domain_tag,
                bytes,
            )?)
        }
        _ => anyhow::bail!("ok action requires a signing request"),
    }
}
#[cfg(feature = "connect")]
async fn send_wallet_action(
    ws: &mut WalletWebSocket,
    k_wallet: &[u8; 32],
    sid: &[u8; 32],
    request: &proto::EnvelopeV1,
    keys: &WalletKeys,
    action: &str,
) -> anyhow::Result<()> {
    match action {
        "ok" => {
            let reply = proto::ConnectPayloadV1::SignResultOk {
                signature: sign_request(keys, &request.payload)?,
            };
            let frame =
                sdk::seal_envelope_current(k_wallet, sid, proto::Dir::WalletToApp, 2, reply);
            ws.send(Message::Binary(Bytes::from(frame.encode())))
                .await?;
            eprintln!("wallet: sent SignResultOk");
            let close = sdk::encrypt_close_current(
                k_wallet,
                sid,
                proto::Dir::WalletToApp,
                3,
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
            let rej = sdk::encrypt_reject_current(
                k_wallet,
                sid,
                proto::Dir::WalletToApp,
                2,
                401,
                "UNAUTHORIZED".into(),
                "user denied".into(),
            );
            ws.send(Message::Binary(Bytes::from(rej.encode()))).await?;
            eprintln!("wallet: sent encrypted Reject");
        }
        "close" => {
            let close = sdk::encrypt_close_current(
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
        other => {
            eprintln!("wallet: unknown action '{other}', doing nothing");
        }
    }
    Ok(())
}
