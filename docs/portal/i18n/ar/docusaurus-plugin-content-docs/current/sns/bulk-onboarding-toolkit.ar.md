---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى التعديلات الخارجية
نفس ارشادات SN-3b دون استنساخ المستودع.
:::

# عدة ادوات التهيئة الأساسية لـ SNS (SN-3b)

**مرجع خارطة الطريق:** SN-3b "أدوات الإعداد المجمعة"  
**الاثار:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

غالبا ما يجهز المسجل للبرامج التعليمية للبالغين `.sora` او `.nexus` مع نفس
موافقات التوريد و القنوات. تجهيز الحمولات النافعة JSONيداً او إعادة البناء
CLI لا يتوسع، لذا يقدم SN-3b builder حتمي من CSV إلى Norito هياكل
`RegisterNameRequestV1` لـ Torii او الـ CLI. وجعل المساعدة من كل صف مسبقاً،
ويصدر كلا من المانيفست مجمع و JSON اختياري مفصول باسطر، ويمكنه ارسال
الحمولات بشكل خارجي مع تسجيل ايصالات منظمة لا لأغراض التدقيق.

## 1. مخطط CSV

يجب أن يتطلب اللون البني التالي (التكوين مرن):| اليمن | مطلوب | الوصف |
|--------|-------|-------|
| `label` | نعم | التسمية المطلوبة (يقبل حالة التباين؛ الاداءة تطبع حسب Norm v1 و UTS-46). |
| `suffix_id` | نعم | معرف لاحق رئيسي (عشري او `0x` hex). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | العدد صحيح `1..=255`. |
| `payment_asset_id` | نعم | اصل التوريد (مثل `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | الرقم الصحيح غير الموقع يمثل الوحدات الاصلية. |
| `settlement_tx` | نعم | قيمة JSON او سلسلة حرفية تصفيه احجز الدفع او التجزئة. |
| `payment_payer` | نعم | AccountId الذي فوض الدفع. |
| `payment_signature` | نعم | JSON او سلسلة حرفية تحتوي على دليل توقيع او الخزينة. |
| `controllers` | اختياري | قائمة فصلة بفاصلة او فاصلة منقوطة لناوين حسابات المراقب المالي. الافتراضي `[owner]` عند الحذف. |
| `metadata` | اختياري | JSON inline او `@path/to/file.json` يقدم تلميحات محلل TXT وغيرها. افتراضي `{}`. |
| `governance` | اختياري | JSON inline او `@path` يشير الى `GovernanceHookV1`. `--require-governance` يفرض هذا العمود. |

يمكن لاياقة الاشارة الى ملف خارجي عبر نوعية الخلايا الحيوية بـ `@`.
يتم حل المسارات بنسبة الى ملف CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

الخيارات:- `--require-governance` رفض الصفوف بدون ربط (مفيد لمزادات premium او
  للآيات المحجوزة).
- `--default-controllers {owner,none}` تقرر ما اذا كانت خلايا المتحكمات
  الفارغة تعود الى حساب المالك.
- `--controllers-column`، `--metadata-column`، و `--governance-column` تسمح
  باعادة التسمية الاعمدة الاختيارية عند العمل مع الصادرات.

عند النجاح كتب السكربت مجمع:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
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

اذا تمنيت `--ndjson`، كاتب كل `RegisterNameRequestV1` ايضا كطر JSON واحد
حتى تبدأ الامتعة من الطلبات مباشرة الى Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. الارسال الالي

### 3.1 وضع Torii REST

حدد `--submit-torii-url` مع `--submit-token` او `--submit-token-file` لارسال كل
أدخل في البيان مباشرة إلى Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- يصدر المساعد `POST /v1/sns/names` لكل طلب ويتوقف عند اول خطا HTTP.
  تم اضافة الردود الى مسار التسجيل كسجلات NDJSON.
- `--poll-status` أعيد عن `/v1/sns/names/{namespace}/{literal}` بعد كل
  إرسال (حتى `--poll-attempts`, الافتراضي 5) لتسجيل سجل تاكيد. وفر
  `--suffix-map` (JSON تحويل `suffix_id` الى قيم "suffix") كي جديد الااداة من
  اشتقاق لواحق `{label}.{suffix}` عند الاقتراع.
- إعدادات قابلة للضبط: `--submit-timeout`، `--poll-attempts`، و `--poll-interval`.

### 3.2 وضع iroha CLI

لتمرير كل إدخال في البيان عبر CLI، ويتوفر مسار الملف التنفيذي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```- يجب ان تكون وحدات تحكم من النوع `Account` (`controller_type.kind = "Account"`)
  لان CLI حاليا لا يدعم سوى وحدات تحكم معتمدة على الحساب.
- اكتب النقط الخاصة بـ البيانات الوصفية والحوكمة في ملفات محددة لكل طلب
  تسارعها الى `iroha sns register --metadata-json ... --governance-json ...`.
- يتم تسجيل stdout و stderr مع أكواد الخروج؛ الاكواد غير الثلجية توقف التشغيل.

يمكن تشغيل وضعي الارسال معا (Torii و CLI)
لتمرين مسارات احتياطية.

### 3.3 اتصالات الارسال

عند تسارع `--submission-log <path>` للهاتف المحمول السكربت NDJSON الإختيار:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

بما في ذلك ردود Torii الناجحة بما في ذلك منظمة مستخرجة من `NameRecordV1` او
`RegisterNameResponseV1` (مثل `record_status`، `record_pricing_class`،
`record_owner`، `record_expires_at_ms`، `registry_event_version`، `suffix_id`،
`label`) حتى يبدأ متابعة وتقارير ال تور من تحليل السجل دون فتح
نص حر. ارفق هذا السجل مع تذكرة المسجل المعتمد من خلال بيان ثبات لاعادة
إنتاج.

## 4.اتـمتة اصدار بوابة الوثائق

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

سكربت:1. يبني `registrations.manifest.json` و `registrations.ndjson` وينسخ CSV الاصلي
   الى مجلد الاصدار.
2. بيان المرسلين عبر Torii و/او CLI (عند التهيئة)، ويكتب `submissions.log` مع
   صالات منظمة علاه.
3. يصدر `summary.json` الذي يصف الاصدار (المسارات، عنوان Torii، مسار CLI،
   الطابع الزمني) للمبتدئين لكي اتـمتة البوابة من رفع المتجر الى مخزن الاثار.
4. النتيجة `metrics.prom` (تجاوز عبر `--metrics`) متضمنا عدادات معتمدة
   Prometheus للطلبات الاجمالية التنافسية ومجاميع الاصل ونتائج الارسال.
   ربط JSON الملخص بهذا الملف.

تقوم بسير العمل بارشفة مجلد الاصدار كاثر واحد، والذي يحتوي الان على كل ما تحتاجه
وصفات للتدقيق.

##5.القياس ولوحات المتابعة

تم إعداد ملف المعايير القياسية `sns_bulk_release.sh` بالسلاسل التالية:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

قم بتغذية `metrics.prom` الى Sidecar Prometheus لديك (مثلا عبر Promtail او
مستورد دفعات) هجوم على المتصفحين والمضيفين والشركاء عبر الإنترنت
بالجملة. لوحة Grafana `dashboards/grafana/sns_bulk_release.json` تم تعرضها
مع لوحات داخلية لكل الأنظمة اللاحقة، حجم الدفع، ونسب النجاح/فشل الارسال.
تقوم بالتصفية عبر `release` حتى يتقدم المتمرسون للتمرين في CSV
واحد.

## 6. التحقق والحالات بناء على ذلك- **توحيد التسمية:** يتم تطبيع الادخالات باستخدام Python IDNA بأحرف صغيرة وفلاتر
  نورم v1. تفشل التسمات غير الصالحة بسرعة قبل اي اتصال شبكي.
- **حواجز رقمية:** يجب أن لا تقع معرفات اللاحقة وسنوات المدة وتلميحات التسعير ضمن النطاق
  `u16` و `u8`. تقبل الدفع لعدد عشرية او سداسي عشري حتى `i64::MAX`.
- **تحليل البيانات الوصفية او الحكم:** يتم تحليل JSON مباشرة؛ الحل النهائي
  المراجع الملفات نسبة الى موقع CSV. البيانات الوصفية غير الناشئة لا تتحقق.
- **المتحكمون:** خلايا الفارغة تلتزم بـ `--default-controllers`. قوائم المقدمة
  وحدة التحكم صريحة (مثل `<i105-account-id>;<i105-account-id>`) عند التفويض غير الرسمي.

يتم الابلاغ عن الاخطاء باستخدام ارقام الترتيب المنهجية (مثلا
`error: row 12 term_years must be between 1 and 255`). يخرج السكربت بالكود `1`
عند خطأ التحقق و `2` عندما يكون مسار CSV مفقودا.

## 7. الاختبار والاعتمادية

- غطاء `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` تحليل CSV،
  اصدار NDJSON، يفترض التشارك، ومسارات إرسال CLI او Torii.
- المساعدة مكتوبة ببايثون فقط (بدون تبعيات خوف) حيثما `python3`.
  يتم تتبع سجل الالتزامات دائمًا CLI في المستودع الرئيسي الموثوقية.

للانتاج، ارفق مانيفست الإخراج وزمة NDJSON مع تذكرة التسجيل حتى يتمكن المضيفون
من إعادة تشغيل الحمولات الدقيقة التي تم إرسالها إلى Torii.