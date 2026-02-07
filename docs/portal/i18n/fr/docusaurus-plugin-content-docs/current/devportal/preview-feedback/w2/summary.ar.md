---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-summary
titre : ملخص ملاحظات وحالة W2
sidebar_label : par W2
description : ملخص حي لموجة المعاينة المجتمعية (W2).
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - المراجعون المجتمعيون |
| نافذة الدعوة | 2025-06-15 -> 2025-06-29 |
| وسم الاثر | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |
| المشاركون | comm-vol-01...comm-vol-08 |

## ابرز النقاط

1. **الحوكمة والادوات** - تمت الموافقة بالاجماع على سياسة admission المجتمعية في 2025-05-20 ; قالب الطلب المحدث مع حقول الدافع/المنطقة الزمنية موجود تحت `docs/examples/docs_preview_request_template.md`.
2. ** Contrôle en amont ** - تم تنفيذ تغيير وكيل Essayez-le `OPS-TRYIT-188` le 2025-06-09, تم التقاط لوحات Grafana, وارشفة Utilisez le descripteur/somme de contrôle/sonde pour `preview-2025-06-15` comme `artifacts/docs_preview/W2/`.
3. **موجة الدعوات** - تمت دعوة ثمانية مراجعين في 2025-06-15، مع تسجيل الاقرارات في جدول الدعوات بالمتتبع; اكمل الجميع تحقق checksum قبل التصفح.
4. **الاحظات** - Télécharger `docs-preview/w2 #1` (info-bulle d'aide) et `#2` (info-bulle) le 18/06/2025 Publié le 2025-06-21 (Docs-core-04/05)؛ لم تقع حوادث خلال الموجة.

## بنود العمل| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W2-A1 | Recherchez `docs-preview/w2 #1` (info-bulle ci-dessous). | Docs-core-04 | ✅مكتمل (2025-06-21). |
| W2-A2 | معالجة `docs-preview/w2 #2` (ترتيب الشريط الجانبي للترجمة). | Docs-core-05 | ✅مكتمل (2025-06-21). |
| W2-A3 | ارشفة ادلة الخروج + تحديث feuille de route/statut. | Responsable Docs/DevRel | ✅مكتمل (2025-06-29). |

## ملخص الخروج (2025-06-29)

- اكد جميع المراجعين المجتمعيين الثمانية الاكتمال وتم سحب صلاحيات المعاينة ; تم تسجيل الاقرارات في سجل الدعوات بالمتتبع.
- بقيت لقطات القياس النهائية (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) ; اللوجات ونصوص وكيل Essayez-le مرفقة بـ `DOCS-SORA-Preview-W2`.
- Fichiers (descripteur, journal de somme de contrôle, sortie de sonde, rapport de lien, fichiers Grafana, fichiers de somme) et fichiers `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Il s'agit d'une version W2 ou d'une version W3.