---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
راجع `docs/source/sns/bulk_onboarding_toolkit.md` لكيفية تشغيل المشغلين الخارجيين
نفس الشيء SN-3b بدون استنساخ المستودع.
:::

# مجموعة أدوات تأهيل masivo SNS (SN-3b)

**مرجع خريطة الطريق:** SN-3b "أدوات الإعداد المجمعة"  
**المصنوعات اليدوية:** `scripts/sns_bulk_onboard.py`، `scripts/tests/test_sns_bulk_onboard.py`،
`docs/portal/scripts/sns_bulk_release.sh`

كبار المسجلين في القائمة المعدة مسبقًا لقوائم السجلات `.sora` أو `.nexus`
مع نفس الموافقة على الإدارة وقضايا التسوية. حمولات Armar JSON
لا تصعد يدويًا أو قم بتشغيل CLI حتى يدخل SN-3b أداة إنشاء
تحديد ملف CSV إلى Norito لإعداد الهياكل `RegisterNameRequestV1` لـ
Torii أو CLI. المساعد صالح لكل عائلة قديمة، يصدر بيانًا كبيرًا
تمت الموافقة عليه كـ JSON محدد بواسطة خطوط الخطوط الاختيارية، ويمكنك إرسالها إليك
الحمولات الصافية تلقائيًا أثناء تسجيل هياكل التسجيل للمستمعين.

## 1. إسكيما CSV

