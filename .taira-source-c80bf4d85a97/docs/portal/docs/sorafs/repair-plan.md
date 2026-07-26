---
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
---

:::note Canonical source
This page summarizes `docs/source/sorafs_repair_plan.md`. The native ledger is
the only V1 repair authority.
:::

## Canonical command contract

Every command body is one caller-signed Iroha `SignedTransaction` containing
exactly one instruction matching its route:

- `/report` uses `SubmitSorafsRepairTask`.
- `/slash`, `/claim`, `/heartbeat`, `/complete`, and `/fail` use the matching
  `ApplySorafsRepairTaskAction`.
- `/appeal` uses `SubmitSorafsRepairAppeal`.

Torii performs exact instruction/action matching and forwards the signed bytes
through strict durable transaction ingress. These command routes return `202
Accepted`. Transaction authority and native execution enforce permissions,
expected revision, live lease owner/generation/expiry, idempotency, terminal
uniqueness, slash state, and provider-owner appeal authority. The deleted
`SignedAuditorRequestV1` and `RepairWorkerSignaturePayloadV1` envelopes are not
compatibility formats.

## Finalized query contract

`/status`, `/tasks`, `/tasks/{ticket_id}`, and `/events` return `200 OK`
finalized ledger projections. Multi-page clients retain the exact finalized
height/block-hash anchor and immutable exclusive cursor; a stale expected anchor
returns `409 Conflict`. The event route exposes the typed payload-free committed
journal. Local status-by-manifest, SSE, and WebSocket authority routes are not
part of V1.

## Governance and remaining blocker

Escalation and appeal state is committed by native repair instructions.
`RepairEscalationApprovalV1` is a bounded governance publication/reference
payload and cannot replace native task or appeal state. Reserve, reputation,
transparency, and Governance DAG workers must consume finalized repair events
with durable cursors and exactly-once handoff.

The residual public `sorafs_node::RepairManager`, filesystem checkpoint, and
GC/reconciliation consumers remain release blockers. They must be removed, and
the reviewed four-validator deployment must prove exact-live-lease execution,
restart reconciliation, and one terminal outcome across duplicate cross-peer
submissions.
