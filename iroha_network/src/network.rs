use std::{
    convert::{Infallible, TryFrom},
    future::Future,
    sync::Arc,
    time::Duration,
};

use iroha_error::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

use super::{AsyncStream, Request, Response, State};

const BUFFER_SIZE: usize = 2_usize.pow(12);
#[cfg(feature = "test-no-timeout")]
const REQUEST_TIMEOUT_MILLIS: u64 = 5000;
#[cfg(not(feature = "test-no-timeout"))]
const REQUEST_TIMEOUT_MILLIS: u64 = 500;

pub async fn send_request_to(server_url: &str, request: Request) -> Result<Response> {
    timeout(Duration::from_millis(REQUEST_TIMEOUT_MILLIS), async {
        let mut stream = TcpStream::connect(server_url).await?;
        let payload: Vec<u8> = request.into();
        stream.write_all(&payload).await?;
        stream.flush().await?;
        let mut buffer = vec![0_u8; BUFFER_SIZE];
        let read_size = stream.read(&mut buffer).await?;
        Response::try_from(buffer[..read_size].to_vec())
    })
    .await?
}

pub async fn listen<H, F, S>(
    state: State<S>,
    server_url: &str,
    mut handler: H,
) -> Result<Infallible>
where
    H: Send + FnMut(State<S>, Box<dyn AsyncStream>) -> F,
    F: Send + Future<Output = Result<()>> + 'static,
    State<S>: Send + Sync,
{
    let listener = TcpListener::bind(server_url).await?;
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => Box::new(stream),
            Err(error) => {
                iroha_logger::warn!(%error, "Failed to accept connection");
                continue;
            }
        };
        let _drop = tokio::spawn(handler(Arc::clone(&state), stream));
    }
}
