---
lang: dz
direction: ltr
source: docs/source/sorafs_cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d471da0eaaa1f449a1e85b67fd2f858aa6972bb21aacfc73e8a947d9a75a2a69
source_last_modified: "2026-01-22T16:26:46.592293+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CLI
summary: Developer-facing entry point for packaging payloads and emitting chunk plans.
---

# SoraFS CLI

The `sorafs_cli` binary (built from the `sorafs_car` crate with the `cli`
feature) now bundles the full SoraFS packaging workflow instead of scattering
helpers across ad-hoc utilities. Core subcommands include:

_Looking for an end-to-end walkthrough?_ The quickstart at
`docs/source/sorafs/developer/overview.md` chains the key commands together for
local testing and CI.

- `norito build` — compile Kotodama `.ko` sources into deterministic IVM
  bytecode artefacts.
- `car pack` — produce a CAR archive, chunk-fetch plan, and JSON summary for CI.
- `manifest build` — translate the CAR summary into a Norito manifest.
- `manifest submit` — POST manifests (and optional alias proofs) to Torii,
  recomputing the chunk digest from a plan when provided.
- `proof verify` — validate CAR responses against a manifest and emit the
  PoR-ready digests required for registry admission.
- `manifest sign` — emit a keyless (OIDC-backed) signature bundle so pipelines
  can attest to published artefacts without long-lived signing keys.
- `manifest verify-signature` — confirm bundles or raw signatures match the
  manifest digest and (optionally) the computed chunk plan digests before
  promoting artefacts.

## Proxy remediation helper

When downgrade telemetry triggers an automated remediation, operators can flip the local QUIC proxy
between bridge and metadata-only modes without hand-editing JSON. The `proxy set-mode` helper
updates an orchestrator configuration, optionally writes the result to a new path, and emits a
machine-readable summary for audit trails.

```bash
./target/debug/sorafs_cli proxy set-mode \
  --orchestrator-config configs/workstation.json \
  --mode metadata-only \
  --json-out artifacts/proxy_remediation.json
```

Key flags:

- `--orchestrator-config=PATH` – required; points at the JSON configuration that contains
  `local_proxy`.
- `--mode=bridge|metadata-only` – required; selects the desired runtime mode.
- `--config-out=PATH` – optional; writes the updated configuration to a new file instead of editing
  the source configuration in place.
- `--json-out=PATH` – optional; writes the remediation summary to disk (otherwise printed to
  stdout).
- `--dry-run` – skip writing any configuration changes while still emitting the summary JSON.

The summary reports the previous and effective proxy modes, target configuration path, telemetry
label, and guard cache key so downstream automation can record the change or trigger follow-up
actions (for example, notifying browser extensions about the updated mode).

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  car pack \
  --input fixtures/payload.bin \
  --car-out artifacts/payload.car \
  --plan-out artifacts/chunk_plan.json \
  --summary-out artifacts/car_summary.json

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest build \
  --summary artifacts/car_summary.json \
  --manifest-out artifacts/manifest.to \
  --manifest-json-out artifacts/manifest.json \
  --pin-min-replicas=3 \
  --pin-storage-class=warm \
  --pin-retention-epoch=42

# Option 1: provide an explicit token (environment, file, or inline)
export SIGSTORE_ID_TOKEN=$(oidc-client fetch-token)
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest sign \
  --manifest artifacts/manifest.to \
  --bundle-out artifacts/manifest.bundle.json \
  --signature-out artifacts/manifest.sig \
  --identity-token-env=SIGSTORE_ID_TOKEN

# Option 2: rely on the default SIGSTORE_ID_TOKEN fallback
SIGSTORE_ID_TOKEN=$(oidc-client fetch-token) \
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest sign \
  --manifest artifacts/manifest.to \
  --bundle-out artifacts/manifest.bundle.json \
  --signature-out artifacts/manifest.sig

