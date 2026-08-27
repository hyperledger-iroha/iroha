//! Argon2 puzzle issuance microservice backing the SoraNet relay handshake.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use blake3::hash as blake3_hash;
use clap::Parser;
use color_eyre::eyre::{Context, Result, eyre};
use hex::{decode, encode};
use iroha_crypto::soranet::{
    pow::{SignedTicket, Ticket as PowTicket},
    puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
    token::{AdmissionToken, MintError as AdmissionTokenMintError, compute_issuer_fingerprint},
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
use soranet_pq::MlDsaSuite;
use soranet_relay::config::{
    ConfigError as RelayConfigError, HandshakePolicy, PowConfig, PrivateFileBytes, RelayConfig,
    read_bounded_direct_regular_file, read_bounded_private_regular_file,
};
use soranet_relay::token_tool::REVOCATION_LIST_MAX_ENTRIES_V1;
use std::{
    collections::HashSet,
    fmt,
    future::Future,
    net::SocketAddr,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    net::TcpListener,
    signal,
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt::SubscriberBuilder};
const REVOCATION_FILE_MAX_BYTES_V1: usize = 4 * 1024 * 1024;
const MINT_AUTH_TOKEN_FILE_MAX_BYTES_V1: usize = 258;
const MINT_REQUEST_MAX_BYTES_V1: usize = 4 * 1024;
const MAX_CONCURRENT_MINT_REQUESTS_V1: usize = 4;
fn clear_sensitive_vec(value: &mut Vec<u8>) {
    value.resize(value.capacity(), 0);
    zeroize::Zeroize::zeroize(value.as_mut_slice());
    value.clear();
}
fn read_bounded_utf8_file(path: &Path, maximum: usize, artifact: &str) -> std::io::Result<String> {
    let bytes = read_bounded_direct_regular_file(path, maximum, artifact)?;
    String::from_utf8(bytes).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{artifact} is not valid UTF-8: {error}"),
        )
    })
}
fn read_bounded_private_file(
    path: &Path,
    maximum: usize,
    artifact: &str,
) -> std::io::Result<PrivateFileBytes> {
    read_bounded_private_regular_file(path, maximum, artifact)
}
fn decode_exact_hex_bytes(
    value: &str,
    expected_bytes: usize,
    artifact: &str,
) -> Result<Vec<u8>, String> {
    let expected_hex_bytes = expected_bytes
        .checked_mul(2)
        .ok_or_else(|| format!("{artifact} encoded length overflows the platform address space"))?;
    if value.len() != expected_hex_bytes {
        return Err(format!(
            "{artifact} must contain exactly {expected_hex_bytes} hexadecimal characters; found {}",
            value.len()
        ));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(expected_bytes)
        .map_err(|_| format!("failed to reserve the bounded {artifact} buffer"))?;
    decoded.resize(expected_bytes, 0);
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|error| format!("failed to decode {artifact} as hexadecimal: {error}"))?;
    Ok(decoded)
}
fn decode_mldsa_public_key_hex(
    value: &str,
    suite: MlDsaSuite,
    artifact: &str,
) -> Result<Vec<u8>, String> {
    let key = decode_exact_hex_bytes(value, suite.public_key_len(), artifact)?;
    suite
        .validate_public_key(&key)
        .map_err(|error| format!("{artifact} is not valid {suite:?} material: {error}"))?;
    Ok(key)
}
fn secret_file_max_bytes(expected_secret_bytes: usize) -> Result<usize, String> {
    expected_secret_bytes
        .checked_mul(2)
        .and_then(|hex_bytes| hex_bytes.checked_add(1))
        .ok_or_else(|| "secret-key file limit overflows the platform address space".to_owned())
}
struct SensitiveBytes(Vec<u8>);
impl SensitiveBytes {
    fn clear(&mut self) {
        clear_sensitive_vec(&mut self.0);
    }
}
impl Deref for SensitiveBytes {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}
impl AsRef<[u8]> for SensitiveBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl From<Vec<u8>> for SensitiveBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}
impl fmt::Debug for SensitiveBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        self.clear();
    }
}
fn clear_sensitive_string(value: &mut String) {
    // Taking the allocation as bytes avoids an unsafe mutable UTF-8 view. Grow
    // only to the existing capacity so truncated credentials cannot survive in
    // spare storage, then erase it with volatile writes before release.
    let mut bytes = core::mem::take(value).into_bytes();
    bytes.resize(bytes.capacity(), 0);
    zeroize::Zeroize::zeroize(bytes.as_mut_slice());
    bytes.clear();
}
fn decode_private_hex_bytes(
    raw: &mut PrivateFileBytes,
    expected_bytes: usize,
    artifact: &str,
) -> Result<SensitiveBytes, String> {
    let parsed = (|| {
        let expected_hex_bytes = expected_bytes.checked_mul(2).ok_or_else(|| {
            format!("{artifact} encoded length overflows the platform address space")
        })?;
        if raw.len() != expected_hex_bytes
            || !raw
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(format!(
                "{artifact} must contain exactly {expected_hex_bytes} lowercase hexadecimal characters with no whitespace"
            ));
        }
        let mut decoded = Vec::new();
        decoded
            .try_reserve_exact(expected_bytes)
            .map_err(|_| format!("failed to reserve the bounded {artifact} buffer"))?;
        decoded.resize(expected_bytes, 0);
        let mut decoded = SensitiveBytes(decoded);
        hex::decode_to_slice(&*raw, &mut decoded.0)
            .map_err(|_| format!("failed to decode {artifact} as hexadecimal"))?;
        if decoded.iter().all(|byte| *byte == 0) {
            return Err(format!("{artifact} must not be the all-zero value"));
        }
        Ok(decoded)
    })();
    raw.clear();
    parsed
}
#[derive(Parser, Debug)]
#[command(
    name = "soranet-puzzle-service",
    about = "Argon2 puzzle issuance microservice"
)]
struct Args {
    /// Path to the relay configuration JSON file.
    #[arg(long)]
    config: PathBuf,
    /// Address to listen on (host:port).
    #[arg(long, default_value = "127.0.0.1:8088")]
    listen: SocketAddr,
    /// Log level (e.g. info, debug).
    #[arg(long, default_value = "info")]
    log_level: String,
    /// Path to the private bearer token required by credential-minting endpoints.
    #[arg(long)]
    mint_auth_token_path: PathBuf,
    /// Path to file containing hex-encoded ML-DSA issuer secret key.
    #[arg(long)]
    token_secret_path: Option<PathBuf>,
    /// Path to external revocation list (newline-separated hex token IDs).
    #[arg(long)]
    token_revocation_file: Option<PathBuf>,
    /// Refresh interval (seconds) for the revocation file when supplied.
    #[arg(long, default_value_t = 30)]
    token_revocation_refresh_secs: u64,
    /// Path to the ML-DSA secret key file for signing mandatory Argon2 tickets.
    #[arg(long)]
    signed_ticket_secret_path: Option<PathBuf>,
}
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    validate_listen_address(args.listen)?;
    init_tracing(&args.log_level)?;
    let service = PuzzleService::new(&args)?;
    let state = Arc::new(service);
    let mint_authorization = Arc::new(MintAuthorization::load(&args.mint_auth_token_path)?);
    let protected_routes = Router::new()
        .route("/v1/puzzle/mint", post(mint_ticket))
        .route("/v1/token/config", get(get_token_config))
        .route("/v1/token/mint", post(mint_token))
        .layer(DefaultBodyLimit::max(MINT_REQUEST_MAX_BYTES_V1))
        .route_layer(middleware::from_fn_with_state(
            mint_authorization,
            authorize_mint_request,
        ));
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/puzzle/config", get(get_config))
        .merge(protected_routes)
        .with_state(state);
    let listener = TcpListener::bind(args.listen)
        .await
        .wrap_err_with(|| format!("failed to bind {addr}", addr = args.listen))?;
    info!(listen = %args.listen, "starting puzzle service");
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .wrap_err("server error")?;
    info!("puzzle service shutdown complete");
    Ok(())
}
fn validate_listen_address(listen: SocketAddr) -> Result<()> {
    if !listen.ip().is_loopback() {
        return Err(eyre!(
            "puzzle-service must listen on a loopback address because mint authorization uses plaintext HTTP; terminate TLS at a local proxy"
        ));
    }
    Ok(())
}
fn init_tracing(level: &str) -> Result<()> {
    SubscriberBuilder::default()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)),
        )
        .with_target(false)
        .init();
    Ok(())
}
async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        warn!(%error, "failed waiting for ctrl-c");
    }
}
struct MintAuthorization {
    token_hash: [u8; 32],
    permits: Arc<Semaphore>,
}
impl MintAuthorization {
    fn load(path: &Path) -> Result<Self> {
        let mut bytes = read_bounded_private_regular_file(
            path,
            MINT_AUTH_TOKEN_FILE_MAX_BYTES_V1,
            "SoraNet puzzle-service mint authorization token",
        )
        .wrap_err_with(|| {
            format!(
                "failed to read private mint authorization token from {}",
                path.display()
            )
        })?;
        let token_hash = Self::hash_token(&bytes);
        bytes.clear();
        Ok(Self {
            token_hash: token_hash?,
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_MINT_REQUESTS_V1)),
        })
    }
    fn hash_token(bytes: &[u8]) -> Result<[u8; 32]> {
        let token = std::str::from_utf8(bytes)
            .map_err(|_| eyre!("mint authorization token must be valid UTF-8"))?
            .trim_end_matches(['\r', '\n']);
        if !(32..=256).contains(&token.len()) {
            return Err(eyre!(
                "mint authorization token must contain 32 to 256 bytes"
            ));
        }
        if !token.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(eyre!(
                "mint authorization token must contain only printable non-whitespace ASCII"
            ));
        }
        Ok(Self::hash_sensitive_token(token.as_bytes()))
    }
    fn matches(&self, candidate: &str) -> bool {
        let mut candidate_hash = Self::hash_sensitive_token(candidate.as_bytes());
        let matches = constant_time_eq_32(&self.token_hash, &candidate_hash);
        zeroize::Zeroize::zeroize(&mut candidate_hash);
        matches
    }
    fn hash_sensitive_token(token: &[u8]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(token);
        let mut digest = hasher.finalize();
        let output = *digest.as_bytes();
        zeroize::Zeroize::zeroize(&mut digest);
        zeroize::Zeroize::zeroize(&mut hasher);
        output
    }
    fn clear_token_hash(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.token_hash);
    }
}
fn constant_time_eq_32(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for index in 0..32 {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}
impl fmt::Debug for MintAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MintAuthorization")
            .field("token_hash", &"<redacted>")
            .field("available_permits", &self.permits.available_permits())
            .finish_non_exhaustive()
    }
}
impl Drop for MintAuthorization {
    fn drop(&mut self) {
        self.clear_token_hash();
    }
}
fn request_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    let value = value.to_str().ok()?;
    if !(39..=263).contains(&value.len()) {
        return None;
    }
    let mut parts = value.split_ascii_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || !(32..=256).contains(&token.len())
        || !token.bytes().all(|byte| byte.is_ascii_graphic())
        || parts.next().is_some()
    {
        return None;
    }
    Some(token)
}
fn has_single_json_content_type(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}
fn authenticate_and_remove_bearer(
    headers: &mut HeaderMap,
    authorization: &MintAuthorization,
) -> bool {
    let authorized =
        request_bearer_token(headers).is_some_and(|candidate| authorization.matches(candidate));
    // HeaderValue does not expose mutable storage that can be scrubbed. Drop
    // the request's owner immediately so the bearer is neither retained across
    // expensive minting work nor exposed to downstream handlers.
    headers.remove(header::AUTHORIZATION);
    authorized
}
async fn authorize_mint_request(
    State(authorization): State<Arc<MintAuthorization>>,
    mut request: Request,
    next: Next,
) -> Response {
    let authorized = authenticate_and_remove_bearer(request.headers_mut(), &authorization);
    if !authorized {
        let body = JsonBytes::from_value(norito::json!({ "error": "authentication required" }));
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                "Bearer realm=\"soranet-puzzle-service\"",
            )],
            body,
        )
            .into_response();
    }
    if request.method() == axum::http::Method::POST
        && !has_single_json_content_type(request.headers())
    {
        let body = JsonBytes::from_value(norito::json!({
            "error": "content-type must be application/json"
        }));
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, body).into_response();
    }
    let Ok(permit) = Arc::clone(&authorization.permits).try_acquire_owned() else {
        let body = JsonBytes::from_value(norito::json!({ "error": "mint capacity exhausted" }));
        return (StatusCode::TOO_MANY_REQUESTS, body).into_response();
    };
    // A disconnected client cancels this middleware future, but Tokio cannot
    // cancel an Argon2/ML-DSA `spawn_blocking` closure already in progress. Run
    // the protected request in a detached child task that owns the permit, so
    // cancellation cannot make capacity available while CPU work continues.
    let mut response =
        match retain_mint_permit_until_complete(permit, async move { next.run(request).await })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(%error, "protected mint request task failed");
                let body = JsonBytes::from_value(norito::json!({ "error": "mint request failed" }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        };
    mark_sensitive_response(&mut response);
    response
}
async fn retain_mint_permit_until_complete<F, T>(
    permit: OwnedSemaphorePermit,
    operation: F,
) -> Result<T, tokio::task::JoinError>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(async move {
        let _permit = permit;
        operation.await
    })
    .await
}
fn mark_sensitive_response(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
}
struct SignedTicketIssuer {
    public_key: Vec<u8>,
    secret_key: SensitiveBytes,
}
struct PuzzleService {
    descriptor_commit: [u8; 32],
    relay_id: [u8; 32],
    puzzle_params: PuzzleParameters,
    ticket_ttl: Duration,
    min_ticket_ttl: Duration,
    max_future_skew: Duration,
    pow_revocation_store_capacity: u64,
    pow_revocation_store_ttl_secs: u64,
    signed_ticket_issuer: Option<SignedTicketIssuer>,
    token: Option<Mutex<TokenIssuer>>,
}
impl fmt::Debug for PuzzleService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PuzzleService")
            .field("relay_id", &self.relay_id)
            .field(
                "signed_ticket_issuer",
                &self.signed_ticket_issuer.as_ref().map(|_| "<redacted>"),
            )
            .field("token_enabled", &self.token.is_some())
            .finish_non_exhaustive()
    }
}
impl PuzzleService {
    fn new(args: &Args) -> Result<Self> {
        let config = RelayConfig::load(&args.config).wrap_err("failed to load relay config")?;
        let policy = config.handshake_policy();
        let descriptor_commit = policy
            .descriptor_commit_bytes()
            .wrap_err("failed to parse descriptor_commit")?
            .ok_or_else(|| eyre!("handshake.descriptor_commit_hex must be configured"))?;
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("system clock is before the Unix epoch")?
            .as_secs();
        let now_unix = i64::try_from(now_unix).wrap_err("current Unix time exceeds i64")?;
        let relay_id = derive_relay_id(policy, &descriptor_commit, now_unix)
            .wrap_err("failed to derive relay identity for bindings")?;
        let pow_cfg = config.pow_config().clone();
        let puzzle_params = pow_cfg
            .puzzle_parameters()
            .wrap_err("invalid puzzle configuration")?;
        let min_ticket_ttl = puzzle_params.min_ticket_ttl();
        let max_future_skew = puzzle_params.max_future_skew();
        let mint_corridor = max_future_skew
            .checked_sub(min_ticket_ttl)
            .filter(|window| !window.is_zero())
            .ok_or_else(|| {
                eyre!("puzzle ticket policy must leave minting headroom above min_ticket_ttl")
            })?;
        let target_headroom = (mint_corridor / 2)
            .max(Duration::from_secs(1))
            .min(mint_corridor);
        // Split the configured corridor between one successful candidate plus
        // delivery and clock-skew headroom. Each geometric search candidate is
        // independently expiry-bound by the crypto implementation.
        let ticket_ttl = min_ticket_ttl + target_headroom;
        let token_opts = TokenCliOptions {
            secret_path: args.token_secret_path.clone(),
            revocation_file: args.token_revocation_file.clone(),
            revocation_refresh_secs: args.token_revocation_refresh_secs,
        };
        let signed_secret_opts = SignedTicketSecretOptions {
            secret_path: args.signed_ticket_secret_path.clone(),
        };
        let token = token_issuer_from_config(
            relay_id,
            &pow_cfg,
            min_ticket_ttl,
            ticket_ttl,
            max_future_skew,
            &token_opts,
        )
        .wrap_err("failed to initialise admission token policy")?
        .map(Mutex::new);
        let signed_ticket_secret = load_signed_ticket_secret(&signed_secret_opts)?;
        let signed_ticket_public_key = pow_cfg
            .signed_ticket_public_key_hex
            .as_ref()
            .map(|value| {
                decode_mldsa_public_key_hex(value, MlDsaSuite::MlDsa44, "signed-ticket public key")
                    .map_err(|error| eyre!(error))
            })
            .transpose()?;
        let signed_ticket_issuer = match (signed_ticket_public_key, signed_ticket_secret) {
            (Some(public_key), Some(secret_key)) => {
                validate_signed_ticket_keypair(&public_key, &secret_key)?;
                Some(SignedTicketIssuer {
                    public_key,
                    secret_key,
                })
            }
            (None, Some(_)) => {
                return Err(eyre!(
                    "--signed-ticket-secret-path requires pow.signed_ticket_public_key_hex in the relay config"
                ));
            }
            (Some(_), None) => {
                return Err(eyre!(
                    "pow.signed_ticket_public_key_hex requires --signed-ticket-secret-path in the puzzle issuer"
                ));
            }
            (None, None) => None,
        };
        Ok(Self {
            descriptor_commit,
            relay_id,
            puzzle_params,
            ticket_ttl,
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: pow_cfg.revocation_store_capacity,
            pow_revocation_store_ttl_secs: pow_cfg.revocation_store_ttl_secs,
            signed_ticket_issuer,
            token,
        })
    }
    fn clamp_ttl(&self, override_ttl: Option<Duration>) -> Duration {
        let target = override_ttl.unwrap_or(self.ticket_ttl);
        let clamped = if target < self.min_ticket_ttl {
            self.min_ticket_ttl
        } else {
            target
        };
        if clamped > self.max_future_skew {
            self.max_future_skew
        } else {
            clamped
        }
    }
    fn signed_ticket_public_key_hex(&self) -> Option<String> {
        self.signed_ticket_issuer
            .as_ref()
            .map(|issuer| encode(&issuer.public_key))
    }
    fn signed_ticket_public_key_fingerprint_hex(&self) -> Option<String> {
        self.signed_ticket_issuer.as_ref().map(|issuer| {
            let fingerprint = blake3_hash(&issuer.public_key);
            encode(fingerprint.as_bytes())
        })
    }
    fn mint_ticket<R: RngCore + CryptoRng>(
        &self,
        ttl: Duration,
        transcript_hash: [u8; 32],
        rng: &mut R,
    ) -> Result<PowTicket, ChallengeMintError> {
        let binding = PuzzleBinding::new(&self.descriptor_commit, &self.relay_id, &transcript_hash);
        puzzle::mint_ticket(&self.puzzle_params, &binding, ttl, rng)
            .map_err(ChallengeMintError::Puzzle)
    }
    fn token_summary(&self) -> Result<TokenConfigResponse, TokenIssuerError> {
        let Some(issuer_mutex) = &self.token else {
            return Ok(TokenConfigResponse::disabled());
        };
        let mut issuer = issuer_mutex
            .lock()
            .map_err(|_| TokenIssuerError::StateUnavailable)?;
        issuer.refresh_revocations()?;
        TokenConfigResponse::enabled(&issuer, true)
    }
    fn public_token_summary(&self) -> Result<TokenConfigResponse, TokenIssuerError> {
        let Some(issuer_mutex) = &self.token else {
            return Ok(TokenConfigResponse::disabled());
        };
        // Public metadata is unauthenticated and never worth parking a Tokio
        // reactor behind an in-flight ML-DSA mint. It also must not trigger
        // privileged revocation-file I/O.
        let issuer = issuer_mutex.try_lock().map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => TokenIssuerError::Busy,
            std::sync::TryLockError::Poisoned(_) => TokenIssuerError::StateUnavailable,
        })?;
        TokenConfigResponse::enabled(&issuer, false)
    }
    fn mint_token<R: RngCore + CryptoRng>(
        &self,
        ttl_override: Option<Duration>,
        transcript_hash: [u8; 32],
        issued_at: SystemTime,
        rng: &mut R,
    ) -> Result<AdmissionToken, TokenIssuerError> {
        let issuer_mutex = self
            .token
            .as_ref()
            .ok_or(TokenIssuerError::PolicyDisabled)?;
        let mut issuer = issuer_mutex
            .lock()
            .map_err(|_| TokenIssuerError::StateUnavailable)?;
        issuer.refresh_revocations()?;
        let ttl = issuer.clamp_ttl(ttl_override)?;
        issuer.mint(transcript_hash, ttl, issued_at, 0, rng)
    }
}
#[derive(Debug, Error)]
enum ChallengeMintError {
    #[error("puzzle ticket mint failed: {0}")]
    Puzzle(puzzle::MintError),
}
#[derive(Debug, Error)]
enum TokenInitError {
    #[error("token issuer public key missing while pow.token.enabled = true")]
    MissingPublicKey,
    #[error("token issuer secret key missing while pow.token.enabled = true")]
    MissingSecretKey,
    #[error("invalid issuer public key hex: {0}")]
    InvalidPublicKey(String),
    #[error("invalid issuer secret key hex: {0}")]
    InvalidSecretKey(String),
    #[error("admission-token issuer public and secret keys do not form one ML-DSA-44 keypair")]
    KeyPairMismatch,
    #[error("failed to read issuer secret key from {path}: {error}")]
    SecretKeyIo {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("invalid revocation hex entry #{index}: {reason}")]
    InvalidRevocationHex { index: usize, reason: String },
    #[error("failed to load revocation file {path}: {error}")]
    RevocationFile { path: PathBuf, error: String },
    #[error("relay identity key invalid: {0}")]
    RelayIdentity(String),
    #[error("handshake configuration error: {0}")]
    Handshake(String),
    #[error("token issuer capacity error: {0}")]
    Capacity(String),
}
impl From<RelayConfigError> for TokenInitError {
    fn from(err: RelayConfigError) -> Self {
        TokenInitError::Handshake(err.to_string())
    }
}
#[derive(Debug, Error)]
enum TokenIssuerError {
    #[error("admission token policy is disabled")]
    PolicyDisabled,
    #[error("token TTL {requested:?} shorter than required minimum {minimum:?}")]
    TtlTooShort {
        requested: Duration,
        minimum: Duration,
    },
    #[error("token TTL {requested:?} exceeds allowed maximum {maximum:?}")]
    TtlTooLong {
        requested: Duration,
        maximum: Duration,
    },
    #[error("revocation reload failed: {0}")]
    Revocation(String),
    #[error("token mint failed: {0}")]
    Mint(#[from] AdmissionTokenMintError),
    #[error("token expiry overflowed system time")]
    ExpiryOverflow,
    #[error("minted token immediately revoked ({0})")]
    Revoked(String),
    #[error("token issuer capacity error: {0}")]
    Capacity(String),
    #[error("token issuer is busy")]
    Busy,
    #[error("token issuer state is unavailable")]
    StateUnavailable,
}
struct RevocationFile {
    path: PathBuf,
    refresh_interval: Duration,
    last_loaded: Instant,
    entries: HashSet<[u8; 32]>,
}
impl RevocationFile {
    fn new(path: PathBuf, refresh_interval: Duration) -> Result<Self, TokenInitError> {
        let contents = read_bounded_utf8_file(
            &path,
            REVOCATION_FILE_MAX_BYTES_V1,
            "SoraNet puzzle-service revocation file",
        )
        .map_err(|error| TokenInitError::RevocationFile {
            path: path.clone(),
            error: error.to_string(),
        })?;
        let entries = parse_revocation_contents(&contents).map_err(|reason| {
            TokenInitError::RevocationFile {
                path: path.clone(),
                error: reason,
            }
        })?;
        Ok(Self {
            path,
            refresh_interval,
            last_loaded: Instant::now(),
            entries,
        })
    }
    fn refresh_if_due(&mut self) -> Result<(), TokenIssuerError> {
        if self.refresh_interval.is_zero() {
            return Ok(());
        }
        if self.last_loaded.elapsed() < self.refresh_interval {
            return Ok(());
        }
        let contents = read_bounded_utf8_file(
            &self.path,
            REVOCATION_FILE_MAX_BYTES_V1,
            "SoraNet puzzle-service revocation file",
        )
        .map_err(|error| {
            TokenIssuerError::Revocation(format!("failed to read {}: {error}", self.path.display()))
        })?;
        let entries = parse_revocation_contents(&contents).map_err(|reason| {
            TokenIssuerError::Revocation(format!(
                "failed to parse {}: {reason}",
                self.path.display()
            ))
        })?;
        self.entries = entries;
        self.last_loaded = Instant::now();
        Ok(())
    }
}
struct TokenIssuer {
    suite: MlDsaSuite,
    secret_key: SensitiveBytes,
    issuer_fingerprint: [u8; 32],
    relay_id: [u8; 32],
    min_ttl: Duration,
    max_ttl: Duration,
    default_ttl: Duration,
    clock_skew: Duration,
    static_revocations: HashSet<[u8; 32]>,
    revocation_file: Option<RevocationFile>,
}
struct TokenCliOptions {
    secret_path: Option<PathBuf>,
    revocation_file: Option<PathBuf>,
    revocation_refresh_secs: u64,
}
struct SignedTicketSecretOptions {
    secret_path: Option<PathBuf>,
}
struct TokenTiming {
    min_ttl: Duration,
    max_ttl: Duration,
    default_ttl: Duration,
    clock_skew: Duration,
}
impl TokenIssuer {
    fn new(
        suite: MlDsaSuite,
        secret_key: SensitiveBytes,
        issuer_fingerprint: [u8; 32],
        relay_id: [u8; 32],
        timing: TokenTiming,
        static_revocations: HashSet<[u8; 32]>,
        revocation_file: Option<RevocationFile>,
    ) -> Self {
        Self {
            suite,
            secret_key,
            issuer_fingerprint,
            relay_id,
            min_ttl: timing.min_ttl,
            max_ttl: timing.max_ttl,
            default_ttl: timing.default_ttl,
            clock_skew: timing.clock_skew,
            static_revocations,
            revocation_file,
        }
    }
    fn clear_secret_key(&mut self) {
        self.secret_key.clear();
    }
    fn refresh_revocations(&mut self) -> Result<(), TokenIssuerError> {
        if let Some(file) = &mut self.revocation_file {
            file.refresh_if_due()?;
        }
        Ok(())
    }
    fn clamp_ttl(&self, override_ttl: Option<Duration>) -> Result<Duration, TokenIssuerError> {
        let desired = override_ttl.unwrap_or(self.default_ttl);
        if desired < self.min_ttl {
            return Err(TokenIssuerError::TtlTooShort {
                requested: desired,
                minimum: self.min_ttl,
            });
        }
        if desired > self.max_ttl {
            return Err(TokenIssuerError::TtlTooLong {
                requested: desired,
                maximum: self.max_ttl,
            });
        }
        Ok(desired)
    }
    fn mint<R: RngCore + CryptoRng>(
        &mut self,
        transcript_hash: [u8; 32],
        ttl: Duration,
        issued_at: SystemTime,
        flags: u8,
        rng: &mut R,
    ) -> Result<AdmissionToken, TokenIssuerError> {
        let expires_at = issued_at
            .checked_add(ttl)
            .ok_or(TokenIssuerError::ExpiryOverflow)?;
        let token = AdmissionToken::mint(
            self.suite,
            &self.secret_key,
            self.issuer_fingerprint,
            self.relay_id,
            transcript_hash,
            issued_at,
            expires_at,
            flags,
            rng,
        )?;
        let token_id = token.token_id();
        if self.is_revoked(&token_id) {
            return Err(TokenIssuerError::Revoked(encode(token_id)));
        }
        Ok(token)
    }
    fn is_revoked(&self, token_id: &[u8; 32]) -> bool {
        self.static_revocations.contains(token_id)
            || self
                .revocation_file
                .as_ref()
                .is_some_and(|file| file.entries.contains(token_id))
    }
    fn max_ttl(&self) -> Duration {
        self.max_ttl
    }
    fn min_ttl(&self) -> Duration {
        self.min_ttl
    }
    fn default_ttl(&self) -> Duration {
        self.default_ttl
    }
    fn clock_skew(&self) -> Duration {
        self.clock_skew
    }
    fn issuer_fingerprint(&self) -> &[u8; 32] {
        &self.issuer_fingerprint
    }
    fn relay_id(&self) -> &[u8; 32] {
        &self.relay_id
    }
    fn suite_label(&self) -> &'static str {
        match self.suite {
            MlDsaSuite::MlDsa44 => "ml-dsa-44",
            MlDsaSuite::MlDsa65 => "ml-dsa-65",
            MlDsaSuite::MlDsa87 => "ml-dsa-87",
        }
    }
    fn revocation_ids_hex(&self) -> Result<Vec<String>, TokenIssuerError> {
        let maximum = self
            .static_revocations
            .len()
            .checked_add(
                self.revocation_file
                    .as_ref()
                    .map_or(0, |file| file.entries.len()),
            )
            .ok_or_else(|| {
                TokenIssuerError::Capacity(
                    "revocation summary entry count overflowed the platform address space"
                        .to_owned(),
                )
            })?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(maximum).map_err(|_| {
            TokenIssuerError::Capacity(
                "failed to reserve the bounded revocation summary index".to_owned(),
            )
        })?;
        ids.extend(self.static_revocations.iter().copied());
        if let Some(file) = &self.revocation_file {
            ids.extend(file.entries.iter().copied());
        }
        ids.sort_unstable();
        ids.dedup();
        let mut encoded = Vec::new();
        encoded.try_reserve_exact(ids.len()).map_err(|_| {
            TokenIssuerError::Capacity(
                "failed to reserve the bounded revocation summary output".to_owned(),
            )
        })?;
        for id in ids {
            let mut literal = [0_u8; 64];
            hex::encode_to_slice(id, &mut literal).map_err(|error| {
                TokenIssuerError::Capacity(format!(
                    "failed to encode a fixed-width revocation identifier: {error}"
                ))
            })?;
            let text = core::str::from_utf8(&literal).map_err(|error| {
                TokenIssuerError::Capacity(format!(
                    "fixed-width revocation identifier was not UTF-8: {error}"
                ))
            })?;
            let mut item = String::new();
            item.try_reserve_exact(text.len()).map_err(|_| {
                TokenIssuerError::Capacity(
                    "failed to reserve a fixed-width revocation identifier".to_owned(),
                )
            })?;
            item.push_str(text);
            encoded.push(item);
        }
        Ok(encoded)
    }
}
impl fmt::Debug for TokenIssuer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenIssuer")
            .field("suite", &self.suite)
            .field("secret_key", &"<redacted>")
            .field("issuer_fingerprint", &self.issuer_fingerprint)
            .field("relay_id", &self.relay_id)
            .field("static_revocation_count", &self.static_revocations.len())
            .finish_non_exhaustive()
    }
}
impl Drop for TokenIssuer {
    fn drop(&mut self) {
        self.clear_secret_key();
    }
}
#[derive(Debug, Error)]
enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(message) => {
                let body = JsonBytes::from_value(norito::json!({ "error": message }));
                (StatusCode::BAD_REQUEST, body).into_response()
            }
            ApiError::Internal(message) => {
                let body = JsonBytes::from_value(norito::json!({ "error": message }));
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}
#[derive(Debug)]
struct JsonBytes(Vec<u8>);
impl JsonBytes {
    fn from_serializable<T>(value: &T) -> Result<Self, ApiError>
    where
        T: norito::json::JsonSerialize,
    {
        json::to_vec(value)
            .map(JsonBytes)
            .map_err(|err| ApiError::Internal(format!("failed to encode response: {err}")))
    }
    fn from_value(value: json::Value) -> Self {
        let bytes = json::to_vec(&value).expect("Value serialization must succeed");
        JsonBytes(bytes)
    }
}
impl IntoResponse for JsonBytes {
    fn into_response(self) -> Response {
        let mut response = Response::new(Body::from(self.0));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        response
    }
}

