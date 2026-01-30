---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-integrity-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# checksum سے مشروط پریویو پلان

یہ منصوبہ وہ باقی کام بیان کرتا ہے جو ہر پورٹل پریویو آرٹیفیکٹ کو اشاعت سے پہلے قابلِ تصدیق بنانے کے لئے درکار ہے۔ مقصد یہ ہے کہ ریویورز CI میں بنائی گئی بالکل وہی اسنیپ شاٹ ڈاؤن لوڈ کریں، checksum مینی فیسٹ ناقابلِ تغیر رہے، اور پریویو SoraFS کے ذریعے Norito میٹا ڈیٹا کے ساتھ قابلِ دریافت ہو۔

## اہداف

- **متعین (Deterministic) builds:** یقینی بنائیں کہ `npm run build` قابلِ اعادہ آؤٹ پٹ بنائے اور ہمیشہ `build/checksums.sha256` خارج کرے۔
- **تصدیق شدہ پریویوز:** ہر پریویو آرٹیفیکٹ کے ساتھ checksum مینی فیسٹ لازمی ہو اور تصدیق ناکام ہونے پر اشاعت روک دی جائے۔
- **Norito میں شائع شدہ میٹا ڈیٹا:** پریویو ڈسکرپٹرز (commit میٹا ڈیٹا، checksum digest، اور SoraFS CID) کو Norito JSON کے طور پر محفوظ کریں تاکہ گورننس ٹولز ریلیز کو آڈٹ کر سکیں۔
- **آپریٹر ٹولنگ:** ایک مرحلہ وار تصدیقی اسکرپٹ فراہم کریں جو صارفین لوکل طور پر چلا سکیں (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); اسکرپٹ اب checksum + descriptor کی توثیق کے فلو کو ابتدا سے انتہا تک سمیٹتا ہے۔ معیاری پریویو کمانڈ (`npm run serve`) اب `docusaurus serve` سے پہلے یہ ہیلپر خودکار طور پر چلاتی ہے تاکہ لوکل اسنیپ شاٹس checksum گیٹنگ کے تحت رہیں (اور `npm run serve:verified` بطور واضح عرف برقرار رہے).

## مرحلہ 1 — CI نفاذ

1. `.github/workflows/docs-portal-preview.yml` کو اپ ڈیٹ کریں تاکہ:
   - Docusaurus build کے بعد `node docs/portal/scripts/write-checksums.mjs` چلایا جائے (لوکل طور پر پہلے ہی چلتا ہے)۔
   - `cd build && sha256sum -c checksums.sha256` چلایا جائے اور عدم مطابقت پر جاب فیل ہو۔
   - build ڈائریکٹری کو `artifacts/preview-site.tar.gz` کے طور پر پیک کریں، checksum مینی فیسٹ کاپی کریں، `scripts/generate-preview-descriptor.mjs` چلائیں، اور JSON کنفیگ کے ساتھ `scripts/sorafs-package-preview.sh` چلائیں (دیکھیں `docs/examples/sorafs_preview_publish.json`) تاکہ workflow میٹا ڈیٹا اور ایک متعین SoraFS بنڈل دونوں جاری کرے۔
   - اسٹیٹک سائٹ، میٹا ڈیٹا آرٹیفیکٹس (`docs-portal-preview`, `docs-portal-preview-metadata`)، اور SoraFS بنڈل (`docs-portal-preview-sorafs`) اپ لوڈ کریں تاکہ مینی فیسٹ، CAR سمری اور پلان کو دوبارہ build چلائے بغیر دیکھا جا سکے۔
2. pull request میں checksum تصدیق کے نتیجے کا خلاصہ دینے والا CI بیج کمنٹ شامل کریں (یہ `docs-portal-preview.yml` میں GitHub Script کمنٹ قدم کے ذریعے نافذ ہے)۔
3. `docs/portal/README.md` (CI سیکشن) میں workflow کو دستاویزی شکل دیں اور publishing checklist میں verification مراحل کا لنک دیں۔

## توثیقی اسکرپٹ

`docs/portal/scripts/preview_verify.sh` ڈاؤن لوڈ شدہ پریویو آرٹیفیکٹس کی تصدیق دستی `sha256sum` چلائے بغیر کرتا ہے۔ لوکل اسنیپ شاٹس شیئر کرتے وقت `npm run serve` (یا واضح عرف `npm run serve:verified`) استعمال کریں تاکہ اسکرپٹ چل کر `docusaurus serve` ایک ہی قدم میں شروع ہو جائے۔ توثیقی منطق:

