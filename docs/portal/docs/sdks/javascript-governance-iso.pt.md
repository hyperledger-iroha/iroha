---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 885f89984341598dfb4423efc8bcbf139bbf749e37d398e6c23f34399414bfe3
source_last_modified: "2026-01-22T07:33:17.283789+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
slug: /sdks/javascript/governance-iso-examples
---

This field guide expands on the quickstart by demonstrating governance and
ISO&nbsp;20022 bridge flows with `@iroha/iroha-js`. The snippets reuse the same
runtime helpers that ship with `ToriiClient`, so you can copy them directly into
CLI tooling, CI harnesses, or long-running services.

Additional resources:

- `javascript/iroha_js/recipes/governance.mjs` — runnable end-to-end script for
  proposals, ballots, and council rotations.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — CLI helper for submitting
  pacs.008/pacs.009 payloads and polling deterministic status.
- `docs/source/finance/settlement_iso_mapping.md` — canonical ISO field mapping.

## Running the bundled recipes

These examples depend on the scripts in `javascript/iroha_js/recipes/`. Run
`npm install && npm run build:native` beforehand so the generated bindings are
available.

### Governance helper walkthrough

Configure the following environment variables before invoking
`recipes/governance.mjs`:

- `TORII_URL` — Torii endpoint.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — signer account and key (hex). Keep keys in a
  secure secret store.
- `CHAIN_ID` — optional network identifier.
- `GOV_SUBMIT=1` — push the generated transactions to Torii.
- `GOV_FETCH=1` — fetch proposals/locks after submission.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — optional lookups used
  when `GOV_FETCH=1`.

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=<i105-account-id> \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

Hashes are logged for every step, and Torii responses are surfaced when
`GOV_SUBMIT=1` so CI jobs can fail fast on submission errors.

### ISO bridge helper

`recipes/iso_bridge.mjs` submits either a pacs.008 or pacs.009 message and polls
the ISO bridge until the status settles. Configure it with:

- `TORII_URL` — Torii endpoint exposing the ISO bridge APIs.
- `ISO_MESSAGE_KIND` — `pacs.008` (default) or `pacs.009`. The helper uses the
  matching sample builder (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  when you do not supply your own XML.
- `ISO_MESSAGE_SUFFIX` — optional suffix appended to the sample payload IDs to
  keep repeated rehearsals unique (defaults to the current epoch seconds in hex).
- `ISO_CONTENT_TYPE` — override the `Content-Type` header for submissions
  (for example `application/pacs009+xml`); ignored when you only poll an
  existing message id.
- `ISO_MESSAGE_ID` — skip submission altogether and only poll the supplied
  identifier via `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — tune the wait strategy for
  noisy or slow bridge deployments.
- `ISO_RESOLVE_ON_ACCEPTED=1` — exit as soon as Torii returns `Accepted`,
  even if the transaction hash is still pending (handy during bridge maintenance
  when the ledger commit is delayed).

```bash
# Submit a pacs.009 message and wait for completion.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_KIND=pacs.009 \
ISO_POLL_ATTEMPTS=20 \
ISO_POLL_INTERVAL_MS=1500 \
node javascript/iroha_js/recipes/iso_bridge.mjs

# Poll an existing message id without re-submitting XML.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_ID=iso-demo-1 \
node javascript/iroha_js/recipes/iso_bridge.mjs
```

Both scripts exit with status code `1` if Torii never reports a terminal
transition, making them suitable for CI gate jobs.

### ISO alias helper

`recipes/iso_alias.mjs` targets the ISO alias endpoints so rehearsals can cover
blinded-element hashing and alias lookups without writing bespoke tooling. It
calls `ToriiClient.evaluateAliasVoprf` plus `resolveAlias` / `resolveAliasByIndex`
and prints the backend, digest, account binding, source, and deterministic index
returned by Torii.

Environment variables:

- `TORII_URL` — Torii endpoint exposing the alias helpers.
- `ISO_VOPRF_INPUT` — hex-encoded blinded element (defaults to `deadbeef`).
- `ISO_SKIP_VOPRF=1` — skip the VOPRF call when only testing lookups.
- `ISO_ALIAS_LABEL` — literal alias to resolve (e.g., IBAN-style strings).
- `ISO_ALIAS_INDEX` — decimal or `0x`-prefixed index passed to `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — optional headers for secured Torii deployments.

```bash
# Evaluate a blinded element and resolve an alias literal + deterministic index.
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeefcafebabe \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node javascript/iroha_js/recipes/iso_alias.mjs

# Only perform literal resolution.
TORII_URL=https://torii.testnet.sora \
ISO_SKIP_VOPRF=1 \
ISO_ALIAS_LABEL="iso:demo:alpha" \
node javascript/iroha_js/recipes/iso_alias.mjs
```

The helper mirrors Torii’s behaviour: it surfaces 404s when aliases are missing
and treats runtime-disabled errors as soft skips so CI flows can tolerate bridge
maintenance windows.

## Governance workflows

### Inspect contract instances and proposals

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const instances = await torii.listGovernanceInstances("apps", {
  contains: "ledger",
  hashPrefix: "deadbeef",
  order: "hash_desc",
  limit: 5,
});
for (const entry of instances.instances) {
  console.log(`${entry.contract_id} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### Submit proposals and ballots

Use an `AbortController` when you need to cancel or time-bound governance submissions—the SDK
accepts an optional `{ signal }` object for every POST helper shown below.

```ts
const authority = "<i105-account-id>";
const privateKey = Buffer.alloc(32, 0xaa);

// All governance writes accept optional `{ signal }` options for cancellation.
const writeController = new AbortController();
const deployDraft = await torii.governanceProposeDeployContract({
  namespace: "apps",
  contractId: "calc.v1",
  codeHash: "hash:7B38...#ABCD",
  abiHash: Buffer.alloc(32, 0xbb),
  abiVersion: "1",
  window: { lower: 12_345, upper: 12_500 },
  mode: "Plain",
}, { signal: writeController.signal });
console.log("draft instructions", deployDraft.tx_instructions.length);

const ballot = await torii.governanceSubmitPlainBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  referendumId: "ref-plain",
  owner: authority,
  amount: "5000",
  durationBlocks: 7_200,
  direction: "Aye",
}, { signal: writeController.signal });
if (!ballot.accepted) {
  console.warn("ballot rejected", ballot.reason);
}

const zkOwner = "<i105-account-id>"; // canonical I105 account id for ZK public inputs
await torii.governanceSubmitZkBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  electionId: "ref-zk",
  proof: Buffer.alloc(96, 0xcd),
  public: {
    owner: zkOwner,
    amount: "5000",
    duration_blocks: 7_200,
    direction: "Aye",
  },
}, { signal: writeController.signal });
```

### Council VRF and enactment

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "<i105-account-id>",
      variant: "Normal",
      pk: validatorPk,
      proof: validatorProof,
    },
  ],
}, { signal: writeController.signal });
await torii.governancePersistCouncil({
  committeeSize: derived.members.length,
  candidates: derived.members.map((member) => ({
    accountId: member.account_id,
    variant: "Normal",
    pk: validatorPk,
    proof: validatorProof,
  })),
  authority,
  privateKey,
}, { signal: writeController.signal });

const finalizeDraft = await torii.governanceFinalizeReferendumTyped({
  referendumId: "ref-mainnet-001",
  proposalId: "0123abcd...beef",
}, { signal: writeController.signal });
console.log("finalize tx count", finalizeDraft.tx_instructions.length);

const enactDraft = await torii.governanceEnactProposalTyped({
  proposalId: "abcd0123...cafe",
  window: { lower: 10, upper: 25 },
}, { signal: writeController.signal });
console.log("enact tx count", enactDraft.tx_instructions.length);
```

