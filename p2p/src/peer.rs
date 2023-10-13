//! Tokio actor Peer

use bytes::{Buf, BufMut, BytesMut};
use iroha_data_model::prelude::PeerId;
use message::*;
use parity_scale_codec::{DecodeAll, Encode};
use rand::{Rng, RngCore};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::{mpsc, oneshot},
};

use crate::{boilerplate::*, CryptographicError, Error};

/// Max length of message handshake in bytes excluding first message length byte.
pub const MAX_HANDSHAKE_LENGTH: u8 = 255;
/// Default associated data for AEAD
/// [`Authenticated encryption`](https://en.wikipedia.org/wiki/Authenticated_encryption)
pub const DEFAULT_AAD: &[u8; 10] = b"Iroha2 AAD";

pub mod handles {
    //! Module with functions to start peer actor and handle to interact with it.

    use iroha_logger::Instrument;

    use super::{run::RunPeerArgs, *};
    use crate::unbounded_with_len;

    /// Start Peer in [`state::Connecting`] state
    pub fn connecting<T: Pload, K: Kex, E: Enc>(
        peer_id: PeerId,
        connection_id: ConnectionId,
        service_message_sender: mpsc::Sender<ServiceMessage<T>>,
    ) {
        let peer = state::Connecting {
            peer_id,
            connection_id,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Start Peer in [`state::ConnectedFrom`] state
    pub fn connected_from<T: Pload, K: Kex, E: Enc>(
        peer_id: PeerId,
        connection: Connection,
        service_message_sender: mpsc::Sender<ServiceMessage<T>>,
    ) {
        let peer = state::ConnectedFrom {
            peer_id,
            connection,
        };
        let peer = RunPeerArgs {
            peer,
            service_message_sender,
        };
        tokio::task::spawn(run::run::<T, K, E, _>(peer).in_current_span());
    }

    /// Peer actor handle.
    pub struct PeerHandle<T: Pload> {
        // NOTE: it's ok for this channel to be unbounded.
        // Because post messages originate inside the system and their rate is configurable..
        pub(super) post_sender: unbounded_with_len::Sender<T>,
    }

    impl<T: Pload> PeerHandle<T> {
        /// Post message `T` on Peer
        ///
        /// # Errors
        /// Fail if peer terminated
        pub fn post(&self, msg: T) -> Result<(), mpsc::error::SendError<T>> {
            self.post_sender.send(msg)
        }
    }
}

mod run {
    //! Module with peer [`run`] function.

    use iroha_logger::prelude::*;

    use super::{
        cryptographer::Cryptographer,
        handshake::Handshake,
        state::{ConnectedFrom, Connecting, Ready},
        *,
    };
    use crate::{blake2b_hash, unbounded_with_len};

    /// Peer task.
    #[allow(clippy::too_many_lines)]
    #[log(skip_all, fields(conn_id = peer.connection_id(), peer, disambiguator))]
    pub(super) async fn run<T: Pload, K: Kex, E: Enc, P: Entrypoint<K, E>>(
        RunPeerArgs {
            peer,
            service_message_sender,
        }: RunPeerArgs<T, P>,
    ) {
        let conn_id = peer.connection_id();
        let mut peer_id = peer.peer_id().clone();

        iroha_logger::trace!("Peer created");

        // Insure proper termination from every execution path.
        async {
            // Try to do handshake process
            let peer = match peer.handshake().await {
                Ok(ready) => ready,
                Err(error) => {
                    iroha_logger::error!(%error, "Failure during handshake.");
                    return;
                }
            };

            let Ready {
                peer_id: new_peer_id,
                connection:
                    Connection {
                        read,
                        write,
                        id: connection_id,
                    },
                cryptographer,
            } = peer;
            peer_id = new_peer_id;

            let disambiguator = blake2b_hash(&cryptographer.shared_key);

            tracing::Span::current().record("peer", &peer_id.to_string());
            tracing::Span::current().record("disambiguator", disambiguator);

            let (post_sender, mut post_receiver) = unbounded_with_len::unbounded_channel();
            let (peer_message_sender, peer_message_receiver) = oneshot::channel();
            let ready_peer_handle = handles::PeerHandle { post_sender };
            if service_message_sender
                .send(ServiceMessage::Connected(Connected {
                    connection_id,
                    peer_id: peer_id.clone(),
                    ready_peer_handle,
                    peer_message_sender,
                    disambiguator,
                }))
                .await
                .is_err()
            {
                iroha_logger::error!(
                    "Peer is ready, but network dropped connection sender."
                );
                return;
            }
            let Ok(peer_message_sender) = peer_message_receiver.await else {
                // NOTE: this is not considered as error, because network might decide not to connect peer.
                iroha_logger::debug!(
                    "Network decide not to connect peer."
                );
                return;
            };

            iroha_logger::trace!("Peer connected");

            let mut message_reader = MessageReader::new(read, cryptographer.clone());
            let mut message_sender = MessageSender::new(write, cryptographer);

            loop {
                tokio::select! {
                    msg = post_receiver.recv() => {
                        let Some(msg) = msg else {
                            iroha_logger::debug!("Peer handle dropped.");
                            break;
                        };
                        iroha_logger::trace!("Post message");
                        let post_receiver_len = post_receiver.len();
                        if post_receiver_len > 100 {
                            iroha_logger::warn!(size=post_receiver_len, "Peer post messages are pilling up");
                        }
                        if let Err(error) = message_sender.send_message(msg).await {
                            iroha_logger::error!(%error, "Failed to send message to peer.");
                            break;
                        }
                    }
                    msg = message_reader.read_message() => {
                        let msg = match msg {
                            Ok(Some(msg)) => {
                                msg
                            },
                            Ok(None) => {
                                iroha_logger::debug!("Peer send whole message and close connection");
                                break;
                            }
                            Err(error) => {
                                iroha_logger::error!(%error, "Error while reading message from peer.");
                                break;
                            }
                        };
                        iroha_logger::trace!("Received peer message");
                        let peer_message = PeerMessage(peer_id.clone(), msg);
                        if peer_message_sender.send(peer_message).await.is_err() {
                            iroha_logger::error!("Network dropped peer message channel.");
                            break;
                        }
                    }
                    else => break,
                }
                tokio::task::yield_now().await;
            }
        }.await;

        iroha_logger::debug!("Peer is terminated.");
        let _ = service_message_sender
            .send(ServiceMessage::Terminated(Terminated { peer_id, conn_id }))
            .await;
    }

    /// Args to pass inside [`run`] function.
    pub(super) struct RunPeerArgs<T: Pload, P> {
        pub peer: P,
        pub service_message_sender: mpsc::Sender<ServiceMessage<T>>,
    }

    /// Trait for peer stages that might be used as starting point for peer's [`run`] function.
    pub(super) trait Entrypoint<K: Kex, E: Enc>: Handshake<K, E> + Send + 'static {
        fn connection_id(&self) -> ConnectionId;

        fn peer_id(&self) -> &PeerId;
    }

    impl<K: Kex, E: Enc> Entrypoint<K, E> for Connecting {
        fn connection_id(&self) -> ConnectionId {
            self.connection_id
        }

        fn peer_id(&self) -> &PeerId {
            &self.peer_id
        }
    }

    impl<K: Kex, E: Enc> Entrypoint<K, E> for ConnectedFrom {
        fn connection_id(&self) -> ConnectionId {
            self.connection.id
        }

        fn peer_id(&self) -> &PeerId {
            &self.peer_id
        }
    }

    /// Cancellation-safe way to read messages from tcp stream
    struct MessageReader<E: Enc> {
        read: OwnedReadHalf,
        buffer: bytes::BytesMut,
        cryptographer: Cryptographer<E>,
    }

    impl<E: Enc> MessageReader<E> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();

        fn new(read: OwnedReadHalf, cryptographer: Cryptographer<E>) -> Self {
            Self {
                read,
                cryptographer,
                // TODO: eyeball decision of default buffer size of 1 KB, should be benchmarked and optimized
                buffer: BytesMut::with_capacity(1024),
            }
        }

        /// Read message by first reading it's size as u32 and then rest of the message
        ///
        /// # Errors
        /// - Fail in case reading from stream fails
        /// - Connection is closed by there is still unfinished message in buffer
        /// - Forward errors from [`Self::parse_message`]
        async fn read_message<T: Pload>(&mut self) -> Result<Option<T>, Error> {
            loop {
                // Try to get full message
                if let Some(msg) = self.parse_message()? {
                    return Ok(Some(msg));
                }

                if 0 == self.read.read_buf(&mut self.buffer).await? {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    return Err(Error::ConnectionResetByPeer);
                }
            }
        }

        /// Parse message
        ///
        /// # Errors
        /// - Fail to decrypt message
        /// - Fail to decode message
        fn parse_message<T: Pload>(&mut self) -> Result<Option<T>, Error> {
            let mut buf = &self.buffer[..];
            if buf.remaining() < Self::U32_SIZE {
                // Not enough data to read u32
                return Ok(None);
            }
            let size = buf.get_u32() as usize;
            if buf.remaining() < size {
                // Not enough data to read the whole data
                return Ok(None);
            }

            let data = &buf[..size];
            let decrypted = self.cryptographer.decrypt(data)?;
            let decoded = DecodeAll::decode_all(&mut decrypted.as_slice())?;

            self.buffer.advance(size + Self::U32_SIZE);

            Ok(Some(decoded))
        }
    }

