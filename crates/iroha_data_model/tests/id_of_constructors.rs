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
    let domain: DomainId = "soramitsu".parse().unwrap();
    let name: Name = "xor".parse().unwrap();

    let parsed: AssetDefinitionId = "xor#soramitsu".parse().unwrap();
    let via_of = AssetDefinitionId::of(domain.clone(), name.clone());

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn asset_id_of_matches_parse() {
    let _guard = guard_chain_discriminant();
    let domain: DomainId = "wonderland".parse().unwrap();
    let kp = KeyPair::random();
    let account = AccountId::of(domain.clone(), kp.public_key().clone());
    let def: AssetDefinitionId = "rose#wonderland".parse().unwrap();

    let account_literal = format!("{}@{domain}", kp.public_key());
    let parsed: AssetId = format!("rose##{account_literal}").parse().unwrap();
    let via_of = AssetId::of(def, account);

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn nft_id_of_matches_parse() {
    let domain: DomainId = "art".parse().unwrap();
    let name: Name = "mona_lisa".parse().unwrap();

    let parsed: NftId = "mona_lisa$art".parse().unwrap();
    let via_of = NftId::of(domain, name);

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}

#[test]
fn account_id_of_matches_parse() {
    let _guard = guard_chain_discriminant();
    let domain: DomainId = "land".parse().unwrap();
    let kp = KeyPair::random();
    let parsed: AccountId = format!("{}@{domain}", kp.public_key()).parse().unwrap();
    let via_of = AccountId::of(domain, kp.public_key().clone());

    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}
