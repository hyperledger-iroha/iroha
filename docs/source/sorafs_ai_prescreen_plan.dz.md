---
lang: dz
direction: ltr
source: docs/source/sorafs_ai_prescreen_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 950eabe9c931403991143e8966961f6b710fcf802193bf52f34c6b37780f33d3
source_last_modified: "2026-06-25T04:56:56.769083+00:00"
translation_last_reviewed: 2026-06-25
---

# SoraFS AI Pre-screening & Quarantine

## Status

SFM-4a defines deterministic AI pre-screening before public SoraFS gateway
publication. The repository currently ships the reproducibility, corpus, local
model-registry admission/checkpointing, local deterministic screening-result
and moderation-ballot evidence checkpointing, quarantine review/release
evidence checkpointing, local encrypted quarantine object-store/API/CLI
and operator-panel read-model foundation with a
`sorafs_moderation_operator` role gate,
reviewed-quarantine appeal finance handoff and appeal-ballot API/CLI tooling,
payload-free bridge planning from the operator-panel read model,
local operator workflow service readback and signed mutation forwarding,
local browser operator UI,
local payload-free juror notification planning,
local payload-free juror notification delivery manifests,
local juror notification outbox/webhook delivery CLI automation,
local juror notification transport canary evidence tooling,
local payload-free commit/reveal coordination status,
local commit/reveal execution CLI automation,
local supervised commit/reveal executor job bundle generation,
local commit/reveal executor canary evidence tooling,
local payload-free operator workflow canary evidence tooling,
standalone persistent model-registry service foundation, deterministic local
runner CLI output, HTTP service mode, unary gRPC service mode, supervised
bundle generation, HTTP canary evidence, deterministic local committee
aggregation CLI output, locked-manifest committee HTTP service foundation,
supervised committee bundle generation, committee canary evidence,
gateway-policy, honey-audit, and observability foundations.
It does not yet ship captured deployed juror notification transport service
rollout evidence, captured deployed commit/reveal executor job rollout
evidence, or end-to-end release workflow as runnable services.

Implemented locally:

- `crates/iroha_data_model/src/sorafs/moderation.rs` defines
  `ModerationReproManifestV1`, `ModerationReproBodyV1`,
  `ModerationModelFingerprintV1`, `ModerationThresholdsV1`,
  `ModerationReproSignatureV1`, and `AdversarialCorpusManifestV1` with Norito
  encode/decode support and validators for schema version, model coverage,
  duplicate signers, and governance signatures.
- `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  validates governance-signed reproducibility manifests.
- `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  validates adversarial corpus manifests before gateway adoption.
- `sorafs_cli moderation registry-serve --state=PATH` runs a standalone
  persistent HTTP model-registry service. `GET /healthz` and
  `GET /v1/sorafs/moderation/model-registry/status` report the state checkpoint,
  digest, counts, and disabled outbound-network posture;
  `GET /v1/sorafs/moderation/model-registry` returns bounded snapshot readback;
  `POST /v1/sorafs/moderation/model-registry/repro-manifests` and
  `POST /v1/sorafs/moderation/model-registry/corpora` admit base64 canonical
  Norito manifests, validate them with the data-model validators, reject
  conflicting manifest ids, and atomically persist the Norito checkpoint after
  each accepted mutation.
- `sorafs_cli moderation run-local --manifest=PATH --payload=PATH --subject=ID
  --screened-at=UNIX_SECS` validates a governance-signed reproducibility
  manifest, derives deterministic local model scores from the manifest
  seed/material and payload digest, and emits JSON compatible with
  `POST /v1/sorafs/moderation/screening-results`.
- `sorafs_cli moderation runner-serve --manifest=PATH
  [--format=json|norito] [--listen=HOST:PORT] [--max-body-bytes=N]` locks the
  same governance-signed reproducibility manifest into a bounded local HTTP
  runner service. `GET /healthz` and
  `GET /v1/sorafs/moderation/runner/status` report the active manifest and
  disabled outbound-network posture; `POST /v1/sorafs/moderation/runner/screen`
  accepts `subject`, base64 payload bytes, and explicit `screened_at_unix`,
  then returns the same deterministic Torii-compatible screening JSON as
  `run-local`.
- `sorafs_cli moderation runner-bundle --manifest=PATH
  [--format=json|norito] --bundle-out=DIR` validates the same locked
  reproducibility manifest and emits a supervised local HTTP runner deployment
  bundle: manifest copy, `runner.env`, executable `run.sh`, systemd unit,
  launchd plist, README, and
  `sorafs.moderation.runner.bundle.v1` metadata JSON.
- `sorafs_cli moderation runner-canary --manifest=PATH
  [--format=json|norito] --runner-url=URL --payload=PATH --subject=ID
  --screened-at=UNIX_SECS` probes a deployed locked-manifest HTTP runner,
  verifies status and screening responses against the manifest id, runner hash,
  payload digest, score range, and threshold-derived verdict, and emits
  payload-free `sorafs.moderation.runner.rollout_evidence.v1` JSON for rollout
  archives.
