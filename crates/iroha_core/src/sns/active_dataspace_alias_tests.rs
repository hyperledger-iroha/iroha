//! Focused reverse dataspace-alias resolution regressions.

use super::*;

#[test]
fn accepts_static_mapping_without_active_sns_record() {
    let catalog = dataspace_catalog();
    let world = World::default();

    assert_eq!(
        resolve_active_dataspace_id_by_alias(&world.view(), &catalog, "banking", 50)
            .expect("static mapping"),
        DataSpaceId::new(7)
    );
}

#[test]
fn rejects_multiple_dynamic_names_without_retaining_the_complete_alias_set() {
    let catalog = dataspace_catalog();
    let owner = another_owner();
    let address = AccountAddress::from_account_id(&owner).expect("account address");
    let shared_id = DataSpaceId::new(42);
    let mut world = World::default();
    for alias in ["alpha", "beta"] {
        let selector = selector_for_dataspace_alias(alias).expect("selector");
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        record.metadata.insert(
            SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            IrohaJson::new(shared_id.as_u64()),
        );
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
    }

    let error = resolve_active_dataspace_alias_by_id(&world.view(), &catalog, shared_id, 50)
        .expect_err("one id must not select between multiple active names");
    assert!(
        error
            .to_string()
            .contains(ALIAS_CATALOG_MAPPING_CONFLICT_CODE),
        "unexpected error: {error}"
    );
}
