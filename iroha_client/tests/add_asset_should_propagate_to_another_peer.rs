#[cfg(test)]
mod tests {
    use async_std::task;
    use iroha::{config::Configuration, prelude::*};
    use iroha_client::{
        client::{self, Client},
        config::Configuration as ClientConfiguration,
    };
    use iroha_data_model::prelude::*;
    use std::thread;
    use tempfile::TempDir;

    const CONFIGURATION_PATH: &str = "tests/test_config.json";
    const N_PEERS: usize = 4;
    const MAX_FAULTS: usize = 1;

    #[test]
    //TODO: use cucumber_rust to write `gherkin` instead of code.
    fn client_add_asset_quantity_to_existing_asset_should_increase_asset_amount_on_another_peer() {
        // Given
        let configuration =
            Configuration::from_path(CONFIGURATION_PATH).expect("Failed to load configuration.");
        let peers = create_and_start_iroha_peers(N_PEERS);
        thread::sleep(std::time::Duration::from_millis(1000));
        let domain_name = "domain";
        let create_domain = Register::<Peer, Domain>::new(
            Domain::new(domain_name),
            PeerId::new(
                &configuration.torii_configuration.torii_p2p_url,
                &configuration.public_key,
            ),
        );
        let account_name = "account";
        let account_id = AccountId::new(account_name, domain_name);
        let (public_key, _) = configuration.key_pair();
        let create_account = Register::<Domain, Account>::new(
            Account::with_signatory(account_id.clone(), public_key),
            domain_name.to_string(),
        );
        let asset_id = AssetDefinitionId::new("xor", domain_name);
        let create_asset = Register::<Domain, AssetDefinition>::new(
            AssetDefinition::new(asset_id.clone()),
            domain_name.to_string(),
        );
        let mut client_configuration = ClientConfiguration::from_path(CONFIGURATION_PATH)
            .expect("Failed to load configuration.");
        client_configuration.torii_api_url =
            peers.first().expect("Failed to get first peer.").clone();
        let mut iroha_client = Client::new(&client_configuration);
        iroha_client
            .submit_all(vec![
                create_domain.into(),
                create_account.into(),
                create_asset.into(),
            ])
            .expect("Failed to prepare state.");
        thread::sleep(std::time::Duration::from_millis(
            configuration.sumeragi_configuration.pipeline_time_ms() * 2,
        ));
        //When
        let quantity: u32 = 200;
        let mint_asset =
            Mint::<Asset, u32>::new(quantity, AssetId::new(asset_id, account_id.clone()));
        iroha_client
            .submit(mint_asset.into())
            .expect("Failed to create asset.");
        thread::sleep(std::time::Duration::from_millis(
            configuration.sumeragi_configuration.pipeline_time_ms() * 2,
        ));
        //Then
        client_configuration.torii_api_url =
            peers.last().expect("Failed to get last peer.").clone();
        let mut iroha_client = Client::new(&client_configuration);
        let request = client::asset::by_account_id(account_id);
        let query_result = iroha_client
            .request(&request)
            .expect("Failed to execute request.");
        if let QueryResult::FindAssetsByAccountId(result) = query_result {
            assert!(!result.assets.is_empty());
            assert_eq!(
                quantity,
                result.assets.first().expect("Asset should exist.").quantity,
            );
        } else {
            panic!("Wrong Query Result Type.");
        }
    }

    fn create_and_start_iroha_peers(n_peers: usize) -> Vec<String> {
        let peer_keys: Vec<KeyPair> = (0..n_peers)
            .map(|_| KeyPair::generate().expect("Failed to generate key pair."))
            .collect();
        let addresses: Vec<(String, String, String)> = (0..n_peers)
            .map(|i| {
                (
                    format!("127.0.0.1:{}", 7878 + i * 3),
                    format!("127.0.0.1:{}", 7878 + i * 3 + 1),
                    format!("127.0.0.1:{}", 7878 + i * 3 + 2),
                )
            })
            .collect();
        let peer_ids: Vec<PeerId> = peer_keys
            .iter()
            .enumerate()
            .map(|(i, key_pair)| {
                let (p2p_address, _, _) = &addresses[i];
                PeerId {
                    address: p2p_address.clone(),
                    public_key: key_pair.public_key.clone(),
                }
            })
            .collect();
        for i in 0..n_peers {
            let peer_ids = peer_ids.clone();
            let peer_id = peer_ids[i].clone();
            let key_pair = peer_keys[i].clone();
            let (p2p_address, api_address, connect_address) = addresses[i].clone();
            task::spawn(async move {
                let temp_dir = TempDir::new().expect("Failed to create TempDir.");
                let mut configuration = Configuration::from_path(CONFIGURATION_PATH)
                    .expect("Failed to load configuration.");
                configuration.sumeragi_configuration.key_pair = key_pair.clone();
                configuration.sumeragi_configuration.peer_id = peer_id.clone();
                configuration
                    .kura_configuration
                    .kura_block_store_path(temp_dir.path());
                configuration.torii_configuration.torii_p2p_url = p2p_address.clone();
                configuration.torii_configuration.torii_api_url = api_address.clone();
                configuration.torii_configuration.torii_connect_url = connect_address.clone();
                configuration.public_key = key_pair.public_key;
                configuration.private_key = key_pair.private_key.clone();
                configuration
                    .sumeragi_configuration
                    .trusted_peers(peer_ids.clone());
                configuration
                    .sumeragi_configuration
                    .max_faulty_peers(MAX_FAULTS);
                let iroha = Iroha::new(configuration);
                iroha.start().await.expect("Failed to start Iroha.");
                //Prevents temp_dir from clean up untill the end of the tests.
                #[allow(clippy::empty_loop)]
                loop {}
            });
            thread::sleep(std::time::Duration::from_millis(100));
        }
        addresses
            .iter()
            .map(|(_, api_url, _)| api_url)
            .cloned()
            .collect()
    }
}
