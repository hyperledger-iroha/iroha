---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# متعقب دعوات المعاينة

Vous avez besoin de plus d'informations sur le document DOCS-SORA et le lien vers la page d'accueil رؤية اي مجموعة نشطة، من وافق على الدعوات، واي اثار ما زالت بحاجة الى متابعة. حدّثه كلما تم ارسال الدعوات او سحبها او تأجيلها حتى يبقى سجل التدقيق داخل المستودع.

## حالة الموجات| الموجة | المجموعة | تذكرة المتابعة | الموافقون | الحالة | النافذة المستهدفة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Les responsables de Docs + SDK complètent la somme de contrôle | `DOCS-SORA-Preview-W0` (avec GitHub/ops) | Lead Docs/DevRel + Portail TL | مكتمل | T2 2025 الاسابيع 1-2 | La date du 25/03/2025 est la suivante: le 08/04/2025. |
| **W1 - Partenaires** | مشغلو SoraFS et Torii par NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | T2 2025 الاسبوع 3 | الدعوات 2025-04-12 -> 2025-04-26 مع تأكيد جميع الشركاء الثمانية؛ Vous devez utiliser [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) et [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communauté** | قائمة انتظار مجتمعية منتقاة ( 2025-06-29 مع قياس عن بعد اخضر طوال الفترة؛ Il s'agit de [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes bêta** | Beta التمويل/الملاحظة + شريك SDK + داعم للنظام البيئي | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | T1 2026 الاسبوع 8 | الدعوات 2026-02-18 -> 2026-02-28؛ Vous devez également utiliser `preview-20260218` (`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> ملاحظة: اربط كل تذكرة في المتعقب بطلبات المعاينة وارشفها تحت مشروع `docs-portal-preview` حتى تبقى الموافقات قابلة للاكتشاف.

## المهام النشطة (W0)

- Contrôle en amont (gestion des actions GitHub `docs-portal-preview` du 24/03/2025 et descripteur `scripts/preview_verify.sh` pour le contrôle en amont `preview-2025-03-24`).
- توثيق خطوط اساس القياس عن بعد (`docs.preview.integrity`, وحفظ لقطة `TryItProxyErrors` في تذكرة W0).
- تثبيت نص التواصل باستخدام [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) avec l'aperçu `preview-2025-03-24`.
- تسجيل طلبات الدخول لاوائل خمسة responsables (تذاكر `DOCS-SORA-Preview-REQ-01` ... `-05`).
- ارسال اول خمس دعوات 2025-03-25 10:00-10:20 UTC بعد سبعة ايام متتالية من القياس الاخضر؛ Il s'agit d'un document `DOCS-SORA-Preview-W0`.
- متابعة القياس عن بعد + heures de bureau للمضيف (فحوصات يومية حتى 2025-03-31؛ سجل points de contrôle بالاسفل).
- جمع ملاحظات منتصف الموجة / القضايا وووضع الوسم `docs-preview/w0` (انظر [W0 digest](./preview-feedback/w0/summary.md)).
- نشر ملخص الموجة + تأكيدات الخروج (حزمة خروج بتاريخ 2025-04-08؛ انظر [W0 digest](./preview-feedback/w0/summary.md)).
- تتبع موجة W3 beta؛ جدولة موجات لاحقة حسب مراجعة الحوكمة.

## ملخص موجة الشركاء W1- الموافقات القانونية والحوكمة. تم توقيع ملحق الشركاء 2025-04-05؛ Il s'agit de la référence `DOCS-SORA-Preview-W1`.
- القياس عن بعد + Essayez la mise en scène. تم تنفيذ تذكرة التغيير `OPS-TRYIT-147` le 2025-04-06 مع ارشفة لقطات Grafana pour `docs.preview.integrity` et `TryItProxyErrors` et `DocsPortal/GatewayRefusals`.
- تجهيز artefact + somme de contrôle. تم التحقق من حزمة `preview-2025-04-12`؛ Vous utilisez le descripteur/somme de contrôle/sonde comme `artifacts/docs_preview/W1/preview-2025-04-12/`.
- قائمة الدعوات + الارسال. تمت الموافقة على ثماني طلبات شركاء (`DOCS-SORA-Preview-REQ-P01...P08`)؛ ارسلت الدعوات 2025-04-12 15:00-15:21 UTC مع تسجيل الايصالات لكل مراجع.
- ادوات جمع الملاحظات. تم تسجيل heures de bureau اليومية + points de contrôle القياس؛ راجع [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) للملخص.
- القائمة النهائية / سجل الخروج. يسجل [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) Les exportations d'artefacts et d'artefacts 2025-04-26 ليتمكن فريق الحوكمة من اعادة بناء الموجة.

## سجل الدعوات - Mainteneurs principaux de W0| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | نشط | اكد تحقق checksum؛ يركز على مراجعة nav/sidebar. |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | نشط | J'utilise le SDK + quickstarts Norito. |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | نشط | يراجع وحدة Essayez-le + تدفقات ISO. |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | نشط | Il s'agit des runbooks SoraFS + et de l'orchestration. |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | نشط | يراجع ملاحق القياس/الحوادث؛ Utilisez Alertmanager. |

كل الدعوات تشير الى نفس artefact `docs-portal-preview` (تشغيل 2025-03-24, وسم `preview-2025-03-24`) et سجل التحقق المحفوظ في `DOCS-SORA-Preview-W0`. يجب تسجيل اي اضافة/توقف في الجدول اعلاه وفي تذكرة المتعقب قبل الانتقال للموجة التالية.

## سجل points de contrôle - W0| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-03-26 | مراجعة baseline القياس + heures de bureau | Pour `docs.preview.integrity` et `TryItProxyErrors` pour la lecture اكدت heures de bureau اكتمال تحقق somme de contrôle. |
| 2025-03-27 | Commentaires sur les commentaires | تم حفظ الملخص في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md)؛ Vous devez utiliser le système de navigation `docs-preview/w0` pour le système de navigation. |
| 2025-03-31 | فحص قياس الاسبوع الاخير | اخر heures de bureau قبل الخروج؛ اكد المراجعون تقدم المهام، دون تنبيهات. |
| 2025-04-08 | ملخص الخروج + اغلاق الدعوات | تاكيد اكتمال المراجعات، سحب الوصول المؤقت، ارشفة النتائج في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08)؛ تحديث المتعقب قبل تجهيز W1. |

