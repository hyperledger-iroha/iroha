---
lang: am
direction: ltr
source: docs/source/ops/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 612ae481318026a7ae6f84fa7d1186f8c915e68469de6561cb7501a1ce3201c9
source_last_modified: "2025-12-29T18:16:36.003949+00:00"
translation_last_reviewed: 2026-02-07
title: Ops Templates Catalog
summary: Shared postmortem and rollback templates covering SoraNet transport, SNS registrar, DA ingestion, and CDN launches.
---

# Operations Template Catalog

This directory provides the canonical templates required by the roadmap for
SoraNet transport, SNS registrar, DA ingestion, and CDN launches. The goal is to
keep incident investigations and rollback drills consistent across programs and
publish reproducible artefacts along with `status.md` entries.

Use the templates below as the starting point for any new incident postmortem,
chaos rehearsal, or rollback readiness plan:

- [`postmortem_template.md`](./postmortem_template.md) — structured report for
  production incidents or chaos drills.
- [`rollback_playbook_template.md`](./rollback_playbook_template.md) — checklist
  for reversible feature gates (e.g., SoraNet transport policy switches,
  registrar upgrades, DA ingestion changes, CDN deployments).

> Tip: link the filled-out template in `status.md` and attach it to the release
> evidence package for the relevant milestone (SF-6, SNNet-5, DA-1, etc.).

Each template includes placeholders for telemetry snapshots, Norito artefact
hashes, governance approvals, and rollback verification evidence.

## Ready-to-use Playbooks

The following playbooks are already instantiated and can be referenced directly
in release dockets:

- [`soranet_transport_rollback.md`](./soranet_transport_rollback.md) — rollback
  plan for the SNNet-5/SF-6 multi-source default that satisfies the roadmap’s
  “SoraNet transport” requirement. Pair it with the SF-6 rollout checklist and
  attach the captured artefacts to governance packets.
- [`da_ingestion_rollback.md`](./da_ingestion_rollback.md) — rollback plan for the
  Torii `/v1/da/ingest` surface (DA-2/DA-7), including ingress block templates,
  rent/manifest evidence capture, and governance communications.

> Playbooks for the SNS registrar and CDN launches track under the same roadmap
> section and will be added here once their rollout packages are finalised.
