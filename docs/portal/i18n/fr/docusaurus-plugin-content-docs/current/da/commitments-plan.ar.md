---
lang: fr
direction: ltr
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
C'est `docs/source/da/commitments_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة تعهدات توفر البيانات pour Sora Nexus (DA-3)

_مسودة : 2026-03-25 -- المالكون : Core Protocol WG / Smart Contract Team / Storage Team_

Le DA-3 est utilisé pour Nexus pour la voie, les blobs et les blobs
Par DA-2. توضح هذه المذكرة هياكل البيانات القياسية، ووصلات خط انابيب الكتل،
وبرهان العملاء الخفيفين، واسطح Torii/RPC التي يجب ان تكتمل قبل ان يعتمد
المدققون على تعهدات DA اثناء فحوصات القبول او الحوكمة. جميع الحمولات مشفرة
Par Norito؛ Il est compatible avec SCALE et JSON.

## الاهداف

- Utilisez le blob (racine de morceau + hachage manifeste + outil KZG) ici
  كتلة Nexus لكي يتمكن النظراء من اعادة بناء حالة disponibilité بدون الرجوع
  الى تخزين خارج الدفتر.
- توفير براهين عضوية حتمية لكي يتحقق العملاء الخفيفون من ان manifeste hash تم
  تثبيته في كتلة محددة.
- Les modules Torii (`/v2/da/commitments/*`) et les relais et SDK
  وادوات الحوكمة بتدقيق disponibilité دون اعادة تشغيل كل كتلة.
- الحفاظ على ظرف `SignedBlockWire` القياسي عبر تمرير البنى الجديدة من خلال
  Utilisez le code Norito pour le hachage.

## نظرة عامة على النطاق1. **اضافات نموذج البيانات** في `iroha_data_model::da::commitment` مع تغييرات
   Il s'agit d'un `iroha_data_model::block`.
2. **Hooks للمنفذ** حتى يقوم `iroha_core` pour les reçus الخاصة بـ DA
   Torii (`crates/iroha_core/src/queue.rs` et `crates/iroha_core/src/queue.rs` et
   `crates/iroha_core/src/block.rs`).
3. **Persisting/indexes** حتى يتمكن WSV من اجابة استعلامات التعهدات بسرعة
   (`iroha_core/src/wsv/mod.rs`).
4. **اضافات RPC pour Torii**** pour list/query/prove comme `/v2/da/commitments`.
5. **اختبارات تكامل + luminaires** للتحقق من disposition des fils et preuve في
   `integration_tests/tests/da/commitments.rs`.

## 1. اضافات نموذج البيانات

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. Il s'agit d'une question de Merkle.
- `proof_scheme` pour les voies de circulation Lanes من نوع Merkle ترفض حمولات KZG,
  بينما voies `kzg_bls12_381` تتطلب تعهدات KZG غير صفرية. Torii حاليا ينتج
  تعهدات Merkle فقط ويرفض les voies على KZG.
- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. Il y a aussi Lanes Merkle et Merkle.
- `proof_digest` pour le DA-5 PDP/PoTR pour le système d'exploitation
  Il s'agit de blobs.