- `sorafs_cli moderation committee-run --manifest=PATH [--format=json|norito]
  --quorum=N --result=PATH [--result=PATH...]` validates the same
  governance-signed reproducibility manifest, rejects payload-bearing result
  JSON, verifies all local runner outputs share the locked manifest id, runner
  hash, subject, digest, score range, and threshold-derived verdict, and emits
  payload-free `sorafs.moderation.committee.aggregate.v1` JSON using a
  deterministic median score under the requested quorum.
- `sorafs_cli moderation committee-serve --manifest=PATH
  [--format=json|norito] --quorum=N` locks the same manifest and quorum into a
  bounded local HTTP committee service. `GET /healthz` and
  `GET /v1/sorafs/moderation/committee/status` report the active manifest,
  quorum, aggregation rule, and disabled outbound-network posture; `POST
  /v1/sorafs/moderation/committee/aggregate` accepts payload-free runner result
  JSON arrays and returns the same deterministic committee aggregate JSON as
  `committee-run`.
- `sorafs_cli moderation committee-bundle --manifest=PATH
  [--format=json|norito] --quorum=N --bundle-out=DIR` validates the locked
  reproducibility manifest and emits a supervised local HTTP committee
  deployment bundle: manifest copy, `committee.env`, executable `run.sh`,
  systemd unit, launchd plist, README, and
  `sorafs.moderation.committee.bundle.v1` metadata JSON.
- `sorafs_cli moderation committee-canary --manifest=PATH
  [--format=json|norito] --committee-url=URL --quorum=N --result=PATH
  [--result=PATH...]` probes a deployed locked-manifest HTTP committee service,
  verifies status and payload-free aggregate responses against the manifest id,
  runner hash, quorum, deterministic median aggregate, score range, and
  threshold-derived verdict, and emits payload-free
  `sorafs.moderation.committee.rollout_evidence.v1` JSON for rollout archives.
- `sorafs_node::NodeHandle` admits validated moderation reproducibility
  manifests and adversarial corpus manifests into a local model registry,
  rejects conflicting manifest ids, keys corpus manifests by canonical Norito
  BLAKE3 digest, returns deterministic registry snapshots for operator
  plumbing, and checkpoints the registry under the SoraFS data directory when
  storage is enabled.
- Torii exposes the local registry through
  `GET /v1/sorafs/moderation/model-registry?limit=N`,
  `POST /v1/sorafs/moderation/model-registry/repro-manifests`, and
  `POST /v1/sorafs/moderation/model-registry/corpora`; admission requests carry
  base64 canonical Norito manifests and require X-Iroha canonical app
  authentication.
- `iroha sorafs moderation registry list --limit N`,
  `iroha sorafs moderation registry submit-repro --manifest PATH
  [--format=json|norito]`, and `iroha sorafs moderation registry
  submit-corpus --manifest PATH [--format=json|norito]` wrap the local model
  registry readback and signed admission endpoints. Submit commands validate
  JSON or Norito inputs and send canonical Norito manifest bytes to Torii.
- `sorafs_node::NodeHandle` records deterministic local screening-result
  evidence with BLAKE3 record digests, creates pending local quarantine records
  for `quarantine` and `escalate` verdicts, advances those records through
  reviewed and released states with operator metadata, exports/restores
  validated duplicate-checked snapshots, and checkpoints them under
  `moderation-screening/screening-snapshot.to` when SoraFS storage is enabled.
- Torii exposes the local screening/quarantine evidence surface through
  canonical-authenticated `POST /v1/sorafs/moderation/screening-results`,
  bounded `GET /v1/sorafs/moderation/screening-results?limit=N`, and bounded
  `GET /v1/sorafs/moderation/quarantine?limit=N`. Canonical-authenticated
  accounts assigned the `sorafs_moderation_operator` role can call
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/review` and
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/release` to
  advance local queue state. The same role gate protects
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/object`, which
  seals base64 payload bytes into the local encrypted object store after
  digest verification, and `GET` on the same path, which verifies the envelope
  and returns `payload_b64` for authorized operators. The same role gate also
  protects
  `GET /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/operator-panel?limit=N`,
  which bundles the quarantine record, encrypted-object metadata status,
  matching local appeal ballots, operator routes, and next-action hints without
  returning payload bytes.
- `iroha sorafs moderation screening submit --input screening-result.json`
  submits deterministic local runner output through the signed Torii screening
  admission endpoint, while `iroha sorafs moderation screening list --limit N`
  prints bounded local screening-record readback JSON.
- `sorafs_node::NodeHandle` maintains the local moderation ballot lifecycle for
  appeal cases, accepts governance-event-producing commit/reveal/tally payloads
  bound to `SoraFsModerationBallotContextV1`, and exposes bounded ballot and
  ballot-event readback.
- `iroha::client` and `iroha sorafs moderation ballots
  list|get|events|commit|reveal|tally` wrap the local ballot readback and
  signed committee lifecycle endpoints. Commit and reveal commands validate
  JSON or Norito payloads against the data-model validators, then submit the
  canonical Norito bytes to Torii.
- `iroha::client` and `iroha sorafs transparency
  cycles|explorer|tokens|source-entry` wrap the local transparency readback
  and signed source-entry ingest endpoints. Operator tooling can list
  published cycles, fetch a cycle or entry proof, read the local explorer
  snapshot and proof-token issuance index, and submit typed source-entry JSON
  for later transparency publication.
- `iroha::client` and `iroha sorafs appeals pricing
  config|status|quote` plus `iroha sorafs appeals finance` wrap the local
  appeal pricing, asset-lock deposit, settlement, reconciliation, and finance
  report readback endpoints. Pricing quotes validate JSON locally; finance
  mutations use canonical Iroha request signing; deposit readback normalizes
  32-byte escrow ids; report, weekly-rollup, and settlement-receipt readbacks
  support bounded `limit` queries.
- Torii exposes canonical-authenticated
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-handoff`
  to turn a reviewed local quarantine record into a baseline appeal pricing
  quote, quote-bound appeal finance deposit request, deterministic
  quarantine-evidence hash, and native `OpenAssetLock` instruction for payer
  signing. Pending or released quarantine records fail closed.
