---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
C'est `docs/source/da/threat_model.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# نموذج تهديدات توفر البيانات pour Sora Nexus

_اخر مراجعة: 2026-01-19 -- المراجعة القادمة المجدولة: 2026-04-19_

وتيرة الصيانة: Groupe de travail sur la disponibilité des données (<=90 يوما). يجب ان تظهر كل
مراجعة في `status.md` مع روابط لتذاكر التخفيف النشطة واثار المحاكاة.

## الهدف والنطاق

Pour la disponibilité des données (DA) pour Taikai et les blobs Nexus et les blobs
الحوكمة قابلة للاسترجاع تحت اخطاء بيزنطية، وشبكات، ومشغلين. هذا النموذج
يثبت عمل الهندسة لDA-1 (البنية ونموذج التهديدات) et خط اساس لمهام DA اللاحقة
(DA-2 ou DA-10).

المكونات ضمن النطاق:
- Permet d'ingérer les métadonnées Torii et Norito.
- اشجار تخزين blobs المدعومة بSoraFS (طبقات chaud/froid) et سياسات التكرار.
- تعهدات كتل Nexus (formats filaires, épreuves, API client léger).
- Application des hooks pour PDP/PoTR pour DA.
- تدفقات المشغل (épinglage, expulsion, coupure) et وخطوط الرصد.
- موافقات الحوكمة التي تقبل او تزيل مشغلي DA والمحتوى.

خارج النطاق لهذا المستند:
- نمذجة الاقتصاد الكاملة (ضمن مسار DA-7).
- بروتوكولات SoraFS الاساسية المغطاة في نموذج تهديدات SoraFS.
- ارغونوميا SDK للعميل خارج اعتبارات سطح التهديد.

## نظرة عامة معمارية1. **ارسال :** Les blobs d'API sont ingérés par DA dans Torii. تقوم العقدة
   Les blobs et les manifestes Norito (blob, voie, époque, codec)
   Il s'agit de morceaux dans le fichier SoraFS.
2. **الاعلان:** تنتشر نوايا pin et تلميحات التكرار الى مزودي التخزين عبر
   registre (marché SoraFS) مع وسوم سياسة تحدد اهداف الاحتفاظ chaud/froid.
3. **Applications :** Séquenceurs pour blobs Nexus (CID + KZG)
   في الكتلة القياسية. يعتمد light clients على hash الالتزام والmétadonnées
   المعلنة للتحقق من disponibilité.
4. **التكرار:** تسحب عقد التخزين الحصص/chunks المخصصة، et تلبي تحديات PDP/PoTR،
   Les aliments sont chauds et froids.
5. **الب:** يسترجع المستهلكون البيانات عبر SoraFS et DA-aware مع
   التحقق من proofs ورفع طلبات اصلاح عند اختفاء النسخ.
6. **الحوكمة:** يوافق البرلمان ولجنة الاشراف على DA على المشغلين وجداول loyer
   وتصعيدات application. تحفظ ادوات الحوكمة عبر نفس مسار DA لضمان شفافية
   العملية.

## الاصول والمالكون

مقياس الاثر: **حرج** يكسر سلامة/حيوية الدفتر؛ **عال** يحجب remblai DA او
العملاء؛ **متوسط** يخفض الجودة لكنه قابل للاسترجاع؛ **منخفض** اثر محدود.| الاصل | الوصف | السلامة | الاتاحة | السرية | المالك |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (morceaux + manifestes) | Blobs Taikai et Lane sont des objets connectés pour SoraFS | حرج | حرج | متوسط ​​| DA WG / Equipe Stockage |
| Manifestes Norito DA | Métadonnées مصنفة تصف blobs | حرج | عال | متوسط ​​| GT sur le protocole principal |
| تعهدات الكتل | CIDs + KZG pour Nexus | حرج | عال | منخفض | GT sur le protocole principal |
| Lire PDP/PoTR | وتيرة application لنسخ DA | عال | عال | منخفض | Équipe de stockage |
| سجل المشغلين | مزودو التخزين الموافق عليهم والسياسات | عال | عال | منخفض | Conseil de gouvernance |
| Louer des appartements | قيود الدفتر لعائدات DA والعقوبات | عال | متوسط ​​| منخفض | GT Trésorerie |
| لوحات الرصد | SLOs DA وعمق التكرار والتنبيهات | متوسط ​​| عال | منخفض | SRE / Observabilité |
| نوايا الاصلاح | طلبات لاعادة ترطيب chunks المفقودة | متوسط ​​| متوسط ​​| منخفض | Équipe de stockage |

