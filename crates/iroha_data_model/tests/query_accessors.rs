//! Query accessor API coverage.

use iroha_crypto::KeyPair;
use iroha_data_model::prelude::*;
use nonzero_ext::nonzero;

#[test]
fn query_accessors_return_inner_values() {
    let account_id = AccountId::new(KeyPair::random().public_key().clone());
    let roles_query = FindRolesByAccountId::new(account_id.clone());
    assert_eq!(roles_query.account_id(), &account_id);

    let permissions_query = FindPermissionsByAccountId::new(account_id.clone());
    assert_eq!(permissions_query.account_id(), &account_id);

    let asset_definition_id =
        AssetDefinitionId::new("domain".parse().unwrap(), "asset".parse().unwrap());
    let accounts_query = FindAccountsWithAsset::new(asset_definition_id.clone());
    assert_eq!(accounts_query.asset_definition_id(), &asset_definition_id);

    let _: QueryBox<AccountId> = Box::new(FindAccountIds);

    let pagination = Pagination::new(Some(nonzero!(5_u64)), 10);
    assert_eq!(pagination.limit_value(), Some(nonzero!(5_u64)));
    assert_eq!(pagination.offset_value(), 10);
}
