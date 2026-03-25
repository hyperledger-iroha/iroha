---
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى المشغلون الخارجيون
نفس ارشادات SN-3b دون استنساخ المستودع.
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**مرجع خارطة الطريق:** SN-3b "Herramientas de incorporación masiva"  
**الاثار:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

غالبا ما يجهز المسجلون الكبار مئات تسجيلات `.sora` او `.nexus` مع نفس
موافقات الحوكمة وقنوات التسوية. Cargas útiles JSON يدويا او اعادة تشغيل
CLI para el constructor SN-3b creado mediante CSV con Norito
`RegisterNameRequestV1` a Torii y a CLI. يتحقق المساعد من كل صف مسبقا،
ويصدر كلا من manifest مجمع و JSON اختياري مفصول باسطر، ويمكنه ارسال
cargas útiles تلقائيا مع تسجيل ايصالات منظمة لاغراض التدقيق.

## 1. مخطط CSV

يتطلب المحلل صف العناوين التالي (الترتيب مرن):| العمود | مطلوب | الوصف |
|--------|-------|-------|
| `label` | نعم | التسمية المطلوبة (يقبل حالة مختلطة; الاداة تطبع حسب Norm v1 y UTS-46). |
| `suffix_id` | نعم | معرف لاحقة رقمي (عشري او `0x` hexadecimal). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | Aquí está `1..=255`. |
| `payment_asset_id` | نعم | Este es el caso (modelo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | اعداد صحيحة غير موقعة تمثل وحدات الاصل. |
| `settlement_tx` | نعم | قيمة JSON او سلسلة حرفية تصف معاملة الدفع او hash. |
| `payment_payer` | نعم | AccountId aquí. |
| `payment_signature` | نعم | JSON y el administrador y el administrador. |
| `controllers` | اختياري | قائمة مفصولة بفاصلة او فاصلة منقوطة لعناوين حسابات controlador. الافتراضي `[owner]` عند الحذف. |
| `metadata` | اختياري | JSON en línea y `@path/to/file.json` para solucionar problemas y archivos TXT. Nombre `{}`. |
| `governance` | اختياري | JSON en línea y `@path` y `GovernanceHookV1`. `--require-governance` يفرض هذا العمود. |

يمكن لاي عمود الاشارة الى ملف خارجي عبر بادئة قيمة الخلية بـ `@`.
يتم حل المسارات نسبة الى ملف CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

خيارات رئيسية:- `--require-governance` يرفض الصفوف بدون gancho حوكمة (مفيد لمزادات premium او
  التعيينات المحجوزة).
- `--default-controllers {owner,none}` يقرر ما اذا كانت خلايا controladores
  الفارغة تعود الى حساب propietario.
- `--controllers-column`, `--metadata-column` y `--governance-column`.
  باعادة تسمية الاعمدة الاختيارية عند العمل مع exportaciones خارجية.

عند النجاح يكتب السكربت manifiesto مجمع:

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

Aquí está el archivo `--ndjson`, el archivo `RegisterNameRequestV1` y el formato JSON y
حتى تمكن الاتمتة من بث الطلبات مباشرة الى Torii:

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

### 3.1 y Torii REST

حدد `--submit-torii-url` o `--submit-token` y `--submit-token-file` para el hogar
Aquí está el manifiesto de Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- يصدر المساعد `POST /v1/sns/names` لكل طلب ويتوقف عند اول خطا HTTP.
  تضاف الردود الى مسار السجل كسجلات NDJSON.
- `--poll-status` يعيد الاستعلام عن `/v1/sns/names/{namespace}/{literal}` بعد كل
  ارسال (حتى `--poll-attempts`, الافتراضي 5) لتاكيد ظهور السجل. وفر
  `--suffix-map` (JSON y `suffix_id` con "sufijo") para el usuario
  اشتقاق لواحق `{label}.{suffix}` عند sondeo.
- Nombres de usuario: `--submit-timeout`, `--poll-attempts` y `--poll-interval`.

### 3.2 y iroha CLI

لتمرير كل ادخال في manifest عبر CLI، وفر مسار الملف التنفيذي:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```- Estos son controladores de nombre `Account` (`controller_type.kind = "Account"`)
  La CLI requiere que los controladores funcionen correctamente.
- تكتب blobs الخاصة بـ metadatos y gobernanza في ملفات مؤقتة لكل طلب ويتم
  تمريرها الى `iroha sns register --metadata-json ... --governance-json ...`.
- يتم تسجيل stdout y stderr مع اكواد الخروج؛ الاكواد غير الصفرية توقف التشغيل.

يمكن تشغيل وضعي الارسال معا (Torii y CLI) للتحقق المتقاطع منشر المسجل او
لتمرين مسارات reserva.

### 3.3 ايصالات الارسال

El archivo `--submission-log <path>` contiene archivos NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تضمن ردود Torii الناجحة حقولا منظمة مستخرجة من `NameRecordV1` او
`RegisterNameResponseV1` (contra `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) حتى تمكن لوحات المتابعة وتقارير الحوكمة من تحليل السجل دون تفتيش
نص حر. ارفق هذا السجل مع تذكرة المسجل بجانب manifiesto لاثبات قابل لاعادة
الانتاج.

