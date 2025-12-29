---
lang: ru
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
translator: manual
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/examples/docs_preview_invite_template.md (Docs Portal Preview Invite Template) -->

# Шаблон приглашения на просмотр портала документации

Используйте этот шаблон, когда отправляете ревьюерам инструкции по доступу к preview
портала. Замените плейсхолдеры (`<...>`) актуальными значениями, приложите descriptor +
archive, указанные в письме, и сохраните финальный текст в соответствующем intake‑тикете.

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