يتطلب المحلل اللغوي الملف التالي للتغليف (الترتيب مرن):| كولومنا | مطلوب | الوصف |
|---------|----------|-------------|
| `label` | سي | العلامات المطلوبة (إذا كانت مقبولة/ناقصة؛ الأداة طبيعية باستخدام Norm v1 وUTS-46). |
| `suffix_id` | سي | المعرف الرقمي للصوفيجو (عشري أو `0x` سداسي عشري). |
| `owner` | سي | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | سي | انتيرو `1..=255`. |
| `payment_asset_id` | سي | نشاط التسوية (على سبيل المثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | سي | إن الأمعاء ليست علامة على أنها تمثل وحدات من السكان الأصليين النشطين. |
| `settlement_tx` | سي | قيمة JSON أو السلسلة الحرفية التي تصف معاملة الدفع أو التجزئة. |
| `payment_payer` | سي | معرف الحساب الذي سيتم دفعه. |
| `payment_signature` | سي | JSON أو سلسلة حرفية مع اختبار شركة المضيف أو المخزن. |
| `controllers` | اختياري | قائمة مفصولة بنقطة وفاصلة أو فاصلة اتجاهات وحدة التحكم. بسبب الخلل `[owner]` عندما يتم حذفه. |
| `metadata` | اختياري | JSON inline o `@path/to/file.json` الذي يقدم تلميحات المحلل، وسجلات TXT، وما إلى ذلك. من خلال `{}`. |
| `governance` | اختياري | JSON مضمّن أو `@path` ينضم إلى `GovernanceHookV1`. `--require-governance` يتطلب هذا العمود. |يمكن لأي عمود الرجوع إلى ملف خارجي يحدد قيمة الخلية مع `@`.
تتم إعادة ربط المسارات بملف CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

الخيارات الرئيسية:

- `--require-governance` rechaza filas sin un Hook de gobernanza (يستخدم للفقرة
  subastas premium أو asignaciones reservadas).
- `--default-controllers {owner,none}` تقرر ما إذا كانت وحدات التحكم فارغة
  vuelven a la cuenta مالك.
- `--controllers-column`، `--metadata-column`، و`--governance-column` المسموح به
  قم بإعادة تسمية الأعمدة الاختيارية عند العمل مع الصادرات.

في حالة الخروج من النص، قم بكتابة بيان مُتفق عليه:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
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

إذا تم تخصيص `--ndjson`، فكل `RegisterNameRequestV1` يتم كتابته أيضًا كأحد
مستند JSON عبارة عن سطر واحد حتى تتمكن الأتمتة من الإرسال
الطلبات المباشرة Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envios تلقائي

### 3.1 مودو Torii ريست

خاص `--submit-torii-url` mas `--submit-token` أو `--submit-token-file` للفقرة
قم بإدخال كل بيان إلى Torii مباشرة:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- يقوم المساعد بإصدار رقم `POST /v1/sns/names` للطلب والإيقاف قبل ذلك
  خطأ تمهيدي HTTP. يتم توصيل الإجابات إلى مسار السجل مثل السجلات
  ندجسون.
- `--poll-status` قم بزيارة مستشار `/v1/sns/names/{namespace}/{literal}` بعد ذلك
  كل إرسال (يحتوي على `--poll-attempts`، الافتراضي 5) لتأكيد التسجيل
  مرئي. Proporcione `--suffix-map` (JSON de `suffix_id` عبارة عن "لاحقة")
  لكي تشتق الأدوات حرفيًا `{label}.{suffix}` من أجل إجراء الاستقصاء.
- الضبط: `--submit-timeout`، `--poll-attempts`، و`--poll-interval`.

### 3.2 مودو إيروها CLI

لبدء إدخال كل بيان من خلال CLI، قم بالإشارة إلى المسار الثنائي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- وحدات التحكم يجب أن تكون مدخلات `Account` (`controller_type.kind = "Account"`)
  لأن CLI في الواقع يعرض فقط وحدات التحكم القائمة على الحسابات.
- يتم وصف نقاط البيانات الوصفية والحوكمة في أرشيفات زمنية
  أرجو منك أن تقوم بذلك على `iroha sns register --metadata-json ... --governance-json ...`.
- يتم تسجيل جميع إعدادات ومواصفات CLI حيث يتم تسجيل رموز الخروج؛ لوس كوديجوس
  لن يؤدي ذلك إلى إجهاض القذف.

يمكن لوسائل الإرسال المتعددة تنفيذ المجموعات (Torii y CLI) للتحقق
قم بالاطلاع على المسجل أو البحث عن الإجراءات الاحتياطية.

### 3.3 وصفات البيئة

عند تقديم `--submission-log <path>`، يتم إدخال البرنامج النصي في NDJSON الذي
كابتوران:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```تتضمن الردود الخارجية لـ Torii المجالات التعليمية الإضافية
`NameRecordV1` أو `RegisterNameResponseV1` (على سبيل المثال `record_status`،
`record_pricing_class`، `record_owner`، `record_expires_at_ms`،
`registry_event_version`، `suffix_id`، `label`) للوحات المعلومات والتقارير
يمكنك تحليل السجل بدون فحص النص الحر. سجل مساعد هذا أ
تذاكر المسجل مرفقة ببيان الأدلة القابل للتكرار.

## 4. أتمتة إصدار البوابة

أعمال CI واتصال البوابة بـ `docs/portal/scripts/sns_bulk_release.sh`،
que envuelve el helper andguarda artifacts bajo `artifacts/sns/releases/<timestamp>/`:

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

البرنامج النصي:

1. قم ببناء `registrations.manifest.json` و`registrations.ndjson` وانسخ
   ملف CSV الأصلي في دليل الإصدار.
2. أرسل البيان باستخدام Torii و/أو CLI (عند تكوينه)، واكتبه
   `submissions.log` مع إيصالات إنشاءات التوصيل.
3. قم بإصدار `summary.json` لوصف الإصدار (rutas، URL Torii، ruta CLI،
   الطابع الزمني) حتى تتمكن أتمتة البوابة من شحن الحزمة أ
   تخزين القطع الأثرية.
4. قم بإنتاج `metrics.prom` (التجاوز عبر `--metrics`) الذي يحتوي على مقاومات
   بتنسيق Prometheus لإجمالي الطلبات وتوزيع الصوفيات،
   إجمالي الأصول ونتائج البيئة. استئناف استئناف JSON في هذا
   معرف com لهذا التطبيق هو com.archivo.تقوم عمليات سير العمل ببساطة بأرشفة دليل الإصدار باعتباره قطعة أثرية منفردة،
يحتوي الآن على كل ما تحتاج إليه الإدارة من أجل الاستماع.

## 5. القياس عن بعد ولوحات المعلومات

يعرض ملف المقاييس الذي تم إنشاؤه بواسطة `sns_bulk_release.sh` العناصر التالية
سلسلة:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

يتم شحن `metrics.prom` على جانب Prometheus (على سبيل المثال عبر Promtail o
دفعة مستوردة) للحفاظ على المسجلين والمضيفين وأولياء الأمور
خطوتين حول التقدم الكبير. الطاولة Grafana
`dashboards/grafana/sns_bulk_release.json` يعرض نفس البيانات مع اللوحات
Para conteos por sufijo، حجم الدفع ونسب الخروج / سقوط الحسد. ش
مرشح الطاولة لـ `release` حتى يتمكن المدققون من الدخول بمفردهم
مسار CSV.

## 6. التحقق من صحة وطريقة السقوط- **تسوية التسمية:** تتم تسوية الإدخالات مع Python IDNA mas
  الأحرف الصغيرة ومرشحات الأحرف Norm v1. التسميات غير صالحة Fallan Rapido Antes
  من أي مكالمة حمراء.
- **الدرابزين الرقمي:** معرفات اللاحقة وسنوات الفصل الدراسي وتلميحات التسعير
  داخل الحدود `u16` و`u8`. حقول الدفع تقبل الأرقام العشرية المعوية o
  ست عشري hasta `i64::MAX`.
- **تحليل البيانات الوصفية أو الإدارة:** JSON inline يتم تحليله مباشرة؛ لاس
  تصبح المراجع في الأرشيف مرتبطة بموقع CSV. البيانات الوصفية
  لا يوجد أي كائن بحري ينتج عنه خطأ في التحقق.
- **وحدات التحكم:** celdas en blanco respetan `--default-controllers`. بروبورسيوني
  قوائم وحدة التحكم الصريحة (على سبيل المثال `soraカタカナ...;soraカタカナ...`) للتفويض
  الجهات الفاعلة لا مالك.

يتم الإبلاغ عن السقوط بعدد من الملفات السياقية (على سبيل المثال
`error: row 12 term_years must be between 1 and 255`). بيع البرنامج النصي مع كوديغو
`1` في أخطاء التحقق من الصحة و`2` عند فشل مسار CSV.

## 7. الاختبار والإجراء

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` تحليل مكعب CSV،
  انبعاث NDJSON وتطبيق الإدارة وقنوات البيئة بواسطة CLI o Torii.
- المساعد هو Python النقي (بدون تبعيات إضافية) والمراسلة في أي مكان
  هذا المكان متاح `python3`. يتم ارتكاب الجرائم التاريخية معًا
  على غرار CLI في المستودع الرئيسي لإعادة الإنتاج.لخطوط الإنتاج، إضافة إلى البيان الذي تم إنشاؤه وحزمة NDJSON al
تذكرة المسجل حتى يتمكن المضيفون من إعادة إنتاج الحمولات الدقيقة
إذا قمت بإرسال Torii.