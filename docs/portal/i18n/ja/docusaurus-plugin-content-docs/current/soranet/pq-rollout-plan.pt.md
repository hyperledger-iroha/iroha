---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: pos-quantico SNNet-16G ロールアウトのプレイブック
Sidebar_label: ロールアウト PQ の計画
説明: ギア オペレーショナル パラ プロムーバー、ハンドシェイク ハイブリド X25519+ML-KEM は、SoraNet をデフォルトのリレー、クライアント、SDK のカナリアとして実行します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/soranet/pq_rollout_plan.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

SNNet-16G は、SoraNet の輸送システムの展開を完了しました。 OS ノブ `rollout_phase` は、OS オペラドールの調整を許可し、プロモートの決定を決定するために必要な条件を守ります。 Stage A では、マジョリタリアのステージで、姿勢 PQ エストリータでステージ C を編集し、JSON/TOML のブルート パラ カダ サーフェスを表示します。

エステ プレイブックのコア:

- デフォルトの OS ノブ設定 (`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`) はコードベースなし (`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`) に定義されています。
- ロールアウトに伴うクライアント パラメータの SDK および CLI にフラグを設定します。
- カナリア リレー/クライアントのガバナンス ダッシュボードのスケジューリングとプロモーションの期待 (`dashboards/grafana/soranet_pq_ratchet.json`)。
- ファイアドリル Runbook のロールバックとリファレンスのフック ([PQ ラチェット Runbook](./pq-ratchet-runbook.md))。

## マパ・デ・ファセス

| `rollout_phase` |エスタジオ デ アノニマト エフェティボ |エフェイト・パドラオ |ウソ・ティピコ |
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) | Exigir ao menosum ガード PQ por 回路 enquanto a frota aquece。 |カナリアの初期設定のベースライン。 |
| `ramp` | `anon-majority-pq` (ステージ B) | PQ com >= 報道を行うために、セレカオ パラ リレーを中継します。リレーclassicos ficam comoフォールバック。 |カナリア諸島の中継地。 SDK のプレビューを切り替えます。 |
| `default` | `anon-strict-pq` (ステージ C) |車の回路では、PQ および耐久装置アラームのダウングレードが可能です。 |最終的なアポス テレメトリとガバナンスの承認を促進します。 |

表面タンベム定義 `anonymity_policy` 明示的に、アクエレ コンポーネントの表面を確認します。 `rollout_phase` パラ ケ オペラドール ポッサム ヴィラールのステージを省略し、周囲の環境やクライアントの状況を考慮します。

## 構成リファレンス

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