# Option 3: run inside GitHub Actions with `permissions: id-token: write`
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest sign \
  --manifest artifacts/manifest.to \
  --bundle-out artifacts/manifest.bundle.json \
  --signature-out artifacts/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs
```

Looking for a turnkey script? `docs/examples/sorafs_cli_quickstart.sh` wraps the
commands above into a single workflow that expects standard environment
variables (`SORA_PAYLOAD`, `TORII_URL`, `SIGSTORE_ID_TOKEN`, etc.) and writes
every summary JSON to an output directory. It is ideal for CI runners or for
developers who want to capture canonical artefacts without typing each step.
For release automation, see `scripts/release_sorafs_cli.sh`, which packages
the signing and verification flow into a single command that produces bundle,
signature, and summary outputs under `artifacts/sorafs_cli_release/`. The script
accepts a `--config` file (see `docs/examples/sorafs_cli_release.conf`) and
defaults to the curated fixtures in `fixtures/sorafs_manifest/ci_sample/`, so you
can run a dry-run without supplying arguments.
If you need deterministic fixtures to diff against, grab the bundle under
`fixtures/sorafs_manifest/ci_sample` and the accompanying README in
`docs/examples/sorafs_ci_sample/`.

Key behaviours:

- `--chunker-handle` defaults to `sorafs.sf1@1.0.0`. Pass an explicit handle to
  switch chunking profiles once the registry grows additional entries.
- The manifest builder reads the JSON emitted by `car pack`, applies optional
  pin-policy overrides (`--pin-*` flags), and writes both Norito binary output
  and (optionally) a JSON view for debugging.
- The CLI writes a pretty-printed summary to STDOUT and, when `--summary-out`
  is supplied, mirrors the same JSON to disk. Downstream automation can diff the
  summary or extract digests/TLV metadata without parsing the CAR file directly.
- Plans are rendered via `chunk_fetch_specs_to_string`, so the resulting JSON
  matches the format consumed by the multi-source orchestrator.
- `manifest submit` recomputes the chunk SHA3 digest when a plan is provided,
  validates alias inputs, and forwards Ed25519 keys via the Norito
  `ExposedPrivateKey` wrapper just like the Torii Rust client.
- If no identity flag is supplied the CLI reads `SIGSTORE_ID_TOKEN`
  automatically, matching the variable exported by GitHub Actions and GitLab CI.
- Passing `--identity-token-provider=github-actions` lets the CLI fetch the OIDC
  token directly via `ACTIONS_ID_TOKEN_REQUEST_URL`; **you must also provide**
  `--identity-token-audience=<audience>` so the CLI can request the precise
  scope required by your Fulcio policy.
- `proof verify` rebuilds the PoR store, reports the deterministic chunk digest,
  and surfaces the payload/CAR digests so CI can gate registry submission.
- `manifest verify-signature` accepts either a bundle or raw signature/public
  key pair, checks the embedded metadata (chunk digests, token hashes) against
  locally recomputed summaries, and ensures the Ed25519 signature matches the
  manifest digest before continuing the rollout.
- Signature bundles never persist the raw OIDC token unless
  `--include-token=true` is passed; instead a BLAKE3 hash and source metadata are
  recorded so CI systems can audit which identity issued each artefact.

## Keyless Signing Workflow

`manifest sign` can mint Sigstore-verifiable signatures from multiple token
sources. Provide a credential inline, via an explicit environment variable, or
from a file when running locally. Inside GitHub Actions, pass
`--identity-token-provider=github-actions` **and**
`--identity-token-audience=<value>` to let the CLI fetch the job-scoped token
via `ACTIONS_ID_TOKEN_REQUEST_URL`. Choose an explicit audience (for example
`sorafs` or `sigstore-ci`) that matches your Fulcio configuration; the CLI no
longer falls back to a default value.

Every signature bundle embeds the manifest digest, an ephemeral Ed25519 public
key, and identity metadata whose `token_source` now distinguishes inline
(`inline`), environment (`env:VAR`), file (`file:/path`), and provider-driven
(`oidc:github-actions(AUD)`) flows. Downstream tooling can hand the bundle to
`cosign verify-blob --bundle` or a Sigstore transparency log while preserving
the caller’s provenance.

To avoid leaking credentials:

- Bundles store only the BLAKE3 hash and source label for each token; keep
  `--include-token=false` unless an air-gapped verification or a regulated audit
  explicitly requires the raw JWT.
- Store bundles and summaries in restricted artefact buckets; by default only
  the BLAKE3 hash of the token is persisted for auditing.
- Rotate OIDC audiences regularly and audit CI runs using the
  `identity_token_source` field exposed in the CLI summary JSON.

See `docs/examples/sorafs_ci.md` for ready-to-use GitHub Actions and GitLab
pipelines that exercise these flows end-to-end.

## Verify signature bundles

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest verify-signature \
  --manifest artifacts/manifest.to \
  --bundle artifacts/manifest.bundle.json \
  --summary artifacts/car_summary.json \
  --expect-token-hash 77c3...

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest verify-signature \
  --manifest artifacts/manifest.to \
  --signature artifacts/manifest.sig \
  --public-key-hex 90af... \
  --summary artifacts/car_summary.json
```

