---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/governance_playbook.md` وتعمل الان كمرجع بوابة
موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

# دليل حوكمة خدمة أسماء سورا (SN-6)

**الحالة:** صيغ 2026-03-24 - مرجع حي لاستعداد SN-1/SN-6  
**روابط خارطة الطريق:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver y sincronización de puerta de enlace", سياسة العناوين ADDR-1/ADDR-5  
**المتطلبات المسبقة:** مخطط السجل في [`registry-schema.md`](./registry-schema.md), عقد API للمسجل في [`registrar-api.md`](./registrar-api.md), ارشادات تجربة العناوين في [`address-display-guidelines.md`](./address-display-guidelines.md), y قواعد بنية الحساب في [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

يصف هذا الدليل كيف تعتمد هيئات حوكمة خدمة أسماء سورا (SNS) المواثيق، وتوافق على
Las aplicaciones de resolución y de puerta de enlace están conectadas a la aplicación. يفي
Adaptador de corriente CLI `sns governance ...` y Norito
والاثار التدقيقية مرجعا تشغيليا واحدا قبل N1 (الاطلاق العام).

## 1. النطاق والجمهور

يستهدف المستند:

- اعضاء مجلس الحوكمة الذين يصوتون على المواثيق، وسياسات اللاحقات، ونتائج النزاعات.
- اعضاء مجلس guardian الذين يصدرون تجميدات طارئة ويراجعون التراجعات.
- azafatas اللاحقات الذين يديرون طوابير المسجل، ويوافقون على المزادات، ويديرون
  تقسيمات الايرادات.
- مشغلو resolver/gateway المسؤولون عن انتشار SoraDNS, وتحديثات GAR, وحواجز
  التليمترية.
- El producto y el producto se encuentran en el lugar donde se encuentra el producto Norito.
  قابلة للتدقيق.يغطي مراحل البيتا المغلقة (N0) والاطلاق العام (N1) والتوسع (N2) المدرجة في
`roadmap.md` Para que el producto funcione correctamente, puede utilizar dispositivos electrónicos, dispositivos electrónicos y dispositivos electrónicos.
التصعيد.

## 2. الادوار وخريطة الاتصال| الدور | المسؤوليات الاساسية | ابرز الاثار y التليمترية | التصعيد |
|------|----------------------|-------------------------|---------|
| مجلس الحوكمة | يصيغ ويصادق على المواثيق، وسياسات اللاحقات، واحكام النزاعات، وتناوب mayordomos. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, بطاقات تصويت المجلس المخزنة عبر `sns governance charter submit`. | رئيس المجلس + متعقب جدول اعمال الحوكمة. |
| مجلس guardián | يصدر تجميدات blando/duro, y قوانين طارئة, ومراجعات 72 h. | تذاكر guardian الصادرة عبر `sns governance freeze`, y مانيفستات التجاوز المسجلة تحت `artifacts/sns/guardian/*`. | دورية tutor de guardia (<=15 min ACK). |
| azafatas اللاحقة | يديرون طوابير المسجل، والمزادات، وشرائح التسعير، واتصالات العملاء؛ ويقرون بالامتثال. | سياسات Steward في `SuffixPolicyV1`, جداول مرجعية للتسعير, اقرارات Steward المخزنة بجانب المذكرات التنظيمية. | قائد برنامج administrador + PagerDuty خاص بكل لاحقة. |
| عمليات المسجل Y الفوترة | Utilice el `/v1/sns/*`, los parámetros, las funciones y la CLI. | API المسجل ([`registrar-api.md`](./registrar-api.md)), مقاييس `sns_registrar_status_total`, اثباتات الدفع المؤرشفة تحت `artifacts/sns/payments/*`. | مدير مناوبة المسجل ورابط الخزينة. |
| Resolver y resolver | Utilice SoraDNS y utilice el sistema de configuración de servidor de SoraDNS. ويبثون مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolutor SRE de guardia + جسر عمليات البوابة. || الخزينة y مالية | تطبق تقسيم ايرادات 70/30, واقتطاعات referencias, وملفات الضرائب/الخزينة, وشهادات SLA. | Para obtener más información, seleccione Stripe/الخزينة, KPI ربع سنوية تحت `docs/source/sns/regulatory/`. | مراقب المالية + مسؤول الامتثال. |
| جهة اتصال الامتثال والتنظيم | تتبع الالتزامات العالمية (EU DSA, الخ), وتحدث مواثيق KPI, وتقدم الافصاحات. | Las aplicaciones `docs/source/sns/regulatory/`, las aplicaciones `ops/drill-log.md` y las aplicaciones `ops/drill-log.md` están disponibles. | قائد برنامج الامتثال. |
| الدعم / SRE عند الطلب | يعالج الحوادث (تصادمات، انحرافات فوترة، اعطال resolutor), y وينسق رسائل العملاء، ويمتلك الادلة التشغيلية. | قوالب الحوادث، `ops/drill-log.md`, ادلة مختبر مرحلية, y نصوص Slack/war-room المؤرشفة تحت `incident/`. | دورية de guardia لـSNS + ادارة SRE. |

## 3. الاثار المرجعية ومصادر البيانات| الاثر | الموقع | الغرض |
|-------|--------|-------|
| الميثاق + ملاحق KPI | `docs/source/sns/governance_addenda/` | Utilice los KPI y los KPI y acceda a la CLI. |
| مخطط السجل | [`registry-schema.md`](./registry-schema.md) | Ajuste Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| عقد المسجل | [`registrar-api.md`](./registrar-api.md) | حمولات REST/gRPC, مقاييس `sns_registrar_status_total`, وتوقعات حوكمة ganchos. |
| دليل UX للعناوين | [`address-display-guidelines.md`](./address-display-guidelines.md) | Utilice I105 (المفضلة) y (الخيار الثاني) المرجعية التي تعكسها المحافظ/المستكشفات. |
| Descripción SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | اشتقاق المضيفات الحتمي، سير عمل tailer للشفافية، وقواعد التنبيه. |
| مذكرات تنظيمية | `docs/source/sns/regulatory/` | ملاحظات استقبال حسب الولاية (مثل EU DSA), اقرارات Steward, ملاحق قوالب. |
| Taladros perforados | `ops/drill-log.md` | سجل لتجارب الفوضى وIR المطلوبة قبل الخروج من المراحل. |
| تخزين الاثار | `artifacts/sns/` | Los dispositivos Guardian, los solucionadores, los KPI y los controladores de CLI se encuentran en `sns governance ...`. |

يجب ان تشير كل اجراءات الحوكمة الى اثر واحد على الاقل من الجدول اعلاه حتى يتمكن
المدققون من اعادة بناء مسار القرار خلال 24 ساعة.

## 4. ادلة دورة الحياة

### 4.1 حركات الميثاق وmayordomo| الخطوة | المالك | CLI / الدليل | ملاحظات |
|--------|--------|---------------|---------|
| Indicadores clave de rendimiento y KPI | مقرر المجلس + قائد mayordomo | Markdown es el programa `docs/source/sns/governance_addenda/YY/` | تضمين معرفات مواثيق KPI, ganchos تليمترية, وشروط التفعيل. |
| تقديم الاقتراح | رئيس المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (ينتج `CharterMotionV1`) | Utilice la CLI para Norito para conectar `artifacts/sns/governance/<id>/charter_motion.json`. |
| تصويت واعتراف guardián | المجلس + guardianes | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | ارفاق محاضر مجزأة واثباتات النصاب. |
| قبول mayordomo | azafatas de برنامج | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل تغيير سياسات اللاحقات; Aquí está el mensaje `artifacts/sns/governance/<id>/steward_ack.json`. |
| التفعيل | عمليات المسجل | تحديث `SuffixPolicyV1`, تحديث ذاكرة المسجل المؤقتة, نشر ملاحظة في `status.md`. | Esta es la configuración de `sns_governance_activation_total`. |
| سجل التدقيق | الامتثال | اضافة مدخل الى `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` وسجل taladros اذا تم تنفيذ tabletop. | تضمين مراجع لوحات التليمترية وفروقات السياسة. |

### 4.2 الموافقات على التسجيل والمزاد والتسعير1. **الفحص المسبق:** يستعلم المسجل `SuffixPolicyV1` لتاكيد شريحة التسعير، الشروط
   المتاحة، ونوافذ السماح/الاسترداد. ابق جداول التسعير متزامنة مع جدول الشرائح
   3/4/5/6-9/10+ (الشريحة الاساسية + معاملات اللاحقة) الموثقة في hoja de ruta.
2. **Oferta sellada:** Premium, compromiso de 72 h / revelación de 24 h
   Consulte `sns governance auction commit` / `... reveal`. انشر قائمة confirma (hashes)
   فقط) تحت `artifacts/sns/auctions/<name>/commit.json` حتى يتمكن المدققون من
   التحقق من العشوائية.
3. **التحقق من الدفع:** يتحقق المسجلون من `PaymentProofV1` مقابل تقسيمات الخزينة
   (70% de descuento / 30% delegado con referencia de exclusión <=10%). Descarga JSON Norito
   `artifacts/sns/payments/<tx>.json` واربطه في استجابة المسجل (`RevenueAccrualEventV1`).
4. **Hook الحوكمة:** ارفق `GovernanceHookV1` للاسماء premium/guarded مع مراجع
   لمعرفات مقترح المجلس وتواقيع mayordomo. غياب ganchos يؤدي الى
   `sns_err_governance_missing`.
5. **التفعيل + مزامنة resolver:** بمجرد ان يرسل Torii حدث التسجيل، شغل tailer
   الشفافية الخاص بالresolver لتاكيد انتشار الحالة الجديدة GAR/zone (انظر 4.5).
6. **افصاح العميل:** حدث دفتر المستخدِم (billetera/explorador) عبر accesorios المشتركة في
   [`address-display-guidelines.md`](./address-display-guidelines.md) ، مع ضمان ان
   Utilice el I105 y el código QR.

### 4.3 التجديد والفوترة وتسوية الخزينة- **سير عمل التجديد:** يفرض المسجلون نافذة سماح 30 يوم + نافذة استرداد 60 يوم
  Haga clic en `SuffixPolicyV1`. بعد 60 يوما، تتفعل تلقائيا سلسلة اعادة الفتح
  الهولندية (7 veces 10x 15%/يوم) a `sns governance reopen`.
- **تقسيم الايرادات:** كل تجديد او تحويل ينشئ `RevenueAccrualEventV1`. يجب على
  صادرات الخزينة (CSV/Parquet) التسوية مع هذه الاحداث يوميا; ارفق الاثباتات في
  `artifacts/sns/treasury/<date>.json`.
- **Referencias de exclusión:** يتم تتبع نسب referencia الاختيارية لكل لاحقة عبر اضافة
  `referral_share` الى سياسة mayordomo. يصدر المسجلون التقسيم النهائي ويخزنون
  Referencia de مانيـفستات بجانب اثبات الدفع.
- **وتيرة التقارير:** تنشر المالية ملاحق KPI شهرية (التسجيلات، التجديدات، ARPU،
  استخدام النزاعات/bond) تحت `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  Grafana
  ادلة الدفتر.
- **مراجعة KPI شهرية:** يجمع فحص اول ثلاثاء قائد المالية وsteward المناوب وPM
  البرنامج. افتح [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) (تضمين البوابة
  `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`) ، صدّر جداول انتاجية
  المسجل والايرادات، سجل الفروقات في الملحق، وارفق الاثار بالمذكرة. فعّل حادثا
  اذا وجدت المراجعة خروقات SLA (نوافذ تجميد >72 h, ارتفاع اخطاء المسجل، انحراف ARPU).

### 4.4 Herramientas y servicios| المرحلة | المالك | الاجراء والدليل | Acuerdo de Nivel de Servicio |
|---------|--------|------------------|-----|
| طلب تجميد suave | mayordomo / الدعم | Asegúrese de que `SNS-DF-<id>` esté conectado a un enlace de conexión y de enlace. | <=4 h من الاستلام. |
| تذكرة guardián | مجلس guardián | `sns governance freeze --selector <I105> --reason <text> --until <ts>` y `GuardianFreezeTicketV1`. El archivo JSON es `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h تنفيذ. |
| تصديق المجلس | مجلس الحوكمة | يوافق او يرفض التجميدات، ويوثق القرار مع رابط لتذكرة guardian وبصمة bond النزاع. | جلسة المجلس التالية او تصويت غير متزامن. |
| لجنة التحكيم | الامتثال + mayordomo | Verá 7 meses (hoja de ruta) para obtener información sobre `sns governance dispute ballot`. ارفق ايصالات التصويت المجهولة بحزمة الحادث. | الحكم <=7 ايام بعد ايداع bono. |
| استئناف | guardián + المجلس | يضاعف الاستئناف bono ويعيد عملية المحلفين; Esta es la configuración Norito `DisputeAppealV1` y la configuración del sistema. | <=10 días. |
| فك التجميد Yالمعالجة | المسجل + عمليات resolver | Aquí `sns governance unfreeze --selector <I105> --ticket <id>`, está conectado a GAR/resolver. | مباشرة بعد الحكم. |

القوانين الطارئة (تجميدات يطلقها tutor <=72 h) تتبع نفس التدفق لكنها تتطلب
Para obtener más información, consulte el documento `docs/source/sns/regulatory/`.

### 4.5 انتشار solucionador y البوابة1. **Hook الحدث:** يرسل كل حدث تسجيل الى دفق احداث resolver (`tools/soradns-resolver` SSE).
   يشترك فريق solucionador ويسجل الفروقات عبر tailer الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **تحديث قالب GAR:** يجب على البوابات تحديث قوالب GAR المشار اليها بواسطة
   `canonical_gateway_suffix()` y توقيع قائمة `host_pattern`. خزن الفروقات في
   `artifacts/sns/gar/<date>.patch`.
3. **نشر archivo de zona:** استخدم هيكل archivo de zona الموضح في `roadmap.md` (nombre, ttl, cid, prueba)
   Aquí está Torii/SoraFS. Utilice JSON Norito y seleccione `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **فحص الشفافية:** شغل `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   للتأكد من بقاء التنبيهات خضراء. Utilice el dispositivo Prometheus para conectar el dispositivo.
5. **تدقيق البوابة:** سجل عينات رؤوس `Sora-*` (سياسة التخزين المؤقت، CSP, ملخص GAR)
   وارفقها بسجل الحوكمة لكي يثبت المشغلون ان البوابة قدمت الاسم الجديد مع حواجز الحماية المقصودة.

## 5. التليمترية والتقارير| الاشارة | المصدر | الوصف / الاجراء |
|---------|--------|-----------------|
| `sns_registrar_status_total{result,suffix}` | Adaptador de corriente Torii | عداد نجاح/خطا للتسجيلات، التجديدات، التجميدات، التحويلات؛ ينبه عندما يرتفع `result="error"` لكل لاحقة. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | مقاييس Torii | SLO para APIs تغذي لوحات مبنية على `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | solucionador de colas | تكشف ادلة قديمة او انحراف GAR؛ الحواجز معرفة في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI الحوكمة | عداد يزداد عند تفعيل ميثاق/ملحق؛ يستخدم لتسوية قرارات المجلس مقابل الملاحق المنشورة. |
| Calibre `guardian_freeze_active` | Guardián CLI | يتتبع نوافذ تجميد suave/duro لكل محدد؛ ناد SRE اذا بقيت القيمة `1` بعد SLA المعلن. |
| Indicadores clave de rendimiento KPI | المالية / الوثائق | ملخصات شهرية تنشر مع المذكرات التنظيمية؛ تضمها البوابة عبر [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) ليتمكن azafatas y من الوصول لنفس عرض Grafana. |

## 6. متطلبات الادلة والتدقيق| الاجراء | الادلة التي يجب ارشفتها | التخزين |
|---------|--------------------------|---------|
| تغيير الميثاق / السياسة | Utilice Norito para acceder a la CLI de KPI y al administrador. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| تسجيل / تجديد | حمولة `RegisterNameRequestV1`, `RevenueAccrualEventV1`, اثبات الدفع. | `artifacts/sns/payments/<tx>.json`, API de configuración. |
| مزاد | مانيفستات confirmar/revelar, بذرة العشوائية, جدول حساب الفائز. | `artifacts/sns/auctions/<name>/`. |
| تجميد / فك تجميد | تذكرة guardian, تجزئة تصويت المجلس، رابط سجل الحادث، قالب تواصل العملاء. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| انتشار resolver | Para crear un archivo de zona/GAR, utilice JSONL en el tailer y utilice Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| الاستقبال التنظيمي | مذكرة استقبال، متعقب المواعيد النهائية, اقرار Steward, ملخص تغيير KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة بوابات المرحلة| المرحلة | معايير الخروج | حزمة الادلة |
|---------|--------------|-------------|
| N0 - بيتا مغلقة | Utilice la CLI para SN-1/SN-2 y guarde el dispositivo. | حركة الميثاق + ACK Steward, سجلات تجربة جافة للمسجل, تقرير شفافية resolver, ادخال في `ops/drill-log.md`. |
| N1 - اطلاق عام | مزادات + شرائح اسعار ثابتة مفعلة لـ`.sora`/`.nexus`, مسجل ذاتي الخدمة, مزامنة تلقائية للresolver, لوحات فوترة. | فرق ورقة التسعير، نتائج CI للمسجل، ملحق الدفع/KPI, مخرجات tailer الشفافية, ملاحظات تمرين الحوادث. |
| N2 - توسع | `.dao`, un revendedor y un administrador de cuentas. | لقطات شاشة للبوابة، مقاييس SLA للنزاعات، صادرات بطاقات تقييم Steward, ميثاق حوكمة محدث يشير لسياسات revendedor. |

تتطلب مخارج المراحل تدريبات tabletop مسجلة (مسار تسجيل ناجح، تجميد، عطل resolver)
مع ارفاق الاثار في `ops/drill-log.md`.

## 8. الاستجابة للحوادث والتصعيد| المشغل | الشدة | المالك الفوري | الاجراءات الالزامية |
|--------|-------|---------------|----------------------|
| انحراف resolver/GAR او ادلة قديمة | Septiembre 1 | SRE resolver + مجلس guardián | استدعاء on-call للresolver, التقاط مخرجات tailer, تقرير ما اذا كان يجب تجميد الاسماء المتاثرة, نشر تحديث حالة كل 30 mín. |
| تعطل المسجل، فشل الفوترة، او اخطاء API واسعة | Septiembre 1 | مدير مناوبة المسجل | ايقاف المزادات الجديدة، التحول الى CLI يدوي، اخطار stewards/الخزينة، ارفاق سجلات Torii بوثيقة الحادث. |
| نزاع اسم واحد، عدم تطابق الدفع، او تصعيد عميل | Septiembre 2 | mayordomo + قائد الدعم | Asegúrese de que el software esté instalado en el software soft, y que esté conectado a SLA, o que esté instalado en el sistema. |
| ملاحظة تدقيق امتثال | Septiembre 2 | جهة اتصال الامتثال | صياغة معالجة, حفظ مذكرة تحت `docs/source/sns/regulatory/`, جدولة جلسة مجلس متابعة. |
| تدريب او بروفة | Septiembre 3 | PM البرنامج | تنفيذ السيناريو المبرمج من `ops/drill-log.md`, ارشفة الاثار، ووسم الفجوات كمهام في roadmap. |

يجب على كل الحوادث ان تنشئ `incident/YYYY-MM-DD-sns-<slug>.md` مع جداول الملكية
وسجلات الاوامر ومراجع الادلة المنتجة عبر هذا الدليل.

## 9. المراجع

-[`registry-schema.md`](./registry-schema.md)
-[`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG y ADDR)حافظ على تحديث هذا الدليل كلما تغيرت صياغة الميثاق او اسطح CLI او عقود
التليمترية; يجب ان تطابق عناصر hoja de ruta التي تشير الى
`docs/source/sns/governance_playbook.md` احدث مراجعة دائما.