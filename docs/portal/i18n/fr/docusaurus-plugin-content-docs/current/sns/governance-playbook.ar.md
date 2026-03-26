---
lang: fr
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/governance_playbook.md` وتعمل الان كمرجع بوابة
موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

# دليل حوكمة خدمة أسماء سورا (SN-6)

**الحالة:** du 2026-03-24 - مرجع حي لاستعداد SN-1/SN-6  
**Résolu de résolution :** SN-6 "Conformité et résolution des litiges", SN-7 "Résolveur et synchronisation de passerelle", ou ADDR-1/ADDR-5  
**المتطلبات المسبقة :** مخطط السجل في [`registry-schema.md`](./registry-schema.md), عقد API للمسجل في [`registrar-api.md`](./registrar-api.md) ، ارشادات تجربة العناوين في [`address-display-guidelines.md`](./address-display-guidelines.md) ، وقواعد بنية الحساب Dans [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

يصف هذا الدليل كيف تعتمد هيئات حوكمة أسماء سورا (SNS) المواثيق، وتوافق على
Les fonctions de résolution et de passerelle sont également compatibles avec le résolveur et la passerelle. يفي
بمتطلب خارطة الطريق byان تتشارك CLI `sns governance ...` et Norito
واثار التدقيقية مرجعا تشغيليا واحدا قبل N1 (الاطلاق العام).

## 1. النطاق والجمهور

يستهدف المستند:

- اعضاء مجلس الحوكمة الذين يصوتون على المواثيق، وسياسات اللاحقات، ونتائج النزاعات.
- اعضاء مجلس gardien الذين يصدرون تجميدات طارئة ويراجعون التراجعات.
- stewards اللاحقات الذين يديرون طوابير المسجل، ويوافقون على المزادات، ويديرون
  تقسيمات الايرادات.
- Un résolveur/passerelle pour SoraDNS et GAR
  التليمترية.
- فرق الامتثال والخزينة والدعم التي يجبان تثبت ان كل اجراء حوكمة ترك اثار Norito
  قابلة للتدقيق.يغطي مراحل البيتا المغلقة (N0) et والاطلاق العام (N1) et (N2) المدرجة في
`roadmap.md` من خلال ربط كل سير عمل بالادلة المطلوبة ولوحات المتابعة ومسارات
التصعيد.

## 2. الادوار وخريطة الاتصال| الدور | المسؤوليات الاساسية | ابرز الاثار والتليمترية | التصعيد |
|------|------------|---------------|--------------|
| مجلس الحوكمة | Ils sont également des stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, ou `sns governance charter submit`. | رئيس المجلس + متعقب جدول اعمال الحوكمة. |
| مجلس gardien | يصدر تجميدات soft/hard, وقوانين طارئة، ومراجعات 72 h. | Le gardien gardien est `sns governance freeze` et le gardien est `artifacts/sns/guardian/*`. | دورية tuteur de garde (<=15 min ACK). |
| intendants اللاحقة | يديرون طوابير المسجل، والمزادات، وشرائح التسعير، واتصالات العملاء؛ ويقرون بالامتثال. | سياسات steward في `SuffixPolicyV1`, جداول مرجعية للتسعير, اقرارات steward المخزنة بجانب المذكرات التنظيمية. | قائد برنامج steward + PagerDuty est également disponible. |
| عمليات المسجل والفوترة | Utilisez le modèle `/v1/sns/*`, les paramètres d'interface, les paramètres d'interface et la CLI. | API المسجل ([`registrar-api.md`](./registrar-api.md)), مقاييس `sns_registrar_status_total`, اثباتات الدفع المؤرشفة تحت `artifacts/sns/payments/*`. | مدير مناوبة المسجل ورابط الخزينة. |
| مشغلو résolveur والبوابة | يحافظون على SoraDNS et GAR وحالة البوابة متوافقة مع احداث المسجل؛ ويبثون مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Résolveur SRE de garde + جسر عمليات البوابة. || الخزينة والمالية | Il s'agit d'une assurance 70/30, d'une référence et d'un SLA. | مانيفستات تراكم الايرادات، صادرات Stripe/الخزينة، ملاحق KPI ربع سنوية تحت `docs/source/sns/regulatory/`. | مراقب المالية + مسؤول الامتثال. |
| جهة اتصال الامتثال والتنظيم | Il s'agit de la mise en œuvre d'un DSA UE et d'un KPI et d'un KPI. | مذكرات تنظيمية في `docs/source/sns/regulatory/`, عروض مرجعية, ومدخلات `ops/drill-log.md` لتجارب الطاولة. | قائد برنامج الامتثال. |
| الدعم / SRE عند الطلب | Il s'agit d'un résolveur (résolveur) et d'un résolveur. | Pour le système `ops/drill-log.md`, vous pouvez utiliser Slack/war-room pour `incident/`. | دورية de garde لـSNS + ادارة SRE. |

## 3. الاثار المرجعية ومصادر البيانات| الاثر | الموقع | الغرض |
|-------|--------|-------|
| الميثاق + ملاحق KPI | `docs/source/sns/governance_addenda/` | Vous pouvez également utiliser les KPI et les KPI et les CLI. |
| مخطط السجل | [`registry-schema.md`](./registry-schema.md) | Norito المرجعية (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| عقد المسجل | [`registrar-api.md`](./registrar-api.md) | Les REST/gRPC sont des hooks `sns_registrar_status_total`. |
| دليل UX للعناوين | [`address-display-guidelines.md`](./address-display-guidelines.md) | عروض i105 (المفضلة) والمضغوطة (الخيار الثاني) المرجعية التي تعكسها المحافظ/المستكشفات. |
| Par SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | اشتقاق المضيفات الحتمي، سير عمل tailer للشفافية، وقواعد التنبيه. |
| مذكرات تنظيمية | `docs/source/sns/regulatory/` | ملاحظات استقبال حسب الولاية (مثل EU DSA) ، اقرارات steward, ملاحق قوالب. |
| forets | `ops/drill-log.md` | سجل لتجارب الفوضى وIR المطلوبة قبل الخروج من المراحل. |
| تخزين الاثار | `artifacts/sns/` | Il s'agit du Guardian Guardian, du résolveur et du KPI et de la CLI `sns governance ...`. |

يجب ان تشير كل اجراءات الحوكمة الى اثر واحد على الاقل من الجدول اعلاه حتى يتمكن
Il s'est écoulé jusqu'au 24 janvier.

## 4. ادلة دورة الحياة

### 4.1 حركات الميثاق وsteward| الخطوة | المالك | CLI / الدليل | ملاحظات |
|--------|--------|---------------|---------|
| صياغة الملحق وفروق KPI | مقرر المجلس + قائد intendant | Comment Markdown est-il disponible `docs/source/sns/governance_addenda/YY/` | Il y a des crochets pour les KPI et des crochets. |
| تقديم الاقتراح | رئيس المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1`) | Utilisez la CLI pour Norito comme `artifacts/sns/governance/<id>/charter_motion.json`. |
| تصويت واعتراف tuteur | المجلس + tuteurs | `sns governance ballot cast --proposal <id>` et `sns governance guardian-ack --proposal <id>` | ارفاق محاضر مجزأة واثباتات النصاب. |
| قبول intendant | stewards | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل تغيير سياسات اللاحقات; سجل الظرف تحت `artifacts/sns/governance/<id>/steward_ack.json`. |
| التفعيل | عمليات المسجل | Utilisez `SuffixPolicyV1`, puis installez-le dans `status.md`. | Vous êtes en contact avec `sns_governance_activation_total`. |
| سجل التدقيق | الامتثال | Il s'agit d'une perceuse `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et d'une perceuse pour table. | تضمين مراجع لوحات التليمترية وفروقات السياسة. |

### 4.2 Questions et réponses1. **الفحص المسبق:** يستعلم المسجل `SuffixPolicyV1` لتاكيد شريحة التسعير، الشروط
   المتاحة، ونوافذ السماح/الاسترداد. ابق جداول التسعير متزامنة مع جدول الشرائح
   3/4/5/6-9/10+ (الشريحة الاساسية + معاملات اللاحقة) الموثقة في feuille de route.
2. ** Offre sous pli cacheté : ** Prime premium, engagement de 72 h / révélation de 24 h
   Voir `sns governance auction commit` / `... reveal`. انشر قائمة commits (hachages
   فقط) تحت `artifacts/sns/auctions/<name>/commit.json` حتى يتمكن المدققون من
   التحقق من العشوائية.
3. **التحقق من الدفع :** يتحقق المسجلون من `PaymentProofV1` مقابل تقسيمات الخزينة
   (70 % d'administrateurs / 30 % d'intendant et de référence d'exclusion <=10 %). Utiliser JSON Norito
   `artifacts/sns/payments/<tx>.json` est utilisé pour le téléchargement (`RevenueAccrualEventV1`).
4. **Hook الحوكمة:** ارفق `GovernanceHookV1` للاسماء premium/guarded مع مراجع
   لمعرفات مقترح المجلس وتواقيع intendant. غياب crochets يؤدي الى
   `sns_err_governance_missing`.
5. **التفعيل + مزامنة solver:** بمجرد ان يرسل Torii حدث التسجيل، شغل tailer
   Le résolveur est un outil de résolution de problèmes GAR/zone (version 4.5).
6. **افصاح العميل:** حدث دفتر المستخدِم (portefeuille/explorateur) et les luminaires sont ici
   [`address-display-guidelines.md`](./address-display-guidelines.md), je suis en train de le faire
   عروض i105 والمضغوط تتطابق مع توجيهات النص/QR.

### 4.3 التجديد والفوترة وتسوية الخزينة- **Prix du voyage :** يفرض المسجلون نافذة سماح 30 يوم + نافذة استرداد 60 يوم
  Il s'agit de `SuffixPolicyV1`. Depuis 60 ans, vous avez besoin d'aide
  الهولندية (7 ايام، رسوم 10x تنخفض 15%/يوم) par `sns governance reopen`.
- **تقسيم الايرادات:** كل تجديد او تحويل ينشئ `RevenueAccrualEventV1`. يجب على
  صادرات الخزينة (CSV/Parquet) التسوية مع هذه الاحداث يوميا; ارفق الاثباتات في
  `artifacts/sns/treasury/<date>.json`.
- **Référence pour les exclusions :** يتم تتبع نسب référence الاختيارية لكل لاحقة عبر اضافة
  `referral_share` الى سياسة intendant. يصدر المسجلون التقسيم النهائي ويخزنون
  مانيـفستات référence بجانب اثبات الدفع.
- **وتيرة التقارير:** تنشر المالية ملاحق KPI شهرية (التسجيلات، التجديدات، ARPU،
  استخدام النزاعات/bond) تحت `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  يجب ان تعتمد لوحات المتابعة على الجداول المصدرة نفسها حتى تطابق ارقام Grafana
  ادلة الدفتر.
- **مراجعة KPI شهرية :** يجمع فحص اول ثلاثاء قائد المالية وsteward المناوب وPM
  البرنامج. افتح [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) (تضمين البوابة
  `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`) صدّر جداول انتاجية
  المسجل والايرادات، سجل الفروقات في الملحق، وارفق الاثار بالمذكرة. فعّل حادثا
  Il s'agit d'un SLA (prise en charge > 72 h, qui correspond à l'ARPU).

### 4.4 Questions et réponses| المرحلة | المالك | الاجراء والدليل | ANS |
|---------|--------|--------|-----|
| طلب تجميد soft | intendant / الدعم | قدم تذكرة `SNS-DF-<id>` مع اثباتات الدفع، مرجع bond النزاع، والمحدد/المحددات المتاثرة. | <=4 h من الاستلام. |
| تذكرة gardien | مجلس gardien | `sns governance freeze --selector <i105> --reason <text> --until <ts>` à `GuardianFreezeTicketV1`. La version JSON est `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h تنفيذ. |
| تصديق المجلس | مجلس الحوكمة | يوافق او يرفض التجميدات، ويوثق القرار مع رابط لتذكرة tuteur وبصمة bond النزاع. | جلسة المجلس التالية او تصويت غير متزامن. |
| لجنة التحكيم | الامتثال + intendant | عقد لجنة من 7 محلفين (حسب roadmap) مع بطاقات تصويت مجزأة عبر `sns governance dispute ballot`. ارفق ايصالات التصويت المجهولة بحزمة الحادث. | الحكم <=7 ايام بعد ايداع lien. |
| استئناف | tuteur + المجلس | يضاعف الاستئناف bond ويعيد عملية المحلفين; سجل مانيفست Norito `DisputeAppealV1` واربط بالتذكرة الاصلية. | <=10 ايام. |
| فك التجميد والمعالجة | المسجل + عمليات résolveur | Utilisez `sns governance unfreeze --selector <i105> --ticket <id>` pour utiliser GAR/résolveur. | مباشرة بعد الحكم. |

القوانين الطارئة (تجميدات يطلقها tuteur <=72 h) تتبع نفس التدفق لكنها تتطلب
مراجعة مجلس بأثر رجعي وملاحظة شفافية تحت `docs/source/sns/regulatory/`.

### 4.5 Mise à jour du résolveur1. **Hook الحدث:** يرسل كل حدث تسجيل الى دفق احداث résolveur (`tools/soradns-resolver` SSE).
   يشترك فريق solver ويسجل الفروقات عبر tailer الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **تحديث قالب GAR:** يجب على البوابات تحديث قوالب GAR المشار اليها بواسطة
   `canonical_gateway_suffix()` est également compatible avec `host_pattern`. خزن الفروقات في
   `artifacts/sns/gar/<date>.patch`.
3. **Fichier de zone :** Utilisez le fichier de zone comme `roadmap.md` (nom, ttl, cid, preuve)
   Il s'agit de Torii/SoraFS. Utilisez JSON Norito comme `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **فحص الشفافية:** شغل `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   للتأكد من بقاء التنبيهات خضراء. ارفق مخرجات Prometheus النصية بالتقرير الاسبوعي للشفافية.
5. **Conditions de voyage :** Pour le client `Sora-*` (pour le CSP avec GAR)
   وارفقها بسجل الحوكمة لكي يثبت المشغلون ان البوابة قدمت الاسم الجديد مع حواجز الحماية المقصودة.

## 5. التليمترية والتقارير| الاشارة | المصدر | الوصف / الاجراء |
|---------|--------|-----------------|
| `sns_registrar_status_total{result,suffix}` | معالجات مسجل Torii | عداد نجاح/خطا للتسجيلات، التجديدات، التجميدات، التحويلات؛ ينبه عندما يرتفع `result="error"` لكل لاحقة. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | مقاييس Torii | SLO pour l'API Utilisez le code `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` et `soradns_bundle_cid_drift_total` | tailer résolveur | تكشف ادلة قديمة او انحراف GAR؛ حواجز معرفة في `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI الحوكمة | عداد يزداد عند تفعيل ميثاق/ملحق؛ يستخدم لتسوية قرارات المجلس مقابل الملاحق المنشورة. |
| Jauge `guardian_freeze_active` | Gardien CLI | يتتبع نوافذ تجميد soft/hard لكل محدد؛ Il s'agit du SRE `1` pour le SLA. |
| لوحات ملاحق KPI | المالية / الوثائق | ملخصات شهرية تنشر مع المذكرات التنظيمية؛ تضمها البوابة عبر [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) ليتمكن stewards والمنظمون من الوصول لنفس عرض Grafana. |

## 6. متطلبات الادلة والتدقيق| الاجراء | الادلة التي يجب ارشفتها | التخزين |
|---------|----------------|---------|
| تغيير الميثاق / السياسة | مانيفست Norito موقع، نص CLI, فرق KPI, اقرار steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| تسجيل / تجديد | حمولة `RegisterNameRequestV1`, `RevenueAccrualEventV1`, اثبات الدفع. | `artifacts/sns/payments/<tx>.json`, l'API est disponible. |
| مزاد | مانيفستات commit/reveal, بذرة العشوائية, جدول حساب الفائز. | `artifacts/sns/auctions/<name>/`. |
| تجميد / فك تجميد | تذكرة tuteur, تجزئة تصويت المجلس، رابط سجل الحادث، قالب تواصل العملاء. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| انتشار résolveur | Il s'agit d'un fichier zonefile/GAR, qui utilise JSONL pour un tailer, comme Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| الاستقبال التنظيمي | Il s'agit d'un steward et d'un KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة بوابات المرحلة| المرحلة | معايير الخروج | حزمة الادلة |
|---------|--------------|-------------|
| N0 - بيتا مغلقة | Utilisez SN-1/SN-2 et CLI pour connecter Guardian. | Le gestionnaire + ACK steward est associé au résolveur `ops/drill-log.md`. |
| N1 - اطلاق عام | مزادات + شرائح اسعار ثابتة مفعلة لـ`.sora`/`.nexus`, مسجل ذاتي الخدمة، مزامنة تلقائية للresolver, لوحات فوترة. | فرق ورقة التسعير، نتائج CI للمسجل، ملحق الدفع/KPI، مخرجات tailer الشفافية، ملاحظات تمرين الحوادث. |
| N2 - توسع | `.dao`, revendeur de revendeurs, revendeur de services steward. | لقطات شاشة للبوابة، مقاييس SLA للنزاعات، صادرات بطاقات تقييم steward, ميثاق حوكمة محدث يشير Revendeur de produits لسياسات. |

Utilisez un résolveur de table pour résoudre ce problème.
مع ارفاق الاثار في `ops/drill-log.md`.

## 8. الاستجابة للحوادث والتصعيد| المشغل | الشدة | المالك الفوري | الاجراءات الزامية |
|--------|-------|---------------|----------------------|
| Installer solver/GAR et ادلة قديمة | 1 septembre | Résolveur SRE + gardien مجلس | استدعاء on-call لللللل، التقاط مخرجات tailer، تقرير ما اذا كان يجب تجميد الاسماء المتاثرة، نشر تحديث حالة كل 30 minutes. |
| تعطل المسجل، فشل الفوترة، او اخطاء API واسعة | 1 septembre | مدير مناوبة المسجل | ايقاف المزادات الجديدة، التحول الى CLI يدوي، اخطار stewards/الخزينة، ارفاق سجلات Torii بوثيقة الحادث. |
| نزاع اسم واحد، عدم تطابق الدفع، او تصعيد عميل | 2 septembre | intendant + قائد الدعم | Il s'agit d'un logiciel soft qui est également compatible avec SLA. |
| ملاحظة تدقيق امتثال | 2 septembre | جهة اتصال الامتثال | صياغة خطة معالجة، حفظ مذكرة تحت `docs/source/sns/regulatory/`, جدولة جلسة مجلس متابعة. |
| تدريب او بروفة | 3 septembre | PM البرنامج | Il s'agit de la référence `ops/drill-log.md`, qui correspond à la feuille de route. |

يجب على كل الحوادث ان تنشئ `incident/YYYY-MM-DD-sns-<slug>.md` مع جداول الملكية
وسجلات الاوامر ومراجع الادلة المنتجة عبر هذا الدليل.

## 9. المراجع

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (SNS, DG et ADDR)حافظ على تحديث هذا الدليل كلما تغيرت صياغة الميثاق او اسطح CLI او عقود
التليمترية; يجب ان تطابق عناصر feuille de route التي تشير الى
`docs/source/sns/governance_playbook.md` احدث مراجعة دائما.