- Supplying `--bundle` validates the embedded manifest digest, token hash, and
  chunk digest fields against locally recomputed values before checking the
  Ed25519 signature.
- Use `--expect-token-hash` to pin the hashed OIDC identity observed during
  signing; verification fails if the bundle records a different value.
- When only a signature is available, provide `--public-key-hex` explicitly; the
  CLI still recomputes chunk digests when `--summary` or `--chunk-plan` inputs
  are supplied.

## Compile Kotodama bytecode

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  norito build \
  --source contracts/register_domain.ko \
  --bytecode-out artifacts/register_domain.to \
  --abi-version=1 \
  --summary-out artifacts/register_domain.bytecode.json
```

The summary captures the ABI version, output path, byte length, and BLAKE3
digest so downstream tooling can pin compiler outputs in CI.

## Submit manifests to Torii

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/manifest.to \
  --chunk-plan artifacts/chunk_plan.json \
  --torii-url https://localhost:8080 \
  --submitted-epoch=42 \
  --authority=i105... \
  --private-key=ed25519:0123...cafe \
  --summary-out artifacts/manifest.submit.json \
  --response-out artifacts/manifest.submit.body
```

- Provide either `--chunk-plan` (preferred) or an explicit
  `--chunk-digest-sha3` to satisfy registry validation.
- Use `--private-key-file` when the credential is stored on disk. The CLI trims
  whitespace automatically.
- Alias bindings require `--alias-namespace`, `--alias-name`, and
  `--alias-proof` together. The command fails fast if any component is missing.
- Non-success HTTP responses bubble up as errors with the original body so CI
  can halt on policy violations.

## Verify CAR responses

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  proof verify \
  --manifest artifacts/manifest.to \
  --car artifacts/payload.car \
  --summary-out artifacts/manifest.verify.json
```

The verifier emits the canonical payload digest, chunk count, PoR chunk digest,
and CAR digests so CI pipelines can pin the proof bundle before hitting Torii.

## Stream PoR proofs

```bash
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  proof stream \
  --manifest artifacts/manifest.to \
  --torii-url https://torii.local \
  --provider-id-hex 00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff \
  --samples=32 \
  --sample-seed=7 \
  --por-root-hex ba5eba11ba5eba11ba5eba11ba5eba11ba5eba11ba5eba11ba5eba11ba5eba11 \
  --emit-events=false \
  --summary-out artifacts/proof_stream_summary.json \
  --governance-evidence-dir artifacts/proof_stream_evidence
