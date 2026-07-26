---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: Plan de despliegue poscuantico SNNet-16G
サイドバーラベル: プラン・ド・デスリーグ PQ
説明: リレー、クライアント、SDK のデフォルトで、SoraNet でのハンドシェイク X25519+ML-KEM のプロムーバーとカナリアの動作をサポートします。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/soranet/pq_rollout_plan.md`。満天アンバスコピアスシンクロニザダス。
:::

SNNet-16G は、SoraNet の輸送に必要な情報を提供します。 Los controles `rollout_phase` は、ステージ A の進行状況を決定するための実際のガードの調整を許可します。ステージ A のハスタ ラ コベルトゥーラ マヨリタリア デ ステージ バイ ラ ポストラ PQ 制限のステージ C の罪編集 JSON/TOML の詳細な情報を確認します。

エステのプレイブックキューブ:

- ヌエボ ノブの設定の定義 (`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`)、コードベースのケーブル (`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`)。
- SDK と CLI のフラグをマッピングし、クライアントの展開を確認します。
- カナリア リレー/クライアントのダッシュボードのガバナンスとプロモーションのスケジュール設定に期待します (`dashboards/grafana/soranet_pq_ratchet.json`)。
- ファイアドリルのランブックのロールバックと参照のフック ([PQ ラチェット ランブック](./pq-ratchet-runbook.md))。

## マパ・デ・ファセス

| `rollout_phase` | Etapa de anonimato efectiva |効果と欠陥 |ウソ・ティピコ |
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) |安全な PQ の回路を維持する必要があります。 |カナリアのベースラインとセマナス。 |
| `ramp` | `anon-majority-pq` (ステージ B) | Sesga la seleccion hacia リレー PQ para >= dos tercios de cobertura;リレークラシコス永久コモフォールバック。 |リレー地域のカナリア諸島。 SDK でのプレビューを切り替えます。 |
| `default` | `anon-strict-pq` (ステージ C) |アプリケーション回路は単独の PQ でダウングレードの警告に耐えます。 |テレメトリーとガバナンスの承認を完了するための最終的なプロモーション。 |

`anonymity_policy` を明示的に定義し、コンポーネントを一時的に無効にします。 `rollout_phase` は、オペラドールのプエダン カンビア ラ ファセ ウナ ソラ ヴェズ ポル エントルノ y デジャール ケ ロス クライアント ラ ヘデンで、私たちの勇気を忘れないでください。

## 設定のリファレンス

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

オーケストレーターのローダーは、実行時のフォールバック (`crates/sorafs_orchestrator/src/lib.rs:2229`) および `sorafs_orchestrator_policy_events_total` および `sorafs_orchestrator_pq_ratio_*` 経由でのエラーを解決します。 Ver `docs/examples/sorafs_rollout_stage_b.toml` y `docs/examples/sorafs_rollout_stage_c.toml` のスニペット リストの詳細。

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` アホラ・レジストラ・ラ・ファセ・パーセアダ (`crates/iroha/src/client.rs:2315`) パラ・ケ・ヘルパーズ (por ejemplo `iroha_cli app sorafs fetch`) プエダン・レポーター・ラ・ファセ・実際の政治的事実を報告する。

## 自動化

Dos ヘルパー `cargo xtask` は、スケジュールと成果物のキャプチャを自動的に生成します。

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

   ラス デュラシオン アセプタン スフィホス `s`、`m`、`h` または `d`。コマンドは `artifacts/soranet_pq_rollout_plan.json` を発行し、マークダウン (`artifacts/soranet_pq_rollout_plan.md`) を再開して、カンビオの要求を確認します。

2. **企業のドリルの成果物をキャプチャ**

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

   `artifacts/soranet_pq_rollout/<timestamp>_<label>/` のアーカイブをコピーし、`rollout_capture.json` コンメタデータ マス ウナ ファーム Ed25519 のペイロードを記述した計算ダイジェスト BLAKE3 を参照してください。米国は、ガバナンスの有効性を確認するために、ファイアドリルの緊急秘密キーを迅速に取得します。

## SDK と CLI のフラグのマトリクス|表面 |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` 事前に確認します | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント設定 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションの `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションの `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションの `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` または `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

オーケストレーター (`crates/sorafs_orchestrator/src/lib.rs:365`) で SDK マップのミスモ パーサーを切り替え、ロックステップでの設定を複数言語で行うことができます。

## カナリアのスケジュール設定のチェックリスト

1. **プリフライト (T マイナス 2 週間)**

- ステージ A の海域でブラウンアウトが 1% 未満であることを確認し、地域ごとに PQ 海域が 70% 以上であることを確認します (`sorafs_orchestrator_pq_candidate_ratio`)。
   - カナリア諸島でのガバナンス レビューのプログラムを実行します。
   - 実際の `sorafs.gateway.rollout_phase = "ramp"` のステージング (JSON の編集、オーケストレーターおよび再デプロイ)、およびパイプラインの予行演習。

2. **リレーカナリア(T日)**

   - プロモーターは、リレー参加者のマニフェストを含むオーケストレーターや `rollout_phase = "ramp"` を構成する地域を担当します。
   - ダッシュボード PQ ラチェットの「結果ごとのポリシー イベント」と「ブラウンアウト率」を監視します (パネルのロールアウトを含む) と TTL ガード キャッシュの期間を監視します。
   - `sorafs_cli guard-directory fetch` のスナップショットは、聴衆の避難所からの脱出に役立ちます。

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - Cambiar は、クライアントの設定で `rollout_phase = "ramp"` を設定し、SDK 設計のコホートで `stage-b` をオーバーライドします。
   - テレメトリの差分をキャプチャ (`sorafs_orchestrator_policy_events_total` と `client_id` および `region`)、ロールアウトに関する付属のログ。

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - ガバナンス会社を設立し、クライアントの `rollout_phase = "default"` とロータリー チェックリストのリリース準備状況を確認し、オーケストレーターを構成します。

