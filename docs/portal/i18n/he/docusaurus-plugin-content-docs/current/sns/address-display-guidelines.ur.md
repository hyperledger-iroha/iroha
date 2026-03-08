---
lang: he
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

יבא את ExplorerAddressCard מ-'@site/src/components/ExplorerAddressCard';

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/address_display_guidelines.md` کی عکاسی کرتا ہے اور اب
پورٹل کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

والٹس، ایکسپلوررز اور SDK مثالیں اکاؤنٹ ایڈریسز کو غیر متبدل payload سمجھیں۔
Android کی ریٹیل والٹ مثال
`examples/android/retail-wallet` תכנים של UX:

- **دو کاپی اہداف۔** دو واضح کاپی بٹنز دیں: IH58 (ترجیحی) اور صرف Sora والا
  کمپریسڈ فارم (`sora...`، second‑best). IH58 ہمیشہ بیرونی شیئرنگ کے لئے محفوظ ہے اور QR
  payload بناتا ہے۔ کمپریسڈ فارم میں inline وارننگ لازمی ہے کیونکہ یہ صرف
  Sora-aware ایپس میں کام کرتا ہے۔ Android مثال دونوں Material بٹنز اور ٹول ٹپس
  תמונה `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`
  אודם של iOS SwiftUI דגם `examples/ios/NoritoDemo/Sources/ContentView.swift`
  کے اندر `AddressPreviewCard` کے ذریعے یہی UX دہراتا ہے۔
- **Monospace، قابل انتخاب متن۔** دونوں strings کو monospace فونٹ اور
  `textIsSelectable="true"` کے ساتھ رینڈر کریں تاکہ صارفین IME کے بغیر اقدار
  دیکھ سکیں۔ قابلِ ترمیم فیلڈز سے بچیں: IME kana کو بدل سکتا ہے یا zero-width
  کوڈ پوائنٹس داخل کر سکتا ہے۔
- **غیر ضمنی ڈیفالٹ ڈومین کے اشارے۔** جب selector ضمنی `default` ڈومین کی طرف
  ہو تو ایک caption دکھائیں جو آپریٹرز کو یاد دلائے کہ suffix درکار نہیں۔
  הבורר הקנוני של סרגל בורר
  digest encode کرے۔
- **IH58 QR payloads۔** QR کوڈز کو IH58 string encode کرنا چاہیے۔ אגר QR
  generation ناکام ہو تو خالی تصویر کے بجائے واضح error دکھائیں۔
- **کلپ بورڈ پیغام۔** کمپریسڈ فارم کاپی کرنے کے بعد toast یا snackbar دکھائیں
  جو صارفین کو یاد دلائے کہ یہ صرف Sora ہے اور IME سے خراب ہو سکتا ہے۔

ان گارڈ ریلز پر عمل Unicode/IME corruption روکتا ہے اور والٹ/ایکسپلورر UX کے لئے
ADDR-6 roadmap acceptance معیار پورا کرتا ہے۔

## اسکرین شاٹ فکسچرز

لوکلائزیشن ریویوز کے دوران درج ذیل فکسچرز استعمال کریں تاکہ بٹن لیبلز، ٹول ٹپس
اور وارننگز پلیٹ فارمز کے درمیان ہم آہنگ رہیں:

- גרסה אנדרואידית: `/img/sns/address_copy_android.svg`

  ![Android ڈوئل کاپی ریفرنس](/img/sns/address_copy_android.svg)

- iOS גרסה: `/img/sns/address_copy_ios.svg`

  ![iOS ڈوئل کاپی ریفرنس](/img/sns/address_copy_ios.svg)

## עוזרי SDK

ہر SDK ایک سہولت helper فراہم کرتا ہے جو IH58 اور کمپریسڈ فارم کے ساتھ وارننگ
string دیتا ہے تاکہ UI لیئرز مستقل رہیں:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` کمپریسڈ وارننگ string لوٹاتا ہے
  اور اسے `warnings` میں شامل کرتا ہے جب کالرز `sora...` literal دیں، تاکہ والٹ/
  ایکسپلورر ڈیش بورڈز paste/validation فلو میں Sora-only وارننگ دکھا سکیں، نہ کہ
  صرف تب جب وہ کمپریسڈ فارم خود بنائیں۔
