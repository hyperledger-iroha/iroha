//! Account-authorized event and block subscriptions over an injected transport.

use std::{
    num::NonZeroU64,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream};

use super::{AccountClient, Client, NORITO_V1_WEBSOCKET_SUBPROTOCOL, blocks_api, events_api};
use crate::{
    Error, Result,
    data_model::{
        block::SignedBlock,
        events::{EventBox, EventFilterBox},
    },
    http::ws::conn_flow::{Events as _, Init, InitData},
    http_default::DefaultWebSocketRequestBuilder,
    stream::{StreamFrame, StreamRequest, StreamSocket},
};

const SUBSCRIPTION_MAX_BYTES: usize = 256 * 1024;
const MESSAGE_MAX_BYTES: usize = 64 * 1024 * 1024;
const UPGRADE_MAX_BYTES: usize = 64 * 1024;
const DEFAULT_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const EVENTS_OPERATION: &str = "events.stream_websocket";
pub(super) const BLOCKS_OPERATION: &str = "blocks.stream_websocket";

pub(super) fn encode_subscription<T: norito::NoritoSerialize>(
    value: &T,
    operation: &'static str,
) -> Result<Vec<u8>> {
    norito::core::to_bytes_bounded(value, SUBSCRIPTION_MAX_BYTES).map_err(|error| {
        Error::InvalidRequest {
            operation,
            details: format!("failed to encode bounded stream subscription: {error}"),
        }
    })
}

/// Account-scoped event subscriptions.
///
/// ```compile_fail
/// fn public_context(client: &iroha::client::Client) {
///     let _ = client.events();
/// }
/// ```
pub struct Events<'a> {
    account: &'a AccountClient,
}

/// Full signed-block subscriptions; Torii requires `CanReadAllLedgerData`.
///
/// ```compile_fail
/// fn operator_context(client: &iroha::client::OperatorClient) {
///     let _ = client.blocks();
/// }
/// ```
pub struct Blocks<'a> {
    account: &'a AccountClient,
}

impl AccountClient {
    /// Access event subscriptions using this exact account authority.
    #[must_use]
    pub fn events(&self) -> Events<'_> {
        Events { account: self }
    }

    /// Access full-block subscriptions using this exact account authority.
    #[must_use]
    pub fn blocks(&self) -> Blocks<'_> {
        Blocks { account: self }
    }
}

impl Events<'_> {
    /// Subscribe once to the supplied nonempty filters.
    ///
    /// The connection and initial subscription share the context request
    /// deadline. No reconnect or replay occurs automatically. The returned
    /// stream owns its connection and can outlive this account context.
    ///
    /// # Errors
    /// Returns signing, invalid-filter, transport, timeout or protocol errors.
    pub async fn subscribe(
        self,
        filters: impl IntoIterator<Item = impl Into<EventFilterBox>> + Send,
    ) -> Result<EventStream> {
        let init = self.account.context.events_handler(filters)?;
        connect(&self.account.context, EVENTS_OPERATION, init, |bytes| {
            events_api::flow::Events.message(bytes)
        })
        .await
    }
}

impl Blocks<'_> {
    /// Subscribe once from the exact nonzero block height.
    ///
    /// Torii authenticates the account and its global-reader permission. The
    /// SDK does not reconnect or change the requested height automatically.
    ///
    /// # Errors
    /// Returns signing, transport, timeout, authorization or protocol errors.
    pub async fn subscribe(self, height: NonZeroU64) -> Result<BlockStream> {
        let init = self.account.context.blocks_handler(height)?;
        connect(&self.account.context, BLOCKS_OPERATION, init, |bytes| {
            blocks_api::flow::Events.message(bytes)
        })
        .await
    }
}

/// Owned stream of canonical ledger events.
pub type EventStream = SubscriptionStream<EventBox>;
/// Owned stream of canonical signed blocks.
pub type BlockStream = SubscriptionStream<SignedBlock>;

/// A bounded, terminal-on-error subscription with explicit asynchronous close.
///
/// Dropping the stream releases its connection without spawning background work.
/// Use [`Self::close`] to flush a close frame under a bounded deadline.
pub struct SubscriptionStream<T> {
    socket: Box<dyn StreamSocket>,
    decode: fn(Vec<u8>) -> eyre::Result<T>,
    operation: &'static str,
    maximum: usize,
    close_timeout: Duration,
    terminated: bool,
    last_message_bytes: Option<usize>,
}

