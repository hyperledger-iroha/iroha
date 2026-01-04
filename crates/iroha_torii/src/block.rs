use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
};

use iroha_core::kura::Kura;
use iroha_data_model::block::stream::{BlockMessageSend, BlockSubscriptionRequest};

use crate::stream::{self, WebSocketNorito};

/// Type of error for `Consumer`
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error from provided stream/websocket
    #[error("Stream error: {0}")]
    Stream(#[source] stream::Error),
    /// Invalid block subscription height: {0}
    #[error("Invalid block subscription height: {0}")]
    InvalidHeight(String),
}

impl From<stream::Error> for Error {
    fn from(error: stream::Error) -> Self {
        Self::Stream(error)
    }
}

/// Result type for `Consumer`
pub type Result<T> = core::result::Result<T, Error>;

/// Consumer for Iroha `Block`(s).
/// Passes the blocks over the corresponding connection `stream`.
#[derive(Debug)]
pub struct Consumer<'ws> {
    pub stream: &'ws mut WebSocketNorito,
    height: NonZeroUsize,
    kura: Arc<Kura>,
}

impl<'ws> Consumer<'ws> {
    /// Constructs [`Consumer`], which forwards blocks through the `stream`.
    ///
    /// # Errors
    /// Can fail due to timeout or without message at websocket or during decoding request
    #[iroha_futures::telemetry_future]
    pub async fn new(stream: &'ws mut WebSocketNorito, kura: Arc<Kura>) -> Result<Self> {
        let BlockSubscriptionRequest(height) = stream.recv().await?;
        let height = request_height_to_kura(height)?;
        Ok(Consumer {
            stream,
            height,
            kura,
        })
    }

    /// Forwards block if block for given height already exists
    ///
    /// # Errors
    /// Can fail due to timeout. Also receiving might fail
    #[iroha_futures::telemetry_future]
    pub async fn consume(&mut self) -> Result<()> {
        if let Some(block) = self.kura.get_block(self.height) {
            self.stream.send(BlockMessageSend(block)).await?;
            self.height = self.height.checked_add(1).ok_or_else(|| {
                Error::InvalidHeight("maximum block height is achieved".to_string())
            })?;
        }
        Ok(())
    }
}

fn request_height_to_kura(height: NonZeroU64) -> Result<NonZeroUsize> {
    let raw = usize::try_from(height.get())
        .map_err(|_| Error::InvalidHeight("height exceeds platform limits".to_string()))?;
    NonZeroUsize::new(raw)
        .ok_or_else(|| Error::InvalidHeight("height must be non-zero".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_height_to_kura_handles_bounds() {
        let max = u64::try_from(usize::MAX).expect("usize fits in u64");
        let ok = NonZeroU64::new(max).expect("non-zero");
        assert!(request_height_to_kura(ok).is_ok());

        if usize::BITS < 64 {
            let too_large = NonZeroU64::new(max + 1).expect("non-zero");
            assert!(request_height_to_kura(too_large).is_err());
        }
    }
}
