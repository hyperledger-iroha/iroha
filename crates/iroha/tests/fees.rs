use eyre::Result;
use fees_executor_data_model::{isi::*, parameters::*};
use iroha::data_model::prelude::*;
use iroha_data_model::parameter::CustomParameter;
use iroha_executor_data_model::parameter::Parameter as _;
use iroha_test_network::*;
mod upgrade;
use iroha_test_samples::{gen_account_in, BOB_ID, BOB_KEYPAIR};
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

#[test]
fn fees_cannot_update_by_others() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    upgrade_executor(&network.client(), "fees_executor")?;

    // Bob doesn't have `CanModifyFeesOptions` permission
    let test_client = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    let old_amounts = fees_options.amounts;
    let mut new_amounts = old_amounts.clone();
    new_amounts.fixed = new_amounts.fixed.checked_add(100_u32.into()).unwrap();

    let err = test_client
        .submit_blocking(SetDefaultFeesAmountsOptions(new_amounts.clone()))
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
fn fees_can_update_by_owner() -> Result<()> {
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

    let old_amounts = fees_options.amounts;
    let mut new_amounts = old_amounts.clone();
    new_amounts.fixed = new_amounts.fixed.checked_add(100_u32.into()).unwrap();

    test_client.submit_blocking(SetDefaultFeesAmountsOptions(new_amounts.clone()))?;

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(fees_options.amounts, new_amounts);

    Ok(())
}

#[test]
fn fees_options_are_default_for_new_accounts() -> Result<()> {
    let (network, _rt) = NetworkBuilder::new().start_blocking()?;
    upgrade_executor(&network.client(), "fees_executor")?;

    let test_client = network.client();

    let parameters = test_client.query_single(FindParameters)?;
    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .unwrap();

    let old_default_amounts = fees_options.amounts;

    let (user_a, _) = gen_account_in("wonderland");
    test_client.submit_blocking(Register::account(Account::new(user_a.clone())))?;
    let user_a_metadata = test_client
        .query(FindAccounts::new())
        .filter_with(|acc| acc.id.eq(user_a.clone()))
        .execute_single()?
        .metadata()
        .clone();
    let user_a_fees = user_a_metadata
        .get(FeesAmountsOptions::id().name())
        .expect("fee record should be present in metadata");

    assert_eq!(
        Into::<CustomParameter>::into(old_default_amounts.clone()).payload(),
        user_a_fees
    );

    let mut new_default_amounts = old_default_amounts.clone();
    new_default_amounts.fixed = new_default_amounts
        .fixed
        .checked_add(100_u32.into())
        .unwrap();
    test_client.submit_blocking(SetDefaultFeesAmountsOptions(new_default_amounts.clone()))?;

    let (user_b, _) = gen_account_in("wonderland");
    test_client.submit_blocking(Register::account(Account::new(user_b.clone())))?;
    let user_b_metadata = test_client
        .query(FindAccounts::new())
        .filter_with(|acc| acc.id.eq(user_b.clone()))
        .execute_single()?
        .metadata()
        .clone();
    let user_b_fees = user_b_metadata
        .get(FeesAmountsOptions::id().name())
        .expect("fee record should be present in metadata");

    let user_a_metadata = test_client
        .query(FindAccounts::new())
        .filter_with(|acc| acc.id.eq(user_a.clone()))
        .execute_single()?
        .metadata()
        .clone();
    let user_a_fees = user_a_metadata
        .get(FeesAmountsOptions::id().name())
        .expect("fee record should be present in metadata");

    assert_eq!(
        Into::<CustomParameter>::into(old_default_amounts.clone()).payload(),
        user_a_fees
    );
    assert_eq!(
        Into::<CustomParameter>::into(new_default_amounts.clone()).payload(),
        user_b_fees
    );

    Ok(())
}
