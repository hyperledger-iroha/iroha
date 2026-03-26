---
lang: ar
direction: rtl
source: docs/source/sdk/js/governance_iso_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79c0b0771d1c25facdeb5757f066188e2cba7890402259bc52be6d3fda3e589f
source_last_modified: "2026-01-22T07:33:23.443490+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Governance & ISO Bridge Examples

This field guide expands on the quickstart by showing how to drive advanced
governance and ISO&nbsp;20022 bridge flows with `@iroha/iroha-js`. The examples
reuse the production methods that ship with `ToriiClient`, so you can copy the
snippets directly into CLI tools, CI harnesses, or long-running services.

See also:

- `javascript/iroha_js/recipes/governance.mjs` — runnable end-to-end script for
  proposals, ballots, and council rotations.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — CLI helper for submitting
  pacs.008/pacs.009 payloads and polling their deterministic status.
- `docs/source/finance/settlement_iso_mapping.md` — canonical field mapping for
  ISO flows.

## Running the bundled recipes

The examples below rely on the scripts in `javascript/iroha_js/recipes/`.
Always run `npm install && npm run build:native` first so the TypeScript
definitions and the optional native helpers are available.

### Governance helper walkthrough

The governance script prints deterministic transaction hashes by default and can
optionally push them to Torii or fetch the resulting state. Set the following
environment variables before invoking it:

- `TORII_URL` — Torii endpoint.
- `AUTHORITY` and `PRIVATE_KEY_HEX` — signer account and key (hex). Store keys in
  a secure secret manager; never commit them to git.
- `CHAIN_ID` — optional network id (defaults to the zero UUID).
- `GOV_SUBMIT=1` — submit the signed bytes to Torii.
- `GOV_FETCH=1` — fetch proposal/lock snapshots after submission.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — optional lookups for
  `GOV_FETCH`.

```bash
# Build once so the recipes can import the compiled bindings.
npm run build:native

# Print hashes only (safe for CI rehearsals).
TORII_URL=https://torii.testnet.sora node javascript/iroha_js/recipes/governance.mjs

# Submit to a node and fetch the resulting state.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=<katakana-i105-account-id> \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

Every step logs the deterministic hash for auditing. When `GOV_SUBMIT=1` the
script also reports Torii submission responses (or explicit errors) so CI jobs
can gate on success.

### ISO bridge helper

The ISO bridge recipe submits either a pacs.008 (DvP) or pacs.009 (PvP funding)
message and polls Torii until the bridge reports a terminal status. Sample
payloads are generated with the SDK’s schema-compliant builders so the agents,
purpose code, and settlement dates match the ISO 20022 envelope expected by the
bridge. Configure the helper with the following environment variables:

- `TORII_URL` — Torii endpoint that exposes the ISO bridge routes.
- `ISO_MESSAGE_KIND` — `pacs.008` (default) or `pacs.009`.
- `ISO_MESSAGE_ID` — skip submission and only poll the supplied message id.
- `ISO_MESSAGE_SUFFIX` — optional suffix appended to the sample payload IDs to keep repeated demo runs unique (defaults to the current epoch seconds in hex).
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — override the wait strategy.
- `ISO_CONTENT_TYPE` — optional MIME override for submissions (for example `application/pacs009+xml`); ignored when `ISO_MESSAGE_ID` is set because no submission happens.
- `ISO_RESOLVE_ON_ACCEPTED=1` — exit as soon as the bridge marks a submission as
  `Accepted` even if the transaction hash is still pending.

Use the `contentType` option whenever your payload uses a more specific MIME
type. The SDK enforces that overrides are non-empty strings, so mistakes are
caught locally before Torii sees a malformed header. Call the `AndWait`
convenience wrapper when you want the helper to poll `/v1/iso20022/status`
before returning:

```js
await torii.submitIsoPacs009AndWait(xmlPayload, {
  contentType: "application/pacs009+xml",
  wait: { maxAttempts: 20, pollIntervalMs: 1_000 },
});
```

```bash
# Submit a sample pacs.009 payload and wait for a final status.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_KIND=pacs.009 \
ISO_POLL_ATTEMPTS=20 \
ISO_POLL_INTERVAL_MS=1500 \
node javascript/iroha_js/recipes/iso_bridge.mjs

