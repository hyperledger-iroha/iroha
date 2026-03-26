---
lang: fr
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
يعكس `docs/source/sns/bulk_onboarding_toolkit.md` حتى يرى المشغلون الخارجيون
Le SN-3b est utilisé pour le nettoyage.
:::

# عدة ادوات التهيئة بالجملة لـ SNS (SN-3b)

**مرجع خارطة الطريق :** SN-3b "Outil d'intégration en masse"  
**اثار:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

غالبا ما يجهز المسجلون الكبار مئات تسجيلات `.sora` et `.nexus` مع نفس
موافقات الحوكمة وقنوات التسوية. Charges utiles JSON disponibles et mises à jour
CLI pour le générateur SN-3b et CSV pour Norito
`RegisterNameRequestV1` pour Torii et CLI. يتحقق المساعد من كل صف مسبقا،
Il s'agit d'un manifest مجمع et JSON اختياري مفصول باسطر، ويمكنه ارسال
payloads تلقائيا مع تسجيل ايصالات منظمة لاغراض التدقيق.

## 1. Utiliser CSV

يتطلب المحلل صف العناوين التالي (الترتيب مرن):| العمود | مطلوب | الوصف |
|--------|-------|-------|
| `label` | نعم | Il s'agit de la norme v1 et UTS-46. |
| `suffix_id` | نعم | معرف لاحقة رقمي (عشري او `0x` hex). |
| `owner` | نعم | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | نعم | Consultez `1..=255`. |
| `payment_asset_id` | نعم | اصل التسوية (مثل `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | نعم | اعداد صحيحة غير موقعة تمثل وحدات الاصل. |
| `settlement_tx` | نعم | JSON est également un outil de hachage. |
| `payment_payer` | نعم | AccountId est également disponible. |
| `payment_signature` | نعم | JSON est également un steward et un steward. |
| `controllers` | اختياري | Il s'agit d'un contrôleur et d'un contrôleur. الافتراضي `[owner]` عند الحذف. |
| `metadata` | اختياري | JSON en ligne et `@path/to/file.json` sont un résolveur de problèmes et un résolveur TXT. Article `{}`. |
| `governance` | اختياري | JSON en ligne et `@path` ou `GovernanceHookV1`. `--require-governance` يفرض هذا العمود. |

يمكن لاي عمود الاشارة الى ملف خارجي عبر بادئة قيمة الخلية بـ `@`.
Il s'agit d'un fichier CSV.

## 2. تشغيل المساعد

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

خيارات رئيسية:- `--require-governance` crochet pour crochet (pour les produits premium et
  التعيينات المحجوزة).
- `--default-controllers {owner,none}` pour les contrôleurs
  الفارغة تعود الى حساب propriétaire.
- `--controllers-column`, `--metadata-column` et `--governance-column`.
  Il y a beaucoup d'exportations en Chine.

عند النجاح يكتب السكربت manifeste مجمع :

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

Vous pouvez utiliser `--ndjson` pour `RegisterNameRequestV1` avec JSON et
حتى تتمكن الاتمتة من بث الطلبات مباشرة الى Torii :

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

### 3.1 et Torii REST

حدد `--submit-torii-url` pour `--submit-token` et `--submit-token-file` pour
ادخال في manifest مباشرة الى Torii :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- يصدر المساعد `POST /v1/sns/names` لكل طلب ويتوقف عند اول خطا HTTP.
  Il s'agit de NDJSON.
- `--poll-status` est compatible avec `/v1/sns/names/{namespace}/{literal}`.
  ارسال (حتى `--poll-attempts`, الافتراضي 5) لتاكيد ظهور السجل. et
  `--suffix-map` (JSON correspond à `suffix_id` comme "suffixe") dans la version actuelle
  اشتقاق لواحق `{label}.{suffix}` عند sondage.
- Noms d'utilisateur : `--submit-timeout`, `--poll-attempts` et `--poll-interval`.

### 3.2 par iroha CLI

لتمرير كل ادخال في manifest عبر CLI، وفر مسار الملف التنفيذي :

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```- يجب ان تكون contrôleurs من نوع `Account` (`controller_type.kind = "Account"`)
  La CLI est également disponible pour les contrôleurs.
- Il existe des blobs pour les métadonnées et la gouvernance et pour les autres utilisateurs.
  تمريرها الى `iroha sns register --metadata-json ... --governance-json ...`.
- يتم تسجيل stdout et stderr مع اكواد الخروج؛ الاكواد غير الصفرية توقف التشغيل.

يمكن تشغيل وضعي الارسال معا (Torii et CLI) لللتحقق المتقاطع من نشر المسجل او
لتمرين مسارات repli.

### 3.3 Paramètres d'installation

عند تمرير `--submission-log <path>` يضيف السكربت سجلات NDJSON تلتقط:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

تتضمن ردود Torii الناجحة حقولا منظمة مستخرجة من `NameRecordV1` او
`RegisterNameResponseV1` (pour `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) حتى تتمكن لوحات المتابعة وتقارير الحوكمة من تحليل السجل دون تفتيش
C'est vrai. ارفق هذا السجل مع تذكرة المسجل بجانب manifeste لاثبات قابل لاعادة
الانتاج.

## 4. اتـمتة اصدار بوابة الوثائق

تستدعي مهام CI والبوابة `docs/portal/scripts/sns_bulk_release.sh` الذي يلف
La description de `artifacts/sns/releases/<timestamp>/` :

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

السكربت:1. `registrations.manifest.json` et `registrations.ndjson` et CSV
   الى مجلد الاصدار.
2. Afficher le manifeste Torii et/ou CLI (عند التهيئة) et `submissions.log` مع
   الايصالات المنظمة اعلاه.
3. يصدر `summary.json` الذي يصف الاصدار (مسارات، عنوان Torii، مسار CLI،
   timestamp) لكي تتمكن اتـمتة البوابة من رفع الحزمة الى مخزن الاثار.
4. ينتج `metrics.prom` (override عبر `--metrics`) متضمنا عدادات متوافقة مع
   Prometheus لعدد الطلبات الاجمالي وتوزيع اللاحقات ومجاميع الاصل ونتائج الارسال.
   JSON est utilisé pour fonctionner.

Les flux de travail sont plus efficaces que les autres flux de travail
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

Pour `metrics.prom` side-car Prometheus (pour Promtail et Promtail)
مستورد دفعات) للحفاظ على توافق المسجلين وstewards وشركاء الحوكمة حول تقدم
الجملة. Pour Grafana `dashboards/grafana/sns_bulk_release.json` Mise à jour
البيانات مع لوحات لعدد الطلبات لكل لاحقة، حجم الدفع، ونسب نجاح/فشل الارسال.
Le fichier `release` est disponible dans le fichier CSV.
bien.

## 6. التحقق وحالات الفشل- **Étiquette étiquette :** يتم تطبيع الادخالات باستخدام Python IDNA en minuscules et en minuscules
  Norme v1. تفشل التسميات غير الصالحة بسرعة قبل اي اتصال شبكي.
- **Caractéristiques du produit :** Il y a également des identifiants de suffixe, des années de mandat et des conseils sur les prix.
  `u16` et `u8`. Utilisez le code hexadécimal `i64::MAX`.
- **Métadonnées avancées et gouvernance :** JSON inline مباشرة؛ ويتم حل
  Les fichiers PDF sont disponibles en CSV. métadonnées غير الكائن ينتج خطا تحقق.
- **Contrôleurs :** Fonctionnement de `--default-controllers`. قدم قوائم
  contrôleur صريحة (مثل `<i105-account-id>;<i105-account-id>`) عند التفويض لجهات غير المالك.

يتم الابلاغ عن الاخطاء مع ارقام صفوف سياقية (مثلا
`error: row 12 term_years must be between 1 and 255`). يخرج السكربت بالكود `1`
Il s'agit d'un fichier CSV et `2`.

## 7. الاختبار والاعتمادية

- يغطي `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` par CSV,
  Utilisez NDJSON pour utiliser la CLI et Torii.
- المساعد مكتوب ببايثون فقط (بدون تبعيات اضافية) ويعمل حيث يتوفر `python3`.
  Vous pouvez utiliser la CLI pour créer des liens.

للانتاج، ارفق manifest الناتج وحزمة NDJSON مع تذكرة المسجل حتى يتمكن stewards
Les charges utiles utilisées pour les charges utiles sont Torii.