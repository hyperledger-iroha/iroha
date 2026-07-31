---
lang: he
direction: rtl
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שים לב Fonte canonica
Espelha `docs/source/da/commitments_plan.md`. Mantenha as duas versoes em
:::

# Plano de compromissos de Data Availability da Sora Nexus (DA-3)

_Redigido: 2026-03-25 -- תשובות: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 estende o formato de bloco da Nexus עבור קאדה ליין לכלול רישום
deterministicos que descrevem os blobs aceitos pelo DA-2. Esta not captura as
estruturas de dados canonicas, os hooks do pipeline de blocos, as provas de
cliente leve e as superficies Torii/RPC que precisam chegar antes que
validadores possam confiar nos compromissos DA durante admissao ou checks de
governanca. Todos OS מטענים sao codificados em Norito; מודעה sem retired codec או JSON
hoc.

## אובייקטיביות

- Carregar compromissos por blob (שורש נתח + חשיש מניפסט + התחייבות KZG
  אופציונלי) dentro de cada bloco Nexus para que peers possam reconstruir o estado
  דה זמינות sem consultar אחסון פורה לעשות ספר חשבונות.
- Fornecer provas deterministics החברות עבור רמות לקוחות
  verifiquem que um manifest hash foi finalizado em um bloco.
- ייעוץ אקספורמציה Torii (`/v1/da/commitments/*`) והוכחות שמאפשרות ממסרים,
  SDKs e automacao de governanca auditar זמינות סם משוחזר cada bloco.
- Manter o envelope `SignedBlockWire` canonico ao enfiar as novas estruturas
  pelo header de metadata Norito e a derivacao do hash de bloco.

## פנורמה דה אסקופו

1. **Adicoes ao data model** em `iroha_data_model::da::commitment` mais alteracoes
   de header de bloco em `iroha_data_model::block`.
2. **Hooks do executor** para que `iroha_core` ingeste קבלות DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que o WSV responda consultas de compromissos
   rapidamente (`iroha_core/src/wsv/mod.rs`).
4. **Adicoes RPC em Torii** עבור נקודות קצה של רשימה/consulta/prova sob
   `/v1/da/commitments`.
5. **מבחנים אינטגראו + מתקנים** validando o פריסת תיל או fluxo de proof
   em `integration_tests/tests/da/commitments.rs`.

## 1. מודל הנתונים של Adicoes ao

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // V1 lane policy (merkle_sha256 only)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- **V1 protocol invariant:** V1 contains only the `merkle_sha256` proof scheme. It has no KZG variant, commitment field, setup, generation, or verification path; unsupported configuration is rejected before node startup.
- A future KZG design requires a separately reviewed protocol version and an explicit wire-layout change. Hash expansion is not an elliptic-curve commitment.
- The verification endpoint checks commitment consistency against a caller-authenticated canonical block header and committed policy sidecar; it does not independently authenticate block signatures or finality.

### 1.2 Extensao do header de bloco

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

O hash do bundle entra tanto no hash do bloco quanto na metadata de
`SignedBlockWire`. Quando um bloco nao carrega dados DA, o campo fica `None` paraNota de implementacao: `BlockPayload` e o `BlockBuilder` אגורה שקופה
מגדירי/מג'רים של expoem `da_commitments` (ver `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), entao מארח Podem anexar um bundle
preconstruido antes de selar um bloco. Todos os helpers deixam o campo em `None`
ate que Torii encadeie bundles reais.

### 1.3 קידוד תיל

- `SignedBlockWire::canonical_wire()` אדיקונה או כותרת Norito para
  `DaCommitmentBundle` מיידית אפוס לרשימה קיימת. O
  byte de versao e `0x01`.
- `SignedBlockWire::decode_wire()` rejeita חבילות cujo `version` seja
  desconhecido, alinhado a politica Norito תיאור של `norito.md`.
- Atualizacoes de derivacao de hash existem apenas em `block::Hasher`; לקוחות
  leves que decodificam o wire format existente ganham o novo campo
  automaticamente porque o header Norito anuncia sua presenca.

## 2. Fluxo de producao de blocos

