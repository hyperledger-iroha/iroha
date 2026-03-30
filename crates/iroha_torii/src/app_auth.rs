//! Canonical request signing helpers for app-facing HTTP endpoints.
//!
//! Clients may optionally attach:
//! - `X-Iroha-Account`: account id that authorises the request.
//! - `X-Iroha-Signature`: base64 signature over the canonical request bytes plus
//!   freshness metadata.
//! - `X-Iroha-Timestamp-Ms`: unix timestamp in milliseconds included in the
//!   signed payload.
//! - `X-Iroha-Nonce`: caller-chosen nonce included in the signed payload.
//! - `X-Iroha-Witness`: base64 Norito witness for multisig-controlled accounts.
//!
//! The canonical request bytes are:
//! ```text
//! <UPPERCASE_METHOD>\n
//! <path>\n
//! <sorted_query_string>\n
//! <hex_sha256(body)>\n
//! <timestamp_ms>\n
//! <nonce>
//! ```
//! - Query parameters are parsed, percent-decoded (treating `+` as space), sorted
//!   by `(key, value)`, then re-encoded using `application/x-www-form-urlencoded`
//!   rules.
//! - The body hash is computed over the raw request body bytes.
//! - Freshness validation rejects stale timestamps and replayed nonces.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use dashmap::{DashMap, mapref::entry::Entry};
use iroha_config::parameters::{actual::AppApi as AppApiConfig, defaults};
use iroha_core::state::{State as CoreState, WorldReadOnly};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_data_model::{
    ValidationFail,
    account::{AccountController, AccountId},
    query::{
        ErasedIterQuery, Query, QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
        dsl::{CompoundPredicate, HasProjection, PredicateMarker, SelectorMarker, SelectorTuple},
        error::{FindError, QueryExecutionFail},
        parameters::QueryParams,
    },
    soracloud::{
        CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
        CanonicalRequestWitnessV1,
    },
};
use norito::codec::Encode;
use sha2::{Digest as _, Sha256};

/// Header carrying the authorising account id.
pub const HEADER_ACCOUNT: &str = "X-Iroha-Account";
/// Header carrying the base64-encoded signature over the canonical request bytes.
pub const HEADER_SIGNATURE: &str = "X-Iroha-Signature";
/// Header carrying the unix timestamp in milliseconds for freshness checks.
pub const HEADER_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
/// Header carrying the caller-chosen replay nonce.
pub const HEADER_NONCE: &str = "X-Iroha-Nonce";
/// Header carrying the base64 Norito-encoded multisig witness.
pub const HEADER_WITNESS: &str = "X-Iroha-Witness";
const ACCOUNT_HEADER_CONTEXT: &str = "X-Iroha-Account";
/// HTTP request types used for canonical signing.
pub use axum::http::{Method, Uri};

/// Canonical request freshness configuration.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalRequestAuthConfig {
    /// Maximum allowed clock skew for signed requests.
    pub max_clock_skew: Duration,
    /// TTL for nonces retained for replay detection.
    pub nonce_ttl: Duration,
    /// Maximum number of nonce entries held in memory for replay detection.
    pub replay_cache_capacity: NonZeroUsize,
}

impl Default for CanonicalRequestAuthConfig {
    fn default() -> Self {
        Self {
            max_clock_skew: Duration::from_secs(defaults::torii::app_auth::MAX_CLOCK_SKEW_SECS),
            nonce_ttl: Duration::from_secs(defaults::torii::app_auth::NONCE_TTL_SECS),
            replay_cache_capacity: NonZeroUsize::new(
                defaults::torii::app_auth::REPLAY_CACHE_CAPACITY.max(1),
            )
            .expect("default app-auth replay cache capacity must be non-zero"),
        }
    }
}

impl From<&AppApiConfig> for CanonicalRequestAuthConfig {
    fn from(value: &AppApiConfig) -> Self {
        Self {
            max_clock_skew: value.request_signature_max_clock_skew,
            nonce_ttl: value.request_signature_nonce_ttl,
            replay_cache_capacity: value.request_signature_replay_cache_capacity,
        }
    }
}

#[derive(Debug)]
struct ReplayCache {
    ttl: Duration,
    capacity: NonZeroUsize,
    entries: DashMap<String, Instant>,
    order: Mutex<VecDeque<(String, Instant)>>,
}

