---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: priority-snapshot-2025-03
title: Снимок приоритетов — март 2025 (Бета)
description: Зеркало steering snapshot Nexus 2025-03; ожидает ACKs перед публичным rollout.
---

> Канонический источник: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Статус: **Бета / ожидаются ACKs steering** (Networking, Storage, Docs leads).

## Обзор

Мартовский snapshot удерживает инициативы docs/content-network в согласии с
доставочными треками SoraFS (SF-3, SF-6b, SF-9). Как только все leads подтвердят
snapshot в канале steering Nexus, удалите примечание “Beta” выше.

### Фокусные темы

1. **Распространить snapshot приоритетов** — собрать acknowledgements и
   зафиксировать их в minutes совета от 2025-03-05.
2. **Закрыть kickoff Gateway/DNS** — отрепетировать новый facilitation kit (Раздел 6
   в runbook) до воркшопа 2025-03-03.
3. **Миграция операторских runbooks** — портал `Runbook Index` уже live; открыть beta
   preview URL после sign-off onboarding для reviewers.
4. **Треки доставки SoraFS** — согласовать оставшуюся работу SF-3/6b/9 с планом/roadmap:
   - Worker ingestion PoR + status endpoint в `sorafs-node`.
   - Полировка CLI/SDK bindings в интеграциях orchestrator Rust/JS/Swift.
   - Wiring runtime для координатора PoR и события GovernanceLog.

См. исходный файл для полной таблицы, distribution checklist и log entries.