    struct MessageSender<E: Enc> {
        write: OwnedWriteHalf,
        cryptographer: Cryptographer<E>,
        buffer: BytesMut,
    }

    impl<E: Enc> MessageSender<E> {
        const U32_SIZE: usize = core::mem::size_of::<u32>();

        fn new(write: OwnedWriteHalf, cryptographer: Cryptographer<E>) -> Self {
            Self {
                write,
                cryptographer,
                // TODO: eyeball decision of default buffer size of 1 KB, should be benchmarked and optimized
                buffer: BytesMut::with_capacity(1024),
            }
        }

        /// Send byte-encoded message to the peer
        ///
        /// # Errors
        /// - If encryption fail.
        /// - If write to `stream` fail.
        async fn send_message<T: Pload>(&mut self, msg: T) -> Result<(), Error> {
            // Start with fresh buffer
            self.buffer.clear();
            let mut writer = (&mut self.buffer).writer();
            msg.encode_to(&mut writer);
            let encoded_size = self.buffer.remaining();
            let encrypted = self.cryptographer.encrypt(&self.buffer[..encoded_size])?;
            self.buffer.advance(encoded_size);
            assert!(
                !self.buffer.has_remaining(),
                "Buffer must be empty at this point"
            );
            let encrypted_size = encrypted.len();
            #[allow(clippy::cast_possible_truncation)]
            self.buffer.put_u32(encrypted_size as u32);
            self.buffer.put_slice(encrypted.as_slice());
            self.write.write_all(&self.buffer[..]).await?;
            self.write.flush().await?;
            self.buffer.advance(encrypted_size + Self::U32_SIZE);
            assert!(
                !self.buffer.has_remaining(),
                "Buffer must be empty at this point"
            );
            Ok(())
        }
    }
}