# Reuse an existing message id and only poll for status changes.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_ID=iso-demo-1 \
node javascript/iroha_js/recipes/iso_bridge.mjs
```

Both helpers exit with a non-zero status if Torii never reports a terminal
state, making them safe to embed in CI pipelines.

Bridge responses are normalised and validated: `status` is constrained to
`Pending`, `Accepted`, or `Rejected`, and `pacs002_code` must be one of `ACTC`,
`ACSP`, `ACSC`, `ACWC`, `PDNG`, or `RJCT`. Unexpected values raise a
`TypeError` so scripts catch unsupported bridge states before acting on them.

When you already have structured ISO fields handy, call
`torii.submitIsoMessage()` and let the SDK build the XML for you. The helper
accepts `kind: "pacs.009"` (defaults to pacs.008), applies a pacs-specific
`Content-Type`, and reuses the same `AbortSignal` for both submission and
polling:

```js
await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
  },
  { kind: "pacs.009", wait: { maxAttempts: 5, pollIntervalMs: 1500 } },
);
```

### ISO alias helper

`recipes/iso_alias.mjs` exercises the alias endpoints that back the ISO bridge.
It evaluates a blinded alias element via `ToriiClient.evaluateAliasVoprf` and
resolves aliases either by literal label (IBAN-style strings) or by deterministic
index (`resolveAlias` / `resolveAliasByIndex`). Configure it with:

- `TORII_URL` — Torii endpoint exposing the ISO alias APIs.
- `ISO_VOPRF_INPUT` — hex-encoded blinded element forwarded to the VOPRF helper
  (defaults to `deadbeef`). Set `ISO_SKIP_VOPRF=1` to skip this call.
- `ISO_ALIAS_LABEL` — literal alias to resolve; omit when only testing VOPRF or indexed lookups.
- `ISO_ALIAS_INDEX` — decimal or `0x`-prefixed index used with `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — optional headers for locked-down deployments.

```bash
# Evaluate a blinded element and resolve both a label and deterministic index.
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeefcafebabe \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node javascript/iroha_js/recipes/iso_alias.mjs

# Skip VOPRF and only resolve a stored alias.
TORII_URL=https://torii.testnet.sora \
ISO_SKIP_VOPRF=1 \
ISO_ALIAS_LABEL="iso:demo:alpha" \
node javascript/iroha_js/recipes/iso_alias.mjs
```

The script prints the backend/digest metadata for the VOPRF helper and displays
the account, source, and deterministic index returned by the alias resolution
endpoints. When the ISO bridge runtime is disabled, the helper reports the same
error message surfaced by Torii so CI runs can treat it as a soft skip.

The ISO message builders apply the same identifier validation rules captured in
[`docs/source/finance/settlement_iso_mapping.md`](../../finance/settlement_iso_mapping.md);
for example, IBAN inputs must satisfy the canonical mod-97 checksum in addition
to matching the `15–34` character format. This keeps SDK-generated payloads in
lockstep with Torii’s runtime policy.

## Governance workflows

### Inspect contract instances and proposals

```javascript
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

// Abort long-running reads with AbortController.
const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### Submit proposals and ballots

All governance mutation helpers accept an optional `{ signal }` object so UI layers can cancel
long-running submissions or tie them to component lifecycles.

```javascript
const authority = "<katakana-i105-account-id>";
const privateKey = Buffer.alloc(32, 0xaa);

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

const zkOwner = "<katakana-i105-account-id>"; // canonical Katakana i105 account id for ZK public inputs
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

```javascript
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "<katakana-i105-account-id>",
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

```javascript
import { buildPacs008Message } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "<katakana-i105-account-id>" },
  purposeCode: "SECU",
  supplementaryData: { account_id: "<katakana-i105-account-id>", leg: "delivery" },
});
```

All identifiers (BIC, LEI, IBAN, ISO amount) are validated before XML is
generated. Swap `buildPacs008Message` for `buildPacs009Message` to emit PvP
funding payloads.

### Submit and poll ISO messages

```javascript
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

