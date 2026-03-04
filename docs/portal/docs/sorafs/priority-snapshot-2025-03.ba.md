---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
---

> Каноник сығанаҡ: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Статус: **Бета / көтөп руль ACKs** (Селтәр, Һаҡлау, Docs етәкселек итә).

## Обзор

Март снимоктары docs/йөкмәтке-селтәр башланғыстары менән тура килтерелгән тота.
I18NT000000000Х тапшырыу тректары (SF‐3, SF‐6b, SF‐9). Бер тапҡыр бөтә етәкселәр таный
снимок I18NT000000002X руль каналында, өҫтәге “Бета” иҫкәрмәһен алып ташларға.

### Фокус ептәр

1. **Тиҙлек өҫтөнлөклө снимок** — таныу йыйыу һәм уларҙы логин .
   2025-03-05 совет минуты.
2. **Шлюз/ДНС старт яҡын-аут** — репетиция яңы еңеләйтеү комплекты (Section6
   runbood-та) 2025-03-03 оҫтаханаһына тиклем.
3. **Оператор runbook миграцияһы** — порталы I18NI000000004X йәшәй; бета фашлау
   алдан ҡарау URL-адрестан һуң рецензент onboarding ҡул ҡуйыу.
4. **I18NT0000000001X тапшырыу ептәре** — план/юл картаһы менән SF‐3/6б/9 ҡалған эштәрҙе тура килтерергә:
   - `sorafs-node` PoR ашау эшсеһе + статус ос нөктәһе.
   - CLI/SDK бәйләүсе лак буйынса Rust/JS/Swift оркестры интеграциялары.
   - PoR координаторы эшләү ваҡыты проводка һәм идара итеү саралары.

Тулы таблица өсөн сығанаҡ файлын ҡарағыҙ, таратыу тикшерелгән исемлеге, һәм журнал яҙмалары.