- `iroha sorafs moderation quarantine appeal-handoff --quarantine-id HEX
  --input appeal-handoff.json` wraps that local handoff endpoint. The command
  validates the 16-byte quarantine id, rejects empty payload files,
  canonicalizes JSON before signing, and prints the handoff response.
- Torii also exposes canonical-authenticated
  `POST /v1/sorafs/moderation/quarantine/{quarantine_id_hex}/appeal-ballot`
  to verify a confirmed appeal-finance asset-lock deposit, require the
  deterministic reviewed-quarantine handoff evidence hash, and announce the
  existing local moderation ballot. Pending/released quarantine records and
  deposits missing the handoff evidence hash fail closed.
- `iroha sorafs moderation quarantine appeal-ballot --quarantine-id HEX
  --input appeal-ballot.json` wraps that local bridge endpoint. The command
  validates the 16-byte quarantine id, rejects empty payload files,
  canonicalizes JSON before signing, and prints the announced ballot response.
- `iroha sorafs moderation quarantine list|review|release` wraps the local
  quarantine readback and transition endpoints. Review and release commands use
  canonical Iroha request signing, validate 16-byte quarantine ids, default the
  operator identity to the configured CLI account, and submit explicit
  transition timestamps. `iroha sorafs moderation quarantine object store|read`
  wraps the local encrypted payload object endpoints, reads store payload bytes
  from `--payload-file`, rejects empty payload files, signs store/read requests,
  and prints object metadata or `payload_b64` readback JSON.
  `iroha sorafs moderation quarantine operator-panel --quarantine-id HEX
  [--limit N]` wraps the local operator-panel read model and prints a
  payload-free workflow view for operator tooling.
  `iroha sorafs moderation quarantine bridge-plan --quarantine-id HEX
  [--limit N]` reads the same operator-panel view and emits a
  `sorafs.moderation.quarantine.bridge_plan.v1` JSON plan with ordered
  handoff, ballot, tally, and transparency CLI actions while rejecting any
  response that unexpectedly contains payload bytes.
  `iroha sorafs moderation quarantine operator-serve
  [--listen HOST:PORT] [--limit N] [--max-body-bytes N]` runs a local
  payload-free HTTP operator workflow service with health/status,
  operator-panel, and bridge-plan routes backed by the signed Torii
  operator-panel read model. The service rejects request bodies, rejects
  malformed quarantine ids, forwards no payload bytes, and fails closed if
  upstream operator-panel JSON unexpectedly includes `payload_b64`. The same
  service also exposes a payload-free juror notification plan route that derives
  per-juror commit/reveal status, signing accounts, Torii routes, and CLI
  command templates from the operator-panel ballot view without embedding
  private commit or reveal payloads. A companion payload-free juror notification
  delivery manifest route emits deterministic operator-managed delivery records
  with dedup keys, subjects, message bodies, signed Torii routes, and CLI
  command templates for external mail/webhook/scheduler dispatch.
  `iroha sorafs moderation quarantine notifications deliver --manifest PATH
  [--out-dir DIR] [--webhook-url URL]` reads that payload-free manifest,
  validates every notification keeps private payload flags disabled, writes
  canonical outbox JSON files and/or POSTs each notification to a webhook, and
  emits payload-free delivery evidence with notification and response body
  hashes instead of archiving message bodies.
  `iroha sorafs moderation quarantine notifications canary --manifest PATH
  --webhook-url URL [--out PATH]` probes a deployed notification transport
  webhook with the same payload-free manifest and emits
  `sorafs.moderation.juror_notifications.transport_canary.v1` evidence with
  per-probe status, notification hashes, and response hashes, including failed
  probes, without archiving message or response bodies. A
  payload-free commit/reveal coordination status route reports quorum state,
  missing commit/reveal jurors, next actions, and tally-ready request templates
  without embedding private juror payloads. It also
  exposes POST forwarding routes for
  review, release,
  appeal-handoff, appeal-ballot, and ballot-tally requests; those routes require
  JSON bodies, reject `payload_b64`, canonicalize appeal payloads before
  forwarding, and use the configured CLI account as the default review/release
  actor when omitted. Ballot-tally forwarding accepts explicit
  `case_id`/`round_id` fields or derives the first ballot reference from the
  payload-free operator-panel view.
  `iroha sorafs moderation quarantine operator-canary --operator-url URL
  --quarantine-id HEX [--limit N] [--out PATH]` probes deployed operator
  workflow health/status, browser UI, operator-panel, bridge-plan, juror-plan,
  juror-notifications, and commit-reveal-status routes, verifies the expected
  schemas and UI marker, rejects any response that includes payload bytes, and
  emits payload-free `sorafs.moderation.quarantine.operator_canary.v1`
  evidence without copying response bodies into the archive.
