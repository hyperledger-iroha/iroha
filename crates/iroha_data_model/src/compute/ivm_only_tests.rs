//! Canonical IVM compute shapes and rejection of removed runtime selectors.

use super::*;
use norito::core::{DecodeFlagsGuard, header_flags};

const LAYOUTS: [u8; 6] = [
    0,
    header_flags::COMPACT_LEN,
    header_flags::PACKED_SEQ,
    header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
    header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
    header_flags::PACKED_STRUCT
        | header_flags::PACKED_SEQ
        | header_flags::COMPACT_LEN
        | header_flags::FIELD_BITSET,
];

fn manifest() -> ComputeManifest {
    norito::json::from_str(include_str!(
        "../../../../fixtures/compute/manifest_compute_payments.json"
    ))
    .expect("canonical IVM manifest")
}

fn budget() -> ComputeResourceBudget {
    ComputeResourceBudget {
        max_cycles: NonZeroU64::new(5_000_000).unwrap(),
        max_memory_bytes: NonZeroU64::new(128 * 1024 * 1024).unwrap(),
        max_stack_bytes: NonZeroU64::new(2 * 1024 * 1024).unwrap(),
        max_io_bytes: NonZeroU64::new(16 * 1024 * 1024).unwrap(),
        max_egress_bytes: NonZeroU64::new(8 * 1024 * 1024).unwrap(),
        allow_gpu_hints: false,
    }
}

#[test]
fn ivm_budget_rules_and_manifest_roundtrip_every_advertised_layout() {
    for requested in LAYOUTS {
        let _flags = DecodeFlagsGuard::enter(requested);
        let budget = budget();
        let manifest = manifest();
        manifest.validate().expect("valid IVM manifest");
        let rules = manifest.sandbox;
        let bytes = norito::to_bytes(&budget).expect("encode IVM budget");
        assert_eq!(
            norito::decode_from_bytes::<ComputeResourceBudget>(&bytes).unwrap(),
            budget
        );
        let bytes = norito::to_bytes(&rules).expect("encode IVM rules");
        assert_eq!(
            norito::decode_from_bytes::<ComputeSandboxRules>(&bytes).unwrap(),
            rules
        );
        let bytes = norito::to_bytes(&manifest).expect("encode IVM manifest");
        assert_eq!(
            norito::decode_from_bytes::<ComputeManifest>(&bytes).unwrap(),
            manifest
        );
    }
    let budget = budget();
    let rules = manifest().sandbox;
    let budget_json = norito::json::to_value(&budget).unwrap();
    assert_eq!(budget_json.as_object().unwrap().len(), 6);
    assert_eq!(
        norito::json::from_value::<ComputeResourceBudget>(budget_json).unwrap(),
        budget
    );
    let rules_json = norito::json::to_value(&rules).unwrap();
    assert_eq!(rules_json.as_object().unwrap().len(), 5);
    assert_eq!(
        norito::json::from_value::<ComputeSandboxRules>(rules_json).unwrap(),
        rules
    );
}

#[test]
fn ivm_budget_rejects_removed_allowance_in_json() {
    for value in [false, true] {
        let mut document = norito::json::to_value(&budget()).unwrap();
        document
            .as_object_mut()
            .unwrap()
            .insert("allow_wasi".into(), value.into());
        let encoded = norito::json::to_string(&document).unwrap();
        let error = norito::json::from_str::<ComputeResourceBudget>(&encoded)
            .expect_err("removed runtime allowance must be unknown");
        assert!(error.to_string().contains("allow_wasi"), "{error}");
        assert!(norito::json::from_value::<ComputeResourceBudget>(document).is_err());
    }
}

#[test]
fn ivm_rules_and_manifest_reject_every_removed_mode_in_json() {
    for mode in ["IvmOnly", "WasiLite"] {
        let mut document = norito::json::to_value(&manifest()).unwrap();
        let sandbox = document
            .as_object_mut()
            .unwrap()
            .get_mut("sandbox")
            .unwrap();
        sandbox
            .as_object_mut()
            .unwrap()
            .insert("mode".into(), norito::json!({"mode": mode, "value": null}));
        let rules = sandbox.clone();
        let encoded = norito::json::to_string(&rules).unwrap();
        let error = norito::json::from_str::<ComputeSandboxRules>(&encoded)
            .expect_err("the removed mode is never accepted, including the former IVM default");
        assert!(error.to_string().contains("mode"), "{error}");
        assert!(norito::json::from_value::<ComputeSandboxRules>(rules).is_err());
        assert!(norito::json::from_value::<ComputeManifest>(document).is_err());
    }
}

