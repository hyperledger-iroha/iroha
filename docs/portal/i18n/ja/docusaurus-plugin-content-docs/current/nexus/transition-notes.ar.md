---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
タイトル: ملاحظات انتقال Nexus
説明: `docs/source/nexus_transition_notes.md` のテスト、フェーズ B のテスト。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات انتقال Nexus

**フェーズ B - Nexus 移行基盤** マルチレーン。 `roadmap.md` ويحفظ الادلة المشار اليها في B1-B4 في مكان واحد حتى تتمكن SRE と SDK の統合が完了しました。

## いいえ

- ルートトレース ガードレール (B1/B2) デルタ デルタ (B3)レーン (B4)。
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ 2026 年第 1 四半期 يوجد التقرير المفصل في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما تحتفظ هذه الصفحة بالجدولありがとうございます。
- ルートトレースを追跡します。アーティファクトの情報 (ステータス、ダッシュボード、 SDK) をダウンロードしてください。

## قطة ادلة (2026 年第 1 四半期～第 2 四半期)

|重要な情報 |ああ |ああ |ああ |重要 |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance |重要 (2026 年第 1 四半期) |ニュース ニュース ニュースTLS `TRACE-CONFIG-DELTA` は Q2 を再実行します。 |
| **B2 - テレメトリ修復とガードレール** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops |重要 |アラート パックの差分ボット OTLP (`nexus.scheduler.headroom` ログ + ヘッドルーム Grafana)権利を放棄します。 |
| **B3 - 構成デルタの承認** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance |重要 | GOV-2026-03-19 をご覧くださいテレメトリ パックを使用してください。 |
| **B4 - マルチレーン発射リハーサル** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core |重要 (2026 年第 2 四半期) |第 2 四半期の再実行、TLS の再実行検証バリデータ マニフェスト + `.sha256` スロット 912 ～ 936 ワークロード シード `NEXUS-REH-2026Q2` ハッシュ TLS 再実行。 |

## ルーテッドトレースを追跡する

|トレースID |世界時間 (UTC) |認証済み |重要 |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |ナギナ |キュー許可 P95 は 750 ミリ秒以下です。ああ、それは。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |ナギナ | OTLP リプレイ ハッシュは `status.md` です。パリティ、SDK の差分ボット、ドリフト。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |ニュース | TLS で Q2 を再実行テレメトリ パックは、`NEXUS-REH-2026Q2` ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) をテストします。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 |ナギナ |ワークロード シード `NEXUS-REH-2026Q2`;テレメトリ パック + マニフェスト/ダイジェスト في `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 912-936) のアジェンダ في `artifacts/nexus/rehearsals/2026q2/`。 |

يجب على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما يتجاوزああ、それは。 Routed-trace を使用して、`#quarterly-routed-trace-audit-schedule` を実行します。

## バックログ

