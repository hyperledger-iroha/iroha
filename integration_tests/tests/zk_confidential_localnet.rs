//! First-release confidential-ingress surface regression.
//!
//! The first release admits confidential movement only through protocol-bound
//! instructions such as Kagemusha V4 and native anonymous escrow. Generic
//! Shield, Transfer, and Unshield wires must never re-enter the registry.

use iroha_data_model::{
    instruction_registry,
    isi::{
        escrow::{
            CancelAnonymousAssetEscrow, OpenAnonymousAssetEscrow, ReleaseAnonymousAssetEscrow,
            ResolveAnonymousEscrowDispute,
        },
        offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
    },
};

#[test]
fn confidential_surface_excludes_generic_wires() {
    const RETIRED_GENERIC: &[&str] = &[
        "iroha_data_model::isi::zk::Shield",
        "iroha_data_model::isi::zk::ZkTransfer",
        "iroha_data_model::isi::zk::Unshield",
    ];
    let registry = instruction_registry::default();

    for retired in RETIRED_GENERIC {
        assert!(!registry.contains(retired));
        assert!(registry.decode(retired, &[]).is_none());
    }

    for specialized in [
        std::any::type_name::<TopUpKagemushaRecursiveV4>(),
        std::any::type_name::<RedeemKagemushaRecursiveV4>(),
        std::any::type_name::<OpenAnonymousAssetEscrow>(),
        std::any::type_name::<ReleaseAnonymousAssetEscrow>(),
        std::any::type_name::<CancelAnonymousAssetEscrow>(),
        std::any::type_name::<ResolveAnonymousEscrowDispute>(),
    ] {
        assert!(registry.contains(specialized));
        assert_eq!(registry.wire_id(specialized), Some(specialized));
    }
}
