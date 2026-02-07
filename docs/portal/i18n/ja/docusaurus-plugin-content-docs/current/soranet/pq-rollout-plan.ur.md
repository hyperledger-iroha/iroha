---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: SNNet-16G ポスト量子ロールアウト ハンドブック
サイドバー_ラベル: PQ 展開計画
説明: SoraNet ハイブリッド X25519+ML-KEM ハンドシェイク カナリア デフォルト リレー クライアント SDK プロモーション サポート サポート
---

:::note 正規ソース
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ドキュメント セットの引退 دونوں کاپیاں 同期 رکھیں۔
:::

SNNet-16G SoraNet トランスポートのポスト量子ロールアウトの概要`rollout_phase` ノブ演算子 決定的な昇進座標 ステージ A のガード要件 ステージ B のマジョリティ カバレッジ ステージ C の厳格な PQ 姿勢 表面生の JSON/TOML を編集する

プレイブック、カバー、カバー:

- フェーズ定義は設定ノブ (`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`)、コードベースは有線 (`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`) です。
- SDK と CLI フラグのマッピング、クライアントのロールアウト、トラックの追跡
- リレー/クライアントのカナリアのスケジュール管理、ガバナンス ダッシュボード、プロモーション、ゲートの管理 (`dashboards/grafana/soranet_pq_ratchet.json`)。
- ロールバック フックのファイアドリル ランブック参照 ([PQ ラチェット ランブック](./pq-ratchet-runbook.md))。

## フェーズマップ

| `rollout_phase` |効果的な匿名性ステージ |デフォルトの効果 |一般的な使用法 |
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) | فلیٹ کے گرم ہونے تک ہر サーキット کے لئے کم از کم ایک PQ ガード لازم کریں۔ |ベースライン اور ابتدائی Canary ہفتے۔ |
| `ramp` | `anon-majority-pq` (ステージ B) | PQ リレー、バイアス、バイアス、カバレッジ >= 報道古典的なリレーのフォールバック|地域ごとのリレーカナリアSDK プレビューの切り替え|
| `default` | `anon-strict-pq` (ステージ C) | PQ 専用回路はダウングレード アラームを強制します|テレメトリーとガバナンスの承認、プロモーション、プロモーション|

表面明示的 `anonymity_policy` セット コンポーネント フェーズ オーバーライド明示的なステージ、`rollout_phase` 値、遅延、演算子、環境、フェーズ反転、クライアント、継承。

