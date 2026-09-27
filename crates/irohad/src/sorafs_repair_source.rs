//! Production remote repair reader authenticated by current native leases and signed adverts.

use crate::SharedSoraFsProviderCache;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_config::parameters::actual::SorafsRepairSource;
use iroha_core::{query::repair_source::authorize_repair_source_v1, state::State};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, Signature};
use iroha_data_model::{account::AccountId, sorafs::repair_source::RepairSourceRequestV1};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use reqwest::Url;
use sorafs_manifest::EndpointKind;
use sorafs_node::{
    RepairChunkPayload, RepairOrchestrator, RepairOrchestratorError,
    native_repair_worker::NativeRepairExecutionContextV1,
    store::{ChunkFileRecord, StoredManifest},
};
use std::{
    collections::BTreeMap,
    io::Read as _,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Configured source service. Private credentials and endpoints are never formatted.
pub struct NativeRepairSourceV1 {
    state: Arc<State>,
    cache: SharedSoraFsProviderCache,
    authority: AccountId,
    key: KeyPair,
    origins: BTreeMap<[u8; 32], Url>,
    timeout: Duration,
}
impl std::fmt::Debug for NativeRepairSourceV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeRepairSourceV1 { private runtime }")
    }
}
fn rejected() -> RepairOrchestratorError {
    RepairOrchestratorError::other("authenticated repair source rejected")
}
fn now_ms() -> Result<u64, RepairOrchestratorError> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| rejected())?
            .as_millis(),
    )
    .map_err(|_| rejected())
}
fn origin(text: &str) -> Result<Url, RepairOrchestratorError> {
    let url = Url::parse(text).map_err(|_| rejected())?;
    let numeric_loopback = url
        .host_str()
        .and_then(|host| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .is_some_and(|address| address.is_loopback());
    if !(url.scheme() == "https" || url.scheme() == "http" && numeric_loopback)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port() == Some(0)
        || text.len() > 2048
    {
        return Err(rejected());
    }
    Ok(url)
}

fn distinct_chunks<'a>(
    chunks: &[&'a ChunkFileRecord],
) -> Result<BTreeMap<[u8; 32], &'a ChunkFileRecord>, RepairOrchestratorError> {
    let mut unique: BTreeMap<[u8; 32], &'a ChunkFileRecord> = BTreeMap::new();
    for chunk in chunks {
        if unique
            .insert(chunk.digest, *chunk)
            .is_some_and(|previous| previous.length != chunk.length)
        {
            return Err(rejected());
        }
    }
    Ok(unique)
}
#[cfg(unix)]
fn load_key(
    path: &std::path::Path,
    authority: &AccountId,
) -> Result<KeyPair, RepairOrchestratorError> {
    let bytes =
        crate::runtime_credential::load_bounded_runtime_credential_v1(path, 2, 16 * 1024 + 256)
            .map_err(|_| rejected())?;
    let text = std::str::from_utf8(bytes.strip_suffix(b"\n").ok_or_else(rejected)?)
        .map_err(|_| rejected())?;
    let private: ExposedPrivateKey = text.parse().map_err(|_| rejected())?;
    let canonical =
        zeroize::Zeroizing::new(private.try_to_multihash_string().map_err(|_| rejected())?);
    if canonical.as_str() != text {
        return Err(rejected());
    }
    let key = KeyPair::from_private_key(private.0).map_err(|_| rejected())?;
    if !matches!(
        key.public_key().algorithm(),
        Algorithm::Ed25519 | Algorithm::MlDsa
    ) || authority.try_signatory() != Some(key.public_key())
    {
        return Err(rejected());
    }
    Ok(key)
}