impl ReplayCache {
    fn new(ttl: Duration, capacity: NonZeroUsize) -> Self {
        Self {
            ttl: ttl.max(Duration::from_secs(1)),
            capacity,
            entries: DashMap::new(),
            order: Mutex::new(VecDeque::new()),
        }
    }

    fn check_and_insert(&self, key: String) -> bool {
        let now = Instant::now();
        let expires_at = now + self.ttl;

        match self.entries.entry(key.clone()) {
            Entry::Occupied(mut occ) => {
                if *occ.get() > now {
                    return false;
                }
                occ.insert(expires_at);
            }
            Entry::Vacant(vac) => {
                vac.insert(expires_at);
            }
        }

        if let Ok(mut guard) = self.order.lock() {
            guard.push_back((key, expires_at));
            self.prune_locked(&mut guard, now);
        }

        true
    }

    fn prune_locked(&self, order: &mut VecDeque<(String, Instant)>, now: Instant) {
        let cap = self.capacity.get();
        while let Some((_key, expiry)) = order.front() {
            if *expiry > now && order.len() <= cap {
                break;
            }
            let (key, expiry) = order
                .pop_front()
                .expect("front is Some so pop_front must succeed");
            let _ = self
                .entries
                .remove_if(&key, |_k, existing| *existing == expiry);
        }
    }
}

#[derive(Debug)]
struct CanonicalRequestAuthRuntime {
    config: CanonicalRequestAuthConfig,
    replay_cache: Arc<ReplayCache>,
}

impl CanonicalRequestAuthRuntime {
    fn new(config: CanonicalRequestAuthConfig) -> Self {
        Self {
            config,
            replay_cache: Arc::new(ReplayCache::new(
                config.nonce_ttl,
                config.replay_cache_capacity,
            )),
        }
    }
}

fn auth_runtime() -> &'static RwLock<CanonicalRequestAuthRuntime> {
    static STATE: OnceLock<RwLock<CanonicalRequestAuthRuntime>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(CanonicalRequestAuthRuntime::new(Default::default())))
}

fn auth_runtime_snapshot() -> (CanonicalRequestAuthConfig, Arc<ReplayCache>) {
    let guard = auth_runtime()
        .read()
        .expect("canonical request auth config lock");
    (guard.config, guard.replay_cache.clone())
}

/// Configure app-facing canonical request freshness enforcement.
pub fn configure(config: CanonicalRequestAuthConfig) {
    *auth_runtime()
        .write()
        .expect("canonical request auth config lock") = CanonicalRequestAuthRuntime::new(config);
}

/// Authenticated canonical request identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCanonicalRequest {
    /// Account declared in the canonical request headers.
    pub account: AccountId,
    /// Exact account controller key that verified the request signature.
    pub signer: PublicKey,
    /// Full signer set that satisfied the request authorisation.
    pub verified_signers: Vec<PublicKey>,
}

/// Canonicalise a raw query string by decoding, sorting, and re-encoding.
#[must_use]
pub fn canonical_query_string(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return String::new();
    };
    if raw.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        serializer.append_pair(&k, &v);
    }
    serializer.finish()
}

/// Construct canonical request bytes for signing.
#[must_use]
pub fn canonical_request_message(method: &Method, uri: &Uri, body: &[u8]) -> Vec<u8> {
    let query = canonical_query_string(uri.query());
    let mut hasher = Sha256::new();
    hasher.update(body);
    let body_hash = hasher.finalize();
    format!(
        "{}\n{}\n{}\n{}",
        method.as_str().to_ascii_uppercase(),
        uri.path(),
        query,
        hex::encode(body_hash)
    )
    .into_bytes()
}

/// Hash the canonical request bytes used by witness verification.
#[must_use]
pub fn canonical_request_hash(method: &Method, uri: &Uri, body: &[u8]) -> Hash {
    Hash::new(canonical_request_message(method, uri, body))
}

