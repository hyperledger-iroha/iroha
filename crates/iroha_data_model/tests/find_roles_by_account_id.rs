//! Query struct constructor sanity checks

use iroha_crypto::KeyPair;
use iroha_data_model::prelude::*;

#[test]
fn find_roles_by_account_id_constructor_returns_id() {
    let id = AccountId::new(KeyPair::random().public_key().clone());
    let query = FindRolesByAccountId::new(id.clone());
    assert_eq!(query, FindRolesByAccountId::new(id));
}
