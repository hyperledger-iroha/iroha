---
title: SoraFS CLI
summary: Developer-facing entry point for packaging payloads and emitting chunk plans.
---

# SoraFS CLI

The `sorafs_cli` binary (built from the `sorafs_orchestrator` crate) now bundles the full SoraFS packaging workflow instead of scattering
helpers across ad-hoc utilities. Core subcommands include:

_Looking for an end-to-end walkthrough?_ The quickstart at
`docs/source/sorafs/developer/overview.md` chains the key commands together for
local testing and CI.

- `norito build` — compile Kotodama `.ko` sources into deterministic IVM
  bytecode artefacts.
- `car pack` — produce a CAR archive, chunk-fetch plan, and JSON summary for CI.
- `manifest build` — translate the CAR summary into a Norito manifest.
- `manifest submit` — POST the canonical `ManifestV1` payload (and an optional
  alias proof) to Torii's dedicated `/v1/sorafs/pin/register` route and wait for
  the registration response.
- `proof verify` — validate CAR responses against a manifest and emit the
  PoR-ready digests required for registry admission.

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
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  car pack \
  --input fixtures/payload.bin \
  --car-out artifacts/payload.car \
  --plan-out artifacts/chunk_plan.json \
  --summary-out artifacts/car_summary.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest build \
  --summary artifacts/car_summary.json \
  --manifest-out artifacts/manifest.to \
  --manifest-json-out artifacts/manifest.json \
  --pin-min-replicas=3 \
  --pin-storage-class=warm \
  --pin-retention-epoch=42

```

Looking for a turnkey script? `docs/examples/sorafs_cli_quickstart.sh` wraps the
commands above into a single workflow that expects standard environment
variables (`SORA_PAYLOAD`, `TORII_URL`, etc.) and writes
every summary JSON to an output directory. It is ideal for CI runners or for
developers who want to capture canonical artefacts without typing each step.
For release authentication, see `scripts/release_sorafs_cli.sh`. It requires an
external Ed25519 signer, a governed raw public key and reviewed fingerprint, and
an explicitly pinned `sorafs-validate` path and SHA256. It has no fixture,
credential, or verifier defaults.
If you need deterministic content fixtures to diff against, use the
content-only set under `fixtures/sorafs_manifest/ci_sample` and the accompanying
README in `docs/examples/sorafs_ci_sample/`. Those fixtures are not
release-authenticity evidence.

Key behaviours:

- `--chunker-handle` defaults to `sorafs.sf1@1.0.0`. Pass an explicit handle to
  switch chunking profiles once the registry grows additional entries.
- The manifest builder reads the JSON emitted by `car pack`, applies optional
  pin-policy overrides (`--pin-*` flags), and writes both Norito binary output
  and (optionally) a JSON view for debugging.
- The CLI writes a pretty-printed summary to STDOUT and, when `--summary-out`
  is supplied, mirrors the same JSON to disk. Downstream automation can diff the
  summary or extract digests/TLV metadata without parsing the CAR file directly.
- Plans use the strict `sorafs.chunk_fetch_plan.v1` envelope, which binds the
  ordered chunk specifications to the non-zero BLAKE3 digest of the complete
  payload. The multi-source orchestrator rejects bare arrays, missing bindings,
  and substituted payloads.
- `manifest submit` recomputes the chunk SHA3 digest when a plan is provided,
  validates alias inputs, and forwards Ed25519 keys via the Norito
  `ExposedPrivateKey` wrapper just like the Torii Rust client.
- `proof verify` rebuilds the PoR store, reports the deterministic chunk digest,
  and surfaces the payload/CAR digests so CI can gate registry submission.
  Supply `--chunk-plan` for directory or multi-file payloads so verification
  reuses the exact packed chunk boundaries instead of reconstructing a
  single-file plan from the manifest alone.
## Release authenticity

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/release/release_manifest.json \
  --external-signer /run/sorafs-release/ed25519-sign \
  --signing-public-key /run/sorafs-release/release.ed25519.pub \
  --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
  --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
  --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
```

The wrapper signs the canonical aggregate release manifest through the external
signer and verifies immutable snapshots with `sorafs-validate release-manifest`.
The signature must be exactly 64 raw bytes; the public key must be exactly 32
raw bytes and match the reviewed SHA256 fingerprint. A pinned verifier digest
is mandatory. OIDC/cosign attestations remain useful provenance, but they do not
replace this release-authenticity check.

## Compile Kotodama bytecode

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  norito build \
  --source contracts/register_domain.ko \
  --bytecode-out artifacts/register_domain.to \
  --summary-out artifacts/register_domain.bytecode.json
