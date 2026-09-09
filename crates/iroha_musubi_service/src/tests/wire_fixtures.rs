//! Complete pre-extraction publication frames and signed transcript evidence.

use super::*;
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json};

pub(crate) fn check_identity<T>(fixtures: &[json::Value])
where
    T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>,
{
    let nominal_name = T::nominal_name();
    let captured = fixtures
        .iter()
        .find(|row| {
            row.get("nominal_name").and_then(json::Value::as_str) == Some(nominal_name.as_str())
        })
        .expect("declared identity must have an original captured specimen");
    let hash = norito::schema::identity::frame_hash::<T>();
    assert_eq!(
        captured["serialize_hash"].as_str().unwrap(),
        hex::encode(hash)
    );
    assert_eq!(
        captured["deserialize_hash"].as_str().unwrap(),
        hex::encode(hash)
    );
}

pub(crate) fn record<T>(specimen: &str, value: &T) -> json::Value
where
    T: NoritoSerialize + for<'a> NoritoDeserialize<'a> + PartialEq + fmt::Debug,
{
    let frame = norito::encode_canonical(value).expect("canonical fixture frame");
    let decoded: T = norito::decode_canonical(&frame).expect("decode fixture frame");
    assert_eq!(&decoded, value, "{specimen}");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    assert_eq!(frame[6..22], norito::schema::identity::frame_hash::<T>());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut wrong_domain = frame.clone();
    wrong_domain[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_domain).is_err());
    norito::json!({
        "specimen": specimen,
        "nominal_name": (T::nominal_name()),
        "serialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "deserialize_hash": (hex::encode(norito::schema::identity::frame_hash::<T>())),
        "canonical_frame_hex": (hex::encode(&frame)),
        "canonical_payload_hex": (hex::encode(&frame[norito::core::Header::SIZE..])),
    })
}

fn authorization_records(
    runtime: &AuthenticatedMusubiPublicationRuntimeClientV1,
    operation: MusubiPublicationRuntimeOperationV1,
    operation_id: [u8; 32],
    body: &[u8],
) -> Vec<json::Value> {
    let digest = request_digest(operation, body).unwrap();
    let authorization = runtime
        .authorization_at(operation, operation_id, digest, 1_000)
        .unwrap();
    authorization
        .verify(operation, operation_id, digest, 1_001)
        .unwrap();
    let label = format!("{operation:?}");
    let approval = &authorization.approvals[0];
    let frame = norito::encode_canonical(&authorization).unwrap();
    let signing_hash = HashOf::new(&authorization.payload);
    vec![
        record(&format!("{label}.operation"), &operation),
        record(&format!("{label}.payload"), &authorization.payload),
        record(&format!("{label}.approval"), approval),
        record(&format!("{label}.signature"), &approval.signature),
        record(&format!("{label}.signing_hash"), &signing_hash),
        record(&format!("{label}.approvals"), &authorization.approvals),
        record(&format!("{label}.authorization"), &authorization),
        record(
            &format!("{label}.optional_authorization"),
            &Some(authorization.clone()),
        ),
        norito::json!({
            "specimen": (format!("{label}.transcript")),
            "request_digest": (hex::encode(digest)),
            "signing_payload_hex": (hex::encode(authorization.payload.encode())),
            "signing_hash": (hex::encode(signing_hash.as_ref())),
            "authorization_digest": (hex::encode(blake3::hash(&frame).as_bytes())),
            "authorization_header": (encode_authorization_header(&authorization).unwrap()),
        }),
    ]
}

fn control_records(fixture: &ControlServiceFixture) -> Vec<json::Value> {
    let mut records = vec![
        record(
            "registration",
            &fixture.storage_request.finalized_registration,
        ),
        record("storage.request", &fixture.storage_request),
        record("storage.response", &fixture.storage_response),
        record(
            "storage.needs_registration",
            &fixture.storage_response.disposition,
        ),
        record(
            "storage.registered",
            &MusubiStorageLocationDispositionV1::Registered(
                fixture.readback_request.location.clone(),
            ),
        ),
        record("readback.request", &fixture.readback_request),
        record("readback.response", &fixture.readback_response),
        record("receipt", &fixture.storage_request.staging_receipt),
    ];
    for (operation, body) in [
        (
            MusubiPublicationRuntimeOperationV1::StorageCoordination,
            norito::encode_canonical(&fixture.storage_request).unwrap(),
        ),
        (
            MusubiPublicationRuntimeOperationV1::ProviderReadback,
            norito::encode_canonical(&fixture.readback_request).unwrap(),
        ),
    ] {
        records.extend(authorization_records(
            &fixture.runtime,
            operation,
            fixture.storage_request.operation_id,
            &body,
        ));
    }
    records
}