- `sorafs_node::NodeHandle` can seal quarantined payload bytes into a local
  encrypted Norito envelope under the SoraFS data directory, persist a separate
  object index checkpoint, reload it on restart, verify plaintext digests
  against quarantine records, and fail closed on tampered envelopes.
- `sorafs_cli moderation honey-audit` probes configured gateways with
  denylisted digests and emits JSON/Markdown evidence for policy enforcement.
- `docs/examples/ai_moderation_calibration_manifest_202602.json`,
  `docs/examples/ai_moderation_calibration_scorecard_202602.json`, and
  `docs/examples/ai_moderation_perceptual_registry_202602.json` provide the
  committed calibration and registry fixtures.
- `docs/source/sorafs/reports/ai_moderation_calibration_202602.md` records the
  calibration report, with localized mirrors.
- Gateway Authorization Records support moderation directives and slugs through
  `GarModerationDirectiveV1`, `GarModerationAction`, GAR v2 policy parsing, and
  Torii gateway policy checks.
- Torii CID lookup can report local moderation hits, while gateway fetch policy
  still blocks serving when required moderation directives are absent. The CID
  lookup metadata response also bounds embedded site-file listings with
  `limit` (default 50, max 500) while preserving full file counts and
  truncation metadata for operator tooling.
- Torii exposes the configured gateway denylist catalog and pack metadata
  through `/v1/sorafs/denylist/catalog?limit=N` and
  `/v1/sorafs/denylist/packs/{pack_id}`; catalog readback preserves full pack
  counts while bounding returned pack/config-list arrays for operator tooling.
- `dashboards/grafana/ministry_moderation_overview.json` and
  `dashboards/alerts/ministry_moderation_rules.yml` provide the moderation
  ingest, latency, drift, and manifest-health monitoring story.

Not shipped locally:

- Captured deployed juror notification transport service rollout evidence and
  deployed commit/reveal executor job rollout evidence.
- End-to-end ingest -> quarantine -> appeal -> transparency workflow services.

## Target Architecture

The production service remains a staged rollout target:

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Model registry | Stores model artifacts, reproducibility manifests, calibration datasets, and hashes. | Local Torii admission/readback, client/CLI admission tooling, node snapshot/checkpoint foundation, and standalone persistent `registry-serve` HTTP service foundation exist. |
| AI runner | Executes approved models deterministically and emits model scores. | Deterministic local `run-local` CLI, bounded `runner-serve` HTTP mode, production unary `runner-grpc-serve` gRPC service, supervised HTTP runner bundle, and `runner-canary` rollout evidence tooling emit/operate Torii-compatible screening results. |
| Committee orchestrator | Aggregates model outputs and yields `pass`, `quarantine`, or `escalate`. | Threshold schema, calibration report, local screening-result admission, local `committee-run` quorum aggregation, locked-manifest `committee-serve` HTTP aggregation, supervised committee bundle generation, `committee-canary` rollout evidence tooling, local moderation-ballot lifecycle/readback, and client/CLI ballot tooling exist. |
| Quarantine store | Stores flagged content and metadata under moderation access controls. | Local quarantine evidence records, encrypted local payload envelopes, role-gated object store/read, operator-panel read model, review/release API transitions, local CLI queue/review/release/operator-panel commands, and local HTTP operator workflow service with payload-free readback, juror notification planning and delivery manifests, commit/reveal coordination status, a browser operator UI, operator workflow canary evidence tooling, and signed review/release forwarding exist. |
| Moderation bridge | Hands escalations to appeal and transparency workflows. | Reviewed-quarantine appeal handoff and confirmed-deposit appeal-ballot API/CLI, operator-service POST forwarding for appeal handoff/ballot/tally, local juror notification planning, delivery manifests, outbox/webhook delivery CLI automation, and transport canary tooling, local commit/reveal coordination status, local commit/reveal executor CLI automation plus supervised executor job bundle generation and executor canary evidence tooling, appeal pricing/deposit/readback client/CLI tooling, and transparency readback/source-entry client/CLI tooling exist; captured deployed juror notification transport service rollout evidence and deployed executor job rollout evidence remain live rollout gates. |

## Data Model

The shipped reproducibility manifest records the runner and model fingerprints
that gateways should require before adopting a moderation committee:

```norito
struct ModerationReproManifestV1 {
    body: ModerationReproBodyV1,
    signatures: Vec<ModerationReproSignatureV1>,
}

struct ModerationReproBodyV1 {
    schema_version: u16,
    manifest_id: [u8; 16],
    manifest_digest: Digest32,
    runner_hash: Digest32,
    runtime_version: String,
    issued_at_unix: u64,
    seed_material: ModerationSeedMaterialV1,
    thresholds: ModerationThresholdsV1,
    models: Vec<ModerationModelFingerprintV1>,
    notes: Option<String>,
}

struct ModerationThresholdsV1 {
    quarantine: u16,
    escalate: u16,
}
```

