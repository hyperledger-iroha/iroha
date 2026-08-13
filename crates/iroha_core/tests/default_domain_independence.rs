//! Regression guards for domainless universal accounts and explicit domain state.
use std::{fs, path::Path};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    domain::DomainId,
};
#[test]
fn universal_account_address_is_independent_of_explicit_domain_context() {
    let key_pair = KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Ed25519)
        .expect("valid universal-account fixture key");
    let account = AccountId::new(key_pair.public_key().clone());
    let address = AccountAddress::from_account_id(&account).expect("canonical account address");
    let canonical = address.canonical_hex().expect("canonical address bytes");
    for domain in [
        DomainId::try_new("default", "universal").expect("explicit default-named domain"),
        DomainId::try_new("merchant", "retail").expect("explicit routed domain"),
    ] {
        address
            .ensure_domain_matches(&domain)
            .expect("a universal account address has no domain affinity");
        assert_eq!(
            address.canonical_hex().expect("canonical address bytes"),
            canonical,
        );
    }
}
#[test]
fn process_default_domain_cannot_reenter_validation_or_world_state() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("iroha_core is nested under crates");
    let sources = [
        "iroha_data_model/src/account/address.rs",
        "iroha_config/src/parameters/user.rs",
        "iroha_config/src/parameters/actual.rs",
        "iroha_core/src/state.rs",
        "iroha_core/src/alias_setup.rs",
        "iroha_core/src/smartcontracts/isi/domain.rs",
        "iroha_core/src/smartcontracts/isi/sns.rs",
        "iroha_core/src/smartcontracts/isi/world.rs",
        "iroha_torii/src/lib.rs",
        "irohad/src/main.rs",
    ];
    let forbidden = [
        "AccountDomainSelector",
        "domain_selectors",
        "default_account_domain_label",
        "set_default_domain_name",
        "set_default_account_domain_label",
        "pub fn domain_selector",
    ];
    for relative in sources {
        let source = fs::read_to_string(crates_dir.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        for symbol in forbidden {
            assert!(
                !source.contains(symbol),
                "{relative} must not contain the retired default-domain state sink `{symbol}`",
            );
        }
    }
}
