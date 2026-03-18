---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : ملخص ملاحظات منتصف W0
sidebar_label : ملاحظات W0 (منتصف)
description: نقاط تحقق منتصف المرحلة، النتائج، وبنود العمل لموجة المعاينة لمشرفي النواة.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W0 - مشرفو النواة |
| تاريخ الملخص | 2025-03-27 |
| نافذة المراجعة | 2025-03-25 -> 2025-04-08 |
| المشاركون | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| وسم الاثر | `preview-2025-03-24` |

## ابرز النقاط

1. **Somme de contrôle** - اكد جميع المراجعين ان `scripts/preview_verify.sh`
   نجح مع زوج الوصف/الارشيف المشترك. لم تتطلب اي تجاوزات يدوية.
2. **ملاحظات التنقل** - تم تسجيل مشكلتين بسيطتين في ترتيب الشريط الجانبي
   (`docs-preview/w0 #1-#2`). كلتاهما محالتان الى Docs/DevRel ولا تعرقلان
   الموجة.
3. **Récupérer le runbook pour SoraFS** - Télécharger sorafs-ops-01 pour le mettre à jour
   `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. تم فتح مشكلة
   متابعة؛ Il s'agit de W1.
4. **مراجعة القياس** - pour observabilité-01 par `docs.preview.integrity`,
   `TryItProxyErrors` et Try-it avant de partir لم تنطلق تنبيهات.

## بنود العمل| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W0-A1 | Vous pouvez utiliser le portail devportal pour le téléchargement (`preview-invite-*` مجمعة). | Docs-core-01 | مكتمل - الشريط الجانبي يعرض الان وثائق المراجعين بشكل متجاور (`docs/portal/sidebars.js`). |
| W0-A2 | Vous devez utiliser `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | مكتمل - كل runbook يشير الان الى الاخر حتى يرى المشغلون الدليلين اثناء عمليات déploiement. |
| W0-A3 | مشاركة لقطات القياس + حزمة الاستعلامات مع متتبع الحوكمة. | Observabilité-01 | مكتمل - الحزمة مرفقة بـ `DOCS-SORA-Preview-W0`. |

## ملخص الختام (2025-04-08)

- اكد جميع المراجعين الخمسة الاكتمال، ونظفوا البناءات المحلية، وغادروا نافذة
  المعاينة؛ Vous êtes en contact avec `DOCS-SORA-Preview-W0`.
- لم تقع حوادث او تنبيهات خلال الموجة؛ بقيت لوحات القياس خضراء طوال الفترة.
- تم تنفيذ اجراءات التنقل + الروابط المتقاطعة (W0-A1/A2) et في الوثائق
  اعلاه؛ وادلة القياس (W0-A3) مرفقة بالمتتبع.
- تم ارشفة حزمة الادلة: لقطات قياس الشاشة، تاكيدات الدعوة، وهذا الملخص مرتبط
  في تذكرة المتتبع.

## الخطوات التالية

- تنفيذ بنود عمل W0 قبل فتح W1.
- La machine à sous et la mise en scène sont également disponibles pour les joueurs.
  Il s'agit d'un flux d'invitation d'aperçu] (../../preview-invite-flow.md).

_هذا الملخص مرتبط من [aperçu des invitations](../../preview-invite-tracker.md) من اجل
الحفاظ على قابلية تتبع خارطة طريق DOCS-SORA._