---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-11-05T23:45:36.259903+00:00"
translation_last_reviewed: 2026-01-30
---

# Publishing checklist

جب بھی آپ developer portal اپڈیٹ کریں تو اس checklist کا استعمال کریں۔ یہ یقینی بناتی ہے کہ CI build، GitHub Pages deployment، اور دستی smoke tests ہر سیکشن کو کور کریں اس سے پہلے کہ کوئی release یا roadmap milestone آئے۔

## 1. Local validation

- `npm run sync-openapi -- --version=current --latest` (Torii OpenAPI بدلنے پر ایک یا زیادہ `--mirror=<label>` flags شامل کریں تاکہ ایک frozen snapshot بنے).
- `npm run build` – تصدیق کریں کہ `Build on Iroha with confidence` hero copy اب بھی `build/index.html` میں موجود ہے۔
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – checksum manifest verify کریں (downloaded CI artefacts ٹیسٹ کرتے وقت `--descriptor`/`--archive` شامل کریں).
- `npm run serve` – checksum-gated preview helper لانچ کرتا ہے جو `docusaurus serve` کو کال کرنے سے پہلے manifest verify کرتا ہے، تاکہ reviewers کبھی unsigned snapshot نہ براؤز کریں (`serve:verified` alias واضح کالز کیلئے موجود رہتا ہے).
- `npm run start` اور live reload server کے ذریعے اس markdown کو spot-check کریں جسے آپ نے touch کیا ہے۔

## 2. Pull request checks

- `.github/workflows/check-docs.yml` میں `docs-portal-build` job کی کامیابی verify کریں۔
- تصدیق کریں کہ `ci/check_docs_portal.sh` چلا (CI logs میں hero smoke check دکھائی دیتا ہے)۔
- یقینی بنائیں کہ preview workflow نے manifest (`build/checksums.sha256`) upload کیا اور preview verification script کامیاب ہوا (CI logs میں `scripts/preview_verify.sh` output دکھائی دیتا ہے)۔
- GitHub Pages environment کی published preview URL کو PR description میں شامل کریں۔

## 3. Section sign-off

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Hero copy render ہو، quickstart cards valid routes پر جائیں، CTA buttons resolve ہوں۔ |
| Norito | Norito WG | Overview اور getting-started guides تازہ ترین CLI flags اور Norito schema docs کو refer کریں۔ |
| SoraFS | Storage Team | Quickstart مکمل ہو، manifest report fields documented ہوں، fetch simulation instructions verify ہوں۔ |
| SDK guides | SDK leads | Rust/Python/JS guides موجودہ examples compile کریں اور live repos سے link ہوں۔ |
| Reference | Docs/DevRel | Index تازہ ترین specs دکھائے، Norito codec reference `norito.md` سے match کرے۔ |
| Preview artifact | Docs/DevRel | `docs-portal-preview` artifact PR کے ساتھ attach ہو، smoke checks pass ہوں، link reviewers کے ساتھ share ہو۔ |
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device-code login configure ہو (`DOCS_OAUTH_*`)، `security-hardening.md` checklist execute ہو، CSP/Trusted Types headers `npm run build` یا `npm run probe:portal` سے verify ہوں۔ |

ہر row کو اپنے PR review کا حصہ بنائیں، یا follow-up tasks لکھیں تاکہ status tracking درست رہے۔

## 4. Release notes

- `https://docs.iroha.tech/` (یا deployment job سے ملا environment URL) کو release notes اور status updates میں شامل کریں۔
- نئے یا تبدیل شدہ sections کو واضح طور پر بیان کریں تاکہ downstream teams جان سکیں کہ انہیں اپنے smoke tests کہاں دوبارہ چلانے ہیں۔
