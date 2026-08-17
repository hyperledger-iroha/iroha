// Authenticated revision-4 vote-fault support for the four-peer release corridor.
const FOUR_PEER_FAULT_HEIGHT_WINDOW: u64 = 8;
const FOUR_PEER_FAULT_VIEW_WINDOW: u64 = 2;
const FOUR_PEER_FAULT_QUEUE_CAPACITY: usize = 256;
const FOUR_PEER_FAULT_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Debug)]
struct FourPeerAuthenticatedFaultReceipt {
    receiver_index: usize,
    revision: u64,
    held_before: usize,
    dropped_before: u64,
    overflowed_before: u64,
    rejected_before: u64,
}

#[derive(Clone, Debug)]
struct FourPeerAuthenticatedFault {
    sender: PeerId,
    first_height: u64,
    action: ConsensusMessageControlAction,
    receipts: Vec<FourPeerAuthenticatedFaultReceipt>,
}

fn four_peer_authenticated_vote_fault_rules(
    sender: &PeerId,
    first_height: u64,
    action: ConsensusMessageControlAction,
) -> Vec<ConsensusMessageControlRule> {
    let mut rules = Vec::new();
    for height in first_height..first_height.saturating_add(FOUR_PEER_FAULT_HEIGHT_WINDOW) {
        for view in 0..FOUR_PEER_FAULT_VIEW_WINDOW {
            for kind in [
                ConsensusMessageControlKind::PrepareVote,
                ConsensusMessageControlKind::CommitVote,
            ] {
                rules.push(ConsensusMessageControlRule::exact(
                    sender.clone(),
                    kind,
                    height,
                    view,
                    action,
                ));
            }
        }
    }
    rules
}