The adversarial corpus manifest records digest families and variants used for
honey-probe and regression testing. These structures are validation artifacts;
they do not execute models or store quarantined payloads.

The local screening checkpoint stores metadata and digests for gateway or
runner-supplied screening outcomes. It does not execute models. `quarantine`
and `escalate` verdicts enqueue local evidence records that can be reviewed and
released through authenticated Torii endpoints, while quarantined payload bytes
can be sealed locally into encrypted object envelopes whose plaintext digest
must match the queue record. The local Torii object endpoint stores base64
payload bytes and returns base64 readback only after canonical request
authentication, `sorafs_moderation_operator` role authorization, and envelope
verification.

## Gateway Policy Integration

GAR v2 carries moderation directives:

- `allow`, `warn`, `quarantine`, and `block` moderation actions.
- Moderation slugs that must be present on gateway requests before content is
  served.
- Torii gateway policy checks that fail closed with `moderation_required` when
  content requires moderation evidence and the request lacks accepted slugs.

Honey-audit tooling exercises this policy path by probing gateway providers with
known denied digests. Operators should archive the emitted JSON/Markdown reports
with governance evidence for the active moderation cohort.

## Operator CLI

Shipped commands:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-repro --manifest docs/examples/ai_moderation_calibration_manifest_202602.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-corpus --manifest docs/examples/ai_moderation_perceptual_registry_202602.json

iroha sorafs moderation registry submit-repro \
  --manifest moderation-repro.to \
  --format norito

iroha sorafs moderation registry submit-corpus \
  --manifest adversarial-corpus.to \
  --format norito

iroha sorafs moderation registry list --limit 25

iroha sorafs moderation ballots list --limit 25

iroha sorafs moderation ballots get \
  --case-id case-401 \
  --round-id round-7 \
  --limit 25

iroha sorafs moderation ballots events \
  --since 0 \
  --limit 25

iroha sorafs moderation ballots commit \
  --payload ballot-commit.to \
  --format norito

iroha sorafs moderation ballots reveal \
  --payload ballot-reveal.to \
  --format norito

iroha sorafs moderation ballots tally \
  --case-id case-401 \
  --round-id round-7

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation run-local \
  --manifest moderation-repro.to \
  --format norito \
  --payload quarantined-candidate.bin \
  --subject cid:bafy... \
  --screened-at <unix-seconds> \
  --json-out screening-result.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation runner-serve \
  --manifest=moderation-repro.to \
  --format=norito \
  --listen=127.0.0.1:9194 \
  --max-body-bytes=16777216

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation runner-grpc-serve \
  --manifest=moderation-repro.to \
  --format=norito \
  --listen=127.0.0.1:9199 \
  --max-body-bytes=16777216

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation registry-serve \
  --state=artifacts/sorafs-moderation-registry/registry-state.to \
  --listen=127.0.0.1:9198 \
  --max-body-bytes=16777216 \
  --snapshot-limit=500

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation runner-bundle \
  --manifest=moderation-repro.to \
  --format=norito \
  --bundle-out=artifacts/sorafs-moderation-runner \
  --listen=127.0.0.1:9194 \
  --max-body-bytes=16777216

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation runner-canary \
  --manifest=moderation-repro.to \
  --format=norito \
  --runner-url=http://127.0.0.1:9194 \
  --payload=canary-payload.bin \
  --subject=cid:bafy... \
  --screened-at=<unix-seconds> \
  --json-out=runner-rollout-evidence.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation committee-run \
  --manifest=moderation-repro.to \
  --format=norito \
  --quorum=2 \
  --result=runner-a.json \
  --result=runner-b.json \
  --json-out committee-aggregate.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation committee-serve \
  --manifest=moderation-repro.to \
  --format=norito \
  --quorum=2 \
  --listen=127.0.0.1:9196 \
  --max-body-bytes=1048576

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation committee-bundle \
  --manifest=moderation-repro.to \
  --format=norito \
  --quorum=2 \
  --bundle-out=artifacts/sorafs-moderation-committee \
  --listen=127.0.0.1:9196 \
  --max-body-bytes=1048576

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation committee-canary \
  --manifest=moderation-repro.to \
  --format=norito \
  --committee-url=http://127.0.0.1:9196 \
  --quorum=2 \
  --result=runner-a.json \
  --result=runner-b.json \
  --json-out=committee-rollout-evidence.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation honey-audit \
  --manifest-id=<hex32> \
  --honey=<digest_hex> \
  --provider name=<alias>,provider-id=<hex32>,base-url=<url>,stream-token=<base64>

iroha sorafs moderation quarantine list --limit 25

iroha sorafs moderation screening submit --input screening-result.json

iroha sorafs moderation screening list --limit 25

iroha sorafs moderation quarantine review \
  --quarantine-id <hex16> \
  --reviewed-by <operator> \
  --reviewed-at @<unix-seconds> \
  --notes "reviewed for local release workflow"

iroha sorafs moderation quarantine release \
  --quarantine-id <hex16> \
  --release-authority <operator> \
  --released-at @<unix-seconds> \
  --notes "released after local review"