```

The summary captures the ABI version, output path, byte length, and BLAKE3
digest so downstream tooling can pin compiler outputs in CI.

## Submit manifests to Torii

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/manifest.to \
  --chunk-plan artifacts/chunk_plan.json \
  --torii-url https://localhost:8080 \
  --resolve-submitted-epoch=true \
  --authority=<i105-account-id> \
  --private-key=ed25519:0123...cafe \
  --summary-out artifacts/manifest.submit.json \
  --response-out artifacts/manifest.submit.body
```

- The manifest carries its canonical chunk-plan SHA3-256 commitment. Supplying
  `--chunk-plan` or `--chunk-digest-sha3` is optional verification evidence; if
  present, it must match the embedded commitment exactly.
- Use `--submitted-epoch=<N>` to pin an explicit epoch, or
  `--resolve-submitted-epoch=true` to query `<torii-url>/status` and resolve it
  automatically.
- Use `--private-key-file` when the credential is stored on disk. The CLI trims
  whitespace automatically.
- Alias bindings require `--alias-namespace`, `--alias-name`, and
  `--alias-proof` together. The command fails fast if any component is missing.
- The pin-registration request includes the required `manifest_payload`, a
  canonical base64 copy of the exact Norito `ManifestV1`. Torii derives the
  digest, chunk-plan commitment, chunker, content length, and pin policy only
  from those bytes before queueing the transaction; retired parallel summary
  fields are rejected.
- Non-success HTTP responses bubble up as errors with the original body so CI
  can halt on policy violations.

## Verify CAR responses

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  proof verify \
  --manifest artifacts/manifest.to \
  --car artifacts/payload.car \
  --chunk-plan artifacts/chunk_plan.json \
  --summary-out artifacts/manifest.verify.json
```

The verifier emits the canonical payload digest, chunk count, PoR chunk digest,
and CAR digests so CI pipelines can pin the proof bundle before hitting Torii.

## Stream PoR proofs

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  proof stream \
  --manifest=artifacts/manifest.to \
  --torii-url=https://torii.example \
  --provider-id-hex=00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff \
  --bearer-token-env=SORAFS_PROOF_BEARER_TOKEN \
  --samples=32 \
  --sample-seed=7 \
  --emit-events=false \
  --summary-out=artifacts/proof_stream_summary.json \
  --governance-evidence-dir=artifacts/proof_stream_evidence
```

Set `SORAFS_PROOF_BEARER_TOKEN` in the process environment or a runtime secret
injector; never place the token itself in argv. `proof stream` accepts only a
bare HTTPS Torii origin or the exact HTTPS
`/v1/sorafs/proof/stream` gateway URL. HTTP, URL userinfo, query strings,
fragments, redirects, and the retired `--stream-token` form fail closed.

Before POSTing, the CLI uses the same origin and bearer credential to read the
exact native `GET /v1/sorafs/pin/{digest_hex}` projection. The record must be
`Approved`; its digest, root CID, chunker, chunk-plan digest, PoR root, content
length, and policy must match the exact canonical local manifest. Its finalized
height and block hash must both be non-zero. The CLI binds that cursor into
`ProofStreamRequestV1`, derives PoR sampling from the complete request and the
ledger-authoritative root, verifies every row's request digest and cursor, and
consumes the exact ordered sequence through EOF before publishing output.

PoR `--samples` defaults to `32` and must stay in `1..=500`. Any failure,
duplicate, reordered, truncated, extra, forged, or mismatched row rejects the
whole command; V1 has no failure-budget override. When `--emit-events=true`,
events are payload-free projections containing only digests, identifiers,
indices, finalized anchors, result, and timing. Witness bytes, Merkle proof
payloads, signed receipts, nonces, and credentials are never emitted. The final
summary includes the request digest, finalized cursor, and a digest of the
nonce, and its endpoint field contains only the HTTPS origin and path.

When `--governance-evidence-dir` is supplied the CLI writes the rendered summary
JSON, a copy of the manifest, and `metadata.json` (captured timestamp, CLI
version, redacted Torii origin/path, and manifest digest) into the specified directory so
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
  --provider name=alpha,provider-id=9f5c…73aa,gateway-key="$(cat alpha.gateway-key.hex)",base-url=https://gw-alpha.example.org/,stream-token="$(cat alpha.token)" \
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
manifest-scoped stream token, the trusted 32-byte Ed25519 gateway public key as
64 hex characters, the Torii base URL, and the canonical provider identifier.
The key is mandatory and verifies the token before any request is sent; do not
copy it from the same unauthenticated token response. The command derives a
scoreboard from the token metadata, applies
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
- `--local-proxy-manifest-out=PATH` captures the QUIC proxy manifest
  (certificate, ALPN, guard cache key, cache-tagging salt, telemetry hints, and
  Kaigi room policy hint) emitted by the orchestrator. It requires a
  `local_proxy` config with `emit_browser_manifest = true` and a CLI binary
  built with the `local-quic-proxy` feature. Feed the manifest to the browser
  extension or SDK adapters; the JSON summary mirrors the same payload under
  `local_proxy_manifest`.
