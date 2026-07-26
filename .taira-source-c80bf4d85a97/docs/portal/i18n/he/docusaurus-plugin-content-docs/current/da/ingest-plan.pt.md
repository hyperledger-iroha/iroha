---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Espelha `docs/source/da/ingest_plan.md`. Mantenha as duas versoes em
:::

# Plano de ingestao de זמינות נתונים da Sora Nexus

_Redigido: 2026-02-20 - תשובות: Core Protocol WG / Storage Team / DA WG_

O workstream DA-2 estende Torii com uma API de ingestao de blobs que emite
metadados Norito e semeia a replicacao SoraFS. Este documento captura o esquema
proposto, a superficie de API e o fluxo de validacao para que a implementacao
avance sem bloquear em simulacoes pendentes (seguimentos DA-1). Todos OS
פורמטים של מטען DEVEM משתמש קודקים Norito; Nao Sao Permitidos Fallbacks
serde/JSON.

## אובייקטיביות

- Aceitar blobs grandes (segmentos Taikai, cars sidecars de lane, artefatos de
  governanca) de forma deterministica דרך Torii.
- Produzir מציג Norito canonicos que descrevam o blob, parametros de codec,
  perfil de erasure e politica de retencao.
- Persistir metadata de chunks ללא אחסון חם de SoraFS e enfilirar jobs de
  העתק.
- כוונות פומביות של סיכה + תגיות פוליטיקה ללא רישום SoraFS e observadores
  דה גוברננקה.
- קבלות Expor de admissao para que os clientes recuperem prova deterministica
  de publicacao.

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

O מטען e um `DaIngestRequest` codificado em Norito. כתשובה
`application/norito+v1` e retornam `DaIngestReceipt`.

| רפוסטה | Significado |
| --- | --- |
| 202 מקובל | Blob enfileirado para chunking/replicacao; רטרנדו קבלה. |
| 400 בקשה רעה | Violacao de esquema/tamanho (veja validacoes). |
| 401 לא מורשה | Token API ausente/invalido. |
| 409 קונפליקט | Duplicado `client_blob_id` com מטא נתונים שונים. |
| 413 מטען גדול מדי | חריגה או מוגבלת תצורה דה טמנהו לעשות כתם. |
| 429 יותר מדי בקשות | מגבלת שיעור אטינגידו. |
| 500 שגיאה פנימית | Falha inesperada (לוג + התראה). |

## Esquema Norito proposto

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

> Nota de implementacao: as representacoes canonicas em Rust para esses payloads
> agora vivem em `iroha_data_model::da::types`, עטיפת בקשה/קבלה
> em `iroha_data_model::da::ingest` e a estrutura de manifest em
> `iroha_data_model::da::manifest`.

O campo `compression` informa como os callers prepararam o last pay. Torii aceita
`identity`, `gzip`, `deflate`, ו-`zstd`, descomprimindo os bytes antes de hash,
chunking e verificacao de manifests opcionais.

### רשימת תאימות1. בדוק את הכותרת Norito כדי לתאם את `DaIngestRequest`.
2. Falhar se `total_size` diferir do tamanho canonico do payload (descomprimido)
   ou exceder o maximo configurado.
3. Forcar alinhamento de `chunk_size` (potencia de dois, <= 2 MiB).
4. Garantir `data_shards + parity_shards` <= שוויון עולמי מקסימלי >= 2.
5. `retention_policy.required_replica_count` deve respeitar a baseline de
   governanca.
6. Verificacao de assinatura contra hash canonico (excluindo o campo signature).
7. Rejeitar `client_blob_id` duplicado, exceto se o hash doload payload e a metadata
   forem identicos.
8. Quando `norito_manifest` עבור fornecido, סכימה אימות + hash coincide com
   o manifest recalculado apos o chunking; caso contrario o node gera o manifest
   או ארמזנה.
9. Forcar a politica de replicacao configurada: Torii reescreve o
   `RetentionPolicy` enviado com `torii.da_ingest.replication_policy` (veja
   `replication-policy.md`) e rejeita manifests preconstruidos cuja metadata de
   retencao nao coincide com o perfil imposto.