/// Construct canonical request bytes for signature verification with freshness metadata.
#[must_use]
pub fn canonical_request_signature_message(
    method: &Method,
    uri: &Uri,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    let mut msg = canonical_request_message(method, uri, body);
    msg.push(b'\n');
    msg.extend_from_slice(timestamp_ms.to_string().as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(nonce.as_bytes());
    msg
}

/// Encode a signature payload for use in `X-Iroha-Signature` headers.
#[must_use]
pub fn signature_header_value(signature: &Signature) -> String {
    BASE64_STANDARD.encode(signature.payload())
}

#[derive(Encode)]
struct CanonicalRequestWitnessPayloadV1 {
    schema_version: u16,
    subject_account: AccountId,
    timestamp_ms: u64,
    nonce: String,
    canonical_request_hash: Hash,
}

/// Construct the signed payload for a canonical request witness.
///
/// The payload binds the witness to the subject account, freshness fields, and
/// reconstructed canonical request hash. Individual signatures are supplied
/// separately in [`CanonicalRequestWitnessV1::signatures`].
///
/// # Errors
/// Returns [`norito::Error`] when witness encoding fails.
pub fn canonical_request_witness_message(
    witness: &CanonicalRequestWitnessV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::to_bytes(&CanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: witness.subject_account.clone(),
        timestamp_ms: witness.timestamp_ms,
        nonce: witness.nonce.clone(),
        canonical_request_hash: witness.canonical_request_hash,
    })
}

/// Encode a multisig witness payload for use in `X-Iroha-Witness` headers.
///
/// # Errors
/// Returns [`norito::Error`] when witness encoding fails.
pub fn witness_header_value(witness: &CanonicalRequestWitnessV1) -> Result<String, norito::Error> {
    norito::to_bytes(witness).map(|bytes| BASE64_STANDARD.encode(bytes))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn parse_required_header_text(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<String, crate::Error> {
    let value = headers.get(name).ok_or_else(|| {
        crate::Error::Query(ValidationFail::NotPermitted(format!(
            "missing required canonical request header `{name}`"
        )))
    })?;
    let value = std::str::from_utf8(value.as_bytes())
        .map(str::trim)
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(format!(
                "invalid canonical request header `{name}`"
            )))
        })?;
    if value.is_empty() {
        Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
            "invalid canonical request header `{name}`"
        ))))
    } else {
        Ok(value.to_owned())
    }
}

fn parse_account_header_value(
    state: &Arc<CoreState>,
    account_literal: &str,
) -> Result<AccountId, crate::Error> {
    crate::routing::parse_account_literal_with_state(
        state.as_ref(),
        account_literal.trim(),
        &crate::routing::MaybeTelemetry::disabled(),
        ACCOUNT_HEADER_CONTEXT,
    )
    .map(|(account_id, _)| account_id)
    .map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "invalid X-Iroha-Account value".to_owned(),
        ))
    })
}

fn validate_freshness(
    config: &CanonicalRequestAuthConfig,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<(), crate::Error> {
    let delta_ms = now_unix_ms().abs_diff(timestamp_ms);
    let max_skew_ms: u64 = config
        .max_clock_skew
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    if delta_ms > max_skew_ms {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "request timestamp outside allowed skew window".to_owned(),
        )));
    }
    if nonce.len() > 256
        || !nonce.is_ascii()
        || nonce.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "invalid X-Iroha-Nonce value".to_owned(),
        )));
    }
    Ok(())
}

/// Validate an iterable query against the executor on behalf of `authority`.
pub fn validate_iter_query_for_authority<Q>(
    state: &Arc<CoreState>,
    authority: &AccountId,
    query: Q,
) -> Result<(), crate::Error>
where
    Q: Query + 'static,
    Q::Item:
        HasProjection<PredicateMarker> + HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
    Q: norito::codec::Encode,
{
    use iroha_core::smartcontracts::isi::query::{QueryLimits, ValidQueryRequest};

    let payload = norito::codec::Encode::encode(&query);
    let qbox: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Q::Item>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        payload,
    ));
    let iter = QueryWithParams::new(&qbox, QueryParams::default());
    let request = QueryRequest::Start(iter);
    let limits = QueryLimits::new(crate::routing::app_query_limits().max_fetch_size);
    let world = state.world_view();
    let latest_block = state.latest_block_header_fast();
    ValidQueryRequest::validate_for_client_world_parts(
        request,
        authority,
        &world,
        latest_block,
        limits,
    )
    .map(|_| ())
    .map_err(crate::Error::Query)
}

