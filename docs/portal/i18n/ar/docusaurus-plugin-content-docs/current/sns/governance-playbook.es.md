---
lang: ar
direction: rtl
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sns/governance_playbook.md` والآن سيرف كومو
La copie Canonica del Portal. يستمر الملف في الحفاظ على علاقات التجارة.
:::

# دليل إدارة خدمة أسماء الأسماء (SN-6)

**الحالة:** Borrador 2026-03-24 - مرجع حي للتحضير SN-1/SN-6  
**تتضمن خريطة الطريق:** SN-6 "الامتثال وحل النزاعات"، SN-7 "Resolver & Gateway Sync"، سياسة التوجيه ADDR-1/ADDR-5  
**المتطلبات المسبقة:** مفتاح التسجيل في [`registry-schema.md`](./registry-schema.md)، اتفاقية API للمسجل في [`registrar-api.md`](./registrar-api.md)، دليل UX للأوامر في [`address-display-guidelines.md`](./address-display-guidelines.md)، وأنظمة إنشاء الحسابات في [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

تصف قواعد اللعبة هذه كيفية إدارة خدمة أسماء Sora (SNS)
اعتماد البطاقات، والسجلات المناسبة، وتصعيد النزاعات، والتأكد من حالات
يتم المزامنة دائمًا للمحلل والبوابة. استكمال متطلبات خريطة الطريق
ما هو CLI `sns governance ...` والبيانات Norito والمصنوعات اليدوية
تشارك القاعة في مرجع تشغيلي واحد قبل N1 (lanzamiento
العامة).

## 1. القراءة والجمهور

المستند موجه إلى:- أعضاء مجلس المستشارين الحكوميين الذين صوتوا للبطاقات والسياسة والصوفيات
  نتائج الخلافات.
- أعضاء مجلس الحراس الذين يصدرون تحذيرات الطوارئ و
  reversiones revisan.
- مشرفو الصوفية الذين يقومون بتشغيل كولا دي المسجل، ومساعدتهم
  إدارة الإدخالات.
- مشغلو المحلل/البوابة المسؤولون عن نشر SoraDNS،
  تحديثات GAR وحواجز الحماية عن بعد.
- معدات الإخلاص والمساحة والدعم التي يجب أن تثبتها جميعًا
  Accion de gobernanza dejo artefactos Norito Auditables.

قم بتكسير واجهات البيت المغلق (N0) والمساحة العامة (N1) والتوسيع (N2)
listadas en `roadmap.md`, vinculando كل تدفق من العمل مع الأدلة،
 لوحات المعلومات وخطوات التصعيد المطلوبة.

## 2. الأدوار وخريطة جهات الاتصال| رول | المبادئ الأساسية للمسؤولية | المصنوعات اليدوية وأساسيات القياس عن بعد | التصعيد |
|------|---------------------------------------------|------|----------|
| مجلس الحكومة | تنقيح وتصديق الخرائط، وسياسات الصوفيين، وأحكام النزاع، وتناوب المشرفين. | `docs/source/sns/governance_addenda/`، `artifacts/sns/governance/*`، صوت نصيحة التخزين عبر `sns governance charter submit`. | رئاسة المجلس + متتبع جدول أعمال الحكومة. |
| المجلس العسكري للجارديان | تنبعث منها كتل ناعمة/صلبة، وقوانين الطوارئ، ومراجعات لمدة 72 ساعة. | تذاكر الوصي الصادرة حسب `sns governance freeze`، بيان التجاوز في `artifacts/sns/guardian/*`. | Guardia عند الطلب (<= 15 دقيقة ACK). |
| مضيفو الصوفية | مشغل كولا المسجل، الفرعي، مستويات السعر والتواصل مع العملاء؛ أتعرف على المجاملين. | سياسة الوكيل في `SuffixPolicyV1`، مرجعيات الأسعار، اتهامات الوكيل بالإضافة إلى المذكرات التنظيمية. | قائد برنامج المشرفين + PagerDuty للصوفي. |
| عمليات التسجيل والتسوية | نقاط نهاية التشغيل `/v1/sns/*`، ونقاط التوفيق، وإصدار القياس عن بعد، والحفاظ على لقطات CLI. | API للمسجل ([`registrar-api.md`](./registrar-api.md)) ومقاييس `sns_registrar_status_total` واختبار الدفع و`artifacts/sns/payments/*`. | المدير المناوب للمسجل وربط وحدة التخزين. || مشغلي الحل والبوابة | الحفاظ على SoraDNS وGAR وحالة البوابة المخصصة لأحداث المسجل؛ إرسال مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)، `dashboards/alerts/soradns_transparency_rules.yml`. | SRE للحل عند الطلب + بوابة العمليات. |
| Tesoreria y Finanzas | تطبيق Reparto 70/30، اقتطاعات المراجع، الإعلانات المالية/التسويات، وشهادات SLA. | بيانات تراكم المدخلات، والتصدير الشريطي/المخزن، وملحقات مؤشرات الأداء الرئيسية الثلاثية في `docs/source/sns/regulatory/`. | المراقب المالي + المسؤول الرسمي عن الوفاء. |
| Enlace de Cumplimiento y Regulacion | تقوم بالالتزامات العالمية (الاتحاد الأوروبي DSA، وما إلى ذلك)، وتحديث مواثيق مؤشرات الأداء الرئيسية وتقديم الإفصاحات. | المذكرات التنظيمية في `docs/source/sns/regulatory/`، الأسطح المرجعية، المدخلات `ops/drill-log.md` لسطح الطاولة. | قائد برنامج الولاء. |
| Soporte / SRE عند الطلب | طرق الحوادث (الاصطدامات، انحراف التجزيء، طرق الحل)، تنسيق الرسائل إلى العملاء، وهي نتيجة لدفاتر التشغيل. | نباتات الحوادث، `ops/drill-log.md`، الأدلة المعملية، النسخ Slack/war-room و`incident/`. | Rotacion عند الطلب SNS + gestion SRE. |

## 3. المصنوعات اليدوية ومصادر البيانات| قطعة أثرية | أوبيكاسيون | اقتراح |
|----------|---------|---------|
| كارتا + أنيكسوس KPI | `docs/source/sns/governance_addenda/` | مذكرات الشركات مع التحكم في الإصدار، واتفاقيات مؤشرات الأداء الرئيسية، وقرارات الإدارة المرجعية بناءً على تصويتات CLI. |
| Esquema de Registro | [`registry-schema.md`](./registry-schema.md) | الهياكل الأساسية Norito canonicas (`NameRecordV1`، `SuffixPolicyV1`، `RevenueAccrualEventV1`). |
| عقد التسجيل | [`registrar-api.md`](./registrar-api.md) | الحمولات النافعة REST/gRPC ومقاييس `sns_registrar_status_total` وخطاف الإدارة المتوقع. |
| دليل تجربة المستخدم | [`address-display-guidelines.md`](./address-display-guidelines.md) | تم إصدار Canonicos IH58 (المفضل) والاشتراكات (الخيار الأفضل الثاني) من قبل المحافظ/المستكشفين. |
| مستندات SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | الاشتقاق الحتمي للمضيفين، تدفق تخصيص الشفافية وأنظمة التنبيهات. |
| مذكرات تنظيمية | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (p. ej., EU DSA), acuses desteward, anexos plantilla. |
| سجل الحفر | `ops/drill-log.md` | سجل طلبات البحث و IR المطلوبة قبل البدء بالخطوة. |
| Almacen de artifactos | `artifacts/sns/` | اختبار الدفع، وتذاكر الوصي، وفرق الحل، وتصدير مؤشرات الأداء الرئيسية، وإخراج `sns governance ...`. |يجب أن تشير جميع إجراءات الإدارة إلى قطعة أثرية على الأقل
 الجدول الأمامي حتى يتمكن المدققون من إعادة بناء قرار القرار خلال 24 عامًا
 ساعات.

## 4. قواعد اللعبة الخاصة بدراجة الحياة

### 4.1 نماذج المذكرة والمضيف

| باسو | دوينو | CLI / إيفيدنسيا | نوتاس |
|------|-------|----------------|-------|
| Redactar addendum y deltas KPI | مقرر المستشار + قائد المضيف | تخفيض بلانتيلا في `docs/source/sns/governance_addenda/YY/` | قم بتضمين معرفات ميثاق KPI وخطافات القياس عن بعد وشروط التنشيط. |
| عرض تقديمي | رئاسة المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (إنتاج `CharterMotionV1`) | يوضح La CLI أن Norito و`artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimento de Guardians | المستشار + الأوصياء | `sns governance ballot cast --proposal <id>` و `sns governance guardian-ack --proposal <id>` | إضافة دقائق مع التجزئة واختبار النصاب القانوني. |
| قبول ستيوارد | برنامج المشرفين | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل التغيير السياسي للصوفيات؛ حماية حول `artifacts/sns/governance/<id>/steward_ack.json`. |
| التنشيط | عمليات التسجيل | تم تحديث `SuffixPolicyV1`، وقم بتحديث ذاكرة التخزين المؤقت للمسجل، ونشر الملاحظة في `status.md`. | الطابع الزمني للتنشيط في `sns_governance_activation_total`. |
| سجل التدقيق | الولاء | قم بإضافة `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` وسجل الحفر إلى سطح الطاولة. | قم بتضمين مراجع ولوحات معلومات القياس عن بعد والاختلافات السياسية. |

### 4.2 السجلات والأجزاء الفرعية ومواصفات الأسعار1. **الاختبار المبدئي:** يراجع المسجل `SuffixPolicyV1` لتأكيد مستوى
   السعر والمحطات المتاحة ونوافذ النعمة/التخفيض. صيانة هوجاس دي
   الأسعار المتزامنة مع لوحة المستويات 3/4/5/6-9/10+ (قاعدة المستوى +
   معاملات الصوفية) موثقة في خريطة الطريق.
2. ** العطاء المختوم من Subastas: ** للمسابح المتميزة، تنفيذ 72 ساعة التزام /
   كشف على مدار 24 ساعة عبر `sns governance auction commit` / `... reveal`. نشر لا
   قائمة الالتزامات (التجزئة المنفردة) في `artifacts/sns/auctions/<name>/commit.json`
   حتى يتمكن المدققون من التحقق من التباين.
3. **التحقق من الدفع:** المسجلون الصالحون `PaymentProofV1` ضد
   إعادة تجهيز الملابس (70% تخزين الطعام / 30% مضيف مع اقتطاع المرجع <=10%).
   قم بحماية JSON Norito و`artifacts/sns/payments/<tx>.json` وقم بحفظه على
   رد المسجل (`RevenueAccrualEventV1`).
4. **Hook de gobernanza:** Adjuntar `GovernanceHookV1` para nombres premium/guarded
   مع مرجعيات مقترحة للاستشارات وشركات الضيافة. خطافات
   نتيجة خافتة في `sns_err_governance_missing`.
5. **التنشيط + مزامنة المحلل:** مرة واحدة يقوم Torii بإصدار الحدث
   سجل، قم بإلغاء تخصيص جزء الشفافية من المحلل للتأكيد
   تم نشر منطقة GAR/zone الجديدة (الإصدار 4.5).
6. **إفصاح العميل:** تحديث دفتر الأستاذ الموجه للعميل
   (المحفظة/المستكشف) عبر los Installations compartidos en[`address-display-guidelines.md`](./address-display-guidelines.md)، آمن
   تتوافق أجهزة IH58 المقدمة والمشتملة مع دليل النسخ/QR.

### 4.3 تجديدات وتفتيت وتسوية الهياكل- **تدفق التجديد:** يطبق المسجلون النوافذ بفضل الشكر
  30 يومًا + تخفيض 60 يومًا محددًا في `SuffixPolicyV1`. بعد 60
  dias، la secuencia de reapertura holandesa (7 dias، tarifa 10x Decayendo 15٪ / dia)
  يتم تنشيطه تلقائيًا عبر `sns governance reopen`.
- **Reparto de ingresos:** Cada Renovacion o Transferencia create un
  `RevenueAccrualEventV1`. لا تزال تصديرات الملابس (CSV/Parquet) مطلوبة
  التوفيق بين هذه الأحداث يوميًا ؛ مساعد pruebas أون
  `artifacts/sns/treasury/<date>.json`.
- **اقتطاعات الإحالة:** يتم عرض نقاط الإحالة الاختيارية بواسطة
  سوفيجو أغريغاندو `referral_share` بسياسة المضيف. المسجلون
  قم بإصدار الانقسام النهائي وحافظ على بيانات الإحالة جنبًا إلى جنب مع اختبار الدفع.
- **عدد التقارير:** نشر البيانات المالية لمؤشرات الأداء الرئيسية (السجلات،
  التجديدات، ARPU، استخدام المنازعات/السندات) ar
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. لوحات المعلومات ديبن تومار
  نفس الجداول المصدرة بحيث تتزامن أرقام Grafana مع
  دليل دفتر الأستاذ.
- **مراجعة مؤشرات الأداء الرئيسية:** نقطة التفتيش التمهيدية لأعمال التجارة في القيادة
  المالية، والمضيف، ورئيس البرنامج. فتح [لوحة معلومات SNS KPI](./kpi-dashboard.md)
  (تضمين البوابة `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`)،
  تصدير جداول الإنتاجية والإيرادات للمسجل، ودلتا المسجل، والملحق والمصنوعات اليدوية للمذكرة. قم بإلغاء الحادث في حالة المراجعة
  encuentra brechas SLA (نوافذ التجميد > 72 ساعة، صور خطأ في المسجل،
  الانجراف دي ARPU).

### 4.4 المراسلات والمنازعات والمكالمات| فاس | دوينو | العمل والأدلة | جيش تحرير السودان |
|-------|-------|------------------|-----|
| طلب التجميد الناعم | ستيوارد / سوبورتي | تقديم التذكرة `SNS-DF-<id>` مع التحقق من الدفع، ومراجعة سندات النزاع والمحدد (المحددات) المتأثرة. | <=4 ساعات من الإدخال. |
| تذكرة الوصي | مجلس الحراس | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` ينتج `GuardianFreezeTicketV1`. حماية التذكرة JSON على `artifacts/sns/guardian/<id>.json`. | <=30 دقيقة ACK، <=2 ساعة قذف. |
| التصديق على النصيحة | مستشار الحكومة | Aprobar o rechazar congelamientos، وثيقة القرار المضمن في تذكرة الوصي وملخص رابطة النزاع. | الجلسة القادمة للمشورة أو التصويت غير المتزامن. |
| لوحة التحكيم | الولاء + ستيوارد | لوحة استدعاء مكونة من 7 جورادوس (خريطة الطريق الثانية) مع سلاسل من الرصاص عبر `sns governance dispute ballot`. إضافة إيصالات صوتية مجهولة المصدر إلى حزمة الأحداث. | الحكم <=7 أيام بعد إيداع السندات. |
| أبيلاسيون | الحراس + المستشار | الالتماسات تكرر السند وتكرر عملية التحكيم؛ يوضح المسجل Norito `DisputeAppealV1` والتذكرة المرجعية الأولية. | <=10 دياس. |
| إزالة الخلل والعلاج | المسجل + عمليات الحل | قم بتشغيل `sns governance unfreeze --selector <IH58> --ticket <id>`، وقم بتحديث حالة المسجل ونشر اختلافات GAR/resolver. | على الفور بعد صدور الحكم. |قوانين الطوارئ (التجميد النشط للمراقبين <= 72 ساعة) متواصلة
نفس التدفق ولكنه يتطلب مراجعة بأثر رجعي للمشورة ومذكرة
الشفافية في `docs/source/sns/regulatory/`.

### 4.5 نشر المحلل والبوابة

1. **ربط الحدث:** يصدر كل حدث تسجيلي تيارًا من أحداث ديل
   محلل (`tools/soradns-resolver` SSE). تم الاشتراك في عملية الحل
   يختلف التسجيل عبر أداة الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. ** تحديث نباتات GAR: ** تقوم البوابات بتحديث نباتات GAR
   المراجع لـ `canonical_gateway_suffix()` وإعادة تثبيت القائمة
   `host_pattern`. يختلف Guardar في `artifacts/sns/gar/<date>.patch`.
3. **نشر ملف Zonefile:** استخدام هيكل ملف Zonefile الموضح باللغة الإنجليزية
   `roadmap.md` (الاسم، ttl، cid، الإثبات) وقم بإدخال Torii/SoraFS. أرشيفار إل
   JSON Norito و`artifacts/sns/zonefiles/<name>/<version>.json`.
4. **فحص الشفافية:** إخراج `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   للتأكد من أن التنبيهات تظل خضراء دائمًا. أضف خروج النص
   de Prometheus في تقرير الشفافية الأسبوعي.
5. **تدقيق البوابة:** مسجل الرؤوس `Sora-*` (سياسة
   ذاكرة التخزين المؤقت وCSP وملخص GAR) وإضافات إلى سجل الإدارة لذلك
   يمكن للمشغلين التحقق من أن البوابة ستستقبل الاسم الجديد مع الأشخاص
   الدرابزين previstos.

## 5. تقارير القياس عن بعد| سينال | فوينتي | الوصف / العمل |
|--------|--------|---------------------|
| `sns_registrar_status_total{result,suffix}` | معالجات المسجل Torii | أداة قياس الخروج/الخطأ في السجلات والتجديدات والجمعيات والتحويلات؛ تنبيه عند `result="error"` تحت التشغيل. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | مقاييس Torii | SLOs زمن الاستجابة لمعالجات API؛ لوحات المعلومات الغذائية مبنية على `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` و`soradns_bundle_cid_drift_total` | خياط الشفافية للحل | اكتشاف الشوائب القديمة أو انجراف GAR؛ تم تحديد حواجز الحماية في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | الحوكمة CLI | تتم زيادة المبلغ عند تفعيل ميثاق/ملحق؛ إذا كانت الولايات المتحدة الأمريكية قادرة على التوفيق بين قرارات المستشارين والإضافات المنشورة. |
| مقياس `guardian_freeze_active` | الجارديان CLI | فتحات تجميد راستريا ناعمة/صلبة؛ Paginar a SRE si el valor queda en `1` mas alla del SLA المعلنة. |
| لوحات المعلومات الملحقة بمؤشرات الأداء الرئيسية | التمويل / المستندات | مجموعات البيانات الشهرية المنشورة جنبًا إلى جنب مع المذكرات التنظيمية; يتم تضمين البوابة عبر [لوحة معلومات SNS KPI](./kpi-dashboard.md) حتى يتمكن المشرفون والمنظمون من الوصول إلى نفس مشهد Grafana. |

## 6. متطلبات الأدلة والاستماع| أكسيون | الأدلة الأرشيفية | الماسن |
|--------|----------------------|---------|
| كامبيو دي كارتا / سياسة | بيان Norito الثابت، نسخة CLI، فرق مؤشرات الأداء الرئيسية، تهمة المضيف. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| التسجيل / التجديد | الحمولة النافعة `RegisterNameRequestV1`، `RevenueAccrualEventV1`، اختبار الدفع. | `artifacts/sns/payments/<tx>.json`، سجلات API للمسجل. |
| سوباسطة | بيانات الالتزام/الكشف، جزء من التقسيم، جدول بيانات الربح. | `artifacts/sns/auctions/<name>/`. |
| كونغيلار / ديسكونجيلار | تذكرة الوصي، وتجزئة النصيحة، وعنوان URL لسجل الأحداث، وإنشاء اتصالات العميل. | `artifacts/sns/guardian/<ticket>/`، `incident/<date>-sns-*.md`. |
| نشر الحل | يختلف Zonefile/GAR، وهو مستخرج من JSONL من الخياط، ولقطة Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| إنغريسو التنظيمية | مذكرة إدخال، وتعقب المواعيد النهائية، وتوجيه الاتهام إلى المضيف، واستئناف تغييرات مؤشرات الأداء الرئيسية. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة التحقق من بوابات العمل| فاس | معايير الخروج | حزمة الأدلة |
|-------|--------------------|--------------------|
| N0 - بيتا سيرادا | مفتاح التسجيل SN-1/SN-2، دليل المسجل CLI، حفر الوصي مكتمل. | بطاقة الحركة + إقرار المضيف، سجلات التشغيل الجاف للمسجل، تقرير شفافية المحلل، تم إدخاله في `ops/drill-log.md`. |
| N1 - Lanzamiento publico | Subastas + مستويات السعر النشط لـ `.sora`/`.nexus`، الخدمة الذاتية للمسجل، المزامنة التلقائية للمحلل، لوحات المعلومات. | تختلف أسعار اليوم، ونتائج CI للمسجل، وملحق الدفع/مؤشر الأداء الرئيسي، وإخراج خياط الشفافية، وملاحظات الحادث. |
| N2 - التوسع | `.dao`، واجهات برمجة تطبيقات الموزعين، بوابة النزاعات، بطاقات أداء المضيفين، لوحات المعلومات التحليلية. | تسجيلات البوابة، ومقاييس مستوى الخدمة للمنازعات، وتصدير بطاقات أداء المضيفين، وميثاق الإدارة الذي تم تحديثه من خلال سياسات الموزع. |

يتطلب Las Salidas de Fase تدريبات على سطح الطاولة (تسجيل المسار السعيد،
تجميد، انقطاع التيار الكهربائي) مع العناصر الإضافية في `ops/drill-log.md`.

## 8. الرد على الأحداث والتصعيد| الزناد | سيفريداد | دوينو فوري | الأعمال الإلزامية |
|---------|---------|----------------|-----------------------|
| انجراف المحلل/GAR أو اختبار العناصر القديمة | سيف 1 | محلل SRE + مجلس الحراس | قم بصفحة المحلل عند الطلب، والتقط نتائج الخياط، وحدد ما إذا كنت تقوم بجمع الأسماء المتضررة، ونشر الحالة كل 30 دقيقة. |
| صندوق المسجل، بسبب تجزئة أو أخطاء API العامة | سيف 1 | المدير المناوب للمسجل | منع العناصر الفرعية الجديدة، وتغيير دليل CLI، وإخطار المشرفين/المشرفين، والسجلات الإضافية لـ Torii في مستند الحادث. |
| نزاع حول الاسم الفريد أو عدم تطابق الدفع أو تصعيد العميل | سيف 2 | ستيوارد + ليدر دي سوبورتي | Recopilar pruebas de pago، وتقرير ما إذا كان هناك خطأ في التجميد الناعم، والرد على المحامي في SLA، والتسجيل الناتج وتتبع النزاع. |
| قاعة الاستماع للوفاء | سيف 2 | الحصول على الولاء | قم بتحرير خطة الإصلاح، وأرشفة المذكرة في `docs/source/sns/regulatory/`، وجدول جلسة مشورة المتابعة. |
| حفر يا انسايو | سيف 3 | PM ديل البرنامج | قم بتنفيذ السيناريو الموضح من `ops/drill-log.md`، وحفظ العناصر، وتحديد الفجوات كأهداف لخريطة الطريق. |

يجب إنشاء جميع الأحداث `incident/YYYY-MM-DD-sns-<slug>.md` مع اللوحات
الملكية وسجلات الأوامر والمراجع إلى الأدلة المنتجة في هذا الشأن
قواعد اللعبة التي تمارسها.

## 9. المراجع- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (أقسام SNS، DG، ADDR)

حافظ على تحديث قواعد اللعبة هذه عندما تقوم بتغيير نص البطاقة
أسطح CLI أو عقود القياس عن بعد؛ لاس مداخل خارطة الطريق كيو
المرجع `docs/source/sns/governance_playbook.md` deben syncidir siempre con
المراجعة الأحدث.