---
id: runbooks-index
lang: my
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
---

> Mirrors the owner ledger that lives under `docs/source/sorafs/runbooks/`.
> Every new SoraFS operations guide must be linked here once it is published in
> the portal build.

Use this page to verify which runbooks have completed the migration from the
source path, and the portal copy so reviewers can jump straight to the desired
guide during the beta preview.

## Beta preview host

The DocOps wave has now promoted the reviewer-approved beta preview host at
`https://docs.iroha.tech/`. When pointing operators or reviewers to a migrated
runbook, reference that hostname so they exercise the checksum-gated portal
snapshot. Publishing/rollback procedures live in
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Owner(s) | Portal copy | Source |
|---------|----------|-------------|--------|
| Gateway & DNS kickoff | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS operations playbook | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Capacity reconciliation | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Pin registry ops | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Node operations checklist | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Dispute & revocation runbook | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Staging manifest playbook | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai anchor observability | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Verification checklist

- [x] Portal build links to this index (sidebar entry).
- [x] Every migrated runbook lists the canonical source path to keep reviewers
  aligned during doc reviews.
- [x] The DocOps preview pipeline blocks merges when a listed runbook is missing
  from the portal output.

Future migrations (e.g., new chaos drills or governance appendices) should add a
row to the table above and update the DocOps checklist embedded in
`docs/examples/docs_preview_request_template.md`.