## سجل الدعوات - Partenaires W1| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | مكتمل | تم تسليم ملاحظات ops للاوركستريتور 2025-04-20؛ accusé de réception à 15h05 UTC. |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | مكتمل | Déploiement des fonctionnalités pour `docs-preview/w1` ; accusé de réception à 15h10 UTC. |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | مكتمل | تم تسجيل تعديلات litige/liste noire؛ accusé de réception à 15h12 UTC. |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | مكتمل | قبول procédure pas à pas Essayez-le auth؛ accusé de réception à 15h14 UTC. |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | مكتمل | Utiliser RPC/OAuth accusé de réception à 15h16 UTC. |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | مكتمل | دمج ملاحظات سلامة المعاينة؛ accusé de réception à 15h18 UTC. |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | مكتمل | مراجعة télémétrie/rédaction اكتملت؛ accusé de réception à 15h22 UTC. || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | مكتمل | Voir la passerelle DNS du runbook accusé de réception à 15h24 UTC. |

## سجل points de contrôle - W1

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-04-12 | ارسال الدعوات + تحقق artefacts | تم ارسال البريد لكل الشركاء مع descripteur/archive `preview-2025-04-12`؛ الايصالات محفوظة في المتعقب. |
| 2025-04-13 | مراجعة baseline القياس | `docs.preview.integrity` et `TryItProxyErrors` et `DocsPortal/GatewayRefusals` pour la lecture heures de bureau اكدت اكتمال تحقق somme de contrôle. |
| 2025-04-18 | heures de bureau منتصف الموجة | Pour `docs.preview.integrity` تم تسجيل ملاحظتين docs تحت `docs-preview/w1` (صياغة nav + لقطة Essayez-le). |
| 2025-04-22 | فحص قياس نهائي | الوكيل ولوحات القياس سليمة؛ لا قضايا جديدة، تم تدوينها في المتعقب قبل الخروج. |
| 2025-04-26 | ملخص الخروج + اغلاق الدعوات | اكد جميع الشركاء اكتمال المراجعات، تم سحب الدعوات، الادلة ارشفت في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## ملخص cohorte bêta W3