```

`proof stream` replays each proof item as NDJSON (unless suppressed with
`--emit-events=false`) and emits an aggregate summary that tracks success,
failure, latency, and—when `--por-root-hex` is provided—local verification
results. Requests address manifests by `manifest_digest_hex` (BLAKE3-256 of the
canonical manifest) so proof streams remain deterministic across gateways. The
summary JSON mirrors the Torii Prometheus metrics that the gateway
exports (`torii_sorafs_proof_stream_events_total`,
`torii_sorafs_proof_stream_latency_ms`, and
`torii_sorafs_proof_stream_inflight`) and feeds the example Grafana dashboard in
`docs/examples/sorafs_proof_streaming_dashboard.json`. See
`docs/source/sorafs_proof_streaming.md` for a deeper dive and monitoring tips.
The CLI now exits with an error when the stream reports any failures or when
local PoR verification rejects a proof; pass `--max-failures` and/or
`--max-verification-failures` to allow a bounded number of faults during
rehearsals.

When `--governance-evidence-dir` is supplied the CLI writes the rendered summary
JSON, a copy of the manifest, and `metadata.json` (captured timestamp, CLI
version, Torii endpoint, and manifest digest) into the specified directory so
release packets and governance reviews can archive verifiable proof-stream
evidence without re-running the command.

## Multi-source fetch orchestrator CLI

The `sorafs_fetch` developer CLI exposes the same orchestration logic that the
SDKs consume. The new scoreboard integration adds a few noteworthy flags:

- `--telemetry-json=<path|->` feeds provider latency and failure snapshots
  into the scoreboard builder. The JSON may be an array or an object containing
  a `providers` array; each record accepts `provider_id`, `latency_p95_ms`,
  `failure_rate_ewma`, `token_health`, `staking_weight`, `penalty`, and
  `last_updated_unix` fields.
- Scoreboard derivation is now enabled by default. Provide
  `--scoreboard-out=<path>` to persist the computed weights as Norito JSON for
  dashboards or CI diffing, and remember to pass
  `--provider-advert=name=PATH` for every `--provider` entry (or
  `--allow-implicit-provider-metadata` when replaying fixtures that intentionally
  reuse the baked-in capability hints).
- `--deny-provider=name` and `--boost-provider=name:delta` surface the new
  `ScorePolicy` hook. Denied providers are skipped deterministically during
  scheduling, while positive or negative deltas adjust the weighted
  round-robin credits without mutating the advert metadata.
- `--telemetry-region=REGION` tags the emitted `sorafs_orchestrator_*`
  Prometheus metrics (and downstream OpenTelemetry exporters) so dashboards can
  slice active fetches, durations, and retry counters by region or environment.

Scoreboard mode requires adverts for every `--provider` entry so the CLI can
evaluate capability and validity windows; pass `--provider-advert=name=path`
for the full set of fixtures before enabling the flags above.

### Gateway fetch via `sorafs_cli fetch`

The primary CLI now exposes the orchestrator facade directly:

```bash
sorafs_cli fetch \
  --plan artifacts/payload_plan.json \
  --manifest-id 7bb2…9d31 \
  --provider name=alpha,provider-id=9f5c…73aa,base-url=https://gw-alpha.example.org/,stream-token="$(cat alpha.token)" \
  --output artifacts/payload.bin \
  --json-out artifacts/fetch_summary.json \
  --local-proxy-manifest-out artifacts/proxy_manifest.json \
  --local-proxy-mode bridge \
  --local-proxy-norito-spool storage/streaming/soranet_routes \
  --local-proxy-kaigi-spool storage/streaming/soranet_routes \
  --local-proxy-kaigi-policy authenticated \
  --max-peers=2 \
  --retry-budget=4
