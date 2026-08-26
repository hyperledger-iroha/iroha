//! First-release confidential-ingress surface regression.
//!
//! The first release admits confidential movement only through protocol-bound
//! instructions such as Kagemusha V4. Retired generic and escrow-specific
//! confidential wires must never re-enter the registry.
use iroha_data_model::{
    instruction_registry,
    isi::offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
};
#[test]
fn confidential_surface_excludes_retired_wires() {
    let retired = [
        ["iroha_data_model::isi::zk::", "Shield"].concat(),
        ["iroha_data_model::isi::zk::", "ZkTransfer"].concat(),
        ["iroha_data_model::isi::zk::", "Unshield"].concat(),
        [
            "iroha_data_model::isi::escrow::",
            "OpenAnonymous",
            "AssetEscrow",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "AcceptAnonymous",
            "AssetEscrow",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "MarkAnonymous",
            "EscrowPaymentSent",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "ReleaseAnonymous",
            "AssetEscrow",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "CancelAnonymous",
            "AssetEscrow",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "OpenAnonymous",
            "EscrowDispute",
        ]
        .concat(),
        [
            "iroha_data_model::isi::escrow::",
            "ResolveAnonymous",
            "EscrowDispute",
        ]
        .concat(),
    ];
    let registry = instruction_registry::default();
    for retired in &retired {
        assert!(!registry.contains(retired));
        assert!(registry.decode(retired, &[]).is_none());
    }
    for specialized in [
        std::any::type_name::<TopUpKagemushaRecursiveV4>(),
        std::any::type_name::<RedeemKagemushaRecursiveV4>(),
    ] {
        let wire_id = registry
            .wire_id(specialized)
            .expect("specialized instruction has an explicit V1 wire identifier");
        assert_ne!(wire_id, specialized);
        assert!(registry.contains(wire_id));
        assert!(!registry.contains(specialized));
    }
}
