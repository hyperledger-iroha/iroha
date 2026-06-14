//! Data-model mutators: unit tests for Domain and `AssetDefinition`
//!
//! Verifies that new/modified mutators behave correctly and keep state coherent.

use iroha_crypto::KeyPair;
use iroha_data_model::prelude::*;

fn checked_random_account_id() -> AccountId {
    AccountId::new(
        KeyPair::try_random()
            .expect("generate checked mutator account keypair")
            .public_key()
            .clone(),
    )
}

#[test]
fn domain_metadata_mut_and_set_owned_by() {
    // Prepare an authority account to build the domain with
    let domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("valid domain id");
    let authority = checked_random_account_id();

    let mut domain = Domain::new(domain_id.clone()).build(&authority);

    // metadata_mut: insert a key/value and verify it is stored
    let key: Name = "title".parse().unwrap();
    let val: Json = "The Wonderland".into();
    domain.metadata_mut().insert(key.clone(), val.clone());
    assert_eq!(domain.metadata().get(&key), Some(&val));

    // set_owned_by: change owner to a different account id
    let new_owner = checked_random_account_id();
    domain.set_owned_by(new_owner.clone());
    assert_eq!(domain.owned_by(), &new_owner);
}

#[test]
fn asset_definition_mutators_metadata_mintable_owner() {
    // Build an AssetDefinition and exercise mutators
    let _domain_id: DomainId = DomainId::try_new("land", "universal").expect("valid");
    let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("land", "universal").unwrap(),
        "rose".parse().unwrap(),
    );

    // Create an owner account in the same domain
    let owner = checked_random_account_id();

    let mut def = AssetDefinition::numeric(asset_def_id.clone()).build(&owner);

    // metadata_mut: set a metadata entry and verify
    let key: Name = "color".parse().unwrap();
    let val: Json = "red".into();
    def.metadata_mut().insert(key.clone(), val.clone());
    assert_eq!(def.metadata().get(&key), Some(&val));

    // set_mintable: flip mintability and verify
    assert_eq!(def.mintable(), Mintable::Infinitely);
    def.set_mintable(Mintable::Once);
    assert_eq!(def.mintable(), Mintable::Once);
    let limited = Mintable::limited_from_u32(2).expect("non-zero tokens");
    def.set_mintable(limited);
    assert!(matches!(def.mintable(), Mintable::Limited(tokens) if tokens.value() == 2));
    // Consuming once leaves one token and does not flip to Not
    assert!(!def.consume_mintability().expect("mintable"));
    assert!(matches!(def.mintable(), Mintable::Limited(tokens) if tokens.value() == 1));
    // Consuming again exhausts the budget and flips to Not
    assert!(def.consume_mintability().expect("mintable"));
    assert_eq!(def.mintable(), Mintable::Not);

    // set_owned_by: move ownership and verify
    let new_owner = checked_random_account_id();
    def.set_owned_by(new_owner.clone());
    assert_eq!(def.owned_by(), &new_owner);
}

#[test]
fn mintable_limited_rejects_zero_tokens() {
    assert!(Mintable::limited_from_u32(0).is_err());
}
