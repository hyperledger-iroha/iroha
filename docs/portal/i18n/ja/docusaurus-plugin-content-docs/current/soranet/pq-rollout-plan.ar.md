---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ロールアウトプラン
タイトル: SNNet-16G SNNet-16G
サイドバーラベル: PQ
説明: ハンドシェイク、X25519+ML-KEM、SoraNet、カナリア、デフォルトのリレー、クライアント、SDK。
---

:::メモ
テストは `docs/source/soranet/pq_rollout_plan.md` です。 حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة。
:::

SNNet-16G は、SoraNet をサポートします。 `rollout_phase` ステージ A ステージ A ステージ B PQ はステージ C で、JSON/TOML はテストされます。

プレイブック:

- コードベース (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) コードベース (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`)。
- SDK および CLI とクライアントのロールアウトのフラグ。
- リレー/クライアントのスケジュール管理とガバナンス管理 (`dashboards/grafana/soranet_pq_ratchet.json`)。
- フックのロールバックとファイアドリル ([PQ ラチェット Runbook](./pq-ratchet-runbook.md))。

## 認証済み

| `rollout_phase` |ログイン | ログイン | ログインニュース | ニュースऔर देखें
|-----------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (ステージ A) |ガードPQ واحد على الاقل لكل Circuit بينما تسخن المنظومة。 |ベースライン カナリア諸島。 |
| `ramp` | `anon-majority-pq` (ステージ B) | تحيز الاختيار نحو リレー PQ لتحقيق تغطية >= ثلثين؛は、フォールバックをリレーします。 |カナリア諸島、リレー、 SDK を切り替えます。 |
| `default` | `anon-strict-pq` (ステージ C) |回路 PQ がダウングレードされました。 |テレメトリーとガバナンスを強化します。 |

ذا قام سطح ما ايضا بتعيين `anonymity_policy` صريحة فانها تتغلب على المرحلة لذلك المكون. حذف المرحلة الصريحة يجعلها تعتمد على قيمة `rollout_phase` حتى يتمكن المشغلون من تبديل المرحلةクライアントをサポートします。

## عرض المزيد

### オーケストレーター (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

ローダーとオーケストレーターのフォールバック (`crates/sorafs_orchestrator/src/lib.rs:2229`) と `sorafs_orchestrator_policy_events_total` と `sorafs_orchestrator_pq_ratio_*`。 `docs/examples/sorafs_rollout_stage_b.toml` と `docs/examples/sorafs_rollout_stage_c.toml` を確認してください。

### Rust クライアント / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` يسجل الان المرحلة المحللة (`crates/iroha/src/client.rs:2315`) بحيث يمكن لاوامر المساعدة (مثل `iroha_cli app sorafs fetch`) ان最高のパフォーマンスを見せてください。

## ああ

`cargo xtask` のアーティファクト。

1. **

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

   `s`、`m`、`h`、`d`を確認してください。マークダウン (`artifacts/soranet_pq_rollout_plan.md`) は、`artifacts/soranet_pq_rollout_plan.json` マークダウン (`artifacts/soranet_pq_rollout_plan.md`) です。

2. **重要なアーティファクト**

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

   スクリーンショット `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ダイジェスト BLAKE3 アーティファクト `rollout_capture.json`メタデータ Ed25519 を参照してください。秘密鍵を管理し、消防訓練を実施し、ガバナンスを管理します。

## フラグ SDK وCLI

|ああ |カナリア (ステージ A) |ランプ(ステージB) |デフォルト (ステージ C) |
|-------|-------|----------------|---------------------|
| `sorafs_cli` フェッチ | `--anonymity-policy stage-a` 認証済み | 認証済み`--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
|オーケストレーター構成 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust クライアント設定 (`iroha.toml`) | `rollout_phase = "canary"` (デフォルト) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 署名付きコマンド | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`、オプションの `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`、オプションの `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`、オプションの `.ANON_STRICT_PQ` |
| JavaScript オーケストレーター ヘルパー | `rolloutPhase: "canary"` | `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|スイフト `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

SDK の切り替えとステージ パーサーの切り替え、オーケストレーター (`crates/sorafs_orchestrator/src/lib.rs:365`) の切り替え、および SDK の切り替えありがとうございます。

## スケジュールを設定する

1. **プリフライト (T マイナス 2 週間)**

- ブラウンアウト率 ステージ A 1% スコア 1% PQ >=70% スコア(`sorafs_orchestrator_pq_candidate_ratio`)。
   - スロット ガバナンス カナリア。
   - テスト `sorafs.gateway.rollout_phase = "ramp"` ステージング (JSON テスト、オーケストレーター、テスト)、ドライラン テスト。2. **リレーカナリア(T日)**

   - 管理者は、`rollout_phase = "ramp"` オーケストレーターとマニフェストをリレーします。
   - 「結果ごとのポリシー イベント」と「ブラウンアウト率」 PQ ラチェット (ロールアウト) TTL ガード キャッシュ。
   - スナップショット `sorafs_cli guard-directory fetch` を確認してください。

3. **クライアント/SDK カナリア (T プラス 1 週間)**

   - `rollout_phase = "ramp"` クライアントは、`stage-b` SDK をオーバーライドします。
   - テレメトリ (`sorafs_orchestrator_policy_events_total` مجمعة حسب `client_id` و`region`) ロールアウト。

4. **デフォルトのプロモーション (T プラス 3 週間)**

   - ガバナンス、オーケストレーター、クライアント、`rollout_phase = "default"`、チェックリスト、アーティファクトのチェックリストああ。

## ガバナンス

|ニュース | ニュースニュース | ニュース認証済み |ダッシュボード وアラート |
|--------------|----------------|---------------|----------------------------|
| Canary -> ランプ *(ステージ B プレビュー)* |ブラウンアウト ステージ A 1% 14 يوما، `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 منطقة تمت ترقيتها، تحقق Argon2 ticket p95 < 50 ms スロットはガバナンスを強化します。 | JSON/Markdown `cargo xtask soranet-rollout-plan` スナップショット `sorafs_cli guard-directory fetch` (قبل/بعد) حزمة موقعة `cargo xtask soranet-rollout-capture --label canary`، ومحاضرカナリア [PQ ラチェット ランブック](./pq-ratchet-runbook.md)。 | `dashboards/grafana/soranet_pq_ratchet.json` (ポリシー イベント + 電圧低下率)、`dashboards/grafana/soranet_privacy_metrics.json` (SN16 ダウングレード率)、テレメトリ `docs/source/soranet/snnet16_telemetry_plan.md`。 |
|ランプ -> デフォルト *(ステージ C 強制)* |バーンイン 30 時間 SN16 時間 `sn16_handshake_downgrade_total` ベースライン ` sorafs_orchestrator_brownouts_total` カナリア時間リハーサルとプロキシの切り替え。 | `sorafs_cli proxy set-mode --mode gateway|direct` と `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` と `sorafs_cli guard-directory verify` と `cargo xtask soranet-rollout-capture --label default` です。 | PQ ラチェット SN16 ダウングレード `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`。 |
|緊急降格/ロールバックの準備 |バッファ `/policy/proxy-toggle` をダウングレードします。 |チェックリスト `docs/source/ops/soranet_transport_rollback.md` を確認してください。 `sorafs_cli guard-directory import` / `guard-cache prune` を確認してください。 `cargo xtask soranet-rollout-capture --label rollback` を確認してください。 | `dashboards/grafana/soranet_pq_ratchet.json` 、 `dashboards/grafana/soranet_privacy_metrics.json` 、 وكلا حزمتَي التنبيه (`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`)。 |

- アーティファクト `artifacts/soranet_pq_rollout/<timestamp>_<label>/` と `rollout_capture.json` の管理、ガバナンス、スコアボード、promtool トレース、ダイジェスト。
- SHA256 ダイジェスト (議事録 PDF、キャプチャ バンドル、ガード スナップショット) および国会議事堂のダイジェスト素晴らしいステージング。
- テレメトリのアップグレード、`docs/source/soranet/snnet16_telemetry_plan.md` のダウングレード、およびダウングレードああ。

## ダッシュボードとテレメトリ

`dashboards/grafana/soranet_pq_ratchet.json` 「ロールアウト計画」の戦略と戦略統治は重要です。ノブを調整してください。

カナリア وdefault を使用してください。 حدود سياسة منفصلة (`dashboards/alerts/soranet_handshake_rules.yml`)。

## フックによるロールバック

### デフォルト -> ランプ (ステージ C -> ステージ B)

1. オーケストレーター `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (ومزامنة نفس المرحلة عبر اعدادات SDK) ステージ B のテスト。
2. クライアントの管理 `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` 管理 ワークフロー `/policy/proxy-toggle`ありがとうございます。
3. `cargo xtask soranet-rollout-capture --label rollback-default` の差分ファイル、ガード ディレクトリ、promtool、ダッシュボード `artifacts/soranet_pq_rollout/`。

### ランプ -> カナリア (ステージ B -> ステージ A)

1. スナップショット ガード ディレクトリ `sorafs_cli guard-directory import --guard-directory guards.json` ガード ディレクトリ`sorafs_cli guard-directory verify` パケットのハッシュ。
2. `rollout_phase = "canary"` (`anonymity_policy stage-a` をオーバーライド) オーケストレーターとクライアントの設定を変更する PQ ラチェット ドリル [PQ ラチェット] Runbook](./pq-ratchet-runbook.md) ダウングレード。
3. PQ ラチェットとテレメトリー SN16 の管理とガバナンスの管理。

### ガードレール

- `docs/source/ops/soranet_transport_rollback.md` のセキュリティ緩和策 `TODO:` のロールアウト トラッカー。
- حافظ على `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` تحت تغطية `promtool test rules` قبل وبعد اي ロールバック لتوثيق ドリフト في التنبيهاتを攻略します。