1. مناسب SHA ٹول (`sha256sum` یا `shasum -a 256`) کو `build/checksums.sha256` کے خلاف چلاتا ہے۔
2. اختیاری طور پر پریویو ڈسکرپٹر `checksums_manifest` کے digest/فائل نام اور، اگر فراہم ہو، پریویو آرکائیو کے digest/فائل نام کا موازنہ کرتا ہے۔
3. کسی بھی عدم مطابقت پر نان زیرو کوڈ کے ساتھ ختم ہوتا ہے تاکہ ریویورز چھیڑے گئے پریویوز کو بلاک کر سکیں۔

مثال استعمال (CI آرٹیفیکٹس نکالنے کے بعد):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI اور ریلیز انجینئرز کو اسکرپٹ اس وقت چلانا چاہئے جب بھی وہ پریویو بنڈل ڈاؤن لوڈ کریں یا ریلیز ٹکٹ کے ساتھ آرٹیفیکٹس منسلک کریں۔

## مرحلہ 2 — SoraFS اشاعت

1. پریویو workflow میں ایسا جاب شامل کریں جو:
   - `sorafs_cli car pack` اور `manifest submit` کے ذریعے تیار شدہ سائٹ کو SoraFS staging گیٹ وے پر اپ لوڈ کرے۔
   - واپس آنے والے مینی فیسٹ digest اور SoraFS CID کو محفوظ کرے۔
   - `{ commit, branch, checksum_manifest, cid }` کو Norito JSON میں سریلائز کرے (`docs/portal/preview/preview_descriptor.json`)۔
2. ڈسکرپٹر کو build آرٹیفیکٹ کے ساتھ محفوظ کریں اور pull request کمنٹ میں CID ظاہر کریں۔
3. ایسے انٹیگریشن ٹیسٹس شامل کریں جو `sorafs_cli` کو dry-run موڈ میں چلائیں تاکہ مستقبل کی تبدیلیاں میٹا ڈیٹا اسکیم کی مطابقت برقرار رکھیں۔

## مرحلہ 3 — گورننس اور آڈٹ

1. Norito اسکیمہ (`PreviewDescriptorV1`) شائع کریں جو ڈسکرپٹر کی ساخت بیان کرے اور اسے `docs/portal/schemas/` کے تحت رکھیں۔
2. DOCS-SORA publishing checklist کو اپ ڈیٹ کریں تاکہ:
   - `sorafs_cli manifest verify` کو اپ لوڈ شدہ CID کے خلاف چلایا جائے۔
   - checksum مینی فیسٹ digest اور CID کو ریلیز PR کی تفصیل میں ریکارڈ کیا جائے۔
3. گورننس آٹومیشن کو جوڑیں تاکہ ریلیز ووٹس کے دوران ڈسکرپٹر کو checksum مینی فیسٹ کے ساتھ کراس چیک کیا جا سکے۔

## ڈیلیوریبلز اور ذمہ داریاں

| مرحلہ | مالک | ہدف | نوٹس |
|-------|------|-----|------|
| CI میں checksum نفاذ مکمل | Docs انفراسٹرکچر | ہفتہ 1 | فیل گیٹ اور آرٹیفیکٹ اپ لوڈز شامل کرتا ہے۔ |
| SoraFS پریویو اشاعت | Docs انفراسٹرکچر / Storage ٹیم | ہفتہ 2 | staging کریڈینشلز تک رسائی اور Norito اسکیمہ اپ ڈیٹس درکار ہیں۔ |
| گورننس انٹیگریشن | Docs/DevRel لیڈ / گورننس WG | ہفتہ 3 | اسکیمہ شائع کرتا ہے اور چیک لسٹس و روڈ میپ اندراجات اپ ڈیٹ کرتا ہے۔ |

## کھلے سوالات

- SoraFS کا کون سا ماحول پریویو آرٹیفیکٹس رکھے (staging بمقابلہ مخصوص preview lane)؟
- کیا اشاعت سے پہلے پریویو ڈسکرپٹر پر دوہری دستخط (Ed25519 + ML-DSA) درکار ہیں؟
- کیا CI workflow کو `sorafs_cli` چلاتے وقت orchestrator کی ترتیب (`orchestrator_tuning.json`) فکس کرنی چاہئے تاکہ مینی فیسٹس قابلِ اعادہ رہیں؟

فیصلوں کو `docs/portal/docs/reference/publishing-checklist.md` میں درج کریں اور غیر واضح امور حل ہونے پر اس منصوبے کو اپ ڈیٹ کریں۔
