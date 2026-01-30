---
lang: ja
direction: ltr
source: docs/source/sorafs/developer/samples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5945ab49bb8c2c5a32c0efcd2cded510764e72f3eafb853a38c1611e0f1ebf2b
source_last_modified: "2026-01-03T18:07:58.344734+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Sample Projects
summary: Ready-to-run snippets that demonstrate the SoraFS packaging and proof flows.
---

# Sample Projects

The snippets below mirror the workflows described in the quickstart and CLI
cookbook while remaining small enough to drop into test harnesses or CI
pipelines. Companion files live in `docs/examples/` so you can copy them into a
repository without hunting through larger scripts.

## Shell script — end-to-end CLI pipeline

`docs/examples/sorafs_cli_quickstart.sh` ties together CAR packing, manifest
building, Sigstore signing, proof streaming, and verification. The script
expects the following environment variables:

- `SORA_PAYLOAD` — path to the file or directory you want to package.
- `SORA_OUTPUT_DIR` — directory for generated artefacts (defaults to `artifacts`).
- `SIGSTORE_ID_TOKEN` — OIDC token (exported automatically inside GitHub/GitLab
  pipelines).
- `TORII_URL` — Torii endpoint for manifest submission.
- `SORA_AUTHORITY` / `SORA_PRIVATE_KEY` — Norito account + Ed25519 private key.
- Optional: `SORA_PROOF_ENDPOINT`, `SORA_STREAM_TOKEN`, and `SORA_PROVIDER_ID`
  to exercise the proof streaming command.

The script matches the commands documented in
`docs/source/sorafs_cli.md` and prints a JSON summary for each stage so CI jobs
can archive results.

```bash
chmod +x docs/examples/sorafs_cli_quickstart.sh
./docs/examples/sorafs_cli_quickstart.sh
```

## Rust — proof stream aggregation helper

`docs/examples/sorafs_rust_proof_stream.rs` shows how to embed the proof stream
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

## Manifest + bundle fixtures

`fixtures/sorafs_manifest/ci_sample/` captures the entire packaging pipeline for
a deterministic text payload: the CAR archive, chunk plan, manifest (`.to` plus
JSON), signature bundle, detached signature, proof summary, and the CLI output
from `sorafs_cli manifest sign`. The fixtures use a reproducible GitHub
Actions–style JWT and fixed `--issued-at` timestamp so hashes stay stable across
test runs. Pair them with `docs/examples/sorafs_ci_sample/` when you need a
ready-to-clone repository layout or moustache template for release notes.

## TypeScript — manifest validation stub

Until the dedicated SDKs ship, front-end tooling can shell out to the CLI while
validating artefacts using Norito JSON summaries. The snippet below performs
basic checks in Node.js:

```ts
import { readFileSync } from "node:fs";

interface ManifestSummary {
  manifest_blake3_hex: string;
  chunk_digest_sha3_hex: string;
  pin_policy: { min_replicas: number; storage_class: string };
}

const summaryPath = process.argv[2];
if (!summaryPath) {
  throw new Error("usage: ts-node validate-manifest.ts <summary.json>");
}

const summary = JSON.parse(
  readFileSync(summaryPath, { encoding: "utf8" })
) as ManifestSummary;

if (summary.pin_policy.min_replicas < 3) {
  throw new Error("pin policy must require at least 3 replicas");
}
if (summary.pin_policy.storage_class !== "warm") {
  throw new Error("storage class must be warm for production artefacts");
}

console.log(
  `manifest ${summary.manifest_blake3_hex} with chunk digest ${summary.chunk_digest_sha3_hex} looks good`,
);
```

Pair this helper with `sorafs_cli manifest build --manifest-json-out` to gate
PRs on policy defaults without pulling in the full Rust toolchain.

## Where to go next

- **Quickstart walkthrough:** `docs/source/sorafs/developer/overview.md`
- **CI templates:** `docs/examples/sorafs_ci.md`
- **Proof telemetry schema:** `docs/source/sorafs_proof_streaming.md`

If you build a workflow that others might reuse, drop it into
`docs/examples/` and update this page so the catalogue stays fresh.
