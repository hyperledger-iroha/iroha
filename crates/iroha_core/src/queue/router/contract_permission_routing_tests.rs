// Contract permission mutations route by their exact immutable contract address.

use super::*;
use iroha_data_model::isi::{Grant, Revoke};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;

fn permissions(dataspace: DataSpaceId) -> [Permission; 2] {
    let contract = ContractAddress::derive(
        &super::super::super::queue_test_network_id(),
        &ALICE_ID,
        9,
        dataspace,
    )
    .unwrap();
    [
        CanInvokeContractEntrypoint {
            contract: contract.clone(),
            entrypoint: "hajimari".into(),
        }
        .into(),
        CanUseContractPermission {
            contract,
            permission: "write".parse().unwrap(),
        }
        .into(),
    ]
}

#[test]
fn contract_permission_wrappers_route_full_width_address_without_recipient_aliases() {
    let world = crate::state::World::new();
    let view = world.view();
    let catalog = DataSpaceCatalog::default();
    for dataspace in [DataSpaceId::new((1_u64 << 40) + 17), DataSpaceId::UNIVERSAL] {
        for permission in permissions(dataspace) {
            assert!(!dataspace_scoped_permission_target_needs_state(&permission));
            let instructions: [InstructionBox; 4] = [
                Grant::account_permission(permission.clone(), ALICE_ID.clone()).into(),
                Revoke::account_permission(permission.clone(), ALICE_ID.clone()).into(),
                Grant::role_permission(permission.clone(), "service".parse().unwrap()).into(),
                Revoke::role_permission(permission, "service".parse().unwrap()).into(),
            ];
            for instruction in instructions {
                assert_eq!(
                    instruction_transaction_dataspace_target(&*instruction, Some(&catalog), None)
                        .unwrap(),
                    Some(dataspace)
                );
                assert_eq!(
                    instruction_transaction_dataspace_target_with_world(
                        &*instruction,
                        Some(&catalog),
                        &view,
                        Some(0),
                    )
                    .unwrap(),
                    Some(dataspace)
                );
                let target =
                    native_instruction_execution_target(&*instruction, &catalog, &view, 0).unwrap();
                assert_eq!(target.dataspace, Some(dataspace));
                assert!(!target.global);
            }
        }
    }
}

#[test]
fn contract_permission_routing_rejects_inexact_payloads_and_preserves_decode_refusal() {
    let world = crate::state::World::new();
    let view = world.view();
    for permission in permissions(DataSpaceId::new(17)) {
        let mut value: norito::json::Value = permission.payload().try_into_any().unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), norito::json::Value::Bool(true));
        for payload in [Json::try_new(value).unwrap(), "null".parse().unwrap()] {
            let malformed = Permission::new(permission.name().into(), payload);
            assert!(matches!(
                dataspace_scoped_permission_target(&malformed, None, None),
                Err(RoutingResolveError::OrdinaryRouteUnavailable { .. })
            ));
            assert!(matches!(
                dataspace_scoped_permission_target_with_world(&malformed, None, &view, None),
                Err(RoutingResolveError::OrdinaryRouteUnavailable { .. })
            ));
        }
        let error = norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
            || dataspace_scoped_permission_target(&permission, None, None),
        )
        .unwrap_err();
        let RoutingResolveError::Deferred(owner) = error else {
            panic!("actual permission decoder refusal must retain its retry owner");
        };
        assert!(owner.decode_refusal().is_some());
    }
}
