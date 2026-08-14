//! Exact-current positive and forbidden-syscall admission vectors.
use std::ops::Range;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::Hash;
use iroha_data_model::prelude::{
    DecimalValueV1, IntValueV1, NUMERIC_FRAME_HEADER_BYTES_V1, NumericAbiError, QuantityValueV1,
};
use ivm_abi::{
    metadata::{LiteralKindV1, ProgramMetadata, decode_literal_descriptor},
    pointer_abi::{PointerType, validate_tlv_bytes},
};
use ivm_artifact_admission::{verify_contract_artifact, verify_contract_artifact_json};
const CURRENT_FIXTURE: &str =
    include_str!("../../../javascript/iroha_js/test/fixtures/current_rust_contract_artifact.json");
const INTEGER_FIXTURE: &[u8] =
    include_bytes!("../../kotodama_lang/src/samples/tuple_return_demo.to");
const DECIMAL_FIXTURE: &[u8] = include_bytes!("../../../demo/irohaswap.to");
const QUANTITY_FIXTURE: &[u8] = include_bytes!("../../../demo/prediction_market.to");
const STRUCTURED_FIXTURE: &[u8] = include_bytes!("../../ivm/docs/examples/12_nft_flow.to");
const ASSET_DEFINITION_FIXTURE: &[u8] =
    include_bytes!("../../ivm/docs/examples/13_register_and_mint.to");
const DOMAIN_FIXTURE: &[u8] = include_bytes!("../../ivm/docs/examples/16_register_domain.to");
const NUMERIC_FRAME_CHECKSUM_OFFSET: usize = 31;
const POINTER_ENVELOPE_HEADER_BYTES: usize = 7;
const SEMANTIC_LITERAL_ERROR: &str =
    "invalid contract artifact: literal index validation failed: invalid program metadata";
