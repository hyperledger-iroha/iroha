//! Query error surface regression tests.
use integration_tests::sandbox;
use iroha::data_model::query::builder::SingleQueryError;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::gen_account_in;
#[test]
fn non_existent_account_is_specific_error() {
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(non_existent_account_is_specific_error),
    )
    .unwrap() else {
        return;
    };
    let client = network.client();
    let err = super::collections::accounts(&client)
        .unwrap()
        .into_iter()
        .find(|account| account.id() == &gen_account_in("regalia").0)
        .ok_or(SingleQueryError::<()>::ExpectedOneGotNone)
        .expect_err("Should error");
    match err {
        SingleQueryError::ExpectedOneGotNone => {}
        x => panic!("Unexpected error: {x:?}"),
    }
}
