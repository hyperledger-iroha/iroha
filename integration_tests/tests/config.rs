use iroha_config::client_api::{ConfigUpdateDTO, Logger};
use iroha_data_model::Level;
use iroha_test_network::NetworkBuilder;
use nonzero_ext::nonzero;

#[tokio::test]
async fn update_forbidden() -> eyre::Result<()> {
    let network = NetworkBuilder::new().start().await?;

    let update = ConfigUpdateDTO {
        logger: Logger {
            level: Level::ERROR,
            filter: Some("iroha_p2p=trace".parse()?),
        },
    };
    let mut url = network.client().torii_url;
    url.path_segments_mut().unwrap().push("configuration");
    println!("{url}");
    let resp = reqwest::Client::new()
        .post(url)
        .json(&update)
        .send()
        .await?;
    assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

    Ok(())
}

#[test]
fn retrieve_update_config() -> eyre::Result<()> {
    let (network, _rt) = NetworkBuilder::new()
        .with_config(|c| {
            c.write(["network", "block_gossip_size"], 100)
                .write(["queue", "capacity"], 100_000)
                .write(["torii", "allow_config_update"], true);
        })
        .start_blocking()?;
    let client = network.client();

    let config = client.get_config()?;

    assert_eq!(config.network.block_gossip_size, nonzero!(100u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));
    assert_eq!(config.logger.level, Level::DEBUG);
    assert_eq!(config.logger.filter, None);

    client.set_config(&ConfigUpdateDTO {
        logger: Logger {
            level: Level::ERROR,
            filter: Some("iroha_p2p=trace".parse()?),
        },
    })?;

    let config = client.get_config()?;

    assert_eq!(config.network.block_gossip_size, nonzero!(100u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));
    assert_eq!(config.logger.level, Level::ERROR);
    assert_eq!(config.logger.filter, Some("iroha_p2p=trace".parse()?),);

    Ok(())
}
