---
lang: ru
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

# Отчёт о доступности данных Sumeragi

Обработано 3 файла‑сводки из `artifacts/sumeragi-da/20251005T190335Z`.

## Сводка

| Сценарий | Запуски | Пиры | Полезная нагрузка (MiB) | Медиана RBC deliver (мс) | Макс. RBC deliver (мс) | Медиана Commit (мс) | Макс. Commit (мс) | Медиана пропускной способности (MiB/s) | Мин. пропускная способность (MiB/s) | RBC<=Commit | Макс. очередь BG | Макс. потери P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- запусков: 1
- пиров: 4
- полезная нагрузка: 11010048 байт (10.50 MiB)
- наблюдаемые RBC‑чанки: 168
- число голосов READY: 4
- доставка RBC защищена коммитом: yes
- среднее время RBC deliver (мс): 3120.00
- среднее время Commit (мс): 3380.00
- средняя пропускная способность (MiB/s): 3.28
- максимальная/медианная глубина очереди BG post: 18 / 18
- максимальные/медианные потери в очереди P2P: 0 / 0
- байт на пир: 11010048 - 11010048
- broadcast‑сообщения deliver на пир: 1 - 1
- broadcast‑сообщения READY на пир: 1 - 1

| Запуск | Источник | Блок | Высота | View | RBC deliver (мс) | Commit (мс) | Пропускная способность (MiB/s) | RBC<=Commit | READY | Всего чанков | Получено | Макс. очередь BG | Потери P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- запусков: 1
- пиров: 6
- полезная нагрузка: 11010048 байт (10.50 MiB)
- наблюдаемые RBC‑чанки: 168
- число голосов READY: 5
- доставка RBC защищена коммитом: yes
- среднее время RBC deliver (мс): 3280.00
- среднее время Commit (мс): 3560.00
- средняя пропускная способность (MiB/s): 3.21
- максимальная/медианная глубина очереди BG post: 24 / 24
- максимальные/медианные потери в очереди P2P: 0 / 0
- байт на пир: 11010048 - 11010048
- broadcast‑сообщения deliver на пир: 1 - 1
- broadcast‑сообщения READY на пир: 1 - 1

| Запуск | Источник | Блок | Высота | View | RBC deliver (мс) | Commit (мс) | Пропускная способность (MiB/s) | RBC<=Commit | READY | Всего чанков | Получено | Макс. очередь BG | Потери P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- запусков: 1
- пиров: 4
- полезная нагрузка: 11010048 байт (10.50 MiB)
- наблюдаемые RBC‑чанки: 168
- число голосов READY: 4
- доставка RBC защищена коммитом: yes
- среднее время RBC deliver (мс): 3340.00
- среднее время Commit (мс): 3620.00
- средняя пропускная способность (MiB/s): 3.16
- максимальная/медианная глубина очереди BG post: 19 / 19
- максимальные/медианные потери в очереди P2P: 0 / 0
- байт на пир: 11010048 - 11010048
- broadcast‑сообщения deliver на пир: 1 - 1
- broadcast‑сообщения READY на пир: 1 - 1

| Запуск | Источник | Блок | Высота | View | RBC deliver (мс) | Commit (мс) | Пропускная способность (MiB/s) | RBC<=Commit | READY | Всего чанков | Получено | Макс. очередь BG | Потери P2P |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
