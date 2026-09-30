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
Individual validator selection changes the observer endpoint only. It provides
no per-peer lifecycle or alternate generation path. The desktop preserves state
pagination, canonical block/event streams, bounded logs, balance/block summaries,
structured and JSON transaction drafts, signed previews and deployment receipts.

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
