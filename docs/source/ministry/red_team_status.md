---
title: Ministry Red-Team Status (MINFO-9)
summary: Current moderation red-team evidence posture and the runtime-only admission rule for completed drills.
---

# Ministry Red-Team Status

This page complements the [Moderation Red-Team Plan](moderation_red_team_plan.md).
As of 2026-07-25, no completed genuine moderation red-team drill or reviewed
runtime evidence bundle is recorded here. This page is an evidence index, not
evidence by itself.

## Evidence Status

No completed drill is recorded. Production readiness must remain blocked on
this evidence until an authorized drill actually runs against the reviewed
deployment and its payload-free evidence passes independent review.

Only genuine runtime evidence may populate a completed-drill row. Before adding
one, reviewers must verify:

- the drill completed at or before the review time on the reviewed production
  deployment context;
- runtime-generated manifests, dashboard snapshots, alert outcomes, and
  remediation records exist and their digests are independently verified;
- the evidence is payload-free and contains no credentials, private moderation
  material, signing keys, tokens, or synthetic success claims; and
- the reviewer, accountable owners, unresolved findings, and retention location
  are recorded without embedding private evidence.

## Tracking & Tooling

- Use `scripts/ministry/moderation_payload_tool.py` to package injectible
  test payloads for an authorized runtime drill. Generated inputs are not proof
  that the drill ran.
- Record dashboard/log captures via `scripts/ministry/export_red_team_evidence.py`
  immediately after an actual drill so the evidence manifest contains reviewed
  hashes.
- CI guard `ci/check_ministry_red_team.sh` rejects committed drill reports that
  retain template placeholders. Passing that structural guard does not verify
  referenced artefacts or convert examples, scaffolds, or future-dated reports
  into runtime evidence.
