//! Lazy account read adapter over the canonical authenticated archive transport.
//!
//! The environment owns fresh finality and the qualified native schema. Only its portable
//! verified provider result can select an origin or token key. Every token mint reobserves
//! that authority through the same callback; planning retains no stale token verification key.
//! Parent listener tokens, basic
//! authentication, and unrelated default headers are never forwarded to provider origins.
use super::*;
use iroha_data_model::{
    account::AccountId,
    sorafs::provider_admission::discovery::account_read::VerifiedAccountReadProviderV1,
};

/// Closed failure classes from the environment's independently bound provider discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiArchiveDiscoveryErrorV1 {
    /// Current authenticated state or its approved endpoint is temporarily unavailable.
    Unavailable,
    /// Returned state or policy failed exact native authentication.
    Rejected,
    /// The caller's finite overall deadline expired.
    Deadline,
}
/// Caller-owned fresh finality and native provider discovery, evaluated only on a cache miss.
pub type MusubiArchiveProviderDiscoveryV1 = dyn Fn(ProviderId) -> Result<VerifiedAccountReadProviderV1, MusubiArchiveDiscoveryErrorV1>
    + Send
    + Sync;

#[derive(Clone)]
pub(super) struct AccountRegistryV1 {
    pub(super) config: iroha::config::Config,
    pub(super) discovery: Arc<MusubiArchiveProviderDiscoveryV1>,
}
#[derive(Clone)]
pub(super) enum ProviderCredentialV1 {
    Operator(KeyPair),
    Account {
        account: AccountId,
        key_pair: KeyPair,
        chain_id: String,
        discovery: Arc<MusubiArchiveProviderDiscoveryV1>,
    },
}

impl PreparedMusubiArchiveFetchConfigV1 {
    /// Bind account reads to a separately resolved registry signer and authenticated discovery.
    ///
    /// This opens no keys, resolves no DNS and sends no requests. The callback must use an
    /// independently installed network/schema and freshly authenticated finality decision.
    /// A private child must explicitly pass its parent's registry context; none is inferred.
    /// # Errors
    /// Rejects zero/overlong timeout or a signer that is not this single-key account controller.
    pub fn from_account_registry(
        config: iroha::config::Config,
        discovery: Arc<MusubiArchiveProviderDiscoveryV1>,
        request_timeout: Duration,
    ) -> Result<Self, MusubiArchiveRuntimeErrorV1> {
        if request_timeout.is_zero()
            || request_timeout > Duration::from_millis(MAX_REQUEST_TIMEOUT_MS)
            || config.account != AccountId::new(config.key_pair.public_key().clone())
        {
            return Err(permanent("MUSUBI_ARCHIVE_REGISTRY_ACCOUNT_INVALID"));
        }
        Ok(Self {
            providers: Vec::new(),
            network_id: config.network_id,
            client_id: "musubi-v1".into(),
            request_timeout,
            account_registry: Some(Arc::new(AccountRegistryV1 { config, discovery })),
        })
    }
}

impl AuthenticatedMusubiArchiveFetchClientV1 {
    pub(super) fn discover_account_provider(
        &mut self,
        provider: ProviderId,
    ) -> Result<(), MusubiArchiveRuntimeErrorV1> {
        let Some(registry) = &self.account_registry else {
            return Ok(());
        };
        let authority = discover_authority(registry.discovery.as_ref(), provider)?;
        if authority.discovery().network_id() != self.network_id
            || authority.discovery().advert().body.provider_id != *provider.as_bytes()
            || authority.chain_id() != registry.config.chain.to_string()
        {
            return Err(control_integrity(
                "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_SCOPE_MISMATCH",
            ));
        }
        let origin = authority
            .policy()
            .https_origin()
            .map_err(|_| control_integrity("MUSUBI_ARCHIVE_PROVIDER_ORIGIN_INVALID"))?;
        let base_url = parse_gateway_base_url(&format!("{origin}/"))?;
        let http = pinned_http_client(&base_url, self.request_timeout)?;
        if self.providers.len() >= MAX_CONFIGURED_PROVIDERS
            && !self.providers.contains_key(&provider)
        {
            self.providers.pop_first();
        }
        self.providers.insert(
            provider,
            ProviderRuntimeV1 {
                provider,
                base_url,
                http,
                credential: ProviderCredentialV1::Account {
                    account: registry.config.account.clone(),
                    key_pair: registry.config.key_pair.clone(),
                    chain_id: authority.chain_id().to_owned(),
                    discovery: registry.discovery.clone(),
                },
            },
        );
        Ok(())
    }
}

