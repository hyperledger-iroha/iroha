---
lang: pt
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

# Relatório de disponibilidade de dados do Sumeragi

Foram processados 3 arquivos de resumo a partir de `artifacts/sumeragi-da/20251005T190335Z`.

## Resumo

| Cenário | Execuções | Pares | Payload (MiB) | Mediana de RBC deliver (ms) | Máximo de RBC deliver (ms) | Mediana de Commit (ms) | Máximo de Commit (ms) | Mediana de throughput (MiB/s) | Mínimo de throughput (MiB/s) | RBC<=Commit | Fila BG máx. | Queda P2P máx. |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- execuções: 1
- pares: 4
- payload: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- contagem de votos READY: 4
- entrega RBC protegida por Commit: yes
- média de RBC deliver (ms): 3120.00
- média de Commit (ms): 3380.00
- throughput médio (MiB/s): 3.28
- profundidade da fila BG post máx./mediana: 18 / 18
- quedas na fila P2P máx./mediana: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de deliver por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Execução | Origem | Bloco | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total de chunks | Recebidos | Fila BG máx. | Queda P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- execuções: 1
- pares: 6
- payload: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- contagem de votos READY: 5
- entrega RBC protegida por Commit: yes
- média de RBC deliver (ms): 3280.00
- média de Commit (ms): 3560.00
- throughput médio (MiB/s): 3.21
- profundidade da fila BG post máx./mediana: 24 / 24
- quedas na fila P2P máx./mediana: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de deliver por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Execução | Origem | Bloco | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total de chunks | Recebidos | Fila BG máx. | Queda P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- execuções: 1
- pares: 4
- payload: 11010048 bytes (10.50 MiB)
- chunks de RBC observados: 168
- contagem de votos READY: 4
- entrega RBC protegida por Commit: yes
- média de RBC deliver (ms): 3340.00
- média de Commit (ms): 3620.00
- throughput médio (MiB/s): 3.16
- profundidade da fila BG post máx./mediana: 19 / 19
- quedas na fila P2P máx./mediana: 0 / 0
- bytes por par: 11010048 - 11010048
- broadcasts de deliver por par: 1 - 1
- broadcasts READY por par: 1 - 1

| Execução | Origem | Bloco | Altura | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total de chunks | Recebidos | Fila BG máx. | Queda P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
