---
lang: fr
direction: ltr
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
C'est `docs/source/da/ingest_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة ingest لتوفر البيانات في Sora Nexus

_مسودة: 2026-02-20 - المالك: Core Protocol WG / Storage Team / DA WG_

يمدد مسار DA-2 منصة Torii pour ingérer des blobs en utilisant Norito الوصفية
وتبذر تكرار SoraFS. يوثق هذا المستند المخطط المقترح وسطح API وتدفق التحقق حتى
يتقدم التنفيذ دون انتظار محاكاة DA-1 المتبقية. يجب ان تستخدم جميع تنسيقات
الحمولة ترميزات Norito؛ Il est possible de recourir à une solution de secours serde/JSON.

## الاهداف

- قبول blobs كبيرة (قطاعات Taikai, للحارات، وادوات حوكمة) بشكل حتمي
  Voir Torii.
- Le manifeste Norito implémente l'effacement du blob et du codec
  وسياسة الاحتفاظ.
- Les morceaux de morceaux sont disponibles dans le SoraFS et sont connectés à votre ordinateur.
  الطابور.
- Ajouter une broche + un connecteur à un SoraFS.
- اتاحة reçus القبول كي يستعيد العملاء دليلا حتميا على النشر.

## API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Il s'agit d'un `DaIngestRequest` remplacé par un Norito. تستخدم الاستجابات
`application/norito+v1` et `DaIngestReceipt`.| الاستجابة | المعنى |
| --- | --- |
| 202 Accepté | تم وضع الـ blob في طابور التجزئة/التكرار؛ تم ارجاع reçu. |
| 400 requêtes incorrectes | Schéma/الحجم (انظر فحوصات التحقق). |
| 401 Non autorisé | رمز API مفقود/غير صالح. |
| 409 Conflit | Utilisez `client_blob_id` pour les appareils connectés. |
| 413 Charge utile trop importante | يتجاوز حد طول الـ blob المكون. |
| 429 Trop de demandes | تم بلوغ taux limite. |
| 500 Erreur interne | فشل غير متوقع (log + تنبيه). |

## مخطط Norito المقترح

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> ملاحظة تنفيذية: تم نقل التمثيلات Rust القياسية لهذه الحمولات الى
> `iroha_data_model::da::types`, pour les emballages/reçus
> `iroha_data_model::da::ingest` et manifeste pour `iroha_data_model::da::manifest`.

يعلن حقل `compression` كيف جهز المتصلون الحمولة. Torii `identity` و`gzip`
و`deflate` و`zstd` pour le hachage et les manifestes
الاختيارية.

### قائمة تحقق التحقق1. Utilisez le Norito pour le `DaIngestRequest`.
2. افشل اذا كان `total_size` مختلفا عن طول الحمولة القانوني (بعد فك الضغط) او
   يتجاوز الحد الاقصى المكون.
3. Utilisez `chunk_size` (pour = 2.
5. Utilisez le `retention_policy.required_replica_count` pour votre appareil.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع).
7. رفض `client_blob_id` المكرر ما لم تكن قيمة هاش الحمولة والبيانات الوصفية
   متطابقة.
8. Utiliser `norito_manifest` pour utiliser le schéma + hachage du manifeste
   حسابه بعد التجزئة؛ والا يقوم العقدة بتوليد manifeste وتخزينه.
