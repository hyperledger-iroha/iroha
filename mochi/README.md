# Mochi development

Mochi is a desktop client of the same managed workspace used by Kagami. The
native bundle contains matching `mochi`, `kagami` and `iroha3d` executables.
`mochi --workspace <directory>` selects the workspace; no user-supplied TOML,
source build, PATH discovery or project-local secret export is involved.

## Ownership

- `iroha_deploy::managed` owns generated configuration, four-validator lifecycle,
  retained identities, context selection, native worker authentication and logs.
- `mochi-core::developer` binds desktop observations and transaction previews to
  an immutable managed context. Closing Mochi does not stop the managed network.
- Musubi and `iroha_contract_deploy` own compilation, quoted-fee review, original
  transaction recovery and authenticated contract readback. Desktop recovery
  reviews the exact retained plan again before any new dispatch.
- `iroha_fs` owns private filesystem custody on native Unix and Windows.

The UI provides aggregate Start, Stop and explicit stopped-network Reset actions.
Its Private dataspace action selects a network from the installed profile artifact
and delegates the operation, with a sixty-second foreground deadline, to the same
managed worker as Kagami. The desktop performs no parent HTTP requests or
wallet/journal writes.
Failed parent attempts retain their funding, namespace, registration or anchoring stage and
a fixed public failure code. Foreground deadline errors include that last stage
and cause; preparation failures remain distinct from finality catch-up, without
exposing server response bodies or private custody paths. Retries reconcile the
same retained operations.
After initial registration, `anchoring` means the worker is observing the parent
or relaying later child certificates; a retained receipt alone does not complete
that current work.
Local validator readiness and parent attachment progress are displayed separately;
only an independently verified parent receipt is shown as confirmed, including
its original parent and child heights. Retained confirmations remain historical
when the parent is unavailable. A bundle without installed profiles supports
localnets without inventing a public network authority.
Individual validator selection changes the observer endpoint only. It provides
no per-peer lifecycle or alternate generation path. The desktop preserves state
pagination, canonical block/event streams, bounded logs, balance/block summaries,
structured and JSON transaction drafts, signed previews and deployment receipts.
Deployment results identify the original localnet or private dataspace on which
the contract applied. Parent receipts appear separately as historical evidence;
an unavailable parent observation does not undo a verified local deployment.

The generated client signer is also the ledger-stream authority. Node HTTP
operator signing uses a distinct generated key validated against all four node
configs. Neither account defaults to a bundled sample key. Observations and
previews reject a removed/replaced generation and never adopt a new network
implicitly. Canonical stream transport checks endpoint and network identity.

## Focused checks

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check -p mochi-ui --features gui --bin mochi
cargo test -p mochi-core
cargo test -p mochi-integration
cargo test -p mochi-ui --features gui --bin mochi
ci/check_mochi.sh
```

Shared generator/worker tests live in `iroha_deploy`; custody tests live in
`iroha_fs`. Mochi tests cover context binding, query authority, canonical stream
transport and desktop review cancellation. Mock services do not qualify native
four-validator operation or Windows filesystem/process semantics. Release bundle
qualification must exercise the packaged binaries on each native platform.

See [the bundle contract](../specs/mochi_bundle.md) and
[developer-experience implementation goals](../specs/kagami_mochi_devex_goals.md).
Public guides are maintained in [Iroha documentation](https://docs.iroha.tech/).

Packaged macOS builds contain a native `Mochi.app`: the desktop, Kagami and
`iroha3d` live together in `Contents/MacOS`, and installed network profiles live
only in `Contents/Resources`. Linux/Windows packages use `bin`. See
[BUNDLE_README.md](BUNDLE_README.md) for installed commands. Loose Cargo-built
programs remain a development mode; they are not the macOS distribution layout.
App assembly does not claim signing or notarization.