1. A ingestao DA de Torii finaliza um `DaIngestReceipt` e o publica na fila
   פנימי (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coleta todos os קבלות cujo `lane_id` corresponde ao bloco
   em construcao, deduplicando por `(lane_id, client_blob_id, manifest_hash)`.
3. Pouco antes de selar, o Builder ordena os compromissos por `(lane_id, epoch,
   sequence)` para manter o hash deterministico, codifica o bundle com o codec
   Norito, e atualiza `da_commitments_hash`.
4. O bundle completo e armazenado no WSV e emitido junto com o bloco dentro de
   `SignedBlockWire`.

Se a criacao do bloco falhar, os קבלות permanecem na fila para que a proxima

לכידת tentativa os; o registra Builder o ultimo `sequence` כולל ליין
para evitar ataques de replay.

## 3. Superficie RPC וייעוץ

Torii נקודות קצה אקספו:

| רוטה | Metodo | מטען | Notas |
|------|--------|--------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (מסננת טווח לפי נתיב/תקופה/רצף, paginacao) | Retorna `DaCommitmentPage` כולל סה"כ, פשרות ו-hash de bloco. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (מסלול + מניפסט hash ou tupla `(epoch, sequence)`). | תגובה com `DaCommitmentProof` (תקליט + קמינהו Merkle + hash de bloco). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | עוזר חסר אזרחות que refaz o calculo do hash de bloco e valida a inclusao; usado por SDKs que nao podem linkar direto em `iroha_crypto`. |

Todos OS מטענים vivem sob `iroha_data_model::da::commitment`. Os נתבים de
Torii מטפלי montam os ao lado dos endpoints de ingestao DA existentes para
שימוש חוזר בפוליטיקה של טוקן/mTLS.

## 4. Provas de inclusao e clientes levels- O Produtor de blocos constroi uma arvore Merkle binaria sobre a list
  serializada de `DaCommitmentRecord`. A raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empacota o record alvo mais um vetor de
  `(sibling_hash, position)` עבור בדיקה מחדש של בנייה מחדש. כמו
  provas tambem incluem o hash do bloco e o header assinado para que clientes
  levels validem סופיות.
- Helpers de CLI (`iroha_cli app da prove-commitment`) כולל
  בקש/אמת e expoem saidas Norito/hex עבור מפעילים.

## 5. אחסון e indexacao

O WSV armazena compromissos em uma column family dedicada com chave
`manifest_hash`. אינדקסים secundarios cobrem `(lane_id, epoch)` ה
`(lane_id, sequence)` בשביל להתייעץ עם חבילות שלמות. קאדה
להקליט ראסטריה א אלטורה דו בלוקו que o selou, permitindo que nos em catch-up
reconstruam o indice rapidamente a partir do block log.

## 6. Telemetria e Observabilidade

- `torii_da_commitments_total` אינקרמנטה quando um bloco sela ao menos um
  להקליט.
- `torii_da_commitment_queue_depth` חבילת קבלות aguardando rastreia (por
  נתיב).
- O לוח מחוונים Grafana `dashboards/grafana/da_commitments.json` visualiza a
  inclusao em blocos, profundidade da fila e throughput de provas para que os
  שערים דה שחרור da DA-3 possam auditar o comportamento.

## 7. אסטרטגיה דה אשכים

1. **Future KZG protocol** — KZG is not part of V1. A separately reviewed later protocol version must specify polynomial encoding, setup provenance, commitment/opening algorithms, consensus verification, deterministic test vectors, and a versioned wire-layout change. V1 has no enable toggle or reserved accepted value.
2. **פערים ברצף** - Permitimos lanes fora de ordem? O plano atual rejeita פערים
   salvo se a governanca ativar `allow_sequence_skips` עבור שידור חוזר של חירום.
3. **מטמון קליינט קל** - O time de SDK pediu um cache SQLite level para proofs;
   pendente em DA-8.

מגיב estas perguntas em PRs de implementacao move DA-3 de RASCUNHO (este
documento) para EM ANDAMENTO quando o trabalho de codigo comecar.