struct CurrentFixture {
    artifact: Vec<u8>,
    code_hash_hex: String,
    abi_hash_hex: String,
    header_len: usize,
    code_offset: usize,
    entrypoint_count: usize,
}
fn current_fixture() -> CurrentFixture {
    let fixture = norito::json::from_str::<norito::json::Value>(CURRENT_FIXTURE)
        .expect("parse exact-current fixture metadata");
    let fixture = fixture
        .as_object()
        .expect("exact-current fixture metadata must be an object");
    let encoded = fixture
        .get("artifact_base64")
        .and_then(norito::json::Value::as_str)
        .expect("fixture carries artifact_base64");
    let semantics = fixture
        .get("artifact_semantics")
        .and_then(norito::json::Value::as_object)
        .expect("fixture carries artifact_semantics");
    let string_field = |name| {
        semantics
            .get(name)
            .and_then(norito::json::Value::as_str)
            .unwrap_or_else(|| panic!("artifact_semantics carries string {name}"))
            .to_owned()
    };
    let usize_field = |name| {
        semantics
            .get(name)
            .and_then(norito::json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_else(|| panic!("artifact_semantics carries usize {name}"))
    };
    CurrentFixture {
        artifact: STANDARD.decode(encoded).expect("decode fixture artifact"),
        code_hash_hex: string_field("code_hash_hex"),
        abi_hash_hex: string_field("abi_hash_hex"),
        header_len: usize_field("header_len"),
        code_offset: usize_field("code_offset"),
        entrypoint_count: usize_field("entrypoint_count"),
    }
}
fn numeric_fixtures() -> [(&'static str, &'static [u8], PointerType); 3] {
    [
        ("int", INTEGER_FIXTURE, PointerType::Int),
        ("decimal", DECIMAL_FIXTURE, PointerType::Decimal),
        ("quantity", QUANTITY_FIXTURE, PointerType::Quantity),
    ]
}
fn pointer_literal_range(artifact: &[u8], expected_type: PointerType) -> Range<usize> {
    let parsed = ProgramMetadata::parse(artifact).expect("parse numeric contract fixture");
    let section = parsed
        .literal_section
        .expect("numeric contract fixture carries LTLB");
    let descriptors = (0..section.count)
        .map(|index| {
            let entry_start = section.entries_start + index * 8;
            let raw = u64::from_le_bytes(
                artifact[entry_start..entry_start + 8]
                    .try_into()
                    .expect("literal descriptor is eight bytes"),
            );
            let (kind, relative) =
                decode_literal_descriptor(raw).expect("decode literal descriptor");
            let target = section.start
                + usize::try_from(relative).expect("fixture literal offset fits usize");
            (kind, target)
        })
        .collect::<Vec<_>>();
    descriptors
        .iter()
        .enumerate()
        .find_map(|(index, (kind, start))| {
            if *kind != LiteralKindV1::PointerTlv {
                return None;
            }
            let end = descriptors
                .get(index + 1)
                .map_or(section.data_end, |(_, target)| *target);
            let tlv = validate_tlv_bytes(&artifact[*start..end])
                .expect("generated pointer literal has a valid outer envelope");
            (tlv.type_id == expected_type).then_some(*start..end)
        })
        .unwrap_or_else(|| panic!("fixture carries a {expected_type:?} literal"))
}
fn pointer_payload_range(artifact: &[u8], envelope: &Range<usize>) -> Range<usize> {
    let payload_len = usize::try_from(u32::from_be_bytes(
        artifact[envelope.start + 3..envelope.start + POINTER_ENVELOPE_HEADER_BYTES]
            .try_into()
            .expect("pointer envelope length is four bytes"),
    ))
    .expect("u32 payload length fits usize");
    let start = envelope.start + POINTER_ENVELOPE_HEADER_BYTES;
    let end = start + payload_len;
    assert_eq!(
        end + Hash::LENGTH,
        envelope.end,
        "pointer envelope must contain exactly one payload and hash"
    );
    start..end
}
fn reseal_pointer_hash(artifact: &mut [u8], envelope: &Range<usize>) {
    let payload = pointer_payload_range(artifact, envelope);
    let digest = Hash::new(&artifact[payload.clone()]);
    artifact[payload.end..envelope.end].copy_from_slice(digest.as_ref());
}
fn reseal_numeric_checksum_and_pointer_hash(artifact: &mut [u8], envelope: &Range<usize>) {
    let payload = pointer_payload_range(artifact, envelope);
    let checksum = norito::core::hardware_crc64(
        &artifact[payload.start + NUMERIC_FRAME_HEADER_BYTES_V1..payload.end],
    );
    artifact[payload.start + NUMERIC_FRAME_CHECKSUM_OFFSET
        ..payload.start + NUMERIC_FRAME_CHECKSUM_OFFSET + 8]
        .copy_from_slice(&checksum.to_le_bytes());
    reseal_pointer_hash(artifact, envelope);
}
fn decode_numeric_payload(
    artifact: &[u8],
    envelope: &Range<usize>,
    pointer_type: PointerType,
) -> Result<(), NumericAbiError> {
    let payload = pointer_payload_range(artifact, envelope);
    match pointer_type {
        PointerType::Int => IntValueV1::decode_frame(&artifact[payload]).map(|_| ()),
        PointerType::Decimal => DecimalValueV1::decode_frame(&artifact[payload]).map(|_| ()),
        PointerType::Quantity => QuantityValueV1::decode_frame(&artifact[payload]).map(|_| ()),
        other => panic!("expected numeric pointer type, got {other:?}"),
    }
}
fn assert_outer_envelope_valid(
    artifact: &[u8],
    envelope: &Range<usize>,
    pointer_type: PointerType,
) {
    let tlv = validate_tlv_bytes(&artifact[envelope.clone()])
        .expect("outer pointer envelope remains hash-valid");
    assert_eq!(tlv.type_id, pointer_type);
}
fn assert_shared_admission_rejects_semantic_literal(
    artifact: &[u8],
    envelope: &Range<usize>,
    pointer_type: PointerType,
) {
    assert_outer_envelope_valid(artifact, envelope, pointer_type);
    let error = verify_contract_artifact(artifact)
        .expect_err("semantically malformed numeric literal must fail shared admission");
    assert_eq!(error.to_string(), SEMANTIC_LITERAL_ERROR);
    assert_eq!(
        verify_contract_artifact_json(artifact),
        format!("{{\"ok\":false,\"error\":\"{SEMANTIC_LITERAL_ERROR}\"}}")
    );
}
#[test]
fn exact_current_compiler_artifact_is_admitted() {
    let fixture = current_fixture();
    let verified = verify_contract_artifact(&fixture.artifact)
        .expect("exact-current compiler artifact must satisfy shared admission");
    assert_eq!(verified.header_len, fixture.header_len);
    assert_eq!(verified.code_offset, fixture.code_offset);
    assert_eq!(
        verified.contract_interface.entrypoints.len(),
        fixture.entrypoint_count
    );
    assert_eq!(
        hex::encode(verified.code_hash.as_ref()),
        fixture.code_hash_hex
    );
    assert_eq!(
        hex::encode(verified.abi_hash.as_ref()),
        fixture.abi_hash_hex
    );
    let json = verify_contract_artifact_json(&fixture.artifact);
    assert!(json.starts_with("{\"ok\":true,"), "{json}");
}
#[test]
fn host_private_system_syscall_is_rejected() {
    let fixture = current_fixture();
    let mut mutated = fixture.artifact;
    // Canonical wide SYSTEM encoding for host-private syscall 0x00fe0000.
    mutated[fixture.code_offset..fixture.code_offset + 4]
        .copy_from_slice(&[0x00, 0x00, 0xfe, 0x62]);
    let error = verify_contract_artifact(&mutated)
        .expect_err("host-private SYSTEM syscall must fail shared admission");
    assert!(error.to_string().contains("disallowed syscall 0xfe0000"));
    let json = verify_contract_artifact_json(&mutated);
    assert!(json.starts_with("{\"ok\":false,"), "{json}");
}
#[test]
fn hash_valid_numeric_literals_with_inner_checksum_faults_are_rejected() {
    for (name, fixture, pointer_type) in numeric_fixtures() {
        verify_contract_artifact(fixture)
            .unwrap_or_else(|error| panic!("canonical {name} fixture must be admitted: {error}"));
        let mut mutated = fixture.to_vec();
        let envelope = pointer_literal_range(&mutated, pointer_type);
        let payload = pointer_payload_range(&mutated, &envelope);
        mutated[payload.start + NUMERIC_FRAME_CHECKSUM_OFFSET] ^= 0x01;
        reseal_pointer_hash(&mut mutated, &envelope);
        assert!(
            matches!(
                decode_numeric_payload(&mutated, &envelope, pointer_type),
                Err(NumericAbiError::Norito(_))
            ),
            "{name} decoder must observe the inner checksum fault"
        );
        assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, pointer_type);
    }
}
#[test]
fn hash_valid_numeric_literals_with_wrong_schemas_are_rejected() {
    for (name, fixture, pointer_type) in numeric_fixtures() {
        let mut mutated = fixture.to_vec();
        let envelope = pointer_literal_range(&mutated, pointer_type);
        let payload = pointer_payload_range(&mutated, &envelope);
        mutated[payload.start + 6] ^= 0x01;
        reseal_pointer_hash(&mut mutated, &envelope);
        assert_eq!(
            decode_numeric_payload(&mutated, &envelope, pointer_type),
            Err(NumericAbiError::SchemaMismatch),
            "{name} decoder must observe the nominal schema fault"
        );
        assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, pointer_type);
    }
}
#[test]
fn hash_valid_structured_literals_with_wrong_schemas_are_rejected() {
    for (name, fixture, pointer_type) in [
        ("account", STRUCTURED_FIXTURE, PointerType::AccountId),
        ("nft", STRUCTURED_FIXTURE, PointerType::NftId),
        ("name", STRUCTURED_FIXTURE, PointerType::Name),
        ("json", STRUCTURED_FIXTURE, PointerType::Json),
        (
            "asset definition",
            ASSET_DEFINITION_FIXTURE,
            PointerType::AssetDefinitionId,
        ),
        ("domain", DOMAIN_FIXTURE, PointerType::DomainId),
    ] {
        verify_contract_artifact(fixture)
            .unwrap_or_else(|error| panic!("canonical {name} fixture must be admitted: {error}"));
        let mut mutated = fixture.to_vec();
        let envelope = pointer_literal_range(&mutated, pointer_type);
        let payload = pointer_payload_range(&mutated, &envelope);
        assert!(
            payload.len() > 6,
            "{name} canonical Norito frame carries its schema field"
        );
        mutated[payload.start + 6] ^= 0x01;
        reseal_pointer_hash(&mut mutated, &envelope);
        assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, pointer_type);
    }
}
#[test]
fn hash_valid_noncanonical_integer_literal_is_rejected() {
    let mut mutated = INTEGER_FIXTURE.to_vec();
    let envelope = pointer_literal_range(&mutated, PointerType::Int);
    let payload = pointer_payload_range(&mutated, &envelope);
    let body_start = payload.start + NUMERIC_FRAME_HEADER_BYTES_V1;
    assert_eq!(
        u32::from_le_bytes(
            mutated[body_start..body_start + 4]
                .try_into()
                .expect("integer mantissa length")
        ),
        1
    );
    mutated[body_start + 4] = 0;
    reseal_numeric_checksum_and_pointer_hash(&mut mutated, &envelope);
    assert_eq!(
        decode_numeric_payload(&mutated, &envelope, PointerType::Int),
        Err(NumericAbiError::NonCanonicalMantissa)
    );
    assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, PointerType::Int);
}
#[test]
fn hash_valid_noncanonical_decimal_literal_is_rejected() {
    let mut mutated = DECIMAL_FIXTURE.to_vec();
    let envelope = pointer_literal_range(&mutated, PointerType::Decimal);
    let payload = pointer_payload_range(&mutated, &envelope);
    let body_start = payload.start + NUMERIC_FRAME_HEADER_BYTES_V1;
    assert_eq!(
        u32::from_le_bytes(
            mutated[body_start..body_start + 4]
                .try_into()
                .expect("decimal mantissa length")
        ),
        2
    );
    mutated[body_start + 4..body_start + 6].copy_from_slice(&1000_i16.to_le_bytes());
    mutated[body_start + 6] = 1;
    reseal_numeric_checksum_and_pointer_hash(&mut mutated, &envelope);
    assert_eq!(
        decode_numeric_payload(&mutated, &envelope, PointerType::Decimal),
        Err(NumericAbiError::NonCanonicalDecimal)
    );
    assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, PointerType::Decimal);
}
#[test]
fn hash_valid_negative_quantity_literal_is_rejected() {
    let mut mutated = QUANTITY_FIXTURE.to_vec();
    let envelope = pointer_literal_range(&mutated, PointerType::Quantity);
    let payload = pointer_payload_range(&mutated, &envelope);
    let body_start = payload.start + NUMERIC_FRAME_HEADER_BYTES_V1;
    assert_eq!(
        u32::from_le_bytes(
            mutated[body_start..body_start + 4]
                .try_into()
                .expect("quantity mantissa length")
        ),
        1
    );
    mutated[body_start + 4] = 0xff;
    mutated[body_start + 5] = 0;
    reseal_numeric_checksum_and_pointer_hash(&mut mutated, &envelope);
    assert_eq!(
        decode_numeric_payload(&mutated, &envelope, PointerType::Quantity),
        Err(NumericAbiError::NegativeQuantity)
    );
    assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, PointerType::Quantity);
}
#[test]
fn hash_valid_noncanonical_quantity_literal_is_rejected() {
    let mut mutated = QUANTITY_FIXTURE.to_vec();
    let envelope = pointer_literal_range(&mutated, PointerType::Quantity);
    let payload = pointer_payload_range(&mutated, &envelope);
    let body_start = payload.start + NUMERIC_FRAME_HEADER_BYTES_V1;
    mutated[body_start + 4] = 10;
    mutated[body_start + 5] = 1;
    reseal_numeric_checksum_and_pointer_hash(&mut mutated, &envelope);
    assert_eq!(
        decode_numeric_payload(&mutated, &envelope, PointerType::Quantity),
        Err(NumericAbiError::NonCanonicalDecimal)
    );
    assert_shared_admission_rejects_semantic_literal(&mutated, &envelope, PointerType::Quantity);
}