fn discover_authority(
    discovery: &MusubiArchiveProviderDiscoveryV1,
    provider: ProviderId,
) -> Result<VerifiedAccountReadProviderV1, MusubiArchiveRuntimeErrorV1> {
    discovery(provider).map_err(|e| match e {
        MusubiArchiveDiscoveryErrorV1::Unavailable => {
            unavailable("MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_UNAVAILABLE")
        }
        MusubiArchiveDiscoveryErrorV1::Rejected => {
            control_integrity("MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_INVALID")
        }
        MusubiArchiveDiscoveryErrorV1::Deadline => {
            retryable("MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_DEADLINE")
        }
    })
}

pub(super) fn refresh_account_authority(
    runtime: &ProviderRuntimeV1,
    network: NetworkId,
) -> Result<Option<VerifiedAccountReadProviderV1>, MusubiArchiveRuntimeErrorV1> {
    let ProviderCredentialV1::Account {
        chain_id,
        discovery,
        ..
    } = &runtime.credential
    else {
        return Ok(None);
    };
    // Every mint, including a replacement session, needs the same independently bound fresh
    // finality callback. A cached key is never an alternative after revocation or rotation.
    let authority = discover_authority(discovery.as_ref(), runtime.provider)?;
    let origin = authority
        .policy()
        .https_origin()
        .map_err(|_| control_integrity("MUSUBI_ARCHIVE_PROVIDER_ORIGIN_INVALID"))?;
    let current_url = parse_gateway_base_url(&format!("{origin}/"))?;
    if authority.discovery().network_id() != network
        || authority.discovery().advert().body.provider_id != *runtime.provider.as_bytes()
        || authority.chain_id() != chain_id
        || gateway_origin(&current_url) != gateway_origin(&runtime.base_url)
    {
        return Err(control_integrity(
            "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_SCOPE_MISMATCH",
        ));
    }
    Ok(Some(authority))
}

pub(super) fn verify_current_account_token(
    token: &StreamTokenV1,
    header_key_hex: &str,
    provider: ProviderId,
    key: &PublicKey,
    key_revision: u32,
    policy: &sorafs_manifest::provider_advert::account_read::RegisteredAccountReadV1,
) -> Result<(), MusubiArchiveRuntimeErrorV1> {
    let failed = || control_integrity("MUSUBI_ARCHIVE_TOKEN_AUTHORITY_MISMATCH");
    let (algorithm, bytes) = key.to_bytes();
    if algorithm != iroha_crypto::Algorithm::Ed25519
        || header_key_hex != hex::encode(bytes)
        || token.body.token_pk_version != key_revision
        || token.body.provider_id != *provider.as_bytes()
        || token.body.max_streams > policy.max_streams
        || token.body.rate_limit_bytes > policy.rate_limit_bytes
        || token.body.requests_per_minute > policy.requests_per_minute
        || token
            .body
            .ttl_epoch
            .checked_sub(token.body.issued_at)
            .is_none_or(|ttl| ttl > policy.ttl_secs)
    {
        return Err(failed());
    }
    token.verify_public_key(key).map_err(|_| failed())
}

