---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: Плейбук постквантового ロールアウト SNNet-16G
Sidebar_label: ロールアウト PQ のロールアウト
説明: X25519+ML-KEM ハンドシェイク、SoraNet、カナリア、デフォルト、リレー、クライアント、SDK の説明。
---

:::note Канонический источник
:::

SNNet-16G は、SoraNet のポスト量子ロールアウトに対応します。ノブ `rollout_phase` は、ステージ A のガード要件、ステージ B の過半数をカバーする、ステージ C の厳格な PQ 姿勢を達成するためのプロモーションを実行します。 JSON/TOML が必要になります。

プレイブックの詳細:

- フェーズと設定ノブ (`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`) とコードベース (`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`) を組み合わせます。
- SDK および CLI フラグ、クライアント ロールアウトの機能。
- カナリア スケジューリング、リレー/クライアント、ガバナンス ダッシュボード、ゲート プロモーション (`dashboards/grafana/soranet_pq_ratchet.json`)。
- ロールバック フックとファイアドリル Runbook ([PQ ラチェット Runbook](./pq-ratchet-runbook.md))。

## Карта фаз

| `rollout_phase` | Эффективная 匿名ステージ | Эффект по умолчанию | Типичное использование |
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) | PQ ガード回路、信号を受信します。 |ベースラインとカナリア。 |
| `ramp` | `anon-majority-pq` (ステージ B) | Смещать выбор в сторону PQ リレー для >= двух третей 報道; классические リレー остаются フォールバック。 |中継カナリア。 SDK プレビューが切り替わります。 |
| `default` | `anon-strict-pq` (ステージ C) | PQ 専用回線とダウングレード アラームを備えています。 |テレメトリとサインオフ ガバナンスのプロモーション機能を備えています。 |

Если поверхность также задает явный `anonymity_policy`, он переопределяет phase для этого компонента. Отсутствие явного stage теперь defer-ится к значению `rollout_phase`, чтобы операторы могли переключить Phase один раз на средуクライアントに連絡してください。

## リファレンス конфигурации

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

ローダー オーケストレーターは、フォールバック ステージ (`crates/sorafs_orchestrator/src/lib.rs:2229`) と `sorafs_orchestrator_policy_events_total` および `sorafs_orchestrator_pq_ratio_*` を実行します。 См. `docs/examples/sorafs_rollout_stage_b.toml` および `docs/examples/sorafs_rollout_stage_c.toml` のスニペット。

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь сохраняет разобранную フェーズ (`crates/iroha/src/client.rs:2315`)、ヘルパー команды (например `iroha_cli app sorafs fetch`) の説明デフォルトの匿名性ポリシーの段階。

## 自動化

`cargo xtask` ヘルパーは、アーティファクトをキャプチャします。

1. **Сгенерировать региональный スケジュール**

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

   期間は `s`、`m`、`h` または `d` です。 Команда выпускает `artifacts/soranet_pq_rollout_plan.json` и Markdown summary (`artifacts/soranet_pq_rollout_plan.md`)、который можно приложить к 変更要求。

2. **ドリルアーティファクトを確実に捕捉**

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

   Команда копирует указанные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет BLAKE3 ダイジェストと каждого アーティファクトと пизет `rollout_capture.json` とメタデータと Ed25519ペイロード。秘密鍵、消防訓練議事録、ガバナンス キャプチャを確認します。

## SDK と CLI の開発

|表面 |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` は、フェーズ | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント構成 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションの `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションの `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションの `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` または `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

SDK は、ステージ パーサー、オーケストレーター (`crates/sorafs_orchestrator/src/lib.rs:365`)、ロックステップ フェーズでの多言語展開の切り替えを行います。

## カナリアのスケジュール設定チェックリスト

1. **プリフライト (T マイナス 2 週間)**- ステージ A の電圧低下率 <1%、PQ カバレッジ >=70% (`sorafs_orchestrator_pq_candidate_ratio`)。
   - ガバナンス レビュー スロット、который утверждает окно canary。
   - `sorafs.gateway.rollout_phase = "ramp"`、ステージング (オーケストレーター JSON、再デプロイ)、ドライラン プロモーション パイプラインをサポートします。

2. **リレーカナリア(T日)**

   - オーケストレーター `rollout_phase = "ramp"` は、リレーをマニフェストします。
   - 「結果ごとのポリシー イベント」と「ブラウンアウト率」 - PQ Ratchet ダッシュボード (ロールアウト パネル) - TTL ガード キャッシュ。
   - スナップショット `sorafs_cli guard-directory fetch` および監査ストレージ。

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - クライアント構成の `rollout_phase = "ramp"` と `stage-b` は、SDK コホートをオーバーライドします。
   - テレメトリの差分 (`sorafs_orchestrator_policy_events_total`、`client_id` および `region`) およびロールアウト インシデント ログを表示します。

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - サインオフ ガバナンス、オーケストレーター、クライアント構成、`rollout_phase = "default"`、リリース アーティファクトの準備チェックリストをサポートします。