9. تطبيق سياسة التكرار المكونة: يعيد Torii `RetentionPolicy` العبر
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`) et
   manifeste المجهزة مسبقا اذا لم تطابق بيانات الاحتفاظ الملف المفروض.

### تدفق التجزئة والتكرار1. Utilisez le module `chunk_size` et BLAKE3 pour chunk + Merkle.
2. Utilisez Norito `DaManifestV1` (struct جديدة) pour ajouter un morceau
   (role/group_id) et effacement (اعداد تكافؤ الصفوف والاعمدة مع
   `ipa_commitment`) , وسياسة الاحتفاظ والبيانات الوصفية.
3. Les octets du manifeste sont `config.da_ingest.manifest_store_dir`
   (Torii ou `manifest.encoded` par voie/époque/séquence/billet/empreinte digitale)
   Utilisez le ticket de stockage SoraFS pour le ticket de stockage du ticket de stockage.
4. Fixez la broche `sorafs_car::PinIntent` à l'intérieur de l'appareil.
5. Utilisez le Norito `DaIngestPublished` pour le système d'exploitation (عملاء خفيفين، الحوكمة،
   التحليلات).
6. ارجاع `DaIngestReceipt` للمتصل (موقع بمفتاح خدمة Torii DA) et ترويسة
   `Sora-PDP-Commitment` est la solution SDK la plus utilisée. يتضمن الـ reçu
   Pour `rent_quote` (Norito `DaRentQuote`) et `stripe_layout` pour
   المرسلين من عرض الاجرة الاساسية، وحصة الاحتياطي، وتوقعات مكافاة PDP/PoTR
   وتخطيط effacement ثنائي الابعاد بجانب ticket de stockage قبل الالتزام بالاموال.

## تحديثات التخزين/السجل

- Utilisez `sorafs_manifest` pour `DaManifestV1` pour l'analyse syntaxique.
- Le flux est utilisé pour `da.pin_intent` avec le hachage
  manifeste + identifiant du ticket.
- تحديث خطوط الملاحظة لمتابعة زمن ingest, وthroughput التجزئة، وباك لوج
  التكرار، وعدد الاخفاقات.

## استراتيجية الاختبارات- اختبارات وحدة للتحقق من schéma, وفحوصات التوقيع، وكشف التكرار.
- اختبارات golden لتاكيد ترميز Norito pour `DaIngestRequest` وmanifeste وreçu.
- harnais تكامل يشغل SoraFS + registre وهمي، ويتحقق من تدفقات chunk + pin.
- اختبارات خصائص تغطي ملفات effacement وتركيبات الاحتفاظ العشوائية.
- Fuzzing pour Norito pour les métadonnées.

## Utilisation de la CLI et du SDK (DA-8)- `iroha app da submit` (avec CLI) pour le constructeur/éditeur ingest
  Vous avez besoin de blobs pour le bundle Taikai. تعيش
  Description du produit `crates/iroha_cli/src/commands/da.rs:1`
  effacement/rétention وملفات métadonnées/manifeste اختيارية قبل توقيع
  `DaIngestRequest` Description de la CLI. تحفظ التشغيلات الناجحة
  `da_request.{norito,json}` et `da_receipt.{norito,json}` Détails
  `artifacts/da/submission_<timestamp>/` (remplacer par `--artifact-dir`) par
  تسجل artefacts الاصدار bytes Norito الدقيقة المستخدمة اثناء ingérer.
- يستخدم الامر افتراضيا `client_blob_id = blake3(payload)` pour les remplacements
  عبر `--client-blob-id`, ويحترم خرائط métadonnées JSON (`--metadata-json`) et
  manifestes الجاهزة (`--manifest`) et `--no-submit` hors ligne مع
  `--endpoint` pour Torii pour Torii. يطبع reçu JSON sur stdout اضافة ى
  Il s'agit d'un outil "submit_blob" pour DA-8.
  Utilisez le SDK.
- `iroha app da get` يضيف alias موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى manifeste des artefacts + plan en morceaux
  (`--manifest`, `--plan`, `--manifest-id`) **او** Télécharger le ticket de stockage avec Torii
  Voir `--storage-ticket`. عند استخدام مسار ticket, تقوم CLI بجلب manifest من
  `/v1/da/manifests/<ticket>`, et le lien vers `artifacts/da/fetch_<timestamp>/`
  (remplacer par `--manifest-cache-dir`) et hachage par blob par `--manifest-id`
  Vous avez utilisé Orchestrator pour `--gateway-provider` المعطاة. تبقى جميع boutonsالمتقدمة من جالب SoraFS كما هي (enveloppes manifestes, تسميات العميل، garde
  caches, overrides نقل مجهول, تصدير scoreboard, ومسارات `--output`), ويمكن
  Comment utiliser le point de terminaison du manifeste `--manifest-endpoint` ou Torii
  تعيش فحوصات من النهاية للنهاية ضمن مساحة `da` دون تكرار منطق
  orchestrateur.
- `iroha app da get-blob` manifeste les manifestes en question par rapport à Torii.
  `GET /v1/da/manifests/{storage_ticket}`. يكتب الامر
  `manifest_{ticket}.norito` et `manifest_{ticket}.json` et `chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحده المستخدم) مع
  طباعة امر `iroha app da get` الدقيق (بما في ذلك `--manifest-id`) المطلوب لجلب
  orchestrateur. هذا يبقي المشغلين بعيدا عن ادلة manifeste spool et ان
  fetcher يستخدم دائما artefacts الموقعة الصادرة عن Torii. يعكس عميل Torii
  Dans JavaScript, il s'agit de `ToriiClient.getDaManifest(storageTicketHex)`,
  et les octets Norito pour le manifeste JSON et le plan de morceaux pour les appelants dans le SDK
  Vous avez besoin d'orchestrator pour utiliser la CLI. Utiliser le SDK Swift pour plus de détails
  الاسطح (`ToriiClient.getDaManifestBundle(...)` مع
  `fetchDaPayloadViaGateway(...)`), comme orchestrateur SoraFS
  La fonction iOS est compatible avec les manifestes et récupère les fichiers .
  والتقاط الادلة دون استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]- `iroha app da rent-quote` يحسب rent حتمي وتفصيل الحوافز لحجم تخزين ونافذة احتفاظ
  مقدمة. Fichier `DaRentPolicyV1` (JSON et octets Norito) et fichiers
  Vous pouvez utiliser JSON (`gib`, `months`, pour des raisons de sécurité)
  وحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة في
  محاضر الحوكمة دون نصوص مخصصة. كما يصدر الامر ملخصا من سطر واحد
  `rent_quote ...` pour utiliser JSON pour créer des liens avec les utilisateurs
  تمارين الحوادث. à propos de `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  مع `--policy-label "governance ticket #..."` لحفظ artefacts منسقة تشير الى
  تصويت السياسة او حزمة التهيئة؛ تقوم CLI بقص الوسم المخصص وترفض السلاسل الفارغة
  حتى تبقى قيم `policy_source` قابلة للتنفيذ في لوحات خزينة. راجع
  `crates/iroha_cli/src/commands/da.rs` pour la sécurité et
  `docs/source/da/rent_policy.md` لمخطط السياسة.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ ticket de stockage, ينزل حزمة
  manifest القياسية، يشغل orchestrator متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` Tableau de bord et tableau de bord
  تحت `artifacts/da/prove_availability_<timestamp>/`, ويستدعي مباشرة مساعد PoR
  الحالي (`iroha app da prove`) باستخدام octets المحملة. يستطيع المشغلون ضبط boutons
  orchestrator (`--max-peers`, `--scoreboard-out`, remplace le manifeste) et
  sampler pour (`--sample-count`, `--leaf-index`, `--sample-seed`) pour tousامر واحد artefacts المطلوبة لتدقيقات DA-5/DA-9: نسخة من الحمولة، دليل
  tableau de bord, ainsi que JSON.