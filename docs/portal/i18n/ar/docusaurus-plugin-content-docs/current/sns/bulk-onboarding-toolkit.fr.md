---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
تعكس هذه الصفحة `docs/source/sns/bulk_onboarding_toolkit.md` لتوضيح الأمر
يظهر المشغلون الخارجيون توجيهات SN-3b بدون استنساخ المستودع.
:::

# مجموعة أدوات الصعود إلى كتلة SNS (SN-3b)

**خارطة الطريق المرجعية:** SN-3b "أدوات الإعداد المجمعة"  
**المصنوعات اليدوية:** `scripts/sns_bulk_onboard.py`، `scripts/tests/test_sns_bulk_onboard.py`،
`docs/portal/scripts/sns_bulk_release.sh`

يقوم كبار المسجلين بإعداد مئات من التسجيلات `.sora` ou
`.nexus` مع موافقات الحوكمة وقضايا التسوية.
قم بتصنيع حمولات JSON بشكل رئيسي أو إعادة ربطها بـ CLI بدون مقياس، دونك SN-3b
كتاب محدد منشئ CSV vers Norito الذي يقوم بإعداد الهياكل
`RegisterNameRequestV1` لـ Torii أو CLI. L'helper valide chaque ligne en
حسنًا، سيتم إصدار بيان متفق عليه ومحدد JSON من جديد
خيار الخطوط، ويمكن أن يتم تحميل الحمولات تلقائيًا بالكامل
مسجل هياكل الإنقاذ لعمليات التدقيق.

## 1. مخطط CSV

Le parseur exige la ligne d'en-tete suivante (الأمر مرن):| كولون | يتطلب | الوصف |
|---------|-------|-------------|
| `label` | أوي | Libelle requeste (casse mixte Acceptee؛ l'outil Normalize selon Norm v1 et UTS-46). |
| `suffix_id` | أوي | معرف لاحقة رقمية (عشري أو `0x` سداسي عشري). |
| `owner` | أوي | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | أوي | كامل `1..=255`. |
| `payment_asset_id` | أوي | نشاط التسوية (على سبيل المثال `xor#sora`). |
| `payment_gross` / `payment_net` | أوي | Entiers Non Signespresentant des Units Natives de l'actif. |
| `settlement_tx` | أوي | قيمة JSON أو سلسلة حرفية تشتق من معاملة الدفع أو التجزئة. |
| `payment_payer` | أوي | معرف الحساب الذي يقوم بتفويض الدفع. |
| `payment_signature` | أوي | JSON أو سلسلة حرفية تحتوي على توقيع المضيف أو الخزانة. |
| `controllers` | أوبتيونيل | قائمة منفصلة حسب النقطة الافتراضية أو الافتراضية لعناوين وحدة التحكم في الحساب. بشكل افتراضي `[owner]`. |
| `metadata` | أوبتيونيل | يوفر JSON المضمّن أو `@path/to/file.json` تلميحات المحلل وتسجيلات TXT وما إلى ذلك. افتراضيًا `{}`. |
| `governance` | أوبتيونيل | JSON مضمن أو `@path` يشير إلى `GovernanceHookV1`. `--require-governance` فرض هذا العمود. |يمكن أن يشير كل عمود إلى ملف خارجي يسبق قيمة الخلية
الاسمية `@`. تم تحديد الروابط بدقة من خلال ملف CSV.

## 2. منفذ المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

الخيارات:

- `--require-governance` rejette les lignes sans Hook de goovernance (مفيدة للصب)
  les encheres premium أو المؤثرات الاحتياطية).
- `--default-controllers {owner,none}` يقرر ما إذا كانت وحدات التحكم في الخلايا مرئية
  retombent sur le compte المالك.
- `--controllers-column`، `--metadata-column`، و`--governance-column` الدائم
  قم بإعادة تسمية أعمدة الخيارات الخاصة بالصادرات الكبيرة.

في حالة نجاح النص، سيكتب بيانًا مشتركًا:

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

Si `--ndjson` متاح، كل `RegisterNameRequestV1` مكتوب أيضًا كواحد
مستند JSON sur une ligne afin que les automatisations puissent Streamer les
يطلب التوجيه مقابل Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. أتمتة عمليات الشراء

### الوضع 3.1 Torii REST

حدد `--submit-torii-url` plus `--submit-token` أو `--submit-token-file` من أجل
Pousser chaque entree du Manifest Directement vers Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- المساعدة في الحصول على `POST /v1/sns/registrations` حسب الطلب والانتظار أولاً
  خطأ HTTP. تم إضافة الردود إلى السجل كتسجيل NDJSON.
