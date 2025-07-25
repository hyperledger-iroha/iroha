use std::{convert::Infallible, num::NonZero, sync::Arc};

use iroha_core::kura::Kura;
use iroha_data_model::block::{stream::BlockStreamMessage, SignedBlock};
use tokio::sync::watch;

use crate::stream::{self, WebSocketScale};

/// Type for any error during blocks streaming
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Stream error")]
    Stream(#[from] stream::Error),
    #[error("Protocol violation: {message}")]
    Protocol { message: String },
}

/// Result type for `Consumer`
pub type Result<T> = core::result::Result<T, Error>;

/// Consumer for Iroha `Block`(s).
/// Passes the blocks over the corresponding connection `stream`.
#[derive(Debug)]
pub struct Consumer<'ws> {
    pub stream: &'ws mut WebSocketScale,
    height: NonZero<usize>,
    kura: Arc<Kura>,
    kura_blocks: watch::Receiver<usize>,
}

impl<'ws> Consumer<'ws> {
    /// Constructs [`Consumer`], which forwards blocks through the `stream`.
    ///
    /// # Errors
    ///
    /// Can fail due to timeout or without message at websocket or during decoding request
    #[iroha_futures::telemetry_future]
    pub async fn new(stream: &'ws mut WebSocketScale, kura: Arc<Kura>) -> Result<Self> {
        let BlockStreamMessage::Subscribe(request) = stream.recv().await? else {
            return Err(Error::Protocol {
                message: "client must send BlockStreamMessage::Subscribe first".to_owned(),
            });
        };
        let kura_blocks = kura.subscribe_blocks_count();
        Ok(Consumer {
            stream,
            height: NonZero::new(request.height().get().try_into().unwrap())
                .expect("created from non-zero"),
            kura,
            kura_blocks,
        })
    }

    pub async fn serve(mut self) -> Result<Infallible> {
        loop {
            let BlockStreamMessage::Next = self.stream.recv().await? else {
                return Err(Error::Protocol {
                    message: "client must send BlockStreamMessage::Next".to_owned(),
                });
            };
            self.wait_and_send_one().await?;
        }
    }

    async fn wait_and_send_one(&mut self) -> Result<()> {
        wait_height(&mut self.kura_blocks, self.height.get()).await;

        let block = self
            .kura
            .get_block(self.height)
            .expect("Kura must have at least the given height");

        self.stream
            .send(BlockStreamMessage::Block(SignedBlock::clone(&block)))
            .await?;

        self.height = self.height.checked_add(1).unwrap();

        Ok(())
    }
}

async fn wait_height(recv: &mut watch::Receiver<usize>, at_least: usize) {
    if *recv.borrow() >= at_least {
        return;
    }

    loop {
        recv.changed()
            .await
            .expect("Kura must not be dropped while stream exists");

        if *recv.borrow_and_update() >= at_least {
            return;
        }
    }
}
