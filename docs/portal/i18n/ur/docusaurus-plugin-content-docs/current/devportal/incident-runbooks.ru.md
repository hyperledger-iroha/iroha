---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# واقعات اور ٹریننگ رول بیک کی کتابیں

## مقصد

روڈ میپ آئٹم ** دستاویزات -9 ** کے لئے ایپلی کیشن پلے بوکس اور ریہرسل پلان کی ضرورت ہے
پورٹل آپریٹرز بغیر کسی تخمینے کے ترسیل کی ناکامیوں سے صحت یاب ہوسکتے ہیں۔ یہ نوٹ
تین اعلی سگنل واقعات کا احاطہ کرتا ہے - ناکام تعیناتی ، نقل کی انحطاط اور
تجزیات کی ناکامی - اور دستاویزات سہ ماہی کی تربیت سے یہ ثابت ہوتا ہے کہ رول بیک الیاس
اور مصنوعی توثیق کام ختم ہونے کے لئے جاری ہے.

### متعلقہ مواد

-.
-.
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops)
  - رجسٹری ٹیلی میٹری اور اسکیلیشن دہلیز۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور مددگار `npm run probe:*`
  چیک لسٹس میں ذکر کیا گیا۔

### عمومی ٹیلی میٹری اور آلات

| سگنل/آلہ | منزل |
| --------------- | ------- |
| `torii_sorafs_replication_sla_total` (میٹ/مس/زیر التواء) | نقل کو روکنے اور ایس ایل اے کی خلاف ورزیوں کا پتہ لگاتا ہے۔ |
| `torii_sorafs_replication_backlog_total` ، `torii_sorafs_replication_completion_latency_epochs` | ٹریج کے لئے بیک بلاگ کی گہرائی اور تکمیل میں تاخیر کی پیمائش کرتی ہے۔ |
| `torii_sorafs_gateway_refusals_total` ، `torii_sorafs_manifest_submit_total{status="error"}` | گیٹ وے کی طرف ناکامیوں کو ظاہر کرتا ہے ، اکثر خراب تعیناتی کے بعد۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | مصنوعی تحقیقات جو گیٹ جاری کرتی ہے اور رول بیکس کو چیک کرتی ہے۔ |
| `npm run check:links` | گیٹ ٹوٹے ہوئے روابط ؛ ہر تخفیف کے بعد استعمال کیا جاتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے) | پروموشن/ریورٹ عرف میکانزم۔ |
| `Docs Portal Publishing` Grafana بورڈ (`dashboards/grafana/docs_portal.json`) | اجتماعی انکار/عرف/ٹی ایل ایس/نقل ٹیلی میٹری۔ پیجریڈی الرٹس ان پینلز کو بطور ثبوت حوالہ دیتے ہیں۔ |

## رن بوک - ناکام تعیناتی یا خراب نمونے

### ٹرگر شرائط

- پیش نظارہ/پیداوار کے نمونے کریش ہو رہے ہیں (`npm run probe:portal -- --expect-release=...`)۔
- Grafana `torii_sorafs_gateway_refusals_total` پر انتباہات یا
  `torii_sorafs_manifest_submit_total{status="error"}` رول آؤٹ کے بعد۔
- کیو اے نے دستی طور پر ٹوٹے ہوئے راستوں یا پراکسی کی ناکامیوں کو نوٹس لیا ہے اس کے فورا بعد ہی اسے آزمائیں
  تشہیر عرف

### فوری طور پر کنٹینمنٹ

1. ** منجمد تعیناتی: ** مارک سی آئی پائپ لائن `DEPLOY_FREEZE=1` (ان پٹ گٹ ہب ورک فلو)
   یا جینکنز کی نوکری کو روکیں تاکہ نئی نمونے سامنے نہ آئیں۔
2. ** نمونے کو ٹھیک کریں: ** ڈاؤن لوڈ `build/checksums.sha256` ،
   `portal.manifest*.{json,to,bundle,sig}` ، اور ناکام تعمیر سے آؤٹ پٹ کی تحقیقات ،
   تو اس رول بیک سے مراد عین مطابق ہضم ہوتا ہے۔
