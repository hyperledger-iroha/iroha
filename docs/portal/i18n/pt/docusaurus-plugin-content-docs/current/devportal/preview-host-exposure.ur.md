---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA روڈ میپ کا تقاضا ہے کہ ہر عوامی پریویو وہی pacote verificado por soma de verificação استعمال کرے جو ریویورز لوکل طور پر چلاتے ہیں۔ ریویور آن بورڈنگ (اور convite منظوری ٹکٹ) مکمل ہونے کے بعد اس runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- ریویور آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- تازہ ترین پورٹل build `docs/portal/build/` میں موجود ہو اور checksum تصدیق شدہ ہو (`build/checksums.sha256`).
- SoraFS پریویو اسناد (Torii URL, autoridade, chave privada, جمع شدہ época) ماحولاتی ویری ایبلز میں یا configuração JSON میں محفوظ ہوں جیسے [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Nome do host (`docs-preview.sora.link`, `docs.iroha.tech` وغیرہ) کے ساتھ DNS تبدیلی ٹکٹ کھلا ہو اور on-call رابطے شامل ہوں۔

## مرحلہ 1 - pacote بنائیں اور verificar کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

verificar o manifesto da soma de verificação پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - Artefatos SoraFS پیک کریں

اسٹیٹک سائٹ کو CAR/manifesto determinístico جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` کی ڈیفالٹ ویلیو `docs/portal/artifacts/` ہے۔

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

تیار شدہ `portal.car`, `portal.manifest.*`, descritor, e o manifesto de soma de verificação کو پریویو ویو ٹکٹ کے ساتھ منسلک کریں۔

## مرحلہ 3 - پریویو alias شائع کریں

جب ہوسٹ ایکسپوز کرنے کے لئے تیار ہوں تو pin helper کو **بغیر** `--skip-submit` کے دوبارہ چلائیں۔ Configuração JSON para sinalizadores CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

یہ کمانڈ `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json` لکھتی ہے، جو pacote de evidências de convite کے ساتھ ہونی چاہئیں۔

## Passo 4 - Corte de DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

تیار شدہ JSON کو Ops کے ساتھ شیئر کریں تاکہ DNS تبدیلی عین manifest digest کو ریفرنس کرے۔ اگر rollback کے لئے پچھلا descritor استعمال کیا جا رہا ہو تو `--previous-dns-plan path/to/previous.json` شامل کریں۔

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو sonda کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

sonda, tag de lançamento, cabeçalhos CSP, e metadados de assinatura Você pode usar o cache de borda (saída curl منسلک کریں) تاکہ آڈیٹرز دیکھ سکیں کہ cache de borda گرم ہے۔

## Pacote de evidências

پریویو ویو ٹکٹ میں درج ذیل artefatos شامل کریں اور دعوتی ای میل میں ان کا حوالہ دیں:

| Artefato | مقصد |
|----------|------|
| `build/checksums.sha256` | ثابت کرتا ہے کہ bundle CI build سے میل کھاتا ہے۔ |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | carga útil canônica SoraFS + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | envio de manifesto e vinculação de alias کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (metadados DNS), promoção de rota (`Sora-Route-Binding`) e ponteiro `route_plan` (JSON + modelos de cabeçalho), limpeza de cache, operações کے لئے reversão ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | دستخط شدہ descritor جو arquivo + soma de verificação کو جوڑتا ہے۔ |
| Saída `probe` | تصدیق کرتا ہے کہ لائیو ہوسٹ متوقع tag de lançamento دکھا رہا ہے۔ |