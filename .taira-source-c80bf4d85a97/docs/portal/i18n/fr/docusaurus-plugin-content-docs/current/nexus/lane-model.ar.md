---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/lane-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : modèle nexus-lane
titre : نموذج voies في Nexus
description : Il s'agit d'une voie réservée aux voies et à l'état mondial de Sora Nexus.
---

# نموذج voies pour Nexus et WSV

> **الحالة:** Pour NX-1 - تصنيف voies, هندسة التهيئة، وتخطيط التخزين جاهزة للتنفيذ.  
> **المالكون :** Nexus Groupe de travail principal, GT sur la gouvernance  
> **Feuille de route suivante :** NX-1 pour `roadmap.md`

Utilisez le SDK `docs/source/nexus_lanes.md` pour Sora Nexus et votre SDK Les voies lanes sont des mono-repo. تحافظ الهندسة المستهدفة على حتمية world state مع السماح لمساحات البيانات (voies) الفردية بتشغيل مجموعات مدققين عامة او خاصة باعمال معزولة.

## المفاهيم

- **Lane:** shard est associé au grand livre Nexus et est associé au backlog تنفيذ. معرف بـ `LaneId` ثابت.
- **Espace de données :** حاوية حوكمة تجمع lane او اكثر تشترك في سياسات الامتثال والتوجيه والتسوية.
- **Manifeste de voie :** التوجيه.
- **Engagement mondial :** preuve de la voie de circulation et de la voie transversale. حلقة NPoS العالمية ترتب engagements.

## تصنيف voies

Il y a des voies et des crochets. هندسة التهيئة (`LaneConfig`) تلتقط هذه السمات كي تتمكن العقد والادوات من فهم التخطيط دون منطق مخصص.| nouvelle voie | الرؤية | عضوية المدققين | Lire WSV | الحوكمة الافتراضية | سياسة التسوية | الاستخدام المعتاد |
|---------------|------------|------------|--------------|--------------------|-------------------|-------------|
| `default_public` | عام | Sans autorisation (enjeu mondial) | نسخة حالة كاملة | SORA Parlement | `xor_global` | دفتر عام اساسي |
| `public_custom` | عام | Sans autorisation et avec participation | نسخة حالة كاملة | وحدة مرجحة بال participation | `xor_lane_weighted` | تطبيقات عامة عالية السعة |
| `private_permissioned` | مقيد | مجموعة مدققين ثابتة (معتمدة من الحوكمة) | Engagements et preuves | Conseil fédéré | `xor_hosted_custody` | CBDC, اعمال كونسورتيوم |
| `hybrid_confidential` | مقيد | عضوية مختلطة؛ تغلف Preuves ZK | Engagements + افصاح انتقائي | وحدة نقود قابلة للبرمجة | `xor_dual_fund` | نقود قابلة للبرمجة مع حفظ الخصوصية |

يجب على كل نوع lane التصريح بما يلي:

- Alias للداتاسبيس - تجميع مقروء للبشر يربط سياسات الامتثال.
- Poignée de gouvernance - معرف يحل عبر `Nexus.governance.modules`.
- Poignée de règlement - معرف يستهلكه routeur de règlement لخصم مخازن XOR.
- Métadonnées des tableaux de bord (et des tableaux de bord `/status`).

## هندسة تهيئة voies (`LaneConfig`)