3. ** اسٹیک ہولڈرز کو مطلع کریں: ** اسٹوریج ایس آر ای ، لیڈ دستاویزات/ڈیوریل اور گورننس ڈیوٹی آفیسر
   (خاص طور پر اگر `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. آخری معروف گوڈ (ایل کے جی) کے ظاہر کا تعین کریں۔ پروڈکشن ورک فلو ان میں ذخیرہ کرتا ہے
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`۔
2. شپنگ مددگار کا استعمال کرتے ہوئے عرف کو اس منشور پر دوبارہ بنائیں:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. ایل کے جی کے ڈائجسٹ اور ناکام مینی فیسٹ کے ساتھ واقعہ کے ٹکٹ میں ایک سمری رول بیک لکھیں۔

### توثیق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   ۔
4. `npm run probe:tryit-proxy` اس بات کو یقینی بنانے کے لئے کہ کوشش کی جانے والی پراکسی واپس آگئی ہے۔

واقعے کے بعد ###1. بنیادی وجہ کو سمجھنے کے بعد ہی تعیناتی پائپ لائن کو فعال کریں۔
2. [`devportal/deploy-guide`] (./deploy-guide) میں "سیکھا گیا سبق" سیکشن شامل کریں
   نئے نتائج ، اگر کوئی ہے۔
3. ناکام ٹیسٹ (تحقیقات ، لنک چیکر ، وغیرہ) کے لئے نقائص بنائیں۔

## رن بوک - نقل کی ہراس

### ٹرگر شرائط

- الرٹ: `رقم (torii_sorafs_replication_sla_total {نتیجہ =" میٹ "}) /
  کلیمپ_مین (رقم (torii_sorafs_replication_sla_total {نتیجہ = ~ "met | med | med"}) ، 1) <
  0.95` 10 منٹ کے لئے۔
- `torii_sorafs_replication_backlog_total > 10` 10 منٹ کے لئے (دیکھیں
  `pin-registry-ops.md`)۔
- گورننس کی رہائی کے بعد عرف کی سست دستیابی کی اطلاع ہے۔

### triage

1. ڈیش بورڈز چیک کریں [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops) to
   سمجھیں کہ آیا بیک بلاگ اسٹوریج کلاس میں یا بیڑے فراہم کرنے والوں میں مقامی ہے۔
2. Torii سے `sorafs_registry::submit_manifest` تک لاگ ان چیک کریں
   کیا گذارشات گر رہے ہیں؟
3. `sorafs_cli manifest status --manifest ...` کے ذریعے نقل کی صحت کو منتخب طور پر چیک کریں
   (فراہم کنندہ کے ذریعہ نقل کے نتائج کو ظاہر کرتا ہے)۔

### تخفیف

1. دوبارہ جاری کریں ایک اعلی تعداد میں نقل (`--pin-min-replicas 7`) کے ساتھ دوبارہ جاری کریں
   `scripts/sorafs-pin-release.sh` تاکہ شیڈولر ایک بڑے سیٹ پر بوجھ تقسیم کرے
   فراہم کرنے والے واقعے کے لاگ میں نیا ڈائجسٹ ریکارڈ کریں۔
2. اگر بیکلاگ ایک فراہم کنندہ کے ساتھ بندھا ہوا ہے تو ، اسے نقل کے شیڈولر کے ذریعہ عارضی طور پر غیر فعال کریں
   ۔
   فراہم کرنے والے عرف کو اپ ڈیٹ کرتے ہیں۔
3. جب عرف کی تازگی نقل کی برابری سے زیادہ اہم ہوتی ہے تو ، گرم منشور کے لئے ریبینڈ عرف پہلے ہی اسٹیجنگ میں ہے
   .

### بحالی اور بندش

1. `torii_sorafs_replication_sla_total{outcome="missed"}` کی نگرانی کریں اور اس بات کو یقینی بنائیں
   کاؤنٹر مستحکم ہوچکا ہے۔
2. آؤٹ پٹ `sorafs_cli manifest status` کو ثبوت کے طور پر محفوظ کریں کہ ہر نقل معمول پر ہے۔
3. مزید اقدامات کے ساتھ نقل کے بیک بلاگ کو پوسٹ مارٹم بنائیں یا اپ ڈیٹ کریں
   (فراہم کنندہ اسکیلنگ ، ٹیوننگ چنکر ، وغیرہ)۔

## رن بک - تجزیات یا ٹیلی میٹری کو غیر فعال کرنا

### ٹرگر شرائط

- `npm run probe:portal` پاس کرتا ہے ، لیکن ڈیش بورڈز واقعات وصول کرنا بند کردیتے ہیں
  `AnalyticsTracker` 15 منٹ سے زیادہ کے لئے۔
- رازداری کے جائزے میں گرائے ہوئے واقعات میں غیر متوقع اضافہ ریکارڈ کیا گیا ہے۔
- `npm run probe:tryit-proxy` پٹریوں پر فالس `/probe/analytics`۔

### جواب

1. بلڈ ٹائم ان پٹ چیک کریں: `DOCS_ANALYTICS_ENDPOINT` اور
   `DOCS_ANALYTICS_SAMPLE_RATE` میں ریلیز نمو (`build/release.json`)۔
2. `DOCS_ANALYTICS_ENDPOINT` کے ساتھ `npm run probe:portal` کو دوبارہ شروع کریں
   اسٹیجنگ کلیکٹر کو اس بات کی تصدیق کرنے کے لئے کہ ٹریکر پے لوڈ بھیجتا رہتا ہے۔
3. اگر جمع کرنے والے دستیاب نہیں ہیں تو ، `DOCS_ANALYTICS_ENDPOINT=""` انسٹال کریں اور دوبارہ تعمیر کریں ،
   ٹریکر شارٹ سرکٹ کو ؛ واقعے کی ٹائم لائن میں آؤٹ پٹ ونڈو پر قبضہ کریں۔
4. چیک کریں کہ `scripts/check-links.mjs` فنگر پرنٹ `checksums.sha256` جاری رکھتا ہے
   (تجزیات کی ناکامیوں * کو * سائٹ میپ کی توثیق کو روکنا نہیں چاہئے)۔
5. بازیافت کے بعد ، کلکٹر ڈرائیو کے لئے `npm run test:widgets` چلاتا ہے
   یونٹ ٹیسٹ تجزیاتی مددگار سے پہلے دوبارہ شائع کریں۔

واقعے کے بعد ###1. تازہ ترین [`devportal/observability`] (./observability) نئی پابندیوں کے ساتھ
   کلکٹر یا نمونے لینے کی ضروریات۔
2۔ گورننس کا نوٹس جاری کریں اگر تجزیات کا ڈیٹا کھو گیا ہے یا ترمیم کیا گیا ہے
   سیاست سے باہر

## سہ ماہی لچکدار مشقیں

دونوں مشقیں ** ہر سہ ماہی کے پہلے منگل پر چلائیں ** (جنوری/اپریل/جولائی/اکتوبر)
یا کسی بھی بڑے بنیادی ڈھانچے میں تبدیلی کے فورا. بعد۔ میں نمونے اسٹور کریں
`artifacts/devportal/drills/<YYYYMMDD>/`۔

| تعلیم | اقدامات | ثبوت |
| ----- | ----- | -------- |
| ریہرسل عرف رول بیک | 1۔ تازہ ترین پروڈکشن مینی فیسٹ کے ساتھ رول بیک "ناکام تعیناتی" کو دہرائیں۔  2۔ کامیاب تحقیقات کے بعد پیداوار کا دوبارہ پابند کریں۔  3۔ ڈرل فولڈر میں `portal.manifest.submit.summary.json` اور پروبس لاگز کو بچائیں۔ | `rollback.submit.json` ، آؤٹ پٹ تحقیقات اور ریلیز ٹیگ ریہرسل۔ |
| مصنوعی توثیق آڈٹ | 1. چلائیں `npm run probe:portal` اور `npm run probe:tryit-proxy` پیداوار اور اسٹیجنگ کے خلاف۔  2. `npm run check:links` اور آرکائیو `build/link-report.json` چلائیں۔  3۔ Grafana پینلز کی اسکرین شاٹس/برآمدات جو تحقیقات کی کامیابی کی تصدیق کرتے ہیں۔ | فنگر پرنٹ مینی فیسٹ کے لنک کے ساتھ لاگ ان پروبس + `link-report.json`۔ |

دستاویزات/ڈیورل منیجر اور جائزہ گورننس ایس آر ای کو کھو جانے والی مشقوں کو بڑھاوا دیں ،
چونکہ روڈ میپ کو عرف رول بیک میں عرفی سہ ماہی ثبوت کی ضرورت ہوتی ہے
اور پورٹل تحقیقات صحت مند ہیں۔

## پیجریڈی اور آن کال کوآرڈینیشن

- پیجریڈی ** دستاویزات پورٹل پبلشنگ ** سروس سے انتباہات کا مالک ہے
  `dashboards/grafana/docs_portal.json`۔ قواعد `DocsPortal/GatewayRefusals` ،
  `DocsPortal/AliasCache` اور `DocsPortal/TLSExpiry` صفحہ پرائمری دستاویزات/ڈیوریل
  سیکنڈری کے طور پر اسٹوریج SRE کے ساتھ.
- جب پیجنگ کریں تو ، `DOCS_RELEASE_TAG` کی نشاندہی کریں ، متاثرہ افراد کے اسکرین شاٹس منسلک کریں
  پینل Grafana اور اس سے پہلے واقعہ کے نوٹوں میں تحقیقات/لنک چیک آؤٹ پٹ شامل کریں
  تخفیف شروع کی۔
- تخفیف کے بعد (رول بیک یا دوبارہ تعی .ن) دوبارہ چلائیں `npm run probe:portal` ،
  `npm run check:links` اور تازہ Grafana اسنیپ شاٹس کو واپسی میٹرکس دکھا رہا ہے
  ریپڈس میں بند ہونے سے پہلے پیجریڈی واقعے سے تمام شواہد منسلک کریں۔
- اگر دو انتباہات بیک وقت متحرک ہوجاتے ہیں (مثال کے طور پر TLS کی میعاد ختم اور بیکلاگ) ، پہلے
  ٹریج انکار (اشاعت بند کرو) ، رول بیک ، پھر TLS/بیکلاگ بند کریں
  اسٹوریج SRE سے برج تک۔