## 構成リファレンス

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator ローダー ランタイムのフォールバック ステージの解決 (`crates/sorafs_orchestrator/src/lib.rs:2229`) `sorafs_orchestrator_policy_events_total` の `sorafs_orchestrator_pq_ratio_*` 表面の解決`docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` すぐに適用できるスニペット دیکھیں۔

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` 解析されたフェーズ レコード (`crates/iroha/src/client.rs:2315`) ヘルパー コマンド (`iroha_cli app sorafs fetch`) フェーズ デフォルトの匿名性ポリシーレポートレポート

## 自動化

`cargo xtask` ヘルパーは生成をスケジュールし、アーティファクト キャプチャを自動化します。

1. **地域ごとのスケジュールが生成されます**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   期間 `s`、`m`、`h`、یا `d` サフィックス قبول کرتے ہیں۔ `artifacts/soranet_pq_rollout_plan.json` マークダウン サマリー (`artifacts/soranet_pq_rollout_plan.md`) を発行します。 変更要求を発行します。

2. **ドリルアーティファクトと署名とキャプチャ**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   処理 `artifacts/soranet_pq_rollout/<timestamp>_<label>/` 処理 処理 アーティファクト 処理 BLAKE3 ダイジェストの計算処理`rollout_capture.json` 記号 جس میں メタデータ ペイロード Ed25519 署名 記号 ہوتا ہے۔秘密鍵 秘密鍵 消防訓練の議事録 ガバナンス 検証 検証

## SDK および CLI フラグ マトリックス

|表面 |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` یا フェーズ پر انحصار | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント設定 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションの `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションの `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションの `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |SDK は、ステージ パーサーとマップの切り替えとオーケストレーターの切り替え (`crates/sorafs_orchestrator/src/lib.rs:365`) 多言語展開の構成フェーズとロックステップの切り替えを行います。いいえ

## カナリアのスケジュール設定チェックリスト

1. **プリフライト (T マイナス 2 週間)**

- ステージ A 停電率 پچھلے دو ہفتوں میں <1% ہے اور PQ カバレッジ فی 地域 >=70% ہے (`sorafs_orchestrator_pq_candidate_ratio`) を確認します。
   - ガバナンス レビュー スロット スケジュール カナリア窓口の承認
   - ステージング `sorafs.gateway.rollout_phase = "ramp"` 更新 (オーケストレーター JSON 編集、再デプロイ) プロモーション パイプライン、ドライラン

2. **リレーカナリア(T日)**

   - 地域のプロモーション `rollout_phase = "ramp"` オーケストレーター 参加リレー マニフェスト セット ہوئے۔
   - PQ Ratchet ダッシュボード「結果ごとのポリシー イベント」「停電率」ガード キャッシュ TTL 監視モニタ (ダッシュボード ロールアウト パネル 監視)
   - 監査ストレージ `sorafs_cli guard-directory fetch` スナップショットの保存

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - クライアント構成 `rollout_phase = "ramp"` を反転する SDK コホート `stage-b` をオーバーライドする
   - テレメトリ差分キャプチャ (`sorafs_orchestrator_policy_events_total` کو `client_id` اور `region` کے حساب سے グループ کریں) ロールアウト インシデント ログ کے接続する

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - ガバナンスのサインオフ - オーケストレーター - クライアント構成 - `rollout_phase = "default"` - スイッチ - 署名済みの準備チェックリスト - アーティファクトのリリース - 回転 - `rollout_phase = "default"`

## ガバナンスと証拠のチェックリスト

|相変化 |プロモーションゲート |証拠の束 |ダッシュボードとアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ステージ A 停電率 14 パーセント <1% `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 昇格地域 Argon2 チケット検証 p95 < 50 ms プロモーション ガバナンス スロットが予約済み| `cargo xtask soranet-rollout-plan` JSON/Markdown ペア `sorafs_cli guard-directory fetch` ペアのスナップショット (前/後) 署名付き `cargo xtask soranet-rollout-capture --label canary` バンドル カナリア議事録 [PQ ラチェット Runbook](./pq-ratchet-runbook.md) を参照やあ| `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + ブラウンアウト率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、`docs/source/soranet/snnet16_telemetry_plan.md` のテレメトリ参照。 |
|ランプ -> デフォルト *(ステージ C 強制)* | 30 日間の SN16 テレメトリ バーンイン `sn16_handshake_downgrade_total` ベースライン フラットクライアント カナリア `sorafs_orchestrator_brownouts_total` プロキシ トグル リハーサル ログ| `sorafs_cli proxy set-mode --mode gateway|direct` トランスクリプト、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml` 出力、`sorafs_cli guard-directory verify` ログ、署名付き `cargo xtask soranet-rollout-capture --label default` バンドル| PQ ラチェット ボード SN16 ダウングレード パネル `docs/source/sorafs_orchestrator_rollout.md` `dashboards/grafana/soranet_privacy_metrics.json` 文書化|
|緊急降格/ロールバックの準備 |トリガー ダウングレード カウンタのスパイク ガード ディレクトリ検証の失敗 `/policy/proxy-toggle` バッファ ダウングレード イベントの発生| `docs/source/ops/soranet_transport_rollback.md` チェックリスト `sorafs_cli guard-directory import` / `guard-cache prune` ログ `cargo xtask soranet-rollout-capture --label rollback` インシデント チケット 通知テンプレート| `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json`、アラート パック (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |

- アーティファクト `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ストア 生成された `rollout_capture.json` ガバナンス パケット スコアボード promtool トレース ダイジェストوں۔
- アップロードされた証拠 (議事録 PDF、キャプチャ バンドル、ガード スナップショット) SHA256 ダイジェスト プロモーション議事録 添付 議会承認 ステージング クラスター 再実行
- プロモーション チケット、テレメトリ プラン、`docs/source/soranet/snnet16_telemetry_plan.md` ダウングレード語彙、アラートしきい値、正規ソース

## ダッシュボードとテレメトリの更新

`dashboards/grafana/soranet_pq_ratchet.json` 「ロールアウト計画」注釈パネル 船の状態 プレイブック リンク 状態 現在の段階 ガバナンス レビュー アクティブ ステージ٩ی تصدیق کر سکیں۔パネルの説明 設定ノブの説明 同期の確認

アラート ルール `stage` ラベル カナリア デフォルト フェーズ ポリシーしきい値トリガー(`dashboards/alerts/soranet_handshake_rules.yml`)。

## ロールバックフック

### デフォルト -> ランプ (ステージ C -> ステージ B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` オーケストレーター、降格 (SDK 構成、位相ミラー、ステージ B) ステージ B 艦隊、デモート、ステージ Bうーん
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` クライアントの安全な輸送プロファイルの記録記録のキャプチャ `/policy/proxy-toggle` 修復ワークフローの監査可能記録
3. `cargo xtask soranet-rollout-capture --label rollback-default` ガード ディレクトリの差分、promtool の出力、ダッシュボードのスクリーンショット、`artifacts/soranet_pq_rollout/` アーカイブ アーカイブ### ランプ -> カナリア (ステージ B -> ステージ A)

1. プロモーション キャプチャ セキュリティ ガード ディレクトリ スナップショット `sorafs_cli guard-directory import --guard-directory guards.json` セキュリティ インポート セキュリティ `sorafs_cli guard-directory verify` セキュリティ セキュリティ降格パケットのハッシュ値
2. Orchestrator クライアント構成 `rollout_phase = "canary"` セット (`anonymity_policy stage-a` オーバーライド) [PQ ラチェット Runbook](./pq-ratchet-runbook.md) PQ ラチェット ドリルの繰り返しパイプラインのダウングレードが証明される
3. 更新された PQ ラチェット、SN16 テレメトリのスクリーンショット、アラート結果、インシデント ログ、添付ファイル、ガバナンス、通知、通知

### ガードレールのリマインダー

- 降格 `docs/source/ops/soranet_transport_rollback.md` 参照 一時的な緩和 ロールアウト トラッカー `TODO:` ログ フォローアップ フォローアップああ
- `dashboards/alerts/soranet_handshake_rules.yml` セキュリティ `dashboards/alerts/soranet_privacy_rules.yml` ロールバック セキュリティ `promtool test rules` カバレッジ セキュリティ ドリフト キャプチャ バンドル アラート ドリフト キャプチャ バンドルドキュメント ہو۔