mod state {
    //! Module for peer stages.

    use iroha_crypto::ursa::keys::PublicKey;

    use super::{cryptographer::Cryptographer, *};

    /// Peer that is connecting. This is the initial stage of a new
    /// outgoing peer.
    pub(super) struct Connecting {
        pub peer_id: PeerId,
        pub connection_id: ConnectionId,
    }

    impl Connecting {
        pub(super) async fn connect_to(
            Self {
                peer_id,
                connection_id,
            }: Self,
        ) -> Result<ConnectedTo, crate::Error> {
            let stream = TcpStream::connect(peer_id.address.to_string()).await?;
            let connection = Connection::new(connection_id, stream);
            Ok(ConnectedTo {
                peer_id,
                connection,
            })
        }
    }

    /// Peer that is being connected to.
    pub(super) struct ConnectedTo {
        peer_id: PeerId,
        connection: Connection,
    }

    impl ConnectedTo {
        pub(super) async fn send_client_hello<K: Kex, E: Enc>(
            Self {
                peer_id,
                mut connection,
            }: Self,
        ) -> Result<SendKey<E>, crate::Error> {
            let key_exchange = K::new();
            let (local_public_key, local_private_key) = key_exchange.keypair(None)?;
            let write_half = &mut connection.write;
            garbage::write(write_half).await?;
            write_half.write_all(local_public_key.as_ref()).await?;
            // Read server hello with node's public key
            let read_half = &mut connection.read;
            let remote_public_key = {
                garbage::read(read_half).await?;
                // Then we have servers public key
                let mut key = vec![0_u8; 32];
                let _ = read_half.read_exact(&mut key).await?;
                PublicKey(key)
            };
            let shared_key =
                key_exchange.compute_shared_secret(&local_private_key, &remote_public_key)?;
            let cryptographer = Cryptographer::new(shared_key);
            Ok(SendKey {
                peer_id,
                connection,
                cryptographer,
            })
        }
    }

