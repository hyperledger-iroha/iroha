//! Consumer-side Tokio stream adapters for the `SoraNet` record protocol.
/// Define Tokio record reader/writer adapters inside one consumer-owned module.
///
/// The adapter implementation expands in a runtime crate that already owns
/// Tokio. This keeps the cryptographic record layer runtime-agnostic and avoids
/// changing the locked dependency surface merely to provide convenience I/O.
#[macro_export]
macro_rules! define_soranet_record_io_adapters {
    ($module:ident) => {
        mod $module {
            use ::std::{
                cmp,
                io::{self, ErrorKind},
                pin::Pin,
                task::{Context, Poll, ready},
            };
            use ::tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
            use $crate::soranet::record::__RecordZeroize as _;
            use $crate::soranet::record::{
                MAX_RECORD_PLAINTEXT_LEN, RECORD_HEADER_LEN, RecordOpener, RecordSealer,
            };
            /// Async writer that emits bounded authenticated records.
            pub struct RecordWriter<W> {
                inner: W,
                sealer: RecordSealer,
                pending: Vec<u8>,
                pending_offset: usize,
                failed: bool,
            }
            impl<W> RecordWriter<W> {
                /// Wrap an async writer with record protection.
                #[must_use]
                pub fn new(inner: W, sealer: RecordSealer) -> Self {
                    Self {
                        inner,
                        sealer,
                        pending: Vec::new(),
                        pending_offset: 0,
                        failed: false,
                    }
                }
                /// Borrow the underlying writer.
                #[must_use]
                pub fn get_ref(&self) -> &W {
                    &self.inner
                }
            }
            impl<W: AsyncWrite + Unpin> RecordWriter<W> {
                fn poll_drain_pending(
                    &mut self,
                    context: &mut Context<'_>,
                ) -> Poll<io::Result<()>> {
                    if self.failed {
                        return Poll::Ready(Err(io::Error::new(
                            ErrorKind::BrokenPipe,
                            "SoraNet record writer is in a failed state",
                        )));
                    }
                    while self.pending_offset < self.pending.len() {
                        let written = match ready!(
                            Pin::new(&mut self.inner)
                                .poll_write(context, &self.pending[self.pending_offset..])
                        ) {
                            Ok(written) => written,
                            Err(error) => {
                                self.failed = true;
                                return Poll::Ready(Err(error));
                            }
                        };
                        if written == 0 {
                            self.failed = true;
                            return Poll::Ready(Err(io::Error::new(
                                ErrorKind::WriteZero,
                                "record transport accepted zero bytes",
                            )));
                        }
                        self.pending_offset += written;
                    }
                    self.pending.zeroize();
                    self.pending.clear();
                    self.pending_offset = 0;
                    Poll::Ready(Ok(()))
                }
            }
            impl<W> Drop for RecordWriter<W> {
                fn drop(&mut self) {
                    self.pending.zeroize();
                }
            }
            impl<W: AsyncWrite + Unpin> AsyncWrite for RecordWriter<W> {
                fn poll_write(
                    self: Pin<&mut Self>,
                    context: &mut Context<'_>,
                    plaintext: &[u8],
                ) -> Poll<io::Result<usize>> {
                    let this = self.get_mut();
                    ready!(this.poll_drain_pending(context))?;
                    if plaintext.is_empty() {
                        return Poll::Ready(Ok(0));
                    }
                    let accepted = cmp::min(plaintext.len(), MAX_RECORD_PLAINTEXT_LEN);
                    if let Err(error) = this
                        .sealer
                        .seal_into(&plaintext[..accepted], &mut this.pending)
                    {
                        this.failed = true;
                        return Poll::Ready(Err(io::Error::new(ErrorKind::InvalidData, error)));
                    }
                    // Like a bounded buffered writer, accepting plaintext only commits it
                    // to this writer. The next write, flush, or shutdown drains the complete
                    // authenticated record. This makes a cancelled `poll_write` unambiguous:
                    // Pending is returned only while draining data accepted by an earlier
                    // successful call.
                    Poll::Ready(Ok(accepted))
                }
                fn poll_flush(
                    self: Pin<&mut Self>,
                    context: &mut Context<'_>,
                ) -> Poll<io::Result<()>> {
                    let this = self.get_mut();
                    ready!(this.poll_drain_pending(context))?;
                    match ready!(Pin::new(&mut this.inner).poll_flush(context)) {
                        Ok(()) => Poll::Ready(Ok(())),
                        Err(error) => {
                            this.failed = true;
                            Poll::Ready(Err(error))
                        }
                    }
                }
                fn poll_shutdown(
                    self: Pin<&mut Self>,
                    context: &mut Context<'_>,
                ) -> Poll<io::Result<()>> {
                    let this = self.get_mut();
                    ready!(this.poll_drain_pending(context))?;
                    match ready!(Pin::new(&mut this.inner).poll_shutdown(context)) {
                        Ok(()) => Poll::Ready(Ok(())),
                        Err(error) => {
                            this.failed = true;
                            Poll::Ready(Err(error))
                        }
                    }
                }
            }
            /// Async reader that authenticates records before exposing plaintext.
            pub struct RecordReader<R> {
                inner: R,
                opener: RecordOpener,
                header: [u8; RECORD_HEADER_LEN],
                header_offset: usize,
                ciphertext: Vec<u8>,
                ciphertext_offset: usize,
                plaintext: Vec<u8>,
                plaintext_offset: usize,
                body_len: Option<usize>,
                eof: bool,
                failed: bool,
            }
            impl<R> RecordReader<R> {
                /// Wrap an async reader with record authentication.
                #[must_use]
                pub fn new(inner: R, opener: RecordOpener) -> Self {
                    Self {
                        inner,
                        opener,
                        header: [0_u8; RECORD_HEADER_LEN],
                        header_offset: 0,
                        ciphertext: Vec::new(),
                        ciphertext_offset: 0,
                        plaintext: Vec::new(),
                        plaintext_offset: 0,
                        body_len: None,
                        eof: false,
                        failed: false,
                    }
                }
                /// Borrow the underlying reader.
                #[must_use]
                pub fn get_ref(&self) -> &R {
                    &self.inner
                }
                fn fail(
                    &mut self,
                    kind: ErrorKind,
                    message: impl Into<Box<dyn std::error::Error + Send + Sync>>,
                ) -> io::Error {
                    self.failed = true;
                    io::Error::new(kind, message)
                }
            }
            impl<R> Drop for RecordReader<R> {
                fn drop(&mut self) {
                    self.header.zeroize();
                    self.ciphertext.zeroize();
                    self.plaintext.zeroize();
                }
            }
            impl<R: AsyncRead + Unpin> AsyncRead for RecordReader<R> {
                fn poll_read(
                    self: Pin<&mut Self>,
                    context: &mut Context<'_>,
                    output: &mut ReadBuf<'_>,
                ) -> Poll<io::Result<()>> {
                    let this = self.get_mut();
                    if output.remaining() == 0 {
                        return Poll::Ready(Ok(()));
                    }
                    if this.failed {
                        return Poll::Ready(Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "SoraNet record reader is in a failed state",
                        )));
                    }
                    loop {
                        if this.plaintext_offset < this.plaintext.len() {
                            let available = &this.plaintext[this.plaintext_offset..];
                            let copied = cmp::min(available.len(), output.remaining());
                            output.put_slice(&available[..copied]);
                            this.plaintext_offset += copied;
                            if this.plaintext_offset == this.plaintext.len() {
                                this.plaintext.zeroize();
                                this.plaintext.clear();
                                this.plaintext_offset = 0;
                            }
                            return Poll::Ready(Ok(()));
                        }
                        if this.eof {
                            return Poll::Ready(Ok(()));
                        }
                        while this.header_offset < RECORD_HEADER_LEN {
                            let before = this.header_offset;
                            let mut target = ReadBuf::new(&mut this.header[before..]);
                            ready!(Pin::new(&mut this.inner).poll_read(context, &mut target))?;
                            let read = target.filled().len();
                            if read == 0 {
                                if before == 0 {
                                    this.eof = true;
                                    return Poll::Ready(Ok(()));
                                }
                                return Poll::Ready(Err(this.fail(
                                    ErrorKind::UnexpectedEof,
                                    "SoraNet record header ended prematurely",
                                )));
                            }
                            this.header_offset += read;
                        }
                        if this.body_len.is_none() {
                            let body_len = match this.opener.ciphertext_len(&this.header) {
                                Ok(length) => length,
                                Err(error) => {
                                    return Poll::Ready(Err(
                                        this.fail(ErrorKind::InvalidData, error)
                                    ));
                                }
                            };
                            this.ciphertext.clear();
                            this.ciphertext.resize(body_len, 0);
                            this.ciphertext_offset = 0;
                            this.body_len = Some(body_len);
                        }
                        let body_len = this.body_len.expect("record body length initialized");
                        while this.ciphertext_offset < body_len {
                            let before = this.ciphertext_offset;
                            let mut target = ReadBuf::new(&mut this.ciphertext[before..]);
                            ready!(Pin::new(&mut this.inner).poll_read(context, &mut target))?;
                            let read = target.filled().len();
                            if read == 0 {
                                return Poll::Ready(Err(this.fail(
                                    ErrorKind::UnexpectedEof,
                                    "SoraNet record body ended prematurely",
                                )));
                            }
                            this.ciphertext_offset += read;
                        }
                        let opened = this.opener.open_parts_into(
                            &this.header,
                            &this.ciphertext,
                            &mut this.plaintext,
                        );
                        if let Err(error) = opened {
                            return Poll::Ready(Err(this.fail(ErrorKind::InvalidData, error)));
                        }
                        this.header_offset = 0;
                        this.ciphertext.clear();
                        this.ciphertext_offset = 0;
                        this.body_len = None;
                        this.plaintext_offset = 0;
                        // Empty authenticated records are valid but must not masquerade as EOF.
                    }
                }
            }
            #[cfg(test)]
            mod tests {
                use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
                use super::*;
                use $crate::{
                    SessionKey,
                    soranet::record::{
                        RecordEndpoint, RecordLayer, RecordStreamContext, RecordStreamKind,
                    },
                };
                #[tokio::test(flavor = "current_thread")]
                async fn adapters_reconstruct_plaintext_across_record_and_read_boundaries() {
                    let key = SessionKey::new(vec![0xA5; 32]);
                    let context = RecordStreamContext::new(
                        RecordEndpoint::Client,
                        RecordStreamKind::Bidirectional,
                        9,
                    );
                    let client = RecordLayer::new(&key, RecordEndpoint::Client)
                        .expect("client")
                        .stream(context)
                        .expect("client stream");
                    let relay = RecordLayer::new(&key, RecordEndpoint::Relay)
                        .expect("relay")
                        .stream(context)
                        .expect("relay stream");
                    let (transport_writer, transport_reader) = tokio::io::duplex(32);
                    let mut writer = RecordWriter::new(transport_writer, client.sealer);
                    let mut reader = RecordReader::new(transport_reader, relay.opener);
                    let write = async {
                        writer.write_all(b"first").await.expect("first");
                        writer.write_all(b"-second").await.expect("second");
                        writer.shutdown().await.expect("shutdown");
                    };
                    let read = async {
                        let mut plaintext = Vec::new();
                        reader.read_to_end(&mut plaintext).await.expect("read");
                        plaintext
                    };
                    let ((), plaintext) = tokio::join!(write, read);
                    assert_eq!(plaintext, b"first-second");
                }
                #[tokio::test(flavor = "current_thread")]
                async fn reader_rejects_tampered_transport_bytes() {
                    let key = SessionKey::new(vec![0x5A; 32]);
                    let context = RecordStreamContext::new(
                        RecordEndpoint::Client,
                        RecordStreamKind::Unidirectional,
                        2,
                    );
                    let mut client = RecordLayer::new(&key, RecordEndpoint::Client)
                        .expect("client")
                        .stream(context)
                        .expect("client stream");
                    let relay = RecordLayer::new(&key, RecordEndpoint::Relay)
                        .expect("relay")
                        .stream(context)
                        .expect("relay stream");
                    let mut record = client.sealer.seal(b"secret").expect("record");
                    *record.last_mut().expect("tag") ^= 1;
                    let (mut transport_writer, transport_reader) = tokio::io::duplex(record.len());
                    transport_writer.write_all(&record).await.expect("write");
                    transport_writer.shutdown().await.expect("shutdown");
                    let mut reader = RecordReader::new(transport_reader, relay.opener);
                    let mut output = Vec::new();
                    let error = reader
                        .read_to_end(&mut output)
                        .await
                        .expect_err("tampered record must fail");
                    assert_eq!(error.kind(), ErrorKind::InvalidData);
                    assert!(output.is_empty());
                }
                #[tokio::test(flavor = "current_thread")]
                async fn reader_rejects_unframed_plaintext_without_exposing_it() {
                    let key = SessionKey::new(vec![0x3C; 32]);
                    let context = RecordStreamContext::new(
                        RecordEndpoint::Client,
                        RecordStreamKind::Bidirectional,
                        4,
                    );
                    let relay = RecordLayer::new(&key, RecordEndpoint::Relay)
                        .expect("relay")
                        .stream(context)
                        .expect("relay stream");
                    let (mut transport_writer, transport_reader) = tokio::io::duplex(32);
                    transport_writer
                        .write_all(b"raw application bytes")
                        .await
                        .expect("write");
                    transport_writer.shutdown().await.expect("shutdown");
                    let mut reader = RecordReader::new(transport_reader, relay.opener);
                    let mut output = Vec::new();
                    let error = reader
                        .read_to_end(&mut output)
                        .await
                        .expect_err("unframed plaintext must fail");
                    assert_eq!(error.kind(), ErrorKind::InvalidData);
                    assert!(output.is_empty());
                }
            }
        }
    };
}
