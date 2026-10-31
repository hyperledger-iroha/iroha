# Mochi development

<img src="assets/mochi-logo.svg" alt="Mochi" width="320">

Mochi is a desktop client of the same managed workspace used by Kagami. The
native bundle contains matching `mochi`, `kagami` and `iroha3d` executables.
`mochi --workspace <directory>` selects the workspace; no user-supplied TOML,
source build, PATH discovery or project-local secret export is involved.

## Desktop tour

- **Home** offers Quickstart: one click creates and starts a four-validator
  localnet under the first unused `local`, `local-2`, … name, with a funded
  developer account and no storage-service provisioning; the header counts the
  elapsed time while validators start. With an environment selected it shows each
  validator's Torii URL, the chain pulse, the developer account's balances, the
  latest blocks and quick actions; clicking a latest block opens it in **Blocks**.
  While the network is ready, each validator card
  also shows that validator's newest block height and hash, read every two seconds
  from its own Explorer. A card is flagged `N blocks behind` while its validator
  stays below a height an earlier read already saw, `hash differs` when it holds
  a different block at a height others share, and `no response` when it does not
  answer; the Validators heading reads `in sync` or `N out of sync`. A
  validator one block short while a commit spreads is not flagged. The chain
  pulse rolls the block height and shows the mean time between blocks,
  transactions per second and rejected transactions over animated bars of the
  newest blocks; hover a bar for its block.
- The header switches, creates and attaches environments and holds Start, Stop,
  Refresh and Reset. The status strip shows phase, running validators, block
  height, chain, network, dataspace and the observed Torii URL; click a value to copy.
- **Accounts** lists registered accounts with their holdings. Quick actions mint,
  send, burn, create a token or create a funded account; tokens, balances and
  recipients are picked by name. Only a definition's owner mints it, and genesis
  owns XOR, so Mint lists the developer's own tokens first and says before review
  when a mint would be rejected. New token leases `<label>.universal` from the name
  service for one year unless the developer already holds it, registers the token
  there and mints its first supply in one transaction; the review shows the exact
  lease price. A new account's key pair is generated in memory, revealed only on
  request and dropped with the environment. Every quick action becomes the same
  signed preview as the composer and is submitted only by an explicit click. A
  preview asks the observed validator for a fee quote, signs the quoted limits and
  lists each fee component; if no quote is available, nothing is signed. The review
  card then shows how the network settled the transaction: applied with its block,
  or rejected or expired with the validators' reason. An unconfirmed outcome keeps
  the original hash for lookup.
- **Blocks** and **Activity** follow the observed validator's canonical streams;
  **Blocks** adds a wider chain pulse once blocks stream in. Idle chains create
  no blocks, so balances refresh only after committed blocks or submissions.
  Opening a block shows its header and commitments, validator signatures and
  commit certificate, every transaction with its instructions, metadata, fee
  intent and applied or rejected result, trigger runs, and the complete block as
  copyable Norito JSON and `SignedBlockWire` hex. Previous and Next walk the
  chain. Decoded bodies of the newest streamed blocks stay in memory up to
  32 MiB; any other block is read again from the observed validator's canonical
  block stream.
- **Endpoints** lists Torii, health, status, metrics, OpenAPI and MCP URLs for each
  validator, the network identity and copyable `kagami` and `curl` commands. It
  never shows generated keys, API tokens or private configuration paths.
- **Contracts** accepts a `.ko`, `.to`, `Musubi.toml` or package directory dropped
  anywhere on the window. After a deployment applies, the call card is filled in
  with the contract alias, its declared initialization entrypoint and an argument
  template naming each parameter. View reads without signing; Call signs one
  bounded call under the fee cap shown beside it. Deploying the same input again
  recovers its journal; the Resume card also lists the environment's saved
  deployments, so a changed or missing input still resumes without typing a path.
  **Transactions**, **State**, **Packages** and **Logs** cover the remaining
  workflows.
- The theme follows the operating system and can be toggled in the header. The
  mascot's face follows the selected network's phase; poke it for a squish.
  Sakura petals fall when a localnet starts or a deployment applies.
  Brand assets and their provenance live in [assets](assets/README.md).

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
New localnet creates another named environment without resetting the current one;
the new environment is selected when ready. Both startup actions require a fresh
Ready observation before changing selection. Failed startup retains the previous
selection and exposes the prepared name for recovery from the environment menu.
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
Exact deployment and fee review appears over every desktop view. Switching views
during preparation cannot hide the review, and only its explicit confirmation
approves dispatch; cancellation or closing Mochi leaves approval withheld. The one
exception is opt-in: with automatic approval enabled on the Contracts card, a
deployment whose quoted fees total at most the cap in one fee asset dispatches
without the dialog, and Activity records the approval. Larger or mixed-asset fees
still ask, and the setting lasts only until Mochi closes.
Deployment results identify the original localnet or private dataspace on which
the contract applied. Parent receipts appear separately as historical evidence;
an unavailable parent observation does not undo a verified local deployment. A failed automatic
workspace refresh keeps the completed deployment receipt visible, clears live context, streams,
logs and activity, and reports the observation error. Explicit selection and Stop
retain their existing clearing behavior.

Deployment results also show artifact-derived initialization guidance when the
contract declares `hajimari`/`始まり`. It stays separate from the Applied receipt:
the hook requires an explicit call, and recovered guidance does not assert its
current state. The call card is filled in for that call. Arguments follow the
verified entrypoint schema's JSON forms: integers, decimals and quantities are
strings such as `"5"`, booleans are `true`/`false`.

The Packages view exposes explicit Publish, Resume publication and Recover package files
actions through `DeveloperWorkspace::publish_package` and the shared Musubi engine.
For local publication, select a storage-enabled environment created in the same workspace
with `kagami localnet up registry --with-storage`. Recovery requires its existing selected context
and original operation ID. Original namespace custody and generation-bound operation/cache
paths are shared with Kagami. The view retains canonical exit status, output and diagnostics
even when the subsequent workspace observation fails. These source paths still require
combined native three-provider publication and cold-package qualification.

Balances, accounts and the State view read Torii's collection endpoints
(`/v1/accounts`, `/v1/assets/definitions`, `/v1/domains`, `/v1/explorer/assets`),
signed by the generated client. Peers use the signed `FindPeers` query, the only
listing that signed `/v1/query` admits.

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

Design previews render every view headlessly to PPM images. The harness runs a
genuine Quickstart with matching installed programs, then stages phases and
fixture observations on that real generation; its balances and blocks are not
ledger evidence:

```sh
MOCHI_PREVIEW_RUNTIME=<dir with kagami and iroha3d> MOCHI_PREVIEW_OUT=<dir> \
  cargo test -p mochi-ui --features gui --bin mochi design_previews -- --ignored --nocapture
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