    /// Peer that is being connected from
    pub(super) struct ConnectedFrom {
        pub peer_id: PeerId,
        pub connection: Connection,
    }

    impl ConnectedFrom {
        pub(super) async fn read_client_hello<K: Kex, E: Enc>(
            Self {
                peer_id,
                mut connection,
                ..
            }: Self,
        ) -> Result<SendKey<E>, crate::Error> {
            let key_exchange = K::new();
            let (local_public_key, local_private_key) = key_exchange.keypair(None)?;
            let read_half = &mut connection.read;
            let remote_public_key = {
                garbage::read(read_half).await?;
                // And then we have clients public key
                let mut key = vec![0_u8; 32];
                let _ = read_half.read_exact(&mut key).await?;
                PublicKey(key)
            };
            let write_half = &mut connection.write;
            garbage::write(write_half).await?;
            write_half.write_all(local_public_key.as_ref()).await?;
            let shared_key =
                key_exchange.compute_shared_secret(&local_private_key, &remote_public_key)?;
            let cryptographer = Cryptographer::new(shared_key);
            Ok(SendKey {
                peer_id,
                connection,
                cryptographer,
            })
        }
    }

    /// Peer that needs to send key.
    pub(super) struct SendKey<E: Enc> {
        peer_id: PeerId,
        connection: Connection,
        cryptographer: Cryptographer<E>,
    }

    impl<E: Enc> SendKey<E> {
        pub(super) async fn send_our_public_key(
            Self {
                peer_id,
                mut connection,
                cryptographer,
            }: Self,
        ) -> Result<GetKey<E>, crate::Error> {
            let write_half = &mut connection.write;

            // We take our public key from our `id` and will replace it with theirs when we read it
            // Packing length and message in one network packet for efficiency
            let data = peer_id.public_key().encode();

            let data = &cryptographer.encrypt(data.as_slice())?;

            let mut buf = Vec::<u8>::with_capacity(data.len() + 1);
            #[allow(clippy::cast_possible_truncation)]
            buf.push(data.len() as u8);
            buf.extend_from_slice(data.as_slice());

            write_half.write_all(&buf).await?;
            Ok(GetKey {
                peer_id,
                connection,
                cryptographer,
            })
        }
    }

    /// Peer that needs to get key.
    pub struct GetKey<E: Enc> {
        peer_id: PeerId,
        connection: Connection,
        cryptographer: Cryptographer<E>,
    }

