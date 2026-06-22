<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi Modelo formal (TLA+ / Apalache)

Este directorio contiene modelos formales delimitados para la seguridad y vivacidad de Sumeragi.

## Alcance

`Sumeragi.tla` captura la ruta de confirmación:
- progresión de fase (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- umbrales de votación y quórum (`CommitQuorum`, `ViewQuorum`),
- quórum de participación ponderado (`StakeQuorum`) para guardias de compromiso estilo NPoS,
- Causalidad de glóbulos rojos (`Init -> Chunk -> Ready -> Deliver`) con evidencia de encabezado/resumen,
- GST y supuestos débiles de equidad sobre acciones de progreso honestas.

`SumeragiFrontierRecovery.tla` captura la clase de suspensión de Taira enfocada alrededor de uno
bloque fronterizo contiguo pendiente:
- evidencia de votación confirmada por debajo o con quórum,
- atrasos en la cola de votos y drenaje local,
- estado de carga útil faltante versus local,
- propiedad de recuperación de frontera nueva versus obsoleta,
- marcador de reprogramación de quórum/ritmo de ventana,
- evidencia de frontera futura/nueva visión que pueda volver a anclar la frontera local,
- confirmación determinista post-GST, retransmisión, rotación de vista limitada y
  Resultados de caída sin evidencia.

Ambos modelos abstraen intencionalmente los formatos de cable, ECDSA/firma
verificación y detalles completos de la red.

## Archivos- `Sumeragi.tla`: modelo de protocolo y propiedades.
- `Sumeragi_fast.cfg`: conjunto de parámetros más pequeño compatible con CI.
- `Sumeragi_deep.cfg`: conjunto de parámetros de tensión mayor.
- `SumeragiFrontierRecovery.tla`: modelo de recuperación de frontera enfocado.
- `SumeragiFrontierRecovery_fast.cfg`: conjunto de parámetros de frontera más pequeño compatible con CI.
- `SumeragiFrontierRecovery_deep.cfg`: conjunto de límites de vista/ventana/acumulación de frontera más grande.
- `SumeragiFrontierRecovery_wide.cfg`: conjunto encuadernado con fronteras más anchas manual.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: mutación de propietario obsoleto de falla esperada.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: mutación en cola de votos de fracaso esperado.

## Propiedades

Invariantes:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Propiedad temporal:
- `EventuallyCommit` (`[] (gst => <> committed)`), con equidad post-GST codificada
  operativamente en `Next` (protecciones de tiempo de espera/prevención de fallas activadas)
  acciones de avance). Esto mantiene el modelo comprobable con Apalache 0.52.x, que
  no admite operadores de equidad `WF_` dentro de propiedades temporales marcadas.

Invariantes de recuperación de frontera:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, que descarta un terminal
  estado posterior al GST donde `pending /\ voteBacked /\ ~committed` no tiene recuperación,
  confirmación, retransmisión, rotación o transición de caída limitada.Propiedad temporal de recuperación de frontera:
- `PostGstVoteBackedFrontierEventuallyResolves`: después del GST, todos los problemas no resueltos
  El estado fronterizo pendiente respaldado por votos finalmente alcanza el compromiso y la carga útil.
  recuperación, retransmisión de quórum, reanclaje de frontera futura o vista limitada
  rotación.
- `RecoveredPayloadEventuallyAdvances`: un estado fronterizo respaldado por votos que ha
  recuperada, la carga útil no puede permanecer pendiente para siempre sin comprometerse,
  retransmitir, reanclar o rotar.
- `QuorumRetransmitEventuallyLeavesPending`: una vez que se ha activado la retransmisión de quórum
  para un estado fronterizo respaldado por el voto, el envoltorio pendiente eventualmente debe aclararse.
- `FutureFrontierEvidenceEventuallyReanchors`: evidencia de frontera/nueva visión posterior
  debe limpiar el envoltorio pendiente o ser consumido como un anclaje de frontera.

## Mapa de supuestos

El modelo de frontera es intencionalmente finito. Estas son la implementación
superficies que abstrae:| Concepto de modelo | Superficie de implementación |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | Manejo de `PendingBlock` y verificaciones de carga útil local en `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, además de materialización de propiedad de frontera/BlockCreated en `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Conteo de votos de confirmación y control de ingreso de votos ejercidos por `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` e `reschedule_ignores_quorum_timeout_vote_queue_backlog` en `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Estado de propietario de frontera activo/obsoleto en `frontier_slot_has_active_owner_state_for_view(...)`, rendimiento de propietario obsoleto en `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` y limpieza de reemplazo en `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Ritmo de reprogramación del quórum respaldado por votos en `reschedule_stale_pending_blocks_with_now(...)`; la cobertura de regresión incluye `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Reparación de carrocería de frontera exacta y admisión de reparación de glóbulos rojos obsoletos en `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` e `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Selección de destino de retransmisión de quórum, `rebroadcast_pending_block_updates(...)`, y llamadas deterministas de cambio de vista en `reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | Evidencia futura de quórum de nueva vista/frontera superior en `on_pacemaker_propose_ready(...)`, cubierta por `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## Corriendo

Desde la raíz del repositorio:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

El corredor establece un Apalache `--length` explícito para cada modo:| Modo | Longitud | Uso previsto |
| --- | ---: | --- |
| `fast` | 10 | Comprobación de ruta de confirmación de CI |
| `deep` | 10 | Verificación de ruta de confirmación más amplia |
| `frontier-fast` | 10 | Control de fronteras de CI |
| `frontier-deep` | 12 | Control fronterizo más amplio |
| `frontier-wide` | 14 | Comprobación de estrés fronterizo manual/nocturno |

`APALACHE_LENGTH=<n>` anula el valor predeterminado por modo cuando se explora localmente un
contraejemplo o ampliación de una prueba acotada.

### Configuración local reproducible (no se requiere Docker)

Instale la cadena de herramientas local de Apalache anclada utilizada por este repositorio:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

El ejecutor detecta automáticamente esta instalación en:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Después de la instalación, `ci/check_sumeragi_formal.sh` debería funcionar sin variables de entorno adicionales:

```bash
bash ci/check_sumeragi_formal.sh
```

Las mutaciones de fracaso esperado están intencionalmente fuera del CI normal. ellos deberían
fallan bajo Apalache y son útiles al cambiar el modelo:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Si Apalache no está en `PATH`, puedes:

- establezca `APALACHE_BIN` en la ruta ejecutable, o
- utilice el respaldo Docker (habilitado de forma predeterminada cuando `docker` está disponible):
  - imagen: `APALACHE_DOCKER_IMAGE` (predeterminado `ghcr.io/apalache-mc/apalache:0.52.2`)
  - requiere un demonio Docker en ejecución
  - deshabilite el respaldo con `APALACHE_ALLOW_DOCKER=0`.

Ejemplos:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Notas- Este modelo complementa (no reemplaza) las pruebas ejecutables del modelo Rust en
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  y
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Las comprobaciones están limitadas por valores constantes en los archivos `.cfg`.
- PR CI ejecuta estas comprobaciones en `.github/workflows/pr.yml` vía
  `ci/check_sumeragi_formal.sh`.