pub(super) fn account_request_headers(
    account: &AccountId,
    key_pair: &KeyPair,
    network: &NetworkId,
    url: &Url,
    body: &[u8],
) -> Result<HeaderMap, MusubiArchiveRuntimeErrorV1> {
    let failed = || permanent("MUSUBI_ARCHIVE_ACCOUNT_SIGNING_FAILED");
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| failed())?
        .as_millis()
        .try_into()
        .map_err(|_| failed())?;
    let nonce = random_nonce()?;
    let payload = iroha::client::canonical_network_request_signature_message(
        network,
        &iroha::http::Method::POST,
        url,
        body,
        timestamp_ms,
        &nonce,
    )
    .map_err(|_| failed())?;
    let signature = Signature::try_new(key_pair.private_key(), &payload).map_err(|_| failed())?;
    let values = [
        (
            "x-iroha-account",
            iroha::client::canonical_request_account_header_value(account).map_err(|_| failed())?,
        ),
        (
            "x-iroha-timestamp-ms",
            iroha::client::canonical_request_timestamp_header_value(timestamp_ms)
                .map_err(|_| failed())?,
        ),
        ("x-iroha-nonce", nonce),
        (
            "x-iroha-signature",
            iroha::client::canonical_request_signature_header_value(&signature)
                .map_err(|_| failed())?,
        ),
    ];
    let mut headers = HeaderMap::new();
    for (name, value) in values {
        headers.insert(name, value.parse().map_err(|_| failed())?);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoked_current_custody_prevents_initial_and_replacement_mint_before_http() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let selected = config();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        // Planning retains only the authenticated origin/scope. No old signer key or token
        // policy is kept in these credentials as an alternative to a current callback.
        let runtime = ProviderRuntimeV1 {
            provider: ProviderId::new([1; 32]),
            base_url: Url::parse("https://storage.example.com/").unwrap(),
            http: HttpClient::builder()
                .https_only(true)
                .timeout(Duration::from_millis(1))
                .build()
                .unwrap(),
            credential: ProviderCredentialV1::Account {
                account: selected.account,
                key_pair: selected.key_pair,
                chain_id: selected.chain.to_string(),
                discovery: Arc::new(move |_| {
                    counted.fetch_add(1, Ordering::SeqCst);
                    Err(MusubiArchiveDiscoveryErrorV1::Rejected)
                }),
            },
        };
        for _ in 0..2 {
            assert_eq!(
                mint_stream_token(
                    &runtime,
                    &selected.network_id,
                    &ManifestDigest::new([1; 32]),
                    "test",
                    1
                )
                .err()
                .expect("current custody must refuse the mint")
                .code(),
                "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_INVALID"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_response_signed_by_the_retired_key_cannot_refresh_a_rotated_session() {
        let old = KeyPair::try_from_seed(vec![0x81; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        let current =
            KeyPair::try_from_seed(vec![0x82; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        let provider = ProviderId::new([1; 32]);
        let policy = sorafs_manifest::provider_advert::account_read::RegisteredAccountReadV1 {
            https_host: "storage.example.com".into(),
            https_port: 443,
            ttl_secs: 60,
            max_streams: 1,
            rate_limit_bytes: 1024,
            requests_per_minute: 10,
        };
        let mut token = StreamTokenV1 {
            body: sorafs_manifest::StreamTokenBodyV1 {
                token_id: "01J3E4ZCMQ3GP2H3R5PSNF6Z7X".into(),
                manifest_cid: vec![1, 0x55, 1],
                provider_id: *provider.as_bytes(),
                profile_handle: "sorafs.sf1@1.0.0".into(),
                max_streams: 1,
                ttl_epoch: 61,
                rate_limit_bytes: 1024,
                issued_at: 1,
                requests_per_minute: 10,
                token_pk_version: 1,
            },
            signature: vec![],
        };
        let sign = |token: &mut StreamTokenV1, key: &KeyPair| {
            token.signature = Signature::new(
                key.private_key(),
                &token.body.signing_payload_bytes().unwrap(),
            )
            .payload()
            .to_vec();
        };
        let old_header = hex::encode(old.public_key().to_bytes().1);
        let current_header = hex::encode(current.public_key().to_bytes().1);
        sign(&mut token, &old);
        verify_current_account_token(&token, &old_header, provider, old.public_key(), 1, &policy)
            .unwrap();
        assert!(
            verify_current_account_token(
                &token,
                &old_header,
                provider,
                current.public_key(),
                2,
                &policy
            )
            .is_err()
        );
        token.body.token_pk_version = 2;
        sign(&mut token, &old);
        assert!(
            verify_current_account_token(
                &token,
                &current_header,
                provider,
                current.public_key(),
                2,
                &policy
            )
            .is_err()
        );
        sign(&mut token, &current);
        verify_current_account_token(
            &token,
            &current_header,
            provider,
            current.public_key(),
            2,
            &policy,
        )
        .unwrap();
        let mut narrower = policy;
        narrower.rate_limit_bytes = 512;
        assert!(
            verify_current_account_token(
                &token,
                &current_header,
                provider,
                current.public_key(),
                2,
                &narrower
            )
            .is_err()
        );
    }

    fn config() -> iroha::config::Config {
        let key_pair =
            KeyPair::try_from_seed(vec![0x77; 32], iroha_crypto::Algorithm::Ed25519).unwrap();
        iroha::config::Config {
            chain: "build-registry".parse().unwrap(),
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"registry genesis"),
            )),
            account: AccountId::new(key_pair.public_key().clone()),
            account_chain_discriminant: 753,
            key_pair,
            basic_auth: None,
            api_token: None,
            torii_api_url: Url::parse("https://registry.example.com/").unwrap(),
            torii_request_timeout: Duration::from_secs(1),
            transaction_ttl: Duration::from_secs(1),
            transaction_status_timeout: Duration::from_secs(1),
            transaction_add_nonce: false,
            sorafs_alias_cache: sorafs_manifest::alias_cache::AliasCachePolicy::new(
                Duration::from_secs(60),
                Duration::from_secs(10),
                Duration::from_secs(120),
                Duration::from_secs(10),
                Duration::from_secs(10),
                Duration::from_secs(60),
                Duration::from_secs(10),
                Duration::from_secs(10),
            ),
            sorafs_anonymity_policy: Default::default(),
            sorafs_rollout_phase: Default::default(),
        }
    }
    #[test]
    fn preparing_and_materializing_account_context_does_not_discover_or_resolve_dns() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = calls.clone();
        let prepared = PreparedMusubiArchiveFetchConfigV1::from_account_registry(
            config(),
            Arc::new(move |_| {
                captured.fetch_add(1, Ordering::SeqCst);
                Err(MusubiArchiveDiscoveryErrorV1::Unavailable)
            }),
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(prepared.network_id(), config().network_id);
        let mut client = prepared.build_client().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            client
                .discover_account_provider(ProviderId::new([1; 32]))
                .unwrap_err()
                .code(),
            "MUSUBI_ARCHIVE_PROVIDER_DISCOVERY_UNAVAILABLE"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(client.providers.is_empty());
        let mut foreign = config();
        foreign.account = AccountId::new(KeyPair::try_random().unwrap().public_key().clone());
        assert!(
            PreparedMusubiArchiveFetchConfigV1::from_account_registry(
                foreign,
                Arc::new(|_| Err(MusubiArchiveDiscoveryErrorV1::Rejected)),
                Duration::from_secs(1)
            )
            .is_err()
        );
    }
    #[test]
    fn account_headers_bind_exact_network_path_body_and_freshness_without_other_credentials() {
        let config = config();
        let url =
            Url::parse("https://storage.example.com/v1/sorafs/storage/token/account").unwrap();
        let body = br#"{"provider_id_hex":"11"}"#;
        let headers = account_request_headers(
            &config.account,
            &config.key_pair,
            &config.network_id,
            &url,
            body,
        )
        .unwrap();
        assert_eq!(headers.len(), 4);
        assert!(!headers.contains_key("x-api-token"));
        assert!(!headers.contains_key("authorization"));
        assert!(!headers.contains_key(OPERATOR_PUBLIC_KEY_HEADER));
        let timestamp = headers["x-iroha-timestamp-ms"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let nonce = headers["x-iroha-nonce"].to_str().unwrap();
        let sig = Signature::from_bytes(
            &STANDARD
                .decode(headers["x-iroha-signature"].as_bytes())
                .unwrap(),
        );
        let message = iroha::client::canonical_network_request_signature_message(
            &config.network_id,
            &iroha::http::Method::POST,
            &url,
            body,
            timestamp,
            nonce,
        )
        .unwrap();
        sig.verify(config.key_pair.public_key(), &message).unwrap();
        let changed = iroha::client::canonical_network_request_signature_message(
            &config.network_id,
            &iroha::http::Method::POST,
            &url,
            b"changed",
            timestamp,
            nonce,
        )
        .unwrap();
        assert!(sig.verify(config.key_pair.public_key(), &changed).is_err());
        let fresh = account_request_headers(
            &config.account,
            &config.key_pair,
            &config.network_id,
            &url,
            body,
        )
        .unwrap();
        assert_ne!(headers["x-iroha-nonce"], fresh["x-iroha-nonce"]);
    }
}
