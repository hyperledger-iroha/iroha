// UI fanout for the canonical SDK event subscription.

/// Broadcast fanout of SDK-decoded events and UI summaries.
pub struct EventStream {
    sender: broadcast::Sender<EventStreamEvent>,
    initial_receiver: std::sync::Mutex<Option<broadcast::Receiver<EventStreamEvent>>>,
    worker: JoinHandle<()>,
}
impl EventStream {
    fn new(mut subscription: iroha::client::streams::EventStream) -> Self {
        let (sender, initial_receiver) = broadcast::channel(128);
        let forwarder = sender.clone();
        let worker = tokio::spawn(async move {
            while let Some(item) = subscription.next().await {
                let raw_len = subscription.last_message_bytes();
                match item {
                    Ok(value) => {
                        let summary = EventSummary::from_event(&value);
                        let _ = forwarder.send(EventStreamEvent::Event {
                            summary,
                            event: Arc::new(value),
                            raw_len: raw_len
                                .expect("decoded SDK items carry received message bytes"),
                        });
                    }
                    Err(error) => {
                        let stage = if subscription.last_message_bytes().is_some() {
                            EventDecodeStage::Frame
                        } else {
                            EventDecodeStage::Stream
                        };
                        let mut failure =
                            EventStreamDecodeError::new(stage, raw_len, error.to_string());
                        failure.source = Some(Arc::new(error));
                        let _ = forwarder.send(EventStreamEvent::DecodeError { error: failure });
                        return;
                    }
                }
            }
            let _ = forwarder.send(EventStreamEvent::Closed);
        });
        Self {
            sender,
            initial_receiver: std::sync::Mutex::new(Some(initial_receiver)),
            worker,
        }
    }
    /// Acquire a receiver; the first receiver retains events produced during construction.
    pub fn subscribe(&self) -> broadcast::Receiver<EventStreamEvent> {
        self.initial_receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| self.sender.subscribe())
    }
    /// Cancel the fanout task and release its owned SDK connection.
    pub fn abort(&self) {
        self.worker.abort();
    }
    /// Whether the fanout task has finished.
    pub fn is_finished(&self) -> bool {
        self.worker.is_finished()
    }
}
impl Drop for EventStream {
    fn drop(&mut self) {
        self.abort();
    }
}