```

Input flags mirror the developer tool: every `--provider` entry supplies a
manifest-scoped stream token, the Torii base URL, and the canonical provider
identifier. The command derives a scoreboard from the token metadata, applies
the optional `--max-peers` cap, and threads the retry policy through the
orchestrator. A machine-readable summary is emitted to stdout (and, when
`--json-out` is provided, to disk) containing:

- {{#include sorafs/snippets/multi_source_flag_notes.txt}}

- `--guard-directory=PATH` loads a pinned guard set JSON (see below) and
  `--guard-cache=PATH` persists cache updates across runs. When you need
  tamper-evidence for the cache (e.g., storing it on shared network volumes),
  pass a 32-byte hex key via `--guard-cache-key=HEX`; the orchestrator signs
  cached guard lists with the key and refuses to load caches whose MAC fails.
- The `guard-directory` subcommand helps you keep snapshots current:

  ```bash
  sorafs_cli guard-directory fetch \
    --url https://directory.soranet.dev/mainnet_snapshot.norito \
    --output ./state/guard_directory.norito \
    --expected-directory-hash <directory-hash-hex>

  sorafs_cli guard-directory verify \
    --path ./state/guard_directory.norito \
    --expected-directory-hash <directory-hash-hex>
  ```

  `fetch` downloads and verifies the SRCv2 payload before writing it to disk,
  while `verify` replays the validation pipeline for artefacts sourced from
  other teams, emitting a JSON summary that mirrors the orchestrator output.
- `--scoreboard-out=PATH` persists the computed eligibility/weighting snapshot
  to Norito JSON for audits. Pair it with `--scoreboard-now=UNIX_SECS` when you
  need deterministic fixtures for CI or release evidence.
- `--telemetry-source-label=LABEL` records which OTLP stream produced the
  concurrency snapshot inside the scoreboard metadata so
  `cargo xtask sorafs-adoption-check --require-telemetry` can reject captures
  that do not prove their telemetry origin.
- Guard directory endpoints may now carry a `"tags"` array—mark SoraNet exits
  capable of proxying Norito streaming traffic with `"norito-stream"` so the
  orchestrator prioritises those URLs when preparing privacy routes.
- `--local-proxy-manifest-out=PATH` captures the QUIC proxy manifest (certificate, ALPN, guard cache key, cache-tagging salt, telemetry hints, and Kaigi room policy hint) emitted by the orchestrator. Feed the manifest to the browser extension or SDK adapters; the JSON summary mirrors the same payload under `local_proxy_manifest`.
- `--local-proxy-kaigi-spool=PATH`/`--local-proxy-kaigi-policy=public|authenticated` override the Kaigi spool directory and advertised room policy for a single run, matching the Norito overrides.

- `chunk_count`, `assembled_bytes`, and `payload` (base64) for quick integrity
  checks in CI pipelines.
- `provider_reports`, mirroring the multi-source fetch outcome with success /
  failure counts and the disabled flag for each provider.
- `chunk_receipts`, recording which provider ultimately served every chunk.
- `local_proxy_manifest`, populated when `local_proxy` is enabled in the orchestrator config. The object mirrors the browser handshake manifest (certificate PEM, ALPN label, guard cache key, cache-tagging salt, telemetry hints) and the same payload is written to `--local-proxy-manifest-out=PATH` for browser extensions.
- `manifest_digest_hex`, `manifest_payload_digest_hex`, `manifest_car_digest_hex`, `manifest_content_length`, `manifest_chunk_count`, `manifest_chunk_profile_handle`, and `manifest_governance` surface the manifest metadata downloaded from the gateway. These fields mirror the manifest response returned by `/v1/sorafs/storage/manifest/{id}`, confirm that the orchestrator rebuilt the CAR archive against the expected payload, and expose the council signatures bundled with the manifest (`manifest_governance.council_signatures`).
- `car_archive` now contains the assembled CAR diagnostics (`payload_digest_hex`, `archive_digest_hex`, `cid_hex`, `root_cids_hex`, `size`) alongside `verified=true` and `por_leaf_count`, proving that the CAR bytes emitted by the gateway match the manifest digests and PoR tree recorded on ingest.
- `ineligible_providers`, listing any aliases filtered out by capability or
  validity window checks, so SREs can surface advert drift before re-running the
  fetch.
- `telemetry_region`, echoing the region label supplied on the command line so
  CI and observability pipelines can correlate the summary with exported
  metrics.

`--output` streams verified chunks to disk while downloads are still in flight,
making it easy to compare the reconstructed payload against canonical fixtures.

## Moderation reproducibility validator

Gateways and SREs can vet AI moderation calibration artefacts before admitting
them by running:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-repro \
  --manifest docs/examples/ai_moderation_calibration_manifest_202602.json
```