### 1.2 توسيع ترويسة الكتلة

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Vous pouvez utiliser le hachage pour le hachage et le hachage `SignedBlockWire`. عندما لاNom du produit : `BlockPayload` et `BlockBuilder` Nom du produit
setters/getters pour `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) pour les hôtes من ارفاق حزمة مسبقة
الانشاء قبل ختم الكتلة. جميع الدوال المساعدة تترك الحقل `None` ou Torii
حزم حقيقية.

### 1.3 fil de fil

- `SignedBlockWire::canonical_wire()` pour Norito pour
  `DaCommitmentBundle` مباشرة بعد قائمة المعاملات الحالية. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروفة، بما
  يتوافق مع سياسة Norito الموضحة في `norito.md`.
- Le hachage est utilisé pour `block::Hasher` ; العملاء الخفيفون الذين
  Format de fil pour le format filaire Norito
  تعلن وجوده.

## 2. تدفق انتاج الكتل

1. تنهي عملية ingest الخاصة by Torii DA ايصال `DaIngestReceipt` وتقوم بنشره
   على الطابور الداخلي (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. يجمع `PendingBlocks` pour les reçus et `lane_id` pour les reçus.
   مع ازالة التكرار حسب `(lane_id, client_blob_id, manifest_hash)`.
3. Comment construire un constructeur avec `(lane_id, epoch, sequence)`
   Comment utiliser le hachage et le hachage Norito,
   `da_commitments_hash`.
4. Vous devez contacter WSV pour obtenir le numéro `SignedBlockWire`.

اذا فشل انشاء الكتلة تبقى recettes في الطابور ليتم التقاطها في المحاولة
التالية؛ Le constructeur `sequence` est utilisé pour la relecture de la voie.## 3. Comment RPC fonctionne

Torii concerne les points de terminaison :

| المسار | الطريقة | الحمولة | ملاحظات |
|--------|---------|---------|---------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (pour la voie/l'époque/la séquence, avec la pagination) | Il s'agit d'un `DaCommitmentPage` contenant des éléments de hachage et de hachage. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (voie + hachage manifeste et tuple `(epoch, sequence)`). | يعيد `DaCommitmentProof` (enregistrement + مسار Merkle + hachage الكتلة). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد stateless يعيد حساب hash الكتلة ويتحقق من الاشتمال؛ Les SDK sont compatibles avec la version `iroha_crypto`. |

Il s'agit de `iroha_data_model::da::commitment`. يقوم Torii pour
les gestionnaires des points de terminaison ingèrent le jeton/mTLS.

## 4. براهين الاشتمال والعملاء الخفيفون

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  المسلسلة. الجذر يغذي `da_commitments_hash`.
- يحزم `DaCommitmentProof` السجل المستهدف مع متجه من `(sibling_hash, position)`
  لكي يعيد المدققون بناء الجذر. تتضمن البراهين ايضا hash الكتلة والترويسة
  الموقعة حتى يتمكن العملاء الخفيفون من التحقق من finalité.
- تساعد اوامر CLI (`iroha_cli app da prove-commitment`) على تنفيذ دورة طلب/تحقق
  La clé est Norito/hex للمشغلين.

## 5. التخزين والفهرسةLa famille de colonnes WSV est basée sur `manifest_hash`. تغطي
فهارس الثانوية `(lane_id, epoch)` و`(lane_id, sequence)` كي تتجنب الاستعلامات
مسح الحزم كاملة. يتتبع كل سجل ارتفاع الكتلة التي ختمته، مما يسمح للعقد في
مرحلة rattrapage باعادة بناء الفهرس بسرعة من سجل الكتل.

## 6. القياس والرصد

- `torii_da_commitments_total` يزيد عند ختم كتلة تحتوي على سجل واحد على الاقل.
- `torii_da_commitment_queue_depth` يتتبع reçus المنتظرة للتجميع (لكل voie).
- لوحة Grafana `dashboards/grafana/da_commitments.json` تعرض ادراج الكتل وعمق
  Le débit et le débit sont les mêmes que ceux du DA-3 à la hauteur.

## 7. استراتيجية الاختبارات

1. **اختبارات وحدات** لترميز/فك ترميز `DaCommitmentBundle` et تحديثات اشتقاق hash
   الكتلة.
2. **Luminaires dorés** تحت `fixtures/da/commitments/` تلتقط octets الحزمة
   القياسية وبراهين Merkle.
3. **اختبارات تكامل** تشغل مدققين اثنين، وتبتلع blobs تجريبية، وتتحقق من ان
   كلا العقدتين تتفقان على محتوى الحزمة واستجابات الاستعلام/البرهان.
4. **اختبارات عملاء خفيفين** pour `integration_tests/tests/da/commitments.rs`
   (Rust) تستدعي `/prove` وتتحقق من البرهان دون التحدث الى Torii.
5. **Smoke CLI** Voir `scripts/da/check_commitments.sh` pour les utilisateurs
   قابلة لاعادة الانتاج.

## 8. خطة الاطلاق| المرحلة | الوصف | معيار الخروج |
|---------|-------|--------------|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord` et Norito. | `cargo test -p iroha_data_model` ينجح مع luminaires جديدة. |
| P1 - Version Core/WSV | Il s'agit d'un générateur de blocs + et d'un gestionnaire RPC. | `cargo test -p iroha_core` et `integration_tests/tests/da/commitments.rs` sont des preuves groupées. |
| P2 - ادوات المشغلين | Les helpers للـ CLI et Grafana وتحديثات توثيق تحقق proof. | `iroha_cli app da prove-commitment` pour Devnet اللوحة تعرض بيانات حية. |
| P3 - بوابة الحوكمة | تفعيل مدقق الكتل الذي يفرض تعهدات DA على voies المحددة في `iroha_config::nexus`. | Statut et feuille de route pour DA-3. |

## اسئلة مفتوحة

1. **Par défaut KZG vs Merkle** - هل يجب تخطي تعهدات KZG للـ blobs الصغيرة لتقليل
   حجم الكتلة؟ Nom : `kzg_commitment` Nom du produit
   `iroha_config::da.enable_kzg`.
2. **Écarts de séquence** - هل نسمح بفجوات الترتيب؟ الخطة الحالية ترفض الفجوات الا
   اذا فعلت الحوكمة `allow_sequence_skips` لاعادة تشغيل طارئة.
3. **Cache client léger** - Utiliser le SDK pour SQLite et SQLite متابعة
   Utilisez DA-8.

الاجابة على هذه الاسئلة في PRs التنفيذ تنقل DA-3 من مسودة (هذه الوثيقة) الى
قيد العمل بمجرد بدء التنفيذ البرمجي.