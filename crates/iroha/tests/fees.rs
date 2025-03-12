use eyre::Result;
use fees_executor_data_model::parameters::*;
use iroha::data_model::prelude::*;
use iroha_executor_data_model::parameter::Parameter as _;
use iroha_test_network::*;
mod upgrade;
use upgrade::upgrade_executor;

#[test]
fn fees_options_in_updated_executor() -> Result<()> {
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

#[test]
fn fees_options_cannot_change() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();
    upgrade_executor(&test_client, "fees_executor")?;

    let err = test_client
        .submit_blocking(SetParameter::new(Parameter::Custom(
            FeesOptions::default().into(),
        )))
        .expect_err("Isi was not rejected");

    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .expect("Error {err} is not TransactionRejectionReason");

    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));

    Ok(())
}

#[test]
fn technical_domain_cannot_unregister() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();
    upgrade_executor(&test_client, "fees_executor")?;

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    let err = test_client
        .submit_blocking(Unregister::domain(
            fees_options.asset.account().domain().clone(),
        ))
        .expect_err("Isi was not rejected");

    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .expect("Error {err} is not TransactionRejectionReason");

    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));

    Ok(())
}

#[test]
fn technical_asset_definition_cannot_unregister() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;

    let test_client = network.client();
    upgrade_executor(&test_client, "fees_executor")?;

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    let err = test_client
        .submit_blocking(Unregister::asset_definition(
            fees_options.asset.definition().clone(),
        ))
        .expect_err("Isi was not rejected");

    let rejection_reason = err
        .downcast_ref::<TransactionRejectionReason>()
        .expect("Error {err} is not TransactionRejectionReason");

    assert!(matches!(
        rejection_reason,
        &TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_))
    ));

    Ok(())
}
