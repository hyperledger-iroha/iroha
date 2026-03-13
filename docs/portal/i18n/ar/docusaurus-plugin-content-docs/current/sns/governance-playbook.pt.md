---
lang: ar
direction: rtl
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sns/governance_playbook.md` وagora تخدم كما أ
نسخة كانونيكا تفعل البوابة. الملف الدائم PRs de traducao.
:::

# قواعد اللعبة التي تحكم خدمة Sora Name (SN-6)

**الحالة:** Redigido 24-03-2026 - مرجع حي لـ prontidao SN-1/SN-6  
**روابط خريطة الطريق:** SN-6 "الامتثال وحل النزاعات"، SN-7 "Resolver & Gateway Sync"، politica de endereco ADDR-1/ADDR-5  
**المتطلبات المسبقة:** تسجيل الدخول [`registry-schema.md`](./registry-schema.md)، عقد واجهة برمجة التطبيقات (API) التسجيل [`registrar-api.md`](./registrar-api.md)، دليل تجربة المستخدم [`address-display-guidelines.md`](./address-display-guidelines.md)، وإعادة ضبط إعدادات الحساب في [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

يتم وصف دليل التشغيل هذا كمجموعة إدارية لخدمة Sora Name Service (SNS)
Adotam Cartas، والموافقة على السجلات، وتصعيد النزاعات وإثبات حالات
محلل وبوابة دائمة متزامنة. إنه يشمل متطلبات خريطة الطريق
ما هو CLI `sns governance ...` والبيانات Norito وأعمال السمع
مشاركة مرجع واحد فولتادا مع المشغل قبل N1 (lancamento
العامة).

## 1. الإسكوبو والجمهور

O documento se Destina a:- أعضاء مجلس الإدارة الذين يصوتون على البطاقات والسياسات اللاحقة
  نتائج النزاع.
- أعضاء مجلس الحراس الذين يصدرون تجمعات الطوارئ e
  مراجعة العكسية.
- مشرفو اللاحقة التي تعمل على أولاد المسجل، يوافقون على Leiloes e Gerenciam
  تقسيم الإيصالات.
- مشغلو محلل/بوابة الاستجابة لنشر SoraDNS، وتحديثهم
  GAR وحواجز الحماية عن بعد.
- معدات المطابقة والتخزين والدعم التي يجب أن تثبتها جميعًا
  Acao de Goveranca deixou artefatos Norito Auditaveis.

يتم طحنها كخطوات تجريبية (N0) وتوسعة عامة (N1) وتوسيع (N2)
القوائم في `roadmap.md`، كل تدفق العمل كأدلة
الضروريات ولوحات المعلومات وآليات التصعيد.

## 2. بابيس وخريطة الاتصال| بابيل | المسؤوليات المبدئية | Artefatos والقياس عن بعد المبدئي | اسكالاكاو |
|-------|--------------------------------------------|----------|---|
| مجلس الإدارة | قم بإعادة النظر والتصديق على البطاقات والسياسات اللاحقة وحقائق النزاع وتقارير المضيف. | `docs/source/sns/governance_addenda/`، `artifacts/sns/governance/*`، تم التصويت للتوصيات عبر `sns governance charter submit`. | رئيس المجلس + مدير جدول أعمال الحكومة. |
| نصيحة الأوصياء | قم بإصدار تجمعات ناعمة/صلبة وشرائع الطوارئ ومراجعات لمدة 72 ساعة. | صادر حارس التذاكر حسب `sns governance freeze`، بيانات تجاوز المسجلين في `artifacts/sns/guardian/*`. | حارس Rotacao عند الطلب (<= 15 دقيقة ACK). |
| مضيفو اللاحقة | عمليات التسجيل والتسجيل ومستويات الأسعار والتواصل مع العملاء؛ إعادة المطابقة. | سياسة المضيف في `SuffixPolicyV1`، وفقًا للمراجع السابقة، وإقرارات المضيف المخزّنة جنبًا إلى جنب مع المذكرات التنظيمية. | يقوم القائد ببرمجة مضيف + PagerDuty لاحقاً. |
| عمليات التسجيل والكوبرانكا | تعمل نقاط نهاية Operam `/v2/sns/*` على تسوية الدفعات وإصدار القياس عن بعد وحفظ لقطات CLI. | واجهة برمجة التطبيقات الخاصة بالمسجل ([`registrar-api.md`](./registrar-api.md)) والمقاييس `sns_registrar_status_total` واختبارات الدفع الأرشيفية في `artifacts/sns/payments/*`. | المدير المناوب يقوم بالتسجيل والاتصال بالمخزن. || مشغلو البوابة الإلكترونية | Mantem SoraDNS وGAR وحالة البوابة أيضًا مع الأحداث المسجلة؛ إرسال مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)، `dashboards/alerts/soradns_transparency_rules.yml`. | محلل SRE عند الطلب + بوابة العمليات. |
| Tesouraria e financas | تطبيق تقسيم الإيصالات 70/30، واستقطاعات الإحالة، والسجلات المالية/المحفظة، وشهادات SLA. | بيانات تراكم الإيصالات، تصدير الشريط/المربع، ملحقات مؤشرات الأداء الرئيسية الثلاثة في `docs/source/sns/regulatory/`. | المراقب المالي + الرسمي للمطابقة. |
| الاتصال بالمطابقة والتنظيم | مرافقة obrigacoes globais (الاتحاد الأوروبي DSA، وما إلى ذلك)، وتحديث مواثيق مؤشرات الأداء الرئيسية وسجلات الإفصاح. | المذكرات التنظيمية في `docs/source/sns/regulatory/`، الأسطح المرجعية، مدخلات `ops/drill-log.md` لاستخدام سطح الطاولة. | قائد برنامج المطابقة. |
| دعم / SRE عند الطلب | Lida com events (colisoes، Drift de cobranca، quedas desolver)، تنسيق الرسائل إلى العملاء وتقديم دفاتر التشغيل. | قوالب الأحداث، `ops/drill-log.md`، الأدلة المعملية، نسخ أرشيفات Slack/war-room في `incident/`. | Rotacao عند الطلب SNS + gestao SRE. |

## 3. Artefatos canonicos وخطوط البيانات| ارتيفاتو | لوكاليزاكاو | اقتراح |
|----------|-----------|----------|
| كارتا + إضافات KPI | `docs/source/sns/governance_addenda/` | تم دمج البطاقات مع التحكم في العكس، ومواثيق مؤشرات الأداء الرئيسية، وقرارات الإدارة المرجعية من خلال تصويتات CLI. |
| تسجيل الدخول | [`registry-schema.md`](./registry-schema.md) | Estruturas Norito canonicas (`NameRecordV1`، `SuffixPolicyV1`، `RevenueAccrualEventV1`). |
| عقد التسجيل | [`registrar-api.md`](./registrar-api.md) | الحمولات الصافية REST/gRPC والمقاييس `sns_registrar_status_total` وربط توقعات الإدارة. |
| دليل UX de enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | يتم تقديم الكنسيات I105 (المفضلة) والمركبات (الثانية الأفضل من نوعها) المخصصة للمحافظ/المستكشفين. |
| مستندات SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)، [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | اشتقاق حتمية المضيفين، وتدفق تخصيص الشفافية، وأنظمة التنبيه. |
| مذكرات تنظيمية | `docs/source/sns/regulatory/` | Notas de entrada por jurisdicao (مثل EU DSA)، وإقرارات المضيف، ومرفقات القالب. |
| سجل الحفر | `ops/drill-log.md` | سجل المطالبات المطلوبة من قبل IR و IR قبل الأقوال. |
| تخزين التحف | `artifacts/sns/` | اختبار الدفع، حارس التذاكر، فرق الحل، تصدير KPI و CLI المنتج من خلال `sns governance ...`. |يجب أن تشير جميع أدوات الإدارة إلى أقل ما يمكن من صنعة على اللوحة
هناك حاجة إلى أن يقوم المدققون بإعادة بناء قرارهم خلال 24 ساعة.

## 4. قواعد اللعبة الخاصة بدراجة الحياة

### 4.1 مذكرة ومضيف

| ايتابا | الرد | CLI / إيفيدنسيا | نوتاس |
|-------|------------|-----------------|-------|
| قم بإعادة تسجيل الإضافات ودلتا مؤشرات الأداء الرئيسية | Relator do conselho + ليدر ستيوارد | تم تصميم القالب Markdown في `docs/source/sns/governance_addenda/YY/` | بما في ذلك معرفات مؤشرات الأداء الرئيسية وخطافات القياس عن بعد وشروط التنشيط. |
| Enviar proposta | الرئيس دو كونسيلهو | `sns governance charter submit --input SN-CH-YYYY-NN.md` (المنتج `CharterMotionV1`) | يصدر بيان CLI Norito من `artifacts/sns/governance/<id>/charter_motion.json`. |
| التصويت والاعتراف الوصي | كونسيلهو + الأوصياء | `sns governance ballot cast --proposal <id>` و`sns governance guardian-ack --proposal <id>` | قم بإضافة هذه التصنيفات واختبار النصاب القانوني. |
| مضيفة أسيتاكاو | برنامج ستيوارد | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatorio antes de mudar politicas de sufixo؛ مظروف المسجل em `artifacts/sns/governance/<id>/steward_ack.json`. |
| أتيفاكاو | العمليات تقوم بالتسجيل | تم تحديث `SuffixPolicyV1`، وتم تحديث ذاكرة التخزين المؤقت للمسجل، ونشر الملاحظة في `status.md`. | الطابع الزمني للنشاط المسجل في `sns_governance_activation_total`. |
| سجل الاستماع | كونفورميداد | قم بإضافة الإدخال إلى `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ولا يوجد سجل حفر على سطح الطاولة. | قم بتضمين المراجع ولوحات القياس عن بعد والاختلافات السياسية. |

### 4.2 إجراءات التسجيل والتسجيل والتسجيل1. **الاختبار المبدئي:** استشر المسجل `SuffixPolicyV1` لتأكيد مستوى
   Preco, termos disponiveis e janelas de graca/redencao. Mantenha folhas de
   يتم المزامنة مسبقًا مع لوحة المستوى 3/4/5/6-9/10+ (مستوى القاعدة +
   coeficiantes de sufixo) توثيق لا يوجد خريطة طريق.
2. ** عرض ليلي مختوم: ** قسط حمامات السباحة، تنفيذ o التزام لمدة 72 ساعة /
   كشف على مدار 24 ساعة عبر `sns governance auction commit` / `... reveal`. عام أ
   قائمة الالتزامات (apenas hash) في `artifacts/sns/auctions/<name>/commit.json`
   حتى يتمكن المدققون من التحقق من التباين.
3. **التحقق من الدفع:** صحيح المسجلين `PaymentProofV1` مقابل
   قسم tesouraria (70% tesouraria / 30% مضيف مع اقتطاع الإحالة <= 10%).
   أرمازين أو JSON Norito في `artifacts/sns/payments/<tx>.json` وvincule-o na
   رد المسجل (`RevenueAccrualEventV1`).
4. **خطاف الإدارة:** Anexe `GovernanceHookV1` para nomes premium/guarded com
   قم بالرجوع إلى معرفات الاقتراحات والمشورة ومساعدي المضيفين. خطافات
   النتيجة النهائية هي `sns_err_governance_missing`.
5. **تنشيط + حل المزامنة:** حتى يقوم Torii بإصدار حدث التسجيل،
   قم بإجراء أداة حل الشفافية لتأكيد الحالة الجديدة
   GAR/منطقة حد ذاتها بروباغو (إصدار 4.5).
6. **الكشف عن العميل:** تحديث دفتر الأستاذ إلى العميل (المحفظة/المستكشف)
   عبر تركيبات نظام التشغيل المتوافقة مع [`address-display-guidelines.md`](./address-display-guidelines.md)،ضمان عرض I105 ودمج اتجاهات النسخ/QR.

### 4.3 التجديدات والكوبرانكا وإصلاح الأنسجة- **تدفق التجديد:** المسجلون يطبقون تاريخ الميلاد لمدة 30 يومًا + تاريخ
  يحتوي على 60 يومًا محددًا من `SuffixPolicyV1`. Apos 60 دياس، أ
  تسلسل إعادة البناء الهولندي (7 أيام، الأصناف 10x decaindo 15%/dia) e
  يتم التنشيط تلقائيًا عبر `sns governance reopen`.
- **قسم الاستقبال:** كل تجديد أو تحويل يتم إنشاؤه
  `RevenueAccrualEventV1`. يجب التوفيق بين صادرات tesouraria (CSV/Parquet).
  هذه الأحداث يوميا؛ anexe يبرهن على `artifacts/sns/treasury/<date>.json`.
- **اقتطاعات الإحالة:** النسبة المئوية للإحالة الاختيارية من خلال
  لاحقة لإضافة `referral_share` إلى سياسة المضيف. المسجلون يصدرون أ
  القسمة النهائية وتخزين بيانات الإحالة بعد إثبات الدفع.
- **نطاق العلاقات:** المالية العامة anexos KPI mensais (السجلات،
  التجديدات، ARPU، استخدام المنازعات/السندات) في `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  ستجعل لوحات المعلومات تضغط على اللوحات المصدرة حتى تتمكن من رؤية أرقامها
  Grafana باتام كوم كما هو الحال في دفتر الأستاذ.
- **مراجعة مؤشرات الأداء الرئيسية:** نقطة التفتيش الأولى في نهاية المطاف أو قائدها
  المالية، ستيوارد دي بلانتاو، و PM تفعل البرنامج. العبرة o [لوحة معلومات SNS KPI](./kpi-dashboard.md)
  (قم بتضمين البوابة الإلكترونية `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`)،
  تصدير كجداول الإنتاجية + إيصال المسجل، سجل دلتا رقم
  قم بإرفاق العناصر الفنية بمذكرة. سيتم العثور على حادثة في حالة المراجعةquebras de SLA (تجميد التجميد > 72 ساعة، صور خطأ في التسجيل، انجراف ARPU).

### 4.4 التجمعات والمنازعات والمكالمات

| فاس | الرد | Acao e evidencia | جيش تحرير السودان |
|------|----------------------------|---|-----|
| بيديدو دي فريز سوفت | ستيوارد / دعم | فتح التذكرة `SNS-DF-<id>` مع اختبار الدفع ومراجعة سندات النزاع واختيار (المستندات). | <=4 ساعات للداخل. |
| حارس التذاكر | الوصي كونسيلهو | `sns governance freeze --selector <I105> --reason <text> --until <ts>` المنتج `GuardianFreezeTicketV1`. أرمازين أو JSON قم بعمل تذكرة على `artifacts/sns/guardian/<id>.json`. | <=30 دقيقة ACK، <=2 ساعة تنفيذ. |
| Ratificacao do conselho | مجلس الإدارة | الموافقة على الاتفاقات أو قبولها، وتوثيق القرار باستخدام الرابط الخاص بوصي التذكرة وملخص رابطة النزاع. | قم بالجلسة التالية للمشورة أو التصويت. |
| لوحة المراجحة | كونفورميداد + ستيوارد | Convocar Paintel de 7 jurados (خريطة الطريق المتوافقة) مع cedulas hasheadas عبر `sns governance dispute ballot`. قم بإضافة إيصالات صوتية مجهولة المصدر إلى حزمة الحادث. | Veredito <= 7 أيام من أجل إيداع السندات. |
| أبيلاكاو | الجارديان + المستشار | يتم إجراء الالتماسات من خلال السندات وتكرار إجراءات المحاكمة؛ بيان المسجل Norito `DisputeAppealV1` والتذكرة المرجعية الأولية. | <=10 دياس. |
| إزالة الخلل والمعالجة | المسجل + عمليات الحل | قم بتنفيذ `sns governance unfreeze --selector <I105> --ticket <id>`، وقم بتهيئة حالة المسجل ونشر اختلافات GAR/resolver. | على الفور أو التحقق. |قوانين الطوارئ (التجمعات المتصاعدة للوصي <= 72 ساعة) تتبع أو في نفس الوقت
التدفق، ولكن يتطلب مراجعة رجعية للمشورة وملاحظة شفافة
`docs/source/sns/regulatory/`.

### 4.5 نشر بوابة الحل

1. **ربط الحدث:** يصدر كل حدث تسجيل لدفق الأحداث
   محلل (`tools/soradns-resolver` SSE). يتم إدخال عملية الحل
   يختلف التسجيل عبر خياط الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **تحديث قالب GAR:** تعمل البوابات على تحديث قوالب GAR
   المراجع لـ `canonical_gateway_suffix()` وإعادة إنشاء القائمة
   `host_pattern`. أرمازين يختلف عن `artifacts/sns/gar/<date>.patch`.
3. **نشر ملف المنطقة:** استخدم هيكل ملف المنطقة الموضح في `roadmap.md`
   (الاسم، ttl، cid، الإثبات) والحسد لـ Torii/SoraFS. أرشيف o JSON Norito em
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **التحقق من الشفافية:** تنفيذ `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   لضمان أن التنبيهات خضراء تمامًا. قم بإرفاق نص بالسيدة
   Prometheus مرتبط بمؤشر الشفافية.
5. **مراجعة البوابة:** تسجيل جميع الرؤوس `Sora-*` (سياسة
   ذاكرة التخزين المؤقت، وCSP، وDigest GAR) وملحق كسجل لإدارة المشغلين
   يمكنك إثبات أن البوابة تخدم الاسم الجديد مثل حواجز الحماية المتوقعة.

## 5. القياس عن بعد والعلاقات| سينال | فونتي | وصف / أكاو |
|-------|-------|------------------|
| `sns_registrar_status_total{result,suffix}` | المعالجات تقوم بالتسجيل Torii | مصحح النجاح/الأخطاء للسجلات والتجديدات والجمعيات والتحويلات؛ تنبيه عند `result="error"` يتم تعزيزه باللاحقة. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | متريكاس Torii | SLOs زمن الاستجابة لمعالجات API؛ لوحات معلومات الطعام مبنية على `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` و`soradns_bundle_cid_drift_total` | خياط الشفافية للحل | يكتشف اكتشاف الأشياء القديمة أو الانجراف في GAR؛ حواجز الحماية المحددة في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI دي الحكم | يتم زيادة المبلغ عند الميثاق/الإضافة؛ يتم استخدامه للتوفيق بين قرارات النصيحة والإضافات المنشورة. |
| مقياس `guardian_freeze_active` | ولي الأمر CLI | مرافقة لقطع التجميد الناعمة/الصلبة؛ pagine SRE se o valor ficar `1` alem do SLA declarado. |
| لوحات المعلومات الخاصة بـ KPI | فينانكاس / مستندات | مجموعات البيانات المنشورة شهريًا جنبًا إلى جنب مع المذكرات التنظيمية; يمكنك تشغيل البوابة عبر [لوحة معلومات SNS KPI](./kpi-dashboard.md) حتى يتمكن المضيفون والمنظمون من الوصول إلى تأشيرة Grafana. |

## 6. متطلبات الأدلة والاستماع| أكاو | الأدلة والمحفوظات | أرمازينامينتو |
|------|----------------------|---------------|
| مودانكا دي كارتا / بوليتيكا | تم اغتيال البيان Norito، ونص CLI، وفرق KPI، وإقرار المضيف. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| التسجيل / تجديد | الحمولة `RegisterNameRequestV1`، `RevenueAccrualEventV1`، اختبار الدفع. | `artifacts/sns/payments/<tx>.json`، سجلات واجهة برمجة التطبيقات الخاصة بالمسجل. |
| ليلاو | الالتزام/الكشف عن البيانات، حتى مع التقسيم، وتخطيط حساب الثأر. | `artifacts/sns/auctions/<name>/`. |
| كونغيلار / ديسكونجيلار | حارس التذكرة، وتجزئة التصويت، وعنوان URL لسجل الأحداث، ونموذج الاتصال بالعميل. | `artifacts/sns/guardian/<ticket>/`، `incident/<date>-sns-*.md`. |
| نشر الحل | Diff Zonefile/GAR، trecho JSONL do tailer، snapshot Prometheus. | `artifacts/sns/resolver/<date>/` + علاقات الشفافية. |
| كمية تنظيمية | مذكرة الاستيعاب، وتعقب المواعيد النهائية، وإقرار المضيف، واستئناف تحديثات مؤشرات الأداء الرئيسية. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة التحقق من بوابة المرحلة| فاس | معايير الصيدة | حزمة الأدلة |
|------|--------------------|--------------------|
| N0 - بيتا فيشادا | مفتاح التسجيل SN-1/SN-2، دليل المسجل CLI، حفر الوصي الكامل. | بطاقة الحركة + مضيف ACK، سجلات التشغيل الجاف للمسجل، علاقة الشفافية للمحلل، يتم إدخالها في `ops/drill-log.md`. |
| N1 - Lancamento publico | Leiloes + مستويات الإصلاح المسبق لـ `.sora`/`.nexus`، الخدمة الذاتية للمسجل، المزامنة التلقائية للمحلل، لوحات المعلومات كوبرانكا. | يختلف حجم التكلفة عن نتائج CI للمسجل، وملحق الدفع/مؤشر الأداء الرئيسي، وتخصيص الشفافية، وملاحظات الحوادث. |
| N2 - اكسبانساو | `.dao`، واجهات برمجة تطبيقات الموزعين، بوابة النزاع، بطاقات أداء المضيفين، لوحات المعلومات التحليلية. | لقطات البوابة، ومقاييس SLA للخلاف، وتصدير بطاقات الأداء للمضيف، وبطاقة الإدارة التي تم تحديثها مع سياسات الموزع. |

As sayas de fase exigem يتدرب على تسجيل سطح الطاولة (fluxo feliz deregistro،
تجميد، انقطاع في الحل) مع العناصر المرفقة في `ops/drill-log.md`.

## 8. الرد على الأحداث والتصعيد| جاتيلهو | قطع | دونو فوري | اكويس أوبريجاتورياس |
|---------|----------|--------------|-------------------|
| انجراف محلل/GAR أو اكتشاف الأشياء القديمة | سيف 1 | محلل SRE + ولي الأمر conselho | قم بصفحة المحلل عند الطلب، والتقاط رسالة الخياط، وقرر أن تقوم بجمع الأسماء المشاركة، والحالة العامة كل 30 دقيقة. |
| شيء من المسجل، خطأ كوبرانكا، أو أخطاء API العامة | سيف 1 | المدير المناوب يقوم بالمسجل | قم بقراءة المزيد من التفاصيل، بالإضافة إلى دليل CLI، وإخطار المشرفين/المشرفين، والسجلات الملحقة بـ Torii في مستند الحادث. |
| نزاع حول الاسم الفريد أو عدم تطابق الدفع أو تصعيد العميل | سيف 2 | ستيوارد + قائد الدعم | اجمع إثباتات الدفع، وحدد ما إذا كان التجميد سهلًا وضروريًا، والرد على طلب الحصول على اتفاقية مستوى الخدمة (SLA)، وتسجيل النتيجة في تعقب النزاع. |
| مراقبة الامتثال | سيف 2 | الاتصال بالمطابقة | قم بإعادة كتابة خطة الإصلاح، وحفظ المذكرة في `docs/source/sns/regulatory/`، وجدول جلسة المشورة المرافقة. |
| حفر أو إنسايو | سيف 3 | مساء القيام بالبرنامج | قم بتنفيذ السيناريو المكرر `ops/drill-log.md`، واحفظ العناصر، وحدد الفجوات مثل مهام خريطة الطريق. |

يجب إنشاء جميع الأحداث `incident/YYYY-MM-DD-sns-<slug>.md` مع اللوحات
الملكية وسجلات الأوامر والمراجع كأدلة منتجة منذ فترة طويلة
قواعد اللعبة التي تمارسها.

## 9. المراجع- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (secoes SNS، DG، ADDR)

حافظ على تحديث هذا الدليل باستمرار لأن نص البطاقة هو سطح
CLI أو عقود القياس عن بعد؛ كما تفعل المدخلات خارطة الطريق كيو
مرجع `docs/source/sns/governance_playbook.md` دائما مراسل أ
المراجعة النهائية.