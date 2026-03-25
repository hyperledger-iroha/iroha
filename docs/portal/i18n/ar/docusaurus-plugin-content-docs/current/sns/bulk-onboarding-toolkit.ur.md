---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
هذه الصفحة `docs/source/sns/bulk_onboarding_toolkit.md` هي إجابة البيروني
يقوم آبيرز باستنساخ كل من الطيور وSN-3b وهو عبارة عن صواريخ مضادة للطائرات.
:::

# مجموعة أدوات الإعداد المجمع لـ SNS (SN-3b)

**نظام الحوالة:** SN-3b "أدوات الإعداد المجمعة"  
**المصنوعات اليدوية:** `scripts/sns_bulk_onboard.py`، `scripts/tests/test_sns_bulk_onboard.py`،
`docs/portal/scripts/sns_bulk_release.sh`

أكثر المسجلين `.sora` أو `.nexus` التسجيلات هي الحوكمة
الموافقات وقضبان التسوية هي أكثر من مجرد رقم قياسي
ہیں۔ لا يوجد مقياس جديد لحمولات JSON التي نستخدمها أو CLI
لـ SN-3b أداة إنشاء CSV-to-Norito الحتمية منشئ Torii أو CLI
لئے `RegisterNameRequestV1` الهياكل الأساسية. مساعد صف کو پہلے
التحقق من صحة الكلمة والبيان المجمع وإخراج JSON الاختياري المفصول بسطر جديد
تتضمن عمليات التدقيق والتدقيق الإيصالات المنظمة وسجلات الحمولات النافعة
يجب أن تقوم بإرسال رسالتك.

## 1. تنسيق CSV

المحلل اللغوي يدمج درج صف الرأس (الطلب المرن):| العمود | مطلوب | الوصف |
|--------|----------|-------------|
| `label` | نعم | التسمية المطلوبة (تم قبول الحالة المختلطة؛ الأداة Norm v1 وUTS-46 متوافقة مع التطبيع کرتا ہے). |
| `suffix_id` | نعم | معرف اللاحقة الرقمية (عشري أو `0x` سداسي عشري). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | عدد صحيح `1..=255`. |
| `payment_asset_id` | نعم | أصل التسوية (مثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | الأعداد الصحيحة غير الموقعة هي وحدات الأصول الأصلية التي تمثل كريں. |
| `settlement_tx` | نعم | قيمة JSON أو سلسلة حرفية أو معاملة دفع أو تجزئة. |
| `payment_payer` | نعم | AccountId جس نے إذن الدفع کی۔ |
| `payment_signature` | نعم | JSON أو سلسلة حرفية هي إثبات توقيع الوكيل/الخزانة. |
| `controllers` | اختياري | عناوين حساب المراقب کی فاصلة منقوطة/قائمة مفصولة بفواصل۔ خالٍ من جديد على `[owner]`. |
| `metadata` | اختياري | تلميحات محلل JSON المضمّن أو `@path/to/file.json`، وسجلات TXT وغيرها. الافتراضي `{}`. |
| `governance` | اختياري | Inline JSON أو `@path` أو `GovernanceHookV1` طرف ثالث. `--require-governance` هو العمود الذي يحتاج إلى كرتا. |

هذا العمود هو أيضًا عمود سيل ويليو `@` للإشارة إلى الملف الخارجي.
مسارات ملف CSV ذات حل نسبي.

## 2. مساعد چلانا```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

الخيارات الرئيسية:

- `--require-governance` ربط الصفوف کے بغیر رفض کرتا ہے (قسط
  المزادات أو المهام المحجوزة کے لئے مفید).
- `--default-controllers {owner,none}` مالك خلايا التحكم الفارغة
  حساب على الهاتف المحمول أو لا.
- `--controllers-column`، `--metadata-column`، و`--governance-column` المنبع
  يتم تصدير البطاقة إلى الأعمدة الاختيارية التي تم تغييرها.

البيان المجمع للنص البرمجي هو:

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
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
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

إذا كان `--ndjson` فهذا يعني أن `RegisterNameRequestV1` هو JSON أحادي السطر
المستند الذي تم إنشاؤه مسبقًا لتنفيذ طلبات التشغيل الآلي Torii
تيار كر سكي:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. التقديمات الآلية

### 3.1 Torii وضع REST

`--submit-torii-url` يعمل على `--submit-token` أو `--submit-token-file`
البيان الذي تم إدخاله لـ Torii للدفع:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- طلب مساعد `POST /v1/sns/names` و HTTP
  حدث خطأ أثناء الإجهاض. يتم إلحاق مسار سجل الردود بسجلات NDJSON
  ہوتے ہيں.
- `--poll-status` تم الإرسال بعد `/v1/sns/names/{namespace}/{literal}`
  استعلام جديد مرة أخرى (أكثر من `--poll-attempts`، الافتراضي 5) سجل هذا
  مرئية ہونے کی تصدیق ہو۔ `--suffix-map` (JSON جو `suffix_id` قيم "اللاحقة"
  خريطة الخريطة) أداة أداة الرسم `{label}.{suffix}` تشتق الأحرف الحرفية من كر سك.
- الانضباطي: `--submit-timeout`، `--poll-attempts`، `--poll-interval`.

### 3.2 وضع iroha CLIالإدخال الواضح لـ CLI يتتبع المسار الثنائي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- وحدات التحكم کو `Account` إدخالات ہونا چاہئے (`controller_type.kind = "Account"`)
  إن CLI في الواقع تقوم بكشف وحدات التحكم القائمة على الحساب.
- البيانات الوصفية ونقاط الحوكمة يمكن طلبها من الملفات المؤقتة
  ها و`iroha sns register --metadata-json ... --governance-json ...` كو باس
  هذا ما حدث.
- CLI stdout/stderr وسجل رموز الخروج ہوتے ہیں؛ يتم تشغيل الرموز غير الصفرية لإحباط العملية.

أوضاع الإرسال دون أي شيء آخر (Torii وCLI) بالإضافة إلى المسجل
عمليات النشر والتحقق المتبادل من التدريبات الاحتياطية أو الاحتياطية.

### 3.3 إيصالات التقديم

جب `--submission-log <path>` إلحاق إدخالات البرنامج النصي NDJSON بـ كرتا ہے:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تم تحديد استجابات Torii الناجحة بواسطة `NameRecordV1` أو `RegisterNameResponseV1`
تتضمن الحقول المنظمة ہوتے ہیں (مثال `record_status`، `record_pricing_class`،
`record_owner`، `record_expires_at_ms`، `registry_event_version`، `suffix_id`،
`label`) تحتوي لوحات المعلومات وسجل تقارير الإدارة على نص حر الشكل
تحليل کر سکیں. سجله وبيان تذاكر المسجل ثم قم بإرفاق البطاقة
أدلة قابلة للتكرار رہے۔

## 4. أتمتة إصدار بوابة المستندات

وظائف CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` للاتصال بالبطاقة، جو
مساعد في تغليف الكرتا والمصنوعات اليدوية `artifacts/sns/releases/<timestamp>/`
تحت المتجر كرتا ہے:

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

البرنامج النصي:1. `registrations.manifest.json`, `registrations.ndjson` بناتا وأصلي
   يحتوي ملف CSV على دليل الإصدار.
2. Torii و/أو CLI إرسال بيان الإرسال (بعد التكوين)، و
   `submissions.log` يحتوي على الإيصالات المنظمة.
3. `summary.json` ينبعث من الكرتا ويطلقها ويصفها (المسارات، Torii URL،
   مسار CLI، الطابع الزمني) بالإضافة إلى حزمة أتمتة البوابة وتخزين العناصر
   تحميل کر سکے۔
4. `metrics.prom` بناتنا (`--metrics` تجاوز التجاوز)، وهو Prometheus-
   عدادات التنسيق: إجمالي الطلبات، توزيع اللاحقة، إجماليات الأصول،
   و نتائج التقديم ۔ ملخص JSON هو ملف وصلة طرفية كرتا ہے۔

يقوم سير العمل بتحرير الدليل فقط وهو عبارة عن قطعة أثرية موجودة في أرشيف البطاقة، وهذا يعني
لقد تم استخدام أسلوب الحوكمة الجيدة والتدقيق في العمل.

## 5. القياس عن بعد ولوحات المعلومات

`sns_bulk_release.sh` ملف المقاييس الموضح في سلسلة درج يعرض العناصر:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` في Prometheus تغذية جانبية (مثال على ذلك Promtail)
أو مستورد الدفعة) إلى جانب المسجلين والمشرفين وأقران الحوكمة بشكل جماعي
التقدم پر الانحياز رہیں۔ لوحة Grafana
`dashboards/grafana/sns_bulk_release.json` ولوحات البيانات محددة: لكل لاحقة
التهم، حجم الدفع، ونسب نجاح/فشل التقديم۔ المجلس `release`
مرشح الكرتا ومراجعي الحسابات يتم تشغيل ملف CSV قبل التدريب على الكمبيوتر.

