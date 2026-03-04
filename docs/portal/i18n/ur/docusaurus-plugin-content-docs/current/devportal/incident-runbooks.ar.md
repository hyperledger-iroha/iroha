---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# واقعہ کے دستورالعمل اور رول بیک مشقیں

## مقصد

روڈ میپ آئٹم ** دستاویزات۔
بغیر کسی اندازہ کے تعیناتی کی ناکامیوں سے بازیافت کریں۔ اس نوٹ میں تین ہائی پروفائل واقعات-ڈیلیپمنٹ کی ناکامی کا احاطہ کیا گیا ہے ،
کاپی ہراس ، تجزیات کی بندش - دستاویزی اور سہ ماہی مشقیں یہ ثابت کرتی ہیں کہ عرف کا رول بیک
مصنوعی توثیق ابھی بھی ختم ہونے کے لئے کام کرتی ہے۔

### متعلقہ مواد

-.
-.
-`docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops)
  - ٹیلی میٹیا لاگ اور بڑھتی ہوئی حدود۔
- `docs/portal/scripts/sorafs-pin-release.sh` اور `npm run probe:*` مددگار
  چیک لسٹس کے ذریعہ حوالہ دیا گیا۔

### ٹیلی میٹری اور عام ٹولز

| سگنل/ٹول | مقصد |
| --------------- | ------- |
| `torii_sorafs_replication_sla_total` (میٹ/مس/زیر التواء) | کاپی اسٹاپس اور ایس ایل اے کی خلاف ورزیوں کا پتہ لگاتا ہے۔ |
| `torii_sorafs_replication_backlog_total` ، `torii_sorafs_replication_completion_latency_epochs` | ٹریج مقاصد کے لئے بیک بلاگ کی گہرائی اور تکمیل کے وقت کی پیمائش کرتا ہے۔ |
| `torii_sorafs_gateway_refusals_total` ، `torii_sorafs_manifest_submit_total{status="error"}` | گیٹ وے کی ناکامیوں کی وضاحت کرتا ہے جو اکثر خراب تعیناتی کی پیروی کرتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | مصنوعی تحقیقات گیٹ ریلیز اور چیک رول بیکس۔ |
| `npm run check:links` | ٹوٹے ہوئے لنکس پورٹل ؛ ہر تخفیف کے بعد استعمال کریں۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے encapsulated) | عرف کو اپ گریڈ/بحال کرنے کا طریقہ کار۔ |
| `Docs Portal Publishing` Grafana بورڈ (`dashboards/grafana/docs_portal.json`) | ٹیلی میٹیا ریئیکشنز/عرف/ٹی ایل ایس/نقل کا تالاب۔ پیجریڈی الرٹس ان پینلز کی طرف ثبوت کے طور پر اشارہ کرتے ہیں۔ |

## رن بک - تعیناتی ناکام یا خراب نوادرات

### لانچ کی شرائط

- پیش نظارہ/پیداوار (`npm run probe:portal -- --expect-release=...`) کے لئے ناکام تحقیقات۔
- Grafana الرٹس `torii_sorafs_gateway_refusals_total` پر یا
  `torii_sorafs_manifest_submit_total{status="error"}` رول آؤٹ کے بعد۔
- دستی QA نوٹس ٹوٹے ہوئے راستے یا پراکسی کی ناکامی عرف اپ گریڈ کے فورا. بعد اسے آزمائیں۔

### فوری طور پر کنٹینمنٹ

1. ** منجمد تعیناتی: ** CI میں پائپ لائن پر `DEPLOY_FREEZE=1` ڈالیں (گٹ ہب میں ورک فلو کے لئے ان پٹ)
   یا جینکنز کی نوکری کو روکیں تاکہ کوئی اضافی نوادرات آؤٹ پٹ نہ ہوں۔
2.
   `portal.manifest*.{json,to,bundle,sig}` ، اور جب تک رول بیک کی نشاندہی نہیں کی جاتی ہے تب تک ناکام تعمیر سے تحقیقات
   منٹ ہضم کرنے کے لئے.
3.
   (خاص طور پر جب `docs.sora` سے متاثر ہو)۔

### ایک رول بیک انجام دیں

1. آخری مظہر کا تعین کریں جو اچھے (ایل کے جی) کے نام سے جانا جاتا ہے۔ پروڈکشن ورک فلو اس میں ذخیرہ کرتا ہے
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`۔
2. شپنگ ہیلپر کے ذریعہ اس مینی فیسٹ کے ساتھ دوبارہ وابستہ عرف:

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

3. ایل کے جی مینی فیسٹ اور ناکام مینی فیسٹ کے لئے ڈائجسٹس کے ساتھ واقعہ کے ٹکٹ میں رول بیک کا خلاصہ ریکارڈ کریں۔

### توثیق

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`۔
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` اور `sorafs_cli proof verify ...`
   ۔
4. `npm run probe:tryit-proxy` اس بات کو یقینی بنانے کے لئے کہ کوشش کی جانے والی پراکسی بیک اپ اور چل رہی ہے۔

واقعے کے بعد ###

