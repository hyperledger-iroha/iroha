//! Query error surface regression tests.
use integration_tests::sandbox;
use iroha::{
    data_model::{prelude::FindAccountById, query::error::FindError},
    query::QueryError,
};
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
    let missing = gen_account_in("regalia").0;
    let err = client
        .client()
        .query_single(FindAccountById::new(missing.clone()))
        .expect_err("missing account must return a query error");
    match err {
        QueryError::Http {
            status,
            code,
            message,
        } => {
            assert_eq!(status, 404);
            assert_eq!(code, "query_validation_failed");
            assert_eq!(message, FindError::Account(missing).to_string());
        }
        other => panic!("unexpected missing-account error: {other:?}"),
    }
}
