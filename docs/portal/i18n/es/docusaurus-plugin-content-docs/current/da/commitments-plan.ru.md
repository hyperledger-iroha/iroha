---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está escrita `docs/source/da/commitments_plan.md`. Держите обе версии
:::

# Plan de compromisos Disponibilidad de datos Sora Nexus (DA-3)

_Черновик: 2026-03-25 — Владельцы: Core Protocol WG / Equipo de contrato inteligente / Equipo de almacenamiento_

DA-3 расширяет формат блока Nexus так, чтобы каждая lane встраивала
детерминированные записи, описывающие blobs, принятые DA-2. En este documento
зафиксированы канонические структуры данных, хуки блокового пайплайна,
лайт-клиентские доказательства и поверхности Torii/RPC, которые должны появиться
Para esto, los validadores deben cumplir con los compromisos de DA antes de la admisión o
проверках управления. Все payloadы Norito-кодированы; sin ESCALA y JSON ad-hoc.

## Цели- Nuevos compromisos en blob (raíz de fragmento + hash de manifiesto + KZG opcional)
  compromiso) внутри каждого блока Nexus, чтобы пиры могли реконструировать
  состояние disponibilidad без обращения к almacenamiento fuera del libro mayor.
- Дать детерминированные pruebas de membresía, чтобы clientes ligeros могли проверить,
  что manifest hash finalizó el bloque consolidado.
- Exportar Torii запросы (`/v2/da/commitments/*`) y pruebas, позволяющие
  Relés, SDK y control de automatización para comprobar la disponibilidad según cada respuesta.
  bloques.
- Сохранить канонический `SignedBlockWire` sobre, пропуская новые структуры
  Aquí encontrará el encabezado de metadatos Norito y el hash del bloque de derivación.

## Обзор области работ

1. **Modelo de datos compartido** en `iroha_data_model::da::commitment` más personalización
   encabezado de bloque в `iroha_data_model::block`.
2. **Ganchos de ejecutor** чтобы `iroha_core` ingest-ил DA recibos, эмитированные
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/índices** чтобы WSV быстро отвечал на consultas de compromiso
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii Adiciones de RPC** para listar/consultar/probar puntos finales
   `/v2/da/commitments`.
5. **Pruebas de integración + accesorios** para probar el diseño de los cables y el flujo de prueba
   `integration_tests/tests/da/commitments.rs`.

## 1. Modelo de datos de distribución

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
```- `KzgCommitment` tiene una configuración de 48 teclas en `iroha_crypto::kzg`.
  При отсутствии используем только Pruebas de Merkle.
- `proof_scheme` está disponible en los carriles del catálogo; Merkle lanes отклоняют cargas útiles KZG,
  A `kzg_bls12_381` carriles требуют ненулевых compromisos KZG. Torii сейчас
  производит только Merkle compromisos и отклоняет carriles с конфигурацией KZG.
- `KzgCommitment` tiene una configuración de 48 teclas en `iroha_crypto::kzg`.
  Los usuarios de Merkle Lanes utilizaron pruebas de Merkle.
- `proof_digest` закладывает DA-5 PDP/PoTR integrado, чтобы запись содержала
  muestreo rápido, aplicación de manchas.

### 1.2 Encabezado del bloque Расширение

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

Este paquete incluye un bloque de hash, una etiqueta y unos metadatos `SignedBlockWire`. cogoda
накладных расходов.

Nota de implementación: `BlockPayload` и прозрачный `BlockBuilder` теперь имеют
definidores/captadores `da_commitments` (см. `BlockBuilder::set_da_commitments` и
`SignedBlock::set_da_commitments`), donde los hosts pueden registrarse
предварительно собранный paquete до запечатывания блока. Все ayudante-constructorы
Asegúrese de que el `None` y el Torii no estén disponibles en paquetes reales.

### 1.3 Codificación de cables- `SignedBlockWire::canonical_wire()` incluye el encabezado Norito
  `DaCommitmentBundle` сразу после списка транзакций. Byte de versión `0x01`.
- `SignedBlockWire::decode_wire()` otклоняет paquetes con неизвестной `version`,
  в соответствии с Norito político из `norito.md`.
- Derivación de hash обновляется только в `block::Hasher`; clientes ligeros, которые
  декодируют существующий formato de cable, автоматически получают новое поле, потому
  что Norito encabezado объявляет его наличие.

## 2. Поток выпуска блоков

1. Torii DA ingesta finaliza `DaIngestReceipt` y publica ego en внутреннюю
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` собирает все recibos с совпадающим `lane_id` для блока в
   построении, дедуплицируя по `(lane_id, client_blob_id, manifest_hash)`.
3. Antes de completar los compromisos del constructor según `(lane_id, epoch,
   secuencia)` para el hash determinado, el paquete de codificación Norito codec y
   обновляет `da_commitments_hash`.
4. El paquete polaco se encuentra en WSV y se emite en bloques en
   `SignedBlockWire`.