1. بنیادی وجہ کو سمجھنے کے بعد ہی تعیناتی پائپ لائن کو دوبارہ متحرک کریں۔
2. [`devportal/deploy-guide`] (./deploy-guide) میں "اسباق سیکھے گئے" سیکشن کو اپ ڈیٹ کریں
   ضرورت پڑنے پر نئے نوٹ شامل کریں۔
3. ناکام ٹیسٹ (تحقیقات ، لنک چیکر ، وغیرہ) کے لئے کھلے نقائص۔

## رن بک - ٹرانسکرپٹ انحطاط

### لانچ کی شرائط- الرٹ: `رقم (torii_sorafs_replication_sla_total {نتیجہ =" میٹ "})/
  کلیمپ_مین (رقم (torii_sorafs_replication_sla_total {نتیجہ = ~ "met | med | med"}) ، 1) <
  0.95` 10 منٹ کے لئے۔
- `torii_sorafs_replication_backlog_total > 10` 10 منٹ کے لئے (دیکھیں
  `pin-registry-ops.md`)۔
- گورننس کی رہائی کے بعد عرف کی سست دستیابی کی اطلاع ہے۔

### triage

1. ڈیش بورڈز چیک کریں [`sorafs/pin-registry-ops`] (../sorafs/pin-registry-ops) اس بات کا تعین کرنے کے لئے
   بیک بلاگ اسٹوریج یا بیڑے فراہم کرنے والوں کے زمرے تک محدود ہے۔
2. `sorafs_registry::submit_manifest` کے لئے Torii رجسٹر چیک کریں
   گذارشات ناکام ہوجاتی ہیں۔
3. `sorafs_cli manifest status --manifest ...` (ہر فراہم کنندہ کے لئے نتائج دکھاتا ہے) کے ذریعے کاپی کی صداقت کو چیک کریں۔

### تخفیف

1. دوبارہ ریلیز کریں ایک اعلی تعداد میں کاپیاں (`--pin-min-replicas 7`) کے ساتھ ظاہر کریں
   `scripts/sorafs-pin-release.sh` شیڈولر بڑی تعداد میں فراہم کنندگان کی بڑی تعداد میں بوجھ تقسیم کرتا ہے۔
   واقعے کے لاگ میں نیا ڈائجسٹ ریکارڈ کریں۔
2. اگر بیکلاگ ایک فراہم کنندہ کے ساتھ بندھا ہوا ہے تو ، اسے نقل کے شیڈولر کے ذریعہ عارضی طور پر غیر فعال کریں
   ۔
3. جب عرف کی تازگی کاپی کی برابری سے زیادہ اہم ہوتی ہے تو ، عرف کو ایک گرم منشور کی طرف راغب کریں
   اسٹیج (`docs-preview`) ، پھر SRE کے بیک بلاگ کو صاف کرنے کے بعد فالو اپ مینی فیسٹ تعینات کریں۔

### بازیافت اور شٹ ڈاؤن

1. مانیٹر `torii_sorafs_replication_sla_total{outcome="missed"}` کو یقینی بنانے کے لئے گنتی مستحکم ہے۔
2. `sorafs_cli manifest status` کی پیداوار کو اس بات کے ثبوت کے طور پر گرفت میں لیں کہ ہر نقل تعمیل میں واپس آچکی ہے۔
3. مندرجہ ذیل مراحل کے ساتھ بیک بلاگ کے لئے پوسٹ مارٹم بنائیں یا اپ ڈیٹ کریں
   (فراہم کنندگان کو وسعت دیں ، چنکر کو ایڈجسٹ کریں ، وغیرہ)۔

## رن بک - تجزیات یا ٹیلی میٹری میں رکاوٹ

### لانچ کی شرائط

- `npm run probe:portal` کامیاب ہوتا ہے لیکن ڈیش بورڈز واقعات کو نگلنے سے باز آتے ہیں
  `AnalyticsTracker` 15 منٹ سے زیادہ کے لئے۔
- رازداری کے جائزے میں گرائے ہوئے واقعات میں غیر متوقع اضافے کا پتہ چلتا ہے۔
- `npm run probe:tryit-proxy` `/probe/analytics` راستوں پر ناکام ہے۔

### جواب

1. تعمیر کے وقت کے آدانوں کو چیک کریں: `DOCS_ANALYTICS_ENDPOINT`
   `DOCS_ANALYTICS_SAMPLE_RATE` میں آرٹ فیکٹ ورژن (`build/release.json`)۔
2. `DOCS_ANALYTICS_ENDPOINT` کے ساتھ `npm run probe:portal` کی طرف اشارہ کیا
   اس بات کی تصدیق کرنے کے لئے کہ ٹریکر ابھی بھی پے لوڈ بھیج رہا ہے۔
3. اگر جمع کرنے والوں کو روکا جاتا ہے تو ، `DOCS_ANALYTICS_ENDPOINT=""` سیٹ کریں اور دوبارہ تعمیر کریں
   ٹریکر کو شارٹ سرکٹ کرنے کے لئے ؛ واقعے کی ٹائم لائن میں آؤٹ پٹ ونڈو کو ریکارڈ کریں۔
4. تصدیق کریں کہ `scripts/check-links.mjs` ابھی بھی فنگر پرنٹنگ `checksums.sha256` ہے
   (تجزیات کی مداخلتوں کو * سائٹ کا نقشہ کی توثیق کی روک تھام نہیں کرنا چاہئے۔)
5. کلکٹر کی بازیافت کے بعد ، تجزیاتی مددگار کے لئے یونٹ ٹیسٹ چلانے کے لئے `npm run test:widgets` چلائیں
   دوبارہ شائع کرنے سے پہلے۔

واقعے کے بعد ###

1. اپ ڈیٹ [`devportal/observability`] (./observability) جمع کرنے والے کے لئے کسی بھی نئی پابندی کے ساتھ یا
   نمونے لینے کی ضروریات۔
2. گورننس کی اطلاع جاری کریں اگر تجزیات کا ڈیٹا کھو گیا ہے یا پالیسی سے باہر اس پر نظر ثانی کی گئی ہے۔

## سہ ماہی لچکدار مشقیں

دونوں مشقیں ** ہر سہ ماہی کے پہلے منگل کے دوران کریں ** (جنوری/اپریل/جولائی/اکتوبر)
یا کسی بھی بڑے بنیادی ڈھانچے میں تبدیلی کے فورا. بعد۔ اس کے تحت نوادرات کو اسٹور کریں
`artifacts/devportal/drills/<YYYYMMDD>/`۔| ورزش | اقدامات | گائیڈ |
| ----- | ----- | -------- |
| عرف کے لئے رول بیک ورزش | 1. جدید ترین پروڈکشن مینی فیسٹ کا استعمال کرتے ہوئے "ناکام تعیناتی" رول بیک کو دوبارہ شروع کریں۔  2۔ کامیاب تحقیقات کے بعد پیداوار سے دوبارہ رابطہ کریں۔  3۔ ورزش فولڈر میں `portal.manifest.submit.summary.json` اور لاگ ان پروبس کو رجسٹر کریں۔ | `rollback.submit.json` ، آؤٹ پٹ تحقیقات ، اور مشق کے لئے ریلیز ٹیگ۔ |
| مصنوعی توثیق آڈٹ | 1. آؤٹ پٹ اور اسٹیجنگ کے خلاف `npm run probe:portal` اور `npm run probe:tryit-proxy` کو آن کریں۔  2۔ `npm run check:links` اور آرکائیو `build/link-report.json` چلائیں۔  3۔ کامیاب تحقیقات کی تصدیق کے ل I Grafana بورڈز کی اسکرین شاٹس/برآمدات سے منسلک ہونا۔ | پروبس + `link-report.json` ریکارڈز مینی فیسٹ کے فنگر پرنٹ کی نشاندہی کرتے ہیں۔ |

دستاویزات/ڈیورل منیجر کو کھو جانے والی مشقوں کو بڑھاوا دیں اور ایس آر ای گورننس کا جائزہ لیں ، جیسا کہ روڈ میپ کی ضرورت ہے
ناگزیر سہ ماہی ثبوت کہ عرف اور تحقیقات پورٹل کا رول بیک برقرار ہے۔

## پیجریڈی اور آن کال فارمیٹنگ

- پیجریڈی سروس ** دستاویزات پورٹل پبلشنگ ** کے ذریعہ تیار کردہ انتباہات کا مالک ہے
  `dashboards/grafana/docs_portal.json`۔ قواعد `DocsPortal/GatewayRefusals` ،
  `DocsPortal/AliasCache` ، اور `DocsPortal/TLSExpiry` دستاویزات/deverl کے لئے پیجنگ کر رہے ہیں
  بیک اپ کے طور پر اسٹوریج SRE کے ساتھ اہم.
- جب کہا جاتا ہے تو ، `DOCS_RELEASE_TAG` منسلک کریں ، اور متاثرہ Grafana بورڈز کے اسکرین شاٹس منسلک کریں ،
  اور تخفیف شروع کرنے سے پہلے واقعات کے نوٹوں میں تحقیقات/لنک چیک آؤٹ پٹس کو لنک کریں۔
- تخفیف کے بعد (رول بیک یا دوبارہ تعی .ن) ، ریبوٹ `npm run probe:portal` ،
  `npm run check:links` ، اور Grafana کے نئے اسنیپ شاٹس بیک میٹرکس دکھا رہے ہیں
  دہلیز کے اندر پیجریڈی کے واقعے کو بند کرنے سے پہلے اس کے تمام شواہد منسلک کریں۔
- اگر ایک ہی وقت میں دو الرٹس کو متحرک کیا جاتا ہے (جیسے TLS بیکلاگ کے ساتھ میعاد ختم ہوجاتا ہے) ، تو پہلے رد ections نٹ سے نمٹیں
  (اشاعت بند کرو) ، ایک رول بیک انجام دیں ، پھر پل پر اسٹوریج ایس آر ای کے ساتھ ٹی ایل ایس/بیکلاگ پر کارروائی کریں۔