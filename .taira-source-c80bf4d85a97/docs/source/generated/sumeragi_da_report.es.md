---
lang: es
direction: ltr
source: docs/source/generated/sumeragi_da_report.md
status: needs-update
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: fd381ca301bd673a337c07379e755e0cd9c390072fa60aece98b60cddcea351a
source_last_modified: "2025-11-02T04:40:40.073146+00:00"
translation_last_reviewed: 2025-11-14
---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to `docs/source/generated/sumeragi_da_report.md` for current semantics.

# Informe de disponibilidad de datos de Sumeragi

Se procesaron 3 archivos de resumen desde `artifacts/sumeragi-da/20251005T190335Z`.

## Resumen

| Escenario | Ejecuciones | Pares | Carga útil (MiB) | Mediana RBC deliver (ms) | Máximo RBC deliver (ms) | Mediana de Commit (ms) | Máximo de Commit (ms) | Mediana de throughput (MiB/s) | Mínimo de throughput (MiB/s) | RBC<=Commit | Máx. cola BG | Máx. descartes P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- ejecuciones: 1
- pares: 4
- carga útil: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- número de votos READY: 4
- entrega RBC protegida por Commit: yes
- media de entrega RBC (ms): 3120.00
- media de Commit (ms): 3380.00
- throughput medio (MiB/s): 3.28
- profundidad máxima/mediana de la cola BG post: 18 / 18
- descartes máximos/medianos de la cola P2P: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de entrega por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Ejecución | Origen | Bloque | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Recibidos | Máx. cola BG | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- ejecuciones: 1
- pares: 6
- carga útil: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- número de votos READY: 5
- entrega RBC protegida por Commit: yes
- media de entrega RBC (ms): 3280.00
- media de Commit (ms): 3560.00
- throughput medio (MiB/s): 3.21
- profundidad máxima/mediana de la cola BG post: 24 / 24
- descartes máximos/medianos de la cola P2P: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de entrega por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Ejecución | Origen | Bloque | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Recibidos | Máx. cola BG | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- ejecuciones: 1
- pares: 4
- carga útil: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- número de votos READY: 4
- entrega RBC protegida por Commit: yes
- media de entrega RBC (ms): 3340.00
- media de Commit (ms): 3620.00
- throughput medio (MiB/s): 3.16
- profundidad máxima/mediana de la cola BG post: 19 / 19
- descartes máximos/medianos de la cola P2P: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de entrega por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Ejecución | Origen | Bloque | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Recibidos | Máx. cola BG | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
