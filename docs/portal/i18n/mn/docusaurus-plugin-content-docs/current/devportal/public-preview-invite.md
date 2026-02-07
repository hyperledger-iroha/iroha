---
id: public-preview-invite
lang: mn
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
---

## Program goals

This playbook explains how to announce and run the public preview once the
reviewer onboarding workflow is live. It keeps the DOCS-SORA roadmap honest by
ensuring every invite ships with verifiable artefacts, security guidance, and a
clear feedback path.

- **Audience:** curated list of community members, partners, and maintainers who
  signed the preview acceptable-use policy.
- **Ceilings:** default wave size ≤ 25 reviewers, 14-day access window, incident
  response within 24h.

## Launch gating checklist

Complete these tasks before sending any invitation:

1. Latest preview artefacts uploaded in CI (`docs-portal-preview`,
   checksum manifest, descriptor, SoraFS bundle).
2. `npm run --prefix docs/portal serve` (checksum-gated) tested on the same tag.
3. Reviewer onboarding tickets approved and linked to the invite wave.
4. Security, observability, and incident docs validated
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Feedback form or issue template prepared (include fields for severity,
   reproduction steps, screenshots, and environment info).
6. Announcement copy reviewed by Docs/DevRel + Governance.

## Invite package

Every invite must include:

1. **Verified artefacts** — Provide the SoraFS manifest/plan or GitHub artefact
   links plus the checksum manifest and descriptor. Reference the verification
   command explicitly so reviewers can run it before launching the site.
2. **Serve instructions** — Include the checksum-gated preview command:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Security reminders** — Call out that tokens expire automatically, links
   must not be shared, and incidents should be reported immediately.
4. **Feedback channel** — Link to the issue template/form and clarify response
   time expectations.
5. **Program dates** — Provide start/end dates, office hours or sync meetings,
   and the next refresh window.

The sample email in
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
covers these requirements. Update the placeholders (dates, URLs, contacts)
before sending.

## Expose the preview host

Only promote the preview host once onboarding is complete and the change ticket
is approved. See the [preview host exposure guide](./preview-host-exposure.md)
for the end-to-end build/publish/verify steps used in this section.

1. **Build and package:** Stamp the release tag and produce deterministic
   artefacts.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   The pin script writes `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   and `portal.dns-cutover.json` under `artifacts/sorafs/`. Attach those files to the
   invite wave so every reviewer can verify the same bits.

2. **Publish the preview alias:** Rerun the command without `--skip-submit`
   (supply `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, and the
   governance-issued alias proof). The script will bind the manifest to
   `docs-preview.sora` and emit `portal.manifest.submit.summary.json` plus
   `portal.pin.report.json` for the evidence bundle.

3. **Probe the deployment:** Confirm the alias resolves and the checksum matches
   the tag before sending invites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Keep `npm run serve` (`scripts/serve-verified-preview.mjs`) handy as a
   fallback so reviewers can spin up a local copy if the preview edge blips.

## Communications timeline

| Day | Action | Owner |
| --- | --- | --- |
| D-3 | Finalise invite copy, refresh artefacts, dry-run verification | Docs/DevRel |
| D-2 | Governance sign-off + change ticket | Docs/DevRel + Governance |
| D-1 | Send invites using the template, update tracker with recipient list | Docs/DevRel |
| D | Kickoff call / office hours, monitor telemetry dashboards | Docs/DevRel + On-call |
| D+7 | Midpoint feedback digest, triage blocking issues | Docs/DevRel |
| D+14 | Close wave, revoke temporary access, publish summary in `status.md` | Docs/DevRel |

## Access tracking & telemetry

1. Record every recipient, invite timestamp, and revocation date with the
   preview feedback logger (see
   [`preview-feedback-log`](./preview-feedback-log)) so every wave shares the
   same evidence trail:

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Supported events are `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, and `access-revoked`. The log lives at
   `artifacts/docs_portal_preview/feedback_log.json` by default; attach it to
   the invite wave ticket together with consent forms. Use the summary helper
   to produce an auditable roll-up before the close-out note:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   The JSON summary enumerates invites per wave, open recipients, feedback
   counts, and the timestamp of the most recent event. The helper is backed by
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   so the same workflow can run locally or in CI. Use the digest template in
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   when publishing the wave recap.
2. Tag telemetry dashboards with the `DOCS_RELEASE_TAG` used for the wave so
   spikes can be correlated with invite cohorts.
3. Run `npm run probe:portal -- --expect-release=<tag>` after the deploy to
   confirm the preview environment advertises the correct release metadata.
4. Capture any incidents in the runbook template and link them to the cohort.

## Feedback & close-out

1. Aggregate feedback in a shared doc or issue board. Label items with
   `docs-preview/<wave>` so roadmap owners can query them easily.
2. Use the preview logger’s summary output to populate the wave report, then
   summarise the cohort in `status.md` (participants, major findings, planned
   fixes) and update `roadmap.md` if the DOCS-SORA milestone changed.
3. Follow the offboarding steps from
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoke access, archive
   requests, and thank participants.
4. Prepare the next wave by refreshing artefacts, re-running the checksum gates,
   and updating the invite template with new dates.

Consistently applying this playbook keeps the preview program auditable and
gives Docs/DevRel a repeatable way to scale invites as the portal approaches GA.