// Already have a message id? Just wait for status changes.
await torii.waitForIsoMessageStatus(process.env.ISO_MESSAGE_ID, {
  maxAttempts: 10,
  pollIntervalMs: 2_000,
});
```

Both `resolveOnAccepted` and `resolveOnAcceptedWithoutTransaction` are accepted; use whichever
fits your coding style when you want the helper to treat `Accepted` statuses (without a transaction
hash) as terminal.

Both helpers throw `IsoMessageTimeoutError` if the bridge never reports a
terminal state. The lower-level `submitIsoPacs008` / `submitIsoPacs009` methods
return immediately when you need to orchestrate custom polling logic, and
`getIsoMessageStatus` exposes a single-shot lookup.

### Related surfaces

- `torii.getSorafsPorWeeklyReport("2026-W05")` fetches the ISO-week PoR bundle
  referenced in the roadmap and can reuse the wait helpers above to build
  alerts/dashboards.
- `resolveAlias` / `resolveAliasByIndex` expose ISO bridge alias bindings so
  billing and reconciliation tools can prove account ownership before issuing a
  payment instruction.

### Troubleshooting ISO waits & capturing evidence

`waitForIsoMessageStatus` exposes the same status vocabulary returned by
Torii (`pending`, `accepted`, `committed`, `rejected`). Use the table below to
decide whether to keep polling or surface an error:

| Status | Meaning | Typical next step |
|--------|---------|-------------------|
| `Pending` / `Accepted` | Bridge accepted the payload but has not attached a Torii transaction hash yet. | Continue polling, or set `resolveOnAccepted`/`resolveOnAcceptedWithoutTransaction` to treat this as terminal when mediation happens off-ledger. |
| `Committed` | Torii recorded the mint/burn leg and exposes `transaction_hash`. | Store the status JSON alongside the settlement record for auditors. |
| `Rejected` | Bridge validation failed; payload includes `reason` and `error_code`. | Escalate with the payload + reason fields and do not retry until the defect is fixed. |
| `null` | Bridge has no knowledge of the message id. | Double-check the ID (e.g., typo vs. environment mix-up) before retrying. |

When the helper exhausts `maxAttempts` or `timeoutMs` it raises
`IsoMessageTimeoutError` (exported from `@iroha/iroha-js`). Catch it to record
diagnostics or to fall back to manual status lookups:

```javascript
import { ToriiClient, IsoMessageTimeoutError } from "@iroha/iroha-js";

try {
  await torii.waitForIsoMessageStatus(messageId, {
    pollIntervalMs: 3_000,
    maxAttempts: 12,
    onPoll: ({ attempt, status }) => {
      console.info(`[iso] attempt=${attempt} status=${status?.status ?? "pending"}`);
    },
  });
} catch (error) {
  if (error instanceof IsoMessageTimeoutError) {
    console.warn(
      `ISO message ${error.messageId} stalled after ${error.attempts} attempts`,
      error.lastStatus,
    );
    const latest = await torii.getIsoMessageStatus(error.messageId, {
      retryProfile: "iso-status",
    });
    console.warn("latest bridge snapshot", latest);
  } else {
    throw error;
  }
}
```

Store the status payload (or timeout snapshot) with the rest of the settlement
evidence:

1. Archive the PoR bundle surfaced by `getSorafsPorWeeklyReport` for the target
   ISO week; auditors expect the `.prom` metrics and JSON summary to match the
   settlement window noted in the ticket.
2. Capture the alias proof/lookup evidence (via `resolveAlias` or the ISO alias
   helper) that maps the IBAN/BIC/alias back to the on-ledger account.
3. Keep `waitForIsoMessageStatus` logs, the returned JSON, and any timeout
   exceptions in the release artefacts so the roadmap’s JS-06 gate can audit
   retries and error handling without scraping CI logs.

Automation tip: wire `onPoll` to your telemetry sink (OpenTelemetry, CloudWatch,
etc.) so failed attempts include the `message_id`, elapsed time, and current
status. This makes it easy to diff `ISO_RESOLVE_ON_ACCEPTED=1` runs against the
default “wait for committed” behaviour and provides the audit trail referenced
in `docs/source/finance/settlement_iso_mapping.md`.
