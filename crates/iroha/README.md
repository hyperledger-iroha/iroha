# Iroha Client

This is the reusable client library for the first Hyperledger Iroha 3 release.
Use it to build applications that communicate with Iroha peers over HTTP and
WebSocket.

This crate is the reusable Rust SDK surface. The `iroha` command-line binary is
built from the separate [`iroha_cli`](../iroha_cli) crate.

Follow the [Iroha 3 Rust tutorial](https://docs.iroha.tech/guide/tutorials/rust.html)
for setup, configuration, and client examples.

## Features

* Submit one or several Iroha Special Instructions (ISI) as a Transaction to Iroha Peer
* Request data based on Iroha Queries from a Peer

Transaction finality waits require state-resolved `Applied` status. Temporary
HTTP 429 responses repeat only the status read, respecting Torii's delta-seconds
`Retry-After` and the original wait deadline. They never resubmit the transaction;
an exhausted deadline retains the last backpressure diagnostic.

## Setup

**Requirements:** install
[Rust 1.93.1](https://www.rust-lang.org/learn/get-started), the toolchain pinned
for this workspace in the repository-root `rust-toolchain.toml`.

Add the following to the manifest file of your Rust project:

```toml
iroha = { git = "https://github.com/hyperledger-iroha/iroha.git", rev = "<IROHA_COMMIT>", package = "iroha" }
```

Pin `<IROHA_COMMIT>` to the revision deployed by your network. For a local
checkout, use `iroha = { path = "/path/to/iroha/crates/iroha" }`.

## Client construction

Construct an asynchronous client with `Client::builder(config).build()?`.
The builder validates the endpoint, signing authority, address discriminant and
HTTP headers. Configure headers with `headers(...)`, HTTP transport with
`http_transport(Arc<dyn iroha::http::HttpTransport>)`, and WebSocket transport with
`stream_transport(Arc<dyn iroha::stream::StreamTransport>)` on the builder.
Configuration is fixed after construction; transport initialization failures
are returned through `iroha::Error`. The explicit blocking HTTP constructor is
`iroha::blocking::Client::with_http_transport(config, transport)?`.

Clones share their HTTP and stream transports and compatibility decision. To
change configuration, copy it with `client.to_builder()`, edit the builder, and
call `build()` to obtain a new context with a fresh compatibility cache and probe
coordinator. The copied builder retains both selected transports. When selecting
a different origin, choose its HTTP and operator credentials explicitly.

Bind account operations with `client.account_client()?` and privileged operations
with `client.operator_client(operator_key_pair)?`. Synchronous applications use
`iroha::blocking`, which owns a reusable runtime and rejects calls from an async
runtime. Remaining synchronous capability methods and authority-owned operations
are tracked in the repository's first-release architecture redesign record.

## Node diagnostics

Use `client.status().get().await?` for the typed status document and
`client.status().version().await?` for the API version. Synchronous applications
use the same capability through `iroha::blocking::Client`, without `.await`.
The operation selects one response representation, enforces the context deadline
and rejects oversized or ambiguously labelled responses. Transport errors retain
their I/O category in `TransportErrorKind`; HTTP errors retain their bounded body.

Submission discovers the node's data-model version and signed-transaction schema
through public `/v1/node/capabilities` metadata before dispatching transaction
bytes. This probe omits canonical account authentication so a fresh account can
submit its registration. Configured transport headers, deadlines, response bounds
and exact version/schema checks still apply.

## Operator configuration

Bind the operator with `client.operator_client(operator_key_pair)?`, then read
`operator.configuration().get().await?`. The returned shared DTO includes the
confidential gas schedule. Configuration uses one operator-signed JSON request,
the context deadline and an 8-MiB response ceiling.

For synchronous operator-only work, construct
`iroha::blocking::OperatorClient::from_client(operator)?` and call
`operator.configuration().get()?`. This facade owns a reusable runtime without
requiring an account context. Public and account contexts have no configuration
capability.

## Event and block streams

Bind `let account = client.account_client()?`, then use
`account.events().subscribe(filters).await?` or
`account.blocks().subscribe(height).await?`. Filters must be nonempty; block
height is `NonZeroU64`, and the account must hold `CanReadAllLedgerData` to read
full signed blocks. These capabilities belong only to the account context.
Both sign the exact upgrade and use the `iroha-norito-v1` WebSocket subprotocol.

The owned streams implement `Stream<Item = iroha::Result<EventBox>>` or
`Stream<Item = iroha::Result<SignedBlock>>`. Initial subscription messages are
limited to 256 KiB and received messages to 64 MiB, including fragmentation.
Upgrade response bodies are limited to 64 KiB. Connection establishment and the
initial send share `torii_request_timeout`; zero disables that deadline.
Receiving has no implicit lifetime deadline. Use an async timeout when a read
needs one.

Call `stream.close().await?` to flush a close frame and release the connection.
Close uses the context request timeout, or five seconds when that timeout is
zero. A normal peer close (1000) ends the stream; abnormal close codes and reasons
remain in `Error::StreamClosed`. Decode, transport and protocol failures terminate
the stream. Dropping it releases I/O; it does not spawn cleanup or reconnect work.
Subscriptions never redirect, retry, resume or replay automatically.

`last_message_bytes()` reports the complete binary message size for the last
item delivered, including malformed binary messages. It is `None` before the
first item or after a transport failure without a delivered binary message.
Pending reads, local receive timeouts and normal completion preserve the last
observation. This metadata also exists on blocking streams. HTTP errors retain
an unambiguous `Retry-After` delta as `retry_after`; it is a hint for caller-owned
policy and never causes the SDK to retry.

For synchronous code, use
`iroha::blocking::AccountClient::from_client(account)?`, then the same capability
without `.await`. Its stream uses the reusable owned runtime:
`recv(Some(duration))` bounds one wait, `recv(None)` waits without an idle timeout,
and `close()` returns a typed result. `Ok(None)` means normal completion; a wait
expiry returns `Error::Timeout { operation: "stream.receive" }` and permits another
receive on the same subscription. Blocking calls reject execution inside an async
runtime. Custom stream connectors implement `StreamTransport` and `StreamSocket`;
the SDK retains request signing, protocol checks, deadlines and typed decoding.

## Subscription drafts

Subscription preparation returns a typed `SubscriptionDraft`; its payload variant
owns `Box<TransactionPayload>`, and its instruction variant owns the instruction
list. Its JSON representation retains the `kind` and `value` fields. Preparation
futures are `Send`, and signing remains an explicit account operation.

## Examples

We highly recommend looking at the sample [`iroha_cli`](../iroha_cli) binary
crate, which builds the `iroha` executable, as well as our
[tutorial](https://docs.iroha.tech/guide/tutorials/rust.html) for more examples
and explanations.