impl<T> SubscriptionStream<T> {
    /// Exact received binary-message length for the last delivered item.
    ///
    /// This is `None` before the first item and after a transport or abnormal
    /// close error. Binary messages record their received length even if decoding
    /// or the message-size check fails. Pending polls and normal end-of-stream
    /// preserve the previous observation. No decoded value is re-encoded.
    #[must_use]
    pub const fn last_message_bytes(&self) -> Option<usize> {
        self.last_message_bytes
    }

    /// Send the close handshake and release the connection.
    ///
    /// # Errors
    /// Returns a typed close failure or deadline expiry. With a disabled request
    /// deadline, closing still has a five-second resource-release deadline.
    pub async fn close(mut self) -> Result<()> {
        tokio::time::timeout(self.close_timeout, self.socket.close())
            .await
            .map_err(|_| Error::Timeout {
                operation: self.operation,
            })?
    }
}

impl<T> Stream for SubscriptionStream<T> {
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        let frame = futures_util::ready!(Pin::new(&mut self.socket).poll_next(cx));
        match &frame {
            Some(Ok(StreamFrame::Binary(bytes))) => self.last_message_bytes = Some(bytes.len()),
            Some(Ok(StreamFrame::Close {
                code: Some(1000), ..
            })) => {}
            _ => self.last_message_bytes = None,
        }
        let result = match frame {
            Some(Ok(StreamFrame::Binary(bytes))) if bytes.len() > self.maximum => {
                Some(Err(Error::ResponseTooLarge {
                    maximum: self.maximum,
                    actual: Some(bytes.len()),
                }))
            }
            Some(Ok(StreamFrame::Binary(bytes))) => {
                Some((self.decode)(bytes).map_err(|error| Error::Decode {
                    operation: self.operation,
                    details: error.to_string(),
                }))
            }
            Some(Ok(StreamFrame::Close {
                code: Some(1000), ..
            })) => None,
            Some(Ok(StreamFrame::Close { code, reason })) => Some(Err(Error::StreamClosed {
                operation: self.operation,
                code,
                reason,
            })),
            Some(Err(error)) => Some(Err(error)),
            None => Some(Err(Error::StreamClosed {
                operation: self.operation,
                code: None,
                reason: "connection ended without a close disposition".to_owned(),
            })),
        };
        if !matches!(&result, Some(Ok(_))) {
            self.terminated = true;
        }
        Poll::Ready(result)
    }
}

fn validate_upgrade(operation: &'static str, response: &http::Response<Vec<u8>>) -> Result<()> {
    if response.body().len() > UPGRADE_MAX_BYTES {
        return Err(Error::ResponseTooLarge {
            maximum: UPGRADE_MAX_BYTES,
            actual: Some(response.body().len()),
        });
    }
    if response.status() != http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(Error::Http {
            operation,
            status: response.status().as_u16(),
            retry_after: crate::error::retry_after(response.headers()),
            body: response.body().clone(),
        });
    }
    let mut values = response
        .headers()
        .get_all(http::header::SEC_WEBSOCKET_PROTOCOL)
        .iter();
    if values.next().and_then(|value| value.to_str().ok()) != Some(NORITO_V1_WEBSOCKET_SUBPROTOCOL)
        || values.next().is_some()
    {
        return Err(Error::StreamProtocol {
            operation,
            details: format!(
                "Torii WebSocket did not select required subprotocol `{NORITO_V1_WEBSOCKET_SUBPROTOCOL}` exactly once"
            ),
        });
    }
    Ok(())
}

async fn connect<T, I: Init<DefaultWebSocketRequestBuilder>>(
    client: &Client,
    operation: &'static str,
    init: I,
    decode: fn(Vec<u8>) -> eyre::Result<T>,
) -> Result<SubscriptionStream<T>> {
    let InitData { first_message, req } = init.init()?;
    let request = req.build().map_err(|error| Error::InvalidRequest {
        operation,
        details: error.to_string(),
    })?;
    let timeout = client.torii_request_timeout;
    let subscribe = async {
        let mut connection = client
            .stream_transport
            .connect(StreamRequest {
                request,
                operation,
                max_message_bytes: MESSAGE_MAX_BYTES,
                timeout,
            })
            .await?;
        validate_upgrade(operation, &connection.response)?;
        connection.socket.send(first_message).await?;
        Ok(SubscriptionStream {
            socket: connection.socket,
            decode,
            operation,
            maximum: MESSAGE_MAX_BYTES,
            close_timeout: if timeout.is_zero() {
                DEFAULT_CLOSE_TIMEOUT
            } else {
                timeout
            },
            terminated: false,
            last_message_bytes: None,
        })
    };
    if timeout.is_zero() {
        subscribe.await
    } else {
        tokio::time::timeout(timeout, subscribe)
            .await
            .map_err(|_| Error::Timeout { operation })?
    }
}

#[cfg(test)]
mod tests;
