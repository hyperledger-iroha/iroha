//! Ergonomic `of(...)` constructors for composite IDs.
//!
//! Verifies that `of(...)` produces the same identifiers as parsing
//! and that display formatting remains consistent.

use iroha_crypto::KeyPair;
use iroha_data_model::{account::address, prelude::*};

fn guard_chain_discriminant() -> address::ChainDiscriminantGuard {
    address::ChainDiscriminantGuard::enter(address::chain_discriminant())
}

#[test]
fn asset_definition_id_of_matches_parse() {
    let domain: DomainId = DomainId::try_new("soramitsu", "universal").unwrap();
    let name: Name = "xor".parse().unwrap();

    let via_of = AssetDefinitionId::of(domain.clone(), name.clone());
    let parsed: AssetDefinitionId = via_of.to_string().parse().unwrap();

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn asset_id_of_matches_parse() {
    let _guard = guard_chain_discriminant();
    let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let kp = KeyPair::random();
    let account = AccountId::of(kp.public_key().clone());
    let def = AssetDefinitionId::of(domain, "rose".parse().unwrap());
    let via_of = AssetId::of(def.clone(), account);

    let parsed: AssetId = via_of.to_string().parse().unwrap();

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn nft_id_of_matches_parse() {
    let domain: DomainId = DomainId::try_new("art", "universal").unwrap();
    let name: Name = "mona_lisa".parse().unwrap();

    let via_of = NftId::of(domain, name);
    let parsed: NftId = via_of.to_string().parse().unwrap();

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn account_id_of_matches_parse() {
    let _guard = guard_chain_discriminant();
    let kp = KeyPair::random();
    let via_of = AccountId::of(kp.public_key().clone());
    let parsed = AccountId::parse_encoded(&via_of.to_string())
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("canonical AccountId literal parses");

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}
