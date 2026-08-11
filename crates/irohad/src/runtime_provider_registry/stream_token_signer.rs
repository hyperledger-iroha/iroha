//! Exact runtime-registry qualification for the stream-token signer.

use super::*;

/// Revalidate the deployment-owned Ed25519 signer twice at daemon startup.
pub(super) fn qualify_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::StreamTokenSigner;
    let Some(expected) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let signer = dependencies
        .sorafs_stream_token_signer
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(signer.handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if signer.handle() != expected.handle()
        || signer.public_key()
            != expected
                .stream_token_signer_public_key()
                .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let expected_qualification = iroha_torii::sorafs::StreamTokenRuntimeSignerQualificationV1::new(
        expected
            .revision()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?,
        expected
            .policy_digest()
            .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?,
    );
    expected_qualification
        .validate()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    let first = signer.qualification().map_err(map_probe_error)?;
    first
        .validate()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)?;
    if first != expected_qualification
        || signer.handle() != expected.handle()
        || signer.public_key()
            != expected
                .stream_token_signer_public_key()
                .expect("validated key")
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let second = signer.qualification().map_err(map_probe_error)?;
    second
        .validate()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)?;
    if second != first
        || signer.handle() != expected.handle()
        || signer.public_key()
            != expected
                .stream_token_signer_public_key()
                .expect("validated key")
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}

const fn map_probe_error(
    error: iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1,
) -> IrohaRuntimeProviderRegistryErrorV1 {
    match error {
        iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::Unavailable => {
            IrohaRuntimeProviderRegistryErrorV1::Unavailable
        }
        iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::StaleOrRevoked => {
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use iroha_torii::sorafs::{
        StreamTokenRuntimeSigner, StreamTokenRuntimeSignerProbeErrorV1,
        StreamTokenRuntimeSignerQualificationV1, StreamTokenSigningError,
    };

    use super::*;

    const HANDLE: &str = "software://sorafs/stream-token/eu-1";
    const REVISION: u64 = 7;
    const POLICY_DIGEST: [u8; 32] = [0x42; 32];

    fn public_key() -> [u8; 32] {
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![0x58; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("test Ed25519 key pair");
        key_pair
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("32-byte Ed25519 public key")
    }

    fn qualification(revision: u64) -> StreamTokenRuntimeSignerQualificationV1 {
        StreamTokenRuntimeSignerQualificationV1::new(revision, POLICY_DIGEST)
    }

    fn bindings() -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "iroha3-taira".to_owned(),
            network_id: crate::runtime_provider_registry::runtime_provider_test_network_id(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(
                    HANDLE,
                    public_key(),
                    REVISION,
                    POLICY_DIGEST,
                )
                .expect("valid stream-token signer binding"),
            ],
        }
    }

    struct ProviderProbe {
        handle: &'static str,
        public_key: [u8; 32],
        reports: Mutex<
            Vec<
                Result<
                    StreamTokenRuntimeSignerQualificationV1,
                    StreamTokenRuntimeSignerProbeErrorV1,
                >,
            >,
        >,
        calls: AtomicUsize,
    }

    impl StreamTokenRuntimeSigner for ProviderProbe {
        fn handle(&self) -> &str {
            self.handle
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key
        }

        fn qualification(
            &self,
        ) -> Result<StreamTokenRuntimeSignerQualificationV1, StreamTokenRuntimeSignerProbeErrorV1>
        {
            let index = self.calls.fetch_add(1, Ordering::Relaxed);
            self.reports
                .lock()
                .expect("qualification reports")
                .get(index)
                .copied()
                .unwrap_or(Ok(qualification(REVISION)))
        }

        fn sign(&self, _signing_payload: &[u8]) -> Result<[u8; 64], StreamTokenSigningError> {
            Err(StreamTokenSigningError::Refused)
        }
    }

    fn dependencies(
        handle: &'static str,
        public_key: [u8; 32],
        reports: Vec<
            Result<StreamTokenRuntimeSignerQualificationV1, StreamTokenRuntimeSignerProbeErrorV1>,
        >,
    ) -> IrohaRuntimeDeps {
        IrohaRuntimeDeps::default().with_sorafs_stream_token_signer(Arc::new(ProviderProbe {
            handle,
            public_key,
            reports: Mutex::new(reports),
            calls: AtomicUsize::new(0),
        }))
    }

    #[test]
    fn exact_provider_is_observed_twice() {
        qualify_dependency(
            &bindings(),
            &dependencies(HANDLE, public_key(), vec![Ok(qualification(REVISION)); 2]),
        )
        .expect("exact signer");
    }

    #[test]
    fn substituted_identity_is_rejected() {
        assert_eq!(
            qualify_dependency(
                &bindings(),
                &dependencies(HANDLE, public_key(), vec![Ok(qualification(REVISION + 1))]),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        );
        assert_eq!(
            qualify_dependency(&bindings(), &dependencies(HANDLE, [0x59; 32], vec![]),),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn drift_unavailability_and_test_markers_fail_closed() {
        assert_eq!(
            qualify_dependency(
                &bindings(),
                &dependencies(
                    HANDLE,
                    public_key(),
                    vec![Ok(qualification(REVISION)), Ok(qualification(REVISION + 1))],
                ),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked)
        );
        assert_eq!(
            qualify_dependency(
                &bindings(),
                &dependencies(
                    HANDLE,
                    public_key(),
                    vec![Err(StreamTokenRuntimeSignerProbeErrorV1::Unavailable)],
                ),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        );
        assert_eq!(
            qualify_dependency(
                &bindings(),
                &dependencies("software://sorafs/stream-token/test", public_key(), vec![]),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        );
    }
}
