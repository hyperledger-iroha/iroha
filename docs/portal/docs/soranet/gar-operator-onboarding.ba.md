---
lang: ba
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

Был ҡыҫҡаса ҡулланыу өсөн йәйелдерергә SNNet-9 үтәү конфигурацияһы менән ҡабатланған,
аудит-дуҫ процесы. Парлы уны юрисдикция менән обзор шулай һәр оператор
шул уҡ һеңдерелгән һәм дәлилдәр макетын ҡуллана.

## Аҙымдар

1. **Йыйыу конфиг**
   - Импорт `governance/compliance/soranet_opt_outs.json`.
   - Һеҙҙең I18NI000000005X менән берләштерегеҙ, аттестация һеңдереүҙең баҫылған .
     [юрисдикция тикшерелеүе] (gar-jurisdictional-review).
2. **Валидат**
   - I18NI000000006X
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Опциональ: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Дәлилдәр **
   - `artifacts/soranet/compliance/<YYYYMMDD>/` буйынса магазин:
     - `config.json` (һуңғы үтәү блогы)
     - `attestations.json` (URIs + distests)
     - раҫлау журналдары
     - ҡул ҡуйылған PDF-тар/I18NT00000000000Х конверттарына һылтанмалар
4. **Икенье **
   - 18NI00000012X роликты билдәләгеҙ), үҙгәртеп ҡороу оркестры/SDK конфигурациялары,
     һәм раҫлау I18NI000000013X ваҡиғалар логтар ҡайҙа көтөлгән.
5. **Ябығыҙ**
   - Дәлилдәр йыйылмаһы менән идара итеү советы менән файл.
   - ГАР журналында әүҙемләштереү тәҙрәһе + раҫлаусыларҙы теркәгеҙ.
   - Уйынса тикшерелгән даталар юрисдикция тикшерелгән таблица.

## Тиҙ тикшерелгән исемлек

- [ ] `jurisdiction_opt_outs` канон каталогына тап килә.
- [ ] Аттестация үҙләштерә теүәл күсергән.
- [ ] Валидация бойороҡтары йүгерә һәм архивланған.
- [ ] I18NI000000015X-та һаҡланған дәлилдәр өйөмө.
- [ ] рулет тег + GAR logbook яңыртылған.
- [ ] Киләһе тикшерелгән иҫкәртмәләр ҡуйылған.

## Ҡарағыҙ ҙа

- [ГАР Юрисдикция тикшерелеүе] (gar-jurisdictional-review)
- [ГАР үтәү плейбук (сығанаҡ)] (../../../source/soranet/gar_compliance_playbook.md)