## 4. اتـمتة اصدار بوابة الوثائق

تستدعي مهام CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` الذي يلف
Fuentes de datos `artifacts/sns/releases/<timestamp>/`:

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

Artículos:1. يبني `registrations.manifest.json` y `registrations.ndjson` y CSV الاصلي
   الى مجلد الاصدار.
2. يرسل manifest عبر Torii y/او CLI (عند التهيئة), ويكتب `submissions.log` مع
   الايصالات المنظمة اعلاه.
3. يصدر `summary.json` الذي يصف الاصدار (المسارات، عنوان Torii, مسار CLI،
   marca de tiempo) لكي تتمكن اتـمتة البوابة من رفع الحزمة الى مخزن الاثار.
4. ينتج `metrics.prom` (anular عبر `--metrics`) متضمنا عدادات متوافقة مع
   Prometheus Para obtener más información, consulte el documento y consulte el documento.
   Utilice el formato JSON.

Flujos de trabajo de تقوم بارشفة مجلد الاصدار كاثر واحد، والذي يحتوي الان كل ما تحتاجه
الاعتمادات للتدقيق.

## 5. القياس ولوحات المتابعة

يعرض ملف المقاييس الناتج عن `sns_bulk_release.sh` السلاسل التالية:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

قم بتغذية `metrics.prom` الى sidecar Prometheus لديك (مثلا عبر Promtail او
مستورد دفعات) للحفاظ على توافق المسجلين وstewards وشركاء الحوكمة حول تقدم
الجملة. لوحة Grafana `dashboards/grafana/sns_bulk_release.json` تعرض نفس
البيانات مع لوحات لعدد الطلبات لكل لاحقة، حجم الدفع، ونسب نجاح/فشل الارسال.
تقوم اللوحة بالتصفية عبر `release` حتى يتمكن المدققون من التعمق في تشغيل CSV
Y.

## 6. التحقق وحالات الفشل- **etiqueta de la etiqueta:** يتم تطبيع الادخالات باستخدام Python IDNA con letras minúsculas
  Norma v1. تفشل التسميات غير الصالحة بسرعة قبل اي اتصال شبكي.
- **حواجز رقمية:** يجب ان تقع identificadores de sufijo y años de plazo y sugerencias de precios ضمن حدود
  `u16` y `u8`. تقبل حقول الدفع اعدادا عشرية او hex حتى `i64::MAX`.
- **تحليل metadatos y gobernanza:** يتم تحليل JSON en línea مباشرة؛ ويتم حل
  مراجع الملفات نسبة الى موقع CSV. metadatos غير الكائن ينتج خطا تحقق.
- **Controladores:** الخلايا الفارغة تلتزم بـ `--default-controllers`. قدم قوائم
  controlador صريحة (مثل `i105...;i105...`) عند التفويض لجهات غير المالك.

يتم الابلاغ عن الاخطاء مع ارقام صفوف سياقية (مثلا
`error: row 12 term_years must be between 1 and 255`). يخرج السكربت بالكود `1`
Verifique el archivo CSV y `2`.

## 7. الاختبار والاعتمادية

- Este es el archivo CSV `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py`.
  Conecte NDJSON y conecte la CLI a Torii.
- المساعد مكتوب ببايثون فقط (بدون تبعيات اضافية) ويعمل حيث يتوفر `python3`.
  يتم تتبع سجل الالتزامات بجانب CLI في المستودع الرئيسي للموثوقية.

للانتاج، ارفق manifiesto الناتج وحزمة NDJSON مع تذكرة المسجل حتى يتمكن mayordomos
من اعادة تشغيل الـ payloads الدقيقة التي تم ارسالها الى Torii.