iroha sorafs moderation quarantine object store \
  --quarantine-id <hex16> \
  --payload-file quarantined.bin \
  --captured-at @<unix-seconds> \
  --content-type application/octet-stream \
  --notes "sealed local payload"

iroha sorafs moderation quarantine object read \
  --quarantine-id <hex16>

iroha sorafs moderation quarantine operator-panel \
  --quarantine-id <hex16> \
  --limit 25

iroha sorafs moderation quarantine bridge-plan \
  --quarantine-id <hex16> \
  --limit 25

iroha sorafs moderation quarantine appeal-handoff \
  --quarantine-id <hex16> \
  --input appeal-handoff.json

iroha sorafs appeals pricing config

iroha sorafs appeals pricing status

iroha sorafs appeals pricing quote \
  --input appeal-pricing-quote.json

iroha sorafs appeals finance deposits create \
  --input appeal-deposit-request.json

iroha sorafs appeals finance deposits confirm \
  --input appeal-deposit-confirm.json

iroha sorafs appeals finance deposits get \
  --escrow-id <hex32>

iroha sorafs appeals finance deposits settle \
  --input appeal-deposit-settlement.json

iroha sorafs appeals finance deposits reconcile \
  --input appeal-deposit-reconciliation.json

iroha sorafs appeals finance deposits submit-settlement \
  --input appeal-settlement-submission.json

iroha sorafs appeals finance reports --limit 25

iroha sorafs appeals finance weekly-rollups --limit 25

