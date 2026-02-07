---
slug: /norito/ledger-walkthrough
lang: mn
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
---

This walkthrough complements the [Norito quickstart](./quickstart.md) by showing
how to mutate and inspect ledger state with the `iroha` CLI. You will register a
new asset definition, mint some units into the default operator account, transfer
part of the balance to another account, and verify the resulting transactions
and holdings. Each step mirrors the flows covered in the Rust/Python/JavaScript
SDK quickstarts so you can confirm parity between CLI and SDK behaviour.

## Prerequisites

- Follow the [quickstart](./quickstart.md) to boot the single-peer network via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Ensure `iroha` (the CLI) is built or downloaded and that you can reach the
  peer using `defaults/client.toml`.
- Optional helpers: `jq` (formatting JSON responses) and a POSIX shell for the
  environment-variable snippets used below.

Throughout the guide, replace `$ADMIN_ACCOUNT` and `$RECEIVER_ACCOUNT` with the
account IDs you plan to use. The defaults bundle already includes two accounts
derived from the demo keys:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Confirm the values by listing the first few accounts:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspect the genesis state

Start by exploring the ledger the CLI is targeting:

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

These commands rely on Norito-backed responses, so filtering and pagination are
deterministic and match what the SDKs receive.

## 2. Register an asset definition

Create a new, infinitely mintable asset called `coffee` inside the `wonderland`
domain:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

The CLI prints the submitted transaction hash (for example,
`0x5f…`). Save it so you can query the status later.

## 3. Mint units into the operator account

Asset quantities live under the `(asset definition, account)` pair. Mint 250
units of `coffee#wonderland` into `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

Again, capture the transaction hash (`$MINT_HASH`) from the CLI output. To
double-check the balance, run:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

or, to target just the new asset:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. Transfer part of the balance to another account

Move 50 units from the operator account to `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Save the transaction hash as `$TRANSFER_HASH`. Query the holdings on both
accounts to verify the new balances:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. Verify ledger evidence

Use the saved hashes to confirm that both transactions committed:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

You can also stream recent blocks to see which block included the transfer:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Every command above uses the same Norito payloads as the SDKs. If you replicate
this flow via code (see the SDK quickstarts below), the hashes and balances will
line up as long as you target the same network and defaults.

## SDK parity links

- [Rust SDK quickstart](../sdks/rust) — demonstrates registering instructions,
  submitting transactions, and polling status from Rust.
- [Python SDK quickstart](../sdks/python) — shows the same register/mint
  operations with Norito-backed JSON helpers.
- [JavaScript SDK quickstart](../sdks/javascript) — covers Torii requests,
  governance helpers, and typed query wrappers.

Run the CLI walkthrough first, then repeat the scenario with your preferred SDK
to make sure both surfaces agree on transaction hashes, balances, and query
outputs.