    impl<E: Enc> GetKey<E> {
        /// Read the peer's public key
        pub(super) async fn read_their_public_key(
            Self {
                mut peer_id,
                mut connection,
                cryptographer,
            }: Self,
        ) -> Result<Ready<E>, crate::Error> {
            let read_half = &mut connection.read;
            let size = read_half.read_u8().await? as usize;
            // Reading public key
            let mut data = vec![0_u8; size];
            let _ = read_half.read_exact(&mut data).await?;

            let data = cryptographer.decrypt(data.as_slice())?;

            let pub_key = DecodeAll::decode_all(&mut data.as_slice())?;

            peer_id.public_key = pub_key;
            Ok(Ready {
                peer_id,
                connection,
                cryptographer,
            })
        }
    }

    /// Peer that is ready for communication after finishing the
    /// handshake process.
    pub(super) struct Ready<E: Enc> {
        pub peer_id: PeerId,
        pub connection: Connection,
        pub cryptographer: Cryptographer<E>,
    }
}

mod handshake {
    //! Implementations of the handshake process.

    use async_trait::async_trait;

    use super::{state::*, *};

    #[async_trait]
    pub(super) trait Stage<K: Kex, E: Enc> {
        type NextStage;

        async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error>;
    }

    macro_rules! stage {
        ( $func:ident : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func(self).await ; $curstage => $nextstage);
        };
        ( $func:ident :: <$($generic_param:ident),+> : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func::<$($generic_param),+>(self).await ; $curstage => $nextstage);
        };
        // Internal case
        (@base $self:ident $call:expr ; $curstage:ty => $nextstage:ty ) => {
            #[async_trait]
            impl<K: Kex, E: Enc> Stage<K, E> for $curstage {
                type NextStage = $nextstage;

                async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error> {
                    // NOTE: Need this due to macro hygiene
                    let $self = self;
                    $call
                }
            }
        }
    }

    stage!(connect_to: Connecting => ConnectedTo);
    stage!(send_client_hello::<K, E>: ConnectedTo => SendKey<E>);
    stage!(read_client_hello::<K, E>: ConnectedFrom => SendKey<E>);
    stage!(send_our_public_key: SendKey<E> => GetKey<E>);
    stage!(read_their_public_key: GetKey<E> => Ready<E>);

    #[async_trait]
    pub(super) trait Handshake<K: Kex, E: Enc> {
        async fn handshake(self) -> Result<Ready<E>, crate::Error>;
    }

    macro_rules! impl_handshake {
        ( base_case $typ:ty ) => {
            // Base case, should be all states that lead to `Ready`
            #[async_trait]
            impl<K: Kex, E: Enc> Handshake<K, E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    <$typ as Stage<K, E>>::advance_to_next_stage(self).await
                }
            }
        };
        ( $typ:ty ) => {
            #[async_trait]
            impl<K: Kex, E: Enc> Handshake<K, E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    let next_stage = <$typ as Stage<K, E>>::advance_to_next_stage(self).await?;
                    <_ as Handshake<K, E>>::handshake(next_stage).await
                }
            }
        };
    }

    impl_handshake!(base_case GetKey<E>);
    impl_handshake!(SendKey<E>);
    impl_handshake!(ConnectedFrom);
    impl_handshake!(ConnectedTo);
    impl_handshake!(Connecting);
}

pub mod message {
    //! Module for peer messages

    use super::*;

    /// Connection and Handshake was successful
    pub struct Connected<T: Pload> {
        /// Peer Id
        pub peer_id: PeerId,
        /// Connection Id
        pub connection_id: ConnectionId,
        /// Handle for peer to send messages and terminate command
        pub ready_peer_handle: handles::PeerHandle<T>,
        /// Channel to send peer messages channel
        pub peer_message_sender: oneshot::Sender<mpsc::Sender<PeerMessage<T>>>,
        /// Disambiguator of connection (equal for both peers)
        pub disambiguator: u64,
    }

    /// Messages received from Peer
    pub struct PeerMessage<T: Pload>(pub PeerId, pub T);

