//! Exact runtime-registry qualification for stream-token gateway admission.
use super::*;
/// Revalidate the deployment-owned quota, sequence, lease, and outbox provider.
pub(super) fn qualify_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::StreamTokenGatewayAdmission;
    let Some(expected) = bindings.iter().find(|binding| binding.slot() == slot) else {
        return Ok(());
    };
    let provider = dependencies
        .sorafs_stream_token_gateway_admission
        .as_ref()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
    if !is_production_runtime_handle(provider.handle()) {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let expected_qualification = expected
        .stream_token_gateway_admission_qualification()
        .ok_or(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    expected_qualification
        .validate()
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
    if expected.stream_token_gateway_admission_max_pending()
        != Some(expected_qualification.max_pending)
        || expected.stream_token_gateway_admission_max_tracked_tokens()
            != Some(expected_qualification.max_tracked_tokens)
        || !matches!(
            expected.stream_token_gateway_admission_reconcile_max_items(),
            Some(1..=iroha_torii::sorafs::STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1)
        )
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
    }
    let first = provider.qualification().map_err(map_provider_error)?;
    if first != expected_qualification || provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    let second = provider.qualification().map_err(map_provider_error)?;
    if second != first || provider.handle() != expected.handle() {
        return Err(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
    }
    Ok(())
}
const fn map_provider_error(
    error: iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
) -> IrohaRuntimeProviderRegistryErrorV1 {
    use iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1 as Error;
    match error {
        Error::Unavailable | Error::ReputationCallback => {
            IrohaRuntimeProviderRegistryErrorV1::Unavailable
        }
        Error::BindingMismatch | Error::InvalidRequest | Error::SubstitutedOutcome => {
            IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
        }
        Error::Rejected | Error::Conflict | Error::StaleOrRevoked | Error::Ambiguous => {
            IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_torii::sorafs::{
        StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1,
        StreamTokenGatewayAdmissionProviderV1, StreamTokenGatewayAdmissionQualificationV1,
        StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionRecordV1,
        StreamTokenGatewayAdmissionRequestV1, StreamTokenGatewayAdmissionResultV1,
    };
    use std::sync::Arc;
    const HANDLE: &str = "sealed://sorafs/stream-admission/eu-1";
    fn qualification(revision: u64) -> StreamTokenGatewayAdmissionQualificationV1 {
        StreamTokenGatewayAdmissionQualificationV1 {
            gateway_id: [0x41; 32],
            revision,
            policy_digest: [0x42; 32],
            max_pending: 64,
            max_tracked_tokens: 32,
            lease_ttl_ms: 120_000,
        }
    }
    fn bindings() -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "runtime-provider-test".to_owned(),
            network_id: crate::runtime_provider_registry::runtime_provider_test_network_id(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_stream_token_gateway_admission(
                    HANDLE,
                    qualification(7),
                    64,
                    32,
                    16,
                )
                .expect("valid gateway binding"),
            ],
        }
    }
    #[derive(Debug)]
    struct ProviderProbe {
        handle: &'static str,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    }
    impl StreamTokenGatewayAdmissionProviderV1 for ProviderProbe {
        fn handle(&self) -> &str {
            self.handle
        }
        fn qualification(
            &self,
        ) -> Result<StreamTokenGatewayAdmissionQualificationV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Ok(self.qualification)
        }
        fn admit(
            &self,
            _request: &StreamTokenGatewayAdmissionRequestV1,
        ) -> Result<StreamTokenGatewayAdmissionResultV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }
        fn pending(
            &self,
            _max_items: u32,
        ) -> Result<StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }
        fn acknowledge(
            &self,
            _record: StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }
        fn release_lease(
            &self,
            _record: StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }
    }
    fn dependencies(
        handle: &'static str,
        qualification: StreamTokenGatewayAdmissionQualificationV1,
    ) -> IrohaRuntimeDeps {
        IrohaRuntimeDeps::default().with_sorafs_stream_token_gateway_admission(Arc::new(
            ProviderProbe {
                handle,
                qualification,
            },
        ))
    }
    #[test]
    fn exact_provider_is_observed_twice() {
        qualify_dependency(&bindings(), &dependencies(HANDLE, qualification(7)))
            .expect("exact provider");
    }
    #[test]
    fn substituted_revision_is_rejected() {
        assert_eq!(
            qualify_dependency(&bindings(), &dependencies(HANDLE, qualification(8))),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        );
    }
    #[test]
    fn test_marked_provider_is_rejected() {
        assert_eq!(
            qualify_dependency(
                &bindings(),
                &dependencies("memory://stream-admission-test", qualification(7)),
            ),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        );
    }
}
