# Iroha Client

This is the Iroha 2 client library crate. With it you can build your own client applications to communicate with peers in an Iroha 2 network via HTTP/WebSocket.

This crate is the reusable Rust SDK surface. The `iroha` command-line binary is
built from the separate [`iroha_cli`](../iroha_cli) crate.

Follow the [Iroha 2 tutorial](https://docs.iroha.tech/guide/tutorials/rust.html) for instructions on how to set up, configure, and use the Iroha 2 client and client library.

## Features

* Submit one or several Iroha Special Instructions (ISI) as a Transaction to Iroha Peer
* Request data based on Iroha Queries from a Peer

## Setup

**Requirements:** a working [Rust toolchain](https://www.rust-lang.org/learn/get-started) (version 1.60), installed and configured.

Add the following to the manifest file of your Rust project:

```toml
iroha = { path = "path/to/iroha" }
```

## Examples

We highly recommend looking at the sample [`iroha_cli`](../iroha_cli) binary
crate, which builds the `iroha` executable, as well as our
[tutorial](https://docs.iroha.tech/guide/tutorials/rust.html) for more examples
and explanations.