- ارسلت الدعوات 2026-02-18 مع تحقق checksum وتسجيل الايصالات في نفس اليوم.
- تم جمع feedback تحت `docs-preview/20260218` مع تذكرة الحوكمة `DOCS-SORA-Preview-20260218`; تم توليد digest + ملخص عبر `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- تم سحب الوصول 2026-02-28 byعد فحص القياس النهائي؛ Il s'agit d'une application pour W3.

## سجل الدعوات - Communauté W2| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | مكتمل | accusé de réception à 16:06 UTC؛ Voir le SDK de démarrage rapide Publié le 2025-06-29. |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | مكتمل | مراجعة الحوكمة/SNS اكتملت؛ Publié le 2025-06-29. |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | مكتمل | Procédure pas à pas de ملاحظات Norito؛ accusé de réception le 29/06/2025. |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | مكتمل | Télécharger les runbooks SoraFS accusé de réception le 29/06/2025. |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | مكتمل | مشاركة ملاحظات accessibilité/UX؛ accusé de réception le 29/06/2025. |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | مكتمل | تسجيل ملاحظات localisation؛ accusé de réception le 29/06/2025. |
| comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | مكتمل | تم تسليم مراجعات docs SDK mobile؛ accusé de réception le 29/06/2025. || comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | مكتمل | مراجعة ملحق observabilité اكتملت؛ accusé de réception le 29/06/2025. |

## سجل points de contrôle - W2

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-06-15 | ارسال الدعوات + تحقق artefacts | تم مشاركة descriptor/archive `preview-2025-06-15` مع 8 مراجعين؛ الايصالات محفوظة في المتعقب. |
| 2025-06-16 | مراجعة baseline القياس | `docs.preview.integrity` et `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont disponibles سجلات الوكيل Essayez-le تظهر رموز المجتمع نشطة. |
| 2025-06-18 | heures de bureau وفرز القضايا | اقتراحان (`docs-preview/w2 #1` dans l'info-bulle, `#2` dans la localisation) - كلاهما اسند الى Docs. |
| 2025-06-21 | فحص قياس + اصلاحات docs | Voir `docs-preview/w2 #1/#2` ; لوحات القياس خضراء، دون حوادث. |
| 2025-06-24 | heures de bureau للاسبوع الاخير | اكد المراجعون ما تبقى من الملاحظات؛ لا تنبيهات. |
| 2025-06-29 | ملخص الخروج + اغلاق الدعوات | Vous pouvez utiliser des instantanés + des artefacts (انظر [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | heures de bureau وفرز القضايا | تم تسجيل اقتراحين توثيقيين تحت `docs-preview/w1`; لا حوادث ولا تنبيهات. |

## روابط التقارير- كل يوم اربعاء، حدّث الجدول اعلاه وتذكرة الدعوات النشطة بملاحظة قصيرة (الدعوات المرسلة، المراجعين النشطين، الحوادث).
- عند اغلاق موجة، اضف ​​مسار ملخص الملاحظات (مثلا `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et `status.md`.
- اذا تم تفعيل اي معيار توقف في [prévisualiser le flux d'invitation] (./preview-invite-flow.md) et اضف خطوات المعالجة هنا قبل استئناف الدعوات.