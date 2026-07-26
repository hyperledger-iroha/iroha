---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: es
direction: ltr
source: docs/amx.md
status: complete
translator: manual
source_hash: 563ec4d7d4d96fa04a7b210a962b9927046985eb01d1de0954575cf817f9f226
source_last_modified: "2025-11-09T19:43:51.262828+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/amx.md (AMX Execution & Operations Guide) -->

# Guía de Ejecución y Operaciones de AMX

**Estado:** Borrador (NX‑17)  
**Audiencia:** Equipo de protocolo core, ingenieros de AMX/consenso, SRE/Telemetría, equipos de SDK y Torii  
**Contexto:** Completa el ítem de roadmap “Documentación (owner: Docs) — actualizar
`docs/amx.md` con diagramas de tiempo, catálogo de errores, expectativas de
operadores y guía para desarrolladores sobre generación/uso de PVOs.”

## Resumen

Las transacciones atómicas entre espacios de datos (AMX) permiten que un único
envío afecte a múltiples data spaces (DS) preservando la finalidad de slot de
1 s, códigos de fallo deterministas y confidencialidad para los fragmentos en
DS privados. Esta guía captura el modelo de tiempos, el manejo canónico de
errores, los requisitos de evidencia para operadores y las expectativas de
desarrolladores para los Proof Verification Objects (PVOs), de modo que el
entregable de roadmap sea autocontenido sin depender del documento de diseño
de Nexus (`docs/source/nexus.md`).

Garantías clave:

- Cada envío AMX recibe presupuestos deterministas de prepare/commit; los
  excesos abortan con códigos documentados en vez de bloquear lanes.
- Las muestras de DA que no cumplen el presupuesto mueven la transacción al
  siguiente slot y emiten telemetría explícita (`missing-availability warning`) en
  lugar de estancar silenciosamente el throughput.
- Los PVOs desacoplan las pruebas pesadas de la ventana de 1 s al permitir que
  clientes/batchers preregistren artefactos que el host nexus valida
  rápidamente dentro del slot.

## Modelo de tiempos de slot

### Cronograma

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Los presupuestos se alinean con el plan global del ledger: mempool
  70 ms, commit de DA ≤300 ms, consenso 300 ms, IVM/AMX 250 ms, settlement
  40 ms y guard 40 ms.
- Las transacciones que se salen de la ventana de DA se reprograman de forma
  determinista; el resto de incumplimientos devuelven códigos como
  `AMX_TIMEOUT` o `SETTLEMENT_ROUTER_UNAVAILABLE`.
- La porción de guard absorbe la exportación de telemetría y la auditoría final
  para que el slot siga cerrando en 1 s aunque los exporters se retrasen
  brevemente.

### Swim lane cross‑DS

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Cada fragmento de DS debe completar su ventana de prepare de 30 ms antes de
que la lane ensamble el slot. Las pruebas que falten permanecen en el
mempool para el siguiente slot en lugar de bloquear a los peers.

### Checklist de instrumentación

| Métrica / Traza | Origen | SLO / Alerta | Notas |
|-----------------|--------|--------------|-------|
| `iroha_slot_duration_ms` (histograma) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Gate de CI descrito en `ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (hook de commit) | ≥0.95 por ventana de 30 min | Derivada de la telemetría de reprogramación DA; cada bloque actualiza el gauge (`crates/iroha_core/src/telemetry.rs`). |
| `iroha_amx_prepare_ms` | Host IVM | p95 ≤ 30 ms por DS scope | Activa abortos `AMX_TIMEOUT`. |
| `iroha_amx_commit_ms` | Host IVM | p95 ≤ 40 ms por DS scope | Cubre fusión de deltas + ejecución de triggers. |
| `iroha_ivm_exec_ms` | Host IVM | Alerta si >250 ms por lane | Refleja la ventana de ejecución de overlays de IVM. |
| `iroha_amx_abort_total{stage}` | Executor | Alerta si >0.05 abortos/slot o picos sostenidos en una sola etapa | `stage`: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | Planificador AMX | Alerta si >0.1 conflictos/slot | Indica conjuntos R/W inexactos. |

## Catálogo de errores de AMX y expectativas para operadores

Errores típicos:

- `AMX_TIMEOUT` – un fragmento excedió su presupuesto (prepare/exec/commit).
- `AMX_LOCK_CONFLICT` – conflicto de bloqueo detectado por el scheduler.
- `SETTLEMENT_ROUTER_UNAVAILABLE` – el router de settlement no pudo enrutar la
  operación dentro del slot.
- `PVO_MISSING` – se esperaba un PVO preregistrado pero no se encontró.

Operadores deben:

- Vigilar las métricas anteriores y añadir paneles específicos de AMX a
  Grafana.
- Mantener runbooks para reacciones ante picos de `AMX_TIMEOUT` o
  `AMX_LOCK_CONFLICT` (por ejemplo, revisar hints de acceso o tuning de
  presupuestos).
- Archivar evidencias (logs, traces, PVOs) para incidentes de AMX de forma que
  se puedan reconstruir offline.

## Guía para desarrolladores: PVOs

- Los Proof Verification Objects encapsulan pruebas pesadas (por ejemplo
  pruebas ZK de fragmentos privados) en un artefacto que se puede verificar
  rápidamente en el nexus host.
- Los clientes deben preregistrar PVOs fuera de la ruta crítica del slot y
  referenciarlos desde las transacciones AMX a través de identificadores
  estables.
- El diseño de PVO debe garantizar:
  - Encapsulado Norito canónico (campos versionados, cabecera Norito,
    checksum).
  - Separación clara entre datos públicos y privados para preservar la
    confidencialidad.

Consulta `docs/source/nexus.md` para detalles de diseño más profundos y los
formatos exactos de PVO.

