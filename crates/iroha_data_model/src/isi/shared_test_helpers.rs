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

pub(super) fn assert_registry_decodes_type_name<T>(registry: &InstructionRegistry, value: T)
where
    T: Instruction + Encode + 'static + norito::core::NoritoSerialize,
    for<'de> T: norito::core::NoritoDeserialize<'de>,
{
    assert_registry_decodes(registry, std::any::type_name::<T>(), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPE_NAME_CONSUMERS: [&str; 12] = [
        include_str!("bridge.rs"),
        include_str!("ministry.rs"),
        include_str!("kaigi.rs"),
        include_str!("social.rs"),
        include_str!("space_directory.rs"),
        include_str!("governance.rs"),
        include_str!("oracle.rs"),
        include_str!("escrow.rs"),
        include_str!("sorafs.rs"),
        include_str!("verifying_keys.rs"),
        include_str!("vpn.rs"),
        include_str!("smart_contract_code.rs"),
    ];
    const EXPLICIT_WIRE_ID_CONSUMERS: [&str; 14] = [
        include_str!("asset_alias.rs"),
        include_str!("ram_lfe.rs"),
        include_str!("endorsement.rs"),
        include_str!("asset_transfer_control.rs"),
        include_str!("identifier.rs"),
        include_str!("consensus_keys.rs"),
        include_str!("nexus.rs"),
        include_str!("account_recovery.rs"),
        include_str!("rwa.rs"),
        include_str!("zk.rs"),
        include_str!("settlement.rs"),
        include_str!("staking.rs"),
        include_str!("soracloud.rs"),
        include_str!("repo.rs"),
    ];
    const SLICE_ONLY_CONSUMERS: [&str; 3] = [
        include_str!("musubi.rs"),
        include_str!("contract_alias.rs"),
        include_str!("alias_setup.rs"),
    ];

    #[test]
    fn shared_instruction_test_helper_inventory_is_exact() {
        let mut slice_calls = 0;
        let mut registry_calls = 0;
        for source in TYPE_NAME_CONSUMERS {
            assert!(!source.contains("fn assert_slice_roundtrip"));
            assert!(!source.contains("fn assert_registry_decodes"));
            assert!(
                source.contains("assert_registry_decodes_type_name as assert_registry_decodes")
            );
            slice_calls += source.matches("assert_slice_roundtrip(").count();
            registry_calls += source.matches("assert_registry_decodes(").count();
        }
        for source in EXPLICIT_WIRE_ID_CONSUMERS {
            assert!(!source.contains("fn assert_slice_roundtrip"));
            assert!(!source.contains("fn assert_registry_decodes"));
            slice_calls += source.matches("assert_slice_roundtrip(").count();
            registry_calls += source.matches("assert_registry_decodes(").count();
        }
        for source in SLICE_ONLY_CONSUMERS {
            assert!(!source.contains("fn assert_slice_roundtrip"));
            slice_calls += source.matches("assert_slice_roundtrip(").count();
        }
        assert_eq!(slice_calls, 243);
        assert_eq!(registry_calls, 183);
        assert!(include_str!("register.rs").contains("fn assert_slice_roundtrip"));
        assert!(include_str!("privacy.rs").contains("fn assert_slice_roundtrip"));
        assert!(include_str!("defi.rs").contains("fn assert_registry_decodes"));
        assert!(include_str!("transparent.rs").contains("fn assert_registry_decodes_name"));
    }

    #[test]
    #[should_panic]
    fn slice_roundtrip_helper_rejects_trailing_bytes() {
        let value = super::super::Log::new(crate::Level::INFO, "slice mutation".to_owned());
        let mut bytes = norito::codec::Encode::encode(&value);
        bytes.push(0);
        assert_slice_bytes_roundtrip(value, &bytes);
    }

    #[test]
    #[should_panic(expected = "registered")]
    fn registry_helper_rejects_a_mutated_wire_id() {
        let registry = InstructionRegistry::new().register_slice::<super::super::Log>();
        let value = super::super::Log::new(crate::Level::INFO, "registry mutation".to_owned());
        assert_registry_decodes(&registry, "iroha.test.mutated", value);
    }
}
