---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 302b74b4022656e57c2b876a8f15bf5301a593030a18ad1b93780061e5d783ef
source_last_modified: "2026-01-21T19:17:13.232211+00:00"
translation_last_reviewed: 2026-02-07
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
---

:::note Canonical Source
Mirrors `docs/source/sorafs_repair_plan.md`. Keep both versions in sync until the Sphinx set is retired.
:::

## Governance Decision Lifecycle
1. Escalated repairs create a slash proposal draft and open the dispute window.
2. Governance voters submit approve/reject votes during the dispute window.
3. At `escalated_at_unix + dispute_window_secs` the decision is computed deterministically: minimum voters, approvals exceed rejections, and the approval ratio meets the quorum threshold.
4. Approved decisions open an appeal window; appeals recorded before `approved_at_unix + appeal_window_secs` mark the decision as appealed.
5. Penalty caps apply to all proposals; submissions above the cap are rejected.

## Governance Escalation Policy
The escalation policy is sourced from `governance.sorafs_repair_escalation` in `iroha_config` and is enforced for every repair slash proposal.

| Setting | Default | Meaning |
|---------|---------|---------|
| `quorum_bps` | 6667 | Minimum approval ratio (basis points) among counted votes. |
| `minimum_voters` | 3 | Minimum number of distinct voters required to resolve a decision. |
| `dispute_window_secs` | 86400 | Time after escalation before votes are finalized (seconds). |
| `appeal_window_secs` | 604800 | Time after approval during which appeals are accepted (seconds). |
| `max_penalty_nano` | 1,000,000,000 | Maximum slash penalty allowed for repair escalations (nano-XOR). |

- Scheduler-generated proposals are capped at `max_penalty_nano`; auditor submissions above the cap are rejected.
- Vote records are stored in `repair_state.to` with deterministic ordering (`voter_id` sorting) so all nodes derive the same decision timestamp and outcome.