- פייתון: `AccountAddress.display_formats(network_prefix: int = 753)`
- סוויפט: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

ان helpers کو استعمال کریں، UI لیئر میں encode لاجک دوبارہ مت لکھیں۔ JavaScript
עוזר `domainSummary` מטען `selector` (`tag`, `digest_hex`, `registry_id`,
`label`) גירסת משתמשי ממשק משתמש 2000000039 גירסת בורר מקומית 12
سے بیکڈ ہے، بغیر raw payload دوبارہ parse کیے۔

## ایکسپلورر instrumentation ڈیمو

<ExplorerAddressCard />ایکسپلوررز کو والٹ کی telemetry اور accessibility کے کام کو mirror کرنا چاہیے:

- גרסה של `data-copy-mode="ih58|compressed (`sora`)|qr"` תקליטורים של Torii
  میٹرک `torii_address_format_total` کے ساتھ usage counters نکال سکیں۔ اوپر والا
  ڈیمو کمپوننٹ `{mode,timestamp}` کے ساتھ `iroha:address-copy` ایونٹ بھیجتا ہے؛
  اسے اپنے analytics/telemetry pipeline (مثلاً Segment یا NORITO-backed collector)
  سے جوڑیں تاکہ dashboards سرور سائیڈ address format استعمال اور کلائنٹ کاپی
  موڈز کو correlate کر سکیں۔ מוני תחום Torii
  (`torii_address_domain_total{domain_kind}`) תמונה מקומית 12
  ریٹائرمنٹ ریویوز `address_ingest` Grafana بورڈ سے براہ راست 30 دن کا ثبوت
  `domain_kind="local12"`‏
- ہر کنٹرول کے لئے الگ `aria-label`/`aria-describedby` ہنٹس دیں جو بتائیں کہ
  literal شیئر کرنے کے لئے محفوظ ہے (IH58) یا صرف Sora (کمپریسڈ)۔ ضمنی ڈومین
  caption کو description میں شامل کریں تاکہ assistive technology وہی سیاق دکھائے
  جو بصری طور پر نظر آتا ہے۔
- ایک live region (مثلاً `<output aria-live="polite">...</output>`) رکھیں جو کاپی
  نتائج اور وارننگز اعلان کرے، Swift/Android نمونوں میں موجود VoiceOver/TalkBack
  رویے کے مطابق۔

یہ instrumentation ADDR-6b پوری کرتی ہے کیونکہ یہ دکھاتی ہے کہ آپریٹرز Local
selectors کے غیر فعال ہونے سے پہلے Torii ingestion اور client-side copy modes
دونوں کا مشاہدہ کر سکتے ہیں۔

## מקומי -> ערכת כלים להגירה גלובלית

Local selectors کی audit اور conversion خودکار ہو۔ helper JSON audit رپورٹ اور
IH58/کمپریسڈ لسٹ دونوں بناتا ہے جنہیں آپریٹرز readiness tickets کے ساتھ منسلک
کرتے ہیں، جبکہ متعلقہ runbook Grafana dashboards اور Alertmanager قواعد کو لنک
کرتا ہے جو strict-mode cutover کو gate کرتے ہیں۔

## بائنری layout کا فوری حوالہ (ADDR-1a)

כלי כתובות מתקדמים של SDK (פקחים, רמזי אימות, בוני מניפסט)
دکھائیں تو developers کو `docs/account_structure.md` میں موجود canonical wire
فارمیٹ کی طرف بھیجیں۔ layout ہمیشہ `header · selector · controller` ہوتا ہے، جہاں
header bits یہ ہیں:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) موجودہ ہے؛ לא אפס
  `AccountAddressError::InvalidHeaderVersion` אופי
