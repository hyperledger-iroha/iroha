//! Typed Musubi instruction values for generated-record identity capture.

#[path = "../../bin/musubi_fixture_values.rs"]
mod fixture_values;

use super::super::generated_record_identity_tests::capture;
use super::{
    AcceptMusubiPackageMaintainerV1, InviteMusubiPackageMaintainerV1, RecoverMusubiPackageV1,
    RegisterMusubiAliasV1, RegisterMusubiArchiveV1, RegisterMusubiNamespaceBindingV1,
    RemoveMusubiPackageMaintainerV1, RetargetMusubiAliasV1, RetireMusubiArchiveLocationV1,
    SetMusubiArtifactTakedownV1, SetMusubiPackageMaintainerRoleV1, SetMusubiPackageMetadataV1,
    SetMusubiRegistryPolicyV1,
};
use norito::json::Value;

/// Build the missing Musubi generated-record capture rows from canonical typed fixtures.
pub fn values() -> Vec<Value> {
    let document = fixture_values::instruction_document();
    let cases = document
        .get("cases")
        .and_then(Value::as_array)
        .expect("canonical Musubi instruction document cases");
    assert_eq!(cases.len(), 19, "complete Musubi instruction fixture");
    for wire_id in [
        RegisterMusubiNamespaceBindingV1::WIRE_ID,
        RegisterMusubiArchiveV1::WIRE_ID,
        RetireMusubiArchiveLocationV1::WIRE_ID,
        SetMusubiPackageMetadataV1::WIRE_ID,
        InviteMusubiPackageMaintainerV1::WIRE_ID,
        AcceptMusubiPackageMaintainerV1::WIRE_ID,
        SetMusubiPackageMaintainerRoleV1::WIRE_ID,
        RemoveMusubiPackageMaintainerV1::WIRE_ID,
        RegisterMusubiAliasV1::WIRE_ID,
        RecoverMusubiPackageV1::WIRE_ID,
        RetargetMusubiAliasV1::WIRE_ID,
        SetMusubiArtifactTakedownV1::WIRE_ID,
        SetMusubiRegistryPolicyV1::WIRE_ID,
    ] {
        assert!(
            cases
                .iter()
                .any(|case| case.get("wire_id").and_then(Value::as_str) == Some(wire_id)),
            "Musubi instruction fixture is missing `{wire_id}`"
        );
    }
    let values = fixture_values::generated_identity_values();
    vec![
        capture::<RegisterMusubiNamespaceBindingV1>(values.register_namespace),
        capture::<RegisterMusubiArchiveV1>(values.register_archive),
        capture::<RetireMusubiArchiveLocationV1>(values.retire_archive_location),
        capture::<SetMusubiPackageMetadataV1>(values.set_package_metadata),
        capture::<InviteMusubiPackageMaintainerV1>(values.invite_package_maintainer),
        capture::<AcceptMusubiPackageMaintainerV1>(values.accept_package_maintainer),
        capture::<SetMusubiPackageMaintainerRoleV1>(values.set_package_maintainer_role),
        capture::<RemoveMusubiPackageMaintainerV1>(values.remove_package_maintainer),
        capture::<RegisterMusubiAliasV1>(values.register_alias),
        capture::<RecoverMusubiPackageV1>(values.recover_package),
        capture::<RetargetMusubiAliasV1>(values.retarget_alias),
        capture::<SetMusubiArtifactTakedownV1>(values.set_artifact_takedown),
        capture::<SetMusubiRegistryPolicyV1>(values.set_registry_policy),
    ]
}
