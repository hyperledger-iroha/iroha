//! Opaque instruction envelopes and tolerant registrations (`specs/live_runtime_upgrades.md` §5.2).

use std::sync::Arc;

use norito::core::{DecodeFromSlice, DeserializePayload, SerializePayload};

use super::*;
use crate::prelude::Log;

/// A wire ID no release registers.
const UNKNOWN_WIRE_ID: &str = "test.unreleased.instruction.v1";
/// The wire ID a test registry registers `Log` under tolerantly.
const TOLERANT_WIRE_ID: &str = "test.tolerant.log.v1";
/// The wire ID a test registry registers `Log` under strictly.
const STRICT_WIRE_ID: &str = "test.strict.log.v1";

/// Install a thread-local registry for the life of the guard.
struct RegistryGuard(Option<Arc<InstructionRegistry>>);

impl RegistryGuard {
    fn enter(registry: InstructionRegistry) -> Self {
        Self(INSTRUCTION_REGISTRY_OVERRIDE.with(|slot| slot.replace(Some(Arc::new(registry)))))
    }

    /// `Log` registered strictly and tolerantly under two test wire IDs.
    fn test_registry() -> Self {
        Self::enter(
            InstructionRegistry::new()
                .register_with_id_slice_tolerant::<Log>(TOLERANT_WIRE_ID)
                .register_with_id::<CustomInstruction>(STRICT_WIRE_ID),
        )
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        INSTRUCTION_REGISTRY_OVERRIDE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

fn log(message: &str) -> Log {
    Log::new(crate::Level::INFO, message.to_owned())
}

/// The bare `(wire_id, frame)` tuple, as it sits inside a transaction.
fn bare_pair(wire_id: &str, frame: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    norito::core::serialize_to_buffer(&(wire_id.to_owned(), frame.to_vec()), &mut bytes)
        .expect("serialize pair");
    bytes
}

/// The same tuple as a complete Norito frame.
fn framed_pair(wire_id: &str, frame: &[u8]) -> Vec<u8> {
    norito::core::to_bytes(&(wire_id.to_owned(), frame.to_vec())).expect("frame pair")
}

/// A frame that is no valid Norito frame at all.
fn garbage_frame() -> Vec<u8> {
    (0_u8..48).map(|byte| byte.wrapping_mul(37)).collect()
}

/// A frame with a valid header around a payload the `Log` decoder refuses.
fn malformed_log_frame() -> Vec<u8> {
    norito::core::frame_bare_with_header_flags::<Log>(
        &[0xFF, 0x01, 0x02],
        norito::core::default_encode_flags(),
    )
    .expect("frame malformed payload")
}

/// The canonical frame of `value` as a `Log` payload.
fn canonical_log_frame(value: &Log) -> Vec<u8> {
    norito::encode_canonical(value).expect("canonical log frame")
}

/// Every supported layout's frame of `value`, with whether it is the canonical one.
fn log_frames_in_every_layout(value: &Log) -> Vec<(Vec<u8>, bool)> {
    let canonical = canonical_log_frame(value);
    (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .map(|flags| {
            let _requested = norito::core::DecodeFlagsGuard::enter(flags);
            let (payload, actual) = norito::codec::encode_with_header_flags(value);
            let frame = norito::core::frame_bare_with_header_flags::<Log>(&payload, actual)
                .expect("frame log payload");
            let is_canonical = frame == canonical;
            (frame, is_canonical)
        })
        .collect()
}

fn allocation_limits(bytes: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}

fn decode_slice(bytes: &[u8]) -> Result<InstructionBox, norito::Error> {
    InstructionBox::decode_from_slice(bytes).map(|(instruction, used)| {
        assert_eq!(used, bytes.len());
        instruction
    })
}

fn decode_archived_strict(framed: &[u8]) -> Result<InstructionBox, norito::Error> {
    let archived = norito::core::from_bytes::<InstructionBox>(framed).expect("pair frame");
    InstructionBox::try_deserialize(archived)
}

fn decode_archived_lossy(framed: &[u8]) -> InstructionBox {
    let archived = norito::core::from_bytes::<InstructionBox>(framed).expect("pair frame");
    InstructionBox::deserialize(archived)
}

fn decode_json(bytes: &[u8]) -> Result<InstructionBox, norito::json::Error> {
    let json = format!(
        "\"{}\"",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    norito::json::from_json::<InstructionBox>(&json)
}

fn assert_opaque(instruction: &InstructionBox, wire_id: &str, frame: &[u8]) {
    let opaque = instruction.as_opaque().expect("opaque envelope");
    assert_eq!(opaque.wire_id(), wire_id);
    assert_eq!(opaque.frame(), frame);
    assert_eq!(instruction_wire_id(instruction), Some(wire_id));
    assert_eq!(
        framed_instruction_payload(instruction),
        Some((wire_id, frame.to_vec()))
    );
}

#[test]
fn wire_id_rule_matches_the_interface_wire_id_rule() {
    let longest = "a".repeat(MAX_INSTRUCTION_WIRE_ID_BYTES);
    let too_long = "a".repeat(MAX_INSTRUCTION_WIRE_ID_BYTES + 1);
    let cases: [(&str, bool); 12] = [
        ("iroha.log", true),
        ("iroha.instruction.v1::kaigi::CreateKaigi", true),
        ("zk::ScheduleConfidentialPolicyTransition", true),
        ("a-b_c.d:e@f/g", true),
        (longest.as_str(), true),
        ("", false),
        (too_long.as_str(), false),
        ("iroha log", false),
        ("iroha.log!", false),
        ("iroha_data_model::isi::Register<Domain>", false),
        ("iroha.lög", false),
        ("iroha\u{0}log", false),
    ];
    for (wire_id, expected) in cases {
        assert_eq!(is_well_formed_wire_id(wire_id), expected, "{wire_id:?}");
        // Every wire ID an interface definition accepts is a well-formed envelope wire ID and
        // the other way round, so no envelope names a wire ID no interface could answer.
        let interface = crate::runtime::definition::DefinitionDescriptorV1::Interface(
            crate::runtime::definition::InterfaceDescriptorV1 {
                wire_id: wire_id.to_owned(),
                payload: crate::runtime::definition::SchemaV1::Codec("norito:test@v1".into()),
                rejection_codes: Vec::new(),
                events: Vec::new(),
                metering_class: "native.test".into(),
                contract_statement: [0; 32],
            },
        );
        assert_eq!(interface.validate().is_ok(), expected, "{wire_id:?}");
    }
}

#[test]
fn opaque_envelope_constructor_validates_its_wire_id() {
    let opaque = OpaqueInstructionV1::new(UNKNOWN_WIRE_ID, vec![1, 2, 3]).expect("well formed");
    assert_eq!(opaque.wire_id(), UNKNOWN_WIRE_ID);
    assert_eq!(opaque.frame(), &[1, 2, 3]);
    assert_eq!(
        opaque.clone().into_parts(),
        (UNKNOWN_WIRE_ID.to_owned(), vec![1, 2, 3])
    );
    for malformed in ["", "not a wire id", "type<Param>"] {
        assert!(OpaqueInstructionV1::new(malformed, Vec::new()).is_err());
    }
    let debug = format!("{:?}", InstructionBox::from(opaque));
    assert!(debug.contains(UNKNOWN_WIRE_ID), "{debug}");
    assert!(debug.contains("frame_len: 3"), "{debug}");
}

#[test]
fn unknown_wire_id_decodes_opaque_on_every_path_and_reencodes_exactly() {
    let _registry = RegistryGuard::test_registry();
    // The frame is never interpreted: any bytes, even no valid Norito frame, are kept.
    for frame in [
        garbage_frame(),
        Vec::new(),
        canonical_log_frame(&log("unknown")),
    ] {
        let bare = bare_pair(UNKNOWN_WIRE_ID, &frame);
        let framed = framed_pair(UNKNOWN_WIRE_ID, &frame);

        let from_slice = decode_slice(&bare).expect("slice decode");
        assert_opaque(&from_slice, UNKNOWN_WIRE_ID, &frame);
        let strict = decode_archived_strict(&framed).expect("strict archived decode");
        assert_opaque(&strict, UNKNOWN_WIRE_ID, &frame);
        let lossy = decode_archived_lossy(&framed);
        assert_opaque(&lossy, UNKNOWN_WIRE_ID, &frame);
        let canonical = norito::encode_canonical(&from_slice).expect("canonical envelope frame");
        let from_json = decode_json(&canonical).expect("JSON decode");
        assert_opaque(&from_json, UNKNOWN_WIRE_ID, &frame);
        let direct = decode_instruction_envelope(UNKNOWN_WIRE_ID, &frame).expect("envelope");
        assert_opaque(&direct, UNKNOWN_WIRE_ID, &frame);
        for decoded in [&strict, &lossy, &from_json, &direct] {
            assert_eq!(decoded, &from_slice);
        }

        // Re-encoding reproduces the exact bytes, so hashes do not depend on the release.
        let mut reencoded = Vec::new();
        norito::core::serialize_to_buffer(&from_slice, &mut reencoded).expect("re-encode");
        assert_eq!(reencoded, bare);
        assert_eq!(from_slice.encode(), bare);
        assert_eq!(
            SerializePayload::encoded_len_exact(&from_slice),
            Some(bare.len())
        );
        assert_eq!(
            SerializePayload::encoded_len_hint(&from_slice),
            Some(bare.len())
        );
        assert_eq!(norito::core::to_bytes(&from_slice).expect("frame"), framed);
        assert_eq!(
            iroha_crypto::HashOf::new(&from_slice),
            iroha_crypto::HashOf::new(&from_json)
        );
        assert_eq!(
            iroha_crypto::Hash::new(norito::encode_canonical(&from_json).expect("frame")),
            iroha_crypto::Hash::new(&canonical)
        );
        assert_eq!(
            norito::json::to_json(&from_slice).expect("JSON"),
            format!(
                "\"{}\"",
                base64::engine::general_purpose::STANDARD.encode(&canonical)
            )
        );

        // The typed decoder for clients never yields an opaque envelope.
        assert!(decode_instruction_from_pair(UNKNOWN_WIRE_ID, &frame).is_err());
    }
}

#[test]
fn opaque_envelopes_compare_by_wire_id_and_frame() {
    let a = InstructionBox::from(OpaqueInstructionV1::new("test.a.v1", vec![1, 2]).unwrap());
    let a_again = InstructionBox::from(OpaqueInstructionV1::new("test.a.v1", vec![1, 2]).unwrap());
    let other_frame =
        InstructionBox::from(OpaqueInstructionV1::new("test.a.v1", vec![1, 3]).unwrap());
    let other_id = InstructionBox::from(OpaqueInstructionV1::new("test.b.v1", vec![1, 2]).unwrap());
    assert_eq!(a, a_again);
    assert_eq!(a.clone(), a);
    assert_eq!(a.cmp(&a_again), core::cmp::Ordering::Equal);
    assert_ne!(a, other_frame);
    assert_ne!(a, other_id);
    assert!(a < other_frame);
    assert!(a < other_id);
    assert_ne!(a, InstructionBox::from(log("typed")));
}

#[test]
fn opaque_envelope_writes_only_under_its_own_wire_id() {
    let opaque = OpaqueInstructionV1::new(UNKNOWN_WIRE_ID, vec![9, 8, 7]).unwrap();
    let mut bytes = Vec::new();
    let mut encoder = norito::core::Encoder::for_buffer(&mut bytes);
    assert!(
        opaque
            .dyn_write_pair(&mut encoder, "test.other.v1")
            .is_err()
    );
    let mut bytes = Vec::new();
    let mut encoder = norito::core::Encoder::for_buffer(&mut bytes);
    opaque
        .dyn_write_pair(&mut encoder, UNKNOWN_WIRE_ID)
        .expect("own wire id");
    assert_eq!(bytes, bare_pair(UNKNOWN_WIRE_ID, &[9, 8, 7]));
    assert_eq!(opaque.dyn_frame_len().unwrap(), 3);
    let mut frame = Vec::new();
    opaque.dyn_write_frame(&mut frame).unwrap();
    assert_eq!(frame, [9, 8, 7]);
    assert_eq!(opaque.dyn_encode(), bare_pair(UNKNOWN_WIRE_ID, &[9, 8, 7]));
    assert_eq!(opaque.dyn_encoded_len(), Some(opaque.dyn_encode().len()));
}

#[test]
fn malformed_outer_framing_stays_a_decode_error() {
    let _registry = RegistryGuard::test_registry();
    let frame = garbage_frame();
    let bare = bare_pair(UNKNOWN_WIRE_ID, &frame);
    // Truncated or extended tuples.
    for len in [0, 1, bare.len() / 2, bare.len() - 1] {
        assert!(decode_slice(&bare[..len]).is_err(), "truncated to {len}");
    }
    let mut trailing = bare.clone();
    trailing.push(0);
    assert!(InstructionBox::decode_from_slice(&trailing).is_err());
    // Wire IDs outside the envelope rule are malformed framing on every release.
    let too_long = "a".repeat(MAX_INSTRUCTION_WIRE_ID_BYTES + 1);
    for wire_id in ["", "not a wire id", "iroha.log!", too_long.as_str()] {
        let error = decode_slice(&bare_pair(wire_id, &frame)).expect_err("malformed wire id");
        assert_eq!(error.to_string(), "malformed instruction wire identifier");
        assert!(decode_archived_strict(&framed_pair(wire_id, &frame)).is_err());
        let lossy = decode_archived_lossy(&framed_pair(wire_id, &frame));
        assert!(lossy.as_opaque().is_none());
        assert!(lossy.as_any().is::<transparent::InvalidInstruction>());
        assert!(decode_instruction_envelope(wire_id, &frame).is_err());
    }
    // A name that is no UTF-8 at all.
    let mut bytes = Vec::new();
    norito::core::serialize_to_buffer(&(vec![0xFF_u8, 0xFE], frame), &mut bytes)
        .expect("serialize raw name");
    assert!(decode_slice(&bytes).is_err());
}

#[test]
fn registered_wire_ids_keep_strict_decoding() {
    let _registry = RegistryGuard::enter(crate::instruction_registry::default());
    let wire_id = Log::WIRE_ID;
    for frame in [garbage_frame(), malformed_log_frame(), Vec::new()] {
        assert!(decode_slice(&bare_pair(wire_id, &frame)).is_err());
        assert!(decode_archived_strict(&framed_pair(wire_id, &frame)).is_err());
        assert!(decode_instruction_envelope(wire_id, &frame).is_err());
        let lossy = decode_archived_lossy(&framed_pair(wire_id, &frame));
        assert!(lossy.as_opaque().is_none());
        assert!(lossy.as_any().is::<transparent::InvalidInstruction>());
    }
    let value = log("registered");
    let frame = canonical_log_frame(&value);
    let decoded = decode_slice(&bare_pair(wire_id, &frame)).expect("typed decode");
    assert_eq!(decoded.as_any().downcast_ref::<Log>(), Some(&value));
}

#[test]
fn tolerant_registration_keeps_refused_frames_opaque() {
    let _registry = RegistryGuard::test_registry();
    let registry = instruction_registry();
    assert!(registry.is_tolerant(TOLERANT_WIRE_ID));
    assert!(!registry.is_tolerant(STRICT_WIRE_ID));
    assert!(!registry.is_tolerant(UNKNOWN_WIRE_ID));
    drop(registry);

    // The canonical frame of a valid payload decodes into the typed instruction.
    let value = log("tolerant");
    let boxed = InstructionBox::from(value.clone());
    assert_eq!(instruction_wire_id(&boxed), Some(TOLERANT_WIRE_ID));
    let (_, canonical) = framed_instruction_payload(&boxed).expect("frame");
    assert_eq!(canonical, canonical_log_frame(&value));
    let bare = bare_pair(TOLERANT_WIRE_ID, &canonical);
    let typed = decode_slice(&bare).expect("typed decode");
    assert_eq!(typed.as_any().downcast_ref::<Log>(), Some(&value));
    assert_eq!(typed.encode(), bare);
    assert_eq!(
        decode_archived_strict(&framed_pair(TOLERANT_WIRE_ID, &canonical)).expect("strict"),
        typed
    );

    // A frame the decoder refuses, or a valid payload in another layout, stays opaque and
    // re-encodes byte for byte, exactly like a release without the registration keeps it.
    let mut refused = vec![garbage_frame(), malformed_log_frame(), Vec::new()];
    refused.extend(
        log_frames_in_every_layout(&value)
            .into_iter()
            .filter_map(|(frame, is_canonical)| (!is_canonical).then_some(frame)),
    );
    for frame in refused {
        let bare = bare_pair(TOLERANT_WIRE_ID, &frame);
        let opaque = decode_slice(&bare).expect("tolerant decode");
        assert_opaque(&opaque, TOLERANT_WIRE_ID, &frame);
        assert_eq!(opaque.encode(), bare);
        let framed = framed_pair(TOLERANT_WIRE_ID, &frame);
        assert_eq!(decode_archived_strict(&framed).expect("strict"), opaque);
        assert_eq!(decode_archived_lossy(&framed), opaque);
        // The typed decoder for clients still refuses it.
        assert!(decode_instruction_from_pair(TOLERANT_WIRE_ID, &frame).is_err());
    }

    // The same refused frame under a strict registration is an error.
    assert!(decode_slice(&bare_pair(STRICT_WIRE_ID, &malformed_log_frame())).is_err());
}

#[test]
fn local_refusals_never_become_opaque() {
    let _registry = RegistryGuard::test_registry();
    // An unknown envelope charges both of its copies to the decode budget.
    let frame = vec![0xA5; 4096];
    let bare = bare_pair(UNKNOWN_WIRE_ID, &frame);
    let (result, _) =
        norito::core::with_decode_limits_measured(allocation_limits(1024), || decode_slice(&bare));
    assert!(result.expect_err("over budget").is_decode_resource_limit());
    let (result, _) =
        norito::core::with_decode_limits_measured(allocation_limits(usize::MAX), || {
            decode_slice(&bare)
        });
    assert_opaque(&result.expect("within budget"), UNKNOWN_WIRE_ID, &frame);

    // A tolerant registration whose decoder the caller's enclosing admission budget refuses
    // reports that local refusal: it is no verdict about the bytes, so it never becomes an opaque
    // envelope, whichever charge of the decode the budget refuses (the outer framing of a bare
    // pair or the tolerant decoder itself).
    let value = log(&"x".repeat(4096));
    let canonical = canonical_log_frame(&value);
    let bare = bare_pair(TOLERANT_WIRE_ID, &canonical);
    let decode_bare = || decode_slice(&bare);
    let decode_envelope = || decode_instruction_envelope(TOLERANT_WIRE_ID, &canonical);
    let decoders: [&dyn Fn() -> Result<InstructionBox, norito::Error>; 2] =
        [&decode_bare, &decode_envelope];
    for decode in decoders {
        let (within_budget, usage) =
            norito::core::with_decode_limits_measured(allocation_limits(usize::MAX), decode);
        // Within budget the frame decodes into the typed instruction.
        assert_eq!(
            within_budget.expect("typed").as_any().downcast_ref::<Log>(),
            Some(&value)
        );
        let demand = usage.total_allocated_bytes();
        for limit in (0..demand).step_by(97).chain([demand - 1]) {
            let pool = iroha_allocation::AllocationBudget::new(
                norito::core::DecodeBudgetContext::allocation_layout().size(),
            );
            let admission = norito::core::DecodeBudgetContext::try_new_owned(
                allocation_limits(demand.checked_mul(4).expect("admission budget")),
                &pool,
            )
            .expect("owned admission budget");
            let refusal = admission
                .with(|| {
                    norito::with_decode_limits_scope(allocation_limits(limit), || {
                        norito::core::classify_decode_attempt(decode)
                    })
                })
                .expect_err("refused by the enclosing admission budget");
            assert_eq!(
                refusal.kind(),
                norito::core::DecodeAttemptErrorKind::EnclosingLimit,
                "limit {limit}: {refusal:?}"
            );
            assert!(
                refusal.error().is_decode_resource_limit(),
                "limit {limit}: {refusal:?}"
            );
        }
    }
}

#[test]
fn a_protocol_limit_inside_a_tolerant_decoder_is_a_verdict_about_the_bytes() {
    let _registry = RegistryGuard::test_registry();
    // The frame claims a 255-byte field in a 3-byte payload: the decoder's own limit refuses it
    // on every node, so a release without the registration and this one both keep it opaque.
    let frame = malformed_log_frame();
    let strict = decode_instruction_from_pair(TOLERANT_WIRE_ID, &frame)
        .expect_err("the typed decoder refuses the frame");
    assert!(strict.is_decode_resource_limit(), "{strict:?}");
    let decoded = decode_slice(&bare_pair(TOLERANT_WIRE_ID, &frame)).expect("tolerant decode");
    assert_opaque(&decoded, TOLERANT_WIRE_ID, &frame);
}
