---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: SNNet-16G 後の展開のハンドブック
Sidebar_label: 導入計画 PQ
説明: リレー、クライアント、SDK のデフォルトをサポートするハンドシェイク ハイブリッド X25519+ML-KEM の SoraNet の操作ガイド。
---

:::note ソースカノニク
:::

SNNet-16G は、SoraNet の輸送後の展開を終了します。ノブ `rollout_phase` は、JSON/TOML ブリュット プール チャク サーフェスの修飾子を持たずに、ステージ A 対クーベルチュール マジョリテール ステージ B および厳格な PQ ステージ C をサポートします。

Ce プレイブックの内容:

- コードベース (`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`) による位相およびヌーボー ノブの構成 (`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`) ケーブルの定義。
- クライアントのロールアウトに関するフラグ SDK と CLI のマッピング。
- カナリア リレー/クライアントのスケジューリングに加え、プロモーションのガバナンスに関するダッシュボードにも注意を払う (`dashboards/grafana/soranet_pq_ratchet.json`)。
- ロールバックのフックとファイアドリル Runbook の参照 ([PQ ラチェット Runbook](./pq-ratchet-runbook.md))。

## カルテ・デ・フェーズ

| `rollout_phase` |有効なエテープ・ダノニム |デフォルトの効果 |使用例 |
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) | Exiger au moins unguard PQ パーサーキットペンダント que la flotte se rechauffe。 |ベースラインとカナリアの初演。 |
| `ramp` | `anon-majority-pq` (ステージ B) | PQ の選択とリレーの優先順位 >= クーベルチュールの 2 段階目。クラシックはフォールバックで保持されます。 |地域ごとのカナリアリレー。 SDK のプレビューを切り替えます。 |
| `default` | `anon-strict-pq` (ステージ C) |回路の独自性を侵害する者は、PQ およびダウングレードの警告を発します。 |プロモーションの最終的なテレメトリとサインオフのガバナンスが完了します。 |

表面は、`anonymity_policy` で定義されており、位相を注ぐ構成要素をオーバーライドします。 Omettre は、`rollout_phase` の管理者が環境とクライアントの遺産を管理するために、明示的に保守を延期します。

## 設定のリファレンス

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