- `--local-proxy-kaigi-spool=PATH`/`--local-proxy-kaigi-policy=public|authenticated` override the Kaigi spool directory and advertised room policy for a single run, matching the Norito overrides.

- `chunk_count`, `assembled_bytes`, and `payload` (base64) for quick integrity
  checks in CI pipelines.
- `provider_reports`, mirroring the multi-source fetch outcome with success /
  failure counts and the disabled flag for each provider.
- `chunk_receipts`, recording which provider ultimately served every chunk.
- `local_proxy_manifest`, populated when `local_proxy` is enabled in the
  orchestrator config, `emit_browser_manifest` is true, and local QUIC proxy
  runtime support is compiled in. The object mirrors the browser handshake
  manifest (certificate PEM, ALPN label, guard cache key, cache-tagging salt,
  telemetry hints) and the same payload is written to
  `--local-proxy-manifest-out=PATH` for browser extensions.
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

Governed compliance catalog producers also need to vet the perceptual corpus
bundles referenced in MINFO-1c. Use the companion command to lint those
registries:

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
and family/variant counts so operators can record the evidence before an
external governed producer constructs and signs the next catalog.

## Honey-token audit

Use the `moderation honey-audit` helper to probe gateways with digests known to
be blocked by the promoted compliance catalog and capture catalog-sequence
evidence:

```bash
iroha app sorafs moderation honey-audit \
  --manifest-id feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface \
  --honey 35c60c0f4cf6a1116fd17c2a930f37390f34030e7c5f23d77ecbb543c1a2d9ba \
  --expected-cache-version cache-v7 \
  --moderation-key-b64 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
  --provider name=alpha,provider-id=AAAA...,gateway-key=ED25519_PUBLIC_KEY_HEX,base-url=https://gateway.example/,stream-token=BASE64 \
  --json-out artifacts/sorafs_gateway/honey_audit.json \
  --markdown-out artifacts/sorafs_gateway/honey_audit.md
```

- The command fails if any provider returns success or omits/mismatches the
  catalog version advertised by policy. `--require-proof` enforces the
  presence of verified moderation proofs when the gateway publishes them.
- Outputs include a machine-readable JSON summary plus an optional Markdown
  digest for governance packets.
- `fetch` accepts `--expected-cache-version` and `--moderation-key-b64`; when
  provided, the orchestrator rejects responses that are missing the declared
  catalog version and surfaces verified moderation proof tokens alongside the
  policy evidence.

## Gateway compliance control

V1 gateway compliance control is exposed only through authenticated Torii
routes: feed and status reads plus account-signed `stage`, `acknowledge`,
`promote`, and `rollback` mutations under
`/v1/sorafs/gateway/compliance`. The controller accepts bounded,
predecessor-bound, threshold-signed catalogs and keeps durable candidate,
acknowledgement, promoted, last-known-good, and history state.

The SoraFS CLI does not construct, diff, verify, update, or install local
compliance packs. Catalog normalization and threshold signing happen in
governed external producers; operators use the authenticated API and retain
payload-free promotion evidence.

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
  --refund-account=<i105-account-id> \
  --treasury-account=<i105-account-id> \
  --escrow-account=<i105-account-id> \
  --juror=<i105-account-id> --juror=<i105-account-id> --juror=<i105-account-id> \
  --juror=<i105-account-id> --juror=<i105-account-id> --juror=<i105-account-id> --juror=<i105-account-id> \
  --no-show=<i105-account-id> --no-show=<i105-account-id> \
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

### Challenge authority

Torii exposes no manual or externally supplied challenge-ingress route. The
coordinator scheduler is the sole challenge authority, and PoR automation fails
closed until authenticated external drand/VRF feeds are configured. Operators
inspect scheduler output with the status, report, and export commands; they do
not submit challenges through the CLI. The CLI also exposes no command for
recording manual success/failure observations; provider proofs and auditor
verdicts use the authenticated lifecycle instead.

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

The local CLI command set now covers manifest scaffolding, governance proposal
export, gateway fetch authorization, PoR trigger/export/report flows, and PoTR
proof streaming. Release signing and verification are deliberately outside the
CLI and use the governed aggregate-manifest path described above.
Remaining CLI work is release distribution and live-network governance evidence
collection:

- Publish signed, reproducible release artefacts for the CLI and document the
  install path for Homebrew, npm, and crates.io consumers.
- Capture live governance proposal and council-signature runbooks once the
  production deployment publishes its operator signing process.
