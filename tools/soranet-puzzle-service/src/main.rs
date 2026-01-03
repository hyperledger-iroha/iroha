//! Argon2 puzzle issuance microservice backing the SoraNet relay handshake.

use std::{
    collections::HashSet,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use blake3::hash as blake3_hash;
use clap::Parser;
use color_eyre::eyre::{Context, Result, eyre};
use hex::{decode, encode};
use iroha_crypto::{
    Algorithm, KeyPair, PrivateKey,
    soranet::{
        pow::{self, Parameters as PowParameters, SignedTicket, Ticket as PowTicket},
        puzzle::{self, ChallengeBinding as PuzzleBinding, Parameters as PuzzleParameters},
        token::{AdmissionToken, MintError as AdmissionTokenMintError, compute_issuer_fingerprint},
    },
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use rand::{CryptoRng, RngCore, SeedableRng, rngs::StdRng};
use soranet_pq::{MlDsaSuite, sign_mldsa, verify_mldsa};
use soranet_relay::config::{
    ConfigError as RelayConfigError, HandshakePolicy, PowConfig, RelayConfig,
};
use thiserror::Error;
use tokio::{net::TcpListener, signal};
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt::SubscriberBuilder};

const FALLBACK_IDENTITY_SEED: [u8; 32] = [0x42; 32];

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
    /// Hex-encoded ML-DSA issuer secret key for admission tokens.
    #[arg(long)]
    token_secret_hex: Option<String>,
    /// Path to file containing hex-encoded ML-DSA issuer secret key.
    #[arg(long)]
    token_secret_path: Option<PathBuf>,
    /// Path to external revocation list (newline-separated hex token IDs).
    #[arg(long)]
    token_revocation_file: Option<PathBuf>,
    /// Refresh interval (seconds) for the revocation file when supplied.
    #[arg(long, default_value_t = 30)]
    token_revocation_refresh_secs: u64,
    /// Hex-encoded ML-DSA secret key for signing PoW tickets.
    #[arg(long)]
    signed_ticket_secret_hex: Option<String>,
    /// Path to file containing hex-encoded ML-DSA secret key for signing PoW tickets.
    #[arg(long)]
    signed_ticket_secret_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    init_tracing(&args.log_level)?;

    let service = PuzzleService::new(&args)?;
    let state = Arc::new(service);

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/puzzle/config", get(get_config))
        .route("/v1/puzzle/mint", post(mint_ticket))
        .route("/v1/token/config", get(get_token_config))
        .route("/v1/token/mint", post(mint_token))
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

struct PuzzleService {
    descriptor_commit: [u8; 32],
    relay_id: [u8; 32],
    pow_params: PowParameters,
    puzzle_params: Option<PuzzleParameters>,
    ticket_ttl: Duration,
    min_ticket_ttl: Duration,
    max_future_skew: Duration,
    pow_revocation_store_capacity: u64,
    pow_revocation_store_ttl_secs: u64,
    signed_ticket_public_key: Option<Vec<u8>>,
    signed_ticket_secret: Option<Vec<u8>>,
    token: Option<Mutex<TokenIssuer>>,
}

impl PuzzleService {
    fn new(args: &Args) -> Result<Self> {
        let config = RelayConfig::load(&args.config).wrap_err("failed to load relay config")?;
        let policy = config.handshake_policy();
        let relay_id =
            derive_relay_id(policy).wrap_err("failed to derive relay identity for bindings")?;

        let descriptor_commit = policy
            .descriptor_commit_bytes()
            .wrap_err("failed to parse descriptor_commit")?
            .ok_or_else(|| eyre!("handshake.descriptor_commit_hex must be configured"))?;

        let pow_cfg = config.pow_config().clone();
        let base_params = PowParameters::new(
            pow_cfg.difficulty.min(u8::MAX as u32) as u8,
            Duration::from_secs(pow_cfg.max_future_skew_secs),
            Duration::from_secs(pow_cfg.min_ticket_ttl_secs),
        );
        let puzzle_params = pow_cfg
            .puzzle_parameters(&base_params)
            .wrap_err("invalid puzzle configuration")?;
        let ticket_ttl = Duration::from_secs(pow_cfg.min_ticket_ttl_secs.max(1));

        let min_ticket_ttl = puzzle_params
            .as_ref()
            .map(PuzzleParameters::min_ticket_ttl)
            .unwrap_or_else(|| base_params.min_ticket_ttl());
        let max_future_skew = puzzle_params
            .as_ref()
            .map(PuzzleParameters::max_future_skew)
            .unwrap_or_else(|| base_params.max_future_skew());

        let token_opts = TokenCliOptions {
            secret_hex: args.token_secret_hex.clone(),
            secret_path: args.token_secret_path.clone(),
            revocation_file: args.token_revocation_file.clone(),
            revocation_refresh_secs: args.token_revocation_refresh_secs,
        };
        let signed_secret_opts = SignedTicketSecretOptions {
            secret_hex: args.signed_ticket_secret_hex.clone(),
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
                let bytes = decode(value)
                    .map_err(|err| eyre!("invalid signed_ticket_public_key_hex: {err}"))?;
                let expected = MlDsaSuite::MlDsa44.public_key_len();
                if bytes.len() != expected {
                    Err(eyre!(
                        "invalid signed_ticket_public_key_hex length: expected {expected} bytes for ML-DSA-44, got {}",
                        bytes.len()
                    ))
                } else {
                    Ok(bytes)
                }
            })
            .transpose()?;

        if signed_ticket_secret.is_some() && signed_ticket_public_key.is_none() {
            return Err(eyre!(
                "signed_ticket_secret_* supplied but pow.signed_ticket_public_key_hex missing from relay config"
            ));
        }
        if let (Some(secret), Some(public)) = (
            signed_ticket_secret.as_deref(),
            signed_ticket_public_key.as_deref(),
        ) {
            validate_signed_ticket_keypair(public, secret)?;
        }

        Ok(Self {
            descriptor_commit,
            relay_id,
            pow_params: base_params,
            puzzle_params,
            ticket_ttl,
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: pow_cfg.revocation_store_capacity,
            pow_revocation_store_ttl_secs: pow_cfg.revocation_store_ttl_secs,
            signed_ticket_public_key,
            signed_ticket_secret,
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
        self.signed_ticket_public_key.as_ref().map(encode)
    }

    fn signed_ticket_public_key_fingerprint_hex(&self) -> Option<String> {
        self.signed_ticket_public_key.as_ref().map(|key| {
            let fingerprint = blake3_hash(key);
            encode(fingerprint.as_bytes())
        })
    }

    fn mint_ticket<R: RngCore + CryptoRng>(
        &self,
        ttl: Duration,
        transcript_hash: Option<[u8; 32]>,
        rng: &mut R,
    ) -> Result<PowTicket, ChallengeMintError> {
        if let Some(params) = &self.puzzle_params {
            let binding = PuzzleBinding::new(
                &self.descriptor_commit,
                &self.relay_id,
                transcript_hash.as_ref().map(|hash| hash.as_slice()),
            );
            puzzle::mint_ticket(params, &binding, ttl, rng).map_err(ChallengeMintError::Puzzle)
        } else {
            let binding = pow::ChallengeBinding::new(
                &self.descriptor_commit,
                &self.relay_id,
                transcript_hash.as_ref().map(|hash| hash.as_slice()),
            );
            pow::mint_ticket(&self.pow_params, &binding, ttl, rng).map_err(ChallengeMintError::Pow)
        }
    }

    fn token_summary(&self) -> Result<TokenConfigResponse, TokenIssuerError> {
        if let Some(issuer_mutex) = &self.token {
            let mut issuer = issuer_mutex.lock().expect("token issuer mutex poisoned");
            issuer.refresh_revocations()?;
            Ok(TokenConfigResponse::enabled(&issuer))
        } else {
            Ok(TokenConfigResponse::disabled())
        }
    }

    fn mint_token<R: RngCore + CryptoRng>(
        &self,
        ttl_override: Option<Duration>,
        transcript_hash: [u8; 32],
        issued_at: SystemTime,
        flags: u8,
        rng: &mut R,
    ) -> Result<Option<AdmissionToken>, TokenIssuerError> {
        let Some(issuer_mutex) = &self.token else {
            return Ok(None);
        };
        let mut issuer = issuer_mutex.lock().expect("token issuer mutex poisoned");
        issuer.refresh_revocations()?;
        let ttl = issuer.clamp_ttl(ttl_override)?;
        issuer
            .mint(transcript_hash, ttl, issued_at, flags, rng)
            .map(Some)
    }
}

#[derive(Debug, Error)]
enum ChallengeMintError {
    #[error("pow ticket mint failed: {0}")]
    Pow(pow::MintError),
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
    #[error("descriptor manifest error: {message}")]
    DescriptorManifest { message: String },
    #[error("handshake configuration error: {0}")]
    Handshake(String),
}

impl From<RelayConfigError> for TokenInitError {
    fn from(err: RelayConfigError) -> Self {
        TokenInitError::Handshake(err.to_string())
    }
}

#[derive(Debug, Error)]
enum TokenIssuerError {
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
}

struct RevocationFile {
    path: PathBuf,
    refresh_interval: Duration,
    last_loaded: Instant,
    entries: HashSet<[u8; 32]>,
}

impl RevocationFile {
    fn new(path: PathBuf, refresh_interval: Duration) -> Result<Self, TokenInitError> {
        let contents =
            fs::read_to_string(&path).map_err(|error| TokenInitError::RevocationFile {
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
        let contents = fs::read_to_string(&self.path).map_err(|error| {
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
    secret_key: Vec<u8>,
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
    secret_hex: Option<String>,
    secret_path: Option<PathBuf>,
    revocation_file: Option<PathBuf>,
    revocation_refresh_secs: u64,
}

struct SignedTicketSecretOptions {
    secret_hex: Option<String>,
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
        secret_key: Vec<u8>,
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

    fn revocation_ids_hex(&self) -> Vec<String> {
        let mut ids: Vec<[u8; 32]> = self.static_revocations.iter().copied().collect();
        if let Some(file) = &self.revocation_file {
            ids.extend(file.entries.iter().copied());
        }
        ids.sort();
        ids.dedup();
        ids.into_iter().map(encode).collect()
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

#[derive(Debug, JsonSerialize)]
struct ConfigResponse {
    required: bool,
    difficulty: u8,
    max_future_skew_secs: u64,
    min_ticket_ttl_secs: u64,
    ticket_ttl_secs: u64,
    puzzle: Option<PuzzleParamsResponse>,
    token: TokenConfigResponse,
    #[norito(default)]
    revocation_store_capacity: Option<u64>,
    #[norito(default)]
    revocation_store_ttl_secs: Option<u64>,
    #[norito(default)]
    signed_ticket_public_key_hex: Option<String>,
    #[norito(default)]
    signed_ticket_public_key_fingerprint_hex: Option<String>,
    #[norito(default)]
    signed_ticket_signing_enabled: bool,
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

    fn enabled(issuer: &TokenIssuer) -> Self {
        Self {
            enabled: true,
            suite: Some(issuer.suite_label().to_string()),
            relay_id_hex: Some(encode(issuer.relay_id())),
            issuer_fingerprint_hex: Some(encode(issuer.issuer_fingerprint())),
            max_ttl_secs: Some(issuer.max_ttl().as_secs()),
            min_ttl_secs: Some(issuer.min_ttl().as_secs()),
            default_ttl_secs: Some(issuer.default_ttl().as_secs()),
            clock_skew_secs: Some(issuer.clock_skew().as_secs()),
            revocation_ids_hex: issuer.revocation_ids_hex(),
        }
    }
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct MintRequest {
    #[norito(default)]
    ttl_secs: Option<u64>,
    #[norito(default)]
    transcript_hash_hex: Option<String>,
    #[norito(default)]
    signed: bool,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct MintResponse {
    ticket_b64: String,
    #[norito(default)]
    signed_ticket_b64: Option<String>,
    #[norito(default)]
    signed_ticket_fingerprint_hex: Option<String>,
    difficulty: u8,
    ttl_secs: u64,
    expires_at: u64,
}

#[derive(Debug, JsonDeserialize)]
struct MintTokenRequest {
    transcript_hash_hex: String,
    #[norito(default)]
    ttl_secs: Option<u64>,
    #[norito(default)]
    flags: Option<u8>,
    #[norito(default)]
    issued_at_unix: Option<u64>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct MintTokenResponse {
    token_b64: String,
    token_id_hex: String,
    issued_at: u64,
    expires_at: u64,
    ttl_secs: u64,
    flags: u8,
    issuer_fingerprint_hex: String,
    relay_id_hex: String,
}

async fn get_config(State(state): State<Arc<PuzzleService>>) -> Result<JsonBytes, ApiError> {
    let token = state
        .token_summary()
        .map_err(|err| ApiError::Internal(format!("token summary error: {err}")))?;
    let response = ConfigResponse {
        required: state.pow_params.difficulty() > 0 || state.puzzle_params.is_some(),
        difficulty: state.pow_params.difficulty(),
        max_future_skew_secs: state.max_future_skew.as_secs(),
        min_ticket_ttl_secs: state.min_ticket_ttl.as_secs(),
        ticket_ttl_secs: state.ticket_ttl.as_secs(),
        puzzle: state.puzzle_params.as_ref().map(|p| PuzzleParamsResponse {
            memory_kib: p.memory_kib().get(),
            time_cost: p.time_cost().get(),
            lanes: p.lanes().get(),
        }),
        token,
        revocation_store_capacity: Some(state.pow_revocation_store_capacity),
        revocation_store_ttl_secs: Some(state.pow_revocation_store_ttl_secs),
        signed_ticket_public_key_hex: state.signed_ticket_public_key_hex(),
        signed_ticket_public_key_fingerprint_hex: state.signed_ticket_public_key_fingerprint_hex(),
        signed_ticket_signing_enabled: state.signed_ticket_secret.is_some(),
    };
    JsonBytes::from_serializable(&response)
}

async fn get_token_config(State(state): State<Arc<PuzzleService>>) -> Result<JsonBytes, ApiError> {
    let summary = state
        .token_summary()
        .map_err(|err| ApiError::Internal(format!("token summary error: {err}")))?;
    JsonBytes::from_serializable(&summary)
}

async fn mint_ticket(
    State(state): State<Arc<PuzzleService>>,
    body: Bytes,
) -> Result<JsonBytes, ApiError> {
    let payload = if body.is_empty() {
        MintRequest {
            ttl_secs: None,
            transcript_hash_hex: None,
            signed: false,
        }
    } else {
        json::from_slice::<MintRequest>(&body)
            .map_err(|err| ApiError::BadRequest(format!("invalid JSON body: {err}")))?
    };

    let ttl_override = payload.ttl_secs.map(Duration::from_secs);
    let ttl = state.clamp_ttl(ttl_override);
    let transcript_hash = match payload.transcript_hash_hex.as_ref() {
        Some(hex) => Some(hex_to_fixed::<32>(hex).map_err(|reason| {
            ApiError::BadRequest(format!("transcript_hash_hex invalid: {reason}"))
        })?),
        None => None,
    };
    let signed = payload.signed;

    let mut rng = StdRng::from_os_rng();
    let ticket = state
        .mint_ticket(ttl, transcript_hash, &mut rng)
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    let ticket_bytes = ticket.to_vec();
    let ticket_b64 = STANDARD.encode(&ticket_bytes);
    let mut signed_ticket_b64 = None;
    let mut signed_ticket_fingerprint_hex = None;
    if signed {
        let secret = state.signed_ticket_secret.as_ref().ok_or_else(|| {
            ApiError::BadRequest(
                "signed ticket requested but signing key not configured".to_string(),
            )
        })?;
        let signed_ticket =
            SignedTicket::sign(ticket, &state.relay_id, transcript_hash.as_ref(), secret)
                .map_err(|err| ApiError::Internal(format!("signed ticket mint failed: {err}")))?;
        signed_ticket_b64 = Some(STANDARD.encode(signed_ticket.encode()));
        signed_ticket_fingerprint_hex = Some(encode(signed_ticket.revocation_fingerprint()));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| ApiError::Internal(format!("system clock error: {err}")))?
        .as_secs();
    let expires_at = ticket.expires_at;
    let ttl_secs = expires_at.saturating_sub(now);

    let response = MintResponse {
        ticket_b64,
        signed_ticket_b64,
        signed_ticket_fingerprint_hex,
        difficulty: ticket.difficulty,
        ttl_secs,
        expires_at,
    };

    JsonBytes::from_serializable(&response)
}

async fn mint_token(
    State(state): State<Arc<PuzzleService>>,
    body: Bytes,
) -> Result<JsonBytes, ApiError> {
    if state.token.is_none() {
        return Err(ApiError::BadRequest(
            "admission token policy disabled on this relay".to_string(),
        ));
    }

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

    let ttl_override = payload.ttl_secs.map(Duration::from_secs);
    let issued_at = payload
        .issued_at_unix
        .map(|secs| UNIX_EPOCH + Duration::from_secs(secs))
        .unwrap_or_else(SystemTime::now);
    let flags = payload.flags.unwrap_or(0);

    let mut rng = StdRng::from_os_rng();
    let token = match state
        .mint_token(ttl_override, transcript_hash, issued_at, flags, &mut rng)
        .map_err(|err| match err {
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
        })? {
        Some(token) => token,
        None => {
            return Err(ApiError::BadRequest(
                "admission token policy disabled on this relay".to_string(),
            ));
        }
    };

    let token_bytes = token.encode();
    let token_b64 = STANDARD.encode(&token_bytes);
    let issued_at_secs = token.issued_at();
    let expires_at_secs = token.expires_at();
    let ttl_secs = expires_at_secs.saturating_sub(issued_at_secs);
    let response = MintTokenResponse {
        token_b64,
        token_id_hex: encode(token.token_id()),
        issued_at: issued_at_secs,
        expires_at: expires_at_secs,
        ttl_secs,
        flags: token.flags(),
        issuer_fingerprint_hex: encode(token.issuer_fingerprint()),
        relay_id_hex: encode(token.relay_id()),
    };
    JsonBytes::from_serializable(&response)
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

fn derive_relay_id(policy: &HandshakePolicy) -> Result<[u8; 32], TokenInitError> {
    let identity_seed = relay_identity_seed(policy)?;
    let private_key =
        PrivateKey::from_bytes(Algorithm::Ed25519, &identity_seed).map_err(|err| {
            TokenInitError::RelayIdentity(format!("failed to parse identity key: {err}"))
        })?;
    let identity_key = KeyPair::from_private_key(private_key).map_err(|err| {
        TokenInitError::RelayIdentity(format!("failed to derive identity keypair: {err}"))
    })?;
    let (algorithm, payload) = identity_key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err(TokenInitError::RelayIdentity(format!(
            "unsupported identity algorithm {algorithm:?}"
        )));
    }
    if payload.len() != 32 {
        return Err(TokenInitError::RelayIdentity(format!(
            "expected 32-byte identity public key, found {} bytes",
            payload.len()
        )));
    }
    let mut relay_id = [0u8; 32];
    relay_id.copy_from_slice(payload);
    Ok(relay_id)
}

fn relay_identity_seed(policy: &HandshakePolicy) -> Result<[u8; 32], TokenInitError> {
    if let Some(seed) = policy
        .identity_private_key_bytes()
        .map_err(TokenInitError::from)?
    {
        return Ok(seed);
    }
    if let Some(path) = policy.descriptor_manifest_path()
        && let Some(seed) = identity_seed_from_manifest(path)?
    {
        return Ok(seed);
    }
    warn!("relay identity key missing; using fallback test key");
    Ok(FALLBACK_IDENTITY_SEED)
}

fn identity_seed_from_manifest(path: &Path) -> Result<Option<[u8; 32]>, TokenInitError> {
    let bytes = fs::read(path).map_err(|error| TokenInitError::DescriptorManifest {
        message: format!("failed to read {}: {error}", path.display()),
    })?;
    let value: norito::json::Value =
        norito::json::from_slice(&bytes).map_err(|error| TokenInitError::DescriptorManifest {
            message: format!("failed to parse {}: {error}", path.display()),
        })?;
    let Some(hex) = extract_manifest_identity_private_key(&value) else {
        return Ok(None);
    };
    let seed = decode_manifest_identity_seed(hex).map_err(|message| {
        TokenInitError::DescriptorManifest {
            message: format!("{}: {message}", path.display()),
        }
    })?;
    Ok(Some(seed))
}

fn extract_manifest_identity_private_key(value: &norito::json::Value) -> Option<&str> {
    use norito::json::Value;

    match value {
        Value::Object(map) => {
            if let Some(hex) = map.get("identity_private_key_hex").and_then(Value::as_str) {
                return Some(hex);
            }
            if let Some(identity) = map.get("identity").and_then(Value::as_object) {
                if let Some(hex) = identity
                    .get("ed25519_private_key_hex")
                    .and_then(Value::as_str)
                {
                    return Some(hex);
                }
                if let Some(hex) = identity.get("private_key_hex").and_then(Value::as_str) {
                    return Some(hex);
                }
            }
            if let Some(relay) = map.get("relay").and_then(Value::as_object)
                && let Some(identity) = relay.get("identity").and_then(Value::as_object)
            {
                if let Some(hex) = identity
                    .get("ed25519_private_key_hex")
                    .and_then(Value::as_str)
                {
                    return Some(hex);
                }
                if let Some(hex) = identity.get("private_key_hex").and_then(Value::as_str) {
                    return Some(hex);
                }
            }
            for child in map.values() {
                if let Some(hex) = extract_manifest_identity_private_key(child) {
                    return Some(hex);
                }
            }
            None
        }
        Value::Array(array) => array
            .iter()
            .find_map(|item| extract_manifest_identity_private_key(item)),
        _ => None,
    }
}

fn decode_manifest_identity_seed(hex_value: &str) -> Result<[u8; 32], String> {
    let decoded =
        hex::decode(hex_value).map_err(|err| format!("identity private key hex invalid: {err}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "identity private key hex must decode to 32 bytes (got {})",
            decoded.len()
        ));
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&decoded);
    Ok(seed)
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
    let public_key =
        decode(public_hex).map_err(|err| TokenInitError::InvalidPublicKey(err.to_string()))?;
    if public_key.is_empty() {
        return Err(TokenInitError::InvalidPublicKey(
            "decoded public key is empty".to_string(),
        ));
    }
    let issuer_fingerprint = compute_issuer_fingerprint(&public_key);

    let secret_path = cli
        .secret_path
        .as_ref()
        .or(token_cfg.issuer_secret_key_path.as_ref());
    let secret_hex = cli
        .secret_hex
        .as_ref()
        .or(token_cfg.issuer_secret_key_hex.as_ref());

    let secret_key_bytes = if let Some(path) = secret_path {
        let contents = fs::read_to_string(path).map_err(|error| TokenInitError::SecretKeyIo {
            path: path.clone(),
            error,
        })?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Err(TokenInitError::InvalidSecretKey(
                "secret key file is empty".to_string(),
            ));
        }
        decode(trimmed).map_err(|err| TokenInitError::InvalidSecretKey(err.to_string()))?
    } else if let Some(hex) = secret_hex {
        decode(hex).map_err(|err| TokenInitError::InvalidSecretKey(err.to_string()))?
    } else {
        return Err(TokenInitError::MissingSecretKey);
    };

    let mut static_revocations = HashSet::new();
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
        MlDsaSuite::MlDsa44,
        secret_key_bytes,
        issuer_fingerprint,
        relay_id,
        timing,
        static_revocations,
        revocation_file,
    )))
}

fn load_signed_ticket_secret(opts: &SignedTicketSecretOptions) -> Result<Option<Vec<u8>>> {
    if opts.secret_hex.is_none() && opts.secret_path.is_none() {
        return Ok(None);
    }
    if opts.secret_hex.is_some() && opts.secret_path.is_some() {
        return Err(eyre!(
            "set only one of --signed-ticket-secret-hex or --signed-ticket-secret-path"
        ));
    }
    let source_hex = if let Some(path) = opts.secret_path.as_ref() {
        let contents = fs::read_to_string(path).wrap_err_with(|| {
            format!(
                "failed to read signed ticket secret from {}",
                path.display()
            )
        })?;
        contents.trim().to_string()
    } else if let Some(hex) = &opts.secret_hex {
        hex.trim().to_string()
    } else {
        String::new()
    };
    if source_hex.is_empty() {
        return Err(eyre!("signed ticket secret is empty"));
    }
    let decoded =
        decode(&source_hex).wrap_err_with(|| "failed to decode signed ticket secret as hex")?;
    let expected = MlDsaSuite::MlDsa44.secret_key_len();
    if decoded.len() != expected {
        return Err(eyre!(
            "signed ticket secret length invalid: expected {expected} bytes (ML-DSA-44), got {}",
            decoded.len()
        ));
    }
    Ok(Some(decoded))
}

fn validate_signed_ticket_keypair(public_key: &[u8], secret_key: &[u8]) -> Result<()> {
    let probe = b"soranet.pow.signed_ticket.key_check";
    let signature = sign_mldsa(MlDsaSuite::MlDsa44, secret_key, probe)
        .wrap_err("failed to sign probe with signed ticket secret key")?;
    verify_mldsa(MlDsaSuite::MlDsa44, public_key, probe, signature.as_bytes()).wrap_err(
        "pow.signed_ticket_public_key_hex does not match the provided signed ticket secret key",
    )
}

fn hex_to_fixed<const N: usize>(value: &str) -> Result<[u8; N], String> {
    let bytes = decode(value).map_err(|err| err.to_string())?;
    if bytes.len() != N {
        return Err(format!("expected {N} bytes, found {}", bytes.len()));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
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
        set.insert(entry);
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use iroha_crypto::soranet::{pow::ChallengeBinding, token::AdmissionTokenVerifier};
    use soranet_pq::generate_mldsa_keypair;

    use super::*;

    fn base_service() -> PuzzleService {
        let pow_params = PowParameters::new(5, Duration::from_secs(120), Duration::from_secs(30));
        let min_ticket_ttl = pow_params.min_ticket_ttl();
        let max_future_skew = pow_params.max_future_skew();
        PuzzleService {
            descriptor_commit: [0u8; 32],
            relay_id: [0u8; 32],
            pow_params,
            puzzle_params: None,
            ticket_ttl: Duration::from_secs(45),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_public_key: None,
            signed_ticket_secret: None,
            token: None,
        }
    }

    fn token_service() -> (PuzzleService, AdmissionTokenVerifier) {
        let pow_params = PowParameters::new(5, Duration::from_secs(180), Duration::from_secs(30));
        let min_ticket_ttl = pow_params.min_ticket_ttl();
        let max_future_skew = pow_params.max_future_skew();

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44)
            .expect("ML-DSA keypair generation should succeed");
        let secret_key = keypair.secret_key().to_vec();
        let public_key = keypair.public_key().to_vec();
        let issuer_fingerprint = compute_issuer_fingerprint(keypair.public_key());

        let relay_keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let (algorithm, relay_public) = relay_keypair.public_key().to_bytes();
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
            secret_key,
            issuer_fingerprint,
            relay_id,
            timing,
            HashSet::new(),
            None,
        );

        let verifier =
            AdmissionTokenVerifier::new(MlDsaSuite::MlDsa44, public_key, max_ttl, clock_skew);

        let service = PuzzleService {
            descriptor_commit: [0u8; 32],
            relay_id,
            pow_params,
            puzzle_params: None,
            ticket_ttl: Duration::from_secs(45),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_public_key: None,
            signed_ticket_secret: None,
            token: Some(Mutex::new(issuer)),
        };

        (service, verifier)
    }

    fn signed_ticket_service() -> (PuzzleService, Vec<u8>, Vec<u8>) {
        let pow_params = PowParameters::new(4, Duration::from_secs(180), Duration::from_secs(60));
        let min_ticket_ttl = pow_params.min_ticket_ttl();
        let max_future_skew = pow_params.max_future_skew();
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
            pow_params,
            puzzle_params: None,
            ticket_ttl: Duration::from_secs(90),
            min_ticket_ttl,
            max_future_skew,
            pow_revocation_store_capacity: 8_192,
            pow_revocation_store_ttl_secs: 900,
            signed_ticket_public_key: Some(public.clone()),
            signed_ticket_secret: Some(secret.clone()),
            token: None,
        };

        (service, secret, public)
    }

    #[test]
    fn clamp_ttl_respects_bounds() {
        let service = base_service();
        let min = service.pow_params.min_ticket_ttl();
        let max = service.pow_params.max_future_skew();

        assert_eq!(service.clamp_ttl(Some(Duration::from_secs(5))), min);
        assert_eq!(service.clamp_ttl(Some(Duration::from_secs(500))), max);
        assert_eq!(service.clamp_ttl(None), Duration::from_secs(45));
    }

    #[test]
    fn mint_ticket_uses_pow_when_puzzle_disabled() {
        let service = base_service();
        let mut rng = StdRng::from_seed([7u8; 32]);
        let ttl = service.clamp_ttl(Some(Duration::from_secs(40)));
        let ticket = service
            .mint_ticket(ttl, None, &mut rng)
            .expect("pow mint should succeed");

        assert_eq!(ticket.difficulty, service.pow_params.difficulty());
        assert!(ticket.expires_at > 0);
    }

    #[test]
    fn mint_ticket_uses_puzzle_when_configured() {
        let mut service = base_service();
        service.puzzle_params = Some(PuzzleParameters::new(
            NonZeroU32::new(8_192).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            6,
            Duration::from_secs(90),
            Duration::from_secs(30),
        ));

        let mut rng = StdRng::from_seed([9u8; 32]);
        let ttl = service.clamp_ttl(Some(Duration::from_secs(60)));
        let ticket = service
            .mint_ticket(ttl, None, &mut rng)
            .expect("puzzle mint should succeed");

        assert_eq!(
            ticket.difficulty,
            service.puzzle_params.as_ref().unwrap().difficulty()
        );
        assert!(ticket.expires_at > 0);
    }

    #[test]
    fn puzzle_tickets_bind_relay_identity() {
        let mut service = base_service();
        service.descriptor_commit = [0xAB; 32];
        service.relay_id = [0xCD; 32];
        service.puzzle_params = Some(PuzzleParameters::new(
            NonZeroU32::new(1_024).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            8,
            Duration::from_secs(90),
            Duration::from_secs(30),
        ));

        let mut rng = StdRng::seed_from_u64(42);
        let transcript = [0xAA; 32];
        let ttl = service.clamp_ttl(Some(Duration::from_secs(40)));
        let ticket = service
            .mint_ticket(ttl, Some(transcript), &mut rng)
            .expect("puzzle mint should succeed");

        let params = service.puzzle_params.as_ref().expect("params");
        let binding = PuzzleBinding::new(
            &service.descriptor_commit,
            &service.relay_id,
            Some(&transcript),
        );
        puzzle::verify(&ticket, &binding, params).expect("verification should succeed");

        let wrong_binding =
            PuzzleBinding::new(&service.descriptor_commit, &[0xEF; 32], Some(&transcript));
        let err =
            puzzle::verify(&ticket, &wrong_binding, params).expect_err("wrong relay id must fail");
        assert!(matches!(err, puzzle::Error::InvalidSolution));

        let wrong_transcript = PuzzleBinding::new(
            &service.descriptor_commit,
            &service.relay_id,
            Some(&[0xBB; 32]),
        );
        let err = puzzle::verify(&ticket, &wrong_transcript, params)
            .expect_err("wrong transcript must fail");
        assert!(matches!(err, puzzle::Error::InvalidSolution));
    }

    #[tokio::test]
    async fn http_puzzle_mint_binds_transcript() {
        use axum::{body::Bytes, extract::State};

        let mut service = base_service();
        service.descriptor_commit = [0x01; 32];
        service.relay_id = [0x02; 32];
        service.puzzle_params = Some(PuzzleParameters::new(
            NonZeroU32::new(512).unwrap(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
            3,
            Duration::from_secs(90),
            Duration::from_secs(30),
        ));
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
        let ticket_bytes = STANDARD
            .decode(minted.ticket_b64.as_bytes())
            .expect("base64 decode");
        let ticket = PowTicket::parse(&ticket_bytes).expect("ticket parse");

        let params = state.puzzle_params.as_ref().expect("params");
        let binding =
            PuzzleBinding::new(&state.descriptor_commit, &state.relay_id, Some(&transcript));
        puzzle::verify(&ticket, &binding, params).expect("verification succeeds");
    }

    #[tokio::test]
    async fn http_puzzle_mint_returns_signed_ticket() {
        use axum::{body::Bytes, extract::State};

        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
        let mut service = base_service();
        service.relay_id = [0x12; 32];
        service.signed_ticket_public_key = Some(keypair.public_key().to_vec());
        service.signed_ticket_secret = Some(keypair.secret_key().to_vec());
        let state = Arc::new(service);
        let transcript = [0xAB; 32];
        let payload = format!(
            "{{\"ttl_secs\":30,\"transcript_hash_hex\":\"{}\",\"signed\":true}}",
            hex::encode(transcript)
        );

        let response = mint_ticket(State(state.clone()), Bytes::from(payload.into_bytes()))
            .await
            .expect("mint response")
            .0;
        let minted: MintResponse = norito::json::from_slice(&response).expect("decode mint");
        let signed_b64 = minted
            .signed_ticket_b64
            .as_ref()
            .expect("signed ticket present");
        let signed_bytes = STANDARD
            .decode(signed_b64.as_bytes())
            .expect("decode signed ticket");
        let signed = SignedTicket::decode(&signed_bytes).expect("decode signed ticket payload");

        assert_eq!(signed.relay_id, state.relay_id);
        assert_eq!(signed.transcript_hash.as_ref(), Some(&transcript));

        let ticket_bytes = STANDARD
            .decode(minted.ticket_b64.as_bytes())
            .expect("decode ticket");
        let ticket = PowTicket::parse(&ticket_bytes).expect("parse minted ticket");
        assert_eq!(signed.ticket, ticket);

        signed
            .verify(state.signed_ticket_public_key.as_ref().expect("public key"))
            .expect("signature should verify");
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
    fn mint_signed_ticket_when_configured() {
        let (service, secret, public) = signed_ticket_service();
        let transcript_hash = [0x11; 32];
        let mut rng = StdRng::from_seed([0x55; 32]);
        let pow_ticket = service
            .mint_ticket(service.ticket_ttl, Some(transcript_hash), &mut rng)
            .expect("mint ticket");
        let signed = SignedTicket::sign(
            pow_ticket,
            &service.relay_id,
            Some(&transcript_hash),
            &secret,
        )
        .expect("sign ticket");

        let binding = ChallengeBinding::new(
            &service.descriptor_commit,
            &service.relay_id,
            Some(&transcript_hash),
        );
        pow::verify_signed_ticket(&signed, &public, &binding, &service.pow_params, None)
            .expect("signed ticket should verify");
    }

    #[tokio::test]
    async fn http_mint_signed_ticket_returns_signed_payload() {
        use axum::{body::Bytes, extract::State};

        let (service, _secret, public) = signed_ticket_service();
        let state = Arc::new(service);
        let request = MintRequest {
            ttl_secs: Some(30),
            transcript_hash_hex: Some("11".repeat(32)),
            signed: true,
        };
        let body = Bytes::from(json::to_vec(&request).expect("serialize request"));
        let response = mint_ticket(State(Arc::clone(&state)), body)
            .await
            .expect("mint response");
        let parsed: MintResponse =
            json::from_slice(&response.0).expect("deserialize mint response");
        let signed_b64 = parsed
            .signed_ticket_b64
            .as_ref()
            .expect("signed ticket missing")
            .clone();
        let fingerprint = parsed
            .signed_ticket_fingerprint_hex
            .as_ref()
            .expect("fingerprint missing")
            .clone();
        let signed_bytes = STANDARD.decode(signed_b64).expect("decode signed ticket");
        let signed = SignedTicket::decode(&signed_bytes).expect("decode signed ticket");
        let binding = ChallengeBinding::new(
            &state.descriptor_commit,
            &state.relay_id,
            Some(&[0x11; 32]),
        );
        pow::verify_signed_ticket(&signed, &public, &binding, &state.pow_params, None)
            .expect("signed ticket verifies");
        assert_eq!(
            fingerprint,
            hex::encode(signed.revocation_fingerprint()),
            "fingerprint must track the signed ticket signature"
        );
    }

    #[tokio::test]
    async fn http_mint_signed_ticket_without_secret_rejected() {
        use axum::{body::Bytes, extract::State};

        let service = base_service();
        let state = Arc::new(service);
        let request = MintRequest {
            ttl_secs: Some(30),
            transcript_hash_hex: None,
            signed: true,
        };
        let body = Bytes::from(json::to_vec(&request).expect("serialize request"));
        let err = mint_ticket(State(state), body)
            .await
            .expect_err("should fail");
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn mint_token_rejects_out_of_range_ttl() {
        let (service, _) = token_service();
        let mut rng = StdRng::from_seed([0x55; 32]);
        let result = service.mint_token(
            Some(Duration::from_secs(1)),
            [0x11; 32],
            SystemTime::now(),
            0,
            &mut rng,
        );
        assert!(matches!(result, Err(TokenIssuerError::TtlTooShort { .. })));
    }

    #[test]
    fn mint_token_roundtrip_verifies() {
        let (service, verifier) = token_service();
        let mut rng = StdRng::from_seed([0x77; 32]);
        let token = service
            .mint_token(None, [0x22; 32], SystemTime::now(), 0, &mut rng)
            .expect("mint result")
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
        use std::time::SystemTime;

        use axum::{body::Bytes, extract::State};

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
            "{{\"transcript_hash_hex\":\"{}\",\"ttl_secs\":120,\"flags\":0}}",
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
        assert_eq!(minted.flags, token.flags());
        assert_eq!(minted.token_id_hex, hex::encode(token.token_id()));
    }

    #[test]
    fn derive_relay_id_reads_manifest_identity() {
        let identity_seed = [0x24; 32];
        let manifest_path = std::env::temp_dir().join(format!(
            "soranet_manifest_identity_{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let manifest_body = format!(
            "{{\"identity_private_key_hex\":\"{}\"}}",
            hex::encode(identity_seed)
        );
        fs::write(&manifest_path, manifest_body).expect("write manifest");

        let policy = HandshakePolicy {
            descriptor_manifest_path: Some(manifest_path.clone()),
            ..HandshakePolicy::default()
        };

        let relay_id = derive_relay_id(&policy).expect("relay id");
        let expected = {
            let private_key =
                PrivateKey::from_bytes(Algorithm::Ed25519, &identity_seed).expect("private key");
            let pair = KeyPair::from_private_key(private_key).expect("keypair");
            let (algo, public) = pair.public_key().to_bytes();
            assert_eq!(algo, Algorithm::Ed25519);
            let mut id = [0u8; 32];
            id.copy_from_slice(public);
            id
        };
        assert_eq!(relay_id, expected);

        let _ = fs::remove_file(manifest_path);
    }
}
