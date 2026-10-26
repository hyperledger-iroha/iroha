//! Selected discovery routes, credential separation and bounded faucet authorization.

use std::time::Duration;

use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

use super::*;
use crate::bootstrap::tests::{authenticated, metadata};

#[test]
fn selected_provisioning_fields_require_bls_endpoints_and_bounded_faucet_authority() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let mut metadata_record = metadata(&checkpoint);
    validate(&metadata_record).unwrap();
    let issuer = KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519);
    metadata_record.faucet = Some(FaucetPolicy {
        torii_root: metadata_record.torii_roots[0].clone(),
        issuer: AccountId::new(issuer.public_key().clone()),
        asset_definition_id: iroha_wallet::operations::XOR_ASSET_DEFINITION
            .parse()
            .unwrap(),
        amount: 100_u64.into(),
        max_operation_fee: 2_u64.into(),
        max_namespace_rent: 50_u64.into(),
    });
    metadata_record.build_registry = Some(BuildRegistryPolicy {
        torii_roots: vec![metadata_record.torii_roots[0].clone()],
    });
    validate(&metadata_record).unwrap();
    let bootstrap = authenticated(metadata_record.clone(), &checkpoint);
    assert_eq!(bootstrap.metadata(), &metadata_record);
    for roots in [
        vec![],
        vec!["https://foreign.example/".into()],
        vec![metadata_record.torii_roots[0].clone(); 2],
    ] {
        let mut invalid = metadata_record.clone();
        invalid.build_registry.as_mut().unwrap().torii_roots = roots;
        assert!(validate(&invalid).is_err());
    }
    let options = bootstrap
        .private_operation_options(Instant::now() + Duration::from_secs(5))
        .unwrap();
    let faucet = metadata_record.faucet.as_ref().unwrap();
    assert_eq!(
        options.max_total_fees.get(&faucet.asset_definition_id),
        Some(&faucet.max_operation_fee)
    );
    assert_eq!(options.max_total_fees.len(), 1);
    assert!(
        bootstrap
            .private_operation_options(Instant::now())
            .is_err()
    );
    for mutation in [0, 1, 2, 3, 4, 5, 6] {
        let mut changed = metadata_record.clone();
        match mutation {
            0 => changed.account_chain_discriminant = 0,
            1 => changed.peers.push(changed.peers[0].clone()),
            2 => changed.peers[0].torii_root = "https://foreign.example/".into(),
            3 => changed.peers[0].node_id = PeerId::new(issuer.public_key().clone()),
            4 => changed.faucet.as_mut().unwrap().amount = Quantity::zero(),
            5 => changed.faucet.as_mut().unwrap().max_operation_fee = 101_u64.into(),
            _ => changed.faucet.as_mut().unwrap().torii_root = "https://foreign.example/".into(),
        }
        assert!(validate(&changed).is_err());
    }
    let mut public_root_only = metadata_record;
    public_root_only.peers.clear();
    validate(&public_root_only).unwrap();

}

#[test]
fn automatic_parent_clients_use_only_selected_routes_and_never_forward_child_credentials() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = KeyPair::from_seed(vec![92; 32], Algorithm::Ed25519);
    let metadata_record = metadata(&checkpoint);
    let bootstrap = authenticated(metadata_record.clone(), &checkpoint);
    let config = Config::load_table(
        "public-parent.toml",
        toml::toml! {
            chain = (metadata_record.chain_id.clone())
            network_id = (metadata_record.network_id.to_string())
            torii_url = (metadata_record.torii_roots[0].as_str())
            [account]
            chain_discriminant = (metadata_record.account_chain_discriminant)
            public_key = (key.public_key().to_string())
            private_key = (ExposedPrivateKey(key.private_key().clone()).to_string())
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let network = bootstrap.wallet_network().unwrap();
    assert_eq!(network.network_id, config.network_id);
    assert_eq!(
        network.chain_discriminant,
        config.account_chain_discriminant
    );
    assert_eq!(network.torii_url, config.torii_api_url.as_str());
    assert!(bootstrap.private_operation_options(deadline).is_err());
    bootstrap.parent_http_source(&config, deadline).unwrap();
    let mut public_root_only = metadata_record.clone();
    public_root_only.peers.clear();
    let public_bootstrap = authenticated(public_root_only, &checkpoint);
    public_bootstrap.parent_http_source(&config, deadline).unwrap();
    let mut changed = config.clone();
    changed.api_token = Some(iroha::secrecy::SecretString::new(
        "private-child-token".into(),
    ));
    assert!(bootstrap.parent_http_source(&changed, deadline).is_err());
    let mut changed = config.clone();
    changed.torii_api_url = "https://foreign.example/".parse().unwrap();
    assert!(bootstrap.parent_http_source(&changed, deadline).is_err());
    let mut changed = config.clone();
    changed.account_chain_discriminant += 1;
    assert!(bootstrap.parent_http_source(&changed, deadline).is_err());
    assert!(
        bootstrap
            .parent_http_source(&config, Instant::now())
            .is_err()
    );
}
