---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : سجل الملاحظات والقياس W1
sidebar_label : par W1
description: قائمة مجمعة، نقاط قياس، وملاحظات المراجعين لموجة معاينة الشركاء الاولى.
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة **معاينة الشركاء W1**
المرافقة لمهام القبول في [`preview-feedback/w1/plan.md`](./plan.md) et متتبع الموجة في
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). حدثه عند ارسال دعوة،
او تسجيل لقطة قياس، او تصنيف بند ملاحظات حتى يتمكن مراجعو الحوكمة من اعادة تشغيل
الادلة دون ملاحقة تذاكر خارجية.

## قائمة الدفعة| معرف الشريك | تذكرة الطلب | استلام NDA | ارسال الدعوة (UTC) | اقرار/اول دخول (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مكتمل 2025-04-26 | sorafs-op-01 ; Il s'agit d'un orchestrateur. |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مكتمل 2025-04-26 | sorafs-op-02 ; Utilisez la fonction Norito/telemetry. |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مكتمل 2025-04-26 | sorafs-op-03 ; Il s'agit d'un basculement en cas de panne. |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مكتمل 2025-04-26 | torii-int-01 ; مراجعة دليل Torii `/v2/pipeline` et livre de recettes Essayez-le. |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مكتمل 2025-04-26 | torii-int-02 ; شارك في تحديث لقطة Essayez-le (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مكتمل 2025-04-26 | sdk-partenaire-01 ; Livres de recettes pour JS/Swift + Sanity pour ISO. |
| partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مكتمل 2025-04-26 | sdk-partenaire-02 ; تم انهاء الامتثال 2025-04-11، ركز على ملاحظات Connexion/télémétrie. || partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مكتمل 2025-04-26 | passerelle-ops-01 ; دقق دليل عمليات gateway + مسار proxy Essayez-le ici. |

املأ تواريخ **ارسال الدعوة** et **الاقرار** فور اصدار البريد الصادر.
اربط الاوقات بجدول UTC المحدد في خطة W1.

## نقاط القياس

| الطابع الزمني (UTC) | لوحات / sondes | المالك | النتيجة | الاثر |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ كلها خضراء | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Voir `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | ✅ تم التجهيز | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | اللوحات اعلاه + `probe:portal` | Docs/DevRel + Ops | ✅ لقطة قبل الدعوة، بلا تراجعات | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | اللوحات اعلاه + فرق زمن Essayez-le | Responsable Docs/DevRel | ✅ اجتاز فحص منتصف الموجة (0 تنبيهات; زمن Essayez-le p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | اللوحات اعلاه + sonde خروج | Docs/DevRel + liaison gouvernance | ✅ لقطة خروج، صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

عينات ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) مجمعة كصادرات NDJSON + PNG تحت
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` pour les utilisateurs
`docs-preview-integrity-<date>.json` واللقطات المقابلة.

## سجل الملاحظات والتذاكر

استخدم هذا الجدول لتلخيص الملاحظات المقدمة من المراجعين. اربط كل بند بتذكرة GitHub/discuss
بالاضافة الى النموذج المهيكل الملتقط عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).| المرجع | الشدة | المالك | الحالة | ملاحظات |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | ✅ تم الحل 2025-04-18 | تم توضيح صياغة تنقل Essayez-le + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` byالوسم الجديد). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | ✅ تم الحل 2025-04-19 | تم تحديث لقطة Essayez-le + التسمية حسب طلب المراجع؛ Voir `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Informations | Responsable Docs/DevRel | 🟢 مغلق | كانت التعليقات المتبقية اسئلة/اجابات فقط؛ Il s'agit d'un produit que vous pouvez utiliser pour le `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## تتبع اختبار المعرفة والاستبيان

1. سجل درجات الاختبار (الهدف >=90%) لكل مراجع؛ وارفق ملف CSV المصدر بجانب اثار الدعوة.
2. اجمع اجابات الاستبيان النوعية الملتقطة عبر نموذج الملاحظات وكررها تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. جدولة مكالمات المعالجة لمن يقل عن الحد وادونها في هذا الملف.

سجل جميع المراجعين الثمانية >=94% في اختبار المعرفة (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). لم تكن هناك مكالمات
معالجة مطلوبة؛ صادرات الاستبيان لكل شريك محفوظة تحت
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## جرد الاثار

- Descripteur/somme de contrôle d'aperçu : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Sonde ملخص + link-check : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير proxy Essayez-le : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Nom du produit : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Nom du produit: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- صادرات الملاحظات والاستبيان: ضع مجلدات كل مراجع تحت
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Fichier CSV utilisé : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`حافظ على الجرد متزامنا مع تذكرة المتتبع. ارفق الهاشات عند نسخ الاثار الى تذكرة الحوكمة
حتى يتمكن المدققون من التحقق من الملفات دون وصول للصدفة.