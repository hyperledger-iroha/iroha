# Recipes

This directory hosts runnable JavaScript snippets that illustrate common
`@iroha/iroha-js` workflows. Each example favours deterministic Norito payloads
and mirrors the validation logic exported by the SDK.

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

The script uses placeholder key material; replace the sample authority/account
values with real identities before attempting to submit transactions on a live
network.

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
ACCOUNT_ID=sorauロ1Nタセhjセ7pZaG9L7エmBnクbヨ9ヰsウ4dqmナコmチホ24CウオEAE9L4 \
TORII_AUTH_TOKEN=token \
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

- Calls `ToriiClient.evaluateAliasVoprf` to hash blinded alias inputs and
  demonstrates both literal (IBAN-style) and indexed lookups via
  `resolveAlias` / `resolveAliasByIndex`.
- Prints backend/digest metadata for VOPRF requests, surfaces 404 vs runtime
  disabled responses, and highlights the account bindings returned by the alias
  APIs so ISO bridge drills can run without bespoke tooling.

Run with:

```bash
npm install
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeef \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node ./recipes/iso_alias.mjs
```

Environment variables:

- `ISO_VOPRF_INPUT` — hex-encoded blinded element (defaults to `deadbeef`).
- `ISO_ALIAS_LABEL` — resolve a literal alias; omit to skip the lookup.
- `ISO_ALIAS_INDEX` — decimal or `0x`-prefixed index for deterministic lookups.
- `ISO_SKIP_VOPRF=1` — suppress the VOPRF call (useful when only testing lookups).
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

## contracts.mjs

- Automates `/v1/contracts/deploy` and `/v1/contracts/instance` so roadmap JS-06
  deliverables have a reproducible CLI. Bytecode is read from
  `CONTRACT_CODE_PATH`, optional manifest JSON is pulled from
  `CONTRACT_MANIFEST_PATH` or `CONTRACT_MANIFEST_JSON`, and the helper enforces
  the same validation rules as `ToriiClient.deployContract/Instance` (non-empty
  namespace/contract IDs, 32-byte hashes, base64-encoded payloads).
- `CONTRACT_STAGE=register|instance|both` selects whether to only upload
  manifest/bytecode, only activate an instance, or perform both operations. The
  script also honours `TORII_AUTH_TOKEN`/`TORII_API_TOKEN` and accepts private
  keys via `PRIVATE_KEY=ed25519:<hex>` or `PRIVATE_KEY_HEX=<hex>`.
- Prints the Torii responses (code/ABI hashes, namespace bindings) so CI jobs
  can archive evidence bundles alongside release artifacts.

Run with:

```bash
npm install
node ./recipes/contracts.mjs \
  TORII_URL=https://torii.devnet.example \
  AUTHORITY=sorauロ1Nタセhjセ7pZaG9L7エmBnクbヨ9ヰsウ4dqmナコmチホ24CウオEAE9L4 \
  PRIVATE_KEY_HEX=$(cat ~/.iroha/keys/alice.hex) \
  CONTRACT_CODE_PATH=./artifacts/demo_contract.to \
  CONTRACT_MANIFEST_PATH=./artifacts/demo_manifest.json \
  CONTRACT_STAGE=both \
  CONTRACT_NAMESPACE=apps \
  CONTRACT_ID=demo.contract
```

Environment variables:

- `CONTRACT_STAGE` — `register`, `instance`, or `both` (default) to control which REST calls run.
- `CONTRACT_CODE_PATH` — path to the Kotodama `.to` artifact (required).
- `CONTRACT_MANIFEST_PATH` / `CONTRACT_MANIFEST_JSON` — manifest source (optional but recommended).
- `CONTRACT_NAMESPACE` / `CONTRACT_ID` — required when staging the `instance` leg.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — optional headers for locked-down deployments.
- `PRIVATE_KEY` / `PRIVATE_KEY_HEX` — signer credentials; defaults to `ed25519` when unspecified.

## streaming.mjs

- Streams `/v1/events/sse` with a deterministic filter (pipeline transactions by default).
- Persists the `Last-Event-ID` cursor so runs resume after restarts.
- Surfaces pipeline status transitions using `extractPipelineStatusKind` so runbooks can capture
  applied/committed transitions side-by-side with the live JSON payload.

Run with:

```bash
npm install
node ./recipes/streaming.mjs \
  TORII_URL=https://torii.nexus.example \
  PIPELINE_STATUS=Committed \
  STREAM_MAX_EVENTS=25 \
  STREAM_CURSOR_FILE=.cache/torii.cursor
```

Environment variables:

- `STREAM_FILTER_JSON` — override the default pipeline transaction filter with raw JSON.
- `PIPELINE_STATUS` — change the default status filter (Committed/Approved/Applied/Pending, etc.).
- `STREAM_MAX_EVENTS` — stop after N events (`0` keeps the iterator running until interrupted).
- `STREAM_CURSOR_FILE` — path to the cursor file storing the latest `Last-Event-ID`.
- `TORII_API_TOKEN` / `TORII_AUTH_TOKEN` — optional headers for locked-down Torii deployments.

