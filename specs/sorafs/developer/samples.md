---
title: Sample Projects
summary: Ready-to-run snippets that demonstrate the SoraFS packaging and proof flows.
---

# Sample Projects

The snippets below mirror the workflows described in the quickstart and CLI
cookbook while remaining small enough to drop into test harnesses or CI
pipelines. Companion files live in `fixtures/documentation/` so you can copy them into a
repository without hunting through larger scripts.

## Shell script — end-to-end CLI pipeline

`fixtures/documentation/sorafs_cli_quickstart.sh` ties together CAR packing, manifest
building, submission, proof streaming, and verification. The script
expects the following environment variables:

- `SORA_PAYLOAD` — path to the file or directory you want to package.
- `SORA_OUTPUT_DIR` — directory for generated artefacts (defaults to `artifacts`).
- `TORII_URL` — Torii endpoint for manifest submission.
- `SORA_AUTHORITY` / `SORA_PRIVATE_KEY` — Norito account + Ed25519 private key.
- Optional: `SORA_PROOF_ENDPOINT`, `SORA_STREAM_TOKEN`, and `SORA_PROVIDER_ID`
  to exercise the proof streaming command.

The script matches the commands documented in
`specs/sorafs_cli.md` and prints a JSON summary for each stage so CI jobs
can archive results.

```bash
chmod +x fixtures/documentation/sorafs_cli_quickstart.sh
./fixtures/documentation/sorafs_cli_quickstart.sh
```

## Rust — proof stream aggregation helper

`fixtures/documentation/sorafs_rust_proof_stream.rs` shows how to embed the proof stream
metrics aggregator inside a Rust service. It reuses the same `ProofStreamItem`
and `ProofStreamSummary` types that back `sorafs_cli proof stream`, making it
easy to share logic between automation and long-lived processes.

The example fetches NDJSON items from a gateway (or fixture) and returns a
JSON blob mirroring the CLI output. Use it as a starting point when wiring
observability into custom orchestrators or unit tests.

```bash
cargo add sorafs_car norito reqwest hex rand httpmock --dev
```

Add the file to your test suite (for example under `tests/`) and invoke the
`fetch_and_summarise` helper from integration tests or background tasks.

## Content-manifest fixtures

`fixtures/sorafs_manifest/ci_sample/` captures the entire packaging pipeline for
a deterministic text payload: the CAR archive, chunk plan, manifest (`.to` plus
JSON), and proof summary. These are deterministic test inputs, not release
signatures. Production authenticity applies to the aggregate release manifest
through `signing_provider=authenticated_external_signer` with exact
`signing_backend=software`; accepted output is
`signer_qualification=software-key-qualified`. Pair the fixtures with
`fixtures/documentation/sorafs_ci_sample/` when you need a ready-to-clone repository
layout or moustache template for release notes.

## TypeScript — reference payload validation

The JavaScript/TypeScript SDK exposes the native SoraFS reference validator from
`@iroha/iroha-js/sorafs`. Build or install the checksum-pinned native module,
then validate the canonical Norito bytes directly:

```ts
import { readFileSync } from "node:fs";
import {
  SORAFS_ORDERBOOK_PAYLOAD_KINDS,
  validateOrderbookPayload,
} from "@iroha/iroha-js/sorafs";

const payload = readFileSync(
  "fixtures/sorafs_manifest/orderbook/order_request_v1.to",
);
const outcome = validateOrderbookPayload(
  SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
  payload,
  { generatedAtUnix: 1_700_001_234 },
);

if (outcome.status !== "Ok") {
  throw new Error(`${outcome.code}: ${outcome.message}`);
}
```

The wrapper returns the byte-for-byte `ValidationOutcomeV1` JSON contract used
by the committed positive and negative fixtures. It fails closed when the
authenticated native module or required validation export is unavailable; it
does not replace production ledger admission.

## Where to go next

- **Quickstart walkthrough:** `specs/sorafs/developer/overview.md`
- **CI templates:** `fixtures/documentation/sorafs_ci.md`
- **Proof telemetry schema:** `specs/sorafs_proof_streaming.md`

If you build a workflow that others might reuse, drop it into
`fixtures/documentation/` and update this page so the catalogue stays fresh.