## الخصوم والقدرات| الفاعل | القدرات | الدوافع | الملاحظات |
| --- | --- | --- | --- |
| عميل خبيث | Les blobs ارسال تالفة، اعادة تشغيل manifestent قديمة، محاولة DoS على ingérer. | تعطيل بث Taikai, حقن بيانات غير صحيحة. | لا مفاتيح مميزة. |
| عقدة تخزين بيزنطية | Vous avez besoin de preuves PDP/PoTR. | تقليص احتفاظ DA، تجنب rent,، احتجاز البيانات. | يحمل بيانات اعتماد مشغل صحيحة. |
| séquenceur مخترق | Vous devez utiliser les métadonnées. | اخفاء ارسال DA، خلق عدم اتساق. | محدود باغلبية الاجماع. |
| مشغل داخلي | اساءة استخدام الوصول للحوكمة، العبث بسياسات الاحتفاظ، تسريب بيانات اعتماد. | مكسب اقتصادي، تخريب. | وصول الى بنية chaud/froid. |
| خصم شبكة | تقسيم العقد، تاخير التكرار, حقن حركة MITM. | Il s'agit de SLO. | لا يكسر TLS لكنه يمكنه اسقاط/ابطاء الروابط. |
| مهاجم الرصد | العبث باللوحات/التنبيهات، اخفاء الحوادث. | اخفاء انقطاعات DA. | يتطلب وصولا لخط التليمترية. |

## حدود الثقة- **حد الدخول :** العميل الى امتداد DA في Torii. يتطلب auth على مستوى الطلب،
  Il s'agit d'une charge utile.
- **حد التكرار :** Il y a des morceaux et des preuves. العقد متصادقة لكنها قد
  تتصرف بشكل بيزنطي.
- **حد الدفتر:** بيانات الكتلة الملتزمة مقابل التخزين خارج السلسلة. الاجماع
  يحمي السلامة، لكن الاتاحة تتطلب application خارج السلسلة.
- **حد الحوكمة:** قرارات Conseil/Parlement لاعتماد المشغلين والميزانيات
  والعقوبات. الاختراق هنا يؤثر مباشرة على نشر DA.
- **حد الرصد:** جمع metrics/logs المصدرة الى لوحات/تنبيهات. العبث يخفي
  الانقطاعات او الهجمات.

## سيناريوهات التهديد والضوابط

### هجمات مسار ingérer

**السيناريو:** Les charges utiles Norito sont utilisées pour les blobs et les charges utiles.
Les métadonnées sont également disponibles.

**ضوابط**
- Utilisez le schéma Norito pour utiliser le schéma Norito. رفض الاعلام غير المعروفة.
- La gestion de l'ingestion du point de terminaison est Torii.
- La taille des morceaux est définie par le chunker SoraFS.
- خط القبول لا يحفظ manifeste الا بعد تطابق checksum السلامة.
- Replay cache حتمي (`ReplayCache`) يتتبع نوافذ `(lane, epoch, sequence)`,
  يحفظ laisses de hautes eaux على القرص، ويرفض التكرار/اعادة التشغيل القديمة؛
  exploite la propriété وfuzz تغطي empreintes digitales مختلفة وارسال خارج الترتيب.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]**ثغرات المتبقية**
- J'utilise Torii pour ingérer le cache de relecture pour la séquence de séquences
  اعادة التشغيل.
- مخططات Norito DA تملك الان fuzz harnais مخصصا (`fuzz/da_ingest_schema.rs`) pour
  ثوابت encoder/décoder؛ يجب ان تنبه لوحات التغطية عند التراجع.

### احتجاز التكرار

**السيناريو:** مشغلو التخزين البيزنطيون يقبلون مهام pin لكنهم يسقطون chunks,
ويمررون تحديات PDP/PoTR عبر ردود مزورة او تواطؤ.

**ضوابط**
- جدول تحديات PDP/PoTR يمتد لحمولات DA مع تغطية لكل époque.
- تكرار متعدد المصادر مع عتبات quorum؛ orchestrateur يكتشف fragments المفقودة
  ويطلق اصلاحات.
- couper حوكمي مرتبط بفشل preuves والنسخ المفقودة.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) pour les reçus
  Les engagements DA (SignedBlockWire, `.norito`, et JSON) et le bundle JSON sont disponibles
  للحوكمة، وتفشل عند فقدان/عدم تطابق tickets لكي يطلق Alertmanager تنبيهات.

**ثغرات المتبقية**
- harnais المحاكاة في `integration_tests/src/da/pdp_potr.rs` (مغطى عبر
  `integration_tests/tests/da/pdp_potr_simulation.rs`) يختبر سيناريوهات تواطؤ
  Il s'agit du PDP/PoTR qui est en contact avec la société. استمر في
  Le DA-5 est une preuve de preuve.
- سياسة expulsion لطبقة froid تحتاج سجل تدقيق موقع لمنع الاسقاطات الخفية.

