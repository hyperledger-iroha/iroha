//! Iroha is a quite dynamic system so many events can happen.
//! This module contains descriptions of such an events and
//! utility Iroha Special Instructions to work with them.

use iroha_data_model::events::prelude::*;

use crate::{
    proof_filters,
    stream::{self, WebSocketNorito},
};

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
    proof_envelope_hash: Option<Vec<[u8; 32]>>,
}

impl<'ws> Consumer<'ws> {
    /// Constructs [`Consumer`], which consumes `Event`s and forwards it through the `stream`.
    ///
    /// # Errors
    /// Can fail due to timeout or without message at websocket or during decoding request
    #[iroha_futures::telemetry_future]
    pub async fn new(stream: &'ws mut WebSocketNorito) -> Result<Self> {
        let request = stream.recv::<EventSubscriptionRequest>().await?;
        let (proof_backend, proof_call_hash, proof_envelope_hash) =
            proof_filters::normalize_proof_filters(
                request.proof_backend,
                request.proof_call_hash,
                request.proof_envelope_hash,
            );
        Ok(Consumer {
            stream,
            filters: request.filters,
            proof_backend,
            proof_call_hash,
            proof_envelope_hash,
        })
    }

    /// Forwards the `event` over the `stream` if it matches the `filter`.
    ///
    /// # Errors
    /// Can fail due to timeout or sending event. Also receiving might fail
    #[iroha_futures::telemetry_future]
    pub async fn consume(&mut self, event: EventBox) -> Result<()> {
        match event {
            EventBox::PipelineBatch(events) => {
                for event in events {
                    let event = EventBox::Pipeline(event);
                    if !self.filters.iter().any(|filter| filter.matches(&event)) {
                        continue;
                    }
                    if proof_filters::has_any_proof_filters(
                        self.proof_backend.as_ref(),
                        self.proof_call_hash.as_ref(),
                        self.proof_envelope_hash.as_ref(),
                    ) && !proof_filters::event_matches_proof_filters(
                        &event,
                        self.proof_backend.as_ref(),
                        self.proof_call_hash.as_ref(),
                        self.proof_envelope_hash.as_ref(),
                        false,
                    ) {
                        continue;
                    }
                    self.stream
                        .send(EventMessage(event))
                        .await
                        .map_err(Error::from)?;
                }
                Ok(())
            }
            event => {
                if !self.filters.iter().any(|filter| filter.matches(&event)) {
                    return Ok(());
                }
                if proof_filters::has_any_proof_filters(
                    self.proof_backend.as_ref(),
                    self.proof_call_hash.as_ref(),
                    self.proof_envelope_hash.as_ref(),
                ) && !proof_filters::event_matches_proof_filters(
                    &event,
                    self.proof_backend.as_ref(),
                    self.proof_call_hash.as_ref(),
                    self.proof_envelope_hash.as_ref(),
                    false,
                ) {
                    return Ok(());
                }

                self.stream
                    .send(EventMessage(event))
                    .await
                    .map_err(Error::from)
            }
        }
    }
}