## assets_iterators.mjs

- Iterates NFTs and per-account asset holdings using the pagination helpers
  (`iterateNftsQuery`, `iterateAccountAssets`), applying Norito-style sort and
  filter envelopes (id equality for NFTs, optional quantity filters for assets)
  so the output mirrors Torii JSON responses.
- Uses `requirePermissions: true` so secured Torii deployments fail fast when
  API/auth tokens are missing; page sizes and caps can be tuned via env vars.

Run with:

```bash
npm install
TORII_URL=http://localhost:8080 \
ACCOUNT_ID=sorauロ1Nタセhjセ7pZaG9L7エmBnクbヨ9ヰsウ4dqmナコmチホ24CウオEAE9L4 \
NFT_ID=61CtjvNd9T3THAR65GsMVHr82Bjc#sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D \
PAGE_SIZE=25 \
MAX_ITEMS=100 \
node ./recipes/assets_iterators.mjs
```

Environment variables:

- `TORII_URL` — Torii endpoint (defaults to `http://localhost:8080`).
- `ACCOUNT_ID` — account literal for asset iteration.
- `NFT_ID` — optional canonical NFT/asset-holding id (`<base58-asset-definition-id>#<katakana-i105-account-id>`) to filter on (exact match).
- `PAGE_SIZE` / `MAX_ITEMS` — pagination controls.
- `TORII_API_TOKEN` / `TORII_AUTH_TOKEN` — credentials for permissioned nodes.
- `ALLOW_INSECURE=1` — allow HTTP while sending credentials (dev/test only).

## offline_queue.mjs

- Simulates Connect offline queueing with `ConnectQueueJournal`, replaying the oldest
  app-to-wallet frame and emitting snapshots/metrics before exporting an evidence bundle.
- Exercises the diagnostics helpers (`updateConnectQueueSnapshot`, `appendConnectQueueMetric`,
  `exportConnectQueueEvidence`) used by the offline runbook so operators can gather audits without
  hitting Torii.

Run with:

```bash
npm install
npm run example:offline
```

Environment variables:

- `CONNECT_SESSION_ID` — Connect session id to seed the queue (base64url). Default:
  `AQIDBAUGBwgJCgsMDQ4PEA`.
- `CONNECT_QUEUE_ROOT` — where snapshots/metrics/evidence are written. Default:
  `artifacts/js/connect_offline`.
- `CONNECT_WARNING_WATERMARK` / `CONNECT_DROP_WATERMARK` — optional 0–1 floats applied to the
  exported snapshot to mimic production watermarks.
- `CONNECT_NOW_MS` — override the deterministic timestamp used for emitted frames and snapshots.
- `CONNECT_APP_PAYLOAD` / `CONNECT_WALLET_PAYLOAD` — optional payload overrides for the staged
  frames (defaults are deterministic strings).

## offline_pipeline.mjs

- Builds a signed transaction with the native Norito helpers, packages it into an offline envelope,
  writes JSON to disk, and replays the stored envelope against Torii.
- Defaults to an in-process mock Torii server to keep the flow deterministic; set
  `OFFLINE_PIPELINE_USE_MOCK=0` and `TORII_BASE_URL` to target a live node.
- Supports deterministic build-only validation with `OFFLINE_PIPELINE_SKIP_REPLAY=1`, which exits
  after envelope build/parse/write and skips mock/live replay.
- Logs the envelope hash, schema, and metadata so you can archive evidence alongside the stored
  payload.

Run with:

```bash
npm install
npm run build:native
npm run example:offline:pipeline
```

Environment variables:

- `TORII_BASE_URL` — Torii endpoint used for replay; ignored when the mock server is enabled.
- `OFFLINE_PIPELINE_OUT_DIR` — output directory for the stored envelope (default:
  `artifacts/js/offline_pipeline`).
- `OFFLINE_PIPELINE_USE_MOCK` — set to `0` to skip the mock server and use `TORII_BASE_URL`.
- `OFFLINE_PIPELINE_SKIP_REPLAY` — set to `1` to stop after build/parse/write and skip replay.

## governance.mjs

- Builds sample transactions for governance flows (propose deploy, cast ballot,
  enact/finalize referendum, persist council) using the SDK builders.
- Prints deterministic hashes for each transaction; optionally submits them to
  Torii when `GOV_SUBMIT=1`.
- When `GOV_FETCH=1`, reuses the new `ToriiClient` governance helpers to fetch proposals, tallies,
  locks, and unlock stats so you can inspect live state after submitting transactions.

Run with:

```bash
npm install
npm run build:native
node ./recipes/governance.mjs
```

Set `TORII_URL`, `CHAIN_ID`, `AUTHORITY`, and `PRIVATE_KEY_HEX` (32- or 64-byte Ed25519 key)
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