Если создание блока проваливается, recibos остаются в очереди для следующей
попытки; constructor записывает последний включенный `sequence` по каждой carril,
чтобы предотвратить replay атаки.

## 3. RPC y superficie de consulta

Torii anterior al punto final:| Ruta | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (rango-filtro por carril/época/secuencia, paginación) | Incluye `DaCommitmentPage` con recuento total, compromisos y bloque de hash. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto o corte `(epoch, sequence)`). | Отвечает `DaCommitmentProof` (registro + ruta Merkle + bloque hash). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Ayudante sin estado, bloque de hash personalizado e inclusión comprobada; Los SDK se instalan desde el archivo `iroha_crypto`. |

Estas cargas útiles están incluidas en `iroha_data_model::da::commitment`. Torii enrutadores
controladores de montaje junto con puntos finales de ingesta de DA, que se pueden utilizar
tokens políticos/mTLS.

## 4. Pruebas de inclusión y clientes ligeros

- Производитель блока строит бинарное Merkle дерево по сериализованному списку
  `DaCommitmentRecord`. Корень подает `da_commitments_hash`.
- `DaCommitmentProof` actualiza el registro de televisión y el vector `(sibling_hash,
  posición)` чтобы верификаторы смогли восстановить корень. Prueba de hash включает
  bloque y encabezado de подписанный, чтобы clientes ligeros pueden proporcionar finalidad.
- CLI helpers (`iroha_cli app da prove-commitment`) оборачивают цикл solicitud de prueba/
  verifique y coloque Norito/hex según el operador.

## 5. Almacenamiento e informaciónWSV organiza compromisos en la familia de columnas anterior con el botón `manifest_hash`.
Los índices antiguos incluyen `(lane_id, epoch)` e `(lane_id, sequence)`, entre ellos
запросы не сканировали полные paquetes. Каждый record хранит высоту блока, в
котором он был запечатан, что позволяет catch-up узлам быстро восстанавливать
индекс из bloque de registro.

## 6. Telemetría y observabilidad

- `torii_da_commitments_total` увеличивается, когда блок запечатывает minимум
  registro de odin.
- `torii_da_commitment_queue_depth` отслеживает recibos, ожидающие agrupación
  (по carril).
- Grafana tablero `dashboards/grafana/da_commitments.json` visualización
  включение в блок, глубину очереди и prueba de rendimiento para la auditoría del lanzamiento de DA-3
  puerta.

## 7. Prueba de estrategia

1. **Pruebas unitarias** para codificación/decodificación `DaCommitmentBundle` y обновлений
   derivación de hash de bloque.
2. **Accesorios dorados** en `fixtures/da/commitments/` con un paquete de bytes canónicos
   y Pruebas de Merkle.
3. **Pruebas de integración** con pruebas de validación, ingesta de blobs de muestra y prueba
   согласованности paquete y consulta/prueba ответов.
4. **Pruebas de cliente ligero** en `integration_tests/tests/da/commitments.rs` (Rust),
   вызывающие `/prove` y проверяющие prueba без обращения к Torii.
5. **CLI smoke** guión `scripts/da/check_commitments.sh` para los motores de combustión interna
   herramientas del operador.

## 8. Lanzamiento del plan| Fase | Descripción | Criterios de salida |
|-------|-------------|---------------|
| P0 - Fusión del modelo de datos | Utilice `DaCommitmentRecord`, encabezado de bloque actualizado y códecs Norito. | `cargo test -p iroha_data_model` зеленый с новыми accesorios. |
| P1 - Cableado central/WSV | Desarrollar la lógica del generador de colas y bloques, actualizar los índices y eliminar los controladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` проходят с проверками prueba de paquete. |
| P2 - Herramientas del operador | Publicar ayudantes de CLI, panel de control Grafana y documentos actualizados para la verificación de prueba. | `iroha_cli app da prove-commitment` работает на devnet; tablero показывает en vivo данные. |
| P3 - Puerta de gobernanza | Abra el validador de bloques, tres compromisos de DA en los carriles, incluidos en `iroha_config::nexus`. | Entrada de estado + actualización de la hoja de ruta помечают DA-3 как завершенный. |

## Открытые вопросы

1. **Valores predeterminados de KZG vs Merkle** — Нужно ли для маленьких blobs всегда пропускать
   Compromisos de KZG, чтобы уменьшить размер блока? Предложение: оставить
   `kzg_commitment` ofrece opciones y compuertas desde `iroha_config::da.enable_kzg`.
2. **Brechos en la secuencia** — Разрешать ли разрывы последовательности? Plan de tecnología
   отклоняет brechas, если gobernanza не включит `allow_sequence_skips` для
   repetición exclusiva.
3. **Caché de cliente ligero** — El SDK de comando utiliza caché SQLite moderno para pruebas;
   дальнейшая работа под DA-8.Ответы на эти вопросы в implementación RPs переведут DA-3 из статуса "borrador"
(este documento) en "en progreso" después de iniciar el proceso de codificación.