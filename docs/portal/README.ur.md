---
lang: ur
direction: rtl
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/portal/README.md (SORA Nexus Developer Portal) -->

# SORA Nexus ڈیولپر پورٹل

یہ ڈائریکٹری Docusaurus کا workspace رکھتی ہے، جو interactive ڈیولپر پورٹل کو host
کرتا ہے۔ پورٹل، Norito گائیڈز، SDK quickstarts، اور `cargo xtask openapi` کے ذریعے
جنریٹ ہونے والی OpenAPI ریفرنس کو اکٹھا کرتا ہے، اور انہیں SORA Nexus کی branding کے
ساتھ پیش کرتا ہے جو پورے docs پروگرام میں استعمال ہوتی ہے۔

## پیشگی ضروریات

- Node.js 18.18 یا اس سے نیا (Docusaurus v3 baseline)۔
- Yarn 1.x یا npm ≥ 9، پیکیج مینجمنٹ کے لیے۔
- Rust toolchain (OpenAPI sync script اسی پر منحصر ہے)۔

## Bootstrap

```bash
cd docs/portal
npm install    # یا yarn install
```

## دستیاب اسکرپٹس

| کمانڈ | وضاحت |
|-------|--------|
| `npm run start` / `yarn start` | لوکل dev سرور کو live reload کے ساتھ چلاتا ہے (ڈیفالٹ `http://localhost:3000`)۔ |
| `npm run build` / `yarn build` | `build/` میں پروڈکشن build تیار کرتا ہے۔ |
| `npm run serve` / `yarn serve` | تازہ ترین build کو لوکل طور پر serve کرتا ہے (smoke tests کے لیے مفید)۔ |
| `npm run docs:version -- <label>` | موجودہ docs کا snapshot `versioned_docs/version-<label>` میں بناتا ہے (`docusaurus docs:version` کے گرد wrapper)۔ |
| `npm run sync-openapi` / `yarn sync-openapi` | `static/openapi/torii.json` کو `cargo xtask openapi` کے ذریعے regenerate کرتا ہے (`--mirror=<label>` سے spec کو اضافی version snapshots میں copy کیا جا سکتا ہے)۔ |
| `npm run tryit-proxy` | وہ staging proxy لانچ کرتا ہے جو “Try it” کنسول کو پاور دیتا ہے (کنفیگریشن نیچے دیکھیں)۔ |
| `npm run probe:tryit-proxy` | proxy کے خلاف `/healthz` + sample request probe چلاتا ہے (CI/monitoring helper)۔ |
| `npm run manage:tryit-proxy -- <update|rollback>` | proxy کے `.env` target کو update یا rollback کرتا ہے اور backup کو ہینڈل کرتا ہے۔ |
| `npm run sync-i18n` | یقینی بناتا ہے کہ `i18n/` کے تحت جاپانی، عبرانی، ہسپانوی، پرتگالی، فرانسیسی، روسی، عربی اور اردو کے ترجمہ stubs موجود ہوں۔ |
| `npm run sync-norito-snippets` | curated Kotodama example docs + downloadable snippets کو regenerate کرتا ہے (dev server plugin کے ذریعے خودکار طور پر بھی چلتا ہے)۔ |
| `npm run test:tryit-proxy` | Node test runner (`node --test`) کے ذریعے proxy unit tests کو execute کرتا ہے۔ |

OpenAPI sync script کو یہ درکار ہے کہ `cargo xtask openapi`، ریپوزٹری root سے دستیاب
ہو؛ یہ `static/openapi/` میں ایک deterministic JSON فائل لکھتا ہے، اور اب توقع کرتا
ہے کہ Torii router، live spec فراہم کرے (صرف ہنگامی placeholder output کے لیے
`cargo xtask openapi --allow-stub` استعمال کریں)۔

## Docs versioning اور OpenAPI snapshots

- **Docs ورژن کاٹنا:** `npm run docs:version -- 2025-q3` چلائیں (یا جو بھی agreed label ہو)۔
  اس کے بعد جنریٹ ہونے والی `versioned_docs/version-<label>`, `versioned_sidebars`,
  اور `versions.json` فائلوں کو commit کریں۔ navbar کا version dropdown، نئے snapshot
  کو خود بخود surface کرے گا۔
