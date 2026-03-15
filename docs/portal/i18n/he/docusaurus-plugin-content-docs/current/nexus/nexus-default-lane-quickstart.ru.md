---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-default-lane-quickstart
כותרת: Быстрый старт נתיב ברירת מחדל (NX-5)
sidebar_label: נתיב ברירת מחדל Быстрый старт
תיאור: Настройте и проверьте מסלול ברירת מחדל ב-Nexus, чтобы Torii ו-SDK могли опускать lane_id בנתיבים ציבוריים.
---

:::note Канонический источник
Эта страница отражает `docs/source/quickstart/default_lane.md`. Держите обе копии синхронизированными, пока локализационный прогон не попадет в портал.
:::

# Быстрый старт נתיב ברירת מחדל (NX-5)

> **מפת דרכים של קונטיקט:** NX-5 - נתיב ציבורי ברירת מחדל. Рантайм теперь предоставляет fallback `nexus.routing_policy.default_lane`, чтобы REST/gRPC эндпоинты Torii и каждый SDK мозглиском `lane_id`, когда трафик относится к канонической הנתיב הציבורי. Это руководство проводит операторов через настройку каталога, проверку fallback в `/status` ו проверку поведенич конца.

## Предварительные требования

- Сборка Sora/Nexus ל-`irohad` (זאפיש `irohad --sora --config ...`).
- רופא עבור репозиторию конфигураций, чтобы редактировать секции `nexus.*`.
- `iroha_cli`, настроенный на целевой кластер.
- `curl`/`jq` (אילו эквивалент) ל- просмотра מטען `/status` в Torii.

## 1. הגדר את נתיב המסלול ומרחב הנתונים

הצג נתיבים ומרחבי נתונים, которые должны существовать в сети. Фрагмент ниже (вырезан из `defaults/nexus/config.toml`) регистрирует три public lane и соответствующие כינוי למרחב נתונים:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Каждый `index` должен быть уникальным и непрерывным. מרחבי מידע מזהים - это 64-битные значения; в примерах выше используются те же числовые значения, что и индексы ליין, для наглядности.

## 2. התקן את התמונות והאופציות

Секция `nexus.routing_policy` управляет fallback lane и позволяет переопределять маршрутизацию для конкретных инкретныцин аккаунтов. Если ни одно правило не подходит, מתזמן מתקדם ב-`default_lane` ו-`default_dataspace`. Логика נתב находится в `crates/iroha_core/src/queue/router.rs` и прозрачно применяет политику поверхностям Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. Запустить ноду с применной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Нода логирует вычисленную политику маршрутизации при старте. Любые ошибки валидации (отсутствующие индексы, дублирующиеся כינוי, некорректные מרחבי נתונים של ids) внаюслыва

## 4. Подтвердить состояние governance для ליין

После того как нода онлайн, используйте CLI עוזר, чтобы убедиться, что מסלול ברירת מחדל запечатана (מניפסט загружен) и гуктовик. Сводный вид выводит по одной строке на ליין:

```bash
iroha_cli app nexus lane-report --summary
```

פלט לדוגמה:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Если מסלול ברירת המחדל показывает `sealed`, следуйте runbook по governance для lanes перед тем, как разрешить внешний трафик. Флаг `--fail-on-sealed` удобен для CI.

## 5. מטען מצב Проверить ToriiОтвет `/status` раскрывает и политику маршрутизации, וסנימוק מתזמן לנתיבים. Используйте `curl`/`jq`. טלמטרי:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

פלט לדוגמה:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Чтобы посмотреть живые счетчики מתזמן לנתיב `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Это подтверждает, что Snapshot TEU, метаданные כינוי ו флаги manifest соответствуют конфигурации. Тот же מטען используется панелями Grafana לבליעה של נתיב לוח המחוונים.

## 6. Проверить дефолтное поведение клиента

- **Rust/CLI.** `iroha_cli` и клиентский ארגז חלודה опускают поле `lane_id`, когда вы не передаете Grafana00002 נתב תור в этом случае падает в `default_lane`. Используйте явные флаги `--lane-id`/`--dataspace-id` только при работе с не-default lane.
- **JS/Swift/Android.** Последние релизы SDK считают `laneId`/`lane_id` אופציונליים ו-fallback-ято звнабия `/status`. Держите политику маршрутизации синхронизированной между בימוי והפקה, чтобы мобильным приложениям невисне перенастройки.
- **בדיקות צינור/SSE.** Фильтры событий транзакций принимают предикаты `tx_lane_id == <u32>` (см. `docs/source/pipeline.md`). הצג את `/v1/pipeline/events/transactions` עם סרטים סרטניים, чтобы доказать, что записи, отправленные без явнопго lapone, fall back מזהה

## 7. התבוננות и משילות ווים

- `/status` также публикует `nexus_lane_governance_sealed_total` и `nexus_lane_governance_sealed_aliases`, чтобы Alertmanager мог предупреждать, котгда Manifest, la manifest. Держите эти алерты включенными даже на devnet.
- מתזמן כרטיסיות וניהול לוח המחוונים לנתיבים (`dashboards/grafana/nexus_lanes.json`) ожидают поля alias/slug из каталога. Если вы переименовываете alias, переименуйте и соответствующие директории Kura, чтобы аудиторы сохрантеринидин (отслеживается по NX-1).
- Парламентские одобрения לנתיבי ברירת המחדל должны включать план החזרה לאחור. מניפסט hash Зафиксируйте hash и доказательства governance рядом с этим התחלה מהירה в вашем операторском runbook, чтобы будущие роталегие состояние.

Когда эти проверки пройдены, можно считать `nexus.routing_policy.default_lane` источником истины для конфигурации SDK и начинать унаследованные חד-נתיב кодовые пути в сети.