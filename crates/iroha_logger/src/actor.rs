//! Actor encapsulating interaction with logger & telemetry subsystems.
use crate::telemetry;
use iroha_config::logger::Directives;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing_core::Subscriber;
use tracing_subscriber::reload::{self, Error as ReloadError};
/// Handle for communicating with the logging actor.
///
/// `LoggerHandle` is used to dynamically reload log levels and subscribe to
/// telemetry events. Further functionality is tracked at
/// <https://github.com/hyperledger-iroha/iroha/issues/5694>.
#[derive(Clone)]
pub struct LoggerHandle {
    sender: mpsc::Sender<Message>,
}
impl LoggerHandle {
    pub(crate) fn new<S: Subscriber>(
        handle: reload::Handle<tracing_subscriber::filter::EnvFilter, S>,
        telemetry_receiver: mpsc::Receiver<telemetry::ChannelEvent>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let (regular, _) = broadcast::channel(32);
        let (future_forward, _) = broadcast::channel(32);
        let mut actor = LoggerActor {
            message_receiver: rx,
            level_handle: handle,
            telemetry_receiver,
            telemetry_forwarder_regular: regular,
            telemetry_forwarder_future: future_forward,
        };
        tokio::spawn(async move { actor.run().await });
        Self { sender: tx }
    }
    /// Reload the log level filter.
    ///
    /// # Errors
    /// - If reloading on the side of [`reload::Handle`] fails
    /// - If actor communication fails
    pub async fn reload_level(&self, new_value: Directives) -> color_eyre::Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .sender
            .send(Message::ReloadLevel {
                value: new_value,
                respond_to: tx,
            })
            .await;
        Ok(rx.await??)
    }
    /// Subscribe to the telemetry events broadcasting.
    ///
    /// # Errors
    /// If actor communication fails
    pub async fn subscribe_on_telemetry(
        &self,
        channel: telemetry::Channel,
    ) -> color_eyre::Result<broadcast::Receiver<telemetry::Event>, Error> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .sender
            .send(Message::SubscribeOnTelemetry {
                channel,
                respond_to: tx,
            })
            .await;
        Ok(rx.await?)
    }
}
enum Message {
    ReloadLevel {
        value: Directives,
        respond_to: oneshot::Sender<color_eyre::Result<(), ReloadError>>,
    },
    SubscribeOnTelemetry {
        channel: telemetry::Channel,
        respond_to: oneshot::Sender<broadcast::Receiver<telemetry::Event>>,
    },
}
/// Possible errors that might occur while interacting with the actor.
#[derive(Debug)]
pub enum Error {
    /// If dynamic log level reloading failed
    LevelReload(ReloadError),
    /// If actor communication is broken
    Communication(oneshot::error::RecvError),
}
impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::LevelReload(_) => "cannot dynamically reload the log level",
            Self::Communication(_) => "failed to communicate with the actor",
        })
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::LevelReload(error) => error,
            Self::Communication(error) => error,
        })
    }
}
impl From<ReloadError> for Error {
    fn from(error: ReloadError) -> Self {
        Self::LevelReload(error)
    }
}
impl From<oneshot::error::RecvError> for Error {
    fn from(error: oneshot::error::RecvError) -> Self {
        Self::Communication(error)
    }
}
struct LoggerActor<S: Subscriber> {
    message_receiver: mpsc::Receiver<Message>,
    telemetry_receiver: mpsc::Receiver<telemetry::ChannelEvent>,
    telemetry_forwarder_regular: broadcast::Sender<telemetry::Event>,
    telemetry_forwarder_future: broadcast::Sender<telemetry::Event>,
    level_handle: reload::Handle<tracing_subscriber::filter::EnvFilter, S>,
}
impl<S: Subscriber> LoggerActor<S> {
    async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.message_receiver.recv() => {
                    self.handle_message(msg);
                },
                Some(telemetry::ChannelEvent(channel, event)) = self.telemetry_receiver.recv() => {
                    let forward_to = match channel {
                        telemetry::Channel::Regular => &self.telemetry_forwarder_regular,
                        telemetry::Channel::Future => &self.telemetry_forwarder_future,
                    };
                    let _ = forward_to.send(event);
                },
                else => break
            }
            tokio::task::yield_now().await;
        }
    }
    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::ReloadLevel { value, respond_to } => {
                let filter = tracing_subscriber::filter::EnvFilter::try_new(value.to_string())
                    .expect("INTERNAL BUG: Directives is not valid");
                let result = self.level_handle.reload(filter);
                let _ = respond_to.send(result);
            }
            Message::SubscribeOnTelemetry {
                channel: kind,
                respond_to,
            } => {
                let receiver = match kind {
                    telemetry::Channel::Regular => self.telemetry_forwarder_regular.subscribe(),
                    telemetry::Channel::Future => self.telemetry_forwarder_future.subscribe(),
                };
                let _ = respond_to.send(receiver);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error as _, fmt, fmt::Write as _};

    fn reload_error() -> ReloadError {
        let (layer, handle) = reload::Layer::<_, tracing_subscriber::Registry>::new(
            tracing_subscriber::EnvFilter::new("info"),
        );
        drop(layer);
        handle
            .reload(tracing_subscriber::EnvFilter::new("debug"))
            .expect_err("dropped reload layer")
    }

    fn communication_error() -> oneshot::error::RecvError {
        let (sender, receiver) = oneshot::channel::<()>();
        drop(sender);
        receiver.blocking_recv().expect_err("closed oneshot sender")
    }

    #[test]
    fn error_display_preserves_messages_flags_and_original_sources() {
        for (error, expected) in [
            (
                Error::LevelReload(reload_error()),
                "cannot dynamically reload the log level",
            ),
            (
                Error::Communication(communication_error()),
                "failed to communicate with the actor",
            ),
        ] {
            assert_eq!(error.to_string(), expected);
            assert_eq!(format!("{error:>80.3}"), expected);
            assert_eq!(format!("{error:🦀^80.3}"), expected);
            assert_eq!(format!("{error:+#080.3}"), expected);
            let source = error.source().expect("original cause");
            match &error {
                Error::LevelReload(inner) => {
                    assert!(std::ptr::eq(
                        source.downcast_ref::<ReloadError>().unwrap(),
                        inner
                    ));
                    assert!(inner.is_dropped());
                }
                Error::Communication(inner) => assert!(std::ptr::eq(
                    source.downcast_ref::<oneshot::error::RecvError>().unwrap(),
                    inner,
                )),
            }
            assert!(source.source().is_none());
        }
    }

    #[test]
    fn error_from_conversions_preserve_variants_and_public_error_bounds() {
        fn assert_bounds<T: std::error::Error + Send + Sync + 'static>() {}
        assert_bounds::<Error>();
        let Error::LevelReload(error) = Error::from(reload_error()) else {
            panic!("reload error conversion");
        };
        assert!(error.is_dropped());
        assert!(matches!(
            Error::from(communication_error()),
            Error::Communication(_)
        ));
    }

    #[test]
    fn error_display_stops_at_the_first_writer_failure() {
        struct Refuse {
            calls: usize,
        }
        impl fmt::Write for Refuse {
            fn write_str(&mut self, _: &str) -> fmt::Result {
                self.calls += 1;
                assert_eq!(self.calls, 1, "writing continued after failure");
                Err(fmt::Error)
            }
        }
        for error in [
            Error::LevelReload(reload_error()),
            Error::Communication(communication_error()),
        ] {
            let mut writer = Refuse { calls: 0 };
            assert_eq!(write!(writer, "{error}"), Err(fmt::Error));
            assert_eq!(writer.calls, 1);
        }
    }
}