- `--poll-status` إعادة الاستجواب `/v1/sns/registrations/{selector}` بعد كل مرة
  soumission (jusqu'a `--poll-attempts`، الافتراضي 5) لتأكيد ذلك
  أصبح التسجيل مرئيًا. فورنيسيز `--suffix-map` (JSON de `suffix_id`
  Vers des valeurs "suffix") لكي تشتق الأداة النصوص
  `{label}.{suffix}` للاقتراع.
- العناصر القابلة للضبط: `--submit-timeout`، و`--poll-attempts`، و`--poll-interval`.

### 3.2 وضع iroha CLI

من أجل المرور بكل سهولة إلى البيان من خلال CLI، قم بتوفير طريقك
ثنائي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- وحدات التحكم doivent etre des entrees `Account` (`controller_type.kind = "Account"`)
  يعرض Car la CLI فريدة من نوعها لقواعد التحكم في الحسابات.
- البيانات الوصفية النقطية والحوكمة مكتوبة في الملفات المؤقتة بالتساوي
  اطلب وأرسل `iroha sns register --metadata-json ... --governance-json ...`.
- Le stdout وstderr de la CLI بالإضافة إلى أن رموز الفرز يتم تسجيلها؛
  الرموز غير الصفرية تمنع التنفيذ.

يمكن تشغيل وضعي الإدخال معًا (Torii et CLI) للصب
انتقل إلى عمليات نشر المسجل أو كرر الإجراءات الاحتياطية.

### 3.3 احتياطي التغذية

عندما يتم تقديم `--submission-log <path>`، يقوم البرنامج بإضافة مداخل NDJSON
آسر:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```تتضمن الردود Torii reussies هياكل الأبطال الإضافية
`NameRecordV1` أو `RegisterNameResponseV1` (على سبيل المثال `record_status`،
`record_pricing_class`، `record_owner`، `record_expires_at_ms`،
`registry_event_version`، `suffix_id`، `label`) للتعرف على لوحات المعلومات والأشياء
يمكن لتقارير الحوكمة تحليل السجل بدون مفتش النص الحر.
انضم إلى سجل التذاكر المسجل مع البيان للحصول على دليل
قابلة للتكرار.

## 4. أتمتة تحرير البوابة

وظائف CI وبوابة الاتصال `docs/portal/scripts/sns_bulk_release.sh`، qui
قم بتغليف المساعدة وتخزين القطع الأثرية
`artifacts/sns/releases/<timestamp>/`:

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

البرنامج النصي لو:

1. قم بإنشاء `registrations.manifest.json` و`registrations.ndjson` وانسخ الملف
   ملف CSV الأصلي في مرجع الإصدار.
2. قم بإرسال البيان عبر Torii et/ou la CLI (عند التكوين)، عبر الإنترنت
   `submissions.log` مع الهياكل المخصصة.
3. اشتق `summary.json` من الإصدار (الرابط، URL Torii، الرابط CLI،
   الطابع الزمني) حتى تتمكن أتمتة البوابة من تحميل الحزمة مرة أخرى
   مخزون القطع الأثرية.
4. Produit `metrics.prom` (التجاوز عبر `--metrics`) يحتوي على الكمبيوترات
   بتنسيق Prometheus لإجمالي الطلبات، وتوزيع اللواحق،
   إجمالي الأصول ونتائج الاستحواذ. استئناف لو JSON بوانت فيرس
   ملف م.يتم أرشفة سير العمل ببساطة كمرجع للإصدار باعتباره قطعة أثرية واحدة،
الذي قد يضطرب تمامًا دون الحاجة إلى الحوكمة للتدقيق.

## 5. القياس عن بعد ولوحات المعلومات

يعرض ملف المقاييس النوعية `sns_bulk_release.sh` السلسلة
suivantes:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Injectez `metrics.prom` في سيارتك الجانبية Prometheus (على سبيل المثال عبر Promtail ou
دفعة استيراد واحدة) لمحاذاة المسجلين والمضيفين وأزواج الحوكمة
التقدم بشكل جماعي. اللوحة Grafana
`dashboards/grafana/sns_bulk_release.json` تصور الميمات مع
لوحات للحسابات حسب اللاحقة وحجم الدفع ونسب الدفع
reussite/echec des soumissions. مرشح اللوحة على أساس `release` من أجل ذلك
يمكن للمراجعين التركيز على تنفيذ ملف CSV واحد فقط.

## 6. التحقق من الصحة وأوضاع التحقق- **تطبيع التسميات:** تم تطبيع الإدخالات مع Python IDNA plus
  الأحرف الصغيرة ومرشحات الأحرف Norm v1. التسميات غير صالحة صدى
  avant tout appel reseau.
- **الأرقام القياسية:** معرفات اللاحقة وسنوات الفصل الدراسي وتلميحات التسعير مناسبة
  الباقي في البورنيس `u16` و`u8`. يتم قبول أبطال الدفع
  جميع العناصر العشرية أو السداسية التي تحتوي على `i64::MAX`.
- **تحليل البيانات الوصفية أو الإدارة:** يتم تحليل JSON المضمن مباشرة؛ ليه
  المراجع إلى الملفات هي قرارات تتعلق بوضع ملف CSV.
  تؤدي بيانات التعريف غير المهمة إلى خطأ في التحقق من الصحة.
- **وحدات التحكم:** الخلايا تحترم `--default-controllers`. فورنيسيز
  القوائم الصريحة (على سبيل المثال `i105...;i105...`) عند تفويضها
  الجهات الفاعلة غير المالك.

الإشارات هي إشارات بأرقام الخطوط السياقية (على سبيل المثال
`error: row 12 term_years must be between 1 and 255`). تم فرز النص مع لو
الكود `1` في أخطاء التحقق من الصحة و`2` عند إغلاق شريط CSV.

## 7. الاختبارات والمصدر- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` يغطي تحليل ملف CSV،
  الانبعاث NDJSON وحوكمة الإنفاذ وأنظمة التوصيل CLI أو Torii.
- المساعد هو Python Pur (إضافة الاعتماد الإضافي) وجزء من الدورة
  `python3` متاح. يستمر تاريخ الالتزامات في كوت دي لا
  CLI في المستودع الرئيسي لإعادة الإنتاج.

من أجل عمليات الإنتاج، قم بإضافة البيان العام والحزمة NDJSON إلى
تذكرة المسجل تمكن المضيفين من تجديد الحمولات بالضبط
soumis a Torii.