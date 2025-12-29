//! Iroha is a quite dynamic system so many events can happen.
//! This module contains descriptions of such an events and
//! utility Iroha Special Instructions to work with them.

use iroha_data_model::events::prelude::*;

use crate::stream::{self, WebSocketNorito};

/// Type of error for `Consumer`
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error from provided stream/websocket
    #[error("Stream error: {0}")]
    Stream(#[source] stream::Error),
}

impl From<stream::Error> for Error {
    fn from(error: stream::Error) -> Self {
        Self::Stream(error)
    }
}

/// Result type for `Consumer`
pub type Result<T> = core::result::Result<T, Error>;

/// Consumer for Iroha `Event`(s).
/// Passes the events over the corresponding connection `stream` if they match the `filter`.
#[derive(Debug)]
pub struct Consumer<'ws> {
    pub stream: &'ws mut WebSocketNorito,
    filters: Vec<EventFilterBox>,
    proof_backend: Option<Vec<String>>,
    proof_call_hash: Option<Vec<[u8; 32]>>,
}

impl<'ws> Consumer<'ws> {
    /// Constructs [`Consumer`], which consumes `Event`s and forwards it through the `stream`.
    ///
    /// # Errors
    /// Can fail due to timeout or without message at websocket or during decoding request
    #[iroha_futures::telemetry_future]
    pub async fn new(stream: &'ws mut WebSocketNorito) -> Result<Self> {
        let EventSubscriptionRequest(filters) = stream.recv::<EventSubscriptionRequest>().await?;
        // Optional proof-specific filters message; read with a short timeout if present
        let mut consumer = Consumer {
            stream,
            filters,
            proof_backend: None,
            proof_call_hash: None,
        };
        if let Ok(proof) = consumer
            .stream
            .recv_with_timeout::<EventSubscriptionProofFilter>(core::time::Duration::from_millis(
                50,
            ))
            .await
        {
            consumer.proof_backend = proof.proof_backend;
            consumer.proof_call_hash = proof.proof_call_hash;
        }
        Ok(consumer)
    }

    /// Forwards the `event` over the `stream` if it matches the `filter`.
    ///
    /// # Errors
    /// Can fail due to timeout or sending event. Also receiving might fail
    #[iroha_futures::telemetry_future]
    pub async fn consume(&mut self, event: EventBox) -> Result<()> {
        if !self.filters.iter().any(|filter| filter.matches(&event)) {
            return Ok(());
        }
        // Apply optional proof-specific filters even when streaming via WebSocket.
        if let EventBox::Data(ev) = &event {
            if let iroha_data_model::prelude::DataEvent::Proof(pe) = ev.as_ref() {
                let mut drop = false;
                match pe {
                    iroha_data_model::events::data::proof::ProofEvent::Verified(v) => {
                        if let Some(bs) = &self.proof_backend {
                            let backend = v.id.backend.clone();
                            if !bs.contains(&backend) {
                                drop = true;
                            }
                        }
                        if let Some(hs) = &self.proof_call_hash {
                            if !v.call_hash.as_ref().is_some_and(|hash| hs.contains(hash)) {
                                drop = true;
                            }
                        }
                    }
                    iroha_data_model::events::data::proof::ProofEvent::Rejected(r) => {
                        if let Some(bs) = &self.proof_backend {
                            let backend = r.id.backend.clone();
                            if !bs.contains(&backend) {
                                drop = true;
                            }
                        }
                        if let Some(hs) = &self.proof_call_hash {
                            if !r.call_hash.as_ref().is_some_and(|hash| hs.contains(hash)) {
                                drop = true;
                            }
                        }
                    }
                    iroha_data_model::events::data::proof::ProofEvent::Pruned(p) => {
                        if let Some(bs) = &self.proof_backend {
                            if !bs.contains(&p.backend) {
                                drop = true;
                            }
                        }
                    }
                }
                if drop {
                    return Ok(());
                }
            }
        }

        self.stream
            .send(EventMessage(event))
            .await
            .map_err(Into::into)
    }
}