O ローダーがオーケストレーターを解決します。 o ランタイム (`crates/sorafs_orchestrator/src/lib.rs:2229`) でフォールバックを段階的に行います。 o `sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_pq_ratio_*` を介して説明します。 Veja `docs/examples/sorafs_rollout_stage_b.toml` と `docs/examples/sorafs_rollout_stage_c.toml` のスニペットは、実際に提供されます。

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` は、正確な分析 (`crates/iroha/src/client.rs:2315`) のヘルパー コマンド (`iroha_cli app sorafs fetch` の例) により、正確な政治的情報を報告します。

## オートマカオ

ヘルパー `cargo xtask` は、アートキャプチャのスケジュールを自動化します。

1. **地域ごとのスケジュール**

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

   期間は、サフィックス `s`、`m`、`h` または `d` です。 `artifacts/soranet_pq_rollout_plan.json` マークダウン (`artifacts/soranet_pq_rollout_plan.md`) を発行して、変更を要求してください。

2. **精力的に活動する捕獲術**

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

   `artifacts/soranet_pq_rollout/<timestamp>_<label>/` のアルキボス フォルネシドスをコマンドしてください。計算ダイジェスト BLAKE3 パラグラフ アーティファトとエスクリーブ `rollout_capture.json` のコンテンツ メタデータと uma assinatura Ed25519 のペイロードを確認してください。ガバナンスの迅速なキャプチャを検証するためにファイアドリルを実行する際には、メスマ秘密キーを使用してください。

## フラグ SDK および CLI のマトリクス

|表面 |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` 確認してください | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント設定 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションの `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションの `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションの `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` または `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |Todos OS は、SDK マップ パラオ メスモ ステージ パーサーを使用してペロ オーケストレーター (`crates/sorafs_orchestrator/src/lib.rs:365`) を切り替え、多言語フィカム エム ロックステップ コムの確実な構成を展開します。

## カナリアのスケジュール設定のチェックリスト

1. **プリフライト (T マイナス 2 週間)**

- ステージ A でのブラウンアウト率が 1% 未満で、コベルチュラ PQ e ≧ 70% であることを確認します (`sorafs_orchestrator_pq_candidate_ratio`)。
   - カナリア諸島で承認されるガバナンス レビューの議題。
   - Atualizar `sorafs.gateway.rollout_phase = "ramp"` のステージング (エディター、オーケストレーター JSON、再デプロイ)、およびパイプラインの予行演習を実行します。

2. **リレーカナリア(T日)**

   - プロモーターが構成を管理する `rollout_phase = "ramp"` オーケストレーターも、リレー参加者のマニフェストもありません。
   - 「結果ごとのポリシー イベント」と「ブラウンアウト率」を監視します。ダッシュボード PQ ラチェット (ロールアウトのアゴラ パネル) または TTL によるガード キャッシュを監視しません。
   - `sorafs_cli guard-directory fetch` のスナップショットは、聴覚施設に保管されています。

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - `rollout_phase = "ramp"` パラメータのトロカールは、クライアントおよびパッサーの `stage-b` パラメータ コホートの SDK 設計をオーバーライドします。
   - テレメトリの差分キャプチャ (`sorafs_orchestrator_policy_events_total` および `client_id` および `region`) およびロールアウト インシデント ログのアネクスト。

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - ガバナンスのサインオフ、`rollout_phase = "default"` の代替オーケストレーター、クライアント設定、および準備チェックリストのローテーション、OS リリース アーティファクトの保管を行います。

## ガバナンスと証拠のチェックリスト

|相変化 |プロモーションゲート |証拠の束 |ダッシュボードとアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ステージ A 電圧低下率 <1%、最大 14 ディアス、`sorafs_orchestrator_pq_candidate_ratio` >= 0.7 por regiao promovida、Argon2 チケット検証 p95 < 50 ms、e スロット ド ガバナンス パラ プロモーション リザード。 | `cargo xtask soranet-rollout-plan` の JSON/Markdown、`sorafs_cli guard-directory fetch` のスナップショット パラメータ (アンテス/デポワ)、バンドル `cargo xtask soranet-rollout-capture --label canary`、カナリア参照 [PQ ラチェット Runbook](./pq-ratchet-runbook.md) の分。 | `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + ブラウンアウト率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、`docs/source/soranet/snnet16_telemetry_plan.md` のテレメトリ参照。 |
|ランプ -> デフォルト *(ステージ C 強制)* |テレメトリ SN16 の 30 日のバーンインが完了、`sn16_handshake_downgrade_total` フラットなしベースライン、`sorafs_orchestrator_brownouts_total` ゼロデュランテ、カナリア、クライアント、リハーサルのプロキシ切り替えログ。 |トランスクリプト `sorafs_cli proxy set-mode --mode gateway|direct`、出力 `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`、ログ `sorafs_cli guard-directory verify`、バンドル アッシナド `cargo xtask soranet-rollout-capture --label default`。 |メスモ ボード PQ ラチェットと OS パネルのダウングレード SN16 ドキュメント、`docs/source/sorafs_orchestrator_rollout.md` および `dashboards/grafana/soranet_privacy_metrics.json`。 |
|緊急降格/ロールバックの準備 |ダウングレード ソベムのカウンターを実行し、ガード ディレクトリ ファルハを検証し、バッファ `/policy/proxy-toggle` レジストラ ダウングレード イベントの継続を確認します。 | `docs/source/ops/soranet_transport_rollback.md` のチェックリスト、`sorafs_cli guard-directory import` / `guard-cache prune`、`cargo xtask soranet-rollout-capture --label rollback` のログ、インシデントのチケットおよび通知テンプレート。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json`、アンボス アラート パック (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |

- Armazene は、`artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` のガバナンス パケット、コンテンツ、スコアボード、promtool トレース、ダイジェストを作成します。
- Anexe は、議会の広報活動の議事録として SHA256 das evidencias carregadas (議事録 PDF、キャプチャ バンドル、ガード スナップショット) をステージング クラスターとしてダイジェストします。
- テレメトリのプラン、プロバール ケ `docs/source/soranet/snnet16_telemetry_plan.md` のプロモーション チケットなし、およびダウングレード アラートしきい値の語彙の送信を参照します。

## ダッシュボードとテレメトリの取得

`dashboards/grafana/soranet_pq_ratchet.json` には、「ロールアウト計画」に関するパネルが含まれており、戦略プレイブックにリンクされ、ガバナンスを確認するためのレビューとしての正確な段階の評価が行われます。 Mantenha は、パネルのシンクロニザダ com mudancas futuras の設定ノブを説明します。

アラートを行うと、ラベル `stage` がカナリアのデフォルトの差別化ポリシーのしきい値ごとに存在することを保証します (`dashboards/alerts/soranet_handshake_rules.yml`)。

## ロールバックフック

### デフォルト -> ランプ (ステージ C -> ステージ B)1. オーケストレーター com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` での降格 (SDK 構成のメスマ ー) は、ステージ B からの最初のタスクに戻ります。
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` 経由でクライアントに安全なトランスポート プロファイルを強制し、キャプチャランド、トランスクリプト パラメータ、ワークフロー `/policy/proxy-toggle` で監査を続行します。
3. arquivar ガード ディレクトリの差分 `cargo xtask soranet-rollout-capture --label rollback-default`、promtool 出力、および `artifacts/soranet_pq_rollout/` のダッシュボード スクリーンショットをロードします。

### ランプ -> カナリア (ステージ B -> ステージ A)

1. ガード ディレクトリのスナップショットをプロモーション用にキャプチャし、`sorafs_cli guard-directory import --guard-directory guards.json` をロードして `sorafs_cli guard-directory verify` ハッシュを含む降格パケットをインポートします。
2. `rollout_phase = "canary"` (`anonymity_policy stage-a` をオーバーライド) を調整して、オーケストレーターとクライアントの構成を変更し、PQ ラチェット ドリルを実行し、[PQ ラチェット ランブック](./pq-ratchet-runbook.md) パラメータとパイプラインのダウングレードを調整します。
3. PQ ラチェットとテレメトリ SN16 の別のスクリーンショットは、通知ガバナンスの事前通知としてインシデント ログとしてアラート結果を表示します。

### ガードレールのリマインダー

- 参照 `docs/source/ops/soranet_transport_rollback.md` は、一時的なアイテムの登録に必要な降格を確認します。 `TODO:` は、コンパニオン用のロールアウト トラッカーがありません。
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura de `promtool test rules` antes e depois de um rollback para que o warning drift fique documentado junto ao キャプチャ バンドル。