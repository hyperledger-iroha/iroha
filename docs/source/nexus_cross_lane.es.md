---
lang: es
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

# Compromisos cross-lane de Nexus y pipeline de pruebas

> **Estado:** entregable NX-4 - pipeline de compromisos cross-lane y pruebas (objetivo Q4 2025).  
> **Responsables:** Nexus Core WG / Cryptography WG / Networking TL.  
> **Items de roadmap relacionados:** NX-1 (geometria de lanes), NX-3 (settlement router), NX-4 (este documento), NX-8 (scheduler global), NX-11 (conformidad de SDK).

Esta nota describe como los datos de ejecucion por lane se convierten en un compromiso global verificable. Conecta el settlement router existente (`crates/settlement_router`), el lane block builder (`crates/iroha_core/src/block.rs`), las superficies de telemetria/status, y los hooks LaneRelay/DA planificados que aun faltan para el roadmap **NX-4**.

## Objetivos

- Producir un `LaneBlockCommitment` deterministico por lane block que capture settlement, liquidez y datos de varianza sin filtrar estado privado.
- Reenviar estos compromisos (y sus atestaciones DA) al anillo NPoS global para que el merge ledger ordene, valide y persista actualizaciones cross-lane.
- Exponer los mismos payloads via Torii y telemetria para que operadores, SDKs y auditores puedan reproducir el pipeline sin tooling a medida.
- Definir las invariantes y paquetes de evidencia requeridos para graduar NX-4: pruebas de lane, atestaciones DA, integracion del merge ledger y cobertura de regresion.

## Componentes y superficies

