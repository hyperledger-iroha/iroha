//! Ergonomic `of(...)` constructors for composite IDs.
//!
//! Verifies that `of(...)` produces the same identifiers as parsing
//! and that display formatting remains consistent.
use iroha_crypto::KeyPair;
use iroha_data_model::{account::address, prelude::*};
fn guard_chain_discriminant() -> address::ChainDiscriminantGuard {
    address::ChainDiscriminantGuard::enter(address::chain_discriminant())
}
fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked id-constructor keypair")
}
#[test]
fn asset_definition_component_derivation_matches_parse() {
    let domain: DomainId = DomainId::try_new("soramitsu", "universal").unwrap();
    let name: Name = "xor".parse().unwrap();
    let derived = AssetDefinitionId::derive_from_components(domain.clone(), name.clone());
    let parsed: AssetDefinitionId = derived.to_string().parse().unwrap();
    assert_eq!(parsed, derived);
    assert_eq!(format!("{parsed}"), format!("{derived}"));
}
#[test]
fn asset_id_of_matches_parse() {
    let _guard = guard_chain_discriminant();
    let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let kp = checked_random_keypair();
    let account = AccountId::of(kp.public_key().clone());
    let def = AssetDefinitionId::derive_from_components(domain, "rose".parse().unwrap());
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
    let kp = checked_random_keypair();
    let via_of = AccountId::of(kp.public_key().clone());
    let parsed = AccountId::parse_encoded(&via_of.to_string())
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("canonical AccountId literal parses");
    assert_eq!(parsed, via_of);
    assert_eq!(format!("{parsed}"), format!("{via_of}"));
}
