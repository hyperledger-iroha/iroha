---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى المشغلون الخارجيون
نفس ارشادات SN-3b دون استنساخ المستودع.
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**مرجع خارطة الطريق:** SN-3b "Bulk onboarding tooling"  
**الاثار:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

غالبا ما يجهز المسجلون الكبار مئات تسجيلات `.sora` او `.nexus` مع نفس
موافقات الحوكمة وقنوات التسوية. صياغة payloads JSON يدويا او اعادة تشغيل
CLI لا يتوسع، لذا يقدم SN-3b builder حتمي من CSV الى Norito يحضر هياكل
`RegisterNameRequestV1` لـ Torii او الـ CLI. يتحقق المساعد من كل صف مسبقا،
ويصدر كلا من manifest مجمع و JSON اختياري مفصول باسطر، ويمكنه ارسال
payloads تلقائيا مع تسجيل ايصالات منظمة لاغراض التدقيق.

## 1. קובץ CSV

يتطلب المحلل صف العناوين التالي (الترتيب مرن):

| العمود | مطلوب | אוטו |
|--------|-------|-------|
| `label` | نعم | التسمية المطلوبة (يقبل حالة مختلطة; الاداة تطبع حسب Norm v1 و UTS-46). |
| `suffix_id` | نعم | معرف لاحقة رقمي (عشري او `0x` hex). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | عدد صحيح `1..=255`. |
| `payment_asset_id` | نعم | اصل التسوية (مثل `xor#sora`). |
| `payment_gross` / `payment_net` | نعم | اعداد صحيحة غير موقعة تمثل وحدات الاصل. |
| `settlement_tx` | نعم | قيمة JSON او سلسلة حرفية تصف معاملة الدفع او hash. |
| `payment_payer` | نعم | AccountId الذي فوض الدفع. |
| `payment_signature` | نعم | JSON او سلسلة حرفية تحتوي دليل توقيع steward او الخزينة. |
| `controllers` | اختياري | قائمة مفصولة بفاصلة او فاصلة منقوطة لعناوين حسابات controller. الافتراضي `[owner]` عند الحذف. |
| `metadata` | اختياري | JSON inline او `@path/to/file.json` يقدم تلميحات resolver وسجلات TXT وغيرها. الافتراضي `{}`. |
| `governance` | اختياري | JSON inline او `@path` يشير الى `GovernanceHookV1`. `--require-governance` يفرض هذا العمود. |

يمكن لاي عمود الاشارة الى ملف خارجي عبر بادئة قيمة الخلية بـ `@`.
يتم حل المسارات نسبة الى ملف CSV.

##

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

خيارات رئيسية:

- `--require-governance` يرفض الصفوف بدون hook حوكمة (مفيد لمزادات premium او
  التعيينات المحجوزة).
- `--default-controllers {owner,none}` يقرر ما اذا كانت خلايا controllers
  الفارغة تعود الى حساب owner.
- `--controllers-column`, `--metadata-column`, و `--governance-column` تسمح
  باعادة تسمية الاعمدة الاختيارية عند العمل مع exports خارجية.

عند النجاح يكتب السكربت manifest مجمع:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

اذا تم تمرير `--ndjson`، يكتب كل `RegisterNameRequestV1` ايضا كسطر JSON واحد
حتى تتمكن الاتمتة من بث الطلبات مباشرة الى Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. الارسال الالي

### 3.1 ו-Torii REST

حدد `--submit-torii-url` مع `--submit-token` او `--submit-token-file` لارسال كل
ادخال في manifest مباشرة الى Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- يصدر المساعد `POST /v2/sns/registrations` لكل طلب ويتوقف عند اول خطا HTTP.
  تضاف الردود الى مسار السجل كسجلات NDJSON.
- `--poll-status` يعيد الاستعلام عن `/v2/sns/registrations/{selector}` بعد كل
  ارسال (حتى `--poll-attempts`, الافتراضي 5) لتاكيد ظهور السجل. ופיר
  `--suffix-map` (JSON يحول `suffix_id` الى قيم "suffix") كي تتمكن الاداة من
  اشتقاق لواحق `{label}.{suffix}` عند polling.
- תקשורת: `--submit-timeout`, `--poll-attempts`, ו-`--poll-interval`.

### 3.2 ו-CLI

لتمرير كل ادخال في manifest عبر CLI، وفر مسار الملف التنفيذي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- يجب ان تكون controllers من نوع `Account` (`controller_type.kind = "Account"`)
  لان CLI حاليا لا يدعم سوى controllers المعتمدة على الحساب.