## ISO&nbsp;20022 bridge recipes

### Build pacs.008 / pacs.009 payloads

```ts
import { buildPacs008Message } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "<i105-account-id>" },
  purposeCode: "SECU",
  supplementaryData: { account_id: "<i105-account-id>", leg: "delivery" },
});
```

All identifiers (BIC, LEI, IBAN, ISO amount) are validated before XML is
generated. Swap `buildPacs008Message` for `buildPacs009Message` to emit PvP
funding payloads.

### Submit and poll ISO messages

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const status = await torii.submitIsoPacs008AndWait(settlement, {
  wait: {
    maxAttempts: Number(process.env.ISO_POLL_ATTEMPTS ?? 20),
    pollIntervalMs: Number(process.env.ISO_POLL_INTERVAL_MS ?? 3_000),
    resolveOnAccepted: process.env.ISO_RESOLVE_ON_ACCEPTED === "1",
    onPoll: ({ attempt, status: snapshot }) => {
      console.log(`[attempt ${attempt}] status=${snapshot?.status ?? "pending"}`);
    },
  },
});
console.log(status.message_id, status.status, status.transaction_hash);

await torii.waitForIsoMessageStatus(process.env.ISO_MESSAGE_ID!, {
  maxAttempts: 10,
  pollIntervalMs: 2_000,
});

// Build XML on the fly from structured fields (skips the sample payloads).
await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
  },
  {
    kind: "pacs.009",
    wait: { maxAttempts: 5, pollIntervalMs: 1_500 },
  },
);
```

Both `resolveOnAccepted` and `resolveOnAcceptedWithoutTransaction` are valid; use either flag
to treat `Accepted` statuses (without a transaction hash) as terminal when orchestrating polls.

The helpers throw `IsoMessageTimeoutError` if the bridge never reports a
terminal state. Use the lower-level `submitIsoPacs008` / `submitIsoPacs009`
calls when you need to orchestrate custom polling logic; `getIsoMessageStatus`
exposes a single-shot lookup.

### Related surfaces

- `torii.getSorafsPorWeeklyReport("2026-W05")` fetches the ISO-week PoR bundle
  referenced in the roadmap and can reuse the wait helpers for alerts.
- `resolveAlias` / `resolveAliasByIndex` expose ISO bridge alias bindings so
  reconciliation tools can prove account ownership before issuing a payment.