/// JSON bytes whose allocation is scrubbed only after the HTTP body and all
/// clones of its backing `Bytes` have been released.
struct SensitiveJsonBytes(SensitiveBytes);
impl SensitiveJsonBytes {
    fn from_serializable<T>(value: &T) -> Result<Self, ApiError>
    where
        T: norito::json::JsonSerialize,
    {
        json::to_vec(value)
            .map(|bytes| Self(SensitiveBytes(bytes)))
            .map_err(|err| ApiError::Internal(format!("failed to encode response: {err}")))
    }
}
impl fmt::Debug for SensitiveJsonBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveJsonBytes(<redacted>)")
    }
}
impl IntoResponse for SensitiveJsonBytes {
    fn into_response(self) -> Response {
        let bytes = Bytes::from_owner(self.0);
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        response
    }
}
#[derive(Debug, JsonSerialize)]
struct ConfigResponse {
    difficulty: u8,
    max_future_skew_secs: u64,
    min_ticket_ttl_secs: u64,
    ticket_ttl_secs: u64,
    puzzle: PuzzleParamsResponse,
    token: TokenConfigResponse,
    revocation_store_capacity: u64,
    revocation_store_ttl_secs: u64,
    signed_ticket_public_key_hex: Option<String>,
    signed_ticket_public_key_fingerprint_hex: Option<String>,
}
#[derive(Debug, JsonSerialize)]
struct PuzzleParamsResponse {
    memory_kib: u32,
    time_cost: u32,
    lanes: u32,
}
#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct TokenConfigResponse {
    enabled: bool,
    #[norito(default)]
    suite: Option<String>,
    #[norito(default)]
    relay_id_hex: Option<String>,
    #[norito(default)]
    issuer_fingerprint_hex: Option<String>,
    #[norito(default)]
    max_ttl_secs: Option<u64>,
    #[norito(default)]
    min_ttl_secs: Option<u64>,
    #[norito(default)]
    default_ttl_secs: Option<u64>,
    #[norito(default)]
    clock_skew_secs: Option<u64>,
    #[norito(default)]
    revocation_ids_hex: Vec<String>,
}
impl TokenConfigResponse {
    fn disabled() -> Self {
        Self {
            enabled: false,
            suite: None,
            relay_id_hex: None,
            issuer_fingerprint_hex: None,
            max_ttl_secs: None,
            min_ttl_secs: None,
            default_ttl_secs: None,
            clock_skew_secs: None,
            revocation_ids_hex: Vec::new(),
        }
    }
    fn enabled(issuer: &TokenIssuer, include_revocations: bool) -> Result<Self, TokenIssuerError> {
        Ok(Self {
            enabled: true,
            suite: Some(issuer.suite_label().to_string()),
            relay_id_hex: Some(encode(issuer.relay_id())),
            issuer_fingerprint_hex: Some(encode(issuer.issuer_fingerprint())),
            max_ttl_secs: Some(issuer.max_ttl().as_secs()),
            min_ttl_secs: Some(issuer.min_ttl().as_secs()),
            default_ttl_secs: Some(issuer.default_ttl().as_secs()),
            clock_skew_secs: Some(issuer.clock_skew().as_secs()),
            revocation_ids_hex: if include_revocations {
                issuer.revocation_ids_hex()?
            } else {
                Vec::new()
            },
        })
    }
}
#[derive(Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct MintRequest {
    #[norito(default)]
    ttl_secs: Option<u64>,
    transcript_hash_hex: String,
}
#[derive(JsonSerialize, JsonDeserialize)]
struct MintResponse {
    credential_kind: String,
    credential_b64: String,
    #[norito(default)]
    signed_ticket_fingerprint_hex: Option<String>,
    difficulty: u8,
    ttl_secs: u64,
    expires_at: u64,
}
impl MintResponse {
    fn clear_credentials(&mut self) {
        clear_sensitive_string(&mut self.credential_b64);
    }
}
impl fmt::Debug for MintResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MintResponse")
            .field("credential_kind", &self.credential_kind)
            .field("credential_b64", &"<redacted>")
            .field(
                "signed_ticket_fingerprint_hex",
                &self.signed_ticket_fingerprint_hex,
            )
            .field("difficulty", &self.difficulty)
            .field("ttl_secs", &self.ttl_secs)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