| Componente | Responsabilidad | Referencias de implementacion |
|-----------|----------------|-------------------------------|
| Ejecutor de lane y settlement router | Cotizar conversiones XOR, acumular recibos por transaccion, aplicar politica de buffer | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | Drenar `SettlementAccumulator`s, emitir `LaneBlockCommitment`s junto al lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | Empaquetar QCs de lane + pruebas DA, difundirlos por `iroha_p2p`, y alimentar el merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | Verificar QCs de lane, reducir merge hints, persistir compromisos de world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status y dashboards | Exponer `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, gauges del scheduler y tableros Grafana | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Almacenamiento de evidencia | Archivar `LaneBlockCommitment`s, artefactos RBC y snapshots de Alertmanager para auditorias | `docs/settlement-router.md`, `artifacts/nexus/*` (futuro bundle) |

## Estructuras de datos y layout de payload

Los payloads canonicos viven en `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` - hash de transaccion o id provisto por el caller.
- `local_amount_micro` - debito del token de gas del dataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` - entradas deterministicas del libro XOR y el margen de seguridad por recibo (`due - after haircut`).
- `timestamp_ms` - timestamp UTC en milisegundos capturado durante el settlement.

Los recibos heredan las reglas de cotizacion deterministicas de `SettlementEngine` y se agregan dentro de cada `LaneBlockCommitment`.

### `LaneSwapMetadata`

Metadatos opcionales que registran los parametros usados al cotizar:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket de `liquidity_profile` (Tier1-Tier3).
- string `twap_local_per_xor` para que auditores recomputen conversiones con exactitud.

### `LaneBlockCommitment`

Resumen por lane almacenado con cada bloque:

- Encabezado: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Totales: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- `swap_metadata` opcional.
- Vector `receipts` ordenado.

Estas structs ya derivan `NoritoSerialize`/`NoritoDeserialize`, asi que pueden transmitirse on-chain, por Torii o via fixtures sin deriva de esquema.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (ver `crates/iroha_data_model/src/nexus/relay.rs`) empaqueta el `BlockHeader`
de la lane, un `ExecutionQcRecord` opcional, un hash opcional de `DaCommitmentBundle`, el
`LaneBlockCommitment` completo y el conteo de bytes RBC por lane. El sobre almacena un
`settlement_hash` derivado de Norito (via `compute_settlement_hash`) para que los receptores
validen el payload de settlement antes de reenviarlo al merge ledger. Los llamadores deben
rechazar sobres cuando `verify` falla (mismatch de sujeto QC, mismatch de hash DA o mismatch de
settlement hash), cuando `verify_with_quorum` falla (errores de longitud de bitmap de
firmantes/quorum), o cuando la firma QC agregada no puede verificarse contra el roster del
comite por dataspace. La preimagen del QC cubre el hash del lane block mas `parent_state_root` y
`post_state_root`, para que la membresia y la correccion del state-root se verifiquen juntas.

### Seleccion de comite de lane

Los QCs de lane relay se validan contra un comite por dataspace. El tamano del comite es `3f+1`,
donde `f` se configura en el catalogo del dataspace (`fault_tolerance`). El pool de validadores
son los validadores del dataspace: manifests de gobernanza de lane para lanes admin-managed y
registros de staking de lane publica para lanes stake-elected. La membresia del comite se toma de
forma deterministica por epoca usando la semilla de epoca VRF enlazada con `dataspace_id` y
`lane_id` (estable durante la epoca). Si el pool es menor que `3f+1`, la finalidad del lane relay
se pausa hasta que se restaure el quorum. Los operadores pueden extender el pool usando la
instruccion multisig admin `SetLaneRelayEmergencyValidators` (requiere `CanManagePeers` y
`nexus.lane_relay_emergency.enabled = true`, que esta deshabilitado por defecto). Cuando esta
habilitado, la autoridad debe ser una cuenta multisig que cumpla los minimos configurados
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, por defecto 3-de-5). Las
sobrescrituras se almacenan por dataspace, se aplican solo cuando el pool esta bajo quorum, y se
limpian al enviar una lista vacia de validadores. Cuando `expires_at_height` esta definido, la
validacion ignora la sobrescritura una vez que el `block_height` del lane relay envelope supera
la altura de expiracion. El contador de telemetria
`lane_relay_emergency_override_total{lane,dataspace,outcome}` registra si la sobrescritura se
aplico (`applied`) o faltaba/expirada/insuficiente/deshabilitado durante la validacion.

## Ciclo de vida del compromiso

1. **Cotizar y preparar recibos.**  
   La fachada de settlement (`SettlementEngine`, `SettlementAccumulator`) registra un
   `PendingSettlement` por transaccion. Cada registro almacena entradas TWAP, perfil de
   liquidez, timestamps y montos XOR para que luego se convierta en un `LaneSettlementReceipt`.

2. **Sellar recibos en el bloque.**  
   Durante `BlockBuilder::finalize`, cada par `(lane_id, dataspace_id)` drena su acumulador. El
   builder instancia un `LaneBlockCommitment`, copia la lista de recibos, acumula totales y
   almacena metadatos de swap opcionales (via `SwapEvidence`). El vector resultante se empuja a
   la ranura de estado de Sumeragi (`crates/iroha_core/src/sumeragi/status.rs`) para que Torii y
   telemetria lo expongan de inmediato.

3. **Empaquetado de relay y atestaciones DA.**  
   `LaneRelayBroadcaster` ahora consume los `LaneRelayEnvelope`s emitidos durante el sellado del
   bloque y los difunde como frames `NetworkMessage::LaneRelay` de alta prioridad. Los sobres se
   verifican, se deduplican por `(lane_id,dataspace_id,height,settlement_hash)`, y se persisten
   en el snapshot de estado de Sumeragi (`/v1/sumeragi/status`) para operadores y auditores. El
   broadcaster seguira evolucionando para adjuntar artefactos DA (pruebas de chunk RBC, headers
   Norito, manifests SoraFS/Object) y alimentar el merge ring sin bloqueo de head-of-line.

4. **Ordenamiento global y merge ledger.**  
   El anillo NPoS valida cada relay envelope: verificar `lane_qc` contra el comite por dataspace,
   recomputar totales de settlement, verificar pruebas DA, luego alimentar el tip de la lane al
   merge ledger descrito en `docs/source/merge_ledger.md`. Cuando la entrada de merge se sella, el
   hash de world-state (`global_state_root`) ahora compromete cada `LaneBlockCommitment`.

5. **Persistencia y exposicion.**  
   Kura escribe el lane block, la entrada de merge y el `LaneBlockCommitment` de forma atomica
   para que el replay reconstruya la misma reduccion. `/v1/sumeragi/status` expone:
   - `lane_commitments` (metadata de ejecucion).
   - `lane_settlement_commitments` (el payload descrito aqui).
   - `lane_relay_envelopes` (headers de relay, QCs, digests DA, settlement hash y conteos de bytes RBC).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) leen las mismas superficies de telemetria y
  status para mostrar throughput de lane, advertencias de disponibilidad DA, volumen RBC, deltas
  de settlement y evidencia de relay.

## Reglas de verificacion y pruebas

El merge ring DEBE aplicar lo siguiente antes de aceptar un compromiso de lane:

1. **Validez del QC de lane.** Verificar la firma BLS agregada sobre la preimagen del voto de
   ejecucion (hash de bloque, `parent_state_root`, `post_state_root`, altura/vista/epoca,
   `chain_id` y tag de modo) contra el roster del comite por dataspace; asegurar que la longitud
   del bitmap de firmantes coincide con el comite, que los firmantes mapean a indices validos, y
   que la altura del header coincide con `LaneBlockCommitment.block_height`.
2. **Integridad de recibos.** Recomputar los agregados `total_*` desde el vector de recibos;
   rechazar el compromiso si las sumas divergen o los recibos contienen `source_id`s duplicados.
3. **Sanidad de metadatos de swap.** Confirmar que `swap_metadata` (si esta presente) coincide
   con la configuracion de settlement y politica de buffer de la lane.
4. **Atestacion DA.** Validar que las pruebas RBC/SoraFS provistas por el relay hashean al digest
   embebido y que el conjunto de chunks cubre el payload completo del bloque (`rbc_bytes_total`
   en telemetria debe reflejar esto).
5. **Reduccion de merge.** Una vez que las pruebas por lane pasan, incluir el tip de la lane en la
   entrada del merge ledger y recomputar la reduccion Poseidon2 (`reduce_merge_hint_roots`).
   Cualquier mismatch aborta la entrada de merge.
6. **Telemetria y rastro de auditoria.** Incrementar los contadores de auditoria por lane
   (`nexus_audit_outcome_total{lane_id,...}`) y persistir el sobre para que el bundle de evidencia
   contenga tanto la prueba como el rastro de observabilidad.

## Disponibilidad de datos y observabilidad

- **Metricas:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`, y
  `nexus_audit_outcome_total` ya existen en `crates/iroha_telemetry/src/metrics.rs`. Los
  operadores deben alertar sobre picos de missing-availability (los contadores de reschedule son
  de simulacros adversarios.
- **Superficies Torii:**  
  `/v1/sumeragi/status` incluye `lane_commitments`, `lane_settlement_commitments` y snapshots de
  dataspace. `/v1/nexus/lane-config` (planeado) publicara la geometria de `LaneConfig` para que
  clientes puedan mapear `lane_id` <-> etiquetas de dataspace.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` grafica backlog de lane, senales de disponibilidad DA y
  los totales de settlement expuestos arriba. Las definiciones de alertas deben paginar cuando:
  - `nexus_scheduler_dataspace_age_slots` viola la politica.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` aumenta de forma persistente.
  - `total_xor_variance_micro` se desvia de normas historicas.
- **Bundles de evidencia:**  
  Cada release debe adjuntar exportaciones de `LaneBlockCommitment`, snapshots de
  Grafana/Alertmanager y los manifests de DA relay bajo `artifacts/nexus/cross-lane/<date>/`. El
  bundle se convierte en el conjunto de prueba canonico al enviar reportes de readiness de NX-4.

## Checklist de implementacion (NX-4)

1. **Servicio LaneRelay**
   - Esquema definido en `LaneRelayEnvelope`; broadcaster implementado en
     `crates/iroha_core/src/nexus/lane_relay.rs` e integrado al sellado de bloques
     (`crates/iroha_core/src/sumeragi/main_loop.rs`), emitiendo `NetworkMessage::LaneRelay` con
     de-duplicacion por nodo y persistencia de status.
   - Persistir artefactos de relay para auditorias (`artifacts/nexus/relay/...`).
2. **Hooks de atestacion DA**
   - Integrar pruebas de chunk RBC / SoraFS con relay envelopes y almacenar metricas resumen en
     `SumeragiStatus`.
   - Exponer estado DA via Torii y Grafana para operadores.
3. **Validacion de merge ledger**
   - Extender el validador de entradas de merge para requerir relay envelopes, no headers de lane
     crudos.
   - Agregar tests de replay (`integration_tests/tests/nexus/*.rs`) que alimenten compromisos
     sinteticos en el merge ledger y afirmen reduccion deterministica.
4. **Actualizaciones de SDK y tooling**
   - Documentar el layout Norito de `LaneBlockCommitment` para consumidores de SDK
     (`docs/portal/docs/nexus/lane-model.md` ya enlaza aqui; extenderlo con snippets de API).
   - Fixtures deterministicas viven bajo `fixtures/nexus/lane_commitments/*.{json,to}`; correr
     `cargo xtask nexus-fixtures` para regenerar (o `--verify` para validar) las muestras
     `default_public_lane_commitment` y `cbdc_private_lane_commitment` cuando haya cambios de esquema.
5. **Observabilidad y runbooks**
   - Cablear el pack de Alertmanager para las nuevas metricas y documentar el workflow de evidencia
     en `docs/source/runbooks/nexus_cross_lane_incident.md` (seguimiento).

Completar el checklist anterior, junto con esta especificacion, satisface la parte de documentacion
de **NX-4** y desbloquea el trabajo de implementacion restante.
