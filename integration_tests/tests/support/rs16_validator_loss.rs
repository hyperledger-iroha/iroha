//! Actual voter loss, exact surviving quorum and cold signed-row acquisition across committee sizes.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::consensus::NexusFeeSettlementV1, nexus::FeeDebitSource, transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};

/// Stop all f seats before creating any bytes of the target transaction. Surviving voters
/// must apply it with signed transport, then the cold returning seats must fetch its rows.
fn run_with_voter_loss(n: usize) -> Result<()> {
    ensure!(
        (4..=31).contains(&n) && (n - 1) % 3 == 0,
        "invalid committee size"
    );
    init_instruction_registry();
    let f = (n - 1) / 3;
    let q = n - f;
    let recipe = builder_with_fee(n, BOB_ID.clone(), "1")
        .with_base_seed(format!("signed-rs16-voter-loss-{n}"))
        .with_config_layer(|layer| {
            layer.write(["logger", "format"], "json");
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(recipe, "signed-rs16-voter-loss")?
    else {
        bail!("required committee-loss network could not start");
    };
    let mut exits = network
        .all_peers()
        .map(NetworkPeer::events)
        .collect::<Vec<_>>();
    let mut runs = vec![1; n];
    let result = (|| -> Result<()> {
        ensure!(network.validators().len() == n && network.all_peers().count() == n);
        let original = rs16_hold::exact_processes(&network, &rt)?;
        let warm_deadline = Instant::now() + Duration::from_secs(120);
        rs16_hold::wait_applied(&network, 1, warm_deadline, true)?;
        let target = 2;
        let peers = network.validators();
        let missing = &peers[q..];
        for peer in missing {
            rt.block_on(peer.shutdown());
            ensure!(!peer.is_running() && rt.block_on(peer.process_id()).is_none());
        }
        let deadline = Instant::now() + Duration::from_secs(180);
        let target_key = KeyPair::try_from_seed(
            format!("signed-rs16-voter-loss-{n}-paid-target").into_bytes(),
            Algorithm::Ed25519,
        )?;
        let fresh = AccountId::new(target_key.public_key().clone());
        let transaction = peers[0].client().with_request_deadline(deadline)?.submit(
            Register::account(Account::new(fresh.clone())),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;
        let survivors = peers[..q].iter().collect::<Vec<_>>();
        rs16_hold::wait_applied_on(&network, &survivors, target, deadline)?;
        let certified = rs16_hold::certified_at(&network, &peers[0], target, deadline)?;
        let committed = certified.committed();
        let qc = certified
            .commit_qc()
            .ok_or_else(|| eyre!("missing exact-survivor commit certificate"))?;
        let committee = &committed.commitment().schedule.current.committee;
        for (index, seat) in committee.iter().enumerate() {
            let survivor = peers[..q].iter().any(|peer| peer.id() == seat.validator);
            ensure!(
                qc.signers.get(u32::try_from(index)?) == survivor,
                "vote from an absent seat or missing survivor"
            );
        }
        let block = committed.block();
        let (entry, source_id) = (0..block.network_entrypoint_count())
            .find_map(|index| match block.network_entrypoint_at(index) {
                Some(TransactionEntrypoint::External(tx)) if tx.hash() == transaction => {
                    Some((index, iroha_crypto::Hash::from(tx.hash_as_entrypoint())))
                }
                _ => None,
            })
            .ok_or_else(|| eyre!("certified block omitted the original transaction"))?;
        let (_, output) = block
            .network_output_at(u32::try_from(entry)?)
            .ok_or_else(|| eyre!("certified block omitted the original result"))?;
        let receipt = output
            .result
            .nexus_fee_receipt()
            .ok_or_else(|| eyre!("missing paid receipt"))?;
        let fee_policy = iroha_genesis::signed_genesis_root_fee_policy(&network.genesis().0)?;
        ensure!(
            output.result.0.is_ok()
                && receipt.source_id == *source_id.as_ref()
                && receipt.fee_asset_id == fee_policy.fee_asset_id
                && receipt.block_height == target
                && receipt.fee_amount == Quantity::from(1_u32)
                && receipt.debit_source == FeeDebitSource::Account(ALICE_ID.clone())
                && matches!(receipt.settlement, NexusFeeSettlementV1::Burn)
        );
        for (index, peer) in peers[..q].iter().enumerate() {
            let pid = rt
                .block_on(peer.process_id())
                .ok_or_else(|| eyre!("survivor disappeared"))?;
            ensure!(pid == original[index].0);
            rs16_hold::verify_peer_rows(&network, peer, pid, &certified, deadline)?;
        }
        let restart_deadline = tokio::time::Instant::now() + network.peer_startup_timeout();
        for (index, peer) in peers.iter().enumerate().skip(q) {
            let layers = network.config_layers_for_peer(peer).collect::<Vec<_>>();
            rt.block_on(async {
                tokio::time::timeout_at(restart_deadline, peer.start_checked(layers.iter(), None))
                    .await
            })
            .map_err(|_| {
                eyre!(
                    "voter {} missed the committee restart deadline",
                    peer.mnemonic()
                )
            })??;
            runs[index] += 1;
        }
        let repair_deadline = Instant::now() + Duration::from_secs(180);
        rs16_hold::wait_applied(&network, target, repair_deadline, true)?;
        let running = rs16_hold::exact_processes(&network, &rt)?;
        for (index, peer) in peers.iter().enumerate() {
            if index < q {
                ensure!(running[index] == original[index]);
                continue;
            }
            ensure!(
                running[index].1 == original[index].1 + 1,
                "missing distinct restarted process run"
            );
            let recovered = rs16_hold::certified_at(&network, peer, target, repair_deadline)?;
            ensure!(
                recovered.committed().core_hash() == committed.core_hash()
                    && recovered.committed().block_hash() == committed.block_hash()
                    && recovered.committed().result() == committed.result()
            );
            let evidence = rs16_hold::verify_peer_rows(
                &network,
                peer,
                running[index].0,
                &recovered,
                repair_deadline,
            )?;
            ensure!(
                evidence.admitted_rows > 0,
                "returning seat failed to acquire signed network rows"
            );
            ensure!(rs16_local_state::account_present(
                peer,
                &rt,
                &fresh,
                &recovered,
                repair_deadline
            )?);
        }
        eprintln!("RS16_VOTER_LOSS committee={n} stopped={f} exact_signers={q} height={target}");
        Ok(())
    })();
    rt.block_on(network.shutdown());
    result.and(rs16_hold::clean_exits(&network, &rt, &mut exits, &runs))
}

#[test]
fn four_voters_recover_signed_rows_after_exact_quorum_finality() -> Result<()> {
    run_with_voter_loss(4)
}

/// One committee per process on the qualification runner to bound resource use.
#[test]
#[ignore = "multi-daemon committee qualification, select one size on a provisioned host"]
fn production_committee_recovers_signed_rows_after_exact_quorum_finality() -> Result<()> {
    let n = std::env::var("IROHA_RS16_QUALIFICATION_COMMITTEE")
        .map_err(|_| eyre!("select one of 7, 10, 13, 16, 19, 22, 25, 28, 31 validators"))?
        .parse::<usize>()?;
    ensure!(n > 4, "four votes have a dedicated mandatory test");
    run_with_voter_loss(n)
}
