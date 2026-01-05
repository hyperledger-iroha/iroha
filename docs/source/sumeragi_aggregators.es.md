---
lang: es
direction: ltr
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee79dba673794a3dd4f888d3daf39163a827443bb22d413ab4d7f2e252762293
source_last_modified: "2025-12-26T13:17:08.872635+00:00"
translation_last_reviewed: 2026-01-01
---

# Enrutamiento de agregadores Sumeragi

## Resumen

Esta nota captura la estrategia determinista de enrutamiento de collectors ("aggregators") usada por Sumeragi despues de la actualizacion de equidad de la Fase 3. Cada validador calcula el mismo orden de collectors para una altura y vista de bloque dadas. El diseno elimina la dependencia de aleatoriedad ad hoc y mantiene el fan-out normal de votos acotado por la lista de collectors; cuando los collectors no estan disponibles o el quorum se estanca, los rebroadcasts de reprogramacion reutilizan objetivos de collectors con un fallback a la topologia de commit.

## Seleccion determinista

- El nuevo modulo `sumeragi::collectors` expone `deterministic_collectors(topology, mode, k, seed, height, view)` que devuelve un `Vec<PeerId>` reproducible para el par `(height, view)`.
- El modo permissioned rota el conjunto contiguo de collectors de cola por `height + view`, asegurando que cada collector pase a ser primario en un calendario round-robin. Esto preserva el comportamiento original de proxy-tail mientras distribuye la carga de forma uniforme en el segmento de cola (proxy tail + validadores Set B).
- El modo NPoS sigue usando el PRF por epoca, pero el helper ahora centraliza el calculo para que cada llamador reciba el mismo orden. La semilla se deriva de la aleatoriedad de la epoca provista por `EpochManager`.
- `CollectorPlan` rastrea el consumo de los objetivos ordenados y registra si se activo el fallback de gossip. Las actualizaciones de telemetria (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total`) muestran con que frecuencia ocurren los fallbacks y cuanto dura el fan-out redundante.

## Objetivos de equidad

1. **Reproducibilidad:** La misma topologia de validadores, modo de consenso y tupla `(height, view)` debe producir collectors primarios/secundarios identicos en cada peer. El helper oculta particularidades de la topologia (proxy tail, validadores Set B) para que el orden sea portable entre componentes y pruebas.
2. **Rotacion:** En despliegues permissioned el collector primario rota en cada bloque (y tambien cambia despues de un view bump), evitando que un solo validador Set B posea permanentemente las tareas de agregacion. La seleccion NPoS basada en PRF ya aporta aleatoriedad y no se ve afectada.
3. **Observabilidad:** La telemetria sigue reportando asignaciones por collector y el camino de fallback emite una advertencia cuando se activa gossip para que los operadores detecten collectors con mal comportamiento.

## Reintentos y backoff de gossip

- Los validadores mantienen un `CollectorPlan` en el estado de propuesta; el plan registra cuantos collectors han sido contactados y si se alcanzo el limite de fan-out redundante.
- Los planes de collectors se indexan por `(height, view)` y se reinicializan cuando el tema cambia, para que los reintentos de view-change obsoletos no reutilicen objetivos antiguos.
- El redundant send (`r`) se aplica de forma determinista avanzando en el plan. Cuando no hay collectors disponibles para la tupla `(height, view)`, los votos regresan a la topologia completa de commit (excluyendo al propio nodo) para evitar deadlock.
- Cuando el quorum se estanca, la ruta de reprogramacion reemite votos en cache mediante el plan de collectors y recurre a la topologia de commit cuando los collectors estan vacios, son solo locales o estan por debajo del quorum. Esto ofrece un fallback de "gossip" acotado sin pagar el costo de broadcast completo en la ruta rapida de estado estable.
- Cada descarte de una propuesta por el gate de locked commit certificate incrementa `block_created_dropped_by_lock_total`; las rutas de validacion de encabezado fallidas aumentan `block_created_hint_mismatch_total` y `block_created_proposal_mismatch_total`, ayudando a correlacionar fallbacks repetidos con problemas de correccion del leader. El snapshot `/v1/sumeragi/status` tambien exporta los hashes mas recientes de Highest/Locked commit certificate para que los dashboards correlacionen picos de descartes con hashes de bloque especificos.

## Resumen de implementacion

- El nuevo modulo publico `sumeragi::collectors` aloja `CollectorPlan` y `deterministic_collectors` para que tanto las pruebas a nivel de crate como las de integracion puedan verificar propiedades de equidad sin instanciar el actor de consenso completo.
- `CollectorPlan` vive en el estado de propuesta de Sumeragi y se reinicia cuando el pipeline de propuestas termina.
- `Sumeragi` construye planes de collectors via `init_collector_plan` y apunta a collectors al emitir votos de availability/precommit. Los votos de availability y precommit regresan a la topologia de commit cuando los collectors estan vacios, son solo locales o estan por debajo del quorum, y los rebroadcasts regresan bajo las mismas condiciones.
- Las pruebas unitarias e integradas validan la rotacion permissioned, el determinismo PRF y las transiciones de estado de backoff.

## Aprobacion de revision

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG
