//! Shared assertions for instruction codec and registry tests.

use super::{Instruction, InstructionRegistry};
use norito::{codec::Encode, core::DecodeFromSlice};

pub(super) fn assert_slice_roundtrip<T>(value: T)
where
    T: Clone + PartialEq + core::fmt::Debug + Encode,
    for<'a> T: DecodeFromSlice<'a>,
{
    let bytes = value.encode();
    assert_slice_bytes_roundtrip(value, &bytes);
}

fn assert_slice_bytes_roundtrip<T>(value: T, bytes: &[u8])
where
    T: PartialEq + core::fmt::Debug,
    for<'a> T: DecodeFromSlice<'a>,
{
    let (decoded, used) = T::decode_from_slice(bytes).expect("decode from slice");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded, value);
}

pub(super) fn assert_registry_decodes<T>(registry: &InstructionRegistry, wire_id: &str, value: T)
where
    T: Instruction + Encode + 'static + norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    let (payload, flags) = norito::codec::encode_with_header_flags(&value);
    let framed = norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
    let decoded = InstructionRegistry::decode(registry, wire_id, &framed)
        .expect("registered")
        .expect("decode");
    assert_eq!(Instruction::dyn_encode(&*decoded), payload);
}

pub(super) fn assert_registry_decodes_registered_type<T>(registry: &InstructionRegistry, value: T)
where
    T: Instruction + Encode + 'static + norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    let wire_id = registry
        .wire_id(std::any::type_name::<T>())
        .expect("instruction type has an explicit wire identifier");
    assert_registry_decodes(registry, wire_id, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPE_NAME_CONSUMERS: [(&str, &str); 12] = [
        ("bridge.rs", include_str!("bridge.rs")),
        ("ministry.rs", include_str!("ministry.rs")),
        ("kaigi.rs", include_str!("kaigi.rs")),
        ("social.rs", include_str!("social.rs")),
        ("space_directory.rs", include_str!("space_directory.rs")),
        ("governance.rs", include_str!("governance.rs")),
        ("oracle.rs", include_str!("oracle.rs")),
        ("escrow.rs", include_str!("escrow.rs")),
        ("sorafs.rs", include_str!("sorafs.rs")),
        ("verifying_keys.rs", include_str!("verifying_keys.rs")),
        ("vpn.rs", include_str!("vpn.rs")),
        (
            "smart_contract_code.rs",
            include_str!("smart_contract_code.rs"),
        ),
    ];
    const EXPLICIT_WIRE_ID_CONSUMERS: [(&str, &str); 14] = [
        ("asset_alias.rs", include_str!("asset_alias.rs")),
        ("ram_lfe.rs", include_str!("ram_lfe.rs")),
        ("endorsement.rs", include_str!("endorsement.rs")),
        (
            "asset_transfer_control.rs",
            include_str!("asset_transfer_control.rs"),
        ),
        ("identifier.rs", include_str!("identifier.rs")),
        ("consensus_keys.rs", include_str!("consensus_keys.rs")),
        ("nexus.rs", include_str!("nexus.rs")),
        ("account_recovery.rs", include_str!("account_recovery.rs")),
        ("rwa.rs", include_str!("rwa.rs")),
        ("zk.rs", include_str!("zk.rs")),
        ("settlement.rs", include_str!("settlement.rs")),
        ("staking.rs", include_str!("staking.rs")),
        ("soracloud.rs", include_str!("soracloud.rs")),
        ("repo.rs", include_str!("repo.rs")),
    ];
    const SLICE_ONLY_CONSUMERS: [(&str, &str); 3] = [
        ("musubi.rs", include_str!("musubi.rs")),
        ("contract_alias.rs", include_str!("contract_alias.rs")),
        ("alias_setup.rs", include_str!("alias_setup.rs")),
    ];

    #[test]
    fn shared_instruction_test_helper_inventory_is_exact() {
        let mut owners = std::collections::BTreeSet::new();
        for (owner, source) in TYPE_NAME_CONSUMERS {
            assert!(owners.insert(owner), "duplicate helper owner: {owner}");
            assert!(!source.contains("fn assert_slice_roundtrip"));
            assert!(!source.contains("fn assert_registry_decodes"));
            assert!(
                source
                    .contains("assert_registry_decodes_registered_type as assert_registry_decodes")
            );
            assert!(
                source.contains("assert_slice_roundtrip("),
                "{owner} must use the shared slice helper"
            );
            assert!(
                source.contains("assert_registry_decodes("),
                "{owner} must use the shared registry helper"
            );
        }
        for (owner, source) in EXPLICIT_WIRE_ID_CONSUMERS {
            assert!(owners.insert(owner), "duplicate helper owner: {owner}");
            assert!(!source.contains("fn assert_slice_roundtrip"));
            assert!(!source.contains("fn assert_registry_decodes"));
            assert!(
                source.contains("assert_slice_roundtrip("),
                "{owner} must use the shared slice helper"
            );
            assert!(
                source.contains("assert_registry_decodes("),
                "{owner} must use the shared registry helper"
            );
        }
        for (owner, source) in SLICE_ONLY_CONSUMERS {
            assert!(owners.insert(owner), "duplicate helper owner: {owner}");
            assert!(!source.contains("fn assert_slice_roundtrip"));
            assert!(
                source.contains("assert_slice_roundtrip("),
                "{owner} must use the shared slice helper"
            );
        }
        assert!(include_str!("register.rs").contains("fn assert_slice_roundtrip"));
        assert!(include_str!("privacy.rs").contains("fn assert_slice_roundtrip"));
        assert!(include_str!("defi.rs").contains("fn assert_registry_decodes"));
        assert!(include_str!("transparent.rs").contains("fn assert_registry_decodes_name"));
    }

    #[test]
    #[should_panic(expected = "LengthMismatch")]
    fn slice_roundtrip_helper_rejects_trailing_bytes() {
        let value = super::super::Log::new(crate::Level::INFO, "slice mutation".to_owned());
        let mut bytes = norito::codec::Encode::encode(&value);
        bytes.push(0);
        assert_slice_bytes_roundtrip(value, &bytes);
    }

    #[test]
    #[should_panic(expected = "registered")]
    fn registry_helper_rejects_a_mutated_wire_id() {
        let registry = InstructionRegistry::new()
            .register_with_id_slice::<super::super::Log>(super::super::Log::WIRE_ID);
        let value = super::super::Log::new(crate::Level::INFO, "registry mutation".to_owned());
        assert_registry_decodes(&registry, "iroha.test.mutated", value);
    }
}
