---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: سجل الملاحظات والقياس W1
sidebar_label: Mostrar W1
descripción: قائمة مجمعة، نقاط قياس، وملاحظات المراجعين لموجة معاينة الشركاء الاولى.
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة **معاينة الشركاء W1**
المرافقة لمهام القبول في [`preview-feedback/w1/plan.md`](./plan.md) y مدخل متتبع الموجة في
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). حدثه عند ارسال دعوة،
او تسجيل لقطة قياس، او تصنيف بند ملاحظات حتى يتمكن مراجعو الحوكمة من اعادة تشغيل
الادلة دون ملاحقة تذاكر خارجية.

## قائمة الدفعة| معرف الشريك | تذكرة الطلب | NDA | ارسال الدعوة (UTC) | اقرار/اول دخول (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Actualización 2025-04-26 | sorafs-op-01; ركز على ادلة تكافؤ وثائق orquestador. |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Actualización 2025-04-26 | sorafs-op-02; تحقق من الروابط المتقاطعة بين Norito/telemetry. |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Actualización 2025-04-26 | sorafs-op-03; نفذ تمارين conmutación por error متعددة المصادر. |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Actualización 2025-04-26 | torii-int-01; مراجعة دليل Torii `/v1/pipeline` y libro de cocina Pruébelo. |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Actualización 2025-04-26 | torii-int-02; شارك في تحديث لقطة Pruébelo (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Actualización 2025-04-26 | socio-sdk-01; Incluye libros de cocina con JS/Swift + Cordura con ISO. |
| socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Actualización 2025-04-26 | socio-sdk-02; تم انهاء الامتثال 2025-04-11, ركز على ملاحظات Conexión/telemetría. || socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Actualización 2025-04-26 | puerta de enlace-ops-01; دقق دليل عمليات gateway + مسار proxy Pruébelo المجهول. |

املأ تواريخ **ارسال الدعوة** و **الاقرار** فور اصدار البريد الصادر.
اربط الاوقات بجدول UTC المحدد في خطة W1.

## نقاط القياس

| الطابع الزمني (UTC) | Productos químicos / sondas | المالك | النتيجة | الاثر |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | ✅ كلها خضراء | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Nombre `npm run manage:tryit-proxy -- --stage preview-w1` | Operaciones | ✅ تم التجهيز | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | اللوحات اعلاه + `probe:portal` | Documentos/DevRel + Operaciones | ✅ لقطة قبل الدعوة، بلا تراجعات | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | اللوحات اعلاه + فرق زمن Pruébalo | Líder de Docs/DevRel | ✅ اجتاز فحص منتصف الموجة (0 تنبيهات; زمن Pruébelo p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | اللوحات اعلاه + sonda خروج | Docs/DevRel + enlace de gobernanza | ✅ لقطة خروج، صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

عينات ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) Descargas de archivos NDJSON + PNG
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` مع اسماء ملفات
`docs-preview-integrity-<date>.json` واللقطات المقابلة.

## سجل الملاحظات والتذاكر

استخدم هذا الجدول لتلخيص الملاحظات المقدمة من المراجعين. اربط كل بند بتذكرة GitHub/discutir
بالاضافة الى النموذج المهيكل الملتقط عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).| المرجع | الشدة | المالك | الحالة | ملاحظات |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | ✅ تم الحل 2025-04-18 | تم توضيح صياغة تنقل Pruébelo + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` بالوسم الجديد). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | ✅ تم الحل 2025-04-19 | تم تحديث لقطة Pruébalo + التسمية حسب طلب المراجع؛ Nombre `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Información | Líder de Docs/DevRel | 🟢 مغلق | كانت التعليقات المتبقية اسئلة/اجابات فقط؛ تم التقاطها في نموذج ملاحظات كل شريك تحت `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## تتبع اختبار المعرفة والاستبيان

1. سجل درجات الاختبار (الهدف >=90%) لكل مراجع؛ Utilice el archivo CSV para guardar el archivo.
2. اجمع اجابات الاستبيان النوعية الملتقطة عبر نموذج الملاحظات وكررها تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. جدولة مكالمات المعالجة لمن يقل عن الحد وادونها في هذا الملف.

سجل جميع المراجعين الثمانية >=94% في اختبار المعرفة (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). لم تكن هناك مكالمات
معالجة مطلوبة؛ صادرات الاستبيان لكل شريك محفوظة تحت
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## جرد الاثار

- Descriptor de vista previa/suma de verificación: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Sonda de control + verificación de enlace: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير proxy Pruébelo: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Números de referencia: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- حزمة قياس يومية لساعات المكتب: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- صادرات الملاحظات Yالاستبيان: ضع مجلدات كل مراجع تحت
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Archivo CSV: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`حافظ على الجرد متزامنا مع تذكرة المتتبع. ارفق الهاشات عند نسخ الاثار الى تذكرة الحوكمة
حتى يتمكن المدققون من التحقق من الملفات دون وصول للصدفة.