## ガバナンスと証拠のチェックリスト

|相変化 |プロモーションゲート |証拠の束 |ダッシュボードとアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ステージ A 停電率 <1%、14 日以内、`sorafs_orchestrator_pq_candidate_ratio` >= 0.7 プロモーション、Argon2 チケット検証 p95 < 50 ミリ秒、ガバナンス プロモーション забронирован。 | JSON/Markdown ® `cargo xtask soranet-rollout-plan`、スナップショット `sorafs_cli guard-directory fetch` (до/после)、バンドル `cargo xtask soranet-rollout-capture --label canary`、カナリア分 [PQ ラチェット]ランブック](./pq-ratchet-runbook.md)。 | `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + ブラウンアウト率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、テレメトリ参照 × `docs/source/soranet/snnet16_telemetry_plan.md`。 |
|ランプ -> デフォルト *(ステージ C 強制)* | 30 日間の SN16 テレメトリ バーンイン、`sn16_handshake_downgrade_total` ベースライン、`sorafs_orchestrator_brownouts_total` クライアント カナリア、およびプロキシ トグル リハーサル。 | `sorafs_cli proxy set-mode --mode gateway|direct`、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml`、`sorafs_cli guard-directory verify`、バンドル `cargo xtask soranet-rollout-capture --label default` を参照してください。 | PQ ラチェット ボードは SN16 ダウングレード パネル、`docs/source/sorafs_orchestrator_rollout.md` および `dashboards/grafana/soranet_privacy_metrics.json` に対応しています。 |
|緊急降格/ロールバックの準備 |ダウングレード カウンター、ガード ディレクトリの検証、ダウングレード イベント、`/policy/proxy-toggle` が表示されます。 |チェックリスト `docs/source/ops/soranet_transport_rollback.md`、`sorafs_cli guard-directory import` / `guard-cache prune`、`cargo xtask soranet-rollout-capture --label rollback`、インシデント チケット、通知テンプレート。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json` およびアラート パック (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |

- `artifacts/soranet_pq_rollout/<timestamp>_<label>/` のアーカイブ、`rollout_capture.json` のガバナンス パケット、スコアボード、promtool トレース、ダイジェストを保存します。
- SHA256 ダイジェストの作成 (議事録 PDF、キャプチャ バンドル、ガード スナップショット)、プロモーション議事録、議会の承認などの情報を取得к ステージングクラスター。
- テレメトリ プランとプロモーション チケット、`docs/source/soranet/snnet16_telemetry_plan.md` остается каноническим источником для ダウングレード語彙とアラートしきい値。

## ダッシュボードとテレメトリの更新

`dashboards/grafana/soranet_pq_ratchet.json` 注釈パネル「ロールアウト計画」、プレイブック、フェーズ、ガバナンス レビューの説明подтвердить активную ステージ。パネルとノブを組み合わせます。

警告アラート、ルール、ラベル `stage`、カナリア、デフォルト フェーズ、ポリシーしきい値(`dashboards/alerts/soranet_handshake_rules.yml`)。

## ロールバックフック

### デフォルト -> ランプ (ステージ C -> ステージ B)

1. オーケストレーターを降格する (`sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp`) (SDK 構成でフェーズを変更)、ステージ B でステージ B を実行します。
2. クライアントは、トランスポート プロファイル `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`、監査ワークフロー `/policy/proxy-toggle` のトランスクリプトを受け取ります。
3. `cargo xtask soranet-rollout-capture --label rollback-default` では、ガード ディレクトリの差分、promtool の出力、およびダッシュボードのスクリーンショットが `artifacts/soranet_pq_rollout/` に表示されます。

### ランプ -> カナリア (ステージ B -> ステージ A)1. ガード ディレクトリ スナップショット、昇格プロモーション、`sorafs_cli guard-directory import --guard-directory guards.json` および `sorafs_cli guard-directory verify`、降格パケットハッシュ。
2. `rollout_phase = "canary"` (オーバーライド `anonymity_policy stage-a`)、オーケストレーターとクライアント構成、PQ ラチェット ドリルと [PQ ラチェット Runbook](./pq-ratchet-runbook.md)、パイプラインをダウングレードします。
3. PQ ラチェットと SN16 テレメトリのスクリーンショット、アラート結果、インシデント ログ、ガバナンスを確認します。

### ガードレールのリマインダー

- `docs/source/ops/soranet_transport_rollback.md` の降格と緩和策の `TODO:` のロールアウト トラッカーの確認ああ。
- `dashboards/alerts/soranet_handshake_rules.yml` と `dashboards/alerts/soranet_privacy_rules.yml` は、`promtool test rules` とロールバック、アラート ドリフトを確認します。キャプチャバンドル。