// These test-only encoders reproduce rejected bytes. They do not own a frame
// identity or provide a decoder, execution implementation, or compatibility API.
#[derive(norito::SerializePayload)]
struct RemovedBudgetShape {
    max_cycles: NonZeroU64,
    max_memory_bytes: NonZeroU64,
    max_stack_bytes: NonZeroU64,
    max_io_bytes: NonZeroU64,
    max_egress_bytes: NonZeroU64,
    allow_gpu_hints: bool,
    removed_runtime_allowance: bool,
}

// Keep this field opaque to the derive's fixed-width classifier, just as the
// removed enum was. A bare u32 field would change packed-struct size prefixes.
struct RemovedModeDiscriminant(u32);

impl norito::core::SerializePayload for RemovedModeDiscriminant {
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(4)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::SerializePayload::serialize(&self.0, writer)
    }
}

#[derive(norito::SerializePayload)]
struct RemovedRulesShape {
    // A unit enum's payload is its u32 discriminant. Preserve its original
    // leading field position and both discriminants without restoring the enum.
    removed_mode: RemovedModeDiscriminant,
    randomness: ComputeRandomnessPolicy,
    storage: ComputeStorageAccess,
    deny_nondeterministic_syscalls: bool,
    allow_gpu_hints: bool,
    allow_tee_hints: bool,
}

#[test]
fn ivm_budget_rejects_removed_binary_shape_under_current_identity() {
    let value = budget();
    for requested in LAYOUTS {
        let _flags = DecodeFlagsGuard::enter(requested);
        for removed_runtime_allowance in [false, true] {
            let removed = RemovedBudgetShape {
                max_cycles: value.max_cycles,
                max_memory_bytes: value.max_memory_bytes,
                max_stack_bytes: value.max_stack_bytes,
                max_io_bytes: value.max_io_bytes,
                max_egress_bytes: value.max_egress_bytes,
                allow_gpu_hints: value.allow_gpu_hints,
                removed_runtime_allowance,
            };
            let (payload, flags) = norito::codec::encode_with_header_flags(&removed);
            let frame = norito::core::frame_bare_with_header_flags::<ComputeResourceBudget>(
                &payload, flags,
            )
            .expect("frame rejected budget bytes under the current identity");
            assert!(
                norito::decode_from_bytes::<ComputeResourceBudget>(&frame).is_err(),
                "accepted removed budget field in layout {flags:#x}"
            );
        }
    }
}

#[test]
fn ivm_rules_reject_both_removed_binary_modes_under_current_identity() {
    let value = manifest().sandbox;
    for requested in LAYOUTS {
        let _flags = DecodeFlagsGuard::enter(requested);
        for removed_mode in [0, 1] {
            let removed = RemovedRulesShape {
                removed_mode: RemovedModeDiscriminant(removed_mode),
                randomness: value.randomness,
                storage: value.storage,
                deny_nondeterministic_syscalls: value.deny_nondeterministic_syscalls,
                allow_gpu_hints: value.allow_gpu_hints,
                allow_tee_hints: value.allow_tee_hints,
            };
            let (payload, flags) = norito::codec::encode_with_header_flags(&removed);
            let frame =
                norito::core::frame_bare_with_header_flags::<ComputeSandboxRules>(&payload, flags)
                    .expect("frame rejected rule bytes under the current identity");
            assert!(
                norito::decode_from_bytes::<ComputeSandboxRules>(&frame).is_err(),
                "accepted removed mode {removed_mode} in layout {flags:#x}"
            );
        }
    }
}

#[test]
fn ivm_compute_schema_contains_only_current_guardrails() {
    use iroha_schema::Metadata;

    let schema = ComputeManifest::schema();
    let Metadata::Struct(rules) = schema.get::<ComputeSandboxRules>().unwrap() else {
        panic!("sandbox guardrails must remain a struct");
    };
    assert_eq!(
        rules
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "randomness",
            "storage",
            "deny_nondeterministic_syscalls",
            "allow_gpu_hints",
            "allow_tee_hints"
        ]
    );
    assert!(
        schema
            .iter()
            .all(|(_, entry)| !entry.type_name.contains("ComputeSandboxMode"))
    );
    let schema = ComputeResourceBudget::schema();
    let Metadata::Struct(budget) = schema.get::<ComputeResourceBudget>().unwrap() else {
        panic!("resource budgets must remain a struct");
    };
    assert_eq!(
        budget
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "max_cycles",
            "max_memory_bytes",
            "max_stack_bytes",
            "max_io_bytes",
            "max_egress_bytes",
            "allow_gpu_hints"
        ]
    );
}
