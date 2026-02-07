---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-29T18:16:35.114535+00:00"
translation_last_reviewed: 2026-02-07
id: reviewer-onboarding
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
---

## Overview

DOCS-SORA tracks a staged launch of the developer portal. Checksum-gated builds
(`npm run serve`) and hardened Try it flows unblock the next milestone:
onboarding vetted reviewers before the public preview opens broadly. This guide
describes how to collect requests, verify eligibility, provision access, and
offboard participants safely. Refer to the
[preview invite flow](./preview-invite-flow.md) for cohort planning, invite
cadence, and telemetry exports; the steps below focus on the actions to take
once a reviewer has been selected.

- **Scope:** reviewers who need access to the docs preview (`docs-preview.sora`,
  GitHub Pages builds, or SoraFS bundles) before GA.
- **Out-of-scope:** Torii or SoraFS operators (covered by their own onboarding
  kits) and production portal deployments (see
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Roles & prerequisites

| Role | Typical goals | Required artefacts | Notes |
| --- | --- | --- | --- |
| Core maintainer | Verify new guides, run smoke tests. | GitHub handle, Matrix contact, signed CLA on file. | Usually already in the `docs-preview` GitHub team; still file a request so access is auditable. |
| Partner reviewer | Validate SDK snippets or governance content before public release. | Corporate email, legal POC, signed preview terms. | Must acknowledge telemetry + data handling requirements. |
| Community volunteer | Provide usability feedback on guides. | GitHub handle, preferred contact, timezone, acceptance of CoC. | Keep cohorts small; prioritize reviewers who have signed the contributor agreement. |

All reviewer types must:

1. Acknowledge the acceptable-use policy for preview artefacts.
2. Read the security/observability appendices
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Agree to run `docs/portal/scripts/preview_verify.sh` before serving any
   snapshot locally.

## Intake workflow

1. Ask the requester to fill out the
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   form (or copy/paste it into an issue). Capture at least: identity, contact
   method, GitHub handle, intended review dates, and confirmation that the
   security docs were read.
2. Record the request in the `docs-preview` tracker (GitHub issue or governance
   ticket) and assign an approver.
3. Validate prerequisites:
   - CLA / contributor agreement on file (or partner contract reference).
   - Acceptable-use acknowledgement stored in the request.
   - Risk assessment complete (for example, partner reviewers approved by Legal).
4. Approver signs off in the request and links the tracking issue to any
   change-management entry (example: `DOCS-SORA-Preview-####`).

## Provisioning & tooling

1. **Share artefacts** — Provide the latest preview descriptor + archive from
   the CI workflow or SoraFS pin (`docs-portal-preview` artefact). Remind
   reviewers to run:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serve with checksum enforcement** — Point reviewers at the checksum-gated
   command:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   This reuses `scripts/serve-verified-preview.mjs` so no unverified build can be
   launched accidentally.

3. **Grant GitHub access (optional)** — If reviewers need unpublished branches,
   add them to the `docs-preview` GitHub team for the duration of the review and
   record the membership change in the request.

4. **Communicate support channels** — Share the on-call contact (Matrix/Slack)
   and incident procedure from [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetry + feedback** — Remind reviewers that anonymised analytics are
  collected (see [`observability`](./observability.md)). Provide the feedback
  form or issue template referenced in the invite and log the event with the
  [`preview-feedback-log`](./preview-feedback-log) helper so the wave summary
  stays current.

## Reviewer checklist

Before accessing the preview, reviewers must complete the following:

1. Verify the downloaded artefacts (`preview_verify.sh`).
2. Launch the portal via `npm run serve` (or `serve:verified`) to ensure the
   checksum guard is active.
3. Read the security and observability notes linked above.
4. Test the OAuth/Try it console using device-code login (if applicable) and
   avoid reusing production tokens.
5. File findings in the agreed tracker (issue, shared doc, or form) and tag
   them with the preview release tag.

## Maintainer responsibilities & offboarding

| Phase | Actions |
| --- | --- |
| Kickoff | Confirm intake checklist is attached to the request, share artefacts + instructions, append an `invite-sent` entry via [`preview-feedback-log`](./preview-feedback-log), and schedule a midpoint sync if the review lasts longer than one week. |
| Monitoring | Track preview telemetry (look for unusual Try it traffic, probe failures) and follow the incident runbook if anything suspicious occurs. Log `feedback-submitted`/`issue-opened` events as findings arrive so the wave metrics stay accurate. |
| Offboarding | Revoke temporary GitHub or SoraFS access, record `access-revoked`, archive the request (include feedback summary + outstanding actions), and update the reviewer registry. Ask the reviewer to purge local builds and attach the digest generated from [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Use the same process when rotating reviewers between waves. Keeping the
paper trail in the repo (issue + templates) helps DOCS-SORA remain auditable and
lets governance confirm that preview access followed the documented controls.

## Invite templates & tracking

- Start every outreach with the
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  file. It captures the minimum legal language, preview checksum instructions,
  and the expectation that reviewers acknowledge the acceptable-use policy.
- When editing the template, replace the placeholders for `<preview_tag>`,
  `<request_ticket>`, and contact channels. Store a copy of the final message in
  the intake ticket so reviewers, approvers, and auditors can reference the
  exact wording that was sent.
- After dispatching the invite, update the tracking spreadsheet or issue with
  the `invite_sent_at` timestamp and expected end date so the
  [preview invite flow](./preview-invite-flow.md) report can pick up the cohort
  automatically.