impl NativeRepairSourceV1 {
    /// Load one configured software reader after native State and the live discovery cache exist.
    ///
    /// # Errors
    /// Rejects invalid origins, malformed provider identities and unsafe or mismatched credentials.
    #[cfg(unix)]
    pub fn new(
        config: &SorafsRepairSource,
        state: Arc<State>,
        cache: SharedSoraFsProviderCache,
    ) -> Result<Self, RepairOrchestratorError> {
        if !(100..=120_000).contains(&config.timeout_ms)
            || config.origins.is_empty()
            || config.origins.len() > 16
        {
            return Err(rejected());
        }
        let mut origins = BTreeMap::new();
        for (provider, endpoint) in &config.origins {
            let mut id = [0; 32];
            hex::decode_to_slice(provider, &mut id).map_err(|_| rejected())?;
            if id == [0; 32]
                || hex::encode(id) != *provider
                || origins.insert(id, origin(endpoint)?).is_some()
            {
                return Err(rejected());
            }
        }
        let key = load_key(&config.credential, &config.authority)?;
        Ok(Self {
            state,
            cache,
            authority: config.authority.clone(),
            key,
            origins,
            timeout: Duration::from_millis(config.timeout_ms),
        })
    }
    #[cfg(not(unix))]
    /// Reject platforms without the descriptor-authenticated credential reader.
    pub fn new(
        _: &SorafsRepairSource,
        _: Arc<State>,
        _: SharedSoraFsProviderCache,
    ) -> Result<Self, RepairOrchestratorError> {
        Err(rejected())
    }

    fn authorize(
        &self,
        request: &RepairSourceRequestV1,
        endpoint: &Url,
        deadline: Instant,
    ) -> Result<(), RepairOrchestratorError> {
        if Instant::now() >= deadline {
            return Err(rejected());
        }
        let now = now_ms()?;
        authorize_repair_source_v1(&self.state.view(), &self.authority, request, now)
            .map_err(|_| rejected())?;
        let mut cache = self.cache.try_write().map_err(|_| rejected())?;
        cache.prune_stale(now / 1000);
        let record = cache
            .record_by_provider(&request.source_provider)
            .ok_or_else(rejected)?;
        let advert = record.advert();
        if !advert.body.endpoints.iter().any(|advert| {
            advert.kind == EndpointKind::Torii
                && endpoint.host_str() == Some(advert.host_pattern.as_str())
        }) {
            return Err(rejected());
        }
        Ok(())
    }