- `addr_class` single (`0`) اور multisig (`1`) controllers میں فرق بتاتا ہے۔
- `norm_version = 1` Norm v1 selector قواعد encode کرتا ہے؛ مستقبل کے norms اسی
  2-bit فیلڈ کو دوبارہ استعمال کریں گے۔
- `ext_flag` ہمیشہ `0` ہے؛ فعال bits غیر معاون payload extensions دکھاتے ہیں۔

selector فوراً header کے بعد آتا ہے:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs اور SDKs کو selector کی قسم دکھانے کے لئے تیار ہونا چاہیے:

- `0x00` = ضمنی ڈیفالٹ ڈومین (کوئی payload نہیں)۔
- `0x01` = لوکل digest (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = گلوبل رجسٹری انٹری (`registry_id:u32` big-endian).

יש להטמיע מסמכים/בדיקות של מסמכים/בדיקות.

| Selector قسم | קנונית hex |
|--------------|--------------|
| ضمنی ڈیفالٹ | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| لوکل digest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| گلوبل رجسٹری (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |مکمل selector/state ٹیبل کے لئے `docs/source/references/address_norm_v1.md` اور
مکمل byte diagram کے لئے `docs/account_structure.md` دیکھیں۔

## Canonical forms نافذ کرنا

זרימת עבודה של זרימת עבודה של CLI עם ADDR-5:

1. `iroha tools address inspect` اب IH58، کمپریسڈ، اور canonical hex payloads کے ساتھ
   structured JSON summary دیتا ہے۔ summary میں `kind`/`warning` والے `domain`
   آبجیکٹ بھی ہوتے ہیں اور `input_domain` کے ذریعے دیے گئے ڈومین کو بھی echo
   کرتا ہے۔ جب `kind` `local12` ہو تو CLI stderr پر وارننگ دیتا ہے اور JSON
   summary وہی رہنمائی دہراتا ہے تاکہ CI pipelines اور SDKs اسے surface کر سکیں۔
   جب بھی آپ convert شدہ encoding کو `<ih58>@<domain>` کی صورت میں replay کرنا
   چاہیں تو `legacy  suffix` دیں۔
2. SDKs اسی وارننگ/summary کو JavaScript helper کے ذریعے دکھا سکتے ہیں:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed (`sora`));
   ```
  helper literal سے detect کیا گیا IH58 prefix محفوظ رکھتا ہے جب تک آپ
  `networkPrefix` واضح طور پر فراہم نہ کریں؛ اس لئے non-default networks کے
  summaries خاموشی سے default prefix کے ساتھ دوبارہ render نہیں ہوتے۔

3. canonical payload کو `ih58.value` یا `compressed (`sora`)` فیلڈز سے reuse کر کے تبدیل
   کریں (یا `--format` کے ذریعے دوسری encoding مانگیں)۔ یہ strings پہلے سے
   بیرونی شیئرنگ کے لئے محفوظ ہیں۔
4. manifests، registries اور customer-facing docs کو canonical فارم سے اپ ڈیٹ
   کریں اور فریقین کو مطلع کریں کہ cutover مکمل ہونے پر Local selectors ریجیکٹ
   ‏
5. بلک ڈیٹا سیٹس کے لئے
   `iroha tools address audit --input addresses.txt --network-prefix 753`. کمانڈ
   newline-separated literals پڑھتی ہے ( `#` سے شروع ہونے والے comments نظرانداز
   מספר טלפונים של `--input -` מספר טלפונים ו-STDIN טלפונים ניידים, 6
   اندراج کے لئے canonical/IH58 (ترجیحی)/compressed (`sora`) (`sora`, second-best) summaries کے ساتھ JSON رپورٹ بناتی
   rows ہوں تو `--allow-errors` استعمال کریں، اور جب آپریٹرز Local selectors کو
   CI میں بلاک کرنے کے لئے تیار ہوں تو `strict CI post-check` سے آٹومیشن گیٹ کریں۔
6. اگر newline-to-newline rewrite چاہیے تو
  Local-selector remediation spreadsheets کے لئے
  תוכנות תקינות `input,status,format,...` CSV או קידודים קנוניים,
  warnings اور parse failures کو ایک پاس میں نمایاں کرے۔ helper ڈیفالٹ طور پر
  שורות לא מקומיות.
  میں بدلتا ہے، اور `legacy  suffix` پر اصل ڈومین محفوظ رکھتا ہے۔ `--allow-errors`
  کے ساتھ جوڑیں تاکہ خراب literals والے dumps پر بھی scan جاری رہے۔
7. CI/lint automation `ci/check_address_normalize.sh` چلا سکتی ہے، جو
   `fixtures/account/address_vectors.json` سے Local selectors نکال کر
   `iroha tools address normalize` سے تبدیل کرتی ہے، اور
   `iroha tools address audit` دوبارہ چلاتی ہے تاکہ ثابت ہو کہ
   releases اب Local digests نہیں نکالتے۔`torii_address_local8_total{endpoint}` کے ساتھ
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
לוח `torii_address_collision_domain_total{endpoint,domain}`, אוור Grafana
אות אכיפה `dashboards/grafana/address_ingest.json`
production dashboards مسلسل 30 دن تک صفر legit Local submissions اور صفر
Local-12 collisions دکھائیں تو Torii Local-8 gate کو mainnet پر hard-fail کرے
گا، پھر Local-12 کو اس وقت جب global domains میں matching registry entries ہوں۔
اس freeze کے لئے CLI output کو operator-facing نوٹس سمجھیں - وہی warning string
SDK tooltips اور automation میں استعمال ہوتی ہے تاکہ roadmap exit criteria سے
20 regressions diagnose کرتے وقت صرف dev/test clusters میں اسے `false` کریں۔
`torii_address_domain_total{domain_kind}` או Grafana (`dashboards/grafana/address_ingest.json`)
میں mirror کرتے رہیں تاکہ ADDR-7 evidence pack یہ ثابت کر سکے کہ
selectors کو disable کرے۔ חבילת Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) تین guardrails شامل کرتا ہے:

