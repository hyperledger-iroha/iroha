# Docs Portal Preview Invite (Template)

Use this template when sending preview access instructions to reviewers. Replace
the placeholders (`<...>`) with the relevant values, attach the descriptor +
archive artefacts referenced in the message, and store the final text inside
the corresponding intake ticket.

```text
Subject: [DOCS-SORA] docs portal preview <preview_tag> invite for <reviewer/org>

Hi <name>,

Thanks for volunteering to review the docs portal ahead of GA. You are cleared
for wave <wave_id>. Please follow the steps below before browsing the preview:

1. Download the verified artefacts from CI or SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. Run the checksum gate:

   ./docs/portal/scripts/preview_verify.sh \
     --descriptor <path-to-descriptor> \
     --archive <path-to-archive> \
     --build-dir <path-to-extracted-build>

3. Serve the preview with checksum enforcement enabled:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. Read the acceptable-use, security, and observability notes:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. File feedback via <request_ticket> and tag each finding with `<preview_tag>`.

Support is available on <contact_channel>. Incident or security issues must be
reported immediately via <incident_channel>. If you need Torii API tokens,
request them through the ticket—never reuse production credentials.

Preview access expires on <end_date> unless extended in writing. We log
checksums and invite metadata for governance; let us know when you have finished
so we can offboard you cleanly.

Thanks again for helping us stabilise the portal!

— DOCS-SORA team
```