The command parses either JSON or Norito payloads (switch with
`--format=json|norito`), verifies every signature recorded in
`ModerationReproManifestV1::signatures`, ensures the schema version matches
`MODERATION_REPRO_MANIFEST_VERSION_V1`, and rejects manifests whose model list
is empty or whose signatures fail verification. Successful runs emit a concise
summary showing the manifest UUID, model count, signer count, and the
`issued_at` timestamp so CI pipelines can pin the artefact before publishing it
to Torii or storing it alongside calibration evidence.

## Adversarial corpus validator

Gateway denylist automation also needs to vet the perceptual corpus bundles
referenced in MINFO-1c. Use the companion command to lint those registries:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-corpus \
  --manifest docs/examples/ai_moderation_perceptual_registry_202602.json
```

`validate-corpus` accepts the published JSON artefacts or their Norito-encoded
equivalents, enforces `ADVERSARIAL_CORPUS_VERSION_V1`, ensures every manifest
contains at least one family with at least one variant, and rejects entries that
omit perceptual hashes/embeddings or attempt to set a Hamming radius above 32.
When validation succeeds the CLI prints the issued-at timestamp, cohort label,
and family/variant counts so operators can record the evidence before updating
gateway denylists or publishing the manifest alongside GAR tickets.

## Honey-token audit

Use the new `moderation honey-audit` helper to probe gateways with known
denylisted digests and capture cache-version evidence:

```bash
iroha app sorafs moderation honey-audit \
  --manifest-id feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface \
  --honey 35c60c0f4cf6a1116fd17c2a930f37390f34030e7c5f23d77ecbb543c1a2d9ba \
  --expected-cache-version cache-v7 \
  --moderation-key-b64 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
  --provider name=alpha,provider-id=AAAA...,base-url=https://gateway.example/,stream-token=BASE64 \
  --json-out artifacts/sorafs_gateway/honey_audit.json \
  --markdown-out artifacts/sorafs_gateway/honey_audit.md
```

- The command fails if any provider returns success or omits/mismatches the
  cache/denylist version advertised by policy. `--require-proof` enforces the
  presence of verified moderation proofs when the gateway publishes them.
- Outputs include a machine-readable JSON summary plus an optional Markdown
  digest for governance packets.
- `fetch` now accepts `--expected-cache-version` and `--moderation-key-b64`;
  when provided, the orchestrator rejects responses that are missing the
  declared cache/denylist version and surfaces verified Proof-of-Denylist
  tokens alongside the policy evidence.

## Gateway denylist Merkle registry

MINFO-6 introduces a Merkle-anchored registry for denylist bundles so gateways
and auditors can prove inclusion of specific entries. The CLI exposes two
helpers under `sorafs gateway merkle`:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 5 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_5.json

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --descriptor provider:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_provider.json
```

- `merkle snapshot` parses the JSON bundle, validates every entry using the
  existing denylist policy rules, computes deterministic leaf hashes, and emits
  the Merkle root alongside the per-entry descriptors. The command prints a
  summary to stdout and writes the full JSON artefact to `--json-out` (defaults
  to `artifacts/sorafs_gateway/denylist_merkle_snapshot.json`). The artefact
  mirrors the CLI output with:
  - `root_hex` — the BLAKE3 Merkle root that governance anchors.
  - `leaf_count` — number of entries included in the snapshot.
  - `entries[]` — `{index, kind, descriptor, hash_hex, policy_tier}` for every
    entry so auditors can map registry indexes back to the source file.
