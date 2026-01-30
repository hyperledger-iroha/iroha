---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4920ad0fcc05da537cdaaa21d2bddacdac61c4e7b96c63e604fa1471dd6cd34b
source_last_modified: "2025-11-14T04:43:19.996796+00:00"
translation_last_reviewed: 2026-01-30
---

# پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پریویو وہی checksum-verified bundle استعمال کرے جو ریویورز لوکل طور پر چلاتے ہیں۔ ریویور آن بورڈنگ (اور invite منظوری ٹکٹ) مکمل ہونے کے بعد اس runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- ریویور آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- تازہ ترین پورٹل build `docs/portal/build/` میں موجود ہو اور checksum تصدیق شدہ ہو (`build/checksums.sha256`).
- SoraFS پریویو اسناد (Torii URL، authority، private key، جمع شدہ epoch) ماحولاتی ویری ایبلز میں یا JSON config میں محفوظ ہوں جیسے [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- مطلوبہ hostname (`docs-preview.sora.link`, `docs.iroha.tech` وغیرہ) کے ساتھ DNS تبدیلی ٹکٹ کھلا ہو اور on-call رابطے شامل ہوں۔

## مرحلہ 1 - bundle بنائیں اور verify کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

verify اسکرپٹ checksum manifest کے غائب یا چھیڑ چھاڑ ہونے پر آگے بڑھنے سے انکار کرتا ہے، جس سے ہر پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - SoraFS artifacts پیک کریں

اسٹیٹک سائٹ کو deterministic CAR/manifest جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` کی ڈیفالٹ ویلیو `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

تیار شدہ `portal.car`, `portal.manifest.*`, descriptor، اور checksum manifest کو پریویو ویو ٹکٹ کے ساتھ منسلک کریں۔

## مرحلہ 3 - پریویو alias شائع کریں

جب ہوسٹ ایکسپوز کرنے کے لئے تیار ہوں تو pin helper کو **بغیر** `--skip-submit` کے دوبارہ چلائیں۔ JSON config یا صریح CLI flags فراہم کریں:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

یہ کمانڈ `portal.pin.report.json`, `portal.manifest.submit.summary.json` اور `portal.submit.response.json` لکھتی ہے، جو invite evidence bundle کے ساتھ ہونی چاہئیں۔

## مرحلہ 4 - DNS cutover پلان بنائیں

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

تیار شدہ JSON کو Ops کے ساتھ شیئر کریں تاکہ DNS تبدیلی عین manifest digest کو ریفرنس کرے۔ اگر rollback کے لئے پچھلا descriptor استعمال کیا جا رہا ہو تو `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو probe کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

probe جاری release tag، CSP headers، اور signature metadata کی تصدیق کرتا ہے۔ دو ریجنز سے کمانڈ دوبارہ چلائیں (یا curl output منسلک کریں) تاکہ آڈیٹرز دیکھ سکیں کہ edge cache گرم ہے۔

## Evidence bundle

پریویو ویو ٹکٹ میں درج ذیل artifacts شامل کریں اور دعوتی ای میل میں ان کا حوالہ دیں:

| Artifact | مقصد |
|----------|------|
| `build/checksums.sha256` | ثابت کرتا ہے کہ bundle CI build سے میل کھاتا ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | canonical SoraFS payload + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | manifest submission اور alias binding کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS metadata (ٹکٹ، ونڈو، رابطے)، route promotion (`Sora-Route-Binding`) کا خلاصہ، `route_plan` pointer (JSON پلان + header templates)، cache purge معلومات، اور Ops کے لئے rollback ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | دستخط شدہ descriptor جو archive + checksum کو جوڑتا ہے۔ |
| `probe` output | تصدیق کرتا ہے کہ لائیو ہوسٹ متوقع release tag دکھا رہا ہے۔ |
