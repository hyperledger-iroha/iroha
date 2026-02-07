---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : ملخص ملاحظات وخروج W1
sidebar_label : comme W1
description: النتائج، الاجراءات، وادلة الخروج لموجة معاينة الشركاء ومتكاملي Torii.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - شركاء ومتكاملو Torii |
| نافذة الدعوة | 2025-04-12 -> 2025-04-26 |
| وسم الاثر | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |
| المشاركون | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## ابرز النقاط

1. **سير عمل checksum** - تحقق جميع المراجعين من descripteur/archive عبر `scripts/preview_verify.sh` ; تم حفظ السجلات بجانب اقرارات الدعوة.
2. **القياس** - pour les modèles `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` pour le téléphone portable لم تقع حوادث او صفحات تنبيه.
3. **ملاحظات الوثائق (`docs-preview/w1`)** - تم تسجيل ملاحظتين بسيطتين :
   - `docs-preview/w1 #1` : توضيح صياغة التنقل في قسم Essayez-le (تم الحل).
   - `docs-preview/w1 #2` : تحديث لقطة Essayez-le (تم الحل).
4. **Télécharger runbook** - Utiliser SoraFS et `multi-source-rollout` et `multi-source-rollout`. ملاحظات W0.

## بنود العمل| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W1-A1 | تحديث صياغة تنقل Essayez-le via `docs-preview/w1 #1`. | Docs-core-02 | ✅مكتمل (2025-04-18). |
| W1-A2 | Essayez-le Essayez-le via `docs-preview/w1 #2`. | Docs-core-03 | ✅ مكتمل (2025-04-19). |
| W1-A3 | Il y a des informations sur la feuille de route/le statut. | Responsable Docs/DevRel | ✅ مكتمل (راجع المتتبع et status.md). |

## ملخص الخروج (2025-04-26)

- اكد جميع المراجعين الثمانية الاكتمال خلال ساعات المكتب الاخيرة، ونظفوا الاثار المحلية، وتم سحب صلاحياتهم.
- بقيت القياسات خضراء حتى الخروج؛ Les informations fournies par `DOCS-SORA-Preview-W1`.
- تم تحديث سجل الدعوات باقرارات الخروج؛ حول المتتبع W1 الى 🈴 واضاف نقاط التحقق.
- حزمة الادلة (descripteur, سجل checksum, مخرجات sonde, نص وكيل Try it, لقطات القياس، ملخص الملاحظات) ارشفت تحت `artifacts/docs_preview/W1/`.

## الخطوات التالية

- تجهيز خطة apport المجتمعية لـ W2 (موافقة الحوكمة + تعديلات قالب الطلب).
- تحديث وسم اثر المعاينة لموجة W2 واعادة تشغيل سكربت preflight بمجرد تثبيت التواريخ.
- نقل النتائج المناسبة من W1 الى roadmap/status حتى تحصل الموجة المجتمعية على اخر التوجيهات.