#![allow(clippy::module_inception, unused_results, clippy::restriction)]

#[cfg(test)]
mod tests {
    use std::thread;

    use iroha::config::Configuration;
    use iroha_client::client;
    use iroha_data_model::prelude::*;
    use test_network::*;
    use tokio::runtime::Runtime;

    #[test]
    fn genesis_block_is_commited_with_some_offline_peers() {
        // Given
        let rt = Runtime::test();
        let (_network, mut iroha_client) = rt.block_on(<Network>::start_test_with_offline(4, 1, 1));
        let pipeline_time = Configuration::pipeline_time();

        thread::sleep(pipeline_time * 8);

        //When
        let alice_id = AccountId::new("alice", "wonderland");
        let alice_has_roses = 13;

        //Then
        let assets = iroha_client
            .request(client::asset::by_account_id(alice_id))
            .expect("Failed to execute request.");
        let asset = assets
            .iter()
            .find(|asset| asset.id.definition_id == AssetDefinitionId::new("rose", "wonderland"))
            .unwrap();
        assert_eq!(AssetValue::Quantity(alice_has_roses), asset.value);
    }
}
