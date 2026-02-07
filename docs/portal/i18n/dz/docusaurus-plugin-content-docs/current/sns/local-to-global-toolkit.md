---
lang: dz
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
---

This page mirrors [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
from the mono-repo. It packages the CLI helpers and runbooks required by roadmap item **ADDR-5c**.

## Overview

- `scripts/address_local_toolkit.sh` wraps the `iroha` CLI to produce:
  - `audit.json` — structured output from `iroha tools address audit --format json`.
  - `normalized.txt` — converted preferred IH58 / second-best compressed (`sora`) literals for every Local-domain selector.
- Pair the script with the address ingest dashboard (`dashboards/grafana/address_ingest.json`)
  and Alertmanager rules (`dashboards/alerts/address_ingest_rules.yml`) to prove the Local-8 /
  Local-12 cutover is safe. Watch the Local-8 and Local-12 collision panels plus the
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, and `AddressInvalidRatioSlo` alerts before
  promoting manifest changes.
- Reference the [Address Display Guidelines](address-display-guidelines.md) and the
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) for UX and incident-response context.

## Usage

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

Options:

- `--format compressed` for `sora…` output instead of IH58.
- `--no-append-domain` to emit bare literals.
- `--audit-only` to skip the conversion step.
- `--allow-errors` to keep scanning when malformed rows appear (matches the CLI behaviour).

The script writes the artefact paths at the end of the run. Attach both files to
your change-management ticket alongside the Grafana screenshot that proves zero
Local-8 detections and zero Local-12 collisions for ≥30 days.

## CI integration

1. Run the script in a dedicated job and upload its outputs.
2. Block merges when `audit.json` reports Local selectors (`domain.kind = local12`).
   at its default `true` value (only override to `false` on dev/test clusters when
   diagnosing regressions) and add
   `iroha tools address normalize --fail-on-warning --only-local` to CI so regression
   attempts fail before hitting production.

See the source document for more details, sample evidence checklists, and the release-note snippet you can reuse when announcing the cutover to customers.