### Fluxo de chunking e replicao

1. Fatiar o payload em `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nova) capturando compromissos de
   chunk (role/group_id), layout de erasure (contagens de paridade de linhas e)
   colunas mais `ipa_commitment`), politica de retencao e metadata.
3. הבתים של מערכת ההפעלה מתבטאים בקנוניקו
   `config.da_ingest.manifest_store_dir` (Torii escreve arquivos
   `manifest.encoded` por lane/epoch/sequence/ticket/printed finger) para que a
   orquestracao SoraFS os ingira e vincule o כרטיס אחסון aos dados
   persistidos.
4. כוונות פרסום באמצעות `sorafs_car::PinIntent` com tag de governanca e
   פוליטיקה.
5. Emitir evento Norito `DaIngestPublished` למען התצפיתנים
   (clientes leves, governanca, analitica).
6. Retornar `DaIngestReceipt` ao מתקשר (assinado pela chave de servico DA de
   Torii) e emitir o header `Sora-PDP-Commitment` para que os SDKs capturem o
   מחויבות codificado de imediato. אגורה קבלה כוללת `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitindo que os remetentes
   exibam a renda base, reserva, expectativas de bonus PDP/PoTR e o layout de
   מחיקה 2D ao lado do do כרטיס אחסון antes de comprometer fundos.

## Atualizacoes de storage/registry

- Estender `sorafs_manifest` com `DaManifestV1`, ניתוח habilitando
  דטרמיניסטי.
- הוספת זרם רישום חדש `da.pin_intent` בגרסה מטעמה
  referenciando hash de manifest + מזהה כרטיס.
- Atualizar pipelines de observabilidade para rastrear latencia de ingestao,
  תפוקה של chunking, backlog de replicacao ו contagens de falhas.

## אסטרטגיה של אשכים- בדיקות unitarios para validacao de schema, checks de assinatura, deteccao de
  כפילויות.
- בודק קידוד זהב verificando Norito de `DaIngestRequest`, manifest e
  קבלה.
- רתום de integracao subindo mock SoraFS + רישום, validando fluxos de
  נתח + סיכה.
- Tests de propriedades cobrindo perfis de erasure e combinacoes de retencao
  aleatorias.
- Fuzzing de payloads Norito עבור מגן נגד מטא-נתונים.