- `AddressLocal8Resurgence` اس وقت page کرتا ہے جب کوئی context نیا Local-8
  increment رپورٹ کرے۔ strict-mode rollouts روکیں، dashboard میں offending SDK
  کریں جب تک signal صفر نہ ہو جائے، پھر default (`true`) بحال کریں۔
- `AddressLocal12Collision` تب فائر ہوتا ہے جب دو Local-12 labels ایک ہی digest
  پر hash ہوں۔ manifest promotions روکیں، Local -> Global toolkit چلا کر digest
  mapping آڈٹ کریں، اور Nexus governance کے ساتھ coordinate کریں اس سے پہلے کہ
  registry entry دوبارہ جاری ہو یا downstream rollouts بحال ہوں۔
- `AddressInvalidRatioSlo` خبردار کرتا ہے جب fleet-wide invalid ratio (Local-8/
  strict-mode rejections کے بغیر) 10 منٹ تک 0.1% SLO سے بڑھ جائے۔
  `torii_address_invalid_total` استعمال کر کے متعلقہ context/reason کی نشاندہی
  کریں اور owning SDK ٹیم کے ساتھ coordinate کر کے strict mode دوبارہ فعال کریں۔

### ریلیز نوٹ اسنیپٹ (والٹ اور ایکسپلورر)

cutover کے وقت والٹ/ایکسپلورر ریلیز نوٹس میں درج ذیل bullet شامل کریں:

> **Addresses:** `iroha tools address normalize` helper شامل
> کیا گیا اور اسے CI (`ci/check_address_normalize.sh`) میں وائر کیا گیا تاکہ
> میں تبدیل کر سکیں، قبل اس کے کہ Local-8/Local-12 mainnet پر بلاک ہوں۔ کسی بھی
> custom exports کو اپ ڈیٹ کریں تاکہ کمانڈ چلائی جائے اور normalized list کو
> release evidence bundle کے ساتھ منسلک کریں۔