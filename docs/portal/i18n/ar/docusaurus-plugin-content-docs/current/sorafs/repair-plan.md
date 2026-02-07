---
id: repair-plan
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
المرايا `docs/source/sorafs_repair_plan.md`. حافظ على مزامنة كلا الإصدارين حتى يتم إيقاف مجموعة Sphinx.
:::

## دورة حياة قرار الحوكمة
1. تقوم الإصلاحات المتصاعدة بإنشاء مسودة اقتراح مائلة وفتح نافذة النزاع.
2. يقوم ناخبو الحوكمة بتقديم أصوات الموافقة/الرفض خلال نافذة النزاع.
3. في `escalated_at_unix + dispute_window_secs` يتم حساب القرار بشكل حتمي: الحد الأدنى من الناخبين، والموافقات تتجاوز الرفض، ونسبة الموافقة تلبي عتبة النصاب القانوني.
4. القرارات المعتمدة تفتح باب الاستئناف. الاستئنافات المسجلة قبل `approved_at_unix + appeal_window_secs` تضع علامة على القرار باعتباره مستأنفًا.
5. تنطبق الحدود القصوى للعقوبات على جميع المقترحات؛ يتم رفض الطلبات التي تتجاوز الحد الأقصى.

## سياسة تصعيد الحوكمة
يتم الحصول على سياسة التصعيد من `governance.sorafs_repair_escalation` في `iroha_config` ويتم فرضها على كل مقترح شرطة مائلة للإصلاح.

| الإعداد | الافتراضي | معنى |
|---------|--------|---------|
| `quorum_bps` | 6667 | الحد الأدنى لنسبة الموافقة (نقاط الأساس) بين الأصوات التي تم فرزها. |
| `minimum_voters` | 3 | الحد الأدنى لعدد الناخبين المتميزين المطلوب لاتخاذ القرار. |
| `dispute_window_secs` | 86400 | الوقت بعد التصعيد قبل الانتهاء من التصويت (بالثواني). |
| `appeal_window_secs` | 604800 | الوقت الذي يلي الموافقة والذي يتم خلاله قبول الطعون (ثواني). |
| `max_penalty_nano` | 1,000,000,000 | الحد الأقصى لعقوبة القطع المسموح به لتصعيد الإصلاح (nano-XOR). |

- المقترحات التي تم إنشاؤها بواسطة المجدول هي `max_penalty_nano`؛ يتم رفض طلبات المدقق التي تتجاوز الحد الأقصى.
- يتم تخزين سجلات التصويت في `repair_state.to` بترتيب محدد (فرز `voter_id`) بحيث تستمد جميع العقد نفس الطابع الزمني للقرار والنتيجة.