## Tooling de CLI & SDK (DA-8)- `iroha app da submit` (נובו נקודת כניסה CLI) agora envolve o Builder/Pisher de
  ingestao compartilhado para que operadores possam ingerir blobs arbitrarios
  צרור fora do fluxo de Taikai. O comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` e consome umloadload, perfil de
  erasure/retencao e arquivos opcionais de metadata/manifest antes de assinar o
  `DaIngestRequest` canonico com a chave de config da CLI. Execucoes bem-sucedidas
  persistem `da_request.{norito,json}` e `da_receipt.{norito,json}` בכי
  `artifacts/da/submission_<timestamp>/` (עקיפה דרך `--artifact-dir`)
  os artefatos de release registrem os bytes Norito exatos usados durante a
  ingestao.
- O comando usa por padrao `client_blob_id = blake3(payload)` mas aceita
  עוקפים דרך `--client-blob-id`, מפות JSON de metadata (`--metadata-json`)
  e manifests pre-gerados (`--manifest`), e suporta `--no-submit` para preparacao
  לא מקוון mais `--endpoint` עבור מארח Torii התאמה אישית. O קבלה JSON e
  impresso no stdout alem de ser escrito no disco, fechando o requisito de
  כלי "submit_blob" da DA-8 e destravando o trabalho de paridade do SDK.
- `iroha app da get` adiciona um alias focado em DA para o orquestrador multi-source
  que ja alimenta `iroha app sorafs fetch`. Operadores podem apontar para artefatos
  de manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou**
  passar um כרטיס אחסון Torii דרך `--storage-ticket`. Quando o caminho do
  כרטיס e usado, a CLI baixa או manifest de `/v1/da/manifests/<ticket>`,
  מתמיד או צרור יפח `artifacts/da/fetch_<timestamp>/` (עקוף com
  `--manifest-cache-dir`), deriva o hash do blob para `--manifest-id`, e entao
  executa o orquestrador com a list `--gateway-provider` fornecida. Todos OS
  ידיות avancados do fetcher SoraFS permanecem intactos (מעטפות גלויות,
  תוויות דה לקוחות, מטמוני שמירה, עוקפים את ה-transporte anonimo, יצוא דה
  לוח תוצאות נתיבים `--output`), e o endpoint de manifest pode ser sobrescrito
  דרך `--manifest-endpoint` עבור מארח Torii בדיקות אישיות, בדיקות מערכת הפעלה
  זמינות מקצה לקצה.
  logica do orquestrador.
- `iroha app da get-blob` baixa manifests canonicos direto de Torii via
  `GET /v1/da/manifests/{storage_ticket}`. הו אסקרייב קומנדו
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e
  `chunk_plan_{ticket}.json` בכי `artifacts/da/fetch_<timestamp>/` (או אממ
  `--output-dir` fornecido pelo usuario) enquanto imprime o comando exato de
  `iroha app da get` (כולל `--manifest-id`) הכרחי עבור אחזור לעשות
  orquestrador. Isso mantem operadores fora dos diretorios spool de manifests e
  garante que o fetcher semper use os artefatos assinados emitidos por Torii. O
  cliente Torii JavaScript espelha o mesmo fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`, retornando os bytes Norito
  decodificados, manifest JSON ו-chunk plan עבור מתקשרים להזמנת SDK
  sessoes de orquestrador sem usar a CLI. O SDK Swift אגורה אקספו כממסמות
  superficies (`ToriiClient.getDaManifestBundle(...)` mais
  `fetchDaPayloadViaGateway(...)`), חבילות canalizando para o wrapper nativo doorquestrador SoraFS עבור לקוחות iOS possam baixar manifests, executar
  מביא ריבוי מקורות ותפיסת מקורות המאפשרים CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rentas deterministicas e o detalhamento de
  incentivos para um tamanho de storage e Janela de retencao fornecidos. הו עוזר
  צרוך `DaRentPolicyV1` ativa (JSON ou bytes Norito) או o embutido ברירת מחדל,
  valida a politica e imprime um resumo JSON (`gib`, `months`, metadata de
  politica e campos de `DaRentQuote`) para que auditores citem חיובים XOR exatas
  em atas de governanca sem scripts ad-hoc. O comando tambem emite um resumo em
  uma linha `rent_quote ...` antes do payload JSON para manter logs de console
  legiveis durante drills de incidente. Emparelhe
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` עבור מתמשך artefatos bonitos que
  citem o voto ou bundle de config exatos; התאמה אישית של CLI או תווית e
  recusa strings vazias para que valores `policy_source` permanecam acionavis
  nos dashboards de tesouraria. Veja
  `crates/iroha_cli/src/commands/da.rs` עבור subcomando e
  `docs/source/da/rent_policy.md` עבור סכימה פוליטית.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: recebe um כרטיס אחסון,
  חבילה או צרור קנוני דה מניפסט, ביצוע או אופקסטרדור ריבוי מקורות
  (`iroha app sorafs fetch`) בניגוד לרשימה `--gateway-provider` fornecida, מתמיד
  o מטען מטען + לוח תוצאות בכי `artifacts/da/prove_availability_<timestamp>/`,
  e invoca imediatamente o helper PoR existente (`iroha app da prove`) usando os
  בתים baixados. כפתורי ה-Operadores podem ajustar os do orquestrador
  (`--max-peers`, `--scoreboard-out`, עוקף את נקודת הקצה של המניפסט) e o
  sampler de proof (`--sample-count`, `--leaf-index`, `--sample-seed`) enquanto um
  unico comando produz os artefatos esperados por auditorias DA-5/DA-9: copia do
  מטען, הוכחות ללוח התוצאות וקורות חיים של בדיקת JSON.