|認証済み |ああ |ああ |ああ | और देखें
|------|---------------|----------|----------|-----|
| `NEXUS-421` | TLS で `TRACE-CONFIG-DELTA` を再実行してください。 | @release-eng、@sre-core |ルートトレース 2026 年第 2 四半期 |ハッシュ - ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ハッシュ `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`;再放送、再放送。 |
| `TRACE-MULTILANE-CANARY` 準備 | Q2 フィクスチャと SDK ハーネスを確認します。 | @telemetry-ops、SDK プログラム |ニュース 2026-04-30 |重要 - 議題 `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` メタデータ スロット/ワークロードハーネス トラッカーを追跡します。 |
|テレメトリ パック ダイジェスト ローテーション | `scripts/telemetry/validate_nexus_telemetry_pack.py` リリース/リリースは、トラッカーと構成デルタをダイジェストします。 | @telemetry-ops |リリース候補 | - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`)。これは、トラッカーをダイジェストします。 |

## 構成デルタ- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 日本語版。開発者は `defaults/nexus/*.toml` を開発し、ジェネシスを追跡し、追跡者を追跡します。
- 署名された構成バンドルをダウンロードします。 `scripts/telemetry/validate_nexus_telemetry_pack.py`、設定デルタ、設定デルタ、および設定デルタB4 のアーティファクトを表示します。
- Iroha 2 レーン: 構成 `nexus.enabled = false` レーン/データスペース/ルーティングをオーバーライドします。 Nexus (`--sora`) は単一レーンです。
- 追跡者 (GOV-2026-03-19) 追跡者追跡者を追跡します。重要な問題は、次のとおりです。

## متابعات تدريب الاطلاق

- يسجل `docs/source/runbooks/nexus_multilane_rehearsal.md` セキュリティ ロールバックランブックとレーン、輸出業者。
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل artefact تم التحقق منه في تدريب 9 ابريل ويشمل الان ملاحظات/agenda تحضير Q2.追跡者は追跡者を追跡し、追跡者は追跡者を追跡します。
- スニペットの OTLP とエクスポート Grafana (`docs/source/telemetry.md`) のバッチ処理とエクスポーターの処理第 1 四半期のバッチ サイズは 256 ドル、ヘッドルームは 256 ドルです。
- CI/テストのテスト マルチレーンのテスト `integration_tests/tests/nexus/multilane_pipeline.rs` ワークフロー `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) のテスト`pytests/nexus/test_multilane_pipeline.py`;ハッシュ `defaults/nexus/config.toml` (`nexus.enabled = true`、blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) のトラッカー バンドルをバンドルします。

## レーン في وقت التشغيل

- レーンの管理、バインディングの管理、データスペースの管理、調整の実行クラ/التخزين الطبقي، مع ترك الكتالوج دون تغيير。ヘルパーはリレー、レーン、証明、マージ元帳の合成を行います。
- ヘルパーの構成/ライフサイクル Nexus (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) のレーン数認証済みルーティング、TEU スナップショット、マニフェスト レジストリの管理。
- バージョン: データスペースのデータスペース、ストレージ ルート、およびストレージ ルート (階層型コールド ルート/クラ)レーン）。ログインしてください。レーン/データスペースとダッシュボードの比較。

## تيليمتري NPoS のバックプレッシャー

フェーズB フェーズB ペースメーカー ペースメーカー NPOS ゴシップ背圧がかかります。ハーネス `integration_tests/tests/sumeragi_npos_performance.rs` 認証 JSON (`sumeragi_baseline_summary::<scenario>::...`) 認証ありがとう。意味:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

ضبط `SUMERAGI_NPOS_STRESS_PEERS` و`SUMERAGI_NPOS_STRESS_COLLECTORS_K` او `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات اشد ضغطا؛ 1 s/`k=3` المستخدم في B4.

|テスト | テスト | और देखेंニュース | ニュース
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | 12 ブロック タイム、エンベロープ、EMA レイテンシー、ゲージ、冗長送信バンドルああ。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |入学延期と入学許可の延期。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |ジッターとペースメーカーのタイムアウトを表示し、+/-125 パーミルを表示します。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` |ペイロード RBC のデータ ソフト/ハード ストアのデータ バイト数ストア。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |ゲージの冗長送信、コレクタ オン ターゲットの監視。エンドツーエンド。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |チャンク数とバックログ数、フォールト数、ペイロード数を確認します。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

JSON ハーネス スクレイピング Prometheus セキュリティ セキュリティ JSON ハーネス スクレイピング Prometheus セキュリティバックプレッシャーを軽減します。

## قائمة التحديث1. ルートトレースのルートトレースを追跡します。
2. アラートマネージャーの管理と管理。
3. 設定デルタ、トラッカー、ダイジェスト、テレメトリ パック、プル リクエスト。
4. アーティファクト/アーティファクト/アーティファクト/アーティファクト/アーティファクト/アーティファクト/アーティファクト/ロードマップ/ロードマップアドホックなセキュリティ。

## فهرس الادلة

|ああ |ああ |重要 |
|------|----------|------|
|ルートトレース (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |フェーズB1 `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` を参照してください。 |
|トラッカー 構成デルタ | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA のテストは、GOV-2026-03-19 で行われました。 |
|修復 | 修復`docs/source/nexus_telemetry_remediation_plan.md` |アラート パック、OTLP、ガードレール、B2。 |
|トラッカー マルチレーン | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` |アーティファクト 9 マニフェスト/ダイジェスト 検証バリデータ Q2 ロールバック。 |
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |スロット範囲 912 ～ 936、シード `NEXUS-REH-2026Q2`、ハッシュ アーティファクト。 |
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |ハッシュ TLS 解析 Q2 再実行ルートトレースを実行します。 |
| TRACE-MULTILANE-CANARY アジェンダ | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 (スロット範囲、ワークロード シード)。 |
| Runbook のレビュー | `docs/source/runbooks/nexus_multilane_rehearsal.md` |チェックリスト ステージング -> 実行 -> ロールバックレーンと輸出業者。 |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI は、レトロな B4 をサポートします。は、トラッカーの情報をダイジェストします。 |
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` マルチレーンの構成、Sora カタログ ハッシュ、Kura/merge-log、レーン (`blocks/lane_{id:03}_{slug}`)、`ConfigLaneRouter`アーティファクトをダイジェストします。 |