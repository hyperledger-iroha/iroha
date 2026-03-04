---
lang: ba
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **Нисек ҡулланырға:** был шаблон `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` тиклем һәр дрельдан һуң шунда уҡ дубляж. Файл исемдәрен бәләкәй генә хәрефтәр менән тотоғоҙ, дефислы һәм быраулау идентификаторы менән тура килтереп, Alertmanager-ҙа логин.

# Ҡыҙыл-команда быраулау отчеты — `<SCENARIO NAME>`

- **Дилл ID:** `<YYYYMMDD>-<scenario>`
- **Дата һәм тәҙрә:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>` X
- **Сценарий класы:** `smuggling | bribery | gateway | ...`
- **Операторҙар:** `<names / handles>`
- **Приборҙар таҡталары коммиттан туңдырылған:** `<git SHA>`
- **Дәлилдәр өйөмө:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (факультатив):** `<cid>`  
- **Юл картаһы әйберҙәре менән бәйле:** `MINFO-9`, өҫтәүенә теләһә ниндәй бәйләнешле билеттар.

## 1. Маҡсаттар һәм инеү шарттары

- **Тәүге маҡсаттар**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **Пререквиситтар раҫланды**
  - `emergency_canon_policy.md` версияһы `<tag>` .
  - `dashboards/grafana/ministry_moderation_overview.json` Е.
  - Шылтыратыу буйынса вәкәләттәрҙе өҫтөн ҡуйыу: `<name>`

## 2.

| Ваҡыт тамғаһы (UTC) | Актер | Эш / Команда | Һөҙөмтә / Иҫкәрмәләр |
|---------------|---------------------------|------------------|
|  |  |  |  |

> Torii үтенесе идентификаторҙары, өлөшлө хештар, раҫлауҙарҙы өҫтөн ҡуйыу, һәм иҫкәртмәнәсе һылтанмалар индереү.

## 3. Күҙәтеүҙәр һәм метрика

| Метрика | Маҡсат | Күҙәтелгән | Үткәреү/Ут етешһеҙлек | Иҫкәрмәләр |
|-------|--------|-----------|----------|---------|
| Иҫкәртергә яуап латентлығы | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Модерацияны асыҡлау тиҙлеге | `>= <value>` |  |  |  |
| Ҡапҡа аномалияһын асыҡлау | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml` X.
- `Norito manifests:` `<path>`

## 4. Табыштар & Ремедиация

| Ауырлыҡ | Тап | Хужа | Маҡсатлы дата | Статус / Һылтанма |
|---------|----------|----------------------|-----------------||
| Юғары |  |  |  |  |

Документ нисек калибровка күренә, денилисты сәйәсәте, йәки SDK/инструменттар үҙгәрергә тейеш. Һылтанма GitHub/Jira мәсьәләләре һәм иҫкәрмәһе блокировать/блокированный дәүләттәр.

## 5. Идара итеү һәм раҫлауҙар

- **Инцидент командиры ҡул ҡуйыу:** `<name / timestamp>`
- **Идарасы совет тикшерелеүе көнө:** `<meeting id>`
- **Һуңлап тикшерелгән исемлек:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`.

## 6. Ҡушымталар

- `[ ] CLI logbook (`logs/.md`)`.
- `[ ] Dashboard JSON export` X.
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Һәр беркетмәһе менән `[x]` менән бер тапҡыр тейәп дәлилдәр өйөмө һәм SoraFS снимок.

---

_Һуңғы яңыртылған: {{ дата | ғәҙәттәгесә("2026-02-20") }}.