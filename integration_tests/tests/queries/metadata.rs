//! Account collection metadata, exact account reads and missing-key predicates.
use integration_tests::sandbox;
use iroha::{
    collections::{Collection, ListQuery, field},
    data_model::prelude::*,
};
use iroha_executor_data_model::permission::account::CanRegisterAccount;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, BOB_ID, BOB_KEYPAIR};
use std::{collections::BTreeMap, str::FromStr};
#[derive(norito::JsonDeserialize)]
struct AccountMetadata {
    id: AccountId,
    metadata: Metadata,
}
#[test]
#[allow(clippy::too_many_lines)]
fn find_accounts_with_asset() {
    let result: eyre::Result<()> = (|| {
        let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
            NetworkBuilder::new(),
            stringify!(find_accounts_with_asset_metadata),
        )
        .unwrap() else {
            return Ok(());
        };
        let test_client = network.client();
        let wonderland_domain: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let key = Name::from_str("key").unwrap();
        let another_key = Name::from_str("another_key").unwrap();
        // Ensure Alice has the expected default metadata key used by assertions below.
        // Some environments may not preload this value via genesis.
        test_client
            .submit(
                SetKeyValue::account(
                    ALICE_ID.clone(),
                    key.clone(),
                    iroha_primitives::json::Json::new("value"),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .unwrap();
        // Ensure Bob account exists (defaults genesis may omit it).
        let read_accounts = || -> eyre::Result<Vec<AccountMetadata>> {
            test_client
                .list(Collection::Accounts, ListQuery::new().limit(64))
                .map(|row| norito::json::from_value(row?).map_err(Into::into))
                .collect()
        };
        let existing_accounts = read_accounts()?;
        let bob_missing = existing_accounts
            .iter()
            .all(|account| account.id != *BOB_ID);
        if bob_missing {
            test_client
                .submit(
                    Grant::account_permission(
                        CanRegisterAccount {
                            domain: wonderland_domain.clone(),
                        },
                        ALICE_ID.clone(),
                    ),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .expect("Failed to grant account registration permission to Alice");
            test_client
                .submit(
                    Register::account(Account::new(BOB_ID.clone())),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
                .expect("Failed to register Bob account");
        }
        let fee_asset = iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
        test_client.submit(
            Transfer::asset_quantity(
                AssetId::new(fee_asset, ALICE_ID.clone()),
                100u32,
                BOB_ID.clone(),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let bob_client = network
            .peers()
            .first()
            .expect("network has at least one peer")
            .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
        bob_client
            .submit(
                SetKeyValue::account(
                    BOB_ID.clone(),
                    key.clone(),
                    iroha_primitives::json::Json::new(
                        norito::json::object([(
                            "funny",
                            norito::json::to_value(&"value").expect("serialize"),
                        )])
                        .expect("serialize metadata object"),
                    ),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .unwrap();
        bob_client
            .submit(
                SetKeyValue::account(BOB_ID.clone(), another_key.clone(), "value"),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .unwrap();
        // we have the following configuration:
        //          key                 another_key
        // ALICE    "value"             -
        // BOB      {"funny": "value"}  "value"
        // check that bulk retrieval works as expected
        let key_values = read_accounts()?
            .into_iter()
            .filter(|account| account.id == *ALICE_ID || account.id == *BOB_ID)
            .map(|account| {
                (
                    account.id,
                    account.metadata.get(&key).cloned().expect("key must exist"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(key_values.len(), 2);
        let expected_alice = iroha_primitives::json::Json::from("value");
        assert_eq!(key_values.get(&*ALICE_ID), Some(&expected_alice));
        let expected_bob = iroha_primitives::json::Json::new(
            norito::json::object([(
                "funny",
                norito::json::to_value(&"value").expect("serialize"),
            )])
            .expect("serialize metadata object"),
        );
        assert_eq!(key_values.get(&*BOB_ID), Some(&expected_bob));
        // Native exact account reads return the metadata map. Absence is
        // asserted directly, without constructing a synthetic query error.
        let alice = test_client
            .client()
            .query_single(FindAccountById::new(ALICE_ID.clone()))?;
        assert_eq!(alice.id(), &*ALICE_ID);
        assert!(alice.metadata().get(&another_key).is_none());
        let bob = test_client
            .client()
            .query_single(FindAccountById::new(BOB_ID.clone()))?;
        assert_eq!(bob.id(), &*BOB_ID);
        assert_eq!(bob.metadata().get(&another_key), Some(&"value".into()));
        // The server evaluates the missing-key predicate across the complete
        // account collection: Alice must not match, while Bob must match once.
        let accounts = test_client
            .list(
                Collection::Accounts,
                ListQuery::new()
                    .limit(64)
                    .filter(field("metadata.another_key").eq("value")),
            )
            .map(|row| norito::json::from_value::<AccountMetadata>(row?).map_err(Into::into))
            .collect::<eyre::Result<Vec<_>>>()?;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, *BOB_ID);
        Ok(())
    })();
    let _ = sandbox::handle_result(result, stringify!(find_accounts_with_asset_metadata)).unwrap();
}
