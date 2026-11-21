//! Routing preserves full-width child identity without conferring certificate authority.

use super::*;
use iroha_data_model::{
    isi::child_runtime::{AttestChildRuntimeSupportV1, AuthorizeChildRuntimeActionV1},
    runtime::{
        child_admin::{
            ChildRootIdentityV1, ChildRuntimeAdminActionV1, ChildRuntimeAdminCertificateV1,
            ChildRuntimeAdminPlanV1,
        },
        lifecycle::{ChangeSetId, RootScopeV1},
    },
};

#[test]
fn carrier_artifact_staging_keeps_the_exact_explicit_dataspace_route() {
    let dataspace = DataSpaceId::new((1_u64 << 44) + 91);
    let instruction: InstructionBox =
        iroha_data_model::isi::runtime_carrier::StageRuntimeCarrierArtifactV1 {
            artifact_id: iroha_data_model::smart_contract::ContractArtifactId::new(
                dataspace,
                iroha_crypto::Hash::new(b"route only"),
            ),
            code: vec![],
        }
        .into();
    let world = crate::state::World::new();
    assert_eq!(
        contract_artifact_dataspace_target(&*instruction),
        Some(dataspace)
    );
    assert_eq!(
        instruction_transaction_dataspace_target(&*instruction, None, None).unwrap(),
        Some(dataspace)
    );
    assert_eq!(
        instruction_transaction_dataspace_target_with_world(
            &*instruction,
            None,
            &world.view(),
            None
        )
        .unwrap(),
        Some(dataspace)
    );
}

#[test]
fn declared_child_targets_survive_both_routing_readers_without_truncation() {
    let dataspace = DataSpaceId::new((1_u64 << 44) + 27);
    let mut signed = crate::runtime::change_set::test_support::attest_instruction().attestation;
    signed.attestation.root = RootScopeV1::DataSpace(dataspace);
    let network = signed.attestation.network;
    let actions: Vec<InstructionBox> = vec![
        AttestChildRuntimeSupportV1 {
            attestation: signed,
        }
        .into(),
        AuthorizeChildRuntimeActionV1 {
            certificate: ChildRuntimeAdminCertificateV1 {
                plan: ChildRuntimeAdminPlanV1 {
                    root: ChildRootIdentityV1 {
                        network,
                        parent_network: network,
                        dataspace,
                        instance: [7; 32],
                    },
                    policy_id: [4; 32],
                    policy_revision: 1,
                    nonce: 0,
                    valid_from: 2,
                    valid_until: 10,
                    action: ChildRuntimeAdminActionV1::Cancel(ChangeSetId([5; 32])),
                },
                signatures: vec![],
            },
        }
        .into(),
    ];
    let world = crate::state::World::new();
    let view = world.view();
    for instruction in actions {
        // Intentionally unverified carriers: route extraction never pretends to approve them.
        assert_eq!(
            child_runtime_dataspace_target(&*instruction),
            Some(dataspace)
        );
        assert_eq!(
            instruction_transaction_dataspace_target(&*instruction, None, None).unwrap(),
            Some(dataspace)
        );
        assert_eq!(
            instruction_transaction_dataspace_target_with_world(&*instruction, None, &view, None)
                .unwrap(),
            Some(dataspace)
        );
    }
}

#[test]
fn ordinary_universal_support_does_not_acquire_the_child_route() {
    let universal = crate::runtime::change_set::test_support::attest_instruction();
    assert_eq!(child_runtime_dataspace_target(&universal), None);
    let wrong_scope = AttestChildRuntimeSupportV1 {
        attestation: universal.attestation,
    };
    assert_eq!(
        child_runtime_dataspace_target(&wrong_scope),
        Some(DataSpaceId::UNIVERSAL)
    );
}
