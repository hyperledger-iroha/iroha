// Genuine signed private-route dispatch for the native finality barrier controls.

use super::*;
use base64::Engine as _;
use iroha_musubi_service::*;

struct Clock;
impl MusubiPublicationServiceClockV1 for Clock {
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
        Ok(1_000)
    }
}
struct UnusedSeed(ProviderId);
impl MusubiSeedIngressBackendV1 for UnusedSeed {
    fn provider_id(&self) -> ProviderId {
        self.0
    }
    fn stage_exact_car(
        &mut self,
        _: [u8; 32],
        _: &MusubiSeedIngressReceiptBindingV1,
        _: &MusubiArchiveCommitmentV1,
        _: &sorafs_car::CarBuildPlan,
        _: &[u8],
    ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
        panic!("storage route cannot stage another seed")
    }
}
struct UnusedReadback;
impl MusubiProviderReadbackBackendV1 for UnusedReadback {
    fn verify_current_target(
        &self,
        _: &MusubiProviderReadbackRequestV1,
    ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
        panic!("storage route cannot replay a provider readback")
    }
    fn readback_provider(
        &mut self,
        _: &MusubiProviderReadbackRequestV1,
    ) -> Result<MusubiProviderReadbackResponseV1, MusubiPublicationServiceBackendErrorV1> {
        panic!("storage route cannot perform a provider readback")
    }
}

pub(super) fn dispatch(
    backend: Box<dyn MusubiStorageCoordinationBackendV1>,
    request: &MusubiStorageCoordinationRequestV1,
) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1> {
    let binding = &request.staging_receipt.payload.binding;
    let configuration = MusubiPublicationServiceConfigurationV1 {
        network_id: binding.network_id,
        ingress_broker: binding.ingress_broker.clone(),
        seed_provider: binding.seed_provider,
        max_future_clock_skew_ms: 0,
        receipt_lifetime_ms: 60_000,
    };
    let journal = InMemoryMusubiPublicationServiceJournalV1::new(
        MusubiPublicationServiceJournalBindingV1::from_configuration(&configuration),
        2,
        4,
    )
    .unwrap();
    let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(
        binding.ingress_broker.clone(),
        keypair(0x32),
    )
    .unwrap();
    let mut service = MusubiPublicationPrivateServiceV1::new(
        configuration,
        Box::new(Clock),
        Box::new(signer),
        Box::new(journal),
        Box::new(UnusedSeed(binding.seed_provider)),
        backend,
        Box::new(UnusedReadback),
    )
    .unwrap();
    let body = norito::encode_canonical(request).unwrap();
    let operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
    // Independent protocol construction: the real private handler validates these bytes and
    // this signature before it can mint the borrowed backend argument.
    let mut digest = blake3::Hasher::new();
    digest.update(b"iroha-musubi-publication-runtime-request-v1\0");
    digest.update(&norito::encode_canonical(&operation).unwrap());
    digest.update(&(body.len() as u64).to_be_bytes());
    digest.update(&body);
    let payload = MusubiPublicationRuntimeAuthorizationPayloadV1 {
        domain: *b"musubi-pub-runtime-auth-v1\0\0\0\0\0\0",
        version: 1,
        operation,
        operation_id: request.operation_id,
        network_id: request.network_id,
        publisher: request.publisher.clone(),
        request_digest: *digest.finalize().as_bytes(),
        issued_at_ms: 999,
        expires_at_ms: 30_000,
    };
    let key = keypair(0x31);
    assert_eq!(request.publisher, AccountId::new(key.public_key().clone()));
    let signature = SignatureOf::try_new(key.private_key(), &payload).unwrap();
    let authorization = MusubiPublicationRuntimeAuthorizationV1 {
        payload,
        approvals: vec![MusubiPublicationRuntimeAuthorizationApprovalV1 {
            public_key: key.public_key().clone(),
            signature,
        }],
    };
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(norito::encode_canonical(&authorization).unwrap());
    let response = service.handle(MusubiPublicationPrivateHttpRequestV1 {
        method: "POST",
        path: MUSUBI_PUBLICATION_STORAGE_COORDINATION_PATH_V1,
        content_type: MUSUBI_PUBLICATION_NORITO_MEDIA_TYPE_V1,
        authorization: Some(&header),
        seed_ingress_metadata: None,
        body: &body,
    });
    match response.status {
        503 => Err(MusubiPublicationServiceBackendErrorV1::Retryable),
        422 => Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        other => panic!("unexpected native storage barrier response: {other}"),
    }
}
