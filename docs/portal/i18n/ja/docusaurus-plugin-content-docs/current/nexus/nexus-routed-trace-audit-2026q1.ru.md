---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: Routed-trace の 2026 年第 1 四半期 (B1)
説明: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`、охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
Эта страница отражает `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Аержите обе копии синхронизированными, пока не готовы остальные переводы.
:::

# 2026 年第 1 四半期 (B1) の Routed-Trace を実行します

ロードマップ **B1 - ルーテッド トレース監査およびテレメトリ ベースライン** は、ルーテッド トレース Nexus を示します。 Этот отчет фиксирует окно аудита Q1 2026 (январь-март), чтобы совет по управлению мог утвердить телеметрийную Q2. 質問は次のとおりです。

## Область и таймлайн

|トレースID | Окно (UTC) | Цель |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |複数のレーンでゴシップを話します。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | OTLP リプレイ、差分ボット、SDK および AND4/AND7 を利用できます。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |ガバナンス デルタ `iroha_config` とロールバックが RC1 にあります。 |

ルートトレースのルートトレース (телеметрия) を実行する必要があります。 `nexus.audit.outcome` + счетчики Prometheus)、アラート マネージャーと `docs/examples/` です。

## Методология

1. **Сбор телеметрии.** Все узлы эмитировали структурированное событие `nexus.audit.outcome` и сопутствующие метрики (`nexus_audit_outcome_total*`)。 `scripts/telemetry/check_nexus_audit_outcome.py` は、JSON 形式、`docs/examples/nexus_audit_outcomes/` のペイロードを確認します。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка алертов.** `dashboards/alerts/nexus_audit_rules.yml` は、ペイロードを含むハーネスを表示します。 CI は `dashboards/alerts/tests/nexus_audit_rules.test.yml` を表します。 же правила вручную прогонялись в каждом окне.
3. **Съемка дазордов.** Операторы экспортировали панели routed-trace из `dashboards/grafana/soranet_sn16_handshake.json` (здоровье handshake) および обзорные дазорды телеметрии、чтобы связать здоровье очередей с результатами аудита.
4. **注意事項** 緩和策を確認してください。 [Nexus 移行メモ](./nexus-transition-notes) と трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## Результаты

|トレースID | Итог | Доказательства | Примечания |
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス |射撃/回復 алертов (внутренняя ссылка) + リプレイ `dashboards/alerts/tests/soranet_lane_rules.test.yml`; [Nexus 移行ノート](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) を参照してください。 | P95 は 612 ミリ秒 (750 ミリ秒以下) です。 Дальнействих действий не требуется。 |
| `TRACE-TELEMETRY-BRIDGE` |パス |ペイロード `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` は OTLP リプレイ ハッシュ、`status.md` です。 | SDK リダクション ソルトは Rust ベースラインをサポートします。差分ボットを表示します。 |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) |ガバナンス (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS プロファイル マニフェスト (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリ パック マニフェスト (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`) が必要です。 | Q2 再実行 одобренный TLS と подтвердил отсутствие отстающих;テレメトリ マニフェスト фиксирует диапазон слотов 912-936 およびワークロード シード `NEXUS-REH-2026Q2`。 |

ガードレール アラート マネージャー (`NexusAuditOutcomeFailure` оставался) のトレースを追跡します。 зеленым весь квартал）。

## フォローアップ

- ルーテッド トレース TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`;緩和策 `NEXUS-421` と移行メモ。
- OTLP リプレイと Torii の差分を確認し、テストを実行します。 Android AND4/AND7 に対応しています。
- Убедиться、что предстоящие репетиции `TRACE-MULTILANE-CANARY` используют тот же телеметрийный ヘルパー、чтобы Q2 サインオフ опирался на優れたワークフロー。

## Индекс артефактов

|資産 | Локация |
|------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила と тесты алертов | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|結果ペイロードの例 | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|ルートトレースと接続 | [Nexus 移行メモ](./nexus-transition-notes) |

Этот отчет, артефакты выге и экспорты алертов/телеметрии должны быть приложены к журналу резений ガバナンス, чтобы B1 からのアクセス。