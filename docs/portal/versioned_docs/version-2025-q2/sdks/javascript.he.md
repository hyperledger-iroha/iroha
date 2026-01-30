---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-01-30
---

# JavaScript SDK Quickstart

`@iroha2/torii-client` provides a browser and Node.js friendly wrapper around Torii.
This quickstart mirrors the core flows from the SDK recipes so you can get a
client running in a few minutes. For fuller examples, see
`javascript/iroha_js/recipes/` in the repository.

## 1. Install

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

If you plan to sign transactions locally, also install the crypto helpers:

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Create a Torii client

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

The configuration mirrors the constructor used in the recipes. If your node
uses basic auth, pass `{username, password}` via the `basicAuth` option.

## 3. Fetch node status

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

All read operations return Norito-backed JSON objects. See the generated types in
`index.d.ts` for field details.

## 4. Submit a transaction

Signers can build transactions with the helper API:

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

The helper automatically wraps the transaction in the Norito envelope expected
by Torii. For a richer example (including waits for finality), see
`javascript/iroha_js/recipes/registration.mjs`.

## 5. Use high-level helpers

The SDK bundles specialised flows that mirror the CLI:

- **Governance helpers** – `recipes/governance.mjs` demonstrates staging
  proposals and ballots with the `governance` instruction builders.
- **ISO bridge** – `recipes/iso_bridge.mjs` shows how to submit `pacs.008` and
  poll transfer status using the `/v1/iso20022` endpoints.
- **SoraFS & triggers** – Pagination helpers under `src/toriiClient.js` expose
  typed iterators for contracts, assets, triggers, and SoraFS providers.

Import the relevant builder functions from `@iroha2/torii-client` to reuse those flows.

## 6. Error handling

All SDK calls throw rich `ToriiClientError` instances with transport metadata
and the Norito error payload. Wrap calls in `try/catch` or use `.catch()` to
surface context to users:

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## Next steps

- Explore the recipes in `javascript/iroha_js/recipes/` for end-to-end flows.
- Read the generated types in `javascript/iroha_js/index.d.ts` for detailed
  method signatures.
- Pair this SDK with the Norito quickstart to inspect and debug the payloads
  you send to Torii.
