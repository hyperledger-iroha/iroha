---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
É `docs/source/da/ingest_plan.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# Você ingest لتوفر البيانات em Sora Nexus

_Relatório: 20/02/2026 - Trabalho: Core Protocol WG / Storage Team / DA WG_

Use o DA-2 Torii para ingerir blobs ou ingerir blobs Norito
Verifique o SoraFS. Esta é a API e a API que você usa
يتقدم التنفيذ دون انتظار محاكاة DA-1 المتبقية. يجب ان تستخدم جميع تنسيقات
Nome do produto Norito; Não há substituto para serde/JSON.

## الاهداف

- قبول blobs كبيرة (قطاعات Taikai, sidecars للحارات, وادوات حوكمة) بشكل حتمي
  Modelo Torii.
- Manifestos Norito معيارية تصف الـ blob e codec e apagamento
  وسياسة الاحتفاظ.
- حفظ بيانات chunks الوصفية في تخزين SoraFS الساخن وضع مهام التكرار في
  الطابور.
- Não pin + وسوم السياسة في سجل SoraFS ومراقبي حوكمة.
- اتاحة recibos القبول كي يستعيد العملاء دليلا حتميا على النشر.

## API de API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

O código `DaIngestRequest` é compatível com Norito. تستخدم الاستجابات
`application/norito+v1` e `DaIngestReceipt`.

| الاستجابة | المعنى |
| --- | --- |
| 202 Aceito | تم وضع الـ blob في طابور التجزئة/التكرار؛ Eu recebi o recibo. |
| 400 Solicitação incorreta | Use esquema/الحجم (انظر فحوصات التحقق). |
| 401 Não autorizado | رمز API مفقود/غير صالح. |
| 409 Conflito | A chave `client_blob_id` pode ser removida e removida. |
| 413 Carga útil muito grande | Não há nenhum blob no lugar. |
| 429 Muitas solicitações | Não há limite de taxa. |
| 500 Erro interno | فشل غير متوقع (log + تنبيه). |

## مخطط Norito Nome de usuário

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
> `iroha_data_model::da::types`, com invólucros para /recibo aqui
> `iroha_data_model::da::ingest` é o manifesto em `iroha_data_model::da::manifest`.

O `compression` é o mais importante. Torii `identity` e `gzip`
و`deflate` و`zstd` مع فك الضغط قبل hashing والتجزئة والتحقق من manifestos
الاختيارية.

### قائمة تحقق التحقق

1. Coloque o Norito no lugar do `DaIngestRequest`.
2. Use o `total_size` para obter informações sobre o produto (بعد فك الضغط) e
   يتجاوز الحد الاقصى المكون.
3. A opção `chunk_size` (base de dados, <= 2 MiB).
4. ضمان `data_shards + parity_shards` <= الحد الاقصى العالمي e paridade >= 2.
5. Use o `retention_policy.required_replica_count` para removê-lo.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع).
7. رفض `client_blob_id` مكرر ما لم تكن قيمة هاش الحمولة والبيانات الوصفية
   متطابقة.
8. Use `norito_manifest`, usando o esquema + hash para o manifesto.
   حسابه بعد التجزئة؛ والا يقوم العقدة بتوليد manifesto وتخزينه.
9. تطبيق سياسة التكرار المكونة: يعيد Torii ou `RetentionPolicy` المرسل عبر
   `torii.da_ingest.replication_policy` (راجع `replication-policy.md`) e
   manifests المجهزة مسبقا اذا لم تطابق بيانات الاحتفاظ الملف المفروض.

### تدفق التجزئة والتكرار

1. Insira o `chunk_size`, e BLAKE3 para chunk + Merkle.
2. Use Norito `DaManifestV1` (struct جديدة) para definir o pedaço
   (role/group_id), e apagamento (اعداد تكافؤ الصفوف والاعمدة مع
   `ipa_commitment`).
3. Os bytes do manifesto são `config.da_ingest.manifest_store_dir`
   (Torii ou `manifest.encoded` por faixa/época/sequência/bilhete/impressão digital)
   Você pode usar o SoraFS para obter um bilhete de armazenamento e um bilhete de armazenamento.
4. Coloque o pino `sorafs_car::PinIntent` no lugar e no lugar certo.
5. Use Norito `DaIngestPublished` para configurar o software (عملاء خفيفين, الحوكمة,
   التحليلات).
6. Instale `DaIngestReceipt` para o computador (que é o mesmo do Torii DA) e instale o dispositivo
   `Sora-PDP-Commitment` é um SDK de software livre. يتضمن الـ recibo
   Nome `rent_quote` (Norito `DaRentQuote`) e `stripe_layout`
   Você pode usar o PDP/PoTR para obter informações sobre o PDP/PoTR.
   وتخطيط apagamento ثنائي الابعاد بجانب bilhete de armazenamento قبل الالتزام بالاموال.

## تحديثات التخزين/السجل

- Use `sorafs_manifest` ou `DaManifestV1` para analisar o código.
- اضافة stream جديد للسجل `da.pin_intent` مع حمولة ذات اصدار تشير الى hash
  manifesto + ID do ticket.
- تحديث خطوط الملاحظة لمتابعة زمن ingest, وthroughput التجزئة, وباك لوج
  التكرار, وعدد الاخفاقات.

## استراتيجية الاختبارات- اختبارات وحدة للتحقق من esquema, وفحوصات التوقيع, وكشف التكرار.
- Ouro dourado para Norito para `DaIngestRequest` e manifesto e recibo.
- chicote تكامل يشغل SoraFS + registro وهمي, ويتحقق من تدفقات chunk + pin.
- اختبارات خصائص تغطي ملفات apagamento e وتركيبات الاحتفاظ العشوائية.
- Fuzzing é Norito através de metadados.

## Usando CLI e SDK (DA-8)- `iroha app da submit` (não CLI) no construtor/editor ingest.
  حتى يتمكن المشغلون من ادخال blobs عشوائية خارج مسار Pacote Taikai. تعيش
  O nome de `crates/iroha_cli/src/commands/da.rs:1` é um dispositivo que pode ser usado e usado
  apagamento/retenção e metadados/manifesto
  `DaIngestRequest` é compatível com CLI. تحفظ التشغيلات الناجحة
  `da_request.{norito,json}` e `da_receipt.{norito,json}`
  `artifacts/da/submission_<timestamp>/` (substituir por `--artifact-dir`) por
  Gere artefatos como bytes Norito para ingerir.
- يستخدم الامر افتراضيا `client_blob_id = blake3(payload)` para substituir substituições
  Para `--client-blob-id`, você precisa usar metadados JSON (`--metadata-json`) e
  manifests الجاهزة (`--manifest`), ويدعم `--no-submit` للتحضير offline مع
  `--endpoint` é o nome Torii. Obtenha o recibo JSON do stdout
  Você pode usar o tooling "submit_blob" no DA-8 e usar o tooling "submit_blob" no DA-8.
  Baixe o SDK.
- `iroha app da get` يضيف alias موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل
  `iroha app sorafs fetch`. يمكن للمشغلين توجيهه الى artefatos manifesto + plano de bloco
  (`--manifest`, `--plan`, `--manifest-id`) **E** Bilhete de armazenamento de cartão de Torii
  Modelo `--storage-ticket`. عند استخدام مسار ticket, تقوم CLI بجلب manifest من
  `/v2/da/manifests/<ticket>`, e o código de barras `artifacts/da/fetch_<timestamp>/`
  (substituir `--manifest-cache-dir`), e usar hash no blob para `--manifest-id`,
  Use o orquestrador no `--gateway-provider`. Botões de تبقى جميع
  المتقدمة من جالب SoraFS كما هي (envelopes manifestos, تسميات العميل, guarda
  caches, overrides نقل مجهول, تصدير scoreboard, ومسارات `--output`), ويمكن
  O endpoint manifesto deve ser definido como `--manifest-endpoint` para Torii.
  Verifique a disponibilidade do site `da` e verifique a disponibilidade
  orquestrador.
- `iroha app da get-blob` يسحب manifests القياسية مباشرة من Torii عبر
  `GET /v2/da/manifests/{storage_ticket}`. يكتب الامر
  `manifest_{ticket}.norito` e `manifest_{ticket}.json` e `chunk_plan_{ticket}.json`
  Você pode usar `artifacts/da/fetch_<timestamp>/` (e `--output-dir`)
  Use o código `iroha app da get` (ou seja, `--manifest-id`)
  orquestrador. Você pode usar o spool de manifesto e o spool de manifesto
  fetcher يستخدم دائما artefatos الموقعة الصادرة عن Torii. Torii
  No JavaScript, o código é `ToriiClient.getDaManifest(storageTicketHex)`,
  Os bytes Norito são os bytes e o manifesto JSON e o plano de pedaços são os chamadores do SDK
  O orquestrador está conectado ao CLI. Baixe o SDK do Swift para o Android
  Nome (`ToriiClient.getDaManifestBundle(...)` mais
  `fetchDaPayloadViaGateway(...)`), موجها الحزم الى غلاف orquestrador SoraFS
  Você pode usar o iOS para manifestar e buscar متعددة المصادر
  والتقاط الادلة دون استدعاء CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` يحسب rent حتمي وتفصيل الحوافز لحجم تخزين ونافذة احتفاظ
  Não. Use o código `DaRentPolicyV1` (JSON e bytes Norito) e o código
  O arquivo JSON (`gib`, `months`, `months`, `months`, بيانات السياسة,
  وحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة في
  محاضر الحوكمة دون نصوص مخصصة. كما يصدر الامر ملخصا من سطر واحد
  `rent_quote ...` é um arquivo JSON que pode ser usado para criar um arquivo JSON
  تمارين الحوادث. قم بقرن `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  مع `--policy-label "governance ticket #..."` لحفظ artefatos منسقة تشير الى
  تصويت السياسة او حزمة التهيئة؛ Clique em CLI para acessar o site e usar o CLI
  Verifique se o `policy_source` está instalado no carro. راجع
  `crates/iroha_cli/src/commands/da.rs` para o cartão e
  `docs/source/da/rent_policy.md` não funciona.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ ticket de armazenamento, ينزل حزمة
  manifesto orquestrador متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` placar, placar + placar
  تحت `artifacts/da/prove_availability_<timestamp>/`, ويستدعي مباشرة مساعد PoR
  O valor (`iroha app da prove`) gera bytes de valor. يستطيع المشغلون ضبط botões
  orquestrador (`--max-peers`, `--scoreboard-out`, substitui o manifesto do idioma) e
  sampler للاثبات (`--sample-count`, `--leaf-index`, `--sample-seed`) بينما ينتج
  امر واحد artefatos المطلوبة لتدقيقات DA-5/DA-9: نسخة من الحمولة, دليل
  placar, e é criado em JSON.