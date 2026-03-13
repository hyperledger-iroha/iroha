---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
España `docs/source/da/commitments_plan.md`. Mantenha como dos versos em
:::

# Plano de compromisos de disponibilidad de datos de Sora Nexus (DA-3)

_Redigido: 2026-03-25 -- Responsaveis: Core Protocol WG / Equipo de Contrato Inteligente / Equipo de Almacenamiento_

DA-3 estende o formato de bloco da Nexus para que cada carril incorpore registros
determinísticos que descrevem os blobs aceitos pelo DA-2. Esta nota capturada como
estruturas de dados canónicas, os ganchos do pipe de blocos, como provas de
cliente leve e as superficies Torii/RPC que precisam chegar antes que
Los validadores pueden confiar en los compromisos DA durante la admisión o los cheques de
gobernancia. Todas las cargas útiles están codificadas en Norito; sin ESCALA o anuncio JSON
hoc.

## Objetivos- Carregar compromisos por blob (raíz de fragmento + hash de manifiesto + compromiso KZG
  opcional) dentro de cada bloque Nexus para que peers possam reconstruir o estado
  de disponibilidad sem consultar almacenamiento fora do libro mayor.
- Fornecer pruebas de membresía determinísticas para que clientes leves
  verifiquem que un hash manifest foi finalizado en un bloco.
- Export consultas Torii (`/v2/da/commitments/*`) e pruebas que permitanm a relés,
  Los SDK y el control automático de disponibilidad auditan la disponibilidad sin reproducir cada bloque.
- Manter o sobre `SignedBlockWire` canonico ao enfiar as novas estruturas
  El encabezado de metadatos Norito y una derivación del hash de bloque.

## Panorama de Escopo

1. **Adicoes ao data model** em `iroha_data_model::da::commitment` mais alteracoes
   de encabezado de bloque en `iroha_data_model::block`.
2. **Hooks do albacea** para que `iroha_core` ingeste recibos DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que o WSV responda consultas de compromisos
   rápidamente (`iroha_core/src/wsv/mod.rs`).
4. **Adicoes RPC em Torii** para endpoints de lista/consulta/prova sollozo
   `/v2/da/commitments`.
5. **Pruebas de integración + accesorios** validando el diseño del cable y el flujo de prueba
   en `integration_tests/tests/da/commitments.rs`.

## 1. Modelo de datos de Adicoes

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
```- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Cuando ausente, cai para probar Merkle apenas.
- `proof_scheme` deriva del catálogo de carriles; carriles Merkle rejeitam cargas útiles KZG
  enquanto carriles `kzg_bls12_381` exigem compromisos KZG nao cero. Torii actualmente
  so produz compromissos Merkle e rejeita lanes configuradas con KZG.
- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Quando ausente em lanes Merkle, cai para provas Merkle apenas.
- `proof_digest` antecipa a integracao DA-5 PDP/PoTR para que o mesmo record
  enumere el cronograma de muestreo usado para manter blobs vivos.

### 1.2 Ampliación del encabezado de bloque

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
`SignedBlockWire`. Quando um bloco nao carrega dados DA, o campo fica `None` para

Nota de implementación: `BlockPayload` e o `BlockBuilder` ágora transparente
Establecedores/captadores de expoem `da_commitments` (ver `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), estos hosts pueden anexar un paquete
preconstruido antes de selar um bloco. Todos los ayudantes deixam o campo em `None`
ate que Torii encadeie paquetes reales.

### 1.3 Codificación del cable- `SignedBlockWire::canonical_wire()` agrega el encabezado Norito para
  `DaCommitmentBundle` inmediatamente apos a lista de transacciones existentes. oh
  byte de versao e `0x01`.
- `SignedBlockWire::decode_wire()` rejeita manojos cujo `version` seja
  desconhecido, alinhado a politica Norito descrita en `norito.md`.
- Las actualizaciones de derivación de hash existen sólo en `block::Hasher`; clientes
  leves que decodificam o wire format existente ganham o novo campo
  automaticamente porque o header Norito anuncia su presencia.

## 2. Flujo de producción de bloques

1. A ingestao DA de Torii finaliza um `DaIngestReceipt` e o publica na fila
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coleta todos os recibos cujo `lane_id` corresponde ao bloco
   En construcción, deduplicando por `(lane_id, client_blob_id, manifest_hash)`.
3. Pouco antes de selar, o builder ordena os compromisos por `(lane_id, epoch,
   secuencia)` para mantener el hash determinístico, codificar el paquete con el códec
   Norito, y actualiza `da_commitments_hash`.
4. O paquete completo e armazenado no WSV e emitido junto com o bloco dentro de
   `SignedBlockWire`.

Se a criacao do bloco falhar, os recibos permanecenm na fila para que a proxima

tentativa de captura; o constructor registra o ultimo `sequence` incluido por carril
para evitar ataques de repetición.