オーケストレーターのローダーは、実行時のフォールバック (`crates/sorafs_orchestrator/src/lib.rs:2229`) を取得し、`sorafs_orchestrator_policy_events_total` および `sorafs_orchestrator_pq_ratio_*` 経由で公開します。 Voir `docs/examples/sorafs_rollout_stage_b.toml` et `docs/examples/sorafs_rollout_stage_c.toml` は、アップリケのスニペットを作成します。

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` フェーズ解析の保守管理者 (`crates/iroha/src/client.rs:2315`) のコマンドヘルパー (`iroha_cli app sorafs fetch` の例) の定期的なレポーター、フェーズ解析の匿名性の保証を登録します。

## 自動化

Deux ヘルパー `cargo xtask` は、スケジュールの生成とアーティファクトのキャプチャを自動化します。

1. **地域ごとの一般的なスケジュール**

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

   受諾可能なレッサーフィックスは `s`、`m`、`h` または `d` です。 `artifacts/soranet_pq_rollout_plan.json` とマークダウン再開 (`artifacts/soranet_pq_rollout_plan.md`) を変更要求に参加してコマンドを実行します。

2. **ドリル・アベック署名の捕獲者レ・アーティファクト**

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

   `artifacts/soranet_pq_rollout/<timestamp>_<label>/` のコマンドをコピーし、BLAKE3 のダイジェストを計算して、アーティファクトとエクリット `rollout_capture.json` の平均メタデータと署名 Ed25519 シュール ペイロードを追加します。管理の安全性を確認するために、防火訓練に必要な数分間の秘密キーを利用して、キャプチャを実行します。

## フラグの行列 SDK および CLI|表面 |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` 段階的にリポジトリを使用します | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント構成 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションネル `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションネル `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションネル `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` または `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

これにより、オーケストレーター (`crates/sorafs_orchestrator/src/lib.rs:365`) を利用したステージでの SDK マッピングとミーム パーサーの切り替えが行われ、ロック ステップの平均的なフェーズ構成を保持する複数の言語の展開が可能になります。

## カナリアのスケジュール設定のチェックリスト

1. **プリフライト (T マイナス 2 週間)**

- ステージ A の予測値が 1% 未満であること、および PQ の予測値が地域ごとに 70% 以上であることを確認します (`sorafs_orchestrator_pq_candidate_ratio`)。
   - ガバナンス審査の計画立案者は、ラ・フェネトレ・カナリアを承認します。
   - `sorafs.gateway.rollout_phase = "ramp"` のステージング (JSON オーケストレーターの編集と再デプロイ) とプロモーションのパイプラインの予行演習を実行します。

2. **リレーカナリア(T日)**

   - 主催者およびリレー参加者のマニフェストに関する `rollout_phase = "ramp"` に関する地域のプロモウヴォワール。
   - ダッシュボード PQ ラチェット (メンテナンス、パネル ロールアウトを含む) ペンダント、TTL ガード キャッシュの監視「結果ごとのポリシー イベント」と「ブラウンアウト率」。
   - スナップショット `sorafs_cli guard-directory fetch` のキャプチャーは、在庫監査を行う前に実行します。

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - Basculer `rollout_phase = "ramp"` は設定クライアントをオーバーライドし、`stage-b` は SDK のコホートを指定します。
   - テレメトリの差分 (`client_id` および `region` グループの `sorafs_orchestrator_policy_events_total`) およびロールアウトのインシデント ログに参加するキャプチャー。

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - ガバナンス検証者、バスキュラー オーケストレーター、およびクライアント対 `rollout_phase = "default"` と構成チェックリストの準備署名者とリリースのアーティファクトを公正に管理します。

## チェックリストのガバナンスと証拠

|段階的な変化 |ゲートdeプロモーション |証拠の束 |ダッシュボードとアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ブラウンアウト率ステージ A <1% sur les 14 derniers jours、`sorafs_orchestrator_pq_candidate_ratio` >= 0.7 パー リージョン プロミュー、Argon2 チケット検証 p95 < 50 ms、スロット ガバナンス リザーブ。 | JSON/Markdown `cargo xtask soranet-rollout-plan` のペア、スナップショットの表示 `sorafs_cli guard-directory fetch` (avant/apres)、バンドル署名 `cargo xtask soranet-rollout-capture --label canary`、その他のカナリア参照 [PQ ラチェット Runbook](./pq-ratchet-runbook.md)。 | `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + ブラウンアウト率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、`docs/source/soranet/snnet16_telemetry_plan.md` のテレメトリを参照します。 |
|ランプ -> デフォルト *(ステージ C 強制)* |バーンイン テレメトリ SN16 デ 30 時間、`sn16_handshake_downgrade_total` プラットホーム ベースライン、`sorafs_orchestrator_brownouts_total` ゼロデュラント ル カナリア クライアント、およびリハーサル デュ プロキシ トグル ログ。 |転写 `sorafs_cli proxy set-mode --mode gateway|direct`、出撃 `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`、ログ `sorafs_cli guard-directory verify`、およびバンドル署名 `cargo xtask soranet-rollout-capture --label default`。 | Meme tableau PQ Ratchet plus les panneaux SN16 ダウングレード ドキュメント ダン `docs/source/sorafs_orchestrator_rollout.md` および `dashboards/grafana/soranet_privacy_metrics.json`。 |
|緊急降格/ロールバックの準備 |ダウングレード モンテントのコンペティションを決定し、ガード ディレクトリのエコーを確認し、バッファ `/policy/proxy-toggle` でダウングレード イベントを登録します。 |チェックリスト `docs/source/ops/soranet_transport_rollback.md`、ログ `sorafs_cli guard-directory import` / `guard-cache prune`、`cargo xtask soranet-rollout-capture --label rollback`、インシデントのチケットと通知のテンプレート。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json` および詳細パック (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |- Stockez のアーティファクト `artifacts/soranet_pq_rollout/<timestamp>_<label>/` 平均 `rollout_capture.json` は、パレットのガバナンス コンテンツのスコアボード、プロムツールのトレース、およびダイジェストを参照します。
- SHA256 des preuves 料金のダイジェストを添付します (議事録 PDF、キャプチャ バンドル、ガード スナップショット) 議会の承認を得るための昇進議事録、クラスター ステージングへのアクセスなし。
- テレメトリーの計画とプロモーションのチケット `docs/source/soranet/snnet16_telemetry_plan.md` を参照し、ダウングレードおよび詳細な語彙の情報源を参照してください。

## ダッシュボードとテレメトリを 1 時間見てみよう

`dashboards/grafana/soranet_pq_ratchet.json` には、パネルの注釈「ロールアウト計画」の保守担当者が含まれており、プレイブックに対する監視と、ガバナンスの確認段階での審査の段階での実際の公開が行われます。 Gardez は、パネル同期の説明と、将来の進化とノブの設定を説明します。

警告を発し、カナリアの段階と政治のデフォルトのしきい値を制限する有効なラベル `stage` を保証します (`dashboards/alerts/soranet_handshake_rules.yml`)。

## ロールバックのフック

### デフォルト -> ランプ (ステージ C -> ステージ B)

1. Retrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (設定 SDK からのミームフェーズとフェイトミロイター) は、ステージ B をフルで再現します。
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` のクライアントとトランスポートのプロファイルを強制し、修復ワークフローの記録をキャプチャーします `/policy/proxy-toggle` は監査可能です。
3. `cargo xtask soranet-rollout-capture --label rollback-default` を実行して、アーカイバの差分ガード ディレクトリ、プロムツールなどのダッシュボードのスクリーンショット `artifacts/soranet_pq_rollout/` を実行します。

### ランプ -> カナリア (ステージ B -> ステージ A)

1. ハッシュを含むスナップショット ガード ディレクトリ キャプチャの事前プロモーション `sorafs_cli guard-directory import --guard-directory guards.json` と `sorafs_cli guard-directory verify` のパケットの降格をインポートします。
2. オーケストレーターおよび構成クライアントを使用して `rollout_phase = "canary"` (`anonymity_policy stage-a` をオーバーライド) を定義し、[PQ ラチェット ランブック](./pq-ratchet-runbook.md) でパイプラインのダウングレードを実行して PQ ラチェット ドリルを実行します。
3. 添付のスクリーンショットは、PQ Ratchet および SN16 テレメトリの結果を記録し、インシデントの事前通知ガバナンスを実現します。

### 懸垂下降用ガードレール

- 参照 `docs/source/ops/soranet_transport_rollback.md` は、ロールアウト トラッカーをプールするアイテム `TODO:` を参照して、降格と一時的な緩和を記録します。
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` と `dashboards/alerts/soranet_privacy_rules.yml` のクーベルチュール `promtool test rules` は、アベレージ キャプチャ バンドルを取得する前にロールバックを実行します。