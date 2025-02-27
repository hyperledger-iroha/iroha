use eyre::Result;
use fees_executor_data_model::parameters::*;
use iroha::data_model::prelude::*;
use iroha_executor_data_model::parameter::Parameter as _;
use iroha_test_network::*;
mod upgrade;
use upgrade::upgrade_executor;

#[test]
fn fees_example_test() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();
    // Also sets enough fuel based on the profile
    upgrade_executor(&test_client, "fees_executor")?;

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(fees_options, FeesOptions::default());

    Ok(())
}
