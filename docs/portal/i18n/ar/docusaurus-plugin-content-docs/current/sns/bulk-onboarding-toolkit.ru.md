---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
هذا الجزء يرسل `docs/source/sns/bulk_onboarding_toolkit.md` إلى الخلف
توصي المشغلات بـ SN-3b بدون مستودعات النسخ.
:::

# مجموعة أدوات الإعداد المجمع لـ SNS (SN-3b)

**خريطة الطريق الخاصة:** SN-3b "أدوات الإعداد المجمعة"  
**المنتجات:** `scripts/sns_bulk_onboard.py`، `scripts/tests/test_sns_bulk_onboard.py`،
`docs/portal/scripts/sns_bulk_release.sh`

يقوم مسجلو الشركات بإعادة تسجيل جميع المسجلين `.sora` أو
`.nexus` مع أنظمة التحكم والتسوية الوحيدة. Ручная сborка
لا يمكن تخزين حمولات JSON أو واجهة CLI اللاحقة، حيث يتم نشر SN-3b
تحديد CSV-to-Norito builder، الذي يستخدم الهياكل
`RegisterNameRequestV1` لـ Torii أو CLI. المساعدة في التحقق من صحة كل مرة,
قم بإلقاء نظرة على البيان المجمع وملف JSON الاختياري الاختياري، ويمكنك تنفيذه
الحمولات التلقائية، تسجل الإيصالات الهيكلية لمراجعي الحسابات.

## 1. مخطط CSV

يجب أن يقوم المحلل بالخطوات التالية (على سبيل المثال):| كولونكا | بالتأكيد | الوصف |
|---------|------------|----------|
| `label` | نعم | Запрозенная метка (يوفر حالة مختلطة ؛ تم ضبط الأداة على Norm v1 وUTS-46). |
| `suffix_id` | نعم | لاحقة المعرف تشيسلوفي (ست عشرية أو `0x`). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | نعم | سيلوي تشيسلو `1..=255`. |
| `payment_asset_id` | نعم | التسوية النشطة (على سبيل المثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | مجموعة رائعة من الوحدات النشطة. |
| `settlement_tx` | نعم | بداية JSON أو السكتة الدماغية أو تسجيل المعاملات الضريبية أو التجزئة. |
| `payment_payer` | نعم | معرف الحساب، لوحة ترخيص. |
| `payment_signature` | نعم | JSON أو خطوة مع وكيل الدعم أو الخزانة. |
| `controllers` | اختياري | وحدة تحكم العنوان المدرجة، `;` أو `,`. من خلال `[owner]`. |
| `metadata` | اختياري | Inline JSON أو `@path/to/file.json` مع تلميحات المحلل وتسجيل TXT وما إلى ذلك. د. من خلال `{}`. |
| `governance` | اختياري | مضمّن JSON أو `@path` إلى `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

يمكن استخدام أي نقطة أخرى في الملف الداخلي، إذا قمت بإضافة `@` في البداية.
يمكنك إعادة النظر في ملف CSV.

## 2. قم بالمساعدة```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

الخيارات الرئيسية:

- `--require-governance` لإلغاء قفل السلاسل بدون ربط الإدارة (مفيد للإستخدام)
  مزادات متميزة أو مهام محجوزة).
- تمت إعادة تعيين `--default-controllers {owner,none}` لوحدة التحكم الثابتة تمامًا
  الاستسلام للمالك.
- `--controllers-column`، و`--metadata-column`، و`--governance-column`
  قم باستبدال النقطتين الاختياريتين قبل التعامل مع الصادرات الأولية.

في هذه الحالة، النص البرمجي للبيان المصرح به:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<katakana-i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<katakana-i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<katakana-i105-account-id>",
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

إذا تم تحديد `--ndjson`، فسيتم أيضًا تسجيل كل من `RegisterNameRequestV1` كما هو موضح
مستند JSON الوحيد الذي يمكن من خلاله إجراء الأتمتة بشكل أسرع
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. عمليات التشغيل الآلي

### 3.1 الإصدار Torii REST

اكتب `--submit-torii-url` و`--submit-token`، و`--submit-token-file`،
للقيام بذلك، قم بكتابة البيان على سبيل المثال في Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- المساعدة في الاتصال بأحد `POST /v1/sns/names` والتوقف عنه
  خطأ HTTP الأول. يتم إضافة الإجابات إلى السجل عن طريق كتابة NDJSON.
- `--poll-status` تم الاتصال به مرة أخرى `/v1/sns/names/{namespace}/{literal}` بعد ذلك
  بعض الإجراءات (إلى `--poll-attempts`، من خلال 5)، للتأكيد
  видимость записи. قم بتنزيل `--suffix-map` (رسم خرائط JSON `suffix_id` في البداية)
  "suffix")، يمكن للأداة أن تظهر `{label}.{suffix}` للاقتراع.
- الإعدادات: `--submit-timeout`، `--poll-attempts`، و`--poll-interval`.

### 3.2 نسخة iroha CLIللمضي قدماً في كتابة البيان من خلال CLI، قم بالذهاب إلى الثنائية:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- وحدات التحكم تحتاج إلى إدخالات `Account` (`controller_type.kind = "Account"`)،
  مما يؤدي إلى دعم CLI لوحدات التحكم القائمة على الحساب فقط.