- **OpenAPI artefacts کو sync کرنا:** ورژن کاٹنے کے بعد canonical spec اور manifest کو
  `cargo xtask openapi --sign <ed25519 key کا path>` سے ریفریش کریں، پھر
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest` کے ذریعے
  matching snapshot لیں۔ script، `static/openapi/versions/2025-q3/torii.json` لکھتا ہے،
  spec کو `versions/current/torii.json` میں mirror کرتا ہے، `versions.json` اور
  `/openapi/torii.json` کو اپ ڈیٹ کرتا ہے، اور signed `manifest.json` کو ہر version
  directory میں clone کرتا ہے، تاکہ تاریخی specs کے پاس ایک ہی provenance metadata
  ہو۔ نئی spec کو مزید تاریخی snapshots میں copy کرنے کے لیے متعدد `--mirror=<label>`
  flags دیے جا سکتے ہیں۔
- **CI expectations:** وہ commits جو docs کو touch کریں، انہیں (جہاں لاگو ہو) version
  bump اور تازہ OpenAPI snapshots شامل کرنے چاہئیں تاکہ Swagger، RapiDoc اور Redoc
  پینلز، تاریخی specs کے درمیان بغیر fetch errors کے سوئچ کر سکیں۔
- **Manifest enforcement:** `sync-openapi` script صرف تب manifests کو copy کرتا ہے جب
  on‑disk `manifest.json`، نئی جنریٹ ہونے والی spec کے ساتھ match کرتا ہو۔ اگر copy
  skip ہو جائے تو canonical manifest کو ری فریش کرنے کے لیے
  `cargo xtask openapi --sign <key>` دوبارہ چلائیں، پھر sync چلائیں تاکہ versioned
  snapshots، signed metadata کو pick کر سکیں۔ `ci/check_openapi_spec.sh`، generator کو
  دوبارہ چلاتا اور manifest کو verify کرتا ہے، اس سے پہلے کہ commits merge ہونے دیے
  جائیں۔

## ساخت (Structure)

```text
docs/portal/
├── docs/                 # پورٹل کے لیے Markdown/MDX مواد
├── i18n/                 # locale overrides (ja/he) جو sync-i18n بناتا ہے
├── src/                  # React صفحات / components (scaffolding)
├── static/               # static assets جو بغیر تبدیلی serve ہوتے ہیں (OpenAPI JSON سمیت)
├── scripts/              # helper scripts (OpenAPI sync وغیرہ)
├── docusaurus.config.js  # سائٹ کی بنیادی configuration
└── sidebars.js           # sidewalks / navigation ماڈل
```

### Try It proxy کی configuration

“Try it” سینڈ باکس، requests کو `scripts/tryit-proxy.mjs` کے ذریعے route کرتا ہے۔
proxy کو لانچ کرنے سے پہلے environment variables کے ذریعے configure کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

staging/production ماحول میں، ان variables کو اپنی platform configuration میں define
کریں (مثلاً GitHub Actions secrets یا container orchestrator env vars وغیرہ)۔

### Preview URLs اور release notes

- public beta preview: `https://docs.iroha.tech/`
- GitHub ہر deployment کے لیے اسی build کو environment **github-pages** کے تحت بھی
  expose کرتا ہے۔
- وہ pull requests جو portal content میں تبدیلی کرتے ہیں، Actions artefacts
  (`docs-portal-preview`, `docs-portal-preview-metadata`) کے ساتھ آتے ہیں جن میں built
  site، checksum manifest، compressed archive، اور descriptor شامل ہوتے ہیں؛ reviewers
  ان bundles کو download کر کے `index.html` کو لوکل طور پر کھول سکتے ہیں، اور
  checksums کی تصدیق کرنے کے بعد previews شیئر کر سکتے ہیں۔ workflow ہر PR پر ایک
  summary comment ڈالتا ہے (manifest/archive hashes اور SoraFS status کے ساتھ)، جو
  reviewers کو تیزی سے بتاتا ہے کہ verification پاس ہو گئی۔
- preview bundle ڈاؤن لوڈ کرنے کے بعد
  `./docs/portal/scripts/preview_verify.sh --build-dir <extracted build> --descriptor <descriptor> --archive <archive>`
  استعمال کریں تاکہ اس بات کی تصدیق ہو سکے کہ artefacts، CI کے ذریعے بنائے گئے
  نتائج کے ساتھ match کرتے ہیں، اس سے پہلے کہ لنک externally شیئر کیا جائے۔
- release notes یا status updates تیار کرتے وقت preview URL کا حوالہ دیں تاکہ external
  reviewers کو repository clone کیے بغیر portal کا تازہ snapshot دیکھنے کا موقع ملے۔
- preview waves کو `docs/portal/docs/devportal/preview-invite-flow.md` کے ذریعے
  coordinate کریں اور اسے
  `docs/portal/docs/devportal/reviewer-onboarding.md` کے ساتھ جوڑیں، تاکہ ہر دعوت (invite)،
  telemetry export، اور offboarding step، وہی evidence trail reuse کریں۔

</div>

