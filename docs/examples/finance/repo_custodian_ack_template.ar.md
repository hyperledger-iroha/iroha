---
lang: ar
direction: rtl
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6609b9628186b58c0441d1ca1594a3030b7f7d81bb2df9c36af3a9d65cdd963d
source_last_modified: "2025-11-15T20:04:53.140162+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/finance/repo_custodian_ack_template.md -->

# قالب اقرار امين الحفظ للrepo

استخدم هذا القالب عندما يشير repo (ثنائي او ثلاثي الاطراف) الى امين حفظ عبر `RepoAgreement::custodian`. الهدف هو تسجيل SLA الحفظ وحسابات التوجيه وجهات الاتصال للتدريبات قبل انتقال الاصول. انسخ القالب الى دليل الادلة (على سبيل المثال
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`)، املأ القوالب، واحتسب الهاش كجزء من حزمة الحوكمة الموصوفة في
`docs/source/finance/repo_ops.md` القسم 2.8.

## 1. البيانات الوصفية

| الحقل | القيمة |
|-------|-------|
| معرف الاتفاق | `<repo-yyMMdd-XX>` |
| معرف حساب امين الحفظ | `<ih58...>` |
| اعد بواسطة / التاريخ | `<custodian ops lead>` |
| جهات اتصال desk المعترف بها | `<desk lead + counterparty>` |
| دليل الادلة | ``artifacts/finance/repo/<slug>/`` |

## 2. نطاق الحفظ

- **تعريفات الضمان المستلمة:** `<list of asset definition ids>`
- **عملة cash leg / مسار التسوية:** `<xor#sora / other>`
- **نافذة الحفظ:** `<start/end timestamps or SLA summary>`
- **تعليمات دائمة:** `<hash + path to standing instruction document>`
- **متطلبات الاتمتة:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. التوجيه والمراقبة

| البند | القيمة |
|------|-------|
| محفظة الحفظ / حساب الدفتر | `<asset ids or ledger path>` |
| قناة المراقبة | `<Slack/phone/on-call rotation>` |
| جهة اتصال drill | `<primary + backup>` |
| التنبيهات المطلوبة | `<PagerDuty service, Grafana board, etc.>` |

## 4. البيانات

1. *جاهزية الحفظ:* "راجعنا payload `repo initiate` المرحلي بالمعرفات اعلاه ونستعد لقبول الضمان وفق SLA المدرج في القسم 2."
2. *التزام rollback:* "سننفذ playbook rollback المذكور اعلاه اذا وجهنا قائد الحادث، وسنقدم سجلات CLI مع hashes في
   `governance/drills/<timestamp>.log`."
3. *الاحتفاظ بالادلة:* "سنحتفظ بالاقرار والتعليمات الدائمة وسجلات CLI لمدة لا تقل عن `<duration>` ونقدمها لمجلس المالية عند الطلب."

وقع ادناه (التواقيع الالكترونية مقبولة عند تمريرها عبر متتبع الحوكمة).

| الاسم | الدور | التوقيع / التاريخ |
|------|------|------------------|
| `<custodian ops lead>` | مشغل امين الحفظ | `<signature>` |
| `<desk lead>` | Desk | `<signature>` |
| `<counterparty>` | Counterparty | `<signature>` |

> بعد التوقيع، احسب hash الملف (مثال: `sha256sum custodian_ack_<cust>.md`) وسجل digest في جدول حزمة الحوكمة حتى يتمكن المراجعون من التحقق من بايتات الاقرار المشار اليها اثناء التصويت.

</div>
