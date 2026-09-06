# Recipes

This source-checkout directory hosts JavaScript snippets that illustrate common
`@iroha/iroha-js` workflows. Each example favours deterministic Norito payloads
and mirrors the validation logic exported by the SDK. The portable registry
tarball includes only `iso_bridge_builder.mjs` and `nexus_app_transfer.mjs`;
examples that require the Cargo workspace, verified native host, credentials,
or a live Torii endpoint remain source-checkout tools and declare those
prerequisites below.

## batching.mjs

- Shows how to assemble multi-instruction transactions with explicit mint,
  transfer, and burn steps while keeping instruction ordering deterministic.
- Highlights the helper builders that normalise quantities and asset IDs before
  handing instructions to the generic `buildTransaction` helper.
- Prints the resulting transaction hashes so you can compare deterministic
  output across environments before submitting to Torii.

Run with:

```bash
npm install
npm run build:native
node ./recipes/batching.mjs
```

The script uses deterministic sample key material; replace the sample
authority/account values with real identities before attempting to submit
transactions on a live network.

## nexus_app_transfer.mjs

- Runs the Nexus App facade from approval through canonical browser-codec
  finalization, Torii submission, and exact state-resolved Applied finality.
- Uses deterministic fake Connect and Torii dependencies so the installed
  recipe is runnable offline while still checking the canonical payload and
  signed-transaction hashes.

Run with:

```bash
npm install
node ./recipes/nexus_app_transfer.mjs
```

Browser applications can omit the fakes and configure `NexusAppClient` with a
Connect base URL and Torii base URL to use its built-in browser paths.

## nft_account_iteration.mjs

- Demonstrates the iterator helpers for NFTs and per-account asset balances
  with `requirePermissions` enabled so secured Torii nodes fail fast without
  credentials.
- Applies Norito-style filters/sorts that match the server adapters
  (`quantity` comparisons for assets; id sorting for NFTs) and shows how to
  request compressed literals plus `select` projections.

Run with:

```bash
npm install
TORII_URL=http://127.0.0.1:8080 \
ACCOUNT_ID=sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB \
TORII_AUTH_TOKEN=token \
TORII_ALLOW_INSECURE=1 \
node ./recipes/nft_account_iteration.mjs
```

Environment variables:

- `TORII_URL` — Torii endpoint (defaults to `http://127.0.0.1:8080`).
- `ACCOUNT_ID` — account literal for the asset iterator.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — credentials for permissioned Torii.
- `TORII_ALLOW_INSECURE=1` — allow HTTP even when credentials are attached.

## iso_bridge.mjs

- Demonstrates ISO 20022 pacs.008 / pacs.009 submission via
  `ToriiClient.submitIsoPacs008AndWait` / `submitIsoPacs009AndWait`, and shows
  how to reuse `waitForIsoMessageStatus` when you already have a `message_id`.
- Highlights how to provide XML payloads, override polling cadence through env
  variables, opt into resolving when the bridge reports `Accepted`, and inspect
  pacs.002 codes, ledger hashes, and account hints surfaced by the bridge.

Run with:

```bash
npm install
node ./recipes/iso_bridge.mjs
```

Environment variables:

- `TORII_URL` — Torii endpoint with the ISO bridge enabled.
- `ISO_MESSAGE_KIND` — switch between `pacs.008` (default) and `pacs.009`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — control wait cadence.
- `ISO_MESSAGE_ID` — skip submission and wait on an existing message.
- `ISO_RESOLVE_ON_ACCEPTED=1` — treat `Accepted` without a transaction hash as terminal.

If the bridge is protected by API tokens, export `X-API-Token` alongside
`TORII_URL`.

## iso_alias.mjs

- Demonstrates both literal (IBAN-style) and indexed lookups via
  `resolveAlias` / `resolveAliasByIndex`.
- Surfaces 404 vs runtime-disabled responses and highlights the account bindings returned by the alias
  APIs so ISO bridge drills can run without bespoke tooling.

Run with:

```bash
npm install
TORII_URL=https://torii.testnet.sora \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node ./recipes/iso_alias.mjs
```

Environment variables:

- `ISO_ALIAS_LABEL` — resolve a literal alias; omit to skip the lookup.
- `ISO_ALIAS_INDEX` — decimal or `0x`-prefixed index for deterministic lookups.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — optional headers for secured Torii deployments.

## iso_bridge_builder.mjs

- Uses `buildPacs008Message` / `buildPacs009Message` to translate structured
  settlement parameters (BIC/LEI identifiers, IBANs, purpose codes, and
  supplementary data) into deterministic ISO 20022 XML.
- Accepts overrides via environment variables or a JSON config
  (`ISO_BUILDER_CONFIG=/path/to/options.json`) so you can feed the script with
  production reference data.
- Prints the generated payload by default and submits it to Torii when
  `ISO_SUBMIT=1`, reusing the same wait options as `iso_bridge.mjs`.

Run with:

```bash
npm install
node ./recipes/iso_bridge_builder.mjs \
  ISO_SUBMIT=1 TORII_URL=http://localhost:8080 ISO_KIND=pacs.009 \
  ISO_AMOUNT=1250.50 ISO_CURRENCY=USD ISO_PURPOSE=SECU
```

Environment variables (in addition to the common `ISO_*` knobs above):

- `ISO_DEBTOR_AGENT_BIC` / `ISO_DEBTOR_AGENT_LEI` — optional debtor agent BIC + LEI injected as
  `DbtrAgt`. Omit to rely on the instigating agent.
