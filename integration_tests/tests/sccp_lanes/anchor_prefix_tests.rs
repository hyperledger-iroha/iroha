//! Signed fixture and real native-block qualification of the bounded anchor driver.

use super::*;
use iroha::client::TransactionWaitOptions;
use iroha_model_base::name::Name;
use std::collections::BTreeSet;

#[test]
fn anchor_fixture_signs_the_block_bound_without_changing_heartbeat_or_committee() -> Result<()> {
    let mut parameters = SccpParametersV1::taira_default();
    parameters.committee_heartbeat_ms = SCCP_MIN_HEARTBEAT_MS_V1;
    let network = anchor_builder(parameters).build();
    let genesis = network.genesis();
    let (epoch, _) = authenticated_genesis(&genesis.0)?.into_parts();
    ensure!(epoch.committee.len() == 4);
    ensure!(network.block_cadence() == Duration::from_millis(100));
    let mut maximum = None;
    let mut heartbeat = None;
    let mut npos = None;
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions.iter() {
            if let Some(set) = instruction.as_any().downcast_ref::<SetParameter>() {
                match &set.0 {
                    Parameter::Block(
                        iroha::data_model::parameter::BlockParameter::MaxTransactions(value),
                    ) => maximum = Some(value.get()),
                    Parameter::Custom(value) => {
                        if let Some(value) = SumeragiNposParameters::from_custom_parameter(value)? {
                            npos = Some(value);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(initialize) = instruction.as_any().downcast_ref::<InitializeSccpV1>() {
                heartbeat = Some(initialize.parameters.committee_heartbeat_ms);
            }
        }
    }
    ensure!(maximum == Some(1));
    ensure!(heartbeat == Some(SCCP_MIN_HEARTBEAT_MS_V1));
    let npos = npos.ok_or_else(|| eyre!("signed NPoS policy missing"))?;
    ensure!(npos.max_validators == 4 && npos.slashing_delay_blocks == 2);
    ensure!(npos.epoch_length_blocks.get() == SCCP_MIN_HEARTBEAT_MS_V1 / 100);
    Ok(())
}

/// The previous serial loop, retained only as a measured test baseline.
fn serial_prefix(
    client: &iroha::blocking::Client,
    start: u64,
    target: u64,
) -> Result<Vec<anchor_prefix::Receipt>> {
    let key: Name = anchor_prefix::METADATA_KEY.parse()?;
    let mut receipts = Vec::new();
    let mut height = start;
    let mut sequence = 0_u64;
    while height < target {
        sequence = sequence
            .checked_add(1)
            .ok_or_else(|| eyre!("sequence overflow"))?;
        let transaction_hash = client.submit(
            SetKeyValue::account(ALICE_ID.clone(), key.clone(), Json::new(sequence)),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let next_height = client.client().get_sumeragi_status()?.committed_height;
        ensure!(next_height > height);
        height = next_height;
        receipts.push(anchor_prefix::Receipt {
            sequence,
            transaction_hash,
            observed_height: height,
        });
    }
    Ok(receipts)
}

#[test]
fn bounded_anchor_prefix_matches_serial_native_blocks() -> Result<()> {
    // ABBA keeps both implementations on the same signed four-validator fixture.
    // Startup and proof verification are outside each measured driver interval.
    const PREFIX_BLOCKS: u64 = 12;
    init_instruction_registry();
    let (network, runtime) = sandbox::start_network_blocking_or_skip(
        anchor_builder(SccpParametersV1::taira_default()),
        stringify!(bounded_anchor_prefix_matches_serial_native_blocks),
    )?
    .ok_or_else(|| eyre!("anchor prefix qualification requires four actual validators"))?;
    runtime.block_on(network.ensure_blocks(1))?;
    let genesis = network.genesis();
    let (epoch, scope) = authenticated_genesis(&genesis.0)?.into_parts();
    let keys = epoch
        .committee
        .iter()
        .map(|member| {
            let (algorithm, bytes) = member.validator.public_key().try_to_bytes()?;
            ensure!(algorithm == Algorithm::BlsNormal);
            <[u8; 48]>::try_from(bytes).map_err(|_| eyre!("genesis key width"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(keys.len() == 4);
    let instance = iroha_sumeragi::preimage::instance_id(
        &iroha_core::sumeragi::crypto::BlsCrypto::new(),
        &iroha_sumeragi::types::Hash32(*epoch.network_id.as_bytes()),
        network.chain_id().as_str().as_bytes(),
        iroha_sumeragi::preimage::InstanceKind::Global,
        0,
    )
    .0;
    let client = network.peers()[0].client();
    get_status_with_retry_at_least(&client, 1)?;
    let mut samples = Vec::new();
    let mut all_hashes = BTreeSet::new();
    for (sample, pipelined) in [false, true, true, false].into_iter().enumerate() {
        let start = client.client().get_sumeragi_status()?.committed_height;
        let target = start
            .checked_add(PREFIX_BLOCKS)
            .ok_or_else(|| eyre!("target overflow"))?;
        let started = Instant::now();
        let receipts = if pipelined {
            anchor_prefix::drive(&client, start, target, anchor_prefix::WINDOW, |_| Ok(()))?
        } else {
            serial_prefix(&client, start, target)?
        };
        let wall_ms = u64::try_from(started.elapsed().as_millis())?;
        ensure!(receipts.len() == usize::try_from(PREFIX_BLOCKS)?);
        let heights = receipts
            .iter()
            .map(|receipt| receipt.observed_height)
            .collect::<BTreeSet<_>>();
        ensure!(heights == ((start + 1)..=target).collect());
        runtime.block_on(network.ensure_blocks(target))?;
        for receipt in &receipts {
            ensure!(all_hashes.insert(receipt.transaction_hash));
            // Also independently pin the serial baseline's observed status height
            // to the exact transaction, rather than assuming queue ordering.
            let applied = client.wait_for_transaction_applied(
                receipt.transaction_hash,
                TransactionWaitOptions {
                    timeout: client.client().transaction_status_timeout(),
                    poll_interval: Duration::from_secs(2),
                },
            )?;
            ensure!(applied.block_height == Some(receipt.observed_height));
            let height = NonZeroU64::new(receipt.observed_height).unwrap();
            let certificate = client.sccp().finality(SccpFinalityAt::Height(height))?;
            let verified = verify_certificate(
                &instance,
                &certificate.qc,
                &certificate.header,
                &certificate.aggregate_signature,
                &keys,
            )?;
            let (block, _) = read_sccp_block_wire(
                &client,
                height,
                scope,
                &instance,
                &verified.qc.block_hash,
                &verified.result,
                Instant::now() + network.sync_timeout(),
            )?;
            ensure!(block.external_entrypoint_count() == 1 && !block.is_empty());
            let signed = block
                .external_transactions()
                .next()
                .ok_or_else(|| eyre!("ordinary metadata transaction missing"))?;
            ensure!(signed.hash() == receipt.transaction_hash);
            let expected = InstructionBox::from(SetKeyValue::account(
                ALICE_ID.clone(),
                anchor_prefix::METADATA_KEY.parse::<Name>()?,
                Json::new(receipt.sequence),
            ));
            ensure!(
                matches!(signed.instructions(), Executable::Instructions(instructions) if instructions.as_ref() == [expected])
            );
        }
        let final_certificate = client
            .sccp()
            .finality(SccpFinalityAt::Height(NonZeroU64::new(target).unwrap()))?;
        let final_header = SccpFinalityHeaderV1::parse(&final_certificate.header)?;
        agreed_certificate(
            &network,
            &instance,
            &keys,
            target,
            final_header.generation(),
            final_header.is_rotation(),
        )?;
        samples.push(norito::json!({
            "sample": sample, "driver": (if pipelined { "bounded" } else { "serial" }),
            "window": (if pipelined { anchor_prefix::WINDOW } else { 1 }),
            "start_height": start, "end_height": target, "transactions": PREFIX_BLOCKS,
            "wall_ms": wall_ms,
        }));
    }
    eprintln!(
        "SCCP_ANCHOR_PREFIX_ABBA {}",
        norito::json::to_string(&samples)?
    );
    Ok(())
}
