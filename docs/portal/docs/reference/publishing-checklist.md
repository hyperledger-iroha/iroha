# Publishing Checklist

Use this checklist whenever you update the developer portal. It ensures that the
CI build, GitHub Pages deployment, and manual smoke tests cover every section
before a release or roadmap milestone lands.

## 1. Local validation

- `npm run sync-openapi -- --version=current --latest` (add one or more
  `--mirror=<label>` flags when Torii OpenAPI changes for a frozen snapshot).
- `npm run build` – confirm the `Build on Iroha with confidence` hero copy still
  appears in `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – verify the
  checksum manifest (add `--descriptor`/`--archive` when testing downloaded CI
  artefacts).
- `npm run serve` – launches the checksum-gated preview helper which verifies
  the manifest before calling `docusaurus serve`, so reviewers never browse an
  unsigned snapshot (the `serve:verified` alias remains for explicit calls).
- Spot-check the markdown you touched via `npm run start` and the live reload
  server.

## 2. Pull request checks

- Verify the `docs-portal-build` job succeeded in `.github/workflows/check-docs.yml`.
- Confirm `ci/check_docs_portal.sh` ran (CI logs show the hero smoke check).
- Ensure the preview workflow uploaded a manifest (`build/checksums.sha256`) and
  the preview verification script succeeded (CI logs show the
  `scripts/preview_verify.sh` output).
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
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device-code login configured (`DOCS_OAUTH_*`), `security-hardening.md` checklist executed, CSP/Trusted Types headers verified via `npm run build` or `npm run probe:portal`. |

Mark each row as part of your PR review, or note any follow-up tasks so status
tracking stays accurate.

## 4. Release notes

- Include `https://docs.iroha.tech/` (or the environment URL
  from the deployment job) in release notes and status updates.
- Call out any new or changed sections explicitly so downstream teams know where
  to re-run their own smoke tests.
