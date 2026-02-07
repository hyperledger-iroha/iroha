---
lang: ar
direction: rtl
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sns/governance_playbook.md` وهي مستمرة
انسخ Canonique du portail. يستمر مصدر الملف في الحصول على نتائج البيع.
:::

# دليل إدارة خدمة اسم Sora (SN-6)

**الحالة:** Redige 2026-03-24 - مرجع حيوي للتحضير SN-1/SN-6  
**امتيازات خريطة الطريق:** SN-6 "الامتثال وحل النزاعات"، SN-7 "Resolver & Gateway Sync"، سياسة العناوين ADDR-1/ADDR-5  
**المتطلبات المسبقة:** مخطط التسجيل في [`registry-schema.md`](./registry-schema.md)، عقد واجهة برمجة التطبيقات للمسجل في [`registrar-api.md`](./registrar-api.md)، دليل عناوين تجربة المستخدم في [`address-display-guidelines.md`](./address-display-guidelines.md)، ويتم تنظيم بنية الحسابات في [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

يشير هذا الدليل إلى أجهزة إدارة خدمة Sora Name Service (SNS)
اعتماد المخططات، والموافقة على التسجيلات، وتصعيد الدعاوى، وما إلى ذلك
تأكد من مزامنة حالات وحدة الحل والبوابة. يرضي
يتم تحديد متطلبات خريطة الطريق من خلال CLI `sns governance ...`، والبيانات
Norito وعناصر التدقيق جزء من مرجع فريد من نوعه للمشغل
أفانت N1 (الرمح العام).

## 1. بورتي وآخرون

نوع الوثيقة:- أعضاء مجلس الحوكمة الذين يصوتون على المخططات والسياسات
  لاحقة ونتائج التقاضي.
- أعضاء مجلس الأوصياء الذين يصدرون طلبات الاستعجال والفحص
  الإرتدادات.
- مشرفو اللاحقة الذين يديرون ملفات المسجل، يوافقون على الإدخالات
  وإدارة مشاركات العائدات.
- مشغلو الحلول/البوابات المسؤولون عن نشر SoraDNS، من المهام
  jour GAR, et des garde-fous de telemetry.
- معدات المطابقة والخزانة والدعم التي تساعد في توضيح كل شيء
  إجراءات الحوكمة للسماح بالتدقيق Norito القابلة للتدقيق.

يغطي مراحل البيتا (N0)، والانطلاق العام (N1) والتوسع (N2)
enumerees dans `roadmap.md` en reliant chaque Workflow aux preuves requises،
لوحات العدادات وأصوات التصعيد.

## 2. الأدوار وقائمة الاتصال| الدور | مبادئ المسؤولية | المصنوعات اليدوية ومبادئ القياس عن بعد | إسكاليد |
|------|--------------------------------------------|----|---------|
| مجلس الحكم | قم بتحرير وتصديق المخططات والسياسات اللاحقة وأحكام التقاضي وتناوب المشرفين. | `docs/source/sns/governance_addenda/`، `artifacts/sns/governance/*`، نشرات مجلس الأسهم عبر `sns governance charter submit`. | رئيس المجلس + تابع لجدول الحوكمة. |
| مجلس الأوصياء | Emet des gels soft/hard، canons d'urgence، et revues 72 h. | تذاكر الوصي emis الاسمية `sns governance freeze`، بيانات التجاوز المرسلة من `artifacts/sns/guardian/*`. | حارس التناوب عند الطلب (<= 15 دقيقة ACK). |
| مضيفو اللاحقة | إدارة ملفات المسجل والملفات المخزنة ومستويات السعر وعميل الاتصال؛ استطلاع ليه المطابقات. | سياسات المضيف في `SuffixPolicyV1`، وأوراق الجائزة المرجعية، وإقرارات المضيف، بالإضافة إلى مجموعة من المذكرات التنظيمية. | قائد البرنامج + لاحقة PagerDuty. |
| عمليات التسجيل والتجميع | قم بتشغيل نقاط النهاية `/v1/sns/*`، ومطابقة المدفوعات، وإصدار القياس عن بعد، والحفاظ على لقطات CLI. | مسجل واجهة برمجة التطبيقات ([`registrar-api.md`](./registrar-api.md))، المقاييس `sns_registrar_status_total`، إمكانية أرشفة الدفع من خلال `artifacts/sns/payments/*`. | المدير المناوب للمسجل وخزينة الاتصال. || مشغلي الحل والبوابة | صيانة SoraDNS وGAR ومحاذاة حالة البوابة مع أحداث المسجل؛ نشر مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)، `dashboards/alerts/soradns_transparency_rules.yml`. | محلل SRE عند الطلب + بوابة عمليات الجسر. |
| الخزينة والتمويل | قم بتطبيق إعادة التقسيم 70/30، ومنحوتات الإحالة، ومستودعات الأوراق المالية/الخزانة، وشهادات SLA. | بيانات تراكم الإيرادات، تصدير الشريط/الخزانة، ملاحق مؤشرات الأداء الرئيسية الثلاثة إلى `docs/source/sns/regulatory/`. | تمويل كونترولور + الالتزام المسؤول. |
| الاتصال المطابق والتنظيمي | تتناسب مع الالتزامات العالمية (الاتحاد الأوروبي DSA، وما إلى ذلك)، وتتوافق مع jour les covenants KPI et depose des divulgations. | المذكرات التنظيمية في `docs/source/sns/regulatory/`، والمجموعات المرجعية، والمدخلات `ops/drill-log.md` من أجل التدريبات على سطح الطاولة. | قيادة برنامج المطابقة. |
| الدعم / SRE عند الطلب | معالجة الحوادث (الاصطدامات، استخلاص التجزئة، لوحات الحل)، وتنسيق عميل الاتصال، وامتلاك دفاتر التشغيل. | نماذج الأحداث، `ops/drill-log.md`، إجراءات العمل في المشهد، النسخ Slack/war-room archives sous `incident/`. | التناوب عند الطلب SNS + إدارة SRE. |

## 3. المصنوعات اليدوية ومصادر البيانات| قطعة أثرية | التمركز | موضوعي |
|----------|------------|---------|
| الرسم البياني + الإضافات KPI | `docs/source/sns/governance_addenda/` | تم التوقيع على المخططات مع التحكم في الإصدار، واتفاقيات مؤشرات الأداء الرئيسية، وقرارات الحوكمة المرجعية من خلال أصوات CLI. |
| مخطط التسجيل | [`registry-schema.md`](./registry-schema.md) | الهياكل Norito canoniques (`NameRecordV1`، `SuffixPolicyV1`، `RevenueAccrualEventV1`). |
| عقد المسجل | [`registrar-api.md`](./registrar-api.md) | الحمولات النافعة REST/gRPC، تقيس `sns_registrar_status_total` وتراقب خطافات الإدارة. |
| دليل عناوين تجربة المستخدم | [`address-display-guidelines.md`](./address-display-guidelines.md) | يتم إنتاج Rendus canoniques IH58 (المفضل) والضغطات (الخيار الثاني) بواسطة المحافظ/المستكشفين. |
| مستندات SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | يتم تحديد الاشتقاق من قبل المضيفين وسير العمل من خلال تخصيص الشفافية وقواعد التنبيه. |
| المذكرات التنظيمية | `docs/source/sns/regulatory/` | Notes d'accueil par Judiction (على سبيل المثال الاتحاد الأوروبي DSA)، إقرارات الإقرارات، المرفقات النموذجية. |
| مجلة الحفر | `ops/drill-log.md` | تتطلب Journal des rehearsals Chaos et IR طلعات جوية مسبقة. |
| مخزون التحف | `artifacts/sns/` | إجراءات الدفع، وحارس التذاكر، ومحلل الفروق، وتصدير مؤشرات الأداء الرئيسية، وإخراج علامة CLI المنتجة على أساس `sns governance ...`. |جميع إجراءات الحوكمة يجب أن تشير إلى حد ما إلى شيء ما
هذا يعني أن المدققين يمكنهم إعادة بناء الأثر
القرار خلال 24 ساعة

## 4. قواعد اللعبة في دورة الحياة

### 4.1 حركات التفويض والإشراف| إيتاب | مالك | CLI / Preuve | ملاحظات |
|-------|------------|--------------|-------|
| قم بإعادة صياغة الإضافة ودلتا مؤشرات الأداء الرئيسية | مقرر المجلس + المضيف الرئيسي | قالب تخفيض السعر الأسهم `docs/source/sns/governance_addenda/YY/` | قم بتضمين معرفات ميثاق KPI وخطافات القياس عن بعد وشروط التنشيط. |
| Sometre la proposition | رئيس المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (المنتج `CharterMotionV1`) | أصدر La CLI بيانًا Norito مخزنًا في `artifacts/sns/governance/<id>/charter_motion.json`. |
| التصويت والاعتراف الوصي | كونسيل + أولياء الأمور | `sns governance ballot cast --proposal <id>` و`sns governance guardian-ack --proposal <id>` | قم بضم الدقائق والأجزاء السابقة للنصاب القانوني. |
| وكيل القبول | مشرف البرنامج | `sns governance steward-ack --proposal <id> --signature <file>` | يتطلب التغيير المسبق لسياسة اللاحقة؛ قم بتسجيل المغلف Sous `artifacts/sns/governance/<id>/steward_ack.json`. |
| التنشيط | عمليات التسجيل | Mettre a jour `SuffixPolicyV1`, rafraichir lesذاكر التخزين المؤقت للمسجل ونشر ملاحظة في `status.md`. | سجل التنشيط للطابع الزمني في `sns_governance_activation_total`. |
| مجلة التدقيق | مطابق | أضف إدخالاً إلى `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ومجلة الحفر مع تأثير سطح الطاولة. | قم بتضمين مراجع ولوحات القياس عن بعد والاختلافات السياسية. |

### 4.2 موافقات التسجيل والتسجيل والسعر1. **الاختبار المبدئي:** يقوم المسجل بالاستفسار `SuffixPolicyV1` لتأكيد المستوى
   من الجائزة، والشروط المتاحة، ونوافذ النعمة/الفداء. غاردر ليه
   تتم مزامنة ملفات الجائزة مع لوحة المستويات 3/4/5/6-9/10+ (المستوى
   القاعدة + معاملات اللاحقة) الموثقة في خريطة الطريق.
2. **Encheres مختوم العطاء:** Pour les Pools premium، المنفذ le دورة 72 ساعة التزام /
   كشف على مدار 24 ساعة عبر `sns governance auction commit` / `... reveal`. ناشر القائمة
   الالتزامات (التجزئة الفريدة) sous `artifacts/sns/auctions/<name>/commit.json`
   حتى يتمكن المدققون من التحقق من المكان.
3. **التحقق من الدفع:** المسجلون صالحون `PaymentProofV1` الاسمية
   إعادة تقسيم الخزانة (70% خزانة / 30% مضيف مع اقتطاع
   الإحالة <=10%). Stocker لو JSON Norito سو `artifacts/sns/payments/<tx>.json`
   et le lier dans la reponse du المسجل (`RevenueAccrualEventV1`).
4. **خطاف الإدارة:** المرفق `GovernanceHookV1` pour les noms premium/guarded
   بالإشارة إلى معرفات اقتراح المشورة وتوقيعات المضيف.
   الخطافات تتقلص `sns_err_governance_missing`.
5. **محلل التنشيط والمزامنة:** مرة واحدة Torii ستحدث حدث التسجيل،
   قم بإلغاء تحديد جزء الشفافية من المحلل لتأكيد الجديد
   حالة GAR/المنطقة الأكثر انتشارًا (الصورة 4.5).
6. **عميل الإفصاح:** Mettre a jour le ledger oriente client (المحفظة/المستكشف)عبر أجزاء التركيبات في [`address-display-guidelines.md`](./address-display-guidelines.md)،
   En s'assurant que les rendus IH58 et المضغوطات تتوافق مع أدلة النسخ/QR.

### 4.3 خزينة التجديد والتفتيت والمصالحة- **سير العمل للتجديد:** يزين المسجلون نافذة نعمة
  30 يومًا + نافذة الاسترداد لمدة 60 يومًا محددة في `SuffixPolicyV1`.
  أبريل 60 يوم، تسلسل التجديد الهولندي (7 أيام، 10 مرات إضافية)
  يتم إلغاء غلق الخبز بنسبة 15%/اليوم تلقائيًا عبر `sns governance reopen`.
- **Repartition des revenus:** Chaque renouvellement ou Transfert cree un
  `RevenueAccrualEventV1`. يتم تنفيذ صادرات الخزانة (CSV/Parquet).
  التوفيق بين هذه الأحداث اليومية ؛ انضم إلى Preuves أ
  `artifacts/sns/treasury/<date>.json`.
- **اقتطاعات الإحالة:** نسب خيارات الإحالة هي فريدة من نوعها
  باللاحقة والإضافة `referral_share` إلى المضيف السياسي. المسجلين
  قم بتحرير الانقسام النهائي وتخزين بيانات الإحالة في الجزء السفلي من الصفحة
  preuve de paiement.
- **إيقاع إعداد التقارير:** نشر مرفقات مؤشرات الأداء الرئيسية بشكل منتظم
  (التسجيلات، التجديدات، متوسط العائد لكل مستخدم، الاستفادة من الدعاوى/السندات)
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. لوحات المعلومات مهمة
  اضغط على الميمات الجداول المصدرة لتتمكن من الكتابة Grafana
  مراسل aux preuves du دفتر الأستاذ.
- **مراجعة مؤشرات الأداء الرئيسية الشهرية:** نقطة تفتيش دو بريمير ماردي أسوسيه لو ليد فاينانس،
  برنامج مضيف الخدمة وبرنامج PM. افتح [لوحة معلومات SNS KPI](./kpi-dashboard.md)
  (تضمين البوابة `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`)،تصدير جداول الإنتاجية + إعادة تسجيل البيانات وإرسال الدلتا
  في الملحق، قم بضم القطع الأثرية إلى مذكرة. قم بفك الحادث إذا حدث ذلك
  اكتشاف مخالفات SLA (نوافذ التجميد > 72 ساعة، صور أخطاء
  المسجل، اشتقاق ARPU).

### 4.4 المواد الهلامية والقضايا والطعون| المرحلة | مالك | العمل والسبق | جيش تحرير السودان |
|-------|-------------|------------------|-----|
| الطلب على الجل الناعم | ستيوارد / دعم | قم بإيداع تذكرة `SNS-DF-<id>` مع إجراءات الدفع ومرجع سند التقاضي وحدد التأثير (التأثيرات). | <=4 ساعات بعد الدخول. |
| حارس التذاكر | حارس المجلس | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` المنتج `GuardianFreezeTicketV1`. Stocker le JSON du Ticket sous `artifacts/sns/guardian/<id>.json`. | <=30 دقيقة ACK، <=2 ساعة تنفيذ. |
| تصديق المجلس | مجلس الحكم | الموافقة على المواد الهلامية أو رفضها، وتوثيق القرار الخاص بالامتياز مقابل تذكرة الوصي وملخص سند التقاضي. | قم بإجراء جلسة مشورة أو التصويت بشكل غير متزامن. |
| لوحة التحكيم | مطابق + ستيوارد | قم بتكوين لجنة مكونة من 7 محلفين (خارطة طريق محددة) باستخدام النشرات عبر `sns governance dispute ballot`. انضم إلى قائمة التصويت المجهولة المصدر في حزمة الحوادث. | الحكم <= 7 أيام بعد مستودع السندات. |
| نداء | الجارديان + المجلس | تضاعف الطلبات السند وتتكرر عملية المحلفين؛ قم بتسجيل البيان Norito `DisputeAppealV1` وقم بالإشارة إلى التذكرة الأولى. | <=10 يوم. |
| ديجل والعلاج | المسجل + محلل العمليات | قم بتنفيذ `sns governance unfreeze --selector <IH58> --ticket <id>`، ومتابعة حالة المسجل، ونشر اختلافات GAR/resolver. | فورًا بعد صدور الحكم. |Les canons d'urgence (الجل المخفف من قبل الوصي <= 72 ساعة) يتبع تدفق الميم
ولكن من الضروري إجراء مراجعة بأثر رجعي للمشورة ومذكرة شفافة للغاية
`docs/source/sns/regulatory/`.

### 4.5 محلل الانتشار وبوابته

1. **خطاف الحدث:** كل حدث يتم تسجيله مقابل تدفق الأحداث
   محلل (`tools/soradns-resolver` SSE). يتم حل العمليات أيضًا
   قم بتسجيل الاختلافات عبر أداة الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **صمم قالب GAR يوميًا:** يجب أن تعمل البوابات على القوالب يوميًا
   مراجع GAR على أساس `canonical_gateway_suffix()` وأعد تسجيل القائمة
   `host_pattern`. Stocker les diffs dans `artifacts/sns/gar/<date>.patch`.
3. **نشر ملف المنطقة:** استخدم ضغط ملف المنطقة الموجود في
   `roadmap.md` (الاسم، ttl، cid، الإثبات) والضغط على Torii/SoraFS. أرشيفي
   لو JSON Norito سو `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **التحقق من الشفافية:** المنفذ `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   للتأكد من أن التنبيهات تبقى في مكانها الصحيح. انضم إلى النص الذي قمت بإخراجه
   Prometheus au تقرير الشفافية hebdomadaire.
5. **بوابة التدقيق:** Enregistrer des echantillons d'en-tetes `Sora-*` (ذاكرة التخزين المؤقت للسياسة،
   CSP، ملخص GAR) والانضمام إلى مجلة الحوكمة من أجل تحقيق ذلك
   المشغلون قادرون على إثبات أن البوابة تخدم الاسم الجديد مع
   garde-fous prevus.

## 5. القياس عن بعد وإعداد التقارير| إشارة | المصدر | الوصف / العمل |
|--------|--------|---------------------|
| `sns_registrar_status_total{result,suffix}` | مسجل الإدارات Torii | قياس النجاح/الخطأ في التسجيلات والتجديدات والمواد الهلامية والتحويلات؛ تنبيه عند زيادة `result="error"` حسب اللاحقة. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | ميتريكس Torii | SLO زمن الاستجابة لواجهة برمجة تطبيقات المعالجات؛ تم إصدار لوحات المعلومات `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` و`soradns_bundle_cid_drift_total` | خياط محلل الشفافية | اكتشاف التجاوزات المسبقة أو مشتقات GAR؛ تم تحديده بعناية في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | حوكمة CLI | قم بحساب زيادة كل تفعيل للمخطط/الإضافة; استخدم للتوفيق بين قرارات المجلس والإضافات المنشورة. |
| مقياس `guardian_freeze_active` | ولي الأمر CLI | تناسب نوافذ الجل الناعمة/الصلبة؛ صفحة SRE si la valeur Reste `1` au-dela du SLA تعلن. |
| لوحات المعلومات الخاصة بمرفقات KPI | المالية / مستندات | يتم نشر مجموعات البيانات الشهرية مع المذكرات التنظيمية؛ يتم دمج البوابة عبر [لوحة معلومات SNS KPI](./kpi-dashboard.md) حتى يتمكن المشرفون والمنظمون من الوصول إلى الصورة Grafana. |

## 6. متطلبات الأدلة والتدقيق| العمل | دليل أرشيفي | المخزون |
|--------|---------------------|----------|
| تغيير الميثاق / السياسة | بيان Norito التوقيع، نص CLI، فرق KPI، مضيف الإقرار. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| التسجيل / التجديد | الحمولة `RegisterNameRequestV1`، `RevenueAccrualEventV1`، مسبقة الدفع. | `artifacts/sns/payments/<tx>.json`، واجهة برمجة تطبيقات سجلات المسجل. |
| انشر | إظهار الالتزام/الكشف، تفاصيل التفاصيل، جدول حساب الجاجنانت. | `artifacts/sns/auctions/<name>/`. |
| جل / ديجيل | حارس التذكرة، تجزئة التصويت للمشورة، عنوان URL لسجل الحادث، نموذج اتصال العميل. | `artifacts/sns/guardian/<ticket>/`، `incident/<date>-sns-*.md`. |
| محلل الانتشار | ملف منطقة الاختلاف/GAR، ملحق JSONL للخياط، لقطة Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| كمية تنظيمية | المذكرة الرئيسية، وتعقب المواعيد النهائية، ومشرف الإقرار، واستئناف تغييرات مؤشرات الأداء الرئيسية. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة التحقق من بوابة المرحلة| المرحلة | معايير الطلعة | حزمة الأدلة |
|-------|--------------------|------------------|
| N0 - بيتا فيرمي | مخطط التسجيل SN-1/SN-2، دليل مسجل CLI، حارس الحفر مكتمل. | حركة الرسم البياني + مضيف ACK، سجلات التشغيل الجاف للمسجل، تقرير حل الشفافية، الدخول إلى `ops/drill-log.md`. |
| N1 - الرمح العام | المزيد + مستويات إصلاح الأسعار النشطة لـ `.sora`/`.nexus`، والخدمة الذاتية للمسجل، ومحلل المزامنة التلقائية، ولوحات المعلومات. | فرق السعر، نتائج CI للمسجل، الدفعة الملحقة/مؤشر الأداء الرئيسي، نوع تخصيص الشفافية، ملاحظات حول حادثة التدريب. |
| N2 - التوسع | `.dao`، موزع واجهات برمجة التطبيقات، وبوابة التقاضي، ومشرف بطاقات الأداء، وتحليلات لوحات المعلومات. | يلتقط شاشة الباب، ويقيس SLA للتقاضي، ويصدر بطاقات الأداء، وميثاق الحوكمة في اليوم مع الموزعين السياسيين. |

Lesطلعات المرحلة المطلوبة لتدريبات الطاولة المسجلة (سجلات الباركور
مسار سعيد، هلام، محلل Panne) مع المصنوعات اليدوية المرفقة في `ops/drill-log.md`.

## 8. الاستجابة للحوادث والتصعيد| ديكلينشر | شديد | الملكية فورية | واجبات الإجراءات |
|-------------|----------|----------------------|----------------------|
| اشتقاق المحلل/GAR أو preuves perimees | سيف 1 | محلل SRE + ولي الأمر | استدعاء محلل عند الاتصال، والتقاط الترتيب المخصص، وتحديد ما إذا كانت الأسماء تؤثر على ما هو مطلوب وما إلى ذلك، ثم نشر حالة لمدة 30 دقيقة. |
| مسجل Panne، فحص التخصيب، أو أخطاء تعميمات API | سيف 1 | المدير المناوب للمسجل | قم بإيقاف الإدخالات الجديدة، بعد استخدام CLI يدويًا، ومشرفي الإخطار/الخزانة، وضم السجلات Torii إلى مستند الحادث. |
| دعوى قضائية حول اسم واحد أو عدم تطابق الدفع أو عميل التصعيد | سيف 2 | ستيوارد + دعم الرصاص | اجمع مدفوعات الدفع، وحدد ما إذا كان الجل الناعم ضروريًا، والرد على الطلب في اتفاقية مستوى الخدمة (SLA)، وإرسال النتائج إلى متتبع التقاضي. |
| قانون تدقيق المطابقة | سيف 2 | الاتصال المطابق | قم بإعادة وضع خطة إصلاح، وإيداع مذكرة Sous `docs/source/sns/regulatory/`، وتخطيط جلسة مشورة للمتابعة. |
| تدريب أو بروفة | سيف 3 | برنامج PM | قم بتنفيذ السيناريو النصي من `ops/drill-log.md`، وأرشفة العناصر، وتصحيح الفجوات كأجزاء من خريطة الطريق. |يجب إنشاء جميع الأحداث `incident/YYYY-MM-DD-sns-<slug>.md` مع
جداول الملكية وسجلات الأوامر والمراجع المسبقة
produites tout au long de ce playbook.

## 9. المراجع

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (الأقسام SNS، DG، ADDR)

قم بحفظ هذا الدليل في كل يوم من نص المخططات وأسطح CLI
تتغير عقود القياس عن بعد؛ مداخل خريطة الطريق التي تشير إليها
`docs/source/sns/governance_playbook.md` doivent toujours matcher a la
المراجعة الأخيرة.