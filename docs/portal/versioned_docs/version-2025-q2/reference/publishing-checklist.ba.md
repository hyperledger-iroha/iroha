---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-12-29T18:16:35.907815+00:00"
translation_last_reviewed: 2026-02-07
---

# Publishing Checklist

Use this checklist whenever you update the developer portal. It ensures that the
CI build, GitHub Pages deployment, and manual smoke tests cover every section
before a release or roadmap milestone lands.

## 1. Local validation

- `npm run sync-openapi` (when Torii OpenAPI changes).
- `npm run build` – confirm the `Build on Iroha with confidence` hero copy still
  appears in `build/index.html`.
- `cd build && sha256sum -c checksums.sha256` – verify the checksum manifest the
  build generated.
- Spot-check the markdown you touched via `npm run start` and the live reload
  server.

## 2. Pull request checks

- Verify the `docs-portal-build` job succeeded in `.github/workflows/check-docs.yml`.
- Confirm `ci/check_docs_portal.sh` ran (CI logs show the hero smoke check).
- Ensure the preview workflow uploaded a manifest (`build/checksums.sha256`) and
  that `sha256sum -c` passed in CI.
- Add the published preview URL from the GitHub Pages environment to the PR
  description.

## 3. Section sign-off

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Hero copy renders, quickstart cards link to valid routes, CTA buttons resolve. |
| Norito | Norito WG | Overview and getting-started guides reference the latest CLI flags and Norito schema docs. |
| SoraFS | Storage Team | Quickstart runs to completion, manifest report fields documented, fetch simulation instructions verified. |
| SDK guides | SDK leads | Rust/Python/JS guides compile the current examples and link to live repos. |
| Reference | Docs/DevRel | Index lists the newest specs, Norito codec reference matches `norito.md`. |
| Preview artifact | Docs/DevRel | `docs-portal-preview` artifact attached to the PR, smoke checks pass, link shared with reviewers. |

Mark each row as part of your PR review, or note any follow-up tasks so status
tracking stays accurate.

## 4. Release notes

- Include `https://docs.iroha.tech/` (or the environment URL
  from the deployment job) in release notes and status updates.
- Call out any new or changed sections explicitly so downstream teams know where
  to re-run their own smoke tests.