fn error_records() -> Vec<json::Value> {
    use MusubiPublicationServiceErrorCodeV1::*;
    [
        RouteNotFound,
        MediaTypeInvalid,
        AuthorizationInvalid,
        AuthorizationExpired,
        AuthorizationReplay,
        RequestInvalid,
        IdentityMismatch,
        CarBodyMismatch,
        OperationConflict,
        OperationBusy,
        JournalUnavailable,
        SeedIngressUnavailable,
        StorageCoordinationUnavailable,
        ProviderReadbackUnavailable,
        ReceiptSigningUnavailable,
        BackendResponseInvalid,
        ResponseEncodingFailed,
        MethodInvalid,
        TrustedClockUnavailable,
    ]
    .into_iter()
    .flat_map(|code| {
        let response = MusubiPublicationServiceErrorResponseV1 {
            version: 1,
            code,
            retryable: false,
        };
        [
            record(code.as_str(), &code),
            record(&format!("{}.response", code.as_str()), &response),
        ]
    })
    .collect()
}

fn current_records() -> Vec<json::Value> {
    let seed = private_service_fixture(false);
    let witness =
        MusubiSeedIngressCarPlanV1::from_car_build_plan(&seed.plan, &seed.request.commitment)
            .expect("plan witness");
    let mut records = vec![
        record("seed.chunk", &witness.chunks[0]),
        record("seed.file", &witness.files[0]),
        record("seed.chunks", &witness.chunks),
        record("seed.files", &witness.files),
        record("seed.plan", &witness),
        record("seed.request", &seed.request),
        norito::json!({
            "specimen": "seed.transcript",
            "plan_digest": (hex::encode(witness.canonical_digest().unwrap().as_bytes())),
            "plan_length": (witness.canonical_len().unwrap()),
            "envelope_hex": (hex::encode(&seed.car)),
        }),
    ];
    records.extend(authorization_records(
        &seed.runtime,
        MusubiPublicationRuntimeOperationV1::SeedIngress,
        seed.request.operation_id,
        &seed.metadata,
    ));
    records.extend(control_records(&control_service_fixture(false, false)));
    records.extend(error_records());
    records.extend(publication_clock::wire_fixtures::records());
    records.extend(publication_journal::wire_fixtures::records(&seed.request));
    records
}

#[test]
fn publication_wire_frames_match_pre_extraction_goldens() {
    let expected: Vec<json::Value> = json::from_str(include_str!(
        "../../tests/fixtures/musubi_publication_frames.json"
    ))
    .expect("publication frame fixtures");
    let actual = current_records();
    assert_eq!(actual, expected);
}

#[test]
fn publication_schema_identities_match_original_frames() {
    let fixtures: Vec<json::Value> = json::from_str(include_str!(
        "../../tests/fixtures/musubi_publication_frames.json"
    ))
    .unwrap();
    macro_rules! check {
        ($($ty:ty),+ $(,)?) => { $(check_identity::<$ty>(&fixtures);)+ };
    }
    check!(
        MusubiPublicationRuntimeOperationV1,
        MusubiPublicationRuntimeAuthorizationPayloadV1,
        MusubiPublicationRuntimeAuthorizationApprovalV1,
        MusubiPublicationRuntimeAuthorizationV1,
        MusubiSeedIngressCarChunkV1,
        MusubiSeedIngressCarFileV1,
        MusubiSeedIngressCarPlanV1,
        MusubiSeedIngressStageRequestV1,
        MusubiFinalizedArchiveRegistrationEvidenceV1,
        MusubiStorageCoordinationRequestV1,
        MusubiStorageLocationDispositionV1,
        MusubiStorageCoordinationResponseV1,
        MusubiProviderReadbackRequestV1,
        MusubiProviderReadbackResponseV1,
        MusubiPublicationServiceErrorCodeV1,
        MusubiPublicationServiceErrorResponseV1,
        MusubiPublicationServiceJournalBindingV1,
        MusubiPublicationOperationBindingV1,
        MusubiPublicationIdempotencyKeyV1,
        SignatureOf<MusubiPublicationRuntimeAuthorizationPayloadV1>,
        HashOf<MusubiPublicationRuntimeAuthorizationPayloadV1>,
        Option<MusubiPublicationRuntimeAuthorizationV1>,
        Vec<MusubiPublicationRuntimeAuthorizationApprovalV1>,
        Vec<MusubiSeedIngressCarChunkV1>,
        Vec<MusubiSeedIngressCarFileV1>,
    );
    publication_clock::wire_fixtures::check_identities(&fixtures);
    publication_journal::wire_fixtures::check_identities(&fixtures);
}