### العبث بالتعهدات

**السيناريو:** séquenceur مخترق ينشر كتل تحذف او تغير تعهدات DA، ما يسبب فشل
récupérer et récupérer les clients légers.**ضوابط**
- الاجماع يطابق مقترحات الكتل مع قوائم ارسال DA؛ النظراء يرفضون المقترحات
  التي تفتقد تعهدات مطلوبة.
- Les clients légers utilisent des preuves d'inclusion et gèrent للfetch.
- سجل تدقيق يقارن reçus الارسال بتعهدات الكتل.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) pour les reçus
  Les engagements DA (SignedBlockWire, `.norito`, et JSON) et le bundle JSON sont disponibles
  للحوكمة، وتفشل عند فقدان/عدم تطابق tickets لكي يطلق Alertmanager تنبيهات.

**ثغرات المتبقية**
- مغطاة بمهمة المصالحة + crochet Alertmanager؛ حزم الحوكمة تستهلك bundle JSON
  كافتراضي.

### تقسيم الشبكة والرقابة

**السيناريو :** خصم يقسم شبكة التكرار، مانعا العقد من الحصول على chunks
Il s'agit également du PDP/PoTR.

**ضوابط**
- متطلبات مزودين multi-région تضمن مسارات شبكة متنوعة.
- نوافذ التحدي تتضمن jitter وfallback الى قنوات اصلاح خارج النطاق.
- لوحات الرصد تراقب عمق التكرار ونجاح التحديات وزمن récupérer مع عتبات تنبيه.

**ثغرات المتبقية**
- محاكاة التقسيم لاحداث Taikai المباشرة ما زالت مفقودة؛ Effectuez des tests de trempage.
- سياسة حجز سعة اصلاح لم توثق بعد.

### اساءة داخلية

**السيناريو:** مشغل لديه وصول للregistry يتلاعب بسياسات الاحتفاظ، ويمنح
liste blanche pour les utilisateurs et les utilisateurs.**ضوابط**
- اعمال الحوكمة تتطلب تواقيع متعددة الاطراف وسجلات Norito موثقة.
- تغييرات السياسة تصدر احداثا للرصد وسجلات ارشيف.
- Vous pouvez utiliser le chaînage de hachage Norito en ajout uniquement.
- اتمتة مراجعة الوصول ربع السنوية (`cargo xtask da-privilege-audit`) تتفقد
  ادلة manifest/replay (مع مسارات يحددها المشغلون) et وتعلم العناصر المفقودة/
  Il s'agit d'un bundle JSON avec des liens vers des liens.

**ثغرات المتبقية**
- ادلة العبث في اللوحات تحتاج instantanés موقعة.

## سجل المخاطر المتبقية| الخطر | الاحتمال | الاثر | المالك | خطة التخفيف |
| --- | --- | --- | --- | --- |
| Replay des manifestations DA et de la séquence de cache dans DA-2 | ممكن | متوسط ​​| GT sur le protocole principal | Utiliser le cache de séquence + ajouter un nom occasionnel à DA-2 اضافة اختبارات تراجع. |
| تواطؤ PDP/PoTR عند اختراق >f عقد | غير محتمل | عال | Équipe de stockage | اشتقاق جدول تحديات جديد مع échantillonnage multi-fournisseurs التحقق عبر harnais المحاكاة. |
| فجوة تدقيق expulsion في طبقة froid | ممكن | عال | SRE / Equipe Stockage | ربط سجلات موقعة وووصولات on-chain لعمليات expulsion؛ الرصد عبر اللوحات. |
| زمن اكتشاف حذف séquenceur | ممكن | عال | GT sur le protocole principal | `cargo xtask da-commitment-reconcile` pour les reçus et les engagements (SignedBlockWire/`.norito`/JSON) et les billets pour les billets متطابقة. |
| مقاومة التقسيم لبث Taikai المباشر | ممكن | حرج | Réseautage TL | تنفيذ تدريبات تقسيم؛ حجز سعة اصلاح؛ توثيق SOP للفشل. |
| انحراف امتيازات الحوكمة | غير محتمل | عال | Conseil de gouvernance | `cargo xtask da-privilege-audit` ربع سنوي (répertoires manifest/replay + مسارات اضافية) avec JSON موقع + gate لوحة؛ تثبيت artefacts التدقيق على السلسلة. |

## المتابعات المطلوبة1. Utilisez le module Norito pour ingest DA et le module DA-2.
2. تمرير replay cache عبر Torii DA ingest وحفظ مؤشرات عبر اعادة التشغيل.
3. **مكتمل (2026-02-05):** harnais محاكاة PDP/PoTR يمارس تواطؤ + تقسيم مع نمذجة
   QoS du backlog راجع `integration_tests/src/da/pdp_potr.rs` (مع اختبارات في
   `integration_tests/tests/da/pdp_potr_simulation.rs`)
   الحتمية الملتقطة ادناه.
