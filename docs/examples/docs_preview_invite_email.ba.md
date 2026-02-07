---
lang: ba
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-12-29T18:16:35.071633+00:00"
translation_last_reviewed: 2026-02-07
---

# Docs Portal Preview Invite (Sample Email)

Use this sample when drafting the outbound message. It captures the exact copy
sent to the W2 community reviewers (`preview-2025-06-15`) so future waves can
mirror the tone, verification guidance, and evidence trail without reverse
engineering old tickets. Update the artefact links, hashes, request IDs, and
dates before sending a new invite.

```text
Subject: [DOCS-SORA] docs portal preview preview-2025-06-15 invite for Horizon Wallet

Hi Sam,

Thanks again for volunteering Horizon Wallet for the W2 community preview. Wave
W2 is now cleared, so you can begin your review as soon as you complete the
steps below. Please keep the artefacts and access tokens private—every invite is
tracked in DOCS-SORA-Preview-W2 and auditors will cross-check the acknowledgements.

1. Download the verified artefacts (same bits we shipped to SoraFS and CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. Verify the bundle before extracting:

   ./docs/portal/scripts/preview_verify.sh \
     --descriptor ~/Downloads/descriptor.json \
     --archive ~/Downloads/docs-portal-preview.tar.zst \
     --build-dir ~/sora-docs/preview-2025-06-15

3. Serve the preview with checksum enforcement:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. Review the hardened runbooks before testing:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. File feedback via DOCS-SORA-Preview-REQ-C04 and tag each finding with
   `docs-preview/w2`. Use the feedback form if you prefer a structured intake:
   docs/examples/docs_preview_feedback_form.md.

Support is available in Matrix (`#docs-preview:matrix.org`) and we hold office
hours on 2025-06-18 15:00 UTC. For security or incident escalations, page the
docs on-call alias via ops@sora.org or +1-555-0109 immediately—do not wait for
office hours.

Preview access for Horizon Wallet runs 2025-06-15 → 2025-06-29. Let us know as
soon as you are finished so we can revoke the temporary access keys and record
the close-out in the tracker.

Appreciate your help getting the portal to GA!

— DOCS-SORA team
```
