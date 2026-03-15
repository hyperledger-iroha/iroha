---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: Быстрый старт デフォルト レーン (NX-5)
Sidebar_label: デフォルトレーン
説明: フォールバック デフォルト レーン、Nexus、Torii、SDK メソッド、パブリック レーンのレーン ID。
---

:::note Канонический источник
Эта страница отражает `docs/source/quickstart/default_lane.md`。 Лобе копии синхронизированными, пока локализационный прогон не попадет в портал.
:::

# デフォルトレーン (NX-5)

> **ロードマップ:** NX-5 - デフォルトのパブリック レーン。フォールバック `nexus.routing_policy.default_lane`、REST/gRPC эндпоинты Torii および каждый SDK のメソッドопускать `lane_id`、когда трафик относится канонической パブリック レーン。 `/status` とフォールバックを実行し、フォールバックを実行します。 клиента от начала до конца.

## Предварительные требования

- Сборка Sora/Nexus для `irohad` (запуск `irohad --sora --config ...`)。
- Доступ крепозиторию конфигураций、чтобы редактировать секции `nexus.*`。
- `iroha_cli`、настроенный на целевой кластер。
- `curl`/`jq` (説明) ペイロード `/status` と Torii。

## 1. レーンとデータスペースを設定する

レーンとデータスペースを組み合わせて使用できます。 Фрагмент ниже (вырезан из `defaults/nexus/config.toml`) パブリック レーンとエイリアス データスペース:

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

Каждый `index` должен быть уникальным и непрерывным. ID データスペース - 64 個の ID データスペース。 же числовые значения、что индексы lane、для наглядности。

## 2. 問題を解決してください。

Секция `nexus.routing_policy` はフォールバック レーンと переопределять марпрутизацию для конкретных инструкций или префиксов аккаунтов。 `default_lane` および `default_dataspace` を使用して、スケジューラを実行します。ルーターは `crates/iroha_core/src/queue/router.rs` と Torii REST/gRPC をサポートしています。

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


## 3. Запустить ноду с примененной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Нода логирует вычисленную политику марлогирутизации при старте. Любые олибки валидации (отсутствующие индексы、дублирующиеся エイリアス、некорректные ids dataspaces) のゴシップです。

## 4. 統治レーンを維持する

После того как нода онлайн、используйте CLI ヘルパー、чтобы убедиться、что デフォルト レーン запечатана (マニフェスト загружен) および готова к трафику。レーンの位置:

```bash
iroha_cli app nexus lane-report --summary
```

出力例:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

デフォルト レーン、`sealed`、ランブック、ガバナンス レーン、およびタスク。 Флаг `--fail-on-sealed` удобен для CI.

## 5. ステータス ペイロード Torii

`/status` は、レーンを備えたスケジューラーです。 `curl`/`jq`、чтобы подтвердить настроенные значения по умолчанию и проверить, что フォールバックレーンの位置:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

出力例:

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

スケジューラーレーン `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Это подтверждает、что TEU スナップショット、метаданные エイリアスおよびマニフェスト соответствуют конфигурации。ペイロードは Grafana であり、ダッシュボードのレーン取り込みです。

## 6. Проверить дефолтное поведение клиента

- **Rust/CLI.** `iroha_cli` および клиентский クレート Rust の `lane_id`、когда вы не передаете `--lane-id` / `LaneSelector`。キュールーターは `default_lane` にアクセスします。 `--lane-id`/`--dataspace-id` はデフォルト レーンを表示します。
- **JS/Swift/Android.** Последние релизы SDK считают `laneId`/`lane_id` опциональными и fallback-ят на значение, объявленное в `/status`。ステージングと制作、制作、制作、制作、制作、制作、制作、制作、制作аварийные перенастройки.
- **パイプライン/SSE テスト。** Фильтры событий транзакций принимают предикаты `tx_lane_id == <u32>` (см. `docs/source/pipeline.md`)。 `/v1/pipeline/events/transactions` は、этим фильтром, чтобы доказать, что записи, отправленные без явного lane, приходятフォールバック レーン ID。

## 7. 可観測性とガバナンスのフック- `/status` также публикует `nexus_lane_governance_sealed_total` および `nexus_lane_governance_sealed_aliases`、アラート マネージャー メッセージ、レーン теряет マニフェスト。 devnet にアクセスしてください。
- スケジューラとダッシュボード ガバナンス、レーン (`dashboards/grafana/nexus_lanes.json`) のエイリアス/スラッグ、およびそれらの機能。エイリアス、Если вы переименовываете 別名、переименуйте и соответствующие директории Kura、чтобы аудиторы сохраняли детерминированные пути (отслеживается по NX-1)。
- デフォルト レーンのロールバックを解除します。ハッシュ マニフェストとガバナンス クイック スタート、ランブック、タスク ビューを作成します。 требуемое состояние。

Когда эти проверки пройдены, можно считать `nexus.routing_policy.default_lane` источником истины для конфигурации SDK и начинать単一車線の道路を走ることができます。