- تنتقل البيانات الوصفية ونقط الإدارة إلى الملفات المؤقتة من أجل البحث
  يتم الانتقال إلى `iroha sns register --metadata-json ... --governance-json ...`.
- تسجيل الدخول Stdout/stderr ورموز CLI؛ تعمل الرموز الجديدة على منع الغلق.

يمكن الآن البدء في إجراء عمليات الفحص على الفور (Torii وCLI) للاختبارات المسبقة
عمليات نشر المسجل أو المسار الاحتياطي المتكرر.

### 3.3 الإجراءات الوقائية

في `--submission-log <path>`، سيتم كتابة النص البرمجي NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تتضمن الإجابات الجيدة Torii عمودًا هيكليًا من `NameRecordV1` أو
`RegisterNameResponseV1` (على سبيل المثال `record_status`، `record_pricing_class`،
`record_owner`، `record_expires_at_ms`، `registry_event_version`، `suffix_id`،
`label`)، لتمكنك من التحكم في اللوحات والتجاوزات دون تسجيل الدخول
النص. قم بقص هذا السجل من تذكرة المسجل مباشرة مع البيان
воспроизводимого доказательства.

## 4. أتمتة بوابة الإصدار

وظائف CI والبوابة تحدد `docs/portal/scripts/sns_bulk_release.sh`، التي
الحصول على المساعدة والعناصر المصاحبة ضمن `artifacts/sns/releases/<timestamp>/`:

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

النص:1. قم بإنشاء `registrations.manifest.json` و`registrations.ndjson` وانسخ
   يوجد ملف CSV في إصدار الدليل.
2. قم بتسليم البيان من خلال Torii و/أو CLI (عند التثبيت)، اكتبه
   `submissions.log` مع المزيد من الهياكل التنظيمية.
3. قم بتكوين `summary.json` مع الإصدار الموضح (أدخل، Torii URL، أدخل CLI،
   الطابع الزمني)، بحيث يمكنك أتمتة البوابة من خلال تخزين الحزمة في الإطار الزمني
   قطعة أثرية.
4. إنشاء `metrics.prom` (التجاوز من خلال `--metrics`) باستخدام Prometheus
   تذكيرات لموضوع شيسلا، ملحق ملحق، ملخص للنشاط
   والنتائج. ملخص JSON يتبع هذا الملف.

سير العمل هو مجرد أرشفة الدليل كما هو قطعة أثرية واحدة، مشتركة
كل ما تحتاجه للتدقيق.

## 5. أجهزة القياس عن بعد ولوحات الاتصال

مقياس الفشل، المولد للطاقة `sns_bulk_release.sh`، يشارك في السلسلة التالية:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

قم بإدخال `metrics.prom` في Prometheus Sidecar (على سبيل المثال من خلال Promtail أو الدفعة)
المستورد)، من المسجلين والمشرفين وأقران الحوكمة الذين يؤيدون
التقدم في العملية المجمعة. لوحة القيادة Grafana
`dashboards/grafana/sns_bulk_release.json` تصورها كما هي: مجموعة
من خلال الضغط على لوحة المفاتيح والوسادة، نتيجة ناجحة/غير صحية. داش بورد
يتم التصفية إلى `release` حتى يتمكن المدققون من التعرف على أحد برامج CSV.

## 6. التحقق من الصحة وإلغاء الأنظمة- **التسمية الأساسية:** تعمل على تطبيع Python IDNA + الأحرف الصغيرة و
  مرشحات سيمفولوف نورم v1. تتجه أدوات نيفالايد بسهولة إلى مجموعة متنوعة من الطائرات.
- **حواجز الحماية الرقمية:** معرفات اللاحقة وسنوات الفصل الدراسي وتلميحات التسعير مطلوبة
  سبق `u16` و`u8`. بداية اللوحة غير متجانسة أو عرافة إلى
  `i64::MAX`.
- **تحليل البيانات الوصفية/الحوكمة:** inline JSON parсится напямую; خيارات الملفات
  قم بإعادة تعيين ملف CSV. البيانات الوصفية هي جزء من كائن ما للتحقق من صحة البيانات.
- **وحدات التحكم:** يتم الاشتراك في `--default-controllers`. تحدث
  وحدات التحكم في القوائم الرسمية (على سبيل المثال `<katakana-i105-account-id>;<katakana-i105-account-id>`) للمفوضين من غير المالك.

يتم إرسال الرسائل النصية من خلال أرقام الأرقام (على سبيل المثال
`error: row 12 term_years must be between 1 and 255`). قم بإنشاء البرنامج النصي باستخدام الكود `1`
عند التحقق من الصحة و`2`، إذا قمت بالضغط على CSV.

## 7. الاختبار والنسخ

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` يقوم بتحليل تحليل CSV،
  إرجاع NDJSON وحوكمة التنفيذ وطرق التنفيذ عبر CLI أو Torii.
- المساعدة في كتابة لغة بايثون النقية (بدون معلومات إضافية) والعمل عليها
  لقد أصبح جاهزًا الآن `python3`. لجنة التاريخ تراقب بشكل جيد مع CLI في
  المستودعات الأساسية للتجديد.

من أجل تقديم بيان مبتكر وحزمة NDJSON
تذكرة المسجل حتى يتمكن المضيفون من اكتشاف الحمولات الصافية المحددة، والتحكم فيها
في Torii.