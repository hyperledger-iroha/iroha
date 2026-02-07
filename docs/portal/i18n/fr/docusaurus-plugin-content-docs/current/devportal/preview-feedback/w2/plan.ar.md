---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : خطة admission المجتمعية W2
sidebar_label : par W2
description: القبول والموافقات وقائمة ادلة لمجموعة معاينة المجتمع.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - مراجعة المجتمع |
| نافذة الهدف | الربع الثالث 2025 الاسبوع 1 (مبدئي) |
| وسم الاثر (مخطط) | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |

## الاهداف

1. تعريف معايير apport المجتمعية وسير عمل التقييم.
2. الحصول على موافقة الحوكمة على القائمة المقترحة وملحق الاستخدام المقبول.
3. Utiliser la somme de contrôle et la somme de contrôle.
4. تجهيز وكيل Essayez-le واللوحات قبل ارسال الدعوات.

## تفصيل المهام| المعرف | المهمة | المالك | الاستحقاق | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Prise en charge de l'apport alimentaire (CoC) et prise en charge | Responsable Docs/DevRel | 2025-05-15 | ✅ مكتمل | L'admission a été effectuée pour `DOCS-SORA-Preview-W2` et a été publiée le 20/05/2025. |
| W2-P2 | تحديث قالب الطلب باسئلة خاصة بالمجتمع (الدوافع، الاتاحة، احتياجات التوطين) | Docs-core-01 | 2025-05-18 | ✅ مكتمل | ملف `docs/examples/docs_preview_request_template.md` يتضمن الان قسم Community, ومشار اليه في نموذج admission. |
| W2-P3 | تأمين موافقة الحوكمة على خطة apport (تصويت اجتماع + محاضر مسجلة) | Liaison gouvernance | 2025-05-22 | ✅ مكتمل | تم تمرير التصويت بالاجماع في 2025-05-20؛ La description de l'article est la suivante: `DOCS-SORA-Preview-W2`. |
| W2-P4 | جدولة staging لوكيل Essayez-le والتقاط القياس لنافذة W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ مكتمل | Le téléchargement `OPS-TRYIT-188` s'est produit le 2025-06-09 02:00-04:00 UTC؛ لقطات Grafana ارشفت مع التذكرة. |
| W2-P5 | Description du descripteur/somme de contrôle/sonde | Portail TL | 2025-06-07 | ✅ مكتمل | Publié `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` le 10/06/2025 La description est `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | تجميع قائمة دعوات المجتمع (<=25 مراجع، دفعات مرحلية) بمعلومات اتصال معتمدة من الحوكمة | Gestionnaire de communauté | 2025-06-10 | ✅ مكتمل | تمت الموافقة على الدفعة الاولى من 8 مراجعين مجتمعيين؛ معرفات الطلب `DOCS-SORA-Preview-REQ-C01...C08` مسجلة في المتتبع. |

## قائمة ادلة الاثبات

- [x] سجل موافقة الحوكمة (محاضر الاجتماع + رابط التصويت) مرفق بـ `DOCS-SORA-Preview-W2`.
- [x] قالب الطلب المحدث مثبت تحت `docs/examples/`.
- [x] descripteur لاصدار `preview-2025-06-15` et somme de contrôle et sonde وتقرير الروابط ونص وكيل Essayez-le محفوظة تحت `artifacts/docs_preview/W2/`.
- [x] لقطات Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) pour le contrôle en amont de W2.
- [x] جدول roster للدعوات مع معرفات المراجعين وتذاكر الطلب وتواريخ الموافقة معبأة قبل الارسال (راجع قسم W2 في المتتبع).

حافظ على تحديث هذه الخطة؛ Il s'agit de DOCS-SORA pour la version W2.