## 3. Superficie RPC y consultas

Torii expone tres puntos finales:| Rota | Método | Carga útil | Notas |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de rango por carril/época/secuencia, página) | Retorna `DaCommitmentPage` con total, compromisos y hash de bloque. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto o tupla `(epoch, sequence)`). | Responde con `DaCommitmentProof` (record + caminho Merkle + hash de bloco). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que refaz o calculo do hash de bloco e valida a inclusao; Usado por SDK que no pueden vincularse directamente a `iroha_crypto`. |

Todas las cargas útiles viven en sollozo `iroha_data_model::da::commitment`. Los enrutadores de
Torii montamos los manejadores al lado de los puntos finales de ingesta DA existentes para
reutilizar políticas de token/mTLS.

## 4. Provas de inclusión y niveles de clientes

- El productor de blocos constroi uma arvore Merkle binaria sobre una lista
  serializada de `DaCommitmentRecord`. A raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empacota o record alvo mais um vetor de
  `(sibling_hash, position)` para que verificadores reconstruyan la raíz. como
  Provas tambem incluyen o hash do bloco y o header assinado para que clientes
  leves validem a finalidad.
- Helpers de CLI (`iroha_cli app da prove-commitment`) implica el ciclo de
  request/verify e expoem saydas Norito/hex para operadores.

## 5. Almacenamiento e indexaciónO WSV armazena compromissos em uma columna familiar dedicada com chave
`manifest_hash`. Indexes secundarios cobrem `(lane_id, epoch)` e
`(lane_id, sequence)` para que consultas eviten varrer paquetes completos. Cada
record rastreia a altura do bloco que o selou, permitindo que nos em catch-up
Reconstruimos el índice rápidamente a partir del registro de bloques.

## 6. Telemetría y observabilidad

- `torii_da_commitments_total` incrementa cuando un bloque sela ao menos um
  registro.
- `torii_da_commitment_queue_depth` rastreia recibos guardando paquete (por
  carril).
- O tablero Grafana `dashboards/grafana/da_commitments.json` visual a
  inclusao em blocos, profundidade da fila e throughput de provas para que os
  gates de release da DA-3 possam auditar o comportamento.

## 7. Estrategia de testículos

1. **Pruebas unitarios** para codificación/decodificación de `DaCommitmentBundle` e
   Actualizaciones de derivacao do hash de bloco.
2. **Accesorios dorados** sollozo `fixtures/da/commitments/` capturando bytes canónicos
   haz el paquete y prueba Merkle.
3. **Tests de integracao** con dos validadores, ingiriendo blobs de ejemplo e
   verificando que ambos os nos concordam no conteudo do bundle e nas respostas
   de consulta/prueba.
4. **Pruebas de nivel de cliente** en `integration_tests/tests/da/commitments.rs`
   (Rust) que chamam `/prove` y verificam a prova sem falar com Torii.
5. **Smoke CLI** con `scripts/da/check_commitments.sh` para herramientas de mantenimiento
   operadores reproduzivel.

## 8. Plano de implementación| Fase | Descripción | Criterio de dicha |
|------|-----------|-------------------|
| P0 - Fusionar el modelo de datos | Integrar `DaCommitmentRecord`, actualizaciones de encabezado de bloque y códecs Norito. | `cargo test -p iroha_data_model` verde con luminarias novas. |
| P1 - Núcleo de cableado/WSV | Encadear lógica de fila + block builder, persistir índices y exportar manejadores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pasan con afirmaciones de prueba de paquete. |
| P2 - Herramientas de operadores | Entregar ayudantes CLI, panel Grafana y actualizaciones de documentos de verificación de prueba. | `iroha_cli app da prove-commitment` funciona contra devnet; El tablero muestra datos en vivo. |
| P3 - Puerta de gobierno | Habilitar o validador de bloques que requieren compromisos DA nas líneas marcadas en `iroha_config::nexus`. | Entrada de estado + actualización de hoja de ruta marcam DA-3 como COMPLETADO. |

## Perguntas abiertas

1. **KZG vs Merkle defaults** - Devemos semper pular compromisos KZG em blobs
   ¿Pequeños para reducir el tamaño del bloque? Propuesta: manter `kzg_commitment`
   opcional y gatear vía `iroha_config::da.enable_kzg`.
2. **Brechos en la secuencia** - ¿Permitimos carriles para el orden? O plano actual rejeita lagunas
   salvo se a gobernador ativar `allow_sequence_skips` para repetición de emergencia.
3. **Caché de cliente ligero** - El tiempo del SDK solicita el nivel de caché SQLite para pruebas;
   colgante en DA-8.Responder estas perguntas em PRs de implementacao move DA-3 de RASCUNHO (este
documento) para EM ANDAMENTO quando o trabalho de codigo comecar.