    /// Peer faced error or `Terminate` message, send to indicate that it is terminated
    pub struct Terminated {
        /// Peer Id
        pub peer_id: PeerId,
        /// Connection Id
        pub conn_id: ConnectionId,
    }

    /// Messages sent by peer during connection process
    pub enum ServiceMessage<T: Pload> {
        /// Connection and Handshake was successful
        Connected(Connected<T>),
        /// Peer faced error or `Terminate` message, send to indicate that it is terminated
        Terminated(Terminated),
    }
}

mod cryptographer {
    use aead::{generic_array::GenericArray, NewAead};
    use iroha_crypto::ursa::{encryption::symm::SymmetricEncryptor, keys::SessionKey};

    use super::*;

    /// Peer's cryptographic primitives
    pub struct Cryptographer<E: Enc> {
        /// Shared key
        pub shared_key: SessionKey,
        /// Encryptor created from session key, that we got by Diffie-Hellman scheme
        pub encryptor: SymmetricEncryptor<E>,
    }

    impl<E: Enc> Cryptographer<E> {
        /// Decrypt bytes.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy`] error
        pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            self.encryptor
                .decrypt_easy(DEFAULT_AAD.as_ref(), data)
                .map_err(CryptographicError::Decrypt)
                .map_err(Into::into)
        }

        /// Encrypt bytes.
        ///
        /// # Errors
        /// Forwards [`SymmetricEncryptor::decrypt_easy`] error
        pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
            self.encryptor
                .encrypt_easy(DEFAULT_AAD.as_ref(), data)
                .map_err(CryptographicError::Encrypt)
                .map_err(Into::into)
        }

        /// Derives shared key from local private key and remote public key.
        pub fn new(shared_key: SessionKey) -> Self {
            let encryptor = SymmetricEncryptor::<E>::new(<E as NewAead>::new(
                GenericArray::from_slice(shared_key.as_ref()),
            ));
            Self {
                shared_key,
                encryptor,
            }
        }
    }

    impl<E: Enc> Clone for Cryptographer<E> {
        fn clone(&self) -> Self {
            Self::new(self.shared_key.clone())
        }
    }
}

/// An identification for [`Peer`] connections.
pub type ConnectionId = u64;

/// P2P connection
#[derive(Debug)]
pub struct Connection {
    /// A unique connection id
    pub id: ConnectionId,
    /// Reading half of `TcpStream`
    pub read: OwnedReadHalf,
    /// Writing half of `TcpStream`
    pub write: OwnedWriteHalf,
}

impl Connection {
    /// Instantiate new connection from `connection_id` and `stream`.
    pub fn new(id: ConnectionId, stream: TcpStream) -> Self {
        let (read, write) = stream.into_split();
        Connection { id, read, write }
    }
}

mod garbage {
    //! Module with functions to read and write garbage.
    // TODO: why do we need this?

    use super::*;

    /// Generate random garbage bytes and writes then to the stream.
    pub(super) async fn write(stream: &mut OwnedWriteHalf) -> Result<(), Error> {
        let size;
        // Additional byte for the length of the garbage to send everything in one go
        let mut garbage = [0u8; MAX_HANDSHAKE_LENGTH as usize + 1];
        {
            let rng = &mut rand::thread_rng();
            size = rng.gen_range(64..=MAX_HANDSHAKE_LENGTH);
            garbage[0] = size;
            rng.fill_bytes(&mut garbage[1..=(size as usize)]);
        }
        iroha_logger::trace!(size, "Writing garbage");
        stream.write_all(&garbage[..=(size as usize)]).await?;
        Ok(())
    }

    /// Read and discards random garbage bytes from the stream.
    pub(super) async fn read(stream: &mut OwnedReadHalf) -> Result<(), Error> {
        let size = stream.read_u8().await? as usize;
        iroha_logger::trace!(size, "Reading garbage");
        let mut garbage = [0u8; MAX_HANDSHAKE_LENGTH as usize];
        let _ = stream.read_exact(&mut garbage[..size]).await?;
        Ok(())
    }
}