## 6. أوضاع التحقق والفشل- **التعريف الأساسي للتسمية:** يُدخل Python IDNA بأحرف صغيرة وNorm v1
  تعمل مرشحات الأحرف على تطبيعها. تسميات غير صالحة لمكالمات الشبكة سے پہلے
  تفشل بسرعة.
- **حواجز الحماية الرقمية:** معرفات اللاحقة وسنوات الفصل الدراسي وتلميحات التسعير `u16` و`u8`
  حدود اندر ہونے چاہئیں. حقول الدفع العشرية أو الأعداد الصحيحة السداسية `i64::MAX`
  تک قبول کرتے ہیں.
- **تحليل البيانات الوصفية أو الإدارة:** تحليل JSON المضمن للتحليل؛ file
  المراجع موقع CSV هو الحل النسبي. البيانات التعريفية غير الكائنية
  خطأ في التحقق دیتا ہے۔
- **وحدات التحكم:** الخلايا الفارغة `--default-controllers` لتكريم الشرف. غير مالك
  الجهات الفاعلة کو مندوب کرتے وقت قوائم التحكم الصريحة دیں (مثال `i105...;i105...`)۔

حالات الفشل في أرقام الصفوف السياقية کے ساتھ تقرير ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). أخطاء التحقق من صحة البرنامج النصي
`1` ومسار CSV مفقودان منذ `2` ثم خرجا من الكرتا.

## 7. الاختبار والمصدر

- تحليل `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV، NDJSON
  الانبعاثات، وإنفاذ الحوكمة، ومسارات تقديم CLI/Torii التي تغطي المنطقة.
- مساعد Pure Python (لا توجد تبعيات إضافية) وأداة `python3`
  ہو وہاں چلتا ہے. سجل الالتزام CLI الذي يتتبع المستودع الرئيسي
  هذا هو الاستنساخ.يتم تشغيل الإنتاج، والبيان الذي تم إنشاؤه وحزمة NDJSON وتذكرة المسجل
قم بإرفاق مشرفي العمليات Torii لإرسال إعادة تشغيل الحمولات الصافية الدقيقة
كر سكي.