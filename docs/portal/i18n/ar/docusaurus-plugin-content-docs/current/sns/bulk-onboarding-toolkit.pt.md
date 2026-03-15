---
lang: ar
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` للمشغلين الخارجيين
قم بتوجيه SN-3b مباشرة إلى المستودع.
:::

# مجموعة أدوات الإعداد الشامل لـ SNS (SN-3b)

**مرجع خريطة الطريق:** SN-3b "أدوات الإعداد المجمعة"  
**أرتيفاتوس:** `scripts/sns_bulk_onboard.py`، `scripts/tests/test_sns_bulk_onboard.py`،
`docs/portal/scripts/sns_bulk_release.sh`

يقوم المسجلون بشكل متكرر بإعداد مئات من السجلات `.sora` ou
`.nexus` مع رسائل الموافقة على الإدارة وقضايا التسوية. مونتار
الحمولات النافعة JSON يدويًا أو إعادة تنفيذها على CLI بالتصعيد، بما في ذلك إدخال SN-3b
منشئ حتمية ملف CSV لـ Norito الذي يقوم بإعداد الاستراتيجيات
`RegisterNameRequestV1` لـ Torii أو لـ CLI. يا مساعد فاليدا كادا لينها
مع المقدمة، قم بإصدار بيان متفق عليه بمقدار JSON المحدد
البحث عن خط اختياري، ويمكنه إرسال الحمولات تلقائيًا في نفس الوقت
تسجيل الإيصالات التدريبية للمستمعين.

## 1. إسكيما CSV

يقوم المحلل اللغوي بتتبع الخط التالي (الترتيب والمرونة):| كولونا | أوبريجاتوريو | وصف |
|--------|------------|-----------|
| `label` | سيم | تسمية الطلب (حالة مختلطة، وهي عبارة عن أداة طبيعية تتوافق مع المعيار v1 e UTS-46). |
| `suffix_id` | سيم | المعرف الرقمي اللاحق (عشري أو `0x` سداسي عشري). |
| `owner` | سيم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | سيم | انتيرو `1..=255`. |
| `payment_asset_id` | سيم | قضية التسوية (على سبيل المثال `xor#sora`). |
| `payment_gross` / `payment_net` | سيم | إن Inteiros لا تمثل سوى وحدات وطنية ذات أهمية. |
| `settlement_tx` | سيم | تقوم قيمة JSON أو سلسلة حرفية بالفصل عن عملية الدفع أو التجزئة. |
| `payment_payer` | سيم | معرف الحساب الذي يسمح بالدفع. |
| `payment_signature` | سيم | JSON أو سلسلة صراع حرفي لإثبات اغتيال المضيف أو المضيف. |
| `controllers` | اختياري | قائمة منفصلة بواسطة جسر عذراء أو عذراء جهاز التحكم في بطانة الرحم. Padrao `[owner]` عندما يتم حذفه. |
| `metadata` | اختياري | يوفر JSON المضمن أو `@path/to/file.json` تلميحات المحلل، وسجلات TXT، وما إلى ذلك. Padrao `{}`. |
| `governance` | اختياري | JSON مضمّن أو `@path` متوافق مع `GovernanceHookV1`. `--require-governance` exige esta coluna. |يمكن لأي عمود أن يشير إلى ملف خارجي بادئ أو قيمة الهاتف مع `@`.
الكاميرون لها حلول مرتبطة بملف CSV.

## 2. المنفذ أو المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

العمليات الرئيسية:

- `--require-governance` يحتوي على خطاف تحكم (يستخدم للفقرة
  leiloes premium أو السمات المحجوزة).
- `--default-controllers {owner,none}` تقرر ما إذا كانت وحدات التحكم vazias
  فولتام لمالك كونتا.
- `--controllers-column`، `--metadata-column`، و`--governance-column` المسموح به
  قم بإعادة تسمية Colunas Opcionais Ao Trabalhar com للصادرات في المنبع.

في حالة النجاح أو النص المرسوم للبيان المتفق عليه:

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

Se `--ndjson` for necido، كل `RegisterNameRequestV1` يشمل الكتابة والكتابه أيضًا
مستند JSON ذو خط فريد حتى تتمكن الأجهزة من إرسال الطلبات
مباشرة إلى Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. يُرسل تلقائيًا

### 3.1 مودو Torii ريست

`--submit-torii-url` محدد أكثر من `--submit-token` أو `--submit-token-file`
لإرسال كل مدخل للبيان مباشرة إلى Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- يا مساعد ابعث um `POST /v1/sns/registrations` للطلب والإجهاض بدون أول
  خطأ HTTP. كما تم الرد على استفسارك في السجل كتسجيل NDJSON.
- `--poll-status` إعادة استشارة `/v1/sns/registrations/{selector}` بعد كل إرسال
  (ate `--poll-attempts`, default 5) لتأكيد رؤية السجل.
  Forneca `--suffix-map` (JSON de `suffix_id` للقيم "اللاحقة") من أجل ذلك
  تشتق الأدوات حرفيًا `{label}.{suffix}` للاقتراع.
- الضبط: `--submit-timeout`، `--poll-attempts`، e `--poll-interval`.

### 3.2 مودو إيروها CLI

لتدوير كل مدخل للبيان على CLI، أو الطريق إلى المسار الثنائي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- وحدات التحكم يمكن إدخالها `Account` (`controller_type.kind = "Account"`)
  من أجل ظهور واجهة سطر الأوامر (CLI) في الوقت الحالي حتى تعرض وحدات التحكم القائمة على البيانات.