- `account_id` entries are validated locally as encoded account literals
  (canonical I105 only). Alias, UAID, opaque, and
  `` literals are rejected by the validator.
- `merkle proof` recomputes the tree for the given denylist and produces a
  membership proof for the zero-based `--index` requested. The JSON artefact
  stores the root, the entry metadata, and the audit path (sibling hashes,
  direction, and duplicate markers) so hosts can replay the proof without
  re-running the CLI. The proof command prints a condensed view showing which
  entry was proven and the sibling sequence for quick inspection.
  - Prefer `--descriptor <kind:value>` when you copy the descriptor directly
    from the snapshot output (for example `provider:AAAA…`). The CLI resolves
    the descriptor to the correct index and emits the same proof artefact,
    avoiding manual counting.
- Pass `--norito-out=<path>` to either command to persist a Norito-encoded
  artefact (`.to`) alongside the JSON copy. The snapshot/proof payloads are
  encoded with `GATEWAY_MERKLE_SCHEMA_VERSION` so GAR automation and gateways
  can transmit proofs directly over Norito channels without scraping stdout.

Use `gateway update-denylist` to apply additions/removals and emit artefacts in
one run:


- Records are revalidated, descriptors are canonicalised (provider/manifest
  digests are upper-cased), duplicates are rejected unless
  `--allow-replacement` is supplied, and the command aborts if the resulting
  denylist would be empty.
- Removals accept descriptors straight from the snapshot (case-insensitive for
  provider/manifest digests). Use `--allow-missing-removals` to skip absent
  descriptors and `--force` to overwrite an existing `--out` path.
- The command prints the BLAKE3 digest and Merkle root for the updated bundle
  and optionally writes snapshot + Norito + evidence artefacts in one call so
  governance packets and GAR/CDN rollouts can attach a single deterministic
  bundle.

Both commands default to the sample denylist at
`docs/source/sorafs_gateway_denylist_sample.json`, so CI pipelines can dry-run
the tooling. Attach the snapshot and proof artefacts to the canon evidence
bundle whenever new entries are published so governance votes and GAR tickets
have reproducible Merkle roots.

## Appeal pricing quotes

MINFO-7’s congestion-aware deposit rules now ship in the CLI so moderation
services and treasury tooling can quote required stakes deterministically. Use
`sorafs_cli appeal quote` to query the baseline configuration captured in
`docs/source/sorafs_appeal_pricing_plan.md`:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal quote \
  --class=content \
  --backlog=28 \
  --evidence-mb=45 \
  --urgency=normal \
  --panel-size=7
```

The default output is a readable breakdown showing the base rate, backlog
factor, size multiplier, surge multiplier, and clamped deposit (XOR). Pass
`--format=json` to receive a machine-readable envelope with `deposit_xor`,
`valid_until_unix`, and all intermediate multipliers so CI pipelines can archive
quotes alongside moderation evidence. Provide `--config=PATH` (or `--config=-`
to read from stdin) to load a governance-managed manifest such as
`docs/examples/ministry/appeal_pricing_config_baseline.json`, keeping the CLI in
sync with ratified rate tables as soon as they land on the governance DAG.

## Appeal settlement breakdowns

Once an appeal resolves, treasury tooling needs a deterministic way to split
the escrowed deposit between refunds, slashed amounts, and panel stipends. The
`sorafs_cli appeal settle` command reuses the governance-managed settlement
manifest (baseline: `docs/examples/ministry/appeal_settlement_config_baseline.json`)
to emit a human-readable table or JSON payload describing those flows:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal settle \
  --deposit=420 \
  --outcome=overturn \
  --panel-size=9 \
  --format=json
```

