<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Local → Global Address Normalisation Toolkit (ADDR-5c)

Roadmap link: **ADDR-5c** — “Local → Global Normalisation Toolkit”

This guide packages the operational steps, CLI helpers, and automation hooks
addresses) to canonical IH58 or compressed forms ahead of the strict-mode
cutover. Pair it with:

- [Address display guidelines](address_display_guidelines.md) — wallet/explorer UX,
  copy helpers, alert references.
- [Address manifest runbook](../runbooks/address_manifest_ops.md) — incident
  response, Alertmanager rules, rollback guidance.
- Grafana dashboard `address_ingest` and alerts
  (`dashboards/grafana/address_ingest.json`,
  `dashboards/alerts/address_ingest_rules.yml`) — telemetry signals backing the
  cutover SLO.

## 1. Goals

2. Provide deterministic conversion helpers (IH58/compressed) so operators can
   refresh manifests, customer lists, and wallet address books.
3. Capture artefacts (audit report + converted list) suitable for compliance
   submissions and SRE readiness reviews.
4. Reuse the same tooling across CI and manual migrations to avoid drift
   between SDKs, wallets, and on-call playbooks.

## 2. Automation helper

`scripts/address_local_toolkit.sh` wraps the `iroha` CLI and emits two outputs:

1. `audit.json` — structured report from `iroha address audit` with entry-by-entry status,
   Local-domain warnings, and parse errors. Use this to prioritise remediation.
2. `normalized.txt` — converted address list that replaces every Local selector
   with the chosen format (IH58 or compressed) while optionally preserving the
   original domain suffix.

### 2.1 Invocation

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

Flags of note:

- `--format compressed` converts to the `snx1…` Sora alphabet instead of IH58.
- `--no-append-domain` emits bare IH58/compressed values (useful for systems
  that store the domain separately).
- `--audit-only` trims the run to the JSON report (no conversion).
- `--allow-errors` keeps scanning when malformed rows are present; the behaviour
  matches the CLI flags exposed by `iroha address audit/normalize`.
- `IROHA_CLI_BIN=/path/to/iroha scripts/address_local_toolkit.sh …` overrides
  the CLI binary (for example inside CI containers).

### 2.2 JSON report format

`audit.json` mirrors the output of `iroha address audit --format json`, which
already powers SDK heuristics. Each entry contains:

```jsonc
{
  "input": "0x0201b18f…",
  "status": "parsed",
  "summary": {
    "detected_format": {"kind": "canonical_hex"},
    "domain": {
      "kind": "local12",
      "warning": "local-domain selector detected…"
    },
    "ih58": {"value": "ih1qzg…", "prefix": 753},
    "compressed": "snx1…",
    "input_domain": "default"
  }
}
```

- `domain.kind = local12` is the guardrail we track on dashboards and alerts.
- `domain.warning` matches the UX/CLI warning strings cited in the display
  guidelines so operators see the same messaging across tooling.

## 3. CI integration

1. Check the script out as part of your pipeline (point it at your export).
2. Archive both `audit.json` and `normalized.txt` as build artefacts; reference
   them from release tickets or readiness reports.
3. Run `iroha address normalize --fail-on-warning --only-local` during PR
   validation once dashboards show zero legitimate Local usage. This blocks
   regressions before the flag flips on Torii.
4. Promote strict-mode gates: Torii now defaults to `torii.strict_addresses=true`;
   keep it enabled on staging and production once telemetry confirms zero Local
   submissions for 30 days, and only override it to `false` temporarily if a
   regression surfaces on non-production clusters.

## 4. Manual triage workflow

1. Export addresses from your database or wallet.
2. Run the toolkit script; inspect `audit.json` for `domain.kind = local12`.
3. Review/spot-check `normalized.txt` (IH58/compressed). Attach both files to
   your change management system, along with the dashboard screenshot showing
   zero Local detections for your surfaces.
4. Update manifests, customer records, or wallet address books with the
   converted values.
5. Notify downstream teams with the template in
   `docs/source/runbooks/address_manifest_ops.md` (the release note snippet is
   reproduced in the display guidelines).

## 5. Alerting & dashboards recap

- **Grafana (`address_ingest`)**
  - Panels: Local-8 detections (5m), Local-12 collision rate (5m), invalid ratio, top contexts.
  - Data sources: `torii_address_local8_total`, `torii_address_local8_domain_total`, `torii_address_collision_total`, `torii_address_collision_domain_total`, `torii_address_invalid_total`.
- **Alertmanager**
  - `AddressLocal8Resurgence` — pages on any Local-8 increment (treat as release blocker).
  - `AddressLocal12Collision` — pages when two Local-12 labels collide; pause manifest promotions until governance approves the fix.
  - `AddressInvalidRatioSlo` — warns when invalid IH58/compressed submissions exceed the 0.1 % budget for ten minutes.

Both alerts reference the address manifest runbook for escalation. Torii now
defaults to `torii.strict_addresses=true`, so only flip the flag to `false`
while debugging on dev/test clusters and restore the default once the issue is
resolved.

## 6. Evidence bundle checklist

Include the following in your Local → Global migration evidence:

- `audit.json` + `normalized.txt` artefacts from the toolkit script.
- Dashboard screenshot showing zero Local-8 usage and zero Local-12 collisions for ≥30 days.
- Copy of the operator notification / release note snippet announcing the change.
- Link to the change ticket where manifests/configs were updated.

Keeping the workflow standardised ensures ADDR-5c acceptance criteria (“detect,
warn, convert, document, notify”) remain satisfied across SDKs, wallets, and
operator pipelines.
