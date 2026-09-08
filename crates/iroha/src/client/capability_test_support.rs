//! Shared async-only injected transport for capability boundary tests.

use crate::http::{HttpTransport, Response, TransportFuture, TransportRequest};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

pub(super) type Responder =
    dyn Fn(&TransportRequest) -> eyre::Result<Response<Vec<u8>>> + Send + Sync;

pub(super) struct AsyncOnlyTransport {
    pub(super) responder: Box<Responder>,
    pub(super) requests: Arc<Mutex<Vec<TransportRequest>>>,
    pub(super) completed: Arc<AtomicUsize>,
    pub(super) delay: Duration,
}

impl std::fmt::Debug for AsyncOnlyTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AsyncOnlyTransport")
    }
}

impl HttpTransport for AsyncOnlyTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<Response<Vec<u8>>> {
        panic!("capability operations must use asynchronous transport")
    }

    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            let response = (self.responder)(&request);
            self.requests.lock().unwrap().push(request);
            tokio::time::sleep(self.delay).await;
            self.completed.fetch_add(1, Ordering::SeqCst);
            response
        })
    }
}
