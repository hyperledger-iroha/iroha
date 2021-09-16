use std::{
    borrow::Borrow,
    convert::{TryFrom, TryInto},
};

use attohttpc::Response as AttohttpcResponse;
use eyre::{eyre, Error, Result, WrapErr};
pub use http::{Response, StatusCode};
use tungstenite::{client::AutoStream, WebSocket};
pub use tungstenite::{Error as WebSocketError, Message as WebSocketMessage};

type Bytes = Vec<u8>;

pub fn post<U, P, K, V>(url: U, body: Bytes, query_params: P) -> Result<Response<Bytes>>
where
    U: AsRef<str>,
    P: IntoIterator,
    P::Item: Borrow<(K, V)>,
    K: AsRef<str>,
    V: ToString,
{
    let url = url.as_ref();
    let response = attohttpc::post(url)
        .bytes(body)
        .params(query_params)
        .send()
        .wrap_err_with(|| format!("Failed to send http post request to {}", url))?;
    ClientResponse(response).try_into()
}

pub fn get<U, P, K, V>(url: U, body: Bytes, query_params: P) -> Result<Response<Bytes>>
where
    U: AsRef<str>,
    P: IntoIterator,
    P::Item: Borrow<(K, V)>,
    K: AsRef<str>,
    V: ToString,
{
    let url = url.as_ref();
    let response = attohttpc::get(url)
        .bytes(body)
        .params(query_params)
        .send()
        .wrap_err_with(|| format!("Failed to send http get request to {}", url))?;
    ClientResponse(response).try_into()
}

pub type WebSocketStream = WebSocket<AutoStream>;

pub fn web_socket_connect<U>(url: U) -> Result<WebSocketStream>
where
    U: AsRef<str>,
{
    let (stream, _) = tungstenite::connect(url.as_ref())?;
    Ok(stream)
}

struct ClientResponse(AttohttpcResponse);

impl TryFrom<ClientResponse> for Response<Bytes> {
    type Error = Error;

    fn try_from(response: ClientResponse) -> Result<Self> {
        let ClientResponse(response) = response;
        let mut builder = Response::builder().status(response.status());
        let headers = builder
            .headers_mut()
            .ok_or_else(|| eyre!("Failed to get headers map reference."))?;
        for (key, value) in response.headers() {
            headers.insert(key, value.clone());
        }
        response
            .bytes()
            .wrap_err("Failed to get response as bytes")
            .and_then(|bytes| {
                builder
                    .body(bytes)
                    .wrap_err("Failed to construct response bytes body")
            })
    }
}
