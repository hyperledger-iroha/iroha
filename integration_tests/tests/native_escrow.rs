//! Native asset escrow integration scenarios.
use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    data_model::{
        isi::escrow::{
            AcceptAssetEscrow, CancelAssetLock, DrawdownAssetLock, ExpireAssetLock,
            MarkEscrowPaymentSent, OpenAssetEscrow, OpenAssetLock, ReleaseAssetEscrow,
        },
        prelude::*,
    },
    query::QueryError,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    escrow::scoped::AssetEscrowScopeOriginalV1,
    nexus::AxtAssetIncarnationV1,
    query::{asset_escrow_scope::FindAssetEscrowsByAssetDefinitionV1, error::QueryExecutionFail},
};
use iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition;
use iroha_model_base::domain::DomainId;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use std::time::{Duration, Instant};
fn require_rejection_cause(error: &eyre::Report, expected: &str) -> Result<()> {
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(expected)),
        "expected native rejection {expected:?}, got {error:?}"
    );
    Ok(())
}
#[test]
fn native_rejection_checks_do_not_accept_unrelated_sdk_failures() -> Result<()> {
    let native = eyre!("direct debit from native escrow custody account is not allowed")
        .wrap_err("transaction rejected");
    require_rejection_cause(&native, "direct debit from native escrow custody account")?;
    assert!(
        require_rejection_cause(
            &eyre!("fee quote failed"),
            "direct debit from native escrow custody account"
        )
        .is_err()
    );
    Ok(())
}
fn wait_for_escrow_status(
    client: &Client,
    escrow_id: EscrowId,
    expected: AssetEscrowStatus,
    context: &str,
) -> Result<AssetEscrowRecord> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "escrow was not queried".to_owned();
    while Instant::now() < deadline {
        match client
            .client()
            .query_single(FindAssetEscrowById::new(escrow_id))
        {
            Ok(record) => {
                last_observed = format!("{:?}", record.status);
                if record.status == expected {
                    return Ok(record);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for escrow {escrow_id:?} to become {expected:?} after {context}; last_observed={last_observed}"
    ))
}
fn wait_for_asset_value(
    client: &Client,
    asset_id: &AssetId,
    expected: &Quantity,
    context: &str,
) -> Result<Asset> {
    const POLL_INTERVAL: Duration = Duration::from_millis(100);
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    let mut last_observed = "asset was not queried".to_owned();
    while Instant::now() < deadline {
        match client
            .client()
            .query_single(FindAssetById::new(asset_id.clone()))
        {
            Ok(asset) => {
                last_observed = format!("{}", asset.value());
                if asset.value() == expected {
                    return Ok(asset);
                }
            }
            Err(err) => {
                last_observed = format!("query failed: {err}");
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(eyre!(
        "timed out waiting for asset {asset_id} to equal {expected} after {context}; last_observed={last_observed}"
    ))
}
fn registration_incarnation(
    client: &Client,
    network_id: &iroha_data_model::NetworkId,
    definition: &AssetDefinitionId,
    registration_hash: HashOf<SignedTransaction>,
) -> Result<AxtAssetIncarnationV1> {
    let status = client
        .client()
        .get_transaction_status_response(registration_hash)?
        .ok_or_else(|| eyre!("finalized asset registration is missing"))?;
    ensure!(
        status.status.kind == "Applied",
        "asset registration is not applied"
    );
    let height = status
        .status
        .block_height
        .and_then(std::num::NonZeroU64::new)
        .ok_or_else(|| eyre!("asset registration has no canonical block height"))?;
    let account = iroha::blocking::AccountClient::from_client(client.account_client().clone())?;
    let mut stream = account.blocks().subscribe(height)?;
    let block = stream
        .recv(Some(Duration::from_secs(30)))?
        .ok_or_else(|| eyre!("registration block stream ended"))?;
    stream.close()?;
    ensure!(
        block.header().height() == height,
        "registration block height changed"
    );
    let transaction = block
        .external_transactions()
        .find(|tx| tx.hash() == registration_hash)
        .ok_or_else(|| eyre!("registration transaction missing from selected canonical block"))?;
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(eyre!("fixture registration must be explicit instructions"));
    };
    let mut found = false;
    for instruction in instructions {
        match instruction.as_any().downcast_ref::<RegisterBox>() {
            Some(RegisterBox::Account(_)) if !found => (),
            Some(RegisterBox::AssetDefinition(register))
                if !found && &register.object.id == definition =>
            {
                found = true;
            }
            _ if found => (),
            _ => {
                return Err(eyre!(
                    "unexpected lifecycle effect before fixture asset registration"
                ));
            }
        }
    }
    ensure!(
        found,
        "selected transaction did not register the exact fixture asset"
    );
    // This closed fixture prefix contains only account registration before the
    // single asset registration, so the asset lifecycle ordinal is exactly zero.
    Ok(AxtAssetIncarnationV1::derive(
        network_id,
        definition,
        &block.header().hash(),
        &Hash::from(transaction.hash_as_entrypoint()),
        0,
    ))
}
fn is_parallel_query_admission(error: &QueryError) -> bool {
    matches!(error, QueryError::Http { status, code, message }
        if *status == reqwest::StatusCode::TOO_MANY_REQUESTS
            && code == "query_validation_failed"
            && message == &QueryExecutionFail::CapacityLimit.to_string())
}
#[test]
fn scope_retry_requires_exact_parallel_query_admission() {
    let refusal = |status, code: &str, message: String| QueryError::Http {
        status,
        code: code.into(),
        message,
    };
    assert!(is_parallel_query_admission(&refusal(
        reqwest::StatusCode::TOO_MANY_REQUESTS,
        "query_validation_failed",
        QueryExecutionFail::CapacityLimit.to_string(),
    )));
    for error in [
        refusal(
            reqwest::StatusCode::BAD_REQUEST,
            "query_validation_failed",
            QueryExecutionFail::CapacityLimit.to_string(),
        ),
        refusal(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "query_validation_failed",
            "permanent validation failure".into(),
        ),
        refusal(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "different_code",
            QueryExecutionFail::CapacityLimit.to_string(),
        ),
        QueryError::Other(eyre!("429 Too Many Requests: parallel queries")),
    ] {
        assert!(!is_parallel_query_admission(&error));
    }
}
fn read_escrow_scope_before(
    client: &Client,
    definition: &AssetDefinitionId,
    incarnation: AxtAssetIncarnationV1,
    deadline: Instant,
) -> Result<AssetEscrowScopeOriginalV1> {
    // Bind all attempts, including compatibility checks and in-flight HTTP, to
    // the caller's original deadline; only native admission pressure is retried.
    let bounded = client.client().with_request_deadline(deadline);
    let mut last_transient = None;
    while Instant::now() < deadline {
        match bounded.get_asset_escrow_scope_original(
            FindAssetEscrowsByAssetDefinitionV1 {
                asset_definition_id: definition.clone(),
                expected_incarnation: incarnation,
            },
            1024 * 1024,
        ) {
            Ok(original) => return Ok(original),
            Err(error) if is_parallel_query_admission(&error) => last_transient = Some(error),
            Err(error) => {
                let error = eyre::Report::from(error);
                return Err(match last_transient {
                    Some(last) if Instant::now() >= deadline => error.wrap_err(format!(
                        "escrow scope deadline exhausted; last native admission refusal: {last}"
                    )),
                    _ => error,
                });
            }
        }
        std::thread::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    Err(match last_transient {
        Some(error) => eyre::Report::from(error).wrap_err("escrow scope deadline exhausted"),
        None => eyre!("escrow scope deadline exhausted before a request"),
    })
}
fn wait_for_buyer_escrow(
    client: &Client,
    definition: &AssetDefinitionId,
    incarnation: AxtAssetIncarnationV1,
    buyer: &AccountId,
    escrow_id: EscrowId,
    context: &str,
) -> Result<Vec<AssetEscrowRecord>> {
    const TIMEOUT: Duration = Duration::from_secs(30);
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        // The former unrestricted buyer listing is retired. Read the complete,
        // bounded native asset scope as its actual owner and preserve the buyer
        // association assertion. Core's escrow tests separately cover its buyer index.
        let original = read_escrow_scope_before(client, definition, incarnation, deadline)?;
        if let Some(record) = original
            .records
            .iter()
            .find(|record| record.id == escrow_id)
        {
            ensure!(
                record.buyer.as_ref() == Some(buyer),
                "escrow scope changed buyer"
            );
            return Ok(original.records);
        }
        std::thread::sleep(
            Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    Err(eyre!(
        "timed out waiting for complete native escrow scope after {context}"
    ))
}

#[test]
fn native_asset_escrow_aitai_flow_on_multi_peer_network() -> Result<()> {
    let context = stringify!(native_asset_escrow_aitai_flow_on_multi_peer_network);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(NetworkBuilder::new().with_min_peers(4), context)
            .unwrap()
    else {
        return Ok(());
    };
    let client = network.client();
    let result: Result<()> = (|| {
        let seller = ALICE_ID.clone();
        let (buyer, buyer_keypair) = gen_account_in("wonderland");
        let buyer_client = network
            .peers()
            .first()
            .expect("four-validator network")
            .client_for(&buyer, buyer_keypair.private_key().clone());
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "aitai_xor_native".parse()?,
        );
        let seller_asset_id = AssetId::of(asset_definition_id.clone(), seller.clone());
        let fee_asset_definition: AssetDefinitionId =
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
        let registration_hash = client.submit_all(
            [
                InstructionBox::from(Register::account(Account::new(buyer.clone()))),
                // Independently signed escrow actions pay ordinary transaction fees.
                Transfer::asset_quantity(
                    AssetId::of(fee_asset_definition, seller.clone()),
                    100_000_u32,
                    buyer.clone(),
                )
                .into(),
                Register::asset_definition(AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    "aitai_xor_native".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(DomainId::try_new("wonderland", "universal")?),
                ))
                .into(),
                Mint::asset_quantity(100_u64, seller_asset_id.clone()).into(),
                Transfer::asset_quantity(
                    AssetId::of(
                        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
                        seller.clone(),
                    ),
                    100_u32,
                    buyer.clone(),
                )
                .into(),
            ],
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let incarnation = registration_incarnation(
            &client,
            &network.network_id(),
            &asset_definition_id,
            registration_hash,
        )?;
        client.submit(
            iroha_data_model::isi::asset_escrow_scope::InitializeAssetEscrowReadScopeV1 {
                asset_definition_id: asset_definition_id.clone(),
                expected_incarnation: incarnation,
                expected_revision: 0,
            },
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let empty_scope = read_escrow_scope_before(
            &client,
            &asset_definition_id,
            incarnation,
            Instant::now() + Duration::from_secs(30),
        )?;
        assert!(empty_scope.records.is_empty());
        let escrow_id = EscrowId::new(Hash::new("native-aitai-flow"));
        client.submit(
            OpenAssetEscrow::with_evidence_hashes(
                escrow_id,
                asset_definition_id.clone(),
                Quantity::from(40_u64),
                vec![Hash::new("fiat-invoice")],
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let opened =
            wait_for_escrow_status(&client, escrow_id, AssetEscrowStatus::Open, "open escrow")?;
        let custody_asset_id = AssetId::of(asset_definition_id.clone(), opened.custody.clone());
        client.submit(
            Grant::account_permission(
                Permission::from(CanTransferAssetWithDefinition {
                    asset_definition: asset_definition_id.clone(),
                }),
                seller.clone(),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let denied = client
            .submit(
                Transfer::asset_quantity(custody_asset_id, 1_u64, seller.clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err(
                "active native escrow custody must not be drainable through generic transfer",
            );
        require_rejection_cause(&denied, "direct debit from native escrow custody account")?;
        buyer_client.submit(
            AcceptAssetEscrow::new(escrow_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        buyer_client.submit(
            MarkEscrowPaymentSent::new(escrow_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        client.submit(
            ReleaseAssetEscrow::new(escrow_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let released = wait_for_escrow_status(
            &client,
            escrow_id,
            AssetEscrowStatus::Released,
            "release escrow",
        )?;
        assert_eq!(released.buyer, Some(buyer.clone()));
        let buyer_asset_id = AssetId::of(asset_definition_id.clone(), buyer.clone());
        let buyer_asset = wait_for_asset_value(
            &client,
            &buyer_asset_id,
            &Quantity::from(40_u64),
            "release escrow buyer balance",
        )?;
        let seller_asset = wait_for_asset_value(
            &client,
            &seller_asset_id,
            &Quantity::from(60_u64),
            "release escrow seller balance",
        )?;
        assert_eq!(*buyer_asset.value(), Quantity::from(40_u64));
        assert_eq!(*seller_asset.value(), Quantity::from(60_u64));
        let buyer_escrows = wait_for_buyer_escrow(
            &client,
            &asset_definition_id,
            incarnation,
            released.buyer.as_ref().expect("buyer recorded"),
            escrow_id,
            "release escrow complete asset scope",
        )?;
        assert!(buyer_escrows.iter().any(|record| record.id == escrow_id));
        assert_eq!(
            buyer_escrows.len(),
            1,
            "complete scope has exactly one escrow"
        );
        Ok(())
    })();
    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
#[test]
fn native_asset_lock_flow_on_multi_peer_network() -> Result<()> {
    let context = stringify!(native_asset_lock_flow_on_multi_peer_network);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(NetworkBuilder::new().with_min_peers(4), context)
            .unwrap()
    else {
        return Ok(());
    };
    let client = network.client();
    let result: Result<()> = (|| {
        let source = ALICE_ID.clone();
        let (destination, destination_keypair) = gen_account_in("wonderland");
        let (release_authority, release_authority_keypair) = gen_account_in("wonderland");
        let destination_client = network
            .peers()
            .first()
            .expect("four-validator network")
            .client_for(&destination, destination_keypair.private_key().clone());
        let release_authority_client = network
            .peers()
            .first()
            .expect("four-validator network")
            .client_for(
                &release_authority,
                release_authority_keypair.private_key().clone(),
            );
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal")?,
            "aitai_xor_lock_native".parse()?,
        );
        let source_asset_id = AssetId::of(asset_definition_id.clone(), source.clone());
        let fee_asset_definition: AssetDefinitionId =
            iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?;
        client.submit_all(
            [
                InstructionBox::from(Register::account(Account::new(destination.clone()))),
                // Independently signed escrow actions pay ordinary transaction fees.
                Transfer::asset_quantity(
                    AssetId::of(fee_asset_definition.clone(), source.clone()),
                    100_000_u32,
                    destination.clone(),
                )
                .into(),
                InstructionBox::from(Register::account(Account::new(release_authority.clone()))),
                Transfer::asset_quantity(
                    AssetId::of(fee_asset_definition, source.clone()),
                    100_000_u32,
                    release_authority.clone(),
                )
                .into(),
                Register::asset_definition(AssetDefinition::numeric(
                    asset_definition_id.clone(),
                    "aitai_xor_lock_native".to_owned(),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    Some(DomainId::try_new("wonderland", "universal")?),
                ))
                .into(),
                Mint::asset_quantity(100_u64, source_asset_id.clone()).into(),
                Transfer::asset_quantity(
                    AssetId::of(
                        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
                        source.clone(),
                    ),
                    100_u32,
                    destination.clone(),
                )
                .into(),
                Transfer::asset_quantity(
                    AssetId::of(
                        iroha_config::parameters::defaults::nexus::fees::fee_asset_id().parse()?,
                        source.clone(),
                    ),
                    100_u32,
                    release_authority.clone(),
                )
                .into(),
            ],
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let trusted_lock_id = EscrowId::new(Hash::new("native-asset-lock-trusted"));
        client.submit(
            OpenAssetLock::with_options(
                trusted_lock_id,
                asset_definition_id.clone(),
                destination.clone(),
                Quantity::from(40_u64),
                Some(release_authority.clone()),
                None,
                vec![Hash::new("native-lock-open-evidence")],
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let locked = wait_for_escrow_status(
            &client,
            trusted_lock_id,
            AssetEscrowStatus::Locked,
            "open trusted lock",
        )?;
        assert_eq!(locked.buyer, Some(destination.clone()));
        assert_eq!(locked.release_authority, Some(release_authority.clone()));
        assert_eq!(locked.remaining_amount, Quantity::from(40_u64));
        let custody_asset_id = AssetId::of(asset_definition_id.clone(), locked.custody.clone());
        wait_for_asset_value(
            &client,
            &source_asset_id,
            &Quantity::from(60_u64),
            "trusted lock source debit",
        )?;
        wait_for_asset_value(
            &client,
            &custody_asset_id,
            &Quantity::from(40_u64),
            "trusted lock custody credit",
        )?;
        client.submit(
            Grant::account_permission(
                Permission::from(CanTransferAssetWithDefinition {
                    asset_definition: asset_definition_id.clone(),
                }),
                source.clone(),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let denied = client
            .submit(
                Transfer::asset_quantity(custody_asset_id.clone(), 1_u64, source.clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err(
                "active native lock custody must not be drainable through generic transfer",
            );
        require_rejection_cause(&denied, "direct debit from native escrow custody account")?;
        let denied = destination_client
            .submit(
                DrawdownAssetLock::new(
                    trusted_lock_id,
                    Quantity::from(1_u64),
                    Quantity::from(40_u64),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err("destination cannot draw down when a release authority is set");
        require_rejection_cause(&denied, "only release authority may draw down lock")?;
        release_authority_client.submit(
            DrawdownAssetLock::new(
                trusted_lock_id,
                Quantity::from(15_u64),
                Quantity::from(40_u64),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let destination_asset_id = AssetId::of(asset_definition_id.clone(), destination.clone());
        wait_for_asset_value(
            &client,
            &destination_asset_id,
            &Quantity::from(15_u64),
            "trusted lock destination partial credit",
        )?;
        let partially_drawn = wait_for_escrow_status(
            &client,
            trusted_lock_id,
            AssetEscrowStatus::Locked,
            "release-authority partial drawdown",
        )?;
        assert_eq!(partially_drawn.remaining_amount, Quantity::from(25_u64));
        let denied = release_authority_client
            .submit(
                DrawdownAssetLock::new(
                    trusted_lock_id,
                    Quantity::from(1_u64),
                    Quantity::from(40_u64),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .expect_err("a stale independently submitted drawdown must not debit custody twice");
        require_rejection_cause(
            &denied,
            "asset lock remaining amount changed since the drawdown was prepared",
        )?;
        client.submit(
            CancelAssetLock::new(trusted_lock_id, partially_drawn.remaining_amount.clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let cancelled = wait_for_escrow_status(
            &client,
            trusted_lock_id,
            AssetEscrowStatus::Cancelled,
            "cancel trusted lock",
        )?;
        assert_eq!(cancelled.remaining_amount, Quantity::zero());
        wait_for_asset_value(
            &client,
            &source_asset_id,
            &Quantity::from(85_u64),
            "trusted lock cancellation refund",
        )?;
        let expiring_lock_id = EscrowId::new(Hash::new("native-asset-lock-expiring"));
        client.submit(
            OpenAssetLock::with_options(
                expiring_lock_id,
                asset_definition_id.clone(),
                destination.clone(),
                Quantity::from(10_u64),
                None,
                Some(0),
                Vec::new(),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        wait_for_escrow_status(
            &client,
            expiring_lock_id,
            AssetEscrowStatus::Locked,
            "open expiring lock",
        )?;
        destination_client.submit(
            ExpireAssetLock::new(expiring_lock_id),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let expired = wait_for_escrow_status(
            &client,
            expiring_lock_id,
            AssetEscrowStatus::Expired,
            "expire lock",
        )?;
        assert_eq!(expired.remaining_amount, Quantity::zero());
        wait_for_asset_value(
            &client,
            &source_asset_id,
            &Quantity::from(85_u64),
            "expiring lock refund",
        )?;
        wait_for_asset_value(
            &client,
            &destination_asset_id,
            &Quantity::from(15_u64),
            "destination balance after expiry refund",
        )?;
        Ok(())
    })();
    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
