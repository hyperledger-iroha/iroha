---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
タイトル: Заметки о переходе Nexus
説明: Зеркало `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Phase B, график аудита и митигации.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Заметки о переходе Nexus

**フェーズ B - Nexus 移行基盤** チェックリストを確認してください。 `roadmap.md` のマイルストーン、B1 ～ B4 のマイルストーン、ガバナンス、 SRE と SDK がサポートされています。

## Область и cadence

- ルーテッド トレース、テレメトリ ガードレール (B1/B2)、構成デルタ、ガバナンス (B3)、マルチ レーン打ち上げリハーサル (B4) のフォローアップを行います。
- Заменяет временную cadence note、которая была здесь раньбе; 2026 年第 1 四半期に `docs/source/nexus_routed_trace_audit_report_2026q1.md` が発表され、その結果が発表されました。 митигаций。
- ルーテッド トレース、ガバナンス、打ち上げリハーサルを開始します。ダウンストリーム ドキュメント (ステータス、ダッシュボード、SDK ポータル) を参照し、ダウンストリーム ドキュメント (ステータス、ダッシュボード、SDK ポータル) を確認します。テレカ。

## Снимок доказательств (2026 Q1-Q2)

| Поток | Доказательства |所有者 | Статус | Примечания |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance | Заверøено (2026 年第 1 四半期) | Зафиксированы три окна аудита; TLS は `TRACE-CONFIG-DELTA` で第 2 四半期の再実行を行います。 |
| **B2 - テレメトリ修復とガードレール** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops | Завервено |アラート パック、差分ボットおよび OTLP バッチ (`nexus.scheduler.headroom` ログ + ヘッドルーム Grafana) 。権利放棄。 |
| **B3 - 構成デルタの承認** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance | Завервено | Голосование GOV-2026-03-19 зафиксировано;テレメトリ パックのバンドルです。 |
| **B4 - マルチレーン発射リハーサル** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core | Заверøено (2026 年第 2 四半期) | Q2 Canary 再実行 митигацию TLS лага;バリデータ マニフェスト + `.sha256` фиксирует слот-диапазон 912-936、ワークロード シード `NEXUS-REH-2026Q2` およびハッシュ профиля TLS および再実行。 |

## ルートトレース аудита のルート

|トレースID | Окно (UTC) | Результат | Примечания |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Пройдено |キュー受付 P95 оставался значительно ниже цели <=750 ミリ秒。 Действий не требуется。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Пройдено | OTLP リプレイ ハッシュ приложены к `status.md`;パリティ SDK diff ボットのドリフト。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Резено | TLS と第 2 四半期の再実行。テレメトリ パック `NEXUS-REH-2026Q2` ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (最低 `artifacts/nexus/tls_profile_rollout_2026q2/`) および отстающих。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Пройдено |ワークロード シード `NEXUS-REH-2026Q2`;テレメトリ パック + マニフェスト/ダイジェスト × `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 912 ～ 936)、アジェンダ × `artifacts/nexus/rehearsals/2026q2/`。 |

Будущие кварталы должны добавлять новые строки и переносить заверложение, когда таблица Перерастет текущий квартал。ルーテッド トレース、ガバナンス議事録、`#quarterly-routed-trace-audit-schedule` を参照してください。

## Митигации およびバックログ|アイテム | Описание |オーナー | Цель | Статус / Примечания |
|------|---------------|----------|----------|-----|
| `NEXUS-421` | TLS が `TRACE-CONFIG-DELTA` で確認され、証拠が再実行され、再実行されます。 | @release-eng、@sre-core | 2026 年第 2 四半期のルーテッド トレース окно | Закрыто - ハッシュ TLS は `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` と `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`;再放送 отсутствие отстающих。 |
| `TRACE-MULTILANE-CANARY` 準備 |第 2 四半期リハーサル、フィクスチャ、テレメトリ パック、および SDK ハーネスが含まれるヘルパー。 | @telemetry-ops、SDK プログラム | Планерка 2026-04-30 |メタデータ スロット/ワークロードの議題 - `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md`。トラッカーのハーネスを再利用します。 |
|テレメトリ パック ダイジェスト ローテーション | `scripts/telemetry/validate_nexus_telemetry_pack.py` リハーサル/リリースとトラッカー設定デルタのダイジェストを表示します。 | @telemetry-ops | На каждый リリース候補 |説明 - `telemetry_manifest.json` + `.sha256` と `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`)。トラッカーとダイジェストを表示します。 |

## 構成デルタバンドル

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим 概要の差分。 `defaults/nexus/*.toml` のジェネシス、トラッカー、トラッカーの追跡、`defaults/nexus/*.toml` の追跡здесь。
- リハーサル テレメトリ パックの構成バンドルを追加します。パック、валидированный `scripts/telemetry/validate_nexus_telemetry_pack.py`、должен публиковаться вместе с доказательствами config delta、чтобы операторы могли B4 にアクセスしてください。
- バンドル Iroha 2 つのレーン: configs с `nexus.enabled = false` теперь отклоняют はレーン/データスペース/ルーティングをオーバーライドします。Nexus профиль (`--sora`)、поэтому удаляйте секции `nexus.*` из 単一車線。
- ガバナンス (GOV-2026-03-19) の監視、トラッカー、監視、監視、監視Сопировать формат без повторного поиска ритуала одобрения。

