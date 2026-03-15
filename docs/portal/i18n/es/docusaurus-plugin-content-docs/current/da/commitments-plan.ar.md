---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
Aquí `docs/source/da/commitments_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# خطة تعهدات توفر البيانات في Sora Nexus (DA-3)

_مسودة: 2026-03-25 -- المالكون: Grupo de Trabajo de Protocolo Central / Equipo de Contrato Inteligente / Equipo de Almacenamiento_

El DA-3 está conectado al Nexus y está conectado al carril de los blobs del carril.
Desde DA-2. توضح هذه المذكرة هياكل البيانات القياسية, ووصلات خط انابيب الكتل،
وبرهان العملاء الخفيفين، واسطح Torii/RPC التي يجب ان تكتمل قبل ان يعتمد
المدققون على تعهدات DA اثناء فحوصات القبول او الحوكمة. جميع الحمولات مشفرة
بـ Norito؛ Aquí ESCALA y JSON archivos.

## الاهداف

- حمل تعهدات لكل blob (raíz de fragmento + hash de manifiesto + تعهد KZG اختياري) داخل كل
  كتلة Nexus لكي يتمكن النظراء من اعادة بناء حالة disponibilidad بدون الرجوع
  الى تخزين خارج الدفتر.
- توفير براهين عضوية حتمية لكي يتحقق العملاء الخفيفون من ان manifiesto hash تم
  تثبيته في كتلة محددة.
- كشف استعلامات Torii (`/v2/da/commitments/*`) y براهين تسمح للـ relés y SDK
  وادوات الحوكمة بتدقيق disponibilidad دون اعادة تشغيل كل كتلة.
- الحفاظ على ظرف `SignedBlockWire` القياسي عبر تمرير البنى الجديدة من خلال
  ترويسة بيانات Norito الوصفية y hash الكتلة.

## نظرة عامة على النطاق1. **اضافات نموذج البيانات** في `iroha_data_model::da::commitment` مع تغييرات
   ترويسة الكتلة في `iroha_data_model::block`.
2. **Ganchos للمنفذ** حتى يقوم `iroha_core` بابتلاع recibos الخاصة بـ DA
   Fuente de alimentación Torii (`crates/iroha_core/src/queue.rs` y
   `crates/iroha_core/src/block.rs`).
3. **Persistentes/índices** حتى يتمكن WSV من اجابة استعلامات التعهدات بسرعة
   (`iroha_core/src/wsv/mod.rs`).
4. **Use RPC en Torii** para listar/consultar/probar datos `/v2/da/commitments`.
5. **اختبارات تكامل + accesorios** للتحقق من diseño de cables y prueba في
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
  `iroha_crypto::kzg`. عند غيابها نعود الى براهين Merkle فقط.
- `proof_scheme` مشتق من كتالوج lanes؛ carriles de Merkle ترفض حمولات KZG،
  بينما carriles `kzg_bls12_381` تتطلب تعهدات KZG غير صفرية. Torii حاليا ينتج
  تعهدات Merkle فقط ويرفض carriles المهيئة على KZG.
- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. عند غيابها في carriles Merkle نعود الى براهين Merkle فقط.
- `proof_digest` يمهد لتكامل DA-5 PDP/PoTR لكي يسرد السجل نفسه جدول اخذ
  العينات المستخدم للحفاظ على blobs.

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

El hash se utiliza en el hash y en el código `SignedBlockWire`. عندما لاNombre del producto: `BlockPayload` y `BlockBuilder` الشفاف يعرضان الان
establecedores / captadores en `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) لكي تتمكن hosts من ارفاق حزمة مسبقة
الانشاء قبل ختم الكتلة. Fuente de alimentación `None` y Torii
حزم حقيقية.

### 1.3 cable de alimentación

- `SignedBlockWire::canonical_wire()` يضيف ترويسة Norito لـ
  `DaCommitmentBundle` مباشرة بعد قائمة المعاملات الحالية. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروفة, بما
  Utilice el controlador Norito y el `norito.md`.
- تحديثات اشتقاق hash موجودة فقط في `block::Hasher`; العملاء الخفيفون الذين
  يفككون formato de cable الحالي يحصلون تلقائيا على الحقل الجديد لان ترويسة Norito
  تعلن وجوده.

## 2. تدفق انتاج الكتل

1. تنهي عملية ingest الخاصة بـ Torii DA ايصال `DaIngestReceipt` وتقوم بنشره
   على الطابور الداخلي (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. يجمع `PendingBlocks` كل recibos التي تطابق `lane_id` للكتلة قيد البناء،
   Para obtener más información, consulte `(lane_id, client_blob_id, manifest_hash)`.
3. قبل الختم مباشرة, يقوم constructor بفرز التعهدات حسب `(lane_id, epoch, sequence)`
   للحفاظ على hash حتمي، ويقوم بترميز الحزمة باستخدام Norito, ويحدث
   `da_commitments_hash`.
4. Presione el botón WSV y presione el botón `SignedBlockWire`.

اذا فشل انشاء الكتلة تبقى recibos في الطابور ليتم التقاطها في المحاولة
التالية؛ El constructor `sequence` tiene una repetición de carril.## 3. سطح RPC y الاستعلام

Torii Puntos finales de configuración:

| المسار | الطريقة | الحمولة | ملاحظات |
|--------|---------|---------|---------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (carril/época/secuencia, sin paginación) | El `DaCommitmentPage` contiene datos y hash. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto y tupla `(epoch, sequence)`). | Nombre `DaCommitmentProof` (registro + archivo Merkle + hash الكتلة). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد apátrida يعيد حساب hash الكتلة ويتحقق من الاشتمال؛ Los SDK están disponibles en `iroha_crypto`. |

Aquí está el mensaje `iroha_data_model::da::commitment`. يقوم Torii بتركيب
controladores de puntos finales que ingieren tokens/mTLS.

## 4. براهين الاشتمال والعملاء الخفيفون

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  المسلسلة. الجذر يغذي `da_commitments_hash`.
- يحزم `DaCommitmentProof` السجل المستهدف مع متجه من `(sibling_hash, position)`
  لكي يعيد المدققون بناء الجذر. تضمن البراهين ايضا hash الكتلة والترويسة
  الموقعة حتى يتمكن العملاء الخفيفون من التحقق من finalidad.
- تساعد اوامر CLI (`iroha_cli app da prove-commitment`) على تنفيذ دورة طلب/تحقق
  البراهين وتعرض مخارج Norito/hex للمشغلين.

## 5. التخزين والفهرسةيخزن WSV التعهدات في familia de columnas مخصصة بمفتاح `manifest_hash`. تغطي
الفهارس الثانوية `(lane_id, epoch)` و`(lane_id, sequence)` كي تجنب الاستعلامات
مسح الحزم كاملة. يتتبع كل سجل ارتفاع الكتلة التي ختمته، مما يسمح للعقد في
مرحلة ponerse al día باعادة بناء الفهرس بسرعة من سجل الكتل.

## 6. القياس والرصد

- `torii_da_commitments_total` يزيد عند ختم كتلة تحتوي على سجل واحد على الاقل.
- `torii_da_commitment_queue_depth` يتتبع recibos المنتظرة للتجميع (لكل carril).
- لوحة Grafana `dashboards/grafana/da_commitments.json` تعرض ادراج الكتل وعمق
  El rendimiento y el rendimiento de la unidad DA-3 son compatibles con la unidad.

## 7. استراتيجية الاختبارات

1. **اختبارات وحدات** لترميز/فك ترميز `DaCommitmentBundle` وتحديثات اشتقاق hash
   الكتلة.
2. **Accesorios dorados** تحت `fixtures/da/commitments/` تلتقط bytes الحزمة
   القياسية وبراهين Merkle.
3. **اختبارات تكامل** تشغل مدققين اثنين، وتبتلع blobs تجريبية، وتتحقق من ان
   كلا العقدتين تتفقان على محتوى الحزمة واستجابات الاستعلام/البرهان.
4. **اختبارات عملاء خفيفين** en `integration_tests/tests/da/commitments.rs`
   (Óxido) `/prove` y Torii.
5. **Smoke CLI** عبر `scripts/da/check_commitments.sh` لابقاء ادوات المشغلين
   قابلة لاعادة الانتاج.

## 8. خطة الاطلاق| المرحلة | الوصف | معيار الخروج |
|---------|-------|--------------|
| P0 - دمج نموذج البيانات | دمج `DaCommitmentRecord` y ​​تحديثات ترويسة الكتلة y Norito. | `cargo test -p iroha_data_model` ينجح مع accesorios جديدة. |
| P1 - توصيل Core/WSV | تمرير منطق الطابور + constructor de bloques, y controladores RPC. | `cargo test -p iroha_core` y `integration_tests/tests/da/commitments.rs` son una prueba de paquete. |
| P2 - ادوات المشغلين | Hay ayudantes en CLI y Grafana y pruebas de prueba. | `iroha_cli app da prove-commitment` يعمل على devnet؛ اللوحة تعرض بيانات حية. |
| P3 - بوابة الحوكمة | تفعيل مدقق الكتل الذي يفرض تعهدات DA على lanes المحددة في `iroha_config::nexus`. | تحديث estado y hoja de ruta يشيران الى اكتمال DA-3. |

## اسئلة مفتوحة

1. **Valores predeterminados de KZG vs Merkle** - هل يجب تخطي تعهدات KZG للـ blobs الصغيرة لتقليل
   حجم الكتلة؟ الاقتراح: جعل `kzg_commitment` اختياريا وتفعيله عبر
   `iroha_config::da.enable_kzg`.
2. **Brechos de secuencia** - هل نسمح بفجوات الترتيب؟ الخطة الحالية ترفض الفجوات الا
   Lea el código `allow_sequence_skips` para que funcione correctamente.
3. **Caché de cliente ligero** - Utilice el SDK de SQLite para acceder a él متابعة
   Utilice el DA-8.

الاجابة على هذه الاسئلة في PRs التنفيذ تنقل DA-3 من مسودة (هذه الوثيقة) الى
قيد العمل بمجرد بدء التنفيذ البرمجي.