## ガバナンスと証拠のチェックリスト

|カンビオ デ ファセ |ゲート・ド・プロモーション |証拠のバンドル |ダッシュボードとアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ブラウンアウト ステージ A <1% で 14 ディアス、`sorafs_orchestrator_pq_candidate_ratio` >= 0.7 地域プロモーション、Argon2 チケット検証 p95 < 50 ミリ秒、プロモーション リザーブのスロット ド ガバナンス。 | `cargo xtask soranet-rollout-plan` の JSON/マークダウン、`sorafs_cli guard-directory fetch` のスナップショット (アンテス/デスピュー)、バンドル ファームド `cargo xtask soranet-rollout-capture --label canary`、カナリア参照 [PQ ラチェット Runbook](./pq-ratchet-runbook.md) の詳細。 | `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + ブラウンアウト率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、`docs/source/soranet/snnet16_telemetry_plan.md` のテレメトリ参照。 |
|ランプ -> デフォルト *(ステージ C 強制)* |テレメトリ SN16 の 30 日までのバーンイン、ベースラインでの `sn16_handshake_downgrade_total` プラノ、カナリアのクライアントでの `sorafs_orchestrator_brownouts_total`、リハーサルとプロキシの切り替えレジストラード。 | `sorafs_cli proxy set-mode --mode gateway|direct` の転写、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml` の記録、`sorafs_cli guard-directory verify` のログ、`cargo xtask soranet-rollout-capture --label default` のバンドル。 | Mismo テーブルロ PQ ラチェット パネルのダウングレード SN16 ドキュメントと `docs/source/sorafs_orchestrator_rollout.md` および `dashboards/grafana/soranet_privacy_metrics.json`。 |
|緊急降格/ロールバックの準備 |ダウングレードのサブイベントを実行し、バッファ `/policy/proxy-toggle` レジストラ イベントのガード ディレクトリを検証します。 | `docs/source/ops/soranet_transport_rollback.md` のチェックリスト、`sorafs_cli guard-directory import` / `guard-cache prune`、`cargo xtask soranet-rollout-capture --label rollback` のログ、インシデントのチケット、通知テンプレート。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json` および警告パック (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |- 成果物 `artifacts/soranet_pq_rollout/<timestamp>_<label>/` コンエル `rollout_capture.json` のガバナンス コンテンツ スコアボード、プロムツールのダイジェストを作成します。
- 補助機能は、クラスターのステージングに伴う国会議事堂の宣伝用の SHA256 証拠ダイジェスト (議事録 PDF、キャプチャ バンドル、ガード スナップショット) です。
- プロバーク `docs/source/soranet/snnet16_telemetry_plan.md` に関するテレメトリのプランとプロモーションのチケットを参照し、語彙をダウングレードし、アラートを参照します。

## ダッシュボードとテレメトリの実際の操作

`dashboards/grafana/soranet_pq_ratchet.json` アホラには、アノタシオネスパネルの「ロールアウト計画」が含まれており、エステートプレイブックと実際のガバナンス改訂の確認が必要です。ノブの構成に関するパネルの説明をすべて表示します。

アラートを設定すると、政策 `stage` でカナリア諸島のデフォルトのしきい値が無視されます (`dashboards/alerts/soranet_handshake_rules.yml`)。

## ロールバックのフック

### デフォルト -> ランプ (ステージ C -> ステージ B)

1. オーケストレーター コン `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (SDK の構成を参照) ステージ B でのフロータの実行。
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` 経由でクライアントの輸送を許可し、修復ワークフローの記録をキャプチャし、`/policy/proxy-toggle` で監査可能です。
3. ガード ディレクトリのアーカイブ `cargo xtask soranet-rollout-capture --label rollback-default`、プロムツールとダッシュボードのスクリーンショット `artifacts/soranet_pq_rollout/` を保存します。

### ランプ -> カナリア (ステージ B -> ステージ A)

1. 保護ディレクトリのスナップショットをインポートし、`sorafs_cli guard-directory import --guard-directory guards.json` およびハッシュを含むデモのパケットを取り出します。
2. Ajusta `rollout_phase = "canary"` (`anonymity_policy stage-a` をオーバーライド) オーケストレーターのクライアント構成、ダウングレードのパイプラインのテストで PQ ラチェット ドリルの詳細を繰り返します。
3. PQ ラチェットとテレメトリ SN16 の実際のスクリーンショットは、ガバナンスに通知する前に発生したインシデントのログとしてアラートとして表示されます。

### ガードレールの記録

- 参照 `docs/source/ops/soranet_transport_rollback.md` は、後部トラバホ パラバホ ロールアウト トラッカーの `docs/source/ops/soranet_transport_rollback.md` 登録と一時的な緩和を確認します。
- Mantener `dashboards/alerts/soranet_handshake_rules.yml` と `dashboards/alerts/soranet_privacy_rules.yml` は、`promtool test rules` で、ロールバック パラレル ドリフト アラートとドキュメントおよび 6 月 6 日のキャプチャ バンドルを監視します。