/// Verify optional canonical request headers.
///
/// Returns `Ok(Some(identity))` when a signature is present and valid, `Ok(None)` when
/// no signing headers are provided, and an error when headers are malformed or verification fails.
pub fn verify_canonical_request(
    state: &Arc<CoreState>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
    expected_account: Option<&AccountId>,
) -> Result<Option<VerifiedCanonicalRequest>, crate::Error> {
    let account_hdr = headers.get(HEADER_ACCOUNT);
    let signature_hdr = headers.get(HEADER_SIGNATURE);
    let timestamp_hdr = headers.get(HEADER_TIMESTAMP_MS);
    let nonce_hdr = headers.get(HEADER_NONCE);
    let witness_hdr = headers.get(HEADER_WITNESS);
    let all_missing = account_hdr.is_none()
        && signature_hdr.is_none()
        && timestamp_hdr.is_none()
        && nonce_hdr.is_none()
        && witness_hdr.is_none();
    if all_missing {
        return Ok(None);
    }
    if witness_hdr.is_some() {
        if signature_hdr.is_some() || timestamp_hdr.is_some() || nonce_hdr.is_some() {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "X-Iroha-Witness must not be combined with X-Iroha-Signature, X-Iroha-Timestamp-Ms, or X-Iroha-Nonce".to_owned(),
            )));
        }

        let witness_b64 = parse_required_header_text(headers, HEADER_WITNESS)?;
        let witness_bytes = BASE64_STANDARD.decode(witness_b64.trim()).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid base64 in X-Iroha-Witness".to_owned(),
            ))
        })?;
        let witness: CanonicalRequestWitnessV1 = norito::decode_from_bytes(&witness_bytes)
            .map_err(|_| {
                crate::Error::Query(ValidationFail::NotPermitted(
                    "invalid X-Iroha-Witness payload".to_owned(),
                ))
            })?;
        if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(format!(
                "unsupported X-Iroha-Witness schema_version `{}`",
                witness.schema_version
            ))));
        }
        let account = if let Some(account_hdr) = account_hdr {
            let account_literal = std::str::from_utf8(account_hdr.as_bytes())
                .map(str::trim)
                .map_err(|_| {
                    crate::Error::Query(ValidationFail::NotPermitted(
                        "invalid X-Iroha-Account value".to_owned(),
                    ))
                })?;
            let account = parse_account_header_value(state, account_literal)?;
            if account != witness.subject_account {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "X-Iroha-Account does not match X-Iroha-Witness subject_account".to_owned(),
                )));
            }
            account
        } else {
            witness.subject_account.clone()
        };

        if let Some(expected) = expected_account
            && expected != &account
        {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "signed account does not match request path".to_owned(),
            )));
        }

        let (auth_config, replay_cache) = auth_runtime_snapshot();
        validate_freshness(&auth_config, witness.timestamp_ms, &witness.nonce)?;

        let expected_hash = canonical_request_hash(method, uri, body);
        if witness.canonical_request_hash != expected_hash {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "X-Iroha-Witness canonical request hash mismatch".to_owned(),
            )));
        }
        let message = canonical_request_witness_message(&witness).map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Witness payload".to_owned(),
            ))
        })?;

        let world = state.world_view();
        let account_entry = world.account(&account).map_err(|_| {
            crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
                FindError::Account(account.clone()),
            )))
        })?;
        let verified_signers = match account_entry.id.controller() {
            AccountController::Single(_) => {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "single-signature accounts must use X-Iroha-Signature".to_owned(),
                )));
            }
            AccountController::Multisig(policy) => {
                if witness.signatures.is_empty() {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(
                        "X-Iroha-Witness must include at least one signature".to_owned(),
                    )));
                }

                let member_weights: BTreeMap<PublicKey, u16> = policy
                    .members()
                    .iter()
                    .map(|member| (member.public_key().clone(), member.weight()))
                    .collect();
                let mut seen = BTreeSet::new();
                let mut total_weight = 0_u32;
                let mut verified_signers = Vec::with_capacity(witness.signatures.len());

                for CanonicalRequestSignatureWitnessV1 { signer, signature } in &witness.signatures
                {
                    if !seen.insert(signer.clone()) {
                        return Err(crate::Error::Query(ValidationFail::NotPermitted(
                            "X-Iroha-Witness contains duplicate signer keys".to_owned(),
                        )));
                    }
                    let Some(weight) = member_weights.get(signer) else {
                        return Err(crate::Error::Query(ValidationFail::NotPermitted(
                            "X-Iroha-Witness includes a signer outside the account multisig policy"
                                .to_owned(),
                        )));
                    };
                    signature.verify(signer, &message).map_err(|_| {
                        crate::Error::Query(ValidationFail::NotPermitted(
                            "query signature failed verification".to_owned(),
                        ))
                    })?;
                    total_weight = total_weight.saturating_add(u32::from(*weight));
                    verified_signers.push(signer.clone());
                }
                if total_weight < u32::from(policy.threshold()) {
                    return Err(crate::Error::Query(ValidationFail::NotPermitted(
                        "X-Iroha-Witness signatures do not satisfy multisig threshold".to_owned(),
                    )));
                }
                verified_signers
            }
        };

        let replay_key = format!("{account}:{}", witness.nonce);
        if !replay_cache.check_and_insert(replay_key) {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "request nonce already used".to_owned(),
            )));
        }
        let signer = verified_signers
            .first()
            .cloned()
            .expect("non-empty witness signer set");
        return Ok(Some(VerifiedCanonicalRequest {
            account,
            signer,
            verified_signers,
        }));
    }

    if account_hdr.is_none()
        || signature_hdr.is_none()
        || timestamp_hdr.is_none()
        || nonce_hdr.is_none()
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "X-Iroha-Account, X-Iroha-Signature, X-Iroha-Timestamp-Ms, and X-Iroha-Nonce must be set together".to_owned(),
        )));
    };

    let account_literal = parse_required_header_text(headers, HEADER_ACCOUNT)?;
    let account = parse_account_header_value(state, &account_literal)?;

    if let Some(expected) = expected_account
        && expected != &account
    {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "signed account does not match request path".to_owned(),
        )));
    }

    let timestamp_ms = parse_required_header_text(headers, HEADER_TIMESTAMP_MS)?
        .parse::<u64>()
        .map_err(|_| {
            crate::Error::Query(ValidationFail::NotPermitted(
                "invalid X-Iroha-Timestamp-Ms value".to_owned(),
            ))
        })?;
    let nonce = parse_required_header_text(headers, HEADER_NONCE)?;
    let (auth_config, replay_cache) = auth_runtime_snapshot();
    validate_freshness(&auth_config, timestamp_ms, &nonce)?;

    let signature_b64 = parse_required_header_text(headers, HEADER_SIGNATURE)?;
    let signature_bytes = BASE64_STANDARD.decode(signature_b64.trim()).map_err(|_| {
        crate::Error::Query(ValidationFail::NotPermitted(
            "invalid base64 in X-Iroha-Signature".to_owned(),
        ))
    })?;
    let signature = Signature::from_bytes(&signature_bytes);
    let message = canonical_request_signature_message(method, uri, body, timestamp_ms, &nonce);

    let world = state.world_view();
    let account_entry = world.account(&account).map_err(|_| {
        crate::Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(account.clone()),
        )))
    })?;

    let signer = match account_entry.id.controller() {
        AccountController::Single(pk) => {
            if signature.verify(pk, &message).is_ok() {
                pk.clone()
            } else {
                return Err(crate::Error::Query(ValidationFail::NotPermitted(
                    "query signature failed verification".to_owned(),
                )));
            }
        }
        AccountController::Multisig(_) => {
            return Err(crate::Error::Query(ValidationFail::NotPermitted(
                "multisig accounts must use X-Iroha-Witness".to_owned(),
            )));
        }
    };
    let replay_key = format!("{account}:{nonce}");
    if !replay_cache.check_and_insert(replay_key) {
        return Err(crate::Error::Query(ValidationFail::NotPermitted(
            "request nonce already used".to_owned(),
        )));
    }

    Ok(Some(VerifiedCanonicalRequest {
        account,
        signer: signer.clone(),
        verified_signers: vec![signer],
    }))
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use axum::http::Uri;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute as _,
        state::{State, StateReadOnly, World},
        sumeragi::network_topology::Topology,
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        Registrable,
        account::{Account, MultisigMember, MultisigPolicy},
        domain::Domain,
        isi::Register,
        prelude::DomainId,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;

    use super::*;

    const TEST_ACCOUNT_I105: &str =
        "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE";

    fn minimal_state_with_account(account: &AccountId) -> Arc<State> {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(account);
        let account_value = Account::new(account.clone()).build(account);
        Arc::new(State::new_for_testing(
            World::with([domain], [account_value], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }

    fn bind_account_alias_for_test(state: &Arc<State>, account_id: &AccountId, alias: &str) {
        let label = iroha_data_model::account::rekey::AccountLabel::from_literal(
            alias,
            &state.nexus_snapshot().dataspace_catalog,
        )
        .expect("valid account alias");
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        world
            .account_aliases_mut_for_testing()
            .insert(label.clone(), account_id.clone());
        let mut labels = world
            .account_aliases_by_account_mut_for_testing()
            .get(account_id)
            .cloned()
            .unwrap_or_default();
        labels.insert(label.clone());
        world
            .account_aliases_by_account_mut_for_testing()
            .insert(account_id.clone(), labels);
        world.account_rekey_records_mut_for_testing().insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
        );
        tx.apply();
        block.commit().expect("commit account alias for test");
    }

    #[cfg(test)]
    fn test_guard(config: CanonicalRequestAuthConfig) -> impl Drop {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        struct Guard(std::sync::MutexGuard<'static, ()>);
        impl Drop for Guard {
            fn drop(&mut self) {
                configure(CanonicalRequestAuthConfig::default());
            }
        }
        let guard = TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test lock");
        configure(config);
        Guard(guard)
    }

    #[test]
    fn canonical_query_sorting_is_stable() {
        let raw = "b=2&a=3&b=1&space=a+b";
        let canonical = canonical_query_string(Some(raw));
        assert_eq!(canonical, "a=3&b=1&b=2&space=a+b");
    }

    #[test]
    fn canonical_message_includes_body_hash() {
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=5")
            .parse()
            .expect("uri");
        let msg = canonical_request_message(&Method::GET, &uri, b"{\"foo\":1}");
        let rendered = String::from_utf8(msg).expect("utf8");
        assert!(rendered.contains(&format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets")));
        assert!(rendered.contains("limit=5"));
        assert!(
            rendered.ends_with("37a76343c8e3c695feeaadfe52329673ff129c65f99f55ae6056c9254f4c481d")
        );
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "accept-valid-signature";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                .expect("verify");
        assert_eq!(
            verified,
            Some(VerifiedCanonicalRequest {
                account,
                signer: ALICE_KEYPAIR.public_key().clone(),
                verified_signers: vec![ALICE_KEYPAIR.public_key().clone()],
            })
        );
    }

    #[test]
    fn verify_accepts_alias_account_header_and_returns_canonical_i105_account() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        bind_account_alias_for_test(&state, &account, "wallet@universal");
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=10")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "accept-alias-account-header";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_static("wallet@universal"),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&account))
                .expect("verify");
        assert_eq!(
            verified,
            Some(VerifiedCanonicalRequest {
                account,
                signer: ALICE_KEYPAIR.public_key().clone(),
                verified_signers: vec![ALICE_KEYPAIR.public_key().clone()],
            })
        );
    }

    #[test]
    fn verify_rejects_wrong_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "wrong-signature";
        let bad_sig = Signature::new(KeyPair::random().private_key(), b"forged");
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(bad_sig.payload())).unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signature"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn verify_rejects_mismatched_path_account() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let other: AccountId = AccountId::new(KeyPair::random().public_key().clone());
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "mismatched-path-account";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], Some(&other))
            .unwrap_err();
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("signed account does not match request path"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_missing_freshness_headers() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let message = canonical_request_message(&method, &uri, &[]);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("freshness headers must be required");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("must be set together"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_replayed_nonce() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "replayed-nonce";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect("first request must pass");
        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("replay must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("nonce already used"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_stale_timestamp() {
        let _guard = test_guard(CanonicalRequestAuthConfig {
            max_clock_skew: Duration::from_secs(1),
            nonce_ttl: Duration::from_secs(300),
            replay_cache_capacity: nonzero!(128usize),
        });
        let account = ALICE_ID.clone();
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = 1;
        let nonce = "stale-timestamp";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(ALICE_KEYPAIR.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_static("1"),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("stale request must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("timestamp outside allowed skew"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_multisig_account_signature() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = KeyPair::random();
        let signer_two = KeyPair::random();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::GET;
        let uri: Uri = format!("/v1/accounts/{TEST_ACCOUNT_I105}/assets?limit=1")
            .parse()
            .expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "multisig-http-auth";
        let message = canonical_request_signature_message(&method, &uri, &[], timestamp_ms, nonce);
        let signature = Signature::new(signer_one.private_key(), &message);
        let account_literal = account.canonical_i105().expect("i105 account");
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account_literal).unwrap(),
        );
        headers.insert(
            HEADER_SIGNATURE,
            axum::http::HeaderValue::from_str(&BASE64_STANDARD.encode(signature.payload()))
                .unwrap(),
        );
        headers.insert(
            HEADER_TIMESTAMP_MS,
            axum::http::HeaderValue::from_str(&timestamp_ms.to_string()).unwrap(),
        );
        headers.insert(HEADER_NONCE, axum::http::HeaderValue::from_static(nonce));

        let err = verify_canonical_request(&state, &headers, &method, &uri, &[], None)
            .expect_err("multisig app-auth must fail closed");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("X-Iroha-Witness"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn multisig_witness(
        account: &AccountId,
        method: &Method,
        uri: &Uri,
        body: &[u8],
        timestamp_ms: u64,
        nonce: &str,
        signers: &[&KeyPair],
    ) -> CanonicalRequestWitnessV1 {
        let mut witness = CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: account.clone(),
            timestamp_ms,
            nonce: nonce.to_owned(),
            canonical_request_hash: canonical_request_hash(method, uri, body),
            signatures: Vec::new(),
        };
        let message = canonical_request_witness_message(&witness).expect("witness payload");
        witness.signatures = signers
            .iter()
            .map(|signer| CanonicalRequestSignatureWitnessV1 {
                signer: signer.public_key().clone(),
                signature: Signature::new(signer.private_key(), &message),
            })
            .collect();
        witness
    }

    fn witness_headers(account: &AccountId, witness: &CanonicalRequestWitnessV1) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HEADER_ACCOUNT,
            axum::http::HeaderValue::from_str(&account.canonical_i105().expect("i105 account"))
                .expect("valid account header"),
        );
        headers.insert(
            HEADER_WITNESS,
            axum::http::HeaderValue::from_str(
                &witness_header_value(witness).expect("encode witness header"),
            )
            .expect("valid witness header"),
        );
        headers
    }

    #[test]
    fn verify_accepts_valid_multisig_witness() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = KeyPair::random();
        let signer_two = KeyPair::random();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy?view=full".parse().expect("uri");
        let timestamp_ms = now_unix_ms();
        let nonce = "valid-multisig-witness";
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{\"deploy\":true}",
            timestamp_ms,
            nonce,
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        let verified =
            verify_canonical_request(&state, &headers, &method, &uri, b"{\"deploy\":true}", None)
                .expect("verify")
                .expect("witness auth must be present");
        assert_eq!(verified.account, account);
        assert_eq!(verified.signer, signer_one.public_key().clone());
        assert_eq!(
            verified.verified_signers,
            vec![
                signer_one.public_key().clone(),
                signer_two.public_key().clone()
            ]
        );
    }

    #[test]
    fn verify_rejects_duplicate_multisig_witness_signers() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = KeyPair::random();
        let signer_two = KeyPair::random();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let timestamp_ms = now_unix_ms();
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            timestamp_ms,
            "duplicate-multisig-witness",
            &[&signer_one],
        );
        let mut duplicate = witness.clone();
        duplicate.signatures.push(duplicate.signatures[0].clone());
        let headers = witness_headers(&account, &duplicate);

        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("duplicate witness signers must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("duplicate signer"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_multisig_witness_below_threshold() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = KeyPair::random();
        let signer_two = KeyPair::random();
        let signer_three = KeyPair::random();
        let policy = MultisigPolicy::new(
            3,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_three.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            now_unix_ms(),
            "threshold-multisig-witness",
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("threshold failure must reject witness");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("threshold"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verify_rejects_replayed_multisig_witness_nonce() {
        let _guard = test_guard(CanonicalRequestAuthConfig::default());
        let signer_one = KeyPair::random();
        let signer_two = KeyPair::random();
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let state = minimal_state_with_account(&account);
        let method = Method::POST;
        let uri: Uri = "/v1/soracloud/deploy".parse().expect("uri");
        let witness = multisig_witness(
            &account,
            &method,
            &uri,
            b"{}",
            now_unix_ms(),
            "replayed-multisig-witness",
            &[&signer_one, &signer_two],
        );
        let headers = witness_headers(&account, &witness);

        verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect("first multisig witness must pass");
        let err = verify_canonical_request(&state, &headers, &method, &uri, b"{}", None)
            .expect_err("replayed multisig witness must fail");
        match err {
            crate::Error::Query(ValidationFail::NotPermitted(msg)) => {
                assert!(msg.contains("nonce already used"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
