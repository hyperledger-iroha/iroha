//! Exact original provider advertisements with deterministic bounded refresh intervals.

use super::*;
use sorafs_manifest::provider_advert::{
    AdvertSignature, MAX_ADVERT_TTL_SECS, PROVIDER_ADVERT_VERSION_V1, ProviderAdvertV1,
    SignatureAlgorithm,
};

// Native advert refresh guidance is half its maximum TTL. Deriving slots from the original
// admission makes retries reproduce the same signed bytes without another mutable journal.
const REFRESH_SECONDS: u64 = MAX_ADVERT_TTL_SECS / 2;

impl PreparedLocalnet {
    /// Sign the current finite refresh slot for one exact original generated provider.
    ///
    /// The original signed profile selects the provider key, body, network and expiry ceiling.
    /// This produces transport material only; native current admission and custody verification
    /// remain required before the provider can serve account reads.
    /// # Errors
    /// Refuses foreign providers, expired original admission, unsafe or changed key custody,
    /// malformed original material, and a profile that changes during signing.
    pub fn provider_advert(
        &self,
        provider: ProviderId,
        now_seconds: u64,
    ) -> crate::managed::Result<ProviderAdvertV1> {
        let invalid = || Error::Invalid("invalid original generated provider advert".into());
        let manifest = self.stream_token_authorities()?.ok_or_else(invalid)?;
        let plan = decode(&manifest.provider(provider)?.provider_plan).map_err(|_| invalid())?;
        let original = &plan.material;
        if now_seconds < original.issued_at || now_seconds >= original.retention_epoch {
            return Err(invalid());
        }
        let slot = (now_seconds - original.issued_at) / REFRESH_SECONDS;
        let issued_at = original.issued_at + slot * REFRESH_SECONDS;
        let expires_at = issued_at
            .checked_add(MAX_ADVERT_TTL_SECS)
            .ok_or_else(invalid)?
            .min(original.retention_epoch);
        let root = self.context.client_config.parent().ok_or_else(invalid)?;
        let generation = iroha_fs::PrivateDirectory::open_exact(root)?;
        let directory = generation
            .open_child(LOCALNET_RUNTIME_DIRECTORY)?
            .open_child(DIRECTORY)?;
        let directory = open_provider_directory(&directory, manifest.provider(provider)?.slot)?;
        let key = read_service_private_key(&directory, ADVERT_KEY)?;
        if public32(key.public_key()).map_err(|_| invalid())? != original.proposal.advert_key {
            return Err(invalid());
        }
        let mut advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            network_id: *manifest.network_id.as_bytes(),
            issued_at,
            expires_at,
            body: original.advert_body.clone(),
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: original.proposal.advert_key.to_vec(),
                signature: vec![0; 64],
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        };
        advert.signature.signature = iroha_crypto::Signature::try_new(
            key.private_key(),
            &advert.signature_payload_bytes().map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?
        .payload()
        .to_vec();
        advert
            .validate_with_body(now_seconds)
            .map_err(|_| invalid())?;
        advert.verify_signature().map_err(|_| invalid())?;
        encode(&advert).map_err(|_| invalid())?;
        directory.revalidate()?;
        if self.stream_token_authorities()?.as_ref() != Some(&manifest) {
            return Err(invalid());
        }
        Ok(advert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_fs::PublishMode;

    fn prepared(root: &Path) -> PreparedLocalnet {
        let ports = crate::managed::LocalnetPorts::reserve().unwrap();
        prepare_localnet_at(
            "provider-advert",
            root,
            &ports,
            LocalnetServiceProfile::StreamTokenAuthorities,
            None,
        )
        .unwrap()
    }

    #[test]
    fn generated_advert_replays_exact_slots_without_renewing_original_admission() {
        let _resources = crate::managed::native_test_guard();
        let temporary = tempfile::tempdir().unwrap();
        let prepared = prepared(&temporary.path().join("generation"));
        let plan = prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .unwrap()
            .unwrap();
        let provider = plan.provider_id();
        let material = plan.admission_material();
        let start = material.issued_at;
        let first = prepared.provider_advert(provider, start).unwrap();
        first.verify_signature().unwrap();
        assert_eq!(first.network_id, *plan.network_id().as_bytes());
        assert_eq!(first.body, material.advert_body);
        assert_eq!(first.signature.public_key, material.proposal.advert_key);
        assert_eq!(first.expires_at, start + MAX_ADVERT_TTL_SECS);
        assert_eq!(
            encode(&first).unwrap(),
            encode(
                &prepared
                    .provider_advert(provider, start + REFRESH_SECONDS - 1)
                    .unwrap()
            )
            .unwrap()
        );
        let refreshed = prepared
            .provider_advert(provider, start + REFRESH_SECONDS)
            .unwrap();
        assert_eq!(refreshed.issued_at, start + REFRESH_SECONDS);
        assert_eq!(refreshed.body, first.body);
        assert_ne!(encode(&refreshed).unwrap(), encode(&first).unwrap());
        let last = prepared
            .provider_advert(provider, material.retention_epoch - 1)
            .unwrap();
        assert_eq!(last.expires_at, material.retention_epoch);
        assert!(last.ttl() <= MAX_ADVERT_TTL_SECS);
        assert!(prepared.provider_advert(provider, start - 1).is_err());
        assert!(
            prepared
                .provider_advert(provider, material.retention_epoch)
                .is_err()
        );
        assert!(prepared.provider_advert(provider, u64::MAX).is_err());
        assert_eq!(
            prepared
                .provider_service_plans()
                .map(|plans| plans.map(|[first, _, _]| first))
                .unwrap()
                .unwrap()
                .admission_material(),
            material
        );
    }

    #[test]
    fn generated_advert_rejects_cross_purpose_private_key_and_standard_profile() {
        let _resources = crate::managed::native_test_guard();
        let temporary = tempfile::tempdir().unwrap();
        let prepared = prepared(&temporary.path().join("generation"));
        let original = prepared.stream_token_authorities().unwrap().unwrap();
        let provider = original.providers[0].provider_id;
        let issued_at = prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .unwrap()
            .unwrap()
            .admission_material()
            .issued_at;
        let directory = iroha_fs::PrivateDirectory::open_exact(
            prepared
                .context
                .client_config
                .parent()
                .unwrap()
                .join(LOCALNET_RUNTIME_DIRECTORY)
                .join(DIRECTORY),
        )
        .unwrap();
        let network = directory.open_child(NETWORK_DIRECTORY).unwrap();
        let directory = open_provider_directory(&directory, 0).unwrap();
        let key = directory
            .read(ADVERT_KEY, MAX_ROLE_CREDENTIAL_BYTES)
            .unwrap();
        let other = network
            .read(network_material::COUNCIL_KEYS[0], MAX_ROLE_CREDENTIAL_BYTES)
            .unwrap();
        directory
            .write_atomic(ADVERT_KEY, &other, PublishMode::Replace)
            .unwrap();
        assert!(prepared.provider_advert(provider, issued_at).is_err());
        directory
            .write_atomic(ADVERT_KEY, &key, PublishMode::Replace)
            .unwrap();
        assert!(prepared.provider_advert(provider, issued_at).is_ok());
        assert_eq!(
            prepared.stream_token_authorities().unwrap().unwrap(),
            original
        );
        let ports = crate::managed::LocalnetPorts::reserve().unwrap();
        let standard = prepare_localnet_at(
            "standard",
            &temporary.path().join("standard"),
            &ports,
            LocalnetServiceProfile::Standard,
            None,
        )
        .unwrap();
        assert!(standard.provider_advert(provider, issued_at).is_err());
    }
}
