---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w0-resumen
título: ملخص ملاحظات منتصف W0
sidebar_label: ملاحظات W0 (منتصف)
descripción: نقاط تحقق منتصف المرحلة، النتائج، وبنود العمل لموجة المعاينة لمشرفي النواة.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W0 - مشرفو النواة |
| تاريخ الملخص | 2025-03-27 |
| نافذة المراجعة | 2025-03-25 -> 2025-04-08 |
| المشاركون | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidad-01 |
| وسم الاثر | `preview-2025-03-24` |

## ابرز النقاط

1. **سير عمل checksum** - اكد جميع المراجعين ان `scripts/preview_verify.sh`
   نجح مع زوج الوصف/الارشيف المشترك. لم تتطلب اي تجاوزات يدوية.
2. **ملاحظات التنقل** - تم تسجيل مشكلتين بسيطتين في ترتيب الشريط الجانبي
   (`docs-preview/w0 #1-#2`). كلتاهما محالتان الى Docs/DevRel y تعرقلان
   الموجة.
3. **تكافؤ runbook في SoraFS** - طلب sorafs-ops-01 روابط متقاطعة اوضح بين
   `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. تم فتح مشكلة
   متابعة؛ تعالج قبل W1.
4. **مراجعة القياس** - اكد observability-01 ان `docs.preview.integrity`,
   `TryItProxyErrors` Pruebas y pruebas Pruébelo لم تنطلق تنبيهات.

## بنود العمل| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W0-A1 | Utilice el portal de desarrollo para acceder a la aplicación (`preview-invite-*` مجمعة). | Documentos-core-01 | مكتمل - الشريط الجانبي يعرض الان وثائق المراجعين بشكل متجاور (`docs/portal/sidebars.js`). |
| W0-A2 | Utilice el conector `sorafs/orchestrator-ops` y `sorafs/multi-source-rollout`. | Sorafs-ops-01 | مكتمل - كل runbook يشير الان الى الاخر حتى يرى المشغلون الدليلين اثناء عمليات implementación. |
| W0-A3 | مشاركة لقطات القياس + حزمة الاستعلامات مع متتبع الحوكمة. | Observabilidad-01 | مكتمل - الحزمة مرفقة بـ `DOCS-SORA-Preview-W0`. |

## ملخص الختام (2025-04-08)

- اكد جميع المراجعين الخمسة الاكتمال، ونظفوا البناءات المحلية، وغادروا نافذة
  المعاينة؛ Este es el nombre del usuario `DOCS-SORA-Preview-W0`.
- لم تقع حوادث او تنبيهات خلال الموجة؛ بقيت لوحات القياس خضراء طوال الفترة.
- تم تنفيذ اجراءات التنقل + الروابط المتقاطعة (W0-A1/A2) y عكسها في الوثائق
  اعلاه؛ وادلة القياس (W0-A3) مرفقة بالمتتبع.
- تم ارشفة حزمة الادلة: لقطات قياس الشاشة، تاكيدات الدعوة، وهذا الملخص مرتبط
  في تذكرة المتتبع.

## الخطوات التالية

- تنفيذ بنود عمل W0 قبل فتح W1.
- الحصول على موافقة قانونية وحجز slot للـ puesta en escena الخاص بالوكيل، ثم اتباع خطوات
  التحضير لموجة الشركاء الموضحة في [vista previa del flujo de invitación](../../preview-invite-flow.md).

_هذا الملخص مرتبط من [vista previa del rastreador de invitaciones](../../preview-invite-tracker.md) من اجل
Haga clic en el enlace DOCS-SORA._