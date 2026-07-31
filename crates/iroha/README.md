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

## Examples

We highly recommend looking at the sample [`iroha_cli`](../iroha_cli) binary
crate, which builds the `iroha` executable, as well as our
[tutorial](https://docs.iroha.tech/guide/tutorials/rust.html) for more examples
and explanations.