    fn fetch(
        &self,
        client: &reqwest::blocking::Client,
        request: &RepairSourceRequestV1,
        base: &Url,
        deadline: Instant,
    ) -> Result<RepairChunkPayload, RepairOrchestratorError> {
        self.authorize(request, base, deadline)?;
        let url = base
            .join("/v1/sorafs/repair/source")
            .map_err(|_| rejected())?;
        let body = norito::encode_canonical(request).map_err(|_| rejected())?;
        let timestamp = now_ms()?;
        let mut entropy = [0; 32];
        OsRng.try_fill_bytes(&mut entropy).map_err(|_| rejected())?;
        let nonce = hex::encode(entropy);
        let uri = url.path().parse().map_err(|_| rejected())?;
        let message = iroha_torii::canonical_network_request_signature_message(
            self.state.network_id_ref(),
            &iroha_torii::Method::POST,
            &uri,
            &body,
            timestamp,
            &nonce,
        )
        .map_err(|_| rejected())?;
        let signature =
            Signature::try_new(self.key.private_key(), &message).map_err(|_| rejected())?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(rejected)?;
        let mut response = client
            .post(url)
            .timeout(remaining)
            .header("Content-Type", "application/x-norito")
            .header("Accept-Encoding", "identity")
            .header(
                "X-Iroha-Account",
                self.authority.to_canonical_hex().map_err(|_| rejected())?,
            )
            .header("X-Iroha-Signature", STANDARD.encode(signature.payload()))
            .header("X-Iroha-Timestamp-Ms", timestamp.to_string())
            .header("X-Iroha-Nonce", nonce)
            .body(body)
            .send()
            .map_err(|_| rejected())?;
        if !response.status().is_success()
            || response
                .headers()
                .get("Content-Encoding")
                .is_some_and(|value| value != "identity")
            || response
                .content_length()
                .is_some_and(|length| length != u64::from(request.chunk_length))
        {
            return Err(rejected());
        }
        let mut bytes = Vec::with_capacity(request.chunk_length as usize);
        (&mut response)
            .take(u64::from(request.chunk_length) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| rejected())?;
        if bytes.len() != request.chunk_length as usize
            || blake3::hash(&bytes).as_bytes() != &request.chunk_digest
        {
            return Err(rejected());
        }
        self.authorize(request, base, deadline)?;
        Ok(RepairChunkPayload {
            digest: request.chunk_digest,
            bytes,
            source: None,
        })
    }
}
impl RepairOrchestrator for NativeRepairSourceV1 {
    fn rehydrate_missing_chunks(
        &self,
        context: &NativeRepairExecutionContextV1,
        _manifest: &StoredManifest,
        chunks: &[&ChunkFileRecord],
        sink: &mut dyn FnMut(RepairChunkPayload) -> Result<(), RepairOrchestratorError>,
    ) -> Result<(), RepairOrchestratorError> {
        if context.network_id != *self.state.network_id_ref()
            || context.lease_owner_account != self.authority.to_string()
        {
            return Err(rejected());
        }
        let deadline = Instant::now()
            .checked_add(self.timeout)
            .ok_or_else(rejected)?;
        // This trait is called by the blocking native worker; no async runtime is constructed or blocked.
        let client = reqwest::blocking::Client::builder()
            // `origin` permits plaintext only for explicitly configured numeric loopback IPs.
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(self.timeout)
            .build()
            .map_err(|_| rejected())?;
        // A manifest may repeat the same content at multiple offsets. The worker's sink
        // repairs every occurrence together and requires exactly one response per digest.
        for chunk in distinct_chunks(chunks)?.into_values() {
            let mut fetched = None;
            for (source, endpoint) in &self.origins {
                if *source == context.provider_id {
                    continue;
                }
                let request = RepairSourceRequestV1 {
                    floor: context.finalized_cursor,
                    task_id: context.task_id,
                    ticket_id: context.ticket_id.clone(),
                    task_revision: context.task_revision,
                    lease_generation: context.lease_generation,
                    target_provider: context.provider_id,
                    source_provider: *source,
                    manifest_digest: context.manifest_digest,
                    chunk_digest: chunk.digest,
                    chunk_length: chunk.length,
                };
                match self.fetch(&client, &request, endpoint, deadline) {
                    Ok(payload) => {
                        fetched = Some(payload);
                        break;
                    }
                    Err(_) if Instant::now() < deadline => {}
                    Err(error) => return Err(error),
                }
            }
            sink(fetched.ok_or_else(rejected)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn software_repair_credentials_match_the_exact_http_authority() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let directory = tempfile::Builder::new()
            .prefix(".sorafs-repair-credential-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("credential");
        for algorithm in [Algorithm::Ed25519, Algorithm::MlDsa] {
            let key = KeyPair::try_from_seed(vec![0x61; 32], algorithm).unwrap();
            let authority = AccountId::new(key.public_key().clone());
            let encoded = zeroize::Zeroizing::new(format!(
                "{}\n",
                ExposedPrivateKey(key.private_key().clone())
                    .try_to_multihash_string()
                    .unwrap()
            ));
            fs::write(&path, encoded.as_bytes()).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(
                load_key(&path, &authority).unwrap().public_key(),
                key.public_key()
            );
            let other = KeyPair::try_from_seed(vec![0x62; 32], algorithm).unwrap();
            assert!(load_key(&path, &AccountId::new(other.public_key().clone())).is_err());
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(load_key(&path, &authority).is_err());
        }
    }
    #[test]
    fn repeated_content_is_requested_once_and_conflicting_lengths_are_rejected() {
        let first = ChunkFileRecord {
            path: "first.chunk".into(),
            offset: 0,
            length: 1024,
            digest: [1; 32],
            role: None,
            group_id: None,
        };
        let mut repeated = first.clone();
        repeated.path = "second.chunk".into();
        repeated.offset = 1024;
        assert_eq!(distinct_chunks(&[&first, &repeated]).unwrap().len(), 1);
        repeated.length += 1;
        assert!(distinct_chunks(&[&first, &repeated]).is_err());
    }
    #[test]
    fn source_origins_require_tls_or_numeric_loopback_without_credentials_or_path() {
        assert!(origin("https://provider.example").is_ok());
        assert!(origin("http://127.0.0.1:8080").is_ok());
        assert!(origin("http://[::1]:8080").is_ok());
        for invalid in [
            "http://provider.example",
            "http://localhost:8080",
            "http://127.0.0.1.provider.example",
            "http://192.168.1.1",
            "https://user@provider.example",
            "https://provider.example/data",
            "https://provider.example/?token=secret",
            "https://provider.example/#fragment",
        ] {
            assert!(origin(invalid).is_err());
        }
    }
}