iroha sorafs appeals finance settlement-receipts --limit 25
```

The ballot commands operate on the local Torii moderation ballot lifecycle.
Commit and reveal payloads can be JSON or Norito, but are always validated and
submitted as canonical Norito bytes. The `registry-serve` command exposes the
model-registry admission and bounded snapshot surface as a standalone persistent
HTTP service backed by a Norito checkpoint; admission requests use base64
canonical Norito manifests and remain payload-free. The `runner-serve` command exposes the
same locked-manifest deterministic scoring path as `run-local` over local HTTP;
its screening endpoint requires an explicit timestamp and returns
Torii-compatible screening-result JSON. The `runner-bundle` command generates
the supervised HTTP runner artifacts operators install with systemd or launchd
after pinning the audited `sorafs_cli` binary path in `runner.env`. The
`runner-canary` command captures payload-free rollout evidence from a deployed
HTTP runner by checking status and screening responses against the locked
manifest; it does not replace the planned gRPC runner. The `committee-run`
command aggregates payload-free runner outputs under a validated manifest and
quorum using a deterministic median score. The `committee-serve` command locks
the same manifest and quorum into a bounded local HTTP committee service for
status readback and payload-free aggregation requests. The `committee-bundle`
command generates supervised HTTP committee artifacts operators install with
systemd or launchd after pinning the audited `sorafs_cli` binary path in
`committee.env`. The `committee-canary` command captures payload-free rollout
evidence from a deployed HTTP committee service by checking status and aggregate
responses against the locked manifest and deterministic local aggregation. The
screening submit/list and quarantine list/review/release commands operate on the local Torii evidence
queue. The object store/read commands operate on the local encrypted payload object API.
Review/release and object store/read calls must be signed by an account
assigned the `sorafs_moderation_operator` role. The operator-panel command
reads the local payload-free workflow view for one quarantine record, and the
bridge-plan command converts that view into deterministic next CLI actions for
handoff, ballot, tally, and transparency automation. The operator-serve command
serves a browser operator UI at `/` and
`/v1/sorafs/moderation/operator-panel/ui`, exposes the same payload-free
operator-panel and bridge-plan views over a local HTTP service for operator
tooling, adds a payload-free juror-plan view that reports per-juror commit and
reveal readiness with signed Torii routes and CLI command templates, adds a
payload-free juror-notifications view that emits deterministic delivery records
for operator-managed mail/webhook/scheduler dispatch, adds a payload-free
commit-reveal-status view that reports quorum readiness, missing jurors, and
tally-ready request templates, and forwards signed review, release,
appeal-handoff, appeal-ballot, and
ballot-tally POSTs to Torii after rejecting payload bytes. Ballot-tally
POSTs can carry explicit `case_id`/`round_id` fields or derive the first ballot
reference from the payload-free operator-panel view before submitting the signed
tally request. `iroha sorafs moderation ballots execute --status PATH
[--commit-payload PATH...] [--reveal-payload PATH...] [--submit-tally]` reads
the payload-free coordination status, validates local commit/reveal payload
files against the pending juror lists, submits only pending signed
commit/reveal/tally requests through Torii, and emits response status/body
hashes without printing private reveal payload internals. `iroha sorafs
moderation ballots executor-bundle --status PATH --bundle-out DIR
[--commit-payload PATH...] [--reveal-payload PATH...] [--submit-tally]` now
generates a payload-free scheduled executor job bundle with `executor.env`,
executable `run.sh`, systemd service/timer files, launchd plist, README, and
`sorafs.moderation.ballots.executor_bundle.v1` metadata without copying private
commit/reveal payload files. `iroha sorafs moderation ballots executor-canary
--bundle DIR [--execution-summary PATH] [--out PATH]` now verifies a generated
executor bundle plus an optional payload-free `ballots execute` summary,
records artifact hashes, scheduler checks, summary hashes, and pass/fail
status, and emits `sorafs.moderation.ballots.executor_canary.v1` evidence
without archiving private payload files or response bodies. These commands and
service do not replace deployed juror notification transport or captured live
executor job evidence.
The `operator-canary` command captures payload-free rollout evidence from a
deployed operator workflow service by checking health/status, browser UI,
operator-panel, bridge-plan, juror-plan, juror-notifications, and
commit-reveal-status routes for expected schemas and payload-free responses.
The quarantine appeal-handoff command operates on reviewed local quarantine
records and returns a quote-bound deposit request plus native asset-lock
instruction. The quarantine appeal-ballot command verifies a confirmed
handoff-bound deposit and announces the existing local moderation ballot. The
appeal pricing and finance commands operate on the local Torii appeal handoff
surface. Pricing quote, handoff, appeal-ballot, and finance mutation commands
read JSON payload files, reject empty payloads, and re-encode JSON canonically
before submission; finance mutation, deposit readback, handoff, and
appeal-ballot calls use canonical Iroha request signing. These commands do not
replace the planned automated moderation bridge.

## Observability

Implemented local observability:

- Calibration and corpus evidence fixtures in `docs/examples/`.
- Calibration report under `docs/source/sorafs/reports/`.
- Grafana dashboard `dashboards/grafana/ministry_moderation_overview.json`.
- Prometheus alert rules and tests under `dashboards/alerts/`.

Reserved production metrics:

- `sorafs_ai_screening_requests_total{verdict}`.
- `sorafs_ai_screening_latency_seconds_bucket`.
- `sorafs_ai_model_score_bucket{model}`.
- `sorafs_ai_quarantine_backlog`.
- `sorafs_ai_manifest_version_current`.

Dashboards may keep reserved panels for production rollout, but release
evidence must distinguish shipped validation/honey-audit tooling and local
screening/quarantine evidence checkpoints plus review/release API state from a
live governance-evidence rollout and production quarantine workflow.

## Remaining Production Gates

- Promote local screening/quarantine evidence into the production runner and
  committee workflow with live governance evidence.
- Capture live operator workflow evidence from deployed `operator-canary` runs
  for the shipped browser UI,
  role-gated encrypted quarantine object API, local operator-panel read model,
  local operator workflow service including juror-plan and
  juror-notifications and commit-reveal-status readback, and documented role
  provisioning runbook.
- Wire deployed bridge automation to operate juror notification transport jobs,
  run the shipped notification transport canary against the deployed transport,
  install/run the generated commit/reveal executor job bundles, run the shipped
  executor canary against captured payload-free execution summaries, and publish
  transparency entries from the
  shipped reviewed-quarantine appeal handoff, appeal-ballot, local juror-plan,
  local juror-notifications delivery/canary, commit-reveal-status, and ballots
  executor paths.
- Wire live quarantine/escalation producers into the shipped Governance DAG and
  transparency source-entry tooling.
- Add end-to-end tests covering ingest, quarantine, review, release, appeal, and
  transparency publication.
- Update the portal and OpenAPI/operator docs only after the above commands and
  services exist.

## Rollout Status

Completed local foundations:

- Define governance-signed reproducibility manifests and validators.
- Define adversarial corpus manifests and validators.
- Provide deterministic local runner CLI output for Torii screening-result
  admission fixtures.
- Provide locked-manifest local HTTP runner service mode for deterministic
  screening-result emission without outbound network access.
- Provide supervised local HTTP runner deployment bundle generation for
  systemd/launchd installation of the locked-manifest runner service.
- Provide payload-free HTTP runner canary evidence that verifies deployed
  status and screening responses against the locked reproducibility manifest.
- Provide local deterministic committee aggregation over payload-free runner
  results under a validated reproducibility manifest and quorum.
- Provide locked-manifest local HTTP committee aggregation service endpoints for
  status and payload-free quorum aggregation.
- Provide supervised local HTTP committee deployment bundle generation for
  systemd/launchd installation of the locked-manifest committee service.
- Provide payload-free HTTP committee canary evidence that verifies deployed
  status and aggregate responses against the locked reproducibility manifest and
  deterministic local aggregation.
- Admit validated reproducibility and corpus manifests into a local model
  registry snapshot and Norito checkpoint.
- Expose canonical-authenticated Torii endpoints for local model registry
  admission and bounded readback.
- Provide local client and CLI commands for model-registry admission/readback,
  including canonical Norito conversion from JSON or Norito manifest input.
- Provide local client and CLI commands for moderation ballot list/get/events
  readback plus signed commit/reveal/tally submission, including canonical
  Norito conversion from JSON or Norito ballot payload input.
- Provide local operator-service signed ballot-tally forwarding from explicit
  `case_id`/`round_id` input or the first payload-free operator-panel ballot
  reference.
- Provide local client and CLI commands for appeal pricing quote/config/status,
  asset-lock deposit create/confirm/get/settle/reconcile/submit-settlement,
  and bounded finance report, weekly-rollup, and settlement-receipt readback.
- Provide local Torii, client, and CLI reviewed-quarantine appeal handoff that
  derives quote-bound deposit requests and native asset-lock instructions.
- Provide local Torii, client, and CLI reviewed-quarantine appeal-ballot bridge
  tooling that carries handoff-bound confirmed deposits into local ballot
  announcements.
- Persist deterministic local screening-result and pending-quarantine evidence
  snapshots, with Torii admission/readback endpoints for screening results and
  quarantine records.
- Provide local CLI commands for screening-result admission and bounded
  readback.
- Advance local quarantine records through role-gated reviewed and released
  states with checkpointed operator metadata.
- Seal quarantined payload bytes into encrypted local object envelopes with a
  persisted object index and digest/authentication checks.
- Expose canonical-authenticated and `sorafs_moderation_operator` role-gated
  Torii object store/readback for local encrypted quarantine payloads.
- Provide local CLI commands for quarantine queue listing, review, release, and
  encrypted object store/readback.
- Provide local Torii, client, and CLI operator-panel readback that bundles a
  quarantine record, encrypted-object metadata status, matching local ballots,
  operator routes, and next-action hints without payload bytes.
- Provide local CLI bridge planning from the operator-panel read model into
  ordered payload-free handoff, ballot, tally, and transparency actions.
- Provide local payload-free HTTP operator workflow service routes for health,
  operator-panel readback, and bridge-plan generation backed by the signed
  Torii operator-panel view.
- Provide local payload-free juror notification planning from the operator-panel
  ballot view into per-juror commit/reveal status, signed Torii routes, and CLI
  command templates.
- Provide local payload-free juror notification delivery manifests with
  deterministic dedup keys and operator-managed dispatch records for external
  mail/webhook/scheduler transports.
- Provide local juror notification outbox/webhook delivery CLI automation with
  payload-free delivery evidence and response body hashes.
- Provide local juror notification transport canary evidence tooling with
  payload-free probe status, notification hashes, and response hashes.
- Provide local payload-free commit/reveal coordination status with quorum
  readiness, missing-juror lists, and tally-ready request templates.
- Provide local commit/reveal executor CLI automation that consumes the
  payload-free coordination status, submits only pending local commit/reveal
  payload files, can submit ready tally requests, and emits payload-free
  response hashes.
- Provide local supervised commit/reveal executor job bundle generation with
  `executor.env`, executable `run.sh`, systemd service/timer files, launchd
  plist, README, and payload-free metadata without copying private payload
  files.
- Provide local commit/reveal executor canary evidence tooling that verifies
  generated bundle artifacts and optional payload-free execution summaries
  without archiving private payload files or response bodies.
- Provide local browser operator UI routes backed by the payload-free operator
  workflow service.
- Provide local operator workflow service POST forwarding for signed review,
  release, appeal-handoff, appeal-ballot, and ballot-tally requests, with JSON
  body validation and payload-byte rejection.
- Provide local operator workflow canary evidence tooling that probes deployed
  health/status, browser UI, operator-panel, bridge-plan, juror-plan,
  juror-notifications, and commit-reveal-status routes without archiving
  payload bytes.
- Provide moderation validation CLI commands.
- Provide standalone persistent HTTP model-registry service admission/status and
  bounded snapshot endpoints backed by a Norito checkpoint.
- Provide production unary gRPC runner service status/screening endpoints backed
  by the locked reproducibility manifest and deterministic local runner.
- Provide honey-audit gateway probing.
- Provide calibration fixtures, report, dashboards, and alert rules.
- Wire GAR moderation directives into gateway policy checks.
- Expose bounded Torii gateway denylist catalog readback for operator audits.

Remaining rollout work is captured deployed juror notification transport
service rollout evidence, captured deployed commit/reveal executor job rollout
evidence, and live evidence, not the local
manifest/corpus validators, deterministic local runner CLI output,
locked-manifest local HTTP runner service mode, supervised HTTP runner bundle
generation, production unary gRPC runner service, HTTP runner canary rollout
evidence tooling, local committee aggregation CLI, local committee aggregation
HTTP service, supervised committee bundle generation, HTTP committee canary
rollout evidence tooling,
model-registry admission/checkpoint foundation, standalone persistent
model-registry HTTP service, Torii registry endpoints, local moderation ballot
lifecycle/readback/client/CLI foundation, local screening/quarantine evidence
checkpointing, readback, and review/release API transitions, honey-audit tool,
local encrypted quarantine object-store/API/CLI foundation, local quarantine CLI
commands, local quarantine operator role gate, local appeal
pricing/deposit/readback client/CLI bridge, local
reviewed-quarantine appeal handoff and appeal-ballot API/CLI, local
operator-panel read model, local bridge-plan CLI, local payload-free operator
workflow service, local signed operator workflow mutation forwarding, local
payload-free juror notification planning, local payload-free juror notification
delivery manifests, local juror notification outbox/webhook delivery CLI
automation, local juror notification transport canary evidence tooling, local
payload-free commit/reveal coordination status, local commit/reveal executor
CLI automation, local supervised commit/reveal executor job bundle generation,
local commit/reveal executor canary evidence tooling, local operator workflow
canary evidence tooling, documented operator role-provisioning runbook, GAR
policy plumbing, bounded denylist catalog readback, or observability fixtures.
