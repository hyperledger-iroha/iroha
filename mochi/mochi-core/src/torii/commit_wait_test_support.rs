async fn submit_and_wait_for_commit_with_receivers<Fut>(
    tx_hash: HashOf<SignedTransaction>,
    options: SmokeCommitOptions,
    submit: Fut,
    mut block_rx: broadcast::Receiver<BlockStreamEvent>,
    mut event_rx: broadcast::Receiver<EventStreamEvent>,
) -> ToriiResult<u64>
where
    Fut: Future<Output = ToriiResult<()>>,
{
    submit.await?;
    let tx_hash_str = tx_hash.to_string();

    let wait = async {
        loop {
            tokio::select! {
                message = block_rx.recv() => {
                    match message {
                        Ok(BlockStreamEvent::Block { block, .. }) => {
                            if let Some(result) =
                                smoke_transaction_result_in_block(block.as_ref(), &tx_hash)
                            {
                                return result;
                            }
                        }
                        Ok(BlockStreamEvent::DecodeError { error }) => {
                            return Err(ToriiError::Decode(error.message));
                        }
                        Ok(BlockStreamEvent::Closed) => {
                            return Err(ToriiError::Timeout { context: "block stream closed".to_owned() });
                        }
                        Ok(BlockStreamEvent::Lagged { .. } | BlockStreamEvent::Text { .. }) => {}
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => {
                            return Err(ToriiError::Timeout { context: "block stream closed".to_owned() });
                        }
                    }
                }
                message = event_rx.recv() => {
                    match message {
                        Ok(EventStreamEvent::Event { event, .. }) => {
                            if let EventBox::Pipeline(PipelineEventBox::Transaction(tx_event)) = event.as_ref()
                                && tx_event.hash() == &tx_hash
                            {
                                match tx_event.status() {
                                    iroha_data_model::events::pipeline::TransactionStatus::Rejected(reason) => {
                                        return Err(ToriiError::SmokeRejected {
                                            hash: tx_hash_str.clone(),
                                            reason: format!("{reason:?}"),
                                        });
                                    }
                                    iroha_data_model::events::pipeline::TransactionStatus::Expired => {
                                        return Err(ToriiError::SmokeRejected {
                                            hash: tx_hash_str.clone(),
                                            reason: "expired".to_owned(),
                                        });
                                    }
                                    iroha_data_model::events::pipeline::TransactionStatus::Approved => {
                                        if let Some(height) =
                                            tx_event.block_height().map(std::num::NonZeroU64::get)
                                        {
                                            return Ok(height);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(EventStreamEvent::DecodeError { error }) => {
                            return Err(ToriiError::Decode(error.message));
                        }
                        Ok(EventStreamEvent::Closed) => {}
                        Ok(EventStreamEvent::Lagged { .. } | EventStreamEvent::Text { .. }) => {}
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => {}
                    }
                }
            }
        }
    };

    tokio::time::timeout(options.timeout, wait)
        .await
        .map_err(|_| ToriiError::Timeout {
            context: format!("smoke commit {tx_hash_str}"),
        })?
}