- تكتب blobs الخاصة بـ metadata و governance في ملفات مؤقتة لكل طلب ويتم
  تمريرها الى `iroha sns register --metadata-json ... --governance-json ...`.
- يتم تسجيل stdout و stderr مع اكواد الخروج؛ الاكواد غير الصفرية توقف التشغيل.

يمكن تشغيل وضعي الارسال معا (Torii و CLI) للتحقق المتقاطع من نشر المسجل او
لتمرين مسارات fallback.

### 3.3 ايصالات الارسال

عند تمرير `--submission-log <path>` يضيف السكربت سجلات NDJSON تلتقط:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تتضمن ردود Torii الناجحة حقولا منظمة مستخرجة من `NameRecordV1` او
`RegisterNameResponseV1` (מיטל `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) حتى تتمكن لوحات المتابعة وتقارير الحوكمة من تحليل السجل دون تفتيش
نص حر. ارفق هذا السجل مع تذكرة المسجل بجانب manifest لاثبات قابل لاعادة
الانتاج.

## 4. اتـمتة اصدار بوابة الوثائق

تستدعي مهام CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` الذي يلف
المساعد ويخزن الاثار تحت `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

السكربت:

1. يبني `registrations.manifest.json` و `registrations.ndjson` وينسخ CSV الاصلي
   الى مجلد الاصدار.
2. يرسل manifest عبر Torii و/او CLI (عند التهيئة)، ويكتب `submissions.log` مع
   الايصالات المنظمة اعلاه.
3. يصدر `summary.json` الذي يصف الاصدار (المسارات، عنوان Torii، مسار CLI،
   timestamp) لكي تتمكن اتـمتة البوابة من رفع الحزمة الى مخزن الاثار.
4. ينتج `metrics.prom` (override عبر `--metrics`) متضمنا عدادات متوافقة مع
   Prometheus לתקשורת טלפונית ותקשורת.
   يربط JSON الملخص بهذا الملف.

זרימות עבודה של זרימות עבודה דף הבית של זרימות עבודה
الاعتمادات للتدقيق.

## 5. القياس ولوحات المتابعة

يعرض ملف المقاييس الناتج عن `sns_bulk_release.sh` السلاسل التالية:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

قم بتغذية `metrics.prom` الى sidecar Prometheus لديك (مثلا عبر Promtail او
مستورد دفعات) للحفاظ على توافق المسجلين وstewards وشركاء الحوكمة حول تقدم
الجملة. لوحة Grafana `dashboards/grafana/sns_bulk_release.json` تعرض نفس
البيانات مع لوحات لعدد الطلبات لكل لاحقة، حجم الدفع، ونسب نجاح/فشل الارسال.
تقوم اللوحة بالتصفية عبر `release` حتى يتمكن المدققون من التعمق في تشغيل CSV
واحد.

## 6. التحقق وحالات الفشل- **توحيد label:** يتم تطبيع الادخالات باستخدام Python IDNA مع lowercase وفلاتر
  נורמה v1. تفشل التسميات غير الصالحة بسرعة قبل اي اتصال شبكي.
- **حواجز رقمية:** يجب ان تقع suffix ids و term years و pricing hints ضمن حدود
  `u16` ו-`u8`. تقبل حقول الدفع اعدادا عشرية او hex حتى `i64::MAX`.
- **تحليل metadata او governance:** يتم تحليل JSON inline مباشرة؛ ويتم حل
  مراجع الملفات نسبة الى موقع CSV. metadata غير الكائن ينتج خطا تحقق.
- **Controllers:** الخلايا الفارغة تلتزم بـ `--default-controllers`. قدم قوائم
  controller صريحة (مثل `i105...;i105...`) عند التفويض لجهات غير المالك.

يتم الابلاغ عن الاخطاء مع ارقام صفوف سياقية (مثلا
`error: row 12 term_years must be between 1 and 255`). يخرج السكربت بالكود `1`
عند اخطاء التحقق و `2` عندما يكون مسار CSV مفقودا.

## 7. الاختبار والاعتمادية

- يغطي `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` تحليل CSV،
  اصدار NDJSON، فرض الحوكمة، ومسارات ارسال CLI او Torii.
- المساعد مكتوب ببايثون فقط (بدون تبعيات اضافية) ويعمل حيث يتوفر `python3`.
  يتم تتبع سجل الالتزامات بجانب CLI في المستودع الرئيسي للموثوقية.

للانتاج، ارفق manifest الناتج وحزمة NDJSON مع تذكرة المسجل حتى يتمكن stewards
من اعادة تشغيل الـ payloads الدقيقة التي تم ارسالها الى Torii.