The CLI reports the refunded amount, the treasury transfer, funds that remain
held in escrow (for escalated cases), and the total panel rewards (per- juror
stipends plus the per-case bonus). Provide `--config=PATH` or `--config=-` to
hydrate the CLI with the latest settlement manifest so dashboards and scripts
mirror governance-approved payout ratios.

### Appeal disbursement plans

Use `sorafs_cli appeal disburse` when you need the account-aware payout plan
for a resolved appeal. The command accepts the same settlement manifest and
verdict inputs but also requires the refund/treasury/escrow account IDs
alongside the juror roster:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  appeal disburse \
  --deposit=420 \
  --outcome=withdrawn_before_panel \
  --panel-size=7 \
  --refund-account=i105... \
  --treasury-account=i105... \
  --escrow-account=i105... \
  --juror=i105... --juror=i105... --juror=i105... \
  --juror=i105... --juror=i105... --juror=i105... --juror=i105... \
  --no-show=i105... --no-show=i105... \
  --format=json
```

The output emits the refund transfer, both treasury components (deposit share
plus any forfeited panel rewards), the amount left in escrow, and the
juror-level stipend/bonus allocations. No-show jurors are listed explicitly and
their forfeited rewards roll into the treasury component so dashboards and
treasury pipelines can reconcile every XOR based on the manifest-driven rules.

## PoR validator commands

### Inspect challenge status

```bash
sorafs_cli por status \
  --torii-url https://torii.local \
  --manifest 7bb2c8d6a01de9d6264d3525ec6c9f6c2ec6fb6ef1d9d88edb8a94ff4b8f9d31 \
  --status=failed \
  --limit=20
```

The command queries `GET /v1/sorafs/por/status` and prints either a terse table
(`--format=table`, the default) or the raw Norito JSON (`--format=json`). Status
filters accept the canonical labels (`pending`, `verified`, `failed`,
`repaired`, `forced`) and the CLI validates the manifest/provider digests before
dispatching the request so typos fail fast in CI.

### Trigger manual challenges

```bash
sorafs_cli por trigger \
  --torii-url https://torii.local \
  --manifest 7bb2…9d31 \
  --provider d09c…73aa \
  --reason=latency_probe \
  --samples=48 \
  --auth-token artifacts/challenge_token.to
```

The CLI reads a council-signed `ChallengeAuthTokenV1`, confirms the target
manifest/provider pair is permitted, and submits a Norito
`ManualPorChallengeV1` request to `POST /v1/sorafs/por/trigger`. Optional flags
(`--samples`, `--deadline-secs`) override the scheduler defaults on a per-call
basis. Responses are surfaced verbatim so auditors capture the assigned
`challenge_id` or any governance error codes.

### Export GovernanceLog verdicts

```bash
sorafs_cli por export \
  --torii-url https://torii.local \
  --start-epoch=1714000 \
  --end-epoch=1714800 \
  --out artifacts/por_export.parquet
```

`por export` streams the Parquet artefact produced by
`GET /v1/sorafs/por/export` to disk and prints the number of bytes written,
making it easy to wire into nightly governance or observability jobs. Start/end
epochs are optional; omit them to fetch the most recent window.

### Render weekly PoR health reports

```bash
sorafs_cli por report \
  --torii-url https://torii.local \
  --week 2025-W12 \
  --format=markdown
```

Weekly reports are fetched from `GET /v1/sorafs/por/report/{iso_week}`. Markdown
output mirrors the governance briefing (aggregate metrics, provider summaries,
slashing events, and VRF anomalies), while `--format=json` emits the canonical
`PorWeeklyReportV1` payload for dashboards and downstream automation.

## Roadmap

Remaining CLI work focuses on governance integration and distribution polish:

- Norito manifest scaffolding, council signature injection, and bundle
  verification integration with governance tooling.
- Gateway token tooling and PoTR streaming harnesses that reuse the new proof
  reports.
- Packaging and release automation so the CLI ships via Homebrew, npm, and
  crates.io alongside reproducible artefacts.