- `ISO_CREDITOR_AGENT_BIC` / `ISO_CREDITOR_AGENT_LEI` — optional creditor agent records (`CdtrAgt`).
- `ISO_DEBTOR_NAME`, `ISO_DEBTOR_LEI`, `ISO_DEBTOR_ID`, `ISO_DEBTOR_ID_SCHEME` — emit a debtor
  party (`Dbtr`) with a legal name, optional LEI, and proprietary identifier. Leave unset to skip.
- `ISO_CREDITOR_*` — mirrors the debtor fields for the creditor party (`Cdtr`).
- `ISO_SUPPLEMENTARY_JSON` — canonical JSON merged into `SplmtryData` (useful for Norito metadata).

Structured overrides supplied via `ISO_BUILDER_CONFIG` can include the same
`BuildPacs008Options` / `BuildPacs009Options` fields (agents, parties, accounts,
purpose codes, supplementary data). The script deep-merges nested objects so
JSON files can override just a subset of identifiers without rewriting every
field.

## streaming.mjs

- Streams `/v1/events/sse` with a deterministic filter (pipeline transactions by default).
- Treats the endpoint as live-only: reconnects start a new subscription and may have a gap.
- Surfaces pipeline status transitions using `extractPipelineStatusKind` so runbooks can capture
  applied/committed transitions side-by-side with the live JSON payload.

Run with:

```bash
npm install
node ./recipes/streaming.mjs \
  TORII_URL=https://torii.nexus.example \
  PIPELINE_STATUS=Committed \
  STREAM_MAX_EVENTS=25
```

Environment variables:

- `STREAM_FILTER_JSON` — override the default pipeline transaction filter with raw JSON.
- `PIPELINE_STATUS` — change the diagnostic SSE event filter
  (`Queued`/`Approved`/`Committed`/`Applied`, etc.). This does not configure
  transaction finality; waits still require exact global, state-resolved `Applied`.
- `STREAM_MAX_EVENTS` — stop after N events (`0` keeps the iterator running until interrupted).
- `TORII_API_TOKEN` / `TORII_AUTH_TOKEN` — optional headers for locked-down Torii deployments.

## assets_iterators.mjs

- Iterates NFTs and per-account asset holdings using the pagination helpers
  (`iterateNftsQuery`, `iterateAccountAssets`), applying Norito-style sort and
  filter envelopes (id equality for NFTs, optional quantity filters for assets)
  so the output mirrors Torii JSON responses.
- Enables `requirePermissions` automatically when credentials are configured;
  `TORII_REQUIRE_PERMISSIONS=1|0` can override that fail-fast gate explicitly.
  Page sizes and caps can be tuned via environment variables.

Run with:

```bash
npm install
TORII_URL=http://localhost:8080 \
ACCOUNT_ID=sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB \
NFT_ID=61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D \
PAGE_SIZE=25 \
MAX_ITEMS=100 \
TORII_ALLOW_INSECURE=1 \
node ./recipes/assets_iterators.mjs
```

Environment variables:

- `TORII_URL` — Torii endpoint (defaults to `http://localhost:8080`).
- `ACCOUNT_ID` — account literal for asset iteration.
- `NFT_ID` — optional canonical NFT/asset-holding id (`<base58-asset-definition-id>#<i105-account-id>`) to filter on (exact match).
- `PAGE_SIZE` / `MAX_ITEMS` — pagination controls.
- `TORII_API_TOKEN` / `TORII_AUTH_TOKEN` — credentials for permissioned nodes.
- `TORII_ALLOW_INSECURE=1` — allow HTTP while sending credentials (dev/test only).
- `TORII_REQUIRE_PERMISSIONS=1|0` — override the default permission gate; by
  default it is enabled when an API/auth token is configured.

## governance.mjs

- Builds sample transactions for governance flows (propose deploy and cast a
  standalone plain ballot) using the SDK builders.
- Prints deterministic hashes for each transaction; optionally submits them to
  Torii when `GOV_SUBMIT=1`.
- When `GOV_FETCH=1`, reuses the new `ToriiClient` governance helpers to fetch proposals, tallies,
  locks, and unlock stats so you can inspect live state after submitting transactions.
- Parliament finalization, certificate derivation, and exact-height enactment are
  automatic protocol work; the recipe does not expose public finalize/enact calls.

Run with:

```bash
npm install
npm run build:native
node ./recipes/governance.mjs
```

Set `TORII_URL`, `NETWORK_ID`, `AUTHORITY`, and `PRIVATE_KEY_HEX` (32- or 64-byte Ed25519 key)
when submitting to a live node. Ensure the authority holds the necessary
governance permissions before enabling `GOV_SUBMIT=1`.

When fetching governance state, provide the identifiers you want to inspect via
`GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, and optionally `GOV_LOCKS_ID`
(defaults to `GOV_REFERENDUM_ID`). Example:

```bash
GOV_FETCH=1 GOV_REFERENDUM_ID=demo-referendum node ./recipes/governance.mjs
```

## soradns.mjs

- Uses the deterministic host helper to derive canonical/pretty hosts for a
  SoraDNS FQDN without contacting a gateway.
- Validates the host patterns advertised by a Gateway Authorisation Record
  (GAR) so operators can confirm the canonical host, wildcard, and pretty host
  are all authorised before dialing.
- Prints the normalized name, canonical label, host patterns, and GAR coverage
  result for troubleshooting.

Run with:

```bash
npm install
npm run build:native
node ./recipes/soradns.mjs docs.sora --gar-patterns hash.gw.sora.id,*.gw.sora.id,docs.sora.gw.sora.name
```

The script also respects `SORADNS_NAME` and `SORADNS_GAR` environment variables
when you want to avoid passing command-line arguments.