`LaneConfig` est le runtime pour les voies de catalogue. لا تستبدل manifeste الحوكمة؛ بل توفر معرفات تخزين حتمية وتلميحات تيليمتري لكل lane مهيأة.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```- `LaneConfig::from_catalog` يعيد حساب الهندسة عند تحميل التهيئة (`State::set_nexus`).
- تتحول alias الى slugs بحروف صغيرة؛ تتكاثف الاحرف غير الابجدية الرقمية المتتالية الى `_`. Il s'agit de slug qui est `lane{id}`.
- `shard_id` مشتق من مفتاح métadonnées `da_shard_id` (الافتراضي `lane_id`) et journal مؤشر shard المثبت للحفاظ على replay حتمي ل DA عبر redémarre/resharding.
- Préfixes de préfixes associés à la voie WSV et au backend.
- اسماء مقاطع Kura حتمية عبر hôtes؛ يمكن للمدققين تدقيق ادلة المقاطع وmanifestes دون ادوات مخصصة.
- مقاطع الدمج (`lane_{id:03}_merge`) utilise les racines pour l'indice de fusion et les engagements dans la voie.

## تقسيم état-monde- état mondial pour Nexus et pour la voie. voies عامة حفظ حالة كاملة؛ voies الخاصة/confidentiel تصدر racines Merkle/engagement الى fusionner le grand livre.
- Utilisez MV pour 4 étapes avec `LaneConfigEntry::key_prefix`, puis avec `[00 00 00 01] ++ PackedKey`.
- Analyses de gamme (comptes, actifs, déclencheurs, déclencheurs) et analyses de plage.
- Les métadonnées du grand livre de fusion sont: la voie est l'indice de fusion des racines et les racines sont `lane_{id:03}_merge`, ما يسمح بالاحتفاظ او الازالة الموجهة عندما تتقاعد voie.
- Voie croisée (alias de compte, registres d'actifs, manifestes de gouvernance) et voie transversale pour les voies supplémentaires.
- **سياسة الاحتفاظ** - voies العامة تحتفظ باجسام الكتل كاملة؛ voies ذات engagements فقط يمكنها ضغط الاجسام الاقدم بعد points de contrôle لان engagements هي المرجع. voies confidentielles تحتفظ بسجلات مشفرة في مقاطع مخصصة كي لا تعيق workloads اخرى.
- **Outils** - Utilisez l'espace de noms (`kagami`, utilisé par l'administrateur dans la CLI) pour l'espace de noms et le slug et les métriques et les fonctionnalités. Prometheus et Kura.

## Routage des API- Utiliser le logiciel Torii REST/gRPC pour `lane_id`. Il s'agit de `lane_default`.
- Utiliser les SDK pour les voies et les alias des voies de catalogue `LaneId`.
- تعمل قواعد routage على catalogue المعتمد ويمكنها اختيار voie وdataspace معا. Les alias `LaneConfig` sont utilisés pour les tableaux de bord et les journaux.

## Règlement والرسوم

- كل lane تدفع رسوم XOR لمجموعة المدققين العالمية. يمكن لل voies تحصيل رموز gas محلية لكن يجب ان تودع ما يعادل XOR مع engagements.
- preuves التسوية تشمل المبلغ وmetadata التحويل ودليل escrow (مثلا تحويل الى vault الرسوم العالمية).
- Le routeur de règlement (NX-3) utilise des tampons pour la voie de règlement, ainsi que pour les tampons de la voie.

## Gouvernance

- تعلن voies وحدة الحوكمة عبر catalogue. Utilisez `LaneConfigEntry` alias وslug pour votre compte.
- Le registre Nexus manifeste pour la voie et `LaneId` pour l'espace de données, la gestion de gouvernance, la gestion de règlement et les métadonnées.
- Le runtime des hooks est utilisé pour les ponts de télémétrie (`gov_upgrade_id`) et les diffs pour le pont de télémétrie (`nexus.config.diff`).

## État de la télémétrie- `/status` pour les alias des voies et les espaces de données et les poignées et les alias du catalogue `LaneConfig`.
- Utiliser le planificateur (`nexus_scheduler_lane_teu_*`) alias/slugs pour réduire le retard et le TEU.
- `nexus_lane_configured_total` يحصي عدد ادخالات voie المشتقة ويعاد حسابه عند تغير التهيئة. تبعث التيليمتري diffs موقعة عندما تتغير هندسة voies.
- تتضمن jauge l'arriéré pour les métadonnées alias/description لمساعدة المشغلين على ربط ضغط الطوابير بالمجالات التجارية.

## التهيئة وانواع Norito

- `LaneCatalog`, `LaneConfig`, و`DataSpaceCatalog` sont compatibles avec `iroha_data_model::nexus` et sont compatibles avec Norito. manifeste les وSDK.
- `LaneConfig` est disponible dans `iroha_config::parameters::actual::Nexus` et est disponible dans le catalogue L'encodage Norito est un assistant pour le runtime.
- تظل تهيئة المستخدم (`iroha_config::parameters::user::Nexus`) تقبل واصفات voie et dataspace التصريحية؛ Il existe des alias et des voies d'identification.

## العمل المتبقي

- Il s'agit d'un routeur de règlement (NX-3) avec des tampons XOR pour la voie des slugs.
- Outils d'outillage pour les familles de colonnes et les voies pour l'espace de noms et le slug.
- انهاء خوارزمية الدمج (ordonnancement, élagage, détection de conflits) et luminaires رجعية لاعادة التشغيل cross-lane.
- Hooks pour les listes blanches/listes noires et les crochets pour les listes blanches/noires (avec NX-12).

---*ستواصل هذه الصفحة تتبع متابعات NX-1 ou NX-2 ou NX-18. يرجى اظهار الاسئلة المفتوحة في `roadmap.md` او tracker الحوكمة ليبقى البوابة متوافقة مع الوثائق القانونية.*