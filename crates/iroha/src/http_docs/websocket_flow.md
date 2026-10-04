Internal construction of canonical Norito subscriptions.

Public callers use account-owned capabilities. Each subscription signs one
exact upgrade and sends one framed Norito subscription. The returned stream
owns its connection and never reconnects automatically.

`iroha::client::streams::Events::subscribe` documents the public call; this
private module owns its canonical upgrade and subscription frame.

Custom connectors implement `iroha::stream::StreamTransport` and are installed
with `ClientBuilder::stream_transport`. The SDK validates the upgrade response,
subscription deadline, complete-message bounds, canonical decoding and terminal
close disposition around the injected channel. HTTP and stream transport
configuration belongs to the immutable client context.

Blocking callers construct `iroha::blocking::AccountClient` from the validated
account context and use the same `events().subscribe(...)` or
`blocks().subscribe(...)` operation. The resulting stream's `recv(timeout)`
uses the facade's reusable runtime, and `close()` reports bounded close errors.