4. **مكتمل (2026-05-29):** `cargo xtask da-commitment-reconcile` reçus
   Pour les engagements DA (SignedBlockWire/`.norito`/JSON), vous pouvez
   `artifacts/da/commitment_reconciliation.json`, ومربوط بAlertmanager/حزم
   الحوكمة لتنبيهات omission/falsification (`xtask/src/da.rs`).
5. **مكتمل (2026-05-29):** `cargo xtask da-privilege-audit` يتجول في bobine
   manifest/replay (مع مسارات يحددها المشغلون)، ويحدد العناصر المفقودة/غير
   دليل/قابلة للكتابة عالميا، وينتج bundle JSON pour للحوكمة
   (`artifacts/da/privilege_audit.json`), مغلقا فجوة اتمتة مراجعة الوصول.

**اين تتابع:**- replay cache et replay cache pour DA-2. راجع التنفيذ في
  `crates/iroha_core/src/da/replay_cache.rs` (cache supplémentaire) et Torii pour
  `crates/iroha_torii/src/da/ingest.rs` الذي يمرر vérifie les empreintes digitales par `/v1/da/ingest`.
- Le streaming PDP/PoTR est également disponible pour exploiter le flux de preuve
  `crates/sorafs_car/tests/sorafs_cli.rs`, pour les projets PoR/PDP/PoTR
  الفشل المشار اليها في نموذج التهديدات.
- Capacité de trempage de réparation موجودة في
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, pour le trempage Sumeragi
  Il s'agit d'un `docs/source/sumeragi_soak_matrix.md` (مع نسخ محلية).
  Les artefacts sont considérés comme des objets précieux.
- اتمتة المصالحة + privilège-audit موجودة في `docs/automation/da/README.md`
  والاوامر الجديدة `cargo xtask da-commitment-reconcile` /
  `cargo xtask da-privilege-audit` ; استخدم المخارج الافتراضية تحت
  `artifacts/da/` عند ارفاق الادلة بحزم الحوكمة.

## ادلة المحاكاة ونمذجة QoS (2026-02)

Pour le DA-1 #3, pour le harnais pour le PDP/PoTR pour les détails
`integration_tests/src/da/pdp_potr.rs` (مغطي بواسطة
`integration_tests/tests/da/pdp_potr_simulation.rs`). يقوم harnais بتوزيع العقد
Il s'agit de la feuille de route et du PoTR.
Le backlog est très chaud. تشغيل السيناريو
الافتراضي (12 époques, 18 تحديا PDP + نافذتان PoTR لكل époque) انتج المقاييس
التالية:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| المقياس | القيمة | الملاحظات |
| --- | --- | --- |
| Pannes PDP détectées | 48 / 49 (98,0%) | ما زالت partitions تطلق الاكتشاف؛ Il s'agit d'un problème de gigue. |
| Latence de détection moyenne PDP | 0,0 époques | يتم اظهار الاعطال ضمن époque الاصلي. |
| Défaillances PoTR détectées | 28 / 77 (36,4%) | يتم اطلاق الاكتشاف عند فقدان العقدة >=2 نوافذ PoTR, مما يترك معظم الاحداث في سجل المخاطر المتبقية. |
| Latence de détection moyenne PoTR | époques 2.0 | يطابق عتبة تاخر مقدارها époque مضمنة في تصعيد الارشفة. |
| Pic de la file d'attente de réparation | 38 manifestes | يرتفع backlog اصلاحات متاحة لكل époque. |
| Latence de réponse p95 | 30 068 ms | Il y a 30 niveaux de gigue +/-75 ms pour la QoS. |
<!-- END_DA_SIM_TABLE -->

هذه المخرجات تقود الان نماذج لوحات DA وتفي بمعايير قبول "harnais de simulation +
Modélisation QoS" dans la feuille de route.

الان توجد الاتمتة خلف
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
Comment utiliser le harnais Norito JSON
`artifacts/da/threat_model_report.json` افتراضيا. تستخدم الوظائف الليلية هذا
الملف لتحديث المصفوفات في هذا المستند والتنبيه عند انحراف معدلات الاكتشاف
Il s'agit également de QoS.

لتحديث الجدول اعلاه للوثائق، شغل `make docs-da-threat-model`, والذي يستدعي
`cargo xtask da-threat-model-report` et votre réponse
`docs/source/da/_generated/threat_model_report.json` ويعيد كتابة هذا القسم عبر
`scripts/docs/render_da_threat_model_tables.py`. يتم تحديث نسخة `docs/portal`
(`docs/portal/docs/da/threat-model.md`) pour votre recherche en ligne
متزامنتين.