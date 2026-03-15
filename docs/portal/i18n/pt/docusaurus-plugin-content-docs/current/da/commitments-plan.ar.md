---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
É `docs/source/da/commitments_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# Você pode usar o Sora Nexus (DA-3)

_مسودة: 2026-03-25 -- المالكون: Core Protocol WG / Smart Contract Team / Storage Team_

Use o DA-3 para Nexus para remover os blobs da pista.
Em DA-2. توضح هذه المذكرة هياكل البيانات القياسية, ووصلات خط انابيب الكتل,
A solução de problemas Torii/RPC não está disponível para download
المدققون على تعهدات DA اثناء فحوصات القبول او الحوكمة. جميع الحمولات مشفرة
بـ Norito; O SCALE e o JSON são usados.

## الاهداف

- حمل تعهدات لكل blob (chunk root + manifest hash + تعهد KZG اختياري) داخل كل
  كتلة Nexus é uma opção de disponibilidade sem disponibilidade
  Isso é feito por conta própria.
- توفير براهين عضوية حتمية لكي يتحقق العملاء الخفيفون من ان manifest hash تم
  تثبيته في كتلة محددة.
- كشف استعلامات Torii (`/v1/da/commitments/*`) e instalar relés e SDKs
  وادوات الحوكمة بتدقيق disponibilidade دون اعادة تشغيل كل كتلة.
- الحفاظ على ظرف `SignedBlockWire` القياسي عبر تمرير البنى الجديدة من خلال
  Você pode usar Norito como hash e hash.

## نظرة عامة على النطاق

1. **اضافات نموذج البيانات** em `iroha_data_model::da::commitment` de acordo com as instruções
   Resolva o problema em `iroha_data_model::block`.
2. **Hooks للمنفذ** حتى يقوم `iroha_core` بابتلاع recibos الخاصة بـ DA
   Nome do produto Torii (`crates/iroha_core/src/queue.rs` e
   `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** حتى يتمكن WSV من اجابة استعلامات التعهدات بسرعة
   (`iroha_core/src/wsv/mod.rs`).
4. **Recurso RPC em Torii** para listar/consultar/provar em `/v1/da/commitments`.
5. **Configurações + acessórios** para layout de fio e prova de
   `integration_tests/tests/da/commitments.rs`.

## 1. اضافات نموذج البيانات

###1.1`DaCommitmentRecord`

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
  `iroha_crypto::kzg`. Não há nada melhor do que Merkle.
- `proof_scheme` مشتق من كتالوج pistas; pistas de Merkle ترفض حمولات KZG,
  As pistas `kzg_bls12_381` devem ser instaladas no KZG. Torii Torii
  تعهدات Merkle فقط ويرفض lanes المهيئة على KZG.
- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`. Ele está em Lanes Merkle e não em Merkle.
- `proof_digest` يمهد لتكامل DA-5 PDP/PoTR لكي يسرد السجل نفسه جدول اخذ
  Você pode criar blobs.

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

Use hash para usar no hash `SignedBlockWire`. عندما لا

Nome de usuário: `BlockPayload` e `BlockBuilder` de acordo com o código-fonte
setters/getters para `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) para que os hosts sejam configurados com segurança
Isso é o que acontece. Você pode usar o `None` para obter o Torii
É isso.

### Fio de 1,3 fios

- `SignedBlockWire::canonical_wire()` يضيف ترويسة Norito para
  `DaCommitmentBundle` pode ser usado para remover o problema. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروفة, بما
  Você pode usar Norito como `norito.md`.
- تحديثات اشتقاق hash موجودة فقط في `block::Hasher`; العملاء الخفيفون الذين
  Formato de fio sem fio Norito
  É isso.

## 2. تدفق انتاج الكتل

1. Você deve ingerir o código Torii e `DaIngestReceipt` e usar o `DaIngestReceipt`
   Verifique o código de barras (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. Use `PendingBlocks` para recibos do `lane_id` para obter o valor do recibo.
   A solução é `(lane_id, client_blob_id, manifest_hash)`.
3. قبل الختم مباشرة, يقوم builder بفرز التعهدات حسب `(lane_id, epoch, sequence)`
   للحفاظ على hash حتمي, ويقوم بترميز الحزمة باستخدام Norito, ويحدث
   `da_commitments_hash`.
4. Verifique a configuração do WSV e use o `SignedBlockWire`.

Você pode obter recibos no final do mês.
التالية؛ O construtor do construtor `sequence` permite que a lane seja reproduzida.

## 3. Como configurar o RPC

Torii para os endpoints:

| المسار | الطريقة | الحمولة | Produtos |
|----|---------|---------|---------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (período de faixa/época/sequência, sem paginação) | Use `DaCommitmentPage` para obter informações de valor e hash. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto e tupla `(epoch, sequence)`). | يعيد `DaCommitmentProof` (registro + مسار Merkle + hash الكتلة). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | مساعد stateless يعيد حساب hash الكتلة ويتحقق من الاشتمال؛ Use SDKs para obter informações sobre `iroha_crypto`. |O código de segurança é `iroha_data_model::da::commitment`. Torii Torii
manipuladores de endpoints ingerem o token/mTLS.

## 4. براهين الاشتمال والعملاء الخفيفون

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  المسلسلة. O código é `da_commitments_hash`.
- Use `DaCommitmentProof` para obter informações sobre `(sibling_hash, position)`
  Não há nada que você possa fazer. تتضمن البراهين ايضا hash الكتلة والترويسة
  الموقعة حتى يتمكن العملاء الخفيفون com finalidade definitiva.
- Altere a CLI (`iroha_cli app da prove-commitment`) para obter uma interface de usuário/serviço
  O código-fonte é Norito/hex.

## 5. التخزين والفهرسة

A família de colunas do WSV é definida como `manifest_hash`. تغطي
O código `(lane_id, epoch)` e `(lane_id, sequence)` está disponível para download
مسح الحزم كاملة. يتتبع كل سجل ارتفاع الكتلة التي ختمته, مما يسمح للعقد في
مرحلة catch-up باعادة بناء الفهرس بسرعة من سجل الكتل.

## 6. القياس والرصد

- `torii_da_commitments_total` não pode ser usado para evitar problemas.
- `torii_da_commitment_queue_depth` يتتبع recibos da rua.
- A chave Grafana `dashboards/grafana/da_commitments.json` está instalada
  A taxa de transferência e a taxa de transferência do DA-3 são importantes para o DA-3.

## 7. استراتيجية الاختبارات

1. **اختبارات وحدات** لترميز/فك ترميز `DaCommitmentBundle` e وتحديثات اشتقاق hash
   sim.
2. **Fixtures golden** تحت `fixtures/da/commitments/` تلتقط bytes الحزمة
   O escritor e Merkle.
3. **اختبارات تكامل** تشغل مدققين اثنين, e blobs تجريبية, وتتحقق من ان
   كلا العقدتين تتفقان على محتوى الحزمة واستجابات الاستعلام/البرهان.
4. **Efetue o download do arquivo** em `integration_tests/tests/da/commitments.rs`
   (Rust) `/prove` é usado para substituir Torii.
5. **Smoke CLI** عبر `scripts/da/check_commitments.sh` لابقاء ادوات المشغلين
   قابلة لاعادة الانتاج.

## 8. خطة الاطلاق

| المرحلة | الوصف | معيار الخروج |
|--------|-------|-------------|
| P0 - دمج نموذج البيانات | Use `DaCommitmentRecord` e instale o Norito. | `cargo test -p iroha_data_model` ينجح مع fixtures جديدة. |
| P1 - Core/WSV | Ele usa o construtor de bloco +, o construtor de blocos e os manipuladores RPC. | `cargo test -p iroha_core` e `integration_tests/tests/da/commitments.rs` são uma prova de pacote. |
| P2 - ادوات المشغلين | Esses ajudantes são CLI e Grafana e fornecem provas. | `iroha_cli app da prove-commitment` é compatível com devnet; Você pode fazer isso sem problemas. |
| P3 - بوابة الحوكمة | Verifique se a faixa de rodagem está localizada em `iroha_config::nexus`. | تحديث status e roteiro يشيران الى اكتمال DA-3. |

## اسئلة مفتوحة

1. **Padrões KZG vs Merkle** - هل يجب تخطي تعهدات KZG للـ blobs الصغيرة لتقليل
   حجم الكتلة؟ Nome do produto: `kzg_commitment` sem fio e sem fio
   `iroha_config::da.enable_kzg`.
2. **Lacunas de sequência** - هل نسمح بفجوات الترتيب؟ الخطة الحالية ترفض الفجوات الا
   Use o `allow_sequence_skips` para evitar danos.
3. **Cache de cliente leve** - طلب فريق SDK تخزين SQLite خفيف للبراهين؛ متابعة
   É compatível com DA-8.

O objetivo do PRs é o DA-3 de acordo com o modelo (هذه الوثيقة)
Isso significa que você não pode fazer isso.