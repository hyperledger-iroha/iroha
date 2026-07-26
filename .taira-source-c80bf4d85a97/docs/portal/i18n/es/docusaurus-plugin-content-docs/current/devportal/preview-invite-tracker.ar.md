---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# متعقب دعوات المعاينة

Para obtener información sobre el producto, consulte el documento DOCS-SORA y el producto. اي مجموعة نشطة، من وافق على الدعوات، واي اثار ما زالت بحاجة الى متابعة. حدّثه كلما تم ارسال الدعوات او سحبها او تأجيلها حتى يبقى سجل التدقيق داخل المستودع.

## حالة الموجات| الموجة | المجموعة | تذكرة المتابعة | الموافقون | الحالة | النافذة المستهدفة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores de Docs + SDK يتحققون من تدفق suma de comprobación | `DOCS-SORA-Preview-W0` (desde GitHub/ops) | Documentos principales/DevRel + Portal TL | مكتمل | Q2 2025 الاسابيع 1-2 | الدعوات ارسلت 2025-03-25, القياس عن بعد بقي اخضر، ملخص الخروج نشر 2025-04-08. |
| **W1 - Socios** | Certificado de confidencialidad SoraFS y Torii | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Q2 2025 الاسبوع 3 | الدعوات 2025-04-12 -> 2025-04-26 مع تأكيد جميع الشركاء الثمانية؛ Haga clic en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidad** | قائمة انتظار مجتمعية منتقاة ( 2025-06-29 مع قياس عن بعد اخضر طوال الفترة؛ الادلة والنتائج في [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **S3 - Cohortes beta** | Beta التمويل/الملاحظة + شريك SDK + داعم للنظام البيئي | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Primer trimestre de 2026 8 | الدعوات 2026-02-18 -> 2026-02-28؛ تم توليد الملخص وبيانات البوابة عبر موجة `preview-20260218` (انظر [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> ملاحظة: اربط كل تذكرة في المتعقب بطلبات المعاينة وارشفها تحت مشروع `docs-portal-preview` حتى تبقى الموافقات قابلة للاكتشاف.

## المهام النشطة (W0)

- تحديث اثار preflight (تشغيل GitHub Actions `docs-portal-preview` بتاريخ 2025-03-24, y تحقق descriptor عبر `scripts/preview_verify.sh` باستخدام وسم `preview-2025-03-24`).
- توثيق خطوط اساس القياس عن بعد (`docs.preview.integrity`، وحفظ لقطة `TryItProxyErrors` في تذكرة W0).
- تثبيت نص التواصل باستخدام [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) مع وسم vista previa `preview-2025-03-24`.
- تسجيل طلبات الدخول لاوائل خمسة mantenedores (تذاكر `DOCS-SORA-Preview-REQ-01` ... `-05`).
- ارسال اول خمس دعوات 2025-03-25 10:00-10:20 UTC بعد سبعة ايام متتالية من القياس الاخضر؛ Haga clic en `DOCS-SORA-Preview-W0`.
- متابعة القياس عن بعد + horario de oficina للمضيف (فحوصات يومية حتى 2025-03-31؛ سجل puestos de control بالاسفل).
- جمع ملاحظات منتصف الموجة / القضايا ووضع الوسم `docs-preview/w0` (انظر [W0 digest](./preview-feedback/w0/summary.md)).
- نشر ملخص الموجة + تأكيدات الخروج (حزمة خروج بتاريخ 2025-04-08؛ انظر [resumen W0] (./preview-feedback/w0/summary.md)).
- تتبع موجة W3 beta؛ جدولة موجات لاحقة حسب مراجعة الحوكمة.

## ملخص موجة الشركاء W1- الموافقات القانونية والحوكمة. تم توقيع ملحق الشركاء 2025-04-05؛ تمت رفع الموافقات الى `DOCS-SORA-Preview-W1`.
- القياس عن بعد + Pruébalo en escena. تم تنفيذ تذكرة التغيير `OPS-TRYIT-147` في 2025-04-06 مع ارشفة لقطات Grafana لـ `docs.preview.integrity` و `TryItProxyErrors` y `DocsPortal/GatewayRefusals`.
- تجهيز artefacto + suma de comprobación. تم التحقق من حزمة `preview-2025-04-12`؛ Utilice el descriptor/suma de comprobación/sonda `artifacts/docs_preview/W1/preview-2025-04-12/`.
- قائمة الدعوات + الارسال. تمت الموافقة على ثماني طلبات شركاء (`DOCS-SORA-Preview-REQ-P01...P08`)؛ ارسلت الدعوات 2025-04-12 15:00-15:21 UTC مع تسجيل الايصالات لكل مراجع.
- ادوات جمع الملاحظات. تم تسجيل horario de oficina اليومية + puntos de control القياس؛ راجع [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) للملخص.
- القائمة النهائية / سجل الخروج. يسجل [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) الان توقيتات الدعوات/الايصالات، ادلة القياس، exports الاختبارات، ومؤشرات artefactos حتى 2025-04-26 ليتمكن فريق الحوكمة من اعادة بناء الموجة.

## سجل الدعوات - Mantenedores del núcleo W0| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | نشط | اكد تحقق suma de comprobación؛ يركز على مراجعة nav/sidebar. |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | نشط | يختبر وصفات SDK + inicios rápidos Norito. |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | نشط | يراجع وحدة Pruébelo + تدفقات ISO. |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | نشط | Hay runbooks SoraFS + y orquestación. |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | نشط | يراجع ملاحق القياس/الحوادث؛ Utilice Alertmanager. |

كل الدعوات تشير الى نفس artefacto `docs-portal-preview` (تشغيل 2025-03-24, وسم `preview-2025-03-24`) y سجل التحقق المحفوظ في `DOCS-SORA-Preview-W0`. يجب تسجيل اي اضافة/توقف في الجدول اعلاه وفي تذكرة المتعقب قبل الانتقال للموجة التالية.

## سجل puntos de control - W0| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-03-26 | مراجعة baseline القياس + horario de oficina | Baterías `docs.preview.integrity` y `TryItProxyErrors` اكدت horas de oficina اكتمال تحقق suma de comprobación. |
| 2025-03-27 | نشر ملخص comentarios الوسطي | تم حفظ الملخص في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md)؛ Para configurar el navegador, seleccione `docs-preview/w0`. |
| 2025-03-31 | فحص قياس الاسبوع الاخير | اخر horario de oficina قبل الخروج؛ اكد المراجعون تقدم المهام، دون تنبيهات. |
| 2025-04-08 | ملخص الخروج + اغلاق الدعوات | تاكيد اكتمال المراجعات، سحب الوصول المؤقت، ارشفة النتائج في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08)؛ تحديث المتعقب قبل تجهيز W1. |

## سجل الدعوات - Socios de W1| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | مكتمل | تم تسليم ملاحظات operaciones للاوركستريتور 2025-04-20؛ ack Fecha de las 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | مكتمل | سجل تعليقات implementación de `docs-preview/w1`; respuesta a las 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | مكتمل | تم تسجيل تعديلات disputa/lista negra؛ respuesta a las 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | مكتمل | Tutorial completo Pruébelo auth؛ respuesta a las 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | مكتمل | Aplicación RPC/OAuth respuesta a las 15:16 UTC. |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | مكتمل | دمج ملاحظات سلامة المعاينة؛ respuesta a las 15:18 UTC. |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | مكتمل | مراجعة telemetría/redacción اكتملت؛ respuesta a las 15:22 UTC. || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | مكتمل | Aplicación de la puerta de enlace DNS de runbook respuesta a las 15:24 UTC. |

## Puntos de control - W1

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-04-12 | Artículos y artefactos | تم ارسال البريد لكل الشركاء مع descriptor/archivo `preview-2025-04-12`؛ الايصالات محفوظة في المتعقب. |
| 2025-04-13 | مراجعة baseline القياس | `docs.preview.integrity` y `TryItProxyErrors` y `DocsPortal/GatewayRefusals` para el hogar horario de oficina اكدت اكتمال تحقق suma de comprobación. |
| 2025-04-18 | horario de oficina منتصف الموجة | Más información `docs.preview.integrity` Utilice la documentación de `docs-preview/w1` (nav + botón Pruébelo). |
| 2025-04-22 | فحص قياس نهائي | الوكيل ولوحات القياس سليمة؛ لا قضايا جديدة, تم تدوينها في المتعقب قبل الخروج. |
| 2025-04-26 | ملخص الخروج + اغلاق الدعوات | اكد جميع الشركاء اكتمال المراجعات، تم سحب الدعوات، الادلة ارشفت في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## ملخص cohorte beta W3

- ارسلت الدعوات 2026-02-18 مع تحقق checksum وتسجيل الايصالات في نفس اليوم.
- تم جمع retroalimentación تحت `docs-preview/20260218` مع تذكرة الحوكمة `DOCS-SORA-Preview-20260218`; تم توليد digest + ملخص عبر `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- تم سحب الوصول 2026-02-28 بعد فحص القياس النهائي؛ تحديث المتعقب وجداول البوابة لاظهار اكتمال W3.

## سجل الدعوات - Comunidad W2| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | مكتمل | respuesta 16:06 UTC؛ يركز على SDK de inicio rápido؛ تم تأكيد الخروج 2025-06-29. |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | مكتمل | مراجعة الحوكمة/SNS اكتملت؛ تم تأكيد الخروج 2025-06-29. |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | مكتمل | Tutorial del tutorial Norito؛ confirmar 2025-06-29. |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | مكتمل | Más runbooks SoraFS اكتملت؛ confirmar 2025-06-29. |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | مكتمل | مشاركة ملاحظات accesibilidad/UX؛ confirmar 2025-06-29. |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | مكتمل | تسجيل ملاحظات localización؛ confirmar 2025-06-29. |
| comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | مكتمل | تم تسليم مراجعات docs SDK móvil؛ confirmar 2025-06-29. || comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | مكتمل | مراجعة ملحق observabilidad اكتملت؛ confirmar 2025-06-29. |

## Puntos de control - W2

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-06-15 | Artículos y artefactos | تم مشاركة descriptor/archivo `preview-2025-06-15` مع 8 مراجعين؛ الايصالات محفوظة في المتعقب. |
| 2025-06-16 | مراجعة baseline القياس | `docs.preview.integrity` y `TryItProxyErrors` y `DocsPortal/GatewayRefusals` سجلات الوكيل Pruébelo تظهر رموز المجتمع نشطة. |
| 2025-06-18 | horarios de oficina وفرز القضايا | اقتراحان (`docs-preview/w2 #1` información sobre herramientas, `#2` localización de شريط) - كلاهما اسند الى Docs. |
| 2025-06-21 | فحص قياس + اصلاحات documentos | Aquí está `docs-preview/w2 #1/#2`; لوحات القياس خضراء، دون حوادث. |
| 2025-06-24 | horarios de oficina للاسبوع الاخير | اكد المراجعون ما تبقى من الملاحظات؛ لا تنبيهات. |
| 2025-06-29 | ملخص الخروج + اغلاق الدعوات | تم تسجيل الايصالات، سحب وصول المعاينة، ارشفة instantáneas + artefactos (انظر [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | horarios de oficina وفرز القضايا | تم تسجيل اقتراحين توثيقيين تحت `docs-preview/w1`; لا حوادث ولا تنبيهات. |

## روابط التقارير- كل يوم اربعاء، حدّث الجدول اعلاه وتذكرة الدعوات النشطة بملاحظة قصيرة (الدعوات المرسلة، المراجعين النشطين، الحوادث).
- Haga clic en el botón de encendido (`docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y `status.md`.
- اذا تم تفعيل اي معيار توقف في [vista previa del flujo de invitación](./preview-invite-flow.md), اضف خطوات المعالجة هنا قبل استئناف الدعوات.