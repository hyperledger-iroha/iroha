impl EventStream {
    fn new(subscription: WsSubscription) -> Self {
        let mut receiver = subscription.subscribe();
        let (sender, _) = broadcast::channel(128);
        let initial_receiver = sender.subscribe();
        let forwarder = sender.clone();

        let decode_handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(WsFrame::Binary(frame)) => {
                        let raw_len = frame.len();
                        match norito::decode_from_bytes::<EventMessage>(&frame) {
                            Ok(message) => {
                                let event_box: EventBox = message.into();
                                let summary = EventSummary::from_event(&event_box);
                                let event = Arc::new(event_box);
                                let _ = forwarder.send(EventStreamEvent::Event {
                                    summary,
                                    event,
                                    raw_len,
                                });
                            }
                            Err(err) => {
                                let _ = forwarder.send(EventStreamEvent::DecodeError {
                                    error: EventStreamDecodeError::new(
                                        EventDecodeStage::Frame,
                                        raw_len,
                                        err.to_string(),
                                    ),
                                });
                            }
                        }
                    }
                    Ok(WsFrame::Text(text)) => {
                        let truncated = if text.len() > 256 {
                            format!("{}…", &text[..255])
                        } else {
                            text
                        };
                        let _ = forwarder.send(EventStreamEvent::Text { text: truncated });
                    }
                    Ok(WsFrame::Error(message)) => {
                        let _ = forwarder.send(EventStreamEvent::DecodeError {
                            error: EventStreamDecodeError::new(
                                EventDecodeStage::Stream,
                                0,
                                message,
                            ),
                        });
                        break;
                    }
                    Ok(WsFrame::Closed) => {
                        let _ = forwarder.send(EventStreamEvent::Closed);
                        break;
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let _ = forwarder.send(EventStreamEvent::Lagged {
                            skipped: lag_to_usize(skipped),
                        });
                    }
                    Err(RecvError::Closed) => {
                        let _ = forwarder.send(EventStreamEvent::Closed);
                        break;
                    }
                }
            }
        });

        Self {
            subscription,
            sender,
            initial_receiver: std::sync::Mutex::new(Some(initial_receiver)),
            decode_handle,
        }
    }

    /// Acquire a receiver for decoded events.
    pub fn subscribe(&self) -> broadcast::Receiver<EventStreamEvent> {
        self.initial_receiver
            .lock()
            .expect("event stream receiver lock poisoned")
            .take()
            .unwrap_or_else(|| self.sender.subscribe())
    }

    /// Abort both the raw WebSocket subscription and decoder task.
    pub fn abort(&self) {
        self.subscription.abort();
        if !self.decode_handle.is_finished() {
            self.decode_handle.abort();
        }
    }

    /// Check whether the underlying tasks finished.
    pub fn is_finished(&self) -> bool {
        self.subscription.is_finished() && self.decode_handle.is_finished()
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        self.abort();
    }
}
