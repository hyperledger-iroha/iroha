//! Blocking account streams driven by the facade's reusable owned runtime.

use std::{num::NonZeroU64, sync::Arc, time::Duration};

use futures_util::StreamExt;

use super::{AccountClient, RuntimeOwner};
use crate::{
    Result,
    data_model::{
        block::SignedBlock,
        events::{EventBox, EventFilterBox},
    },
};

/// Blocking account event capability.
pub struct Events<'a> {
    account: &'a AccountClient,
}
/// Blocking global-reader block capability.
pub struct Blocks<'a> {
    account: &'a AccountClient,
}

impl AccountClient {
    /// Access blocking event subscriptions using this account authority.
    #[must_use]
    pub fn events(&self) -> Events<'_> {
        Events { account: self }
    }
    /// Access blocking full-block subscriptions using this account authority.
    #[must_use]
    pub fn blocks(&self) -> Blocks<'_> {
        Blocks { account: self }
    }
}

impl Events<'_> {
    /// Subscribe through the account's asynchronous implementation.
    ///
    /// # Errors
    /// Returns subscription errors or rejects calls from an async runtime.
    pub fn subscribe(
        &self,
        filters: impl IntoIterator<Item = impl Into<EventFilterBox>> + Send,
    ) -> Result<SubscriptionStream<EventBox>> {
        let stream = self
            .account
            .runtime
            .block_on(self.account.inner.events().subscribe(filters))??;
        Ok(SubscriptionStream {
            stream,
            runtime: Arc::clone(&self.account.runtime),
        })
    }
}

impl Blocks<'_> {
    /// Subscribe from the exact block height using the asynchronous implementation.
    ///
    /// # Errors
    /// Returns subscription errors or rejects calls from an async runtime.
    pub fn subscribe(&self, height: NonZeroU64) -> Result<SubscriptionStream<SignedBlock>> {
        let stream = self
            .account
            .runtime
            .block_on(self.account.inner.blocks().subscribe(height))??;
        Ok(SubscriptionStream {
            stream,
            runtime: Arc::clone(&self.account.runtime),
        })
    }
}

/// Owned blocking subscription sharing the account facade's runtime.
pub struct SubscriptionStream<T> {
    stream: crate::client::streams::SubscriptionStream<T>,
    runtime: Arc<RuntimeOwner>,
}

impl<T> SubscriptionStream<T> {
    /// Exact received binary-message length for the last delivered item.
    ///
    /// Binary decoding and message-size errors retain the received length;
    /// transport and abnormal-close errors clear it. Before the first item it
    /// is `None`. Receive timeouts and normal end-of-stream preserve the prior
    /// observation. Reading this metadata does not enter the owned runtime.
    #[must_use]
    pub const fn last_message_bytes(&self) -> Option<usize> {
        self.stream.last_message_bytes()
    }

    /// Receive one item, optionally limiting this wait without closing the stream.
    ///
    /// `None` means the peer completed normally. Timeout is a typed error; the
    /// next call may continue receiving without resubscribing or replaying.
    ///
    /// # Errors
    /// Returns stream errors, timeout, or async-runtime rejection.
    pub fn recv(&mut self, timeout: Option<Duration>) -> Result<Option<T>> {
        self.runtime.block_on(async {
            let next = self.stream.next();
            let item = if let Some(timeout) = timeout {
                tokio::time::timeout(timeout, next)
                    .await
                    .map_err(|_| crate::Error::Timeout {
                        operation: "stream.receive",
                    })?
            } else {
                next.await
            };
            item.transpose()
        })?
    }

    /// Close through the asynchronous implementation under its bounded deadline.
    ///
    /// # Errors
    /// Returns close/timeout failures or async-runtime rejection.
    pub fn close(self) -> Result<()> {
        self.runtime.block_on(self.stream.close())?
    }
}