fn arm_four_peer_authenticated_vote_fault(
    network: &sandbox::SerializedNetwork,
    runtime: &tokio::runtime::Runtime,
    sender_index: usize,
    action: ConsensusMessageControlAction,
    context: &str,
) -> Result<FourPeerAuthenticatedFault> {
    let sender = network
        .peers()
        .get(sender_index)
        .ok_or_else(|| eyre!("{context}: missing fault sender {sender_index}"))?
        .id();
    let first_height = network
        .peers()
        .iter()
        .map(|peer| {
            peer_client_with_timeout(peer)
                .get_status()
                .map(|status| status.blocks)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .ok_or_else(|| eyre!("{context}: four-peer network has no status height"))?
        .saturating_add(1);
    let rules = four_peer_authenticated_vote_fault_rules(&sender, first_height, action);
    let mut receipts = Vec::with_capacity(TOTAL_PEERS - 1);
    for (receiver_index, receiver) in network.peers().iter().enumerate() {
        if receiver_index == sender_index {
            continue;
        }
        let control = receiver.consensus_message_control().ok_or_else(|| {
            eyre!(
                "{context}: receiver {receiver_index} lacks authenticated consensus message control"
            )
        })?;
        let before = control
            .read_ack()
            .map_err(|err| eyre!("{context}: read receiver {receiver_index} baseline: {err}"))?;
        ensure!(
            before.rules.is_empty()
                && before.held.is_empty()
                && before.release_pending.is_empty()
                && before.in_flight.is_none()
                && !before.draining
                && !before.fatal,
            "{context}: receiver {receiver_index} did not begin from a healed controller: {before:?}"
        );
        let armed = runtime.block_on(control.apply(
            &rules,
            &[],
            FOUR_PEER_FAULT_QUEUE_CAPACITY,
            FOUR_PEER_FAULT_ACTIVATION_TIMEOUT,
        ))?;
        ensure!(
            armed.revision > before.revision
                && armed.rules == rules
                && armed.queue_capacity == FOUR_PEER_FAULT_QUEUE_CAPACITY
                && armed.dropped >= before.dropped
                && armed.overflowed == before.overflowed
                && armed.rejected_commands == before.rejected_commands
                && !armed.fatal,
            "{context}: receiver {receiver_index} did not acknowledge the exact authenticated vote fault: before={before:?}, armed={armed:?}"
        );
        ensure!(
            armed.rules.iter().all(|rule| {
                rule.sender == sender
                    && rule.authenticated_via == sender
                    && rule.action == action
                    && rule.block_hash.is_none()
            }),
            "{context}: receiver {receiver_index} installed a relayed, hash-rewritten, or wrong-action fault rule"
        );
        receipts.push(FourPeerAuthenticatedFaultReceipt {
            receiver_index,
            revision: armed.revision,
            held_before: before.held.len(),
            dropped_before: before.dropped,
            overflowed_before: before.overflowed,
            rejected_before: before.rejected_commands,
        });
    }
    ensure!(
        receipts.len() == TOTAL_PEERS - 1,
        "{context}: authenticated vote fault did not cover all three non-sender receivers"
    );
    Ok(FourPeerAuthenticatedFault {
        sender,
        first_height,
        action,
        receipts,
    })
}

fn wait_for_four_peer_authenticated_fault_activation(
    network: &sandbox::SerializedNetwork,
    fault: &FourPeerAuthenticatedFault,
    context: &str,
) -> Result<()> {
    let expected_rules =
        four_peer_authenticated_vote_fault_rules(&fault.sender, fault.first_height, fault.action);
    let deadline = Instant::now() + FOUR_PEER_FAULT_ACTIVATION_TIMEOUT;
    loop {
        let mut activation_evidence = 0_u64;
        let mut matched_receivers = 0_usize;
        for receipt in &fault.receipts {
            let ack = network.peers()[receipt.receiver_index]
                .consensus_message_control()
                .expect("controlled four-peer receiver")
                .read_ack()?;
            ensure!(
                ack.revision == receipt.revision
                    && ack.rules == expected_rules
                    && ack.queue_capacity == FOUR_PEER_FAULT_QUEUE_CAPACITY
                    && ack.overflowed == receipt.overflowed_before
                    && ack.rejected_commands == receipt.rejected_before
                    && !ack.fatal,
                "{context}: receiver {} changed or overflowed its authenticated fault command: {ack:?}",
                receipt.receiver_index
            );
            let matched = match fault.action {
                ConsensusMessageControlAction::Hold => ack
                    .held
                    .iter()
                    .filter(|message| {
                        message.sender == fault.sender
                            && message.authenticated_via == fault.sender
                            && matches!(
                                message.kind,
                                ConsensusMessageControlKind::PrepareVote
                                    | ConsensusMessageControlKind::CommitVote
                            )
                    })
                    .count() as u64,
                ConsensusMessageControlAction::Drop => {
                    ack.dropped.saturating_sub(receipt.dropped_before)
                }
            };
            if matched > 0 {
                matched_receivers = matched_receivers.saturating_add(1);
            }
            activation_evidence = activation_evidence.saturating_add(matched);
        }
        if matched_receivers == fault.receipts.len() {
            eprintln!(
                "[multilane-release-gate] authenticated {:?} fault activated for sender {} on all {matched_receivers} receivers with {activation_evidence} matched vote(s)",
                fault.action, fault.sender,
            );
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "{context}: authenticated {:?} fault matched {matched_receivers}/{} receivers before timeout",
            fault.action,
            fault.receipts.len(),
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn heal_four_peer_authenticated_vote_fault(
    network: &sandbox::SerializedNetwork,
    runtime: &tokio::runtime::Runtime,
    fault: &FourPeerAuthenticatedFault,
    context: &str,
) -> Result<()> {
    for receipt in &fault.receipts {
        let control = network.peers()[receipt.receiver_index]
            .consensus_message_control()
            .expect("controlled four-peer receiver");
        let before = control.read_ack()?;
        let healed =
            runtime.block_on(control.heal_and_release_all(FOUR_PEER_FAULT_ACTIVATION_TIMEOUT))?;
        ensure!(
            healed.revision > receipt.revision
                && healed.rules.is_empty()
                && healed.held.is_empty()
                && healed.release_pending.is_empty()
                && healed.in_flight.is_none()
                && !healed.draining
                && healed.drain_fence == Some(healed.revision)
                && !healed.fatal
                && healed.overflowed == receipt.overflowed_before
                && healed.rejected_commands == receipt.rejected_before
                && healed.dropped == before.dropped,
            "{context}: receiver {} did not atomically heal its authenticated fault: before={before:?}, healed={healed:?}",
            receipt.receiver_index
        );
        if fault.action == ConsensusMessageControlAction::Hold {
            let retained = before.held.len().saturating_sub(receipt.held_before);
            if retained > 0 {
                ensure!(
                    healed.delivered.len().saturating_add(healed.retired.len()) >= retained,
                    "{context}: receiver {} did not account for all retained votes at the heal fence: before={before:?}, healed={healed:?}",
                    receipt.receiver_index
                );
            }
        }
    }
    Ok(())
}