- كتل البيانات الوصفية والحوكمة، محفورة في المحفوظات المؤقتة حسب الطلب
  و encaminhados ل `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout وstderr da CLI، أكثر من رموز السعادة، المسجلة أيضًا؛ codigos nao
  صفر إجهاض للتنفيذ.

يمكن استخدام طرق الإرسال المتعددة (Torii e CLI) للتحقق
تقوم عمليات النشر بالتسجيل أو البحث عن إجراءات احتياطية.

### 3.3 وصفات الإرسال

عند `--submission-log <path>` والطلب، قم بإدخال البرنامج النصي الملحق NDJSON الذي
كابتورام:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```تشتمل الإجابات Torii على العديد من المجالات التدريبية الإضافية
`NameRecordV1` أو `RegisterNameResponseV1` (على سبيل المثال `record_status`،
`record_pricing_class`، `record_owner`، `record_expires_at_ms`،
`registry_event_version`، `suffix_id`، `label`) للوحات المعلومات وعلاقاتها
يمكن للإدارة تحليل أو تحليل نص الكتاب بشكل خاص. سجل المرفق
باعتبارها تذاكر مسجلة جنبًا إلى جنب مع بيان الأدلة لإعادة إنتاجها.

## 4. إطلاق البوابة تلقائيًا

Jobs de CI e do Portal chamam `docs/portal/scripts/sns_bulk_release.sh`, que
تغليف أو مساعد أو أرمازينا أرتيفاتوس سوب
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

يا البرنامج النصي:

1. تم إنشاء `registrations.manifest.json` و`registrations.ndjson` والنسخ إلى ملف CSV
   الأصلي لمدير الإصدار.
2. قم بإرسال البيان باستخدام Torii e/ou إلى CLI (عند التكوين)، مرحب به
   `submissions.log` مع الدروس المستفادة أعلاه.
3. قم بإصدار `summary.json` لتنزيل الإصدار (الكاميرات، عنوان URL الخاص بـ Torii، التسجيل من
   CLI، الطابع الزمني) حتى تتمكن البوابة التلقائية من إرسال حزمة إلى o
   تخزين القطع الأثرية.
4. Produz `metrics.prom` (التجاوز عبر `--metrics`) يتنافس مع المنافسين المتوافقين
   com Prometheus لإجمالي الطلبات وتوزيع اللاحقات وإجمالي الأصول
   نتائج التقديم. أو JSON de resumo aponta لهذا الملف.يتم ببساطة حفظ سير العمل أو إصدار مدير باعتباره قطعة مصطنعة فريدة من نوعها،
ما الذي يجب أن نتحدث عنه الآن وما هو الحكم الدقيق للمستمعين.

## 5. لوحات المعلومات والقياس عن بعد

يعرض ملف المقاييس الجيدة لـ `sns_bulk_release.sh` كسلسلة متسلسلة:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

الشحن `metrics.prom` ليس بجانبك من Prometheus (على سبيل المثال عبر Promtail ou
مجموعة مستوردة) لتوفير المسجلين والمضيفين وشركاء الإدارة
الحد الأقصى للتقدم الشامل. يا كوادرو Grafana
`dashboards/grafana/sns_bulk_release.json` تصور نظام البيانات مع الألم
للعدوى باللاحقة وحجم الدفع ونسب النجاح/الفشل
يخضع. مرشح رباعي لـ `release` ليتمكن المدققون من التركيز على أحد
تنفيذ واحد لملف CSV.

## 6. التحقق من صحة طريقة الصيد- **عنوان التسمية:** يتم إدخاله بشكل طبيعي مع Python IDNA أكثر
  الأحرف الصغيرة ومرشحات الأحرف Norm v1. التسميات غير صالحة فالهام السريع
  قبل كل شيء، اتصل بالأحمر.
- **الدرابزين الرقمي:** معرفات لاحقة، وسنوات الفصل الدراسي، وتلميحات التسعير
  في حدود `u16` و`u8`. مجال الدفع العشري الداخلي
  أو أكلت سداسيًا `i64::MAX`.
- **تحليل البيانات الوصفية أو الإدارة:** JSON مضمّن وتحليله مباشرة؛
  المراجع والملفات ذات الحلول المتعلقة بموقع ملف CSV. البيانات الوصفية
  لا يوجد خطأ في المنتج.
- **وحدات التحكم:** شاشات بيضاء قابلة للاسترداد `--default-controllers`. فورنيكا
  القوائم الصريحة (على سبيل المثال `i105...;i105...`) لتفويضها إلى المالك.

Falhas sao Reportadas com numeros de linha contextuais (على سبيل المثال
`error: row 12 term_years must be between 1 and 255`). يا سكريبت ساي كوم كوديجو
`1` في أخطاء التحقق و`2` عندما يكون ملف CSV موجودًا.

## 7. الخصية وإجراءاتها

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` تحليل كوبر CSV،
  إرسال NDJSON وإنفاذ الحوكمة وإرشادات الإرسال على CLI أو Torii.
- مساعد بايثون النقي (ليس له تبعيات إضافية) والتنقل في أي مكان
  تم توفير `python3`. تاريخ ارتكاب الجرائم ومساراتها معًا
  CLI لا يحتوي على مستودع رئيسي لإعادة الإنتاج.لتشغيل المنتج، قم بتنفيذ الملحق أو البيان الموجه أو حزمة NDJSON إلى التذكرة
المسجل حتى يتمكن المضيفون من إعادة إنتاج الحمولات الإضافية التي يتم تقديمها في المنتدى
submetidos ao Torii.