---
lang: mn
direction: ltr
source: docs/source/reports/soradns_transparency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b18f9328e2445fcdbdb5c47137a109bf6a6999a2290c0d04686d6323a3f4ddf9
source_last_modified: "2025-12-29T18:16:36.026962+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraDNS Transparency Report Playbook

This note captures the workflow for producing the weekly SoraDNS transparency
package referenced by roadmap task DG-5a. It ties the new resolver log tailer,
ClickHouse export format, and Prometheus alert rules into a single playbook so
observability and governance operators share a consistent view of resolver
freshness and bundle provenance.

## Data Sources

- **Resolver transparency log** – emitted by `tools/soradns-resolver` when the
  `event_log_path` configuration is set. Each line is a Norito JSON object that
  now includes:
  - Signed Tree Head proxy (`policy_hash_hex`)
  - CAR root CID
  - Freshness window (`issued_at`, `expires_at`, signer, signature)
  - Bundle deltas (`added`, `updated`, `reorged`, `removed`)
  - Resolver advertisement changes
- **Prometheus metrics** – the transparency tailer publishes key metrics
  after ingestion:
  - `soradns_bundle_proof_age_seconds{resolver_id,namehash}` – latest proof age
  - `soradns_bundle_proof_ttl_seconds{resolver_id,namehash}` – freshness TTL
  - `soradns_bundle_cid_drift_total{resolver_id,namehash}` – counter of CID
    changes

## Log Tailer CLI

Run the tailer directly from the workspace using:

- See `scripts/telemetry/run_soradns_transparency_tail.sh` for the managed
  telemetry wrapper. It defaults to reading the resolver log, exporting
  JSONLines to `artifacts/soradns/transparency.jsonl`, writing Prometheus
  metrics to `artifacts/soradns/transparency.prom`, and optionally pushing
  metrics to a configured Pushgateway.
- The underlying CLI (`cargo run -p soradns-resolver --bin soradns_transparency_tail`)
  supports both JSONL and TSV formats, STDIN/STDOUT streaming, and `--metrics-output`
  for direct Prometheus text generation (set to `-` to emit metrics to stdout).

## Signed Tree Head Report CLI

- `cargo run -p soradns-resolver --bin soradns_transparency_report -- --log /var/log/soradns/resolver.transparency.log --output artifacts/soradns/transparency_report.md --sth-output artifacts/soradns/signed_tree_heads.json`
  renders a Markdown summary plus the canonical Signed Tree Head manifest. The report captures
  bundle/resolver counters, proof-age rollups, CID drift totals, and the most recent events so the
  weekly freeze/advisory notes can be produced without hand-editing spreadsheets.
- The JSON artefact mirrors the Markdown "Signed Tree Heads" table: each entry contains
  `{resolver_id, namehash, zone_version, policy_hash_hex, manifest_hash_hex, car_root_cid,
  freshness_{issued,expires}_at, freshness_signer, freshness_signature_hex, cid_drift_total}`.
  Publish the JSON document alongside the Markdown report so operators, auditors, and wallet
  vendors can reproduce the digest comparison without scraping PDF/Markdown output.
- `--recent-limit` (default: 20) controls how many of the latest events are rendered in the
  Markdown timeline. Operators can bump the limit during an incident review for more context, or
  shrink it for the weekly digest. The CLI uses the same parser as the tailer, so the log can be
  streamed through `jq`/`gzip` or read from STDIN for quick ad-hoc checks.
- When torii governance requires an evidence bundle, archive both artefacts under
  `artifacts/soradns/<stamp>/` (e.g., `artifacts/soradns/2026W12/`) along with the tailer metrics
  and Grafana snapshots referenced below.

## Prometheus Alerts

`dashboards/alerts/soradns_transparency_rules.yml` defines the first guardrails:

- `SoradnsProofAgeExceeded` (warning) – proof age > 3600 s for five minutes.
- `SoradnsCidDriftDetected` (critical) – CID drift observed within ten minutes.

The accompanying promtool unit test
`dashboards/alerts/tests/soradns_transparency_rules.test.yml` validates both
rules. Run `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
locally before updating alert logic.

## Weekly Report Template

Publish the transparency update every Monday using the following template:

1. **Summary** – list resolvers, bundle counts, and any CID drifts captured by
   the tailer since the previous report.
2. **Freshness chart** – plot median and P95 proof age per resolver using
   `soradns_bundle_proof_age_seconds`.
3. **CID drift review** – enumerate entries where the counter increased; include
   GAR approval references and remediation status.
4. **STH digest sample** – either embed the "Signed Tree Heads" table emitted by the
   `soradns_transparency_report` CLI or copy the subset relevant to the freeze/advisory notice.
   Make sure the JSON file is linked for anyone validating the hashes programmatically.
5. **Follow-ups** – assign owners for any outstanding investigations and link
   to incident or governance tickets.

The report should embed:

- Tailer output (append `transparency.jsonl` or aggregate slices).
- Grafana snapshots showing alert health.
- Links to ClickHouse queries used for derived metrics.

## Operational Checklist

- [ ] Tail the most recent resolver logs with the CLI and ingest into
      ClickHouse/STH dashboard.
- [ ] Verify Prometheus alerts remain green or have recorded acknowledged
      incidents.
- [ ] Update the transparency report and share with the governance mailing list.
- [ ] File follow-up tickets for any CID drifts or stale proofs beyond policy.