## フォローアップ、打ち上げリハーサル

- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует canary план、名簿のロールバック。ランブックはレーンとエクスポータを備えています。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` は、第 2 四半期の準備メモ/議題を表示します。トラッカーを追跡し、トラッカーを追跡し、追跡を追跡します。
- OTLP コレクター スニペットと Grafana エクスポート (`docs/source/telemetry.md`) およびバッチ ガイダンス エクスポーター。第 1 四半期のバッチ サイズは 256 サンプル、ヘッドルーム アラートは最大です。
- CI/テスト マルチレーンのテスト `integration_tests/tests/nexus/multilane_pipeline.rs` とワークフロー `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)、テスト`pytests/nexus/test_multilane_pipeline.py`;ハッシュ `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) リハーサル バンドルのトラッカーを確認します。

## ランタイムレーンのライフサイクル

- ランタイム レーンのライフサイクルとデータスペース バインディング、クラ/階層型ストレージの調整をサポートします。ヘルパーは、廃止されたレーンのレーン リレー、証明を行うマージ元帳合成をサポートします。
- Nexus 構成/ライフサイクル ヘルパー (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) のレーン/レーンの説明。ルーティング、TEU スナップショット、マニフェスト レジストリなどの情報を確認できます。
- データスペースとストレージ ルート、ディレクトリ (階層型コールド ルート/クラ レーン ディレクトリ) の詳細。 Исправьте базовые пути и повторите;テレメトリの差分レーン/データスペース、ダッシュボードなどの機能を利用できます。

## NPoS телеметрия と доказательства バックプレッシャー

フェーズ B の打ち上げリハーサル、キャプチャー、ペースメーカー NPoS とゴシップの会話背圧。 `integration_tests/tests/sumeragi_npos_performance.rs` のハーネスと JSON 概要 (`sumeragi_baseline_summary::<scenario>::...`) の概要メチャク。 Запусклокально:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Установите `SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K` または `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`、чтобы исследовать более стрессовые топологии; 1 秒/`k=3`、B4 にアクセスできます。| Сценарий / テスト | Покрытие | Ключевая телеметрия |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | 12 日のリハーサル ブロック時間、EMA レイテンシー エンベロープ、冗長送信ゲージの証拠バンドルを確認します。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |入場延期と容量/飽和度を確認してください。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |ペースメーカーのジッターとタイムアウトの表示、+/-125 パーミルの値を表示します。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` | RBC ペイロードをソフト/ハード ストアに保存し、セッション/バイトを保存します。店。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |冗長送信率を測定し、コレクタ オン ターゲットをカウンターし、エンドツーエンドの通信を行います。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |チャンク、バックログ、障害、ペイロードを監視します。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

JSON の例、ハーネスの作成、Prometheus スクレイピング、ガバナンスの作成、管理の実行リハーサル中に背圧アラームを鳴らしたり、背圧アラームを鳴らしたりすることもできます。

## Чеклист обновления

1. ルートトレースのルートトレースを実行します。
2. Обновляйте таблицу митигаций после каждого フォローアップ Alertmanager、даже если действие - закрыть тикет。
3. 設定デルタ、プル リクエスト、トラッカー、テレメトリ パックのダイジェストを実行します。
4. リハーサル/テレメトリーのロードマップを確認するアドホックな機能です。

## Индекс доказательств

| Актив | Локация | Примечания |
|------|----------|------|
|ルーティングトレース監査レポート (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |フェーズ B1。 `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` を参照してください。 |
|構成デルタトラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA の差分概要、レビュアー、GOV-2026-03-19 を参照してください。 |
|テレメトリ修復計画 | `docs/source/nexus_telemetry_remediation_plan.md` |アラート パック、OTLP バッチ サイズ、エクスポート予算ガードレール、B2 など。 |
|マルチレーンリハーサルトラッカー | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 番目のファイル、バリデーターマニフェスト/ダイジェスト、Q2 メモ/議題、およびロールバック証拠。 |
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |スロット範囲 912 ～ 936、シード `NEXUS-REH-2026Q2` およびガバナンス バンドルのハッシュ。 |
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |ハッシュ утвержденного TLS профиля、захваченный в Q2 rerun;ルートトレースの付録を参照してください。 |
| TRACE-MULTILANE-CANARY アジェンダ | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 リハーサル (ウィンドウ、スロット範囲、ワークロード シード、アクション所有者) を開始します。 |
|打ち上げリハーサル ランブック | `docs/source/runbooks/nexus_multilane_rehearsal.md` |ステージング -> 実行 -> ロールバック。レーンとガイダンスエクスポーターをサポートします。 |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI、地下4階。ダイジェストとトラッカーのパックです。 |
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` では、マルチレーン構成、Sora カタログ ハッシュとレーンローカルの Kura/merge-log パス (`blocks/lane_{id:03}_{slug}`) の説明、`ConfigLaneRouter` の説明を参照してください。アーティファクトのダイジェスト。 |