impl Drop for MintResponse {
    fn drop(&mut self) {
        self.clear_credentials();
    }
}
#[derive(Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct MintTokenRequest {
    transcript_hash_hex: String,
    #[norito(default)]
    ttl_secs: Option<u64>,
}
#[derive(JsonSerialize, JsonDeserialize)]
struct MintTokenResponse {
    token_b64: String,
    token_id_hex: String,
    issued_at: u64,
    expires_at: u64,
    ttl_secs: u64,
    issuer_fingerprint_hex: String,
    relay_id_hex: String,
}
impl MintTokenResponse {
    fn clear_credentials(&mut self) {
        clear_sensitive_string(&mut self.token_b64);
    }
}
impl fmt::Debug for MintTokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MintTokenResponse")
            .field("token_b64", &"<redacted>")
            .field("token_id_hex", &self.token_id_hex)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("ttl_secs", &self.ttl_secs)
            .field("issuer_fingerprint_hex", &self.issuer_fingerprint_hex)
            .field("relay_id_hex", &self.relay_id_hex)
            .finish()
    }
}
impl Drop for MintTokenResponse {
    fn drop(&mut self) {
        self.clear_credentials();
    }
}
async fn get_config(State(state): State<Arc<PuzzleService>>) -> Result<JsonBytes, ApiError> {
    let token = state
        .public_token_summary()
        .map_err(|err| ApiError::Internal(format!("token summary error: {err}")))?;
    let response = ConfigResponse {
        difficulty: state.puzzle_params.difficulty(),
        max_future_skew_secs: state.max_future_skew.as_secs(),
        min_ticket_ttl_secs: state.min_ticket_ttl.as_secs(),
        ticket_ttl_secs: state.ticket_ttl.as_secs(),
        puzzle: PuzzleParamsResponse {
            memory_kib: state.puzzle_params.memory_kib().get(),
            time_cost: state.puzzle_params.time_cost().get(),
            lanes: state.puzzle_params.lanes().get(),
        },
        token,
        revocation_store_capacity: state.pow_revocation_store_capacity,
        revocation_store_ttl_secs: state.pow_revocation_store_ttl_secs,
        signed_ticket_public_key_hex: state.signed_ticket_public_key_hex(),
        signed_ticket_public_key_fingerprint_hex: state.signed_ticket_public_key_fingerprint_hex(),
    };
    JsonBytes::from_serializable(&response)
}
async fn get_token_config(State(state): State<Arc<PuzzleService>>) -> Result<JsonBytes, ApiError> {
    // This protected path refreshes and parses the privileged revocation file.
    // Keep both that I/O and contention on the issuer mutex off Tokio workers.
    let summary = tokio::task::spawn_blocking(move || state.token_summary())
        .await
        .map_err(|err| ApiError::Internal(format!("token summary worker failed: {err}")))?
        .map_err(|err| ApiError::Internal(format!("token summary error: {err}")))?;
    JsonBytes::from_serializable(&summary)
}
async fn mint_ticket(
    State(state): State<Arc<PuzzleService>>,
    body: Bytes,
) -> Result<SensitiveJsonBytes, ApiError> {
    if body.is_empty() {
        return Err(ApiError::BadRequest(
            "transcript_hash_hex is required".to_owned(),
        ));
    }
    let payload = json::from_slice::<MintRequest>(&body)
        .map_err(|err| ApiError::BadRequest(format!("invalid JSON body: {err}")))?;
    let ttl_override = payload.ttl_secs.map(Duration::from_secs);
    let ttl = state.clamp_ttl(ttl_override);
    if ttl <= state.min_ticket_ttl {
        return Err(ApiError::BadRequest(format!(
            "ttl_secs must exceed the puzzle minimum remaining ttl of {} seconds",
            state.min_ticket_ttl.as_secs()
        )));
    }
    let transcript_hash = hex_to_fixed::<32>(&payload.transcript_hash_hex)
        .map_err(|reason| ApiError::BadRequest(format!("transcript_hash_hex invalid: {reason}")))?;
    if transcript_hash.iter().all(|byte| *byte == 0) {
        return Err(ApiError::BadRequest(
            "transcript_hash_hex must not be all zeros".to_owned(),
        ));
    }
    let mint_state = Arc::clone(&state);
    enum MintedCredential {
        Raw(PowTicket),
        Signed(SignedTicket),
    }
    let credential = tokio::task::spawn_blocking(move || {
        let mut rng = StdRng::from_os_rng();
        let ticket = mint_state
            .mint_ticket(ttl, transcript_hash, &mut rng)
            .map_err(|err| ApiError::Internal(err.to_string()))?;
        if let Some(issuer) = mint_state.signed_ticket_issuer.as_ref() {
            SignedTicket::sign(
                ticket,
                &mint_state.relay_id,
                &transcript_hash,
                &issuer.secret_key,
            )
            .map(MintedCredential::Signed)
            .map_err(|err| ApiError::Internal(format!("signed ticket mint failed: {err}")))
        } else {
            Ok(MintedCredential::Raw(ticket))
        }
    })
    .await
    .map_err(|err| ApiError::Internal(format!("mint worker failed: {err}")))??;
    let (credential_kind, credential_b64, signed_ticket_fingerprint_hex, expires_at, difficulty) =
        match credential {
            MintedCredential::Raw(ticket) => {
                let expires_at = ticket.expires_at;
                let difficulty = ticket.difficulty;
                let ticket_bytes = SensitiveBytes(ticket.to_vec());
                (
                    "raw".to_owned(),
                    STANDARD.encode(&*ticket_bytes),
                    None,
                    expires_at,
                    difficulty,
                )
            }
            MintedCredential::Signed(signed_ticket) => {
                let expires_at = signed_ticket.ticket.expires_at;
                let difficulty = signed_ticket.ticket.difficulty;
                let fingerprint = encode(signed_ticket.revocation_fingerprint());
                let signed_ticket_bytes = SensitiveBytes(signed_ticket.encode());
                (
                    "signed".to_owned(),
                    STANDARD.encode(&*signed_ticket_bytes),
                    Some(fingerprint),
                    expires_at,
                    difficulty,
                )
            }
        };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| ApiError::Internal(format!("system clock error: {err}")))?
        .as_secs();
    let ttl_secs = expires_at.saturating_sub(now);
    let response = MintResponse {
        credential_kind,
        credential_b64,
        signed_ticket_fingerprint_hex,
        difficulty,
        ttl_secs,
        expires_at,
    };
    SensitiveJsonBytes::from_serializable(&response)
}
async fn mint_token(
    State(state): State<Arc<PuzzleService>>,
    body: Bytes,
) -> Result<SensitiveJsonBytes, ApiError> {
    let payload = if body.is_empty() {
        return Err(ApiError::BadRequest(
            "transcript_hash_hex is required".to_string(),
        ));
    } else {
        json::from_slice::<MintTokenRequest>(&body)
            .map_err(|err| ApiError::BadRequest(format!("invalid JSON body: {err}")))?
    };
    let transcript_hex = payload.transcript_hash_hex.trim();
    if transcript_hex.is_empty() {
        return Err(ApiError::BadRequest(
            "transcript_hash_hex must not be empty".to_string(),
        ));
    }
    let transcript_bytes = decode(transcript_hex)
        .map_err(|err| ApiError::BadRequest(format!("invalid transcript_hash_hex: {err}")))?;
    if transcript_bytes.len() != 32 {
        return Err(ApiError::BadRequest(format!(
            "transcript_hash_hex must decode to 32 bytes (got {})",
            transcript_bytes.len()
        )));
    }
    let mut transcript_hash = [0u8; 32];
    transcript_hash.copy_from_slice(&transcript_bytes);
    if transcript_hash.iter().all(|byte| *byte == 0) {
        return Err(ApiError::BadRequest(
            "transcript_hash_hex must not be all zeros".to_owned(),
        ));
    }
    let ttl_override = payload.ttl_secs.map(Duration::from_secs);
    let issued_at = canonical_issued_at(SystemTime::now())?;
    let mint_state = Arc::clone(&state);
    let minted = tokio::task::spawn_blocking(move || {
        let mut rng = StdRng::from_os_rng();
        mint_state.mint_token(ttl_override, transcript_hash, issued_at, &mut rng)
    })
    .await
    .map_err(|err| ApiError::Internal(format!("token mint worker failed: {err}")))?;
    let token = minted.map_err(|err| match err {
        TokenIssuerError::PolicyDisabled => {
            ApiError::BadRequest("admission token policy disabled on this relay".to_owned())
        }
        TokenIssuerError::TtlTooShort { minimum, .. } => ApiError::BadRequest(format!(
            "ttl_secs shorter than minimum {}",
            minimum.as_secs()
        )),
        TokenIssuerError::TtlTooLong { maximum, .. } => {
            ApiError::BadRequest(format!("ttl_secs exceeds maximum {}", maximum.as_secs()))
        }
        TokenIssuerError::Revocation(message) => ApiError::Internal(message),
        TokenIssuerError::Mint(err) => ApiError::Internal(format!("token mint failed: {err}")),
        TokenIssuerError::ExpiryOverflow => {
            ApiError::Internal("issued_at + ttl overflowed system time".to_string())
        }
        TokenIssuerError::Revoked(id) => {
            ApiError::Internal(format!("minted token immediately revoked ({id})"))
        }
        TokenIssuerError::Capacity(message) => ApiError::Internal(message),
        TokenIssuerError::Busy => ApiError::Internal("token issuer is busy".to_owned()),
        TokenIssuerError::StateUnavailable => {
            ApiError::Internal("token issuer state is unavailable".to_owned())
        }
    })?;
    let token_bytes = SensitiveBytes(token.encode());
    let token_b64 = STANDARD.encode(&*token_bytes);
    let issued_at_secs = token.issued_at();
    let expires_at_secs = token.expires_at();
    let ttl_secs = expires_at_secs.saturating_sub(issued_at_secs);
    let response = MintTokenResponse {
        token_b64,
        token_id_hex: encode(token.token_id()),
        issued_at: issued_at_secs,
        expires_at: expires_at_secs,
        ttl_secs,
        issuer_fingerprint_hex: encode(token.issuer_fingerprint()),
        relay_id_hex: encode(token.relay_id()),
    };
    SensitiveJsonBytes::from_serializable(&response)
}
fn canonical_issued_at(now: SystemTime) -> Result<SystemTime, ApiError> {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ApiError::Internal(format!("system clock error: {error}")))?
        .as_secs();
    Ok(UNIX_EPOCH + Duration::from_secs(seconds))
}
async fn healthz() -> StatusCode {
    StatusCode::OK
}
fn derive_relay_id(
    policy: &HandshakePolicy,
    descriptor_commit: &[u8; 32],
    at_unix: i64,
) -> Result<[u8; 32], TokenInitError> {
    let bundle = policy.load_certificate_bundle_at(at_unix)?;
    if &bundle.certificate.descriptor_commit != descriptor_commit {
        return Err(TokenInitError::RelayIdentity(
            "verified certificate descriptor_commit does not match handshake.descriptor_commit_hex"
                .to_owned(),
        ));
    }
    Ok(bundle.certificate.relay_id)
}
fn token_issuer_from_config(
    relay_id: [u8; 32],
    pow_cfg: &PowConfig,
    min_ticket_ttl: Duration,
    default_ticket_ttl: Duration,
    max_future_skew: Duration,
    cli: &TokenCliOptions,
) -> Result<Option<TokenIssuer>, TokenInitError> {
    let Some(token_cfg) = pow_cfg.token.as_ref() else {
        return Ok(None);
    };
    if !token_cfg.enabled {
        return Ok(None);
    }
    let public_hex = token_cfg
        .issuer_public_key_hex
        .as_ref()
        .ok_or(TokenInitError::MissingPublicKey)?;
    let suite = MlDsaSuite::MlDsa44;
    let public_key =
        decode_mldsa_public_key_hex(public_hex, suite, "admission-token issuer public key")
            .map_err(TokenInitError::InvalidPublicKey)?;
    let issuer_fingerprint = compute_issuer_fingerprint(&public_key);
    let secret_path = cli.secret_path.as_ref();
    let expected_secret_bytes = suite.secret_key_len();
    let secret_key_bytes = if let Some(path) = secret_path {
        let maximum = secret_file_max_bytes(expected_secret_bytes)
            .map_err(TokenInitError::InvalidSecretKey)?;
        let mut contents =
            read_bounded_private_file(path, maximum, "SoraNet admission-token issuer secret key")
                .map_err(|error| TokenInitError::SecretKeyIo {
                path: path.clone(),
                error,
            })?;
        decode_private_hex_bytes(
            &mut contents,
            expected_secret_bytes,
            "admission-token issuer secret key",
        )
        .map_err(TokenInitError::InvalidSecretKey)?
    } else {
        return Err(TokenInitError::MissingSecretKey);
    };
    let derived_public_key = suite
        .public_key_from_secret_key(&secret_key_bytes)
        .map_err(|error| TokenInitError::InvalidSecretKey(error.to_string()))?;
    if derived_public_key != public_key {
        return Err(TokenInitError::KeyPairMismatch);
    }
    let mut static_revocations = HashSet::new();
    static_revocations
        .try_reserve(token_cfg.revocation_list_hex.len())
        .map_err(|_| {
            TokenInitError::Capacity(
                "failed to reserve the bounded static token revocation set".to_owned(),
            )
        })?;
    for (idx, value) in token_cfg.revocation_list_hex.iter().enumerate() {
        let entry = hex_to_fixed::<32>(value)
            .map_err(|reason| TokenInitError::InvalidRevocationHex { index: idx, reason })?;
        static_revocations.insert(entry);
    }
    let refresh_secs = if cli.revocation_file.is_some() {
        cli.revocation_refresh_secs
    } else {
        token_cfg
            .revocation_refresh_secs
            .unwrap_or(cli.revocation_refresh_secs)
    }
    .max(1);
    let revocation_file_path = cli
        .revocation_file
        .as_ref()
        .cloned()
        .or_else(|| token_cfg.revocation_list_path.clone());
    let revocation_file = if let Some(path) = revocation_file_path {
        Some(RevocationFile::new(
            path,
            Duration::from_secs(refresh_secs),
        )?)
    } else {
        None
    };
    let max_ttl = Duration::from_secs(token_cfg.max_ttl_secs.max(1))
        .min(max_future_skew.max(Duration::from_secs(1)));
    let min_ttl = min_ticket_ttl.max(Duration::from_secs(1));
    let mut default_ttl = default_ticket_ttl.max(min_ttl);
    if default_ttl > max_ttl {
        default_ttl = max_ttl;
    }
    let clock_skew = Duration::from_secs(token_cfg.clock_skew_secs.max(1));
    let timing = TokenTiming {
        min_ttl,
        max_ttl,
        default_ttl,
        clock_skew,
    };
    Ok(Some(TokenIssuer::new(
        suite,
        secret_key_bytes,
        issuer_fingerprint,
        relay_id,
        timing,
        static_revocations,
        revocation_file,
    )))
}
fn load_signed_ticket_secret(opts: &SignedTicketSecretOptions) -> Result<Option<SensitiveBytes>> {
    let Some(path) = opts.secret_path.as_ref() else {
        return Ok(None);
    };
    let suite = MlDsaSuite::MlDsa44;
    let expected = suite.secret_key_len();
    let maximum = secret_file_max_bytes(expected).map_err(|error| eyre!(error))?;
    let mut source_hex =
        read_bounded_private_file(path, maximum, "SoraNet signed-ticket secret key")
            .wrap_err_with(|| {
                format!(
                    "failed to read signed ticket secret from {}",
                    path.display()
                )
            })?;
    let decoded = decode_private_hex_bytes(&mut source_hex, expected, "signed-ticket secret key")
        .map_err(|error| eyre!(error))?;
    Ok(Some(decoded))
}
fn validate_signed_ticket_keypair(public_key: &[u8], secret_key: &[u8]) -> Result<()> {
    let derived_public_key = MlDsaSuite::MlDsa44
        .public_key_from_secret_key(secret_key)
        .wrap_err("failed to validate signed ticket secret key")?;
    if derived_public_key != public_key {
        return Err(eyre!(
            "pow.signed_ticket_public_key_hex does not match the provided signed ticket secret key"
        ));
    }
    Ok(())
}
fn hex_to_fixed<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let expected = N
        .checked_mul(2)
        .ok_or_else(|| "hexadecimal width overflows the platform address space".to_owned())?;
    if value.len() != expected {
        return Err(format!(
            "expected {expected} hexadecimal characters, found {}",
            value.len()
        ));
    }
    let mut out = [0u8; N];
    hex::decode_to_slice(value, &mut out).map_err(|error| error.to_string())?;
    Ok(out)
}
fn parse_revocation_contents(contents: &str) -> Result<HashSet<[u8; 32]>, String> {
    let mut set = HashSet::new();
    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let entry =
            hex_to_fixed::<32>(trimmed).map_err(|reason| format!("line {}: {reason}", idx + 1))?;
        if !set.contains(&entry) {
            if set.len() >= REVOCATION_LIST_MAX_ENTRIES_V1 {
                return Err(format!(
                    "revocation list exceeds the first-release limit of {REVOCATION_LIST_MAX_ENTRIES_V1} unique entries"
                ));
            }
            set.try_reserve(1)
                .map_err(|_| "failed to reserve the bounded revocation set".to_owned())?;
            set.insert(entry);
        }
    }
    Ok(set)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{
        Algorithm, KeyPair,
        soranet::token::{
            AdmissionTokenVerifier, InMemoryTokenStore, TokenStore, TokenStoreLimits,
        },
    };
    use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;
    use std::{fmt::Write as _, fs, num::NonZeroU32};
    fn temporary_file_path(label: &str) -> PathBuf {
        std::env::current_dir()
            .expect("current directory")
            .join(format!(
                "soranet_puzzle_{label}_{}_{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time")
                    .as_nanos()
            ))
    }
    #[test]
    fn listen_address_must_be_loopback() {
        validate_listen_address("127.0.0.1:8088".parse().expect("IPv4 loopback"))
            .expect("IPv4 loopback must be accepted");
        validate_listen_address("[::1]:8088".parse().expect("IPv6 loopback"))
            .expect("IPv6 loopback must be accepted");
        let error = validate_listen_address("0.0.0.0:8088".parse().expect("wildcard address"))
            .expect_err("wildcard binding must fail closed");
        assert!(error.to_string().contains("loopback"));
    }
    #[test]
    fn bounded_utf8_reader_accepts_exact_limit_and_rejects_plus_one() {
        let path = temporary_file_path("bounded_utf8");
        fs::write(&path, b"12345678").expect("write exact fixture");
        assert_eq!(
            read_bounded_utf8_file(&path, 8, "fixture").expect("read exact fixture"),
            "12345678"
        );
        fs::write(&path, b"123456789").expect("write oversized fixture");
        assert!(read_bounded_utf8_file(&path, 8, "fixture").is_err());
        let _ = fs::remove_file(path);
    }
    #[cfg(unix)]
    #[test]
    fn private_byte_reader_rejects_group_or_other_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let path = temporary_file_path("private_utf8_permissions");
        fs::write(&path, b"private material").expect("write private fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("set unsafe permissions");
        assert!(read_bounded_private_file(&path, 64, "private fixture").is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("restore private permissions");
        assert_eq!(
            read_bounded_private_file(&path, 64, "private fixture")
                .expect("private permissions accepted")
                .as_ref(),
            b"private material"
        );
        let _ = fs::remove_file(path);
    }
    #[test]
    fn mint_authorization_requires_exact_bearer_and_bounds_concurrency() {
        let path = temporary_file_path("mint_auth");
        fs::write(&path, b"soranet-puzzle-mint-token-00000001\n")
            .expect("write mint authorization fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("protect mint authorization fixture");
        }
        let mut authorization = MintAuthorization::load(&path).expect("load mint authorization");
        assert!(authorization.matches("soranet-puzzle-mint-token-00000001"));
        assert!(!authorization.matches("soranet-puzzle-mint-token-00000002"));
        assert!(constant_time_eq_32(&[0xA5; 32], &[0xA5; 32]));
        for index in 0..32 {
            let mut changed = [0xA5; 32];
            changed[index] ^= 1;
            assert!(!constant_time_eq_32(&[0xA5; 32], &changed));
        }
        let token_hash_hex = hex::encode(authorization.token_hash);
        let rendered = format!("{authorization:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(&token_hash_hex));

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer soranet-puzzle-mint-token-00000001"
                .parse()
                .expect("authorization header"),
        );
        assert_eq!(
            request_bearer_token(&headers),
            Some("soranet-puzzle-mint-token-00000001")
        );
        headers.append(
            header::AUTHORIZATION,
            "Bearer duplicate-token-000000000000000"
                .parse()
                .expect("duplicate authorization header"),
        );
        assert!(request_bearer_token(&headers).is_none());

        headers.clear();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer soranet-puzzle-mint-token-00000001"
                .parse()
                .expect("authorization header"),
        );
        assert!(authenticate_and_remove_bearer(&mut headers, &authorization));
        assert!(headers.get(header::AUTHORIZATION).is_none());

        for invalid_token in ["a".repeat(31), "a".repeat(257)] {
            let mut invalid_headers = HeaderMap::new();
            invalid_headers.insert(
                header::AUTHORIZATION,
                format!("Bearer {invalid_token}")
                    .parse()
                    .expect("bounded invalid authorization header"),
            );
            assert!(request_bearer_token(&invalid_headers).is_none());
        }

        let mut content_headers = HeaderMap::new();
        content_headers.insert(
            header::CONTENT_TYPE,
            "application/json; charset=utf-8"
                .parse()
                .expect("JSON content type"),
        );
        assert!(has_single_json_content_type(&content_headers));
        content_headers.append(
            header::CONTENT_TYPE,
            "text/plain".parse().expect("duplicate content type"),
        );
        assert!(!has_single_json_content_type(&content_headers));

        let permits = (0..MAX_CONCURRENT_MINT_REQUESTS_V1)
            .map(|_| {
                Arc::clone(&authorization.permits)
                    .try_acquire_owned()
                    .expect("configured mint permit")
            })
            .collect::<Vec<_>>();
        assert!(
            Arc::clone(&authorization.permits)
                .try_acquire_owned()
                .is_err()
        );
        drop(permits);
        authorization.clear_token_hash();
        assert_eq!(authorization.token_hash, [0; 32]);
        let _ = fs::remove_file(path);
    }
    #[tokio::test]
    async fn canceled_request_retains_mint_permit_until_blocking_work_finishes() {
        let permits = Arc::new(Semaphore::new(1));
        let permit = Arc::clone(&permits)
            .try_acquire_owned()
            .expect("mint permit");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let request = tokio::spawn(retain_mint_permit_until_complete(permit, async move {
            tokio::task::spawn_blocking(move || {
                started_tx.send(()).expect("signal blocking work start");
                release_rx.recv().expect("release blocking work");
            })
            .await
            .expect("blocking mint worker");
        }));
        tokio::time::timeout(Duration::from_secs(2), started_rx)
            .await
            .expect("blocking work start timeout")
            .expect("blocking work started");
        request.abort();
        assert_eq!(
            permits.available_permits(),
            0,
            "request cancellation must not release capacity while work continues"
        );
        release_tx.send(()).expect("finish blocking work");
        tokio::time::timeout(Duration::from_secs(2), async {
            while permits.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("permit released after blocking work");
        assert_eq!(permits.available_permits(), 1);
    }
    #[test]
    fn internally_generated_token_time_is_canonical_whole_seconds() {
        let subsecond = UNIX_EPOCH + Duration::new(123, 999_999_999);
        assert_eq!(
            canonical_issued_at(subsecond).expect("canonical time"),
            UNIX_EPOCH + Duration::from_secs(123)
        );
        let before_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_nanos(1))
            .expect("representable pre-epoch time");
        assert!(canonical_issued_at(before_epoch).is_err());
    }
    #[test]
    #[allow(unsafe_code)]
    fn private_secret_parser_requires_exact_lowercase_hex_and_wipes_input() {
        let encoded = "ab".repeat(32);
        assert_eq!(
            decode_exact_hex_bytes(&encoded, 32, "fixture secret").expect("decode exact secret"),
            vec![0xab; 32]
        );
        assert!(
            decode_exact_hex_bytes(&encoded[..encoded.len() - 2], 32, "fixture secret").is_err()
        );
        let mut raw = Vec::with_capacity(encoded.len() + 32);
        raw.resize(raw.capacity(), b'f');
        raw[..encoded.len()].copy_from_slice(encoded.as_bytes());
        raw.truncate(encoded.len());
        let mut raw = PrivateFileBytes::from(raw);
        let decoded = decode_private_hex_bytes(&mut raw, 32, "fixture secret")
            .expect("canonical private secret");
        assert_eq!(&*decoded, &[0xAB; 32]);
        assert!(raw.is_empty());
        for invalid in [
            "AB".repeat(32),
            format!("{}\n", "ab".repeat(32)),
            "00".repeat(32),
        ] {
            let mut raw = PrivateFileBytes::from(invalid.into_bytes());
            assert!(decode_private_hex_bytes(&mut raw, 32, "fixture secret").is_err());
            assert!(raw.iter().all(|byte| *byte == 0));
        }
    }
    #[test]
    fn public_key_parser_rejects_inert_mldsa_material() {
        let inert = "00".repeat(MlDsaSuite::MlDsa44.public_key_len());
        assert!(
            decode_mldsa_public_key_hex(&inert, MlDsaSuite::MlDsa44, "fixture public key").is_err()
        );
    }
    #[test]
    fn revocation_parser_caps_unique_retained_entries() {
        let mut exact = String::new();
        for index in 0..REVOCATION_LIST_MAX_ENTRIES_V1 {
            writeln!(&mut exact, "{index:064x}").expect("write revocation fixture");
        }
        let entries = parse_revocation_contents(&exact).expect("exact revocation set");
        assert_eq!(entries.len(), REVOCATION_LIST_MAX_ENTRIES_V1);
        writeln!(&mut exact, "{:064x}", REVOCATION_LIST_MAX_ENTRIES_V1)
            .expect("write overflow entry");
        assert!(parse_revocation_contents(&exact).is_err());
    }
    #[test]
    fn signed_ticket_secret_file_uses_source_derived_limit() {
        let expected = MlDsaSuite::MlDsa44.secret_key_len();
        let path = temporary_file_path("signed_ticket_secret");
        fs::write(&path, "ab".repeat(expected)).expect("write exact secret fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("protect secret fixture");
        }
        let options = SignedTicketSecretOptions {
            secret_path: Some(path.clone()),
        };
        assert_eq!(
            load_signed_ticket_secret(&options)
                .expect("load exact secret")
                .expect("secret present")
                .len(),
            expected
        );
        fs::write(
            &path,
            "0".repeat(secret_file_max_bytes(expected).expect("secret limit") + 1),
        )
        .expect("write oversized secret fixture");
        assert!(load_signed_ticket_secret(&options).is_err());
        let _ = fs::remove_file(path);
    }
    fn test_puzzle_parameters(
        difficulty: u8,
        max_future_skew: Duration,
        min_ticket_ttl: Duration,
    ) -> PuzzleParameters {
        PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            difficulty,
            max_future_skew,
            min_ticket_ttl,
        )
        .expect("test puzzle parameters must be valid")
    }
    fn base_service() -> PuzzleService {
        let min_ticket_ttl = Duration::from_secs(30);
        let max_future_skew = Duration::from_secs(120);
        PuzzleService {
            descriptor_commit: [0u8; 32],
            relay_id: [0u8; 32],
            puzzle_params: test_puzzle_parameters(5, max_future_skew, min_ticket_ttl),
            ticket_ttl: Duration::from_secs(45),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_issuer: None,
            token: None,
        }
    }
    #[test]
    #[allow(unsafe_code)]
    fn secret_holders_redact_debug_and_clear_private_bytes() {
        let timing = TokenTiming {
            min_ttl: Duration::from_secs(1),
            max_ttl: Duration::from_secs(2),
            default_ttl: Duration::from_secs(1),
            clock_skew: Duration::from_secs(1),
        };
        let mut issuer_secret = Vec::with_capacity(36);
        let issuer_secret_capacity = issuer_secret.capacity();
        issuer_secret.resize(issuer_secret_capacity, 222);
        issuer_secret.truncate(4);
        let mut issuer = TokenIssuer::new(
            MlDsaSuite::MlDsa44,
            issuer_secret.into(),
            [1; 32],
            [2; 32],
            timing,
            HashSet::new(),
            None,
        );
        let rendered = format!("{issuer:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("222"));
        issuer.clear_secret_key();
        assert!(issuer.secret_key.is_empty());
        assert_eq!(issuer.secret_key.0.capacity(), issuer_secret_capacity);
        // SAFETY: `SensitiveBytes::clear` initializes and wipes the complete
        // allocation immediately before clearing its logical length.
        unsafe { issuer.secret_key.0.set_len(issuer_secret_capacity) };
        assert!(issuer.secret_key.iter().all(|byte| *byte == 0));
        issuer.secret_key.0.clear();

        let mut service = base_service();
        service.signed_ticket_issuer = Some(SignedTicketIssuer {
            public_key: vec![1],
            secret_key: vec![205; 4].into(),
        });
        let rendered = format!("{service:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("205"));
    }
    fn first_rejected_puzzle_relay_id(
        ticket: &PowTicket,
        params: &PuzzleParameters,
        descriptor_commit: &[u8; 32],
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
        verify_time: SystemTime,
    ) -> [u8; 32] {
        for seed in 1u8..=u8::MAX {
            let candidate = [seed; 32];
            if &candidate == relay_id {
                continue;
            }
            let binding = PuzzleBinding::new(descriptor_commit, &candidate, transcript_hash);
            match puzzle::verify_at(ticket, &binding, params, verify_time) {
                Err(puzzle::Error::InvalidSolution) => return candidate,
                Ok(()) => {}
                Err(err) => panic!("unexpected puzzle verification error: {err}"),
            }
        }
        panic!("failed to find a relay binding rejected by the puzzle predicate")
    }
    fn first_rejected_puzzle_transcript_hash(
        ticket: &PowTicket,
        params: &PuzzleParameters,
        descriptor_commit: &[u8; 32],
        relay_id: &[u8; 32],
        transcript_hash: &[u8; 32],
        verify_time: SystemTime,
    ) -> [u8; 32] {
        for seed in 1u8..=u8::MAX {
            let candidate = [seed; 32];
            if &candidate == transcript_hash {
                continue;
            }
            let binding = PuzzleBinding::new(descriptor_commit, relay_id, &candidate);
            match puzzle::verify_at(ticket, &binding, params, verify_time) {
                Err(puzzle::Error::InvalidSolution) => return candidate,
                Ok(()) => {}
                Err(err) => panic!("unexpected puzzle verification error: {err}"),
            }
        }
        panic!("failed to find a transcript binding rejected by the puzzle predicate")
    }
    fn token_service() -> (PuzzleService, AdmissionTokenVerifier) {
        let min_ticket_ttl = Duration::from_secs(30);
        let max_future_skew = Duration::from_secs(180);
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let secret_key = keypair.secret_key().to_vec();
        let public_key = keypair.public_key().to_vec();
        let issuer_fingerprint = compute_issuer_fingerprint(keypair.public_key());
        let relay_keypair = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
            .expect("derive admission-token relay fixture key");
        let (algorithm, relay_public) = relay_keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        assert_eq!(algorithm, Algorithm::Ed25519);
        assert_eq!(relay_public.len(), 32);
        let mut relay_id = [0u8; 32];
        relay_id.copy_from_slice(relay_public);
        let min_ttl = min_ticket_ttl;
        let max_ttl = Duration::from_secs(240);
        let default_ttl = Duration::from_secs(90);
        let clock_skew = Duration::from_secs(5);
        let timing = TokenTiming {
            min_ttl,
            max_ttl,
            default_ttl,
            clock_skew,
        };
        let issuer = TokenIssuer::new(
            MlDsaSuite::MlDsa44,
            secret_key.into(),
            issuer_fingerprint,
            relay_id,
            timing,
            HashSet::new(),
            None,
        );
        let replay_limits =
            TokenStoreLimits::new(4, Duration::from_secs(300)).expect("fixture replay limits");
        let replay_store: Arc<Mutex<dyn TokenStore + Send>> = Arc::new(Mutex::new(
            InMemoryTokenStore::new(replay_limits).expect("fixture replay store"),
        ));
        let verifier = AdmissionTokenVerifier::try_new(
            MlDsaSuite::MlDsa44,
            public_key,
            max_ttl,
            clock_skew,
            replay_store,
        )
        .expect("generated verifier key must match ML-DSA-44");
        let service = PuzzleService {
            descriptor_commit: [0u8; 32],
            relay_id,
            puzzle_params: test_puzzle_parameters(5, max_future_skew, min_ticket_ttl),
            ticket_ttl: Duration::from_secs(45),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_issuer: None,
            token: Some(Mutex::new(issuer)),
        };
        (service, verifier)
    }
    #[test]
    fn token_service_relay_id_uses_checked_fixture_seed() {
        let (service, _) = token_service();
        let relay_keypair = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
            .expect("derive admission-token relay fixture key");
        let (algorithm, relay_public) = relay_keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        assert_eq!(algorithm, Algorithm::Ed25519);
        assert_eq!(service.relay_id.as_slice(), relay_public);
    }
    #[test]
    fn token_issuer_requires_an_explicit_private_cli_secret_path() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("token fixture keypair");
        let pow = PowConfig {
            token: Some(soranet_relay::config::TokenConfig {
                enabled: true,
                issuer_public_key_hex: Some(hex::encode(keypair.public_key())),
                ..soranet_relay::config::TokenConfig::default()
            }),
            ..PowConfig::default()
        };
        let options = TokenCliOptions {
            secret_path: None,
            revocation_file: None,
            revocation_refresh_secs: 30,
        };
        let error = match token_issuer_from_config(
            [0x11; 32],
            &pow,
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            &options,
        ) {
            Err(error) => error,
            Ok(_) => panic!("missing token secret path must fail closed"),
        };
        assert!(matches!(error, TokenInitError::MissingSecretKey));
    }
    #[test]
    fn token_issuer_rejects_mismatched_keypair_at_startup() {
        let configured =
            generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("configured token keypair");
        let signing = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("signing token keypair");
        let secret_path = temporary_file_path("token_mismatched_secret");
        fs::write(&secret_path, hex::encode(signing.secret_key()))
            .expect("write mismatched token secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o600))
                .expect("protect token secret fixture");
        }
        let mut pow = PowConfig::default();
        pow.token = Some(soranet_relay::config::TokenConfig {
            enabled: true,
            issuer_public_key_hex: Some(hex::encode(configured.public_key())),
            ..soranet_relay::config::TokenConfig::default()
        });
        let options = TokenCliOptions {
            secret_path: Some(secret_path.clone()),
            revocation_file: None,
            revocation_refresh_secs: 30,
        };
        let error = match token_issuer_from_config(
            [0x11; 32],
            &pow,
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            &options,
        ) {
            Err(error) => error,
            Ok(_) => panic!("mismatched token issuer keypair must fail closed"),
        };
        assert!(matches!(error, TokenInitError::KeyPairMismatch));
        let _ = fs::remove_file(secret_path);
    }
    fn signed_ticket_service() -> (PuzzleService, Vec<u8>, Vec<u8>) {
        let min_ticket_ttl = Duration::from_secs(60);
        let max_future_skew = Duration::from_secs(180);
        let puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            max_future_skew,
            min_ticket_ttl,
        )
        .expect("test puzzle parameters must be valid");
        let kp = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let secret = kp.secret_key().to_vec();
        let public = kp.public_key().to_vec();
        let mut relay_id = [0u8; 32];
        relay_id.copy_from_slice(&[0xCD; 32]);
        let mut descriptor_commit = [0u8; 32];
        descriptor_commit.copy_from_slice(&[0xAB; 32]);
        let service = PuzzleService {
            descriptor_commit,
            relay_id,
            puzzle_params,
            ticket_ttl: Duration::from_secs(90),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_issuer: Some(SignedTicketIssuer {
                public_key: public.clone(),
                secret_key: secret.clone().into(),
            }),
            token: None,
        };
        (service, secret, public)
    }
    #[test]
    fn clamp_ttl_respects_bounds() {
        let service = base_service();
        let min = service.puzzle_params.min_ticket_ttl();
        let max = service.puzzle_params.max_future_skew();
        assert_eq!(service.clamp_ttl(Some(Duration::from_secs(5))), min);
        assert_eq!(service.clamp_ttl(Some(Duration::from_secs(500))), max);
        assert_eq!(service.clamp_ttl(None), Duration::from_secs(45));
    }
    #[test]
    fn mint_ticket_uses_mandatory_puzzle() {
        let service = base_service();
        let mut rng = StdRng::from_seed([7u8; 32]);
        let ttl = service.clamp_ttl(Some(Duration::from_secs(40)));
        let ticket = service
            .mint_ticket(ttl, [0x10; 32], &mut rng)
            .expect("puzzle mint should succeed");
        assert_eq!(ticket.difficulty, service.puzzle_params.difficulty());
        assert!(ticket.expires_at > 0);
    }
    #[test]
    fn mint_ticket_uses_puzzle_when_configured() {
        let mut service = base_service();
        service.puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(8_192).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            6,
            Duration::from_secs(90),
            Duration::from_secs(30),
        )
        .expect("test puzzle parameters must be valid");
        let mut rng = StdRng::from_seed([9u8; 32]);
        let ttl = service.clamp_ttl(Some(Duration::from_secs(60)));
        let ticket = service
            .mint_ticket(ttl, [0x20; 32], &mut rng)
            .expect("puzzle mint should succeed");
        assert_eq!(ticket.difficulty, service.puzzle_params.difficulty());
        assert!(ticket.expires_at > 0);
    }
    #[test]
    fn puzzle_tickets_bind_relay_identity() {
        let mut service = base_service();
        service.descriptor_commit = [0xAB; 32];
        service.relay_id = [0xCD; 32];
        service.puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            1,
            Duration::from_secs(90),
            Duration::from_secs(30),
        )
        .expect("test puzzle parameters must be valid");
        let mut rng = StdRng::seed_from_u64(42);
        let transcript = [0xAA; 32];
        let ttl = service.clamp_ttl(Some(Duration::from_secs(40)));
        let ticket = service
            .mint_ticket(ttl, transcript, &mut rng)
            .expect("puzzle mint should succeed");
        let params = &service.puzzle_params;
        let binding =
            PuzzleBinding::new(&service.descriptor_commit, &service.relay_id, &transcript);
        let verify_time = ticket
            .checked_expires_at_time()
            .expect("fixture expiry must be representable")
            .checked_sub(params.min_ticket_ttl())
            .expect("fixture expiry must exceed the minimum ticket ttl");
        puzzle::verify_at(&ticket, &binding, params, verify_time)
            .expect("verification should succeed");
        // A difficulty-one work predicate admits half of all independent
        // challenges by construction. Select deterministic alternate bindings
        // that do not also satisfy the predicate instead of assuming a fixed
        // alternate can never be a valid cross-challenge collision.
        let wrong_relay_id = first_rejected_puzzle_relay_id(
            &ticket,
            params,
            &service.descriptor_commit,
            &service.relay_id,
            &transcript,
            verify_time,
        );
        let wrong_binding =
            PuzzleBinding::new(&service.descriptor_commit, &wrong_relay_id, &transcript);
        let err = puzzle::verify_at(&ticket, &wrong_binding, params, verify_time)
            .expect_err("alternate relay that misses the work predicate must fail");
        assert!(matches!(err, puzzle::Error::InvalidSolution));
        let wrong_transcript_hash = first_rejected_puzzle_transcript_hash(
            &ticket,
            params,
            &service.descriptor_commit,
            &service.relay_id,
            &transcript,
            verify_time,
        );
        let wrong_transcript = PuzzleBinding::new(
            &service.descriptor_commit,
            &service.relay_id,
            &wrong_transcript_hash,
        );
        let err = puzzle::verify_at(&ticket, &wrong_transcript, params, verify_time)
            .expect_err("alternate transcript that misses the work predicate must fail");
        assert!(matches!(err, puzzle::Error::InvalidSolution));
    }
    #[tokio::test]
    async fn http_puzzle_mint_binds_transcript() {
        use axum::{body::Bytes, extract::State};
        let mut service = base_service();
        service.descriptor_commit = [0x01; 32];
        service.relay_id = [0x02; 32];
        service.puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            3,
            Duration::from_secs(90),
            Duration::from_secs(30),
        )
        .expect("test puzzle parameters must be valid");
        let state = Arc::new(service);
        let transcript = [0x44; 32];
        let payload = format!(
            "{{\"ttl_secs\":60,\"transcript_hash_hex\":\"{}\"}}",
            hex::encode(transcript)
        );
        let response = mint_ticket(State(state.clone()), Bytes::from(payload.into_bytes()))
            .await
            .expect("mint response")
            .0;
        let minted: MintResponse = norito::json::from_slice(&response).expect("decode mint");
        assert_eq!(minted.credential_kind, "raw");
        assert!(minted.signed_ticket_fingerprint_hex.is_none());
        let ticket_bytes = STANDARD
            .decode(minted.credential_b64.as_bytes())
            .expect("base64 decode");
        let ticket = PowTicket::parse(&ticket_bytes).expect("ticket parse");
        let params = &state.puzzle_params;
        let binding = PuzzleBinding::new(&state.descriptor_commit, &state.relay_id, &transcript);
        puzzle::verify(&ticket, &binding, params).expect("verification succeeds");
    }
    #[tokio::test]
    async fn http_puzzle_mint_returns_signed_ticket() {
        use axum::{body::Bytes, extract::State};
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let mut service = base_service();
        service.relay_id = [0x12; 32];
        service.puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            service.puzzle_params.difficulty(),
            service.max_future_skew,
            service.min_ticket_ttl,
        )
        .expect("test puzzle parameters must be valid");
        service.signed_ticket_issuer = Some(SignedTicketIssuer {
            public_key: keypair.public_key().to_vec(),
            secret_key: keypair.secret_key().to_vec().into(),
        });
        let state = Arc::new(service);
        let transcript = [0xAB; 32];
        let payload = format!(
            "{{\"ttl_secs\":60,\"transcript_hash_hex\":\"{}\"}}",
            hex::encode(transcript)
        );
        let response = mint_ticket(State(state.clone()), Bytes::from(payload.into_bytes()))
            .await
            .expect("mint response")
            .0;
        let minted: MintResponse = norito::json::from_slice(&response).expect("decode mint");
        assert_eq!(minted.credential_kind, "signed");
        let signed_bytes = STANDARD
            .decode(minted.credential_b64.as_bytes())
            .expect("decode signed ticket");
        let signed = SignedTicket::decode(&signed_bytes).expect("decode signed ticket payload");
        assert_eq!(signed.relay_id, state.relay_id);
        assert_eq!(signed.transcript_hash, transcript);
        let binding = PuzzleBinding::new(&state.descriptor_commit, &state.relay_id, &transcript);
        puzzle::verify_signed_ticket(
            &signed,
            &state
                .signed_ticket_issuer
                .as_ref()
                .expect("signed ticket issuer")
                .public_key,
            &binding,
            &state.puzzle_params,
        )
        .expect("signed Argon2 ticket should verify");
        assert_eq!(
            minted
                .signed_ticket_fingerprint_hex
                .as_deref()
                .expect("fingerprint"),
            hex::encode(signed.revocation_fingerprint())
        );
    }
    #[test]
    fn token_summary_disabled_defaults() {
        let service = base_service();
        let summary = service.token_summary().expect("summary");
        assert!(!summary.enabled);
        assert!(summary.suite.is_none());
        assert!(summary.revocation_ids_hex.is_empty());
    }
    #[test]
    fn token_summary_enabled_reports_details() {
        let (service, _) = token_service();
        let summary = service.token_summary().expect("summary");
        assert!(summary.enabled);
        assert_eq!(summary.suite.as_deref(), Some("ml-dsa-44"));
        assert_eq!(summary.min_ttl_secs, Some(service.min_ticket_ttl.as_secs()));
        assert_eq!(summary.max_ttl_secs, Some(240));
    }
    #[test]
    fn public_token_summary_never_refreshes_revocation_file() {
        let (service, _) = token_service();
        let missing_path = temporary_file_path("missing_public_revocations");
        let _ = fs::remove_file(&missing_path);
        service
            .token
            .as_ref()
            .expect("token issuer")
            .lock()
            .expect("token issuer lock")
            .revocation_file = Some(RevocationFile {
            path: missing_path,
            refresh_interval: Duration::from_nanos(1),
            last_loaded: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .expect("representable prior instant"),
            entries: HashSet::new(),
        });

        service
            .public_token_summary()
            .expect("public metadata must not touch revocation storage");
        assert!(matches!(
            service.token_summary(),
            Err(TokenIssuerError::Revocation(_))
        ));
    }
    #[test]
    fn public_token_summary_fails_fast_while_issuer_is_busy() {
        let (service, _) = token_service();
        let service = Arc::new(service);
        let contender = Arc::clone(&service);
        let issuer_guard = service
            .token
            .as_ref()
            .expect("token issuer")
            .lock()
            .expect("issuer lock");
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            sender
                .send(matches!(
                    contender.public_token_summary(),
                    Err(TokenIssuerError::Busy)
                ))
                .expect("report public summary result");
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(100)),
            Ok(true),
            "public metadata must never wait for the issuer mutex"
        );
        drop(issuer_guard);
        worker.join().expect("summary worker");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn protected_token_summary_keeps_mutex_wait_off_reactor() {
        let (service, _) = token_service();
        let service = Arc::new(service);
        let locker = Arc::clone(&service);
        let (locked_sender, locked_receiver) = std::sync::mpsc::sync_channel(1);
        let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(1);
        let lock_worker = std::thread::spawn(move || {
            let _guard = locker
                .token
                .as_ref()
                .expect("token issuer")
                .lock()
                .expect("issuer lock");
            locked_sender.send(()).expect("report locked issuer");
            let _ = release_receiver.recv_timeout(Duration::from_millis(500));
        });
        locked_receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("issuer lock acquired");

        let summary_task = tokio::spawn(get_token_config(State(Arc::clone(&service))));
        let started = Instant::now();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "protected summary parked the single Tokio reactor"
        );
        release_sender.send(()).expect("release issuer lock");
        summary_task
            .await
            .expect("summary task")
            .expect("protected summary");
        lock_worker.join().expect("lock worker");
    }
    #[tokio::test]
    async fn public_puzzle_config_redacts_token_revocation_ids() {
        let (service, _) = token_service();
        let revoked = [0xA5; 32];
        service
            .token
            .as_ref()
            .expect("token issuer")
            .lock()
            .expect("token issuer lock")
            .static_revocations
            .insert(revoked);
        assert_eq!(
            service
                .token_summary()
                .expect("protected token summary")
                .revocation_ids_hex,
            vec![hex::encode(revoked)]
        );

        let public = get_config(State(Arc::new(service)))
            .await
            .expect("public puzzle config")
            .0;
        let public = String::from_utf8(public.to_vec()).expect("public config UTF-8");
        assert!(public.contains("\"revocation_ids_hex\":[]"));
        assert!(!public.contains(&hex::encode(revoked)));
    }
    #[test]
    fn mint_signed_ticket_when_configured() {
        let (service, secret, public) = signed_ticket_service();
        let transcript_hash = [0x11; 32];
        let mut rng = StdRng::from_seed([0x55; 32]);
        let pow_ticket = service
            .mint_ticket(service.ticket_ttl, transcript_hash, &mut rng)
            .expect("mint ticket");
        let signed = SignedTicket::sign(pow_ticket, &service.relay_id, &transcript_hash, &secret)
            .expect("sign ticket");
        let binding = PuzzleBinding::new(
            &service.descriptor_commit,
            &service.relay_id,
            &transcript_hash,
        );
        puzzle::verify_signed_ticket(&signed, &public, &binding, &service.puzzle_params)
            .expect("signed ticket should verify");
    }
    #[tokio::test]
    async fn http_mint_signed_ticket_returns_signed_payload() {
        use axum::{body::Bytes, extract::State};
        let (service, _secret, public) = signed_ticket_service();
        let state = Arc::new(service);
        let request = MintRequest {
            ttl_secs: Some(90),
            transcript_hash_hex: "11".repeat(32),
        };
        let body = Bytes::from(json::to_vec(&request).expect("serialize request"));
        let response = mint_ticket(State(Arc::clone(&state)), body)
            .await
            .expect("mint response");
        let parsed: MintResponse =
            json::from_slice(&response.0).expect("deserialize mint response");
        assert_eq!(parsed.credential_kind, "signed");
        let fingerprint = parsed
            .signed_ticket_fingerprint_hex
            .as_ref()
            .expect("fingerprint missing")
            .clone();
        let signed_bytes = STANDARD
            .decode(parsed.credential_b64.as_bytes())
            .expect("decode signed ticket");
        let signed = SignedTicket::decode(&signed_bytes).expect("decode signed ticket");
        let binding = PuzzleBinding::new(&state.descriptor_commit, &state.relay_id, &[0x11; 32]);
        puzzle::verify_signed_ticket(&signed, &public, &binding, &state.puzzle_params)
            .expect("signed ticket verifies");
        assert_eq!(
            fingerprint,
            hex::encode(signed.revocation_fingerprint()),
            "fingerprint must track the signed ticket signature"
        );
    }
    #[tokio::test]
    async fn http_mint_rejects_retired_signed_selector() {
        use axum::{body::Bytes, extract::State};
        let service = base_service();
        let state = Arc::new(service);
        let body = Bytes::from(format!(
            "{{\"ttl_secs\":60,\"transcript_hash_hex\":\"{}\",\"signed\":true}}",
            "22".repeat(32)
        ));
        let err = mint_ticket(State(state), body)
            .await
            .expect_err("retired client-side signing selector must fail");
        assert!(matches!(err, ApiError::BadRequest(_)));
    }
    #[tokio::test]
    async fn http_mint_puzzle_rejects_ttl_without_solution_window() {
        use axum::{body::Bytes, extract::State};
        let mut service = base_service();
        service.puzzle_params = PuzzleParameters::try_new(
            NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
            NonZeroU32::new(1).expect("non-zero iterations"),
            NonZeroU32::new(1).expect("non-zero lanes"),
            1,
            service.max_future_skew,
            service.min_ticket_ttl,
        )
        .expect("test puzzle parameters must be valid");
        let state = Arc::new(service);
        let request = MintRequest {
            ttl_secs: Some(state.min_ticket_ttl.as_secs()),
            transcript_hash_hex: "33".repeat(32),
        };
        let body = Bytes::from(json::to_vec(&request).expect("serialize request"));
        let err = mint_ticket(State(state), body)
            .await
            .expect_err("puzzle ttl equal to minimum remainder must fail before Argon2 work");
        assert!(matches!(
            err,
            ApiError::BadRequest(message)
                if message.contains("must exceed") && message.contains("minimum remaining ttl")
        ));
    }
    #[tokio::test]
    async fn http_mint_without_transcript_binding_is_rejected() {
        use axum::{body::Bytes, extract::State};
        let state = Arc::new(base_service());
        let err = mint_ticket(State(Arc::clone(&state)), Bytes::new())
            .await
            .expect_err("unbound ticket minting must fail");
        assert!(matches!(
            err,
            ApiError::BadRequest(message) if message == "transcript_hash_hex is required"
        ));
        let err = mint_ticket(State(state), Bytes::from_static(b"{}"))
            .await
            .expect_err("JSON without a transcript binding must fail");
        assert!(
            matches!(err, ApiError::BadRequest(ref message) if message.contains("transcript_hash_hex")),
            "unexpected error: {err:?}"
        );
    }
    #[tokio::test]
    async fn http_mint_rejects_zero_transcript_binding() {
        use axum::{body::Bytes, extract::State};
        let state = Arc::new(base_service());
        let request = MintRequest {
            ttl_secs: Some(60),
            transcript_hash_hex: "00".repeat(32),
        };
        let body = Bytes::from(json::to_vec(&request).expect("serialize request"));
        let err = mint_ticket(State(state), body)
            .await
            .expect_err("zero transcript binding must fail");
        assert!(matches!(
            err,
            ApiError::BadRequest(message)
                if message == "transcript_hash_hex must not be all zeros"
        ));
    }
    #[test]
    fn mint_token_rejects_out_of_range_ttl() {
        let (service, _) = token_service();
        let mut rng = StdRng::from_seed([0x55; 32]);
        let result = service.mint_token(
            Some(Duration::from_secs(1)),
            [0x11; 32],
            SystemTime::now(),
            &mut rng,
        );
        assert!(matches!(result, Err(TokenIssuerError::TtlTooShort { .. })));
    }
    #[test]
    fn mint_token_rejects_disabled_policy() {
        let service = base_service();
        let mut rng = StdRng::from_seed([0x56; 32]);
        let result = service.mint_token(None, [0x12; 32], SystemTime::now(), &mut rng);
        assert!(matches!(result, Err(TokenIssuerError::PolicyDisabled)));
    }
    #[test]
    fn mint_token_roundtrip_verifies() {
        let (service, verifier) = token_service();
        let mut rng = StdRng::from_seed([0x77; 32]);
        let issued_at = canonical_issued_at(SystemTime::now()).expect("canonical current time");
        let token = service
            .mint_token(None, [0x22; 32], issued_at, &mut rng)
            .expect("token enabled");
        verifier
            .verify(
                &token,
                token.relay_id(),
                token.transcript_hash(),
                SystemTime::now(),
            )
            .expect("verification succeeds");
    }
    #[tokio::test]
    async fn http_token_endpoints_issue_tokens() {
        use axum::{body::Bytes, extract::State};
        use std::time::SystemTime;
        let (service, verifier) = token_service();
        let state = Arc::new(service);
        let config_bytes = get_token_config(State(state.clone()))
            .await
            .expect("token config")
            .0;
        let summary: TokenConfigResponse =
            norito::json::from_slice(&config_bytes).expect("config decode");
        assert!(summary.enabled);
        assert_eq!(summary.suite.as_deref(), Some("ml-dsa-44"));
        let mint_payload = format!(
            "{{\"transcript_hash_hex\":\"{}\",\"ttl_secs\":120}}",
            hex::encode([0xAB; 32])
        );
        let mint_bytes = mint_token(State(state), Bytes::from(mint_payload.into_bytes()))
            .await
            .expect("mint")
            .0;
        let minted: MintTokenResponse = norito::json::from_slice(&mint_bytes).expect("mint decode");
        let token_bytes = STANDARD
            .decode(minted.token_b64.as_bytes())
            .expect("base64 decode");
        let token = AdmissionToken::decode(&token_bytes).expect("token decode");
        verifier
            .verify(
                &token,
                token.relay_id(),
                token.transcript_hash(),
                SystemTime::now(),
            )
            .expect("verification succeeds");
        assert_eq!(token.flags(), 0);
        assert_eq!(minted.token_id_hex, hex::encode(token.token_id()));
    }
    #[tokio::test]
    async fn http_token_mint_rejects_retired_flags() {
        use axum::{body::Bytes, extract::State};
        let (service, _) = token_service();
        let payload = format!(
            "{{\"transcript_hash_hex\":\"{}\",\"ttl_secs\":120,\"flags\":0}}",
            hex::encode([0xAC; 32])
        );
        let error = mint_token(State(Arc::new(service)), Bytes::from(payload))
            .await
            .expect_err("retired token flags input must fail");
        assert!(matches!(error, ApiError::BadRequest(_)));
    }
    #[test]
    fn derive_relay_id_requires_verified_certificate() {
        let policy = HandshakePolicy::default();
        let error = derive_relay_id(&policy, &[0u8; 32], 1)
            .expect_err("missing certificate must fail closed");
        assert!(
            matches!(error, TokenInitError::Handshake(message) if message.contains("handshake.certificate"))
        );
    }
    #[test]
    fn sensitive_responses_disable_intermediary_caching() {
        let mut response = Response::new(Body::empty());
        mark_sensitive_response(&mut response);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::PRAGMA], "no-cache");
    }

    #[test]
    fn mint_responses_redact_and_sensitive_json_owner_scrubs() {
        let mut mint = MintResponse {
            credential_kind: "raw".to_owned(),
            credential_b64: "raw-ticket-capability".to_owned(),
            signed_ticket_fingerprint_hex: Some("fingerprint".to_owned()),
            difficulty: 1,
            ttl_secs: 30,
            expires_at: 31,
        };
        let rendered = format!("{mint:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("raw-ticket-capability"));

        let mut token = MintTokenResponse {
            token_b64: "raw-admission-token-capability".to_owned(),
            token_id_hex: "01".repeat(32),
            issued_at: 1,
            expires_at: 31,
            ttl_secs: 30,
            issuer_fingerprint_hex: "02".repeat(32),
            relay_id_hex: "03".repeat(32),
        };
        let rendered = format!("{token:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("raw-admission-token-capability"));

        let encoded =
            SensitiveJsonBytes::from_serializable(&token).expect("encode sensitive mint response");
        assert!(std::mem::needs_drop::<SensitiveBytes>());
        assert!(
            encoded
                .0
                .windows("raw-admission-token-capability".len())
                .any(|window| window == b"raw-admission-token-capability")
        );
        let mut owner = encoded.0;
        owner.clear();
        assert!(owner.iter().all(|byte| *byte == 0));
        assert!(!format!("{owner:?}").contains("raw-admission-token-capability"));

        assert!(std::mem::needs_drop::<MintResponse>());
        assert!(std::mem::needs_drop::<MintTokenResponse>());
        mint.clear_credentials();
        token.clear_credentials();
        assert!(mint.credential_b64.is_empty());
        assert!(token.token_b64.is_empty());
    }
}
