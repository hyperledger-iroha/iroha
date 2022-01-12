#![allow(missing_docs)]
use std::{thread, time::Duration};

use iroha_data_model::prelude::*;
use test_network::{wait_for_genesis_committed, Peer as TestPeer};

fn create_million_accounts_directly() {
    let (_rt, _peer, mut test_client) = <TestPeer>::start_test_with_runtime();
    wait_for_genesis_committed(vec![test_client.clone()], 0);
    for i in 0_u32..10_000_000_u32 {
        let domain_name = format!("wonderland-{}", i);
        let normal_account_id = AccountId::test(&format!("bob-{}", i), &domain_name);
        let create_domain = RegisterBox::new(IdentifiableBox::from(Domain::test(&domain_name)));
        let create_account = RegisterBox::new(IdentifiableBox::from(NewAccount::new(
            normal_account_id.clone(),
        )));
        if test_client
            .submit_all([create_domain.into(), create_account.into()].to_vec())
            .is_err()
        {
            thread::sleep(Duration::from_millis(15000));
        }
    }
    thread::sleep(Duration::from_secs(1000));
}

fn main() {
    create_million_accounts_directly();
}
