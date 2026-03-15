---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 275059232323a70e1d1a4eb3763057d7e36da2f198806f4a97d31a77c2963c80
source_last_modified: "2026-01-20T13:32:56+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: governance-playbook
lang: ar
direction: rtl
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/governance_playbook.md` وتعمل الان كمرجع بوابة
موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

# دليل حوكمة خدمة أسماء سورا (SN-6)

**الحالة:** صيغ 2026-03-24 - مرجع حي لاستعداد SN-1/SN-6  
**روابط خارطة الطريق:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", سياسة العناوين ADDR-1/ADDR-5  
**المتطلبات المسبقة:** مخطط السجل في [`registry-schema.md`](./registry-schema.md)، عقد API للمسجل في [`registrar-api.md`](./registrar-api.md)، ارشادات تجربة العناوين في [`address-display-guidelines.md`](./address-display-guidelines.md)، وقواعد بنية الحساب في [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

يصف هذا الدليل كيف تعتمد هيئات حوكمة خدمة أسماء سورا (SNS) المواثيق، وتوافق على
التسجيلات، وتصعد النزاعات، وتثبت ان حالات resolver وgateway تبقى متزامنة. يفي
بمتطلب خارطة الطريق بان تتشارك CLI `sns governance ...` ومانيفستات Norito
والاثار التدقيقية مرجعا تشغيليا واحدا قبل N1 (الاطلاق العام).

## 1. النطاق والجمهور

يستهدف المستند:

- اعضاء مجلس الحوكمة الذين يصوتون على المواثيق، وسياسات اللاحقات، ونتائج النزاعات.
- اعضاء مجلس guardian الذين يصدرون تجميدات طارئة ويراجعون التراجعات.
- stewards اللاحقات الذين يديرون طوابير المسجل، ويوافقون على المزادات، ويديرون
  تقسيمات الايرادات.
- مشغلو resolver/gateway المسؤولون عن انتشار SoraDNS، وتحديثات GAR، وحواجز
  التليمترية.
- فرق الامتثال والخزينة والدعم التي يجب ان تثبت ان كل اجراء حوكمة ترك اثار Norito
  قابلة للتدقيق.

يغطي مراحل البيتا المغلقة (N0) والاطلاق العام (N1) والتوسع (N2) المدرجة في
`roadmap.md` من خلال ربط كل سير عمل بالادلة المطلوبة ولوحات المتابعة ومسارات
التصعيد.

## 2. الادوار وخريطة الاتصال

| الدور | المسؤوليات الاساسية | ابرز الاثار والتليمترية | التصعيد |
|------|----------------------|-------------------------|---------|
| مجلس الحوكمة | يصيغ ويصادق على المواثيق، وسياسات اللاحقات، واحكام النزاعات، وتناوب stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, بطاقات تصويت المجلس المخزنة عبر `sns governance charter submit`. | رئيس المجلس + متعقب جدول اعمال الحوكمة. |
| مجلس guardian | يصدر تجميدات soft/hard، وقوانين طارئة، ومراجعات 72 h. | تذاكر guardian الصادرة عبر `sns governance freeze`، ومانيفستات التجاوز المسجلة تحت `artifacts/sns/guardian/*`. | دورية guardian on-call (<=15 min ACK). |
| stewards اللاحقة | يديرون طوابير المسجل، والمزادات، وشرائح التسعير، واتصالات العملاء؛ ويقرون بالامتثال. | سياسات steward في `SuffixPolicyV1`, جداول مرجعية للتسعير, اقرارات steward المخزنة بجانب المذكرات التنظيمية. | قائد برنامج steward + PagerDuty خاص بكل لاحقة. |
| عمليات المسجل والفوترة | تشغل نقاط `/v1/sns/*`, وتسوي المدفوعات, وتصدر التليمترية, وتحافظ على لقطات CLI. | API المسجل ([`registrar-api.md`](./registrar-api.md)), مقاييس `sns_registrar_status_total`, اثباتات الدفع المؤرشفة تحت `artifacts/sns/payments/*`. | مدير مناوبة المسجل ورابط الخزينة. |
| مشغلو resolver والبوابة | يحافظون على SoraDNS وGAR وحالة البوابة متوافقة مع احداث المسجل؛ ويبثون مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver on-call + جسر عمليات البوابة. |
| الخزينة والمالية | تطبق تقسيم ايرادات 70/30، واقتطاعات referral، وملفات الضرائب/الخزينة، وشهادات SLA. | مانيفستات تراكم الايرادات، صادرات Stripe/الخزينة، ملاحق KPI ربع سنوية تحت `docs/source/sns/regulatory/`. | مراقب المالية + مسؤول الامتثال. |
| جهة اتصال الامتثال والتنظيم | تتبع الالتزامات العالمية (EU DSA، الخ)، وتحدث مواثيق KPI، وتقدم الافصاحات. | مذكرات تنظيمية في `docs/source/sns/regulatory/`, عروض مرجعية, ومدخلات `ops/drill-log.md` لتجارب الطاولة. | قائد برنامج الامتثال. |
| الدعم / SRE عند الطلب | يعالج الحوادث (تصادمات، انحرافات فوترة، اعطال resolver)، وينسق رسائل العملاء، ويمتلك الادلة التشغيلية. | قوالب الحوادث، `ops/drill-log.md`, ادلة مختبر مرحلية, ونصوص Slack/war-room المؤرشفة تحت `incident/`. | دورية on-call لـSNS + ادارة SRE. |

## 3. الاثار المرجعية ومصادر البيانات

| الاثر | الموقع | الغرض |
|-------|--------|-------|
| الميثاق + ملاحق KPI | `docs/source/sns/governance_addenda/` | مواثيق موقعة مع تحكم بالنسخ، ومواثيق KPI، وقرارات الحوكمة المشار اليها بتصويتات CLI. |
| مخطط السجل | [`registry-schema.md`](./registry-schema.md) | تراكيب Norito المرجعية (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| عقد المسجل | [`registrar-api.md`](./registrar-api.md) | حمولات REST/gRPC، مقاييس `sns_registrar_status_total`، وتوقعات حوكمة hooks. |
| دليل UX للعناوين | [`address-display-guidelines.md`](./address-display-guidelines.md) | عروض I105 (المفضلة) والمضغوطة (الخيار الثاني) المرجعية التي تعكسها المحافظ/المستكشفات. |
| وثائق SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | اشتقاق المضيفات الحتمي، سير عمل tailer للشفافية، وقواعد التنبيه. |
| مذكرات تنظيمية | `docs/source/sns/regulatory/` | ملاحظات استقبال حسب الولاية (مثل EU DSA)، اقرارات steward، ملاحق قوالب. |
| سجل drills | `ops/drill-log.md` | سجل لتجارب الفوضى وIR المطلوبة قبل الخروج من المراحل. |
| تخزين الاثار | `artifacts/sns/` | اثباتات الدفع، تذاكر guardian، فروقات resolver، صادرات KPI، والمخرجات الموقعة من CLI الناتجة عن `sns governance ...`. |

يجب ان تشير كل اجراءات الحوكمة الى اثر واحد على الاقل من الجدول اعلاه حتى يتمكن
المدققون من اعادة بناء مسار القرار خلال 24 ساعة.

## 4. ادلة دورة الحياة

### 4.1 حركات الميثاق وsteward

| الخطوة | المالك | CLI / الدليل | ملاحظات |
|--------|--------|---------------|---------|
| صياغة الملحق وفروق KPI | مقرر المجلس + قائد steward | قالب Markdown مخزن تحت `docs/source/sns/governance_addenda/YY/` | تضمين معرفات مواثيق KPI، hooks تليمترية، وشروط التفعيل. |
| تقديم الاقتراح | رئيس المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (ينتج `CharterMotionV1`) | تصدر CLI مانيفست Norito مخزن في `artifacts/sns/governance/<id>/charter_motion.json`. |
| تصويت واعتراف guardian | المجلس + guardians | `sns governance ballot cast --proposal <id>` و `sns governance guardian-ack --proposal <id>` | ارفاق محاضر مجزأة واثباتات النصاب. |
| قبول steward | برنامج stewards | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل تغيير سياسات اللاحقات; سجل الظرف تحت `artifacts/sns/governance/<id>/steward_ack.json`. |
| التفعيل | عمليات المسجل | تحديث `SuffixPolicyV1`, تحديث ذاكرة المسجل المؤقتة, نشر ملاحظة في `status.md`. | يتم تسجيل طابع التفعيل في `sns_governance_activation_total`. |
| سجل التدقيق | الامتثال | اضافة مدخل الى `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` وسجل drills اذا تم تنفيذ tabletop. | تضمين مراجع لوحات التليمترية وفروقات السياسة. |

### 4.2 الموافقات على التسجيل والمزاد والتسعير

1. **الفحص المسبق:** يستعلم المسجل `SuffixPolicyV1` لتاكيد شريحة التسعير، الشروط
   المتاحة، ونوافذ السماح/الاسترداد. ابق جداول التسعير متزامنة مع جدول الشرائح
   3/4/5/6-9/10+ (الشريحة الاساسية + معاملات اللاحقة) الموثقة في roadmap.
2. **مزادات sealed-bid:** لمجموعات premium، نفذ دورة 72 h commit / 24 h reveal
   عبر `sns governance auction commit` / `... reveal`. انشر قائمة commits (hashes
   فقط) تحت `artifacts/sns/auctions/<name>/commit.json` حتى يتمكن المدققون من
   التحقق من العشوائية.
3. **التحقق من الدفع:** يتحقق المسجلون من `PaymentProofV1` مقابل تقسيمات الخزينة
   (70% خزينة / 30% steward مع carve-out referral <=10%). خزن JSON Norito تحت
   `artifacts/sns/payments/<tx>.json` واربطه في استجابة المسجل (`RevenueAccrualEventV1`).
4. **Hook الحوكمة:** ارفق `GovernanceHookV1` للاسماء premium/guarded مع مراجع
   لمعرفات مقترح المجلس وتواقيع steward. غياب hooks يؤدي الى
   `sns_err_governance_missing`.
5. **التفعيل + مزامنة resolver:** بمجرد ان يرسل Torii حدث التسجيل، شغل tailer
   الشفافية الخاص بالresolver لتاكيد انتشار الحالة الجديدة GAR/zone (انظر 4.5).
6. **افصاح العميل:** حدث دفتر المستخدِم (wallet/explorer) عبر fixtures المشتركة في
   [`address-display-guidelines.md`](./address-display-guidelines.md)، مع ضمان ان
   عروض I105 والمضغوط تتطابق مع توجيهات النص/QR.

### 4.3 التجديد والفوترة وتسوية الخزينة

- **سير عمل التجديد:** يفرض المسجلون نافذة سماح 30 يوم + نافذة استرداد 60 يوم
  المحددة في `SuffixPolicyV1`. بعد 60 يوما، تتفعل تلقائيا سلسلة اعادة الفتح
  الهولندية (7 ايام، رسوم 10x تنخفض 15%/يوم) عبر `sns governance reopen`.
- **تقسيم الايرادات:** كل تجديد او تحويل ينشئ `RevenueAccrualEventV1`. يجب على
  صادرات الخزينة (CSV/Parquet) التسوية مع هذه الاحداث يوميا; ارفق الاثباتات في
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs referral:** يتم تتبع نسب referral الاختيارية لكل لاحقة عبر اضافة
  `referral_share` الى سياسة steward. يصدر المسجلون التقسيم النهائي ويخزنون
  مانيـفستات referral بجانب اثبات الدفع.
- **وتيرة التقارير:** تنشر المالية ملاحق KPI شهرية (التسجيلات، التجديدات، ARPU،
  استخدام النزاعات/bond) تحت `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  يجب ان تعتمد لوحات المتابعة على الجداول المصدرة نفسها حتى تطابق ارقام Grafana
  ادلة الدفتر.
- **مراجعة KPI شهرية:** يجمع فحص اول ثلاثاء قائد المالية وsteward المناوب وPM
  البرنامج. افتح [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) (تضمين البوابة
  `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`)، صدّر جداول انتاجية
  المسجل والايرادات، سجل الفروقات في الملحق، وارفق الاثار بالمذكرة. فعّل حادثا
  اذا وجدت المراجعة خروقات SLA (نوافذ تجميد >72 h، ارتفاع اخطاء المسجل، انحراف ARPU).

### 4.4 التجميدات والنزاعات والاستئناف

| المرحلة | المالك | الاجراء والدليل | SLA |
|---------|--------|------------------|-----|
| طلب تجميد soft | steward / الدعم | قدم تذكرة `SNS-DF-<id>` مع اثباتات الدفع، مرجع bond النزاع، والمحدد/المحددات المتاثرة. | <=4 h من الاستلام. |
| تذكرة guardian | مجلس guardian | `sns governance freeze --selector <I105> --reason <text> --until <ts>` ينتج `GuardianFreezeTicketV1`. خزّن JSON التذكرة تحت `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h تنفيذ. |
| تصديق المجلس | مجلس الحوكمة | يوافق او يرفض التجميدات، ويوثق القرار مع رابط لتذكرة guardian وبصمة bond النزاع. | جلسة المجلس التالية او تصويت غير متزامن. |
| لجنة التحكيم | الامتثال + steward | عقد لجنة من 7 محلفين (حسب roadmap) مع بطاقات تصويت مجزأة عبر `sns governance dispute ballot`. ارفق ايصالات التصويت المجهولة بحزمة الحادث. | الحكم <=7 ايام بعد ايداع bond. |
| استئناف | guardian + المجلس | يضاعف الاستئناف bond ويعيد عملية المحلفين; سجل مانيفست Norito `DisputeAppealV1` واربط بالتذكرة الاصلية. | <=10 ايام. |
| فك التجميد والمعالجة | المسجل + عمليات resolver | نفذ `sns governance unfreeze --selector <I105> --ticket <id>`, حدث حالة المسجل، ومرر فروقات GAR/resolver. | مباشرة بعد الحكم. |

القوانين الطارئة (تجميدات يطلقها guardian <=72 h) تتبع نفس التدفق لكنها تتطلب
مراجعة مجلس بأثر رجعي وملاحظة شفافية تحت `docs/source/sns/regulatory/`.

### 4.5 انتشار resolver والبوابة

1. **Hook الحدث:** يرسل كل حدث تسجيل الى دفق احداث resolver (`tools/soradns-resolver` SSE).
   يشترك فريق resolver ويسجل الفروقات عبر tailer الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **تحديث قالب GAR:** يجب على البوابات تحديث قوالب GAR المشار اليها بواسطة
   `canonical_gateway_suffix()` واعادة توقيع قائمة `host_pattern`. خزن الفروقات في
   `artifacts/sns/gar/<date>.patch`.
3. **نشر zonefile:** استخدم هيكل zonefile الموضح في `roadmap.md` (name, ttl, cid, proof)
   وادفعه الى Torii/SoraFS. ارشف JSON Norito تحت `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **فحص الشفافية:** شغل `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   للتأكد من بقاء التنبيهات خضراء. ارفق مخرجات Prometheus النصية بالتقرير الاسبوعي للشفافية.
5. **تدقيق البوابة:** سجل عينات رؤوس `Sora-*` (سياسة التخزين المؤقت، CSP، ملخص GAR)
   وارفقها بسجل الحوكمة لكي يثبت المشغلون ان البوابة قدمت الاسم الجديد مع حواجز الحماية المقصودة.

## 5. التليمترية والتقارير

| الاشارة | المصدر | الوصف / الاجراء |
|---------|--------|-----------------|
| `sns_registrar_status_total{result,suffix}` | معالجات مسجل Torii | عداد نجاح/خطا للتسجيلات، التجديدات، التجميدات، التحويلات؛ ينبه عندما يرتفع `result="error"` لكل لاحقة. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | مقاييس Torii | SLO للكمون لمعالجات API؛ تغذي لوحات مبنية على `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` و `soradns_bundle_cid_drift_total` | tailer شفافية resolver | تكشف ادلة قديمة او انحراف GAR؛ الحواجز معرفة في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI الحوكمة | عداد يزداد عند تفعيل ميثاق/ملحق؛ يستخدم لتسوية قرارات المجلس مقابل الملاحق المنشورة. |
| `guardian_freeze_active` gauge | CLI guardian | يتتبع نوافذ تجميد soft/hard لكل محدد؛ ناد SRE اذا بقيت القيمة `1` بعد SLA المعلن. |
| لوحات ملاحق KPI | المالية / الوثائق | ملخصات شهرية تنشر مع المذكرات التنظيمية؛ تضمها البوابة عبر [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) ليتمكن stewards والمنظمون من الوصول لنفس عرض Grafana. |

## 6. متطلبات الادلة والتدقيق

| الاجراء | الادلة التي يجب ارشفتها | التخزين |
|---------|--------------------------|---------|
| تغيير الميثاق / السياسة | مانيفست Norito موقع، نص CLI، فرق KPI، اقرار steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| تسجيل / تجديد | حمولة `RegisterNameRequestV1`, `RevenueAccrualEventV1`, اثبات الدفع. | `artifacts/sns/payments/<tx>.json`, سجلات API للمسجل. |
| مزاد | مانيفستات commit/reveal، بذرة العشوائية، جدول حساب الفائز. | `artifacts/sns/auctions/<name>/`. |
| تجميد / فك تجميد | تذكرة guardian، تجزئة تصويت المجلس، رابط سجل الحادث، قالب تواصل العملاء. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| انتشار resolver | فرق zonefile/GAR، مقتطف JSONL من tailer، لقطة Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| الاستقبال التنظيمي | مذكرة استقبال، متعقب المواعيد النهائية، اقرار steward، ملخص تغيير KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة بوابات المرحلة

| المرحلة | معايير الخروج | حزمة الادلة |
|---------|--------------|-------------|
| N0 - بيتا مغلقة | مخطط سجل SN-1/SN-2، CLI مسجل يدوي، تدريب guardian مكتمل. | حركة الميثاق + ACK steward، سجلات تجربة جافة للمسجل، تقرير شفافية resolver، ادخال في `ops/drill-log.md`. |
| N1 - اطلاق عام | مزادات + شرائح اسعار ثابتة مفعلة لـ`.sora`/`.nexus`, مسجل ذاتي الخدمة، مزامنة تلقائية للresolver، لوحات فوترة. | فرق ورقة التسعير، نتائج CI للمسجل، ملحق الدفع/KPI، مخرجات tailer الشفافية، ملاحظات تمرين الحوادث. |
| N2 - توسع | `.dao`, واجهات reseller، بوابة النزاعات، بطاقات تقييم steward، لوحات تحليلات. | لقطات شاشة للبوابة، مقاييس SLA للنزاعات، صادرات بطاقات تقييم steward، ميثاق حوكمة محدث يشير لسياسات reseller. |

تتطلب مخارج المراحل تدريبات tabletop مسجلة (مسار تسجيل ناجح، تجميد، عطل resolver)
مع ارفاق الاثار في `ops/drill-log.md`.

## 8. الاستجابة للحوادث والتصعيد

| المشغل | الشدة | المالك الفوري | الاجراءات الالزامية |
|--------|-------|---------------|----------------------|
| انحراف resolver/GAR او ادلة قديمة | Sev 1 | SRE resolver + مجلس guardian | استدعاء on-call للresolver، التقاط مخرجات tailer، تقرير ما اذا كان يجب تجميد الاسماء المتاثرة، نشر تحديث حالة كل 30 min. |
| تعطل المسجل، فشل الفوترة، او اخطاء API واسعة | Sev 1 | مدير مناوبة المسجل | ايقاف المزادات الجديدة، التحول الى CLI يدوي، اخطار stewards/الخزينة، ارفاق سجلات Torii بوثيقة الحادث. |
| نزاع اسم واحد، عدم تطابق الدفع، او تصعيد عميل | Sev 2 | steward + قائد الدعم | جمع اثباتات الدفع، تحديد الحاجة الى تجميد soft، الرد على مقدم الطلب ضمن SLA، تسجيل النتيجة في متعقب النزاع. |
| ملاحظة تدقيق امتثال | Sev 2 | جهة اتصال الامتثال | صياغة خطة معالجة، حفظ مذكرة تحت `docs/source/sns/regulatory/`, جدولة جلسة مجلس متابعة. |
| تدريب او بروفة | Sev 3 | PM البرنامج | تنفيذ السيناريو المبرمج من `ops/drill-log.md`, ارشفة الاثار، ووسم الفجوات كمهام في roadmap. |

يجب على كل الحوادث ان تنشئ `incident/YYYY-MM-DD-sns-<slug>.md` مع جداول الملكية
وسجلات الاوامر ومراجع الادلة المنتجة عبر هذا الدليل.

## 9. المراجع

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (اقسام SNS وDG وADDR)

حافظ على تحديث هذا الدليل كلما تغيرت صياغة الميثاق او اسطح CLI او عقود
التليمترية; يجب ان تطابق عناصر roadmap التي تشير الى
`docs/source/sns/governance_playbook.md` احدث مراجعة دائما.
