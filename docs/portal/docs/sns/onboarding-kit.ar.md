---
lang: ar
direction: rtl
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-11-20T12:36:56.774738+00:00"
translation_last_reviewed: 2026-01-01
---

# مقاييس SNS وحزمة onboarding

يجمع بند خارطة الطريق **SN-8** وعدين:

1. نشر dashboards تعرض التسجيلات، التجديدات، ARPU، النزاعات، ونوافذ التجميد لـ `.sora` و `.nexus` و `.dao`.
2. شحن حزمة onboarding كي يتمكن registrars و stewards من ربط DNS والتسعير و APIs بشكل متسق قبل اطلاق اي suffix.

تعكس هذه الصفحة الاصدار المصدر
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
حتى يتبع المراجعون الخارجيون نفس الاجراءات.

## 1. حزمة المقاييس

### Dashboard Grafana و embed البوابة

- استورد `dashboards/grafana/sns_suffix_analytics.json` الى Grafana (او اي host تحليلي اخر)
  عبر API القياسي:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- نفس JSON يغذي iframe لهذه الصفحة (راجع **SNS KPI Dashboard**).
  عند تحديث dashboard، شغل
  `npm run build && npm run serve-verified-preview` داخل `docs/portal` لتاكيد
  بقاء Grafana و embed متزامنين.

### Panels و evidence

| Panel | Metrics | Evidence حوكمة |
|-------|---------|-----------------|
| Registrations & renewals | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput لكل suffix + تتبع SLA. |
| ARPU / net units | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | يمكن للمالية مطابقة manifests مع الايرادات. |
| Disputes & freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | يعرض freezes النشطة ومعدل التحكيم وحمل guardian. |
| SLA / error rates | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | يبرز تراجعات API قبل تاثيرها على العملاء. |
| Bulk manifest tracker | `sns_bulk_release_manifest_total`, metrics الدفع مع labels `manifest_id` | يربط drops CSV بتذاكر settlement. |

قم بتصدير PDF/CSV من Grafana (او iframe) اثناء مراجعة KPI الشهرية
وارفقه بمدخل الملحق المناسب في
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. يسجل stewards ايضا SHA-256
للحزمة المصدرة تحت `docs/source/sns/reports/` (مثلا
`steward_scorecard_2026q1.md`) حتى تتمكن عمليات التدقيق من اعادة مسار evidence.

### اتتمة الملاحق

قم بانشاء ملفات الملحق مباشرة من تصدير dashboard حتى يحصل المراجعون
على ملخص متسق:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- يقوم helper بعمل hash للتصدير ويلتقط UID/tags/عدد panels ويكتب ملحق Markdown تحت
  `docs/source/sns/reports/.<suffix>/<cycle>.md` (انظر مثال `.sora/2026-03`).
- `--dashboard-artifact` ينسخ التصدير الى
  `artifacts/sns/regulatory/<suffix>/<cycle>/` حتى يشير الملحق الى مسار evidence القياسي؛
  استخدم `--dashboard-label` فقط اذا احتجت ارشيفا خارجيا.
- `--regulatory-entry` يشير الى memo تنظيمي. يقوم helper بادراج (او استبدال)
  كتلة `KPI Dashboard Annex` التي تسجل مسار الملحق و artefact وال digest والطابع الزمني
  للحفاظ على تزامن evidence.
- `--portal-entry` يبقي نسخة Docusaurus
  (`docs/portal/docs/sns/regulatory/*.md`) متزامنة حتى لا يقارن المراجعون
  ملخصات ملحقات منفصلة يدويا.
- اذا تخطيت `--regulatory-entry`/`--portal-entry`، ارفق الملف يدويا
  وارفع ايضا snapshots PDF/CSV من Grafana.
- للتصديرات المتكررة، ضع ازواج suffix/cycle في
  `docs/source/sns/regulatory/annex_jobs.json` وشغل
  `python3 scripts/run_sns_annex_jobs.py --verbose`. يقوم helper بالمرور على كل ادخال،
  ينسخ التصدير (افتراضيا `dashboards/grafana/sns_suffix_analytics.json` اذا لم يحدد)
  ويحدث كتلة الملحق في كل memo تنظيمي (وايضا memo البوابة عند توفره) بتمرير واحد.
- شغل `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (او `make check-sns-annex`) لاثبات ان قائمة المهام مرتبة وخالية من التكرار، وان كل memo يحتوي على علامة `sns-annex`، وان stub الخاص بالملحق موجود. يكتب helper `artifacts/sns/annex_schedule_summary.json` بجانب ملخصات locale/hash المستخدمة في حزم الحوكمة.
هذا يزيل خطوات النسخ/اللصق اليدوية ويحافظ على اتساق ملحق SN-8 مع الحماية من drift في الجدول والعلامات والترجمة داخل CI.

## 2. مكونات حزمة onboarding

### ربط suffix

- مخطط registry + قواعد selector:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  و [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- مساعد هيكل DNS:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  مع تدفق التدريب في
  [runbook gateway/DNS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- لكل اطلاق registrar، احفظ ملاحظة قصيرة تحت
  `docs/source/sns/reports/` تلخص عينات selector وادلة GAR و DNS hashes.

### ورقة اسعار سريعة

| طول label | الرسوم الاساسية (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

معاملات suffix: `.sora` = 1.0x، `.nexus` = 0.8x، `.dao` = 1.3x.  
مضاعفات المدة: 2-year -5%، 5-year -12%; grace window = 30 days، redemption
= 60 days (20% fee، min $5، max $200). سجل الاستثناءات المتفق عليها في
تذكرة registrar.

### مزادات premium مقابل renewals

1. **Premium pool** -- commit/reveal بعطاءات مختومة (SN-3). تتبع العطاءات عبر
   `sns_premium_commit_total` وانشر manifest تحت
   `docs/source/sns/reports/`.
2. **Dutch reopen** -- بعد انتهاء grace + redemption، ابدأ بيع Dutch لمدة 7-day
   بسعر 10x ينخفض 15% يوميا. ضع `manifest_id` على manifests حتى يظهر التقدم في
   dashboard.
3. **Renewals** -- راقب `sns_registrar_status_total{resolver="renewal"}` والتقط
   checklist autorenew (اشعارات، SLA، rails دفع احتياطية) داخل تذكرة registrar.

### APIs المطورين والاتتمة

- عقود API: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- helper bulk ومخطط CSV:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- امر مثال:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

ضم manifest ID (مخرجات `--submission-log`) الى فلتر KPI dashboard حتى تتمكن
المالية من مطابقة لوحات الايرادات لكل release.

### حزمة evidence

1. تذكرة registrar مع جهات الاتصال ونطاق suffix و rails الدفع.
2. evidence DNS/resolver (zonefile skeletons + ادلة GAR).
3. ورقة اسعار + overrides معتمدة من الحوكمة.
4. artefacts لاختبار API/CLI (امثلة `curl`، سجلات CLI).
5. لقطة KPI dashboard + تصدير CSV مرفق بالملحق الشهري.

## 3. checklist الاطلاق

| خطوة | Owner | Artefact |
|------|-------|----------|
| Dashboard مستورد | Product Analytics | استجابة API Grafana + UID dashboard |
| تحقق embed البوابة | Docs/DevRel | سجلات `npm run build` + لقطة preview |
| اكتمال تدريب DNS | Networking/Ops | مخرجات `sns_zonefile_skeleton.py` + سجل runbook |
| Dry run لاتمتة registrar | Registrar Eng | سجل submissions لـ `sns_bulk_onboard.py` |
| توثيق evidence الحوكمة | Governance Council | رابط الملحق + SHA-256 للتصدير |

اكمل checklist قبل تفعيل registrar او suffix. الحزمة الموقعة تفتح بوابة SN-8 وتمنح
المدققين مرجعا واحدا عند مراجعة اطلاقات marketplace.
