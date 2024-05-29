use std::thread;

use eyre::Result;
use iroha::{client, data_model::prelude::*};
use iroha_config::parameters::actual::Root as Config;
use test_network::*;

#[test]
// This test suite is also covered at the UI level in the iroha_cli tests
// in test_register_domains.py
fn client_add_domain_with_name_length_more_than_limit_should_not_commit_transaction() -> Result<()>
{
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_500).start_with_runtime();
    wait_for_genesis_committed(&vec![test_client.clone()], 0);
    let pipeline_time = Config::pipeline_time();

    // Given

    let normal_domain_id: DomainId = "sora".parse()?;
    let create_domain = Register::domain(Domain::new(normal_domain_id.clone()));
    test_client.submit(create_domain)?;

    let too_long_domain_name: DomainId = "0".repeat(2_usize.pow(14)).parse()?;
    let create_domain = Register::domain(Domain::new(too_long_domain_name.clone()));
    test_client.submit(create_domain)?;

    thread::sleep(pipeline_time * 2);

    assert!(test_client
        .request(client::domain::by_id(normal_domain_id))
        .is_ok());
    assert!(test_client
        .request(client::domain::by_id(too_long_domain_name))
        .is_err());

    Ok(())
}
