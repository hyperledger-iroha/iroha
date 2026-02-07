---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
title: SoraNet PQ ラチェットファイアドリル
サイドバー_ラベル: PQ ラチェット ランブック
説明: PQ の匿名性ポリシー、昇進、降格、オンコール リハーサル ステップ、決定論的テレメトリ検証。
---

:::note 正規ソース
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ドキュメント セットの引退 دونوں کاپیاں 同期 رکھیں۔
:::

## 大事

ランブック SoraNet ステージングされたポスト量子 (PQ) 匿名性ポリシー ファイアドリル シーケンス ファイアドリル シーケンスオペレーターの昇進 (ステージ A -> ステージ B -> ステージ C) PQ の供給 コントロールされた降格 ステージ B/A のリハーサルドリル テレメトリ フック (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) は、インシデント リハーサル ログを検証し、アーティファクトを検証します。

## 前提条件

- 能力の重み付け `sorafs_orchestrator` バイナリ (コミット ドリル リファレンス) ٯیا ہے)۔
- Prometheus/Grafana スタックのスタック `dashboards/grafana/soranet_pq_ratchet.json` のサービス
- 名目上のガード ディレクトリのスナップショットドリル、コピー、フェッチ、検証、次のとおりです。

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ソース ディレクトリ JSON パブリッシュ 回転ヘルパー `soranet-directory build` バイナリ Norito バイナリ 再エンコード ローテーション ヘルパー

- CLI によるメタデータのキャプチャと発行者のローテーション アーティファクトの前段階の処理:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- ネットワークの監視、オンコール チームの変更ウィンドウの管理

## プロモーションの手順

1. **段階監査**

   ステージステージ:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   プロモーション سے پہلے `anon-guard-pq` 期待してください

2. **ステージ B (多数決 PQ) 昇格**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - マニフェストの更新 ہونے کے لئے >=5 منٹ انتظار کریں۔
   - Grafana میں (ダッシュボード `SoraNet PQ Ratchet Drill`) [ポリシー イベント] パネル `stage=anon-majority-pq` کے لئے `outcome=met` کی تصدیق کریں۔
   - スクリーンショット パネル JSON キャプチャー インシデント ログ 添付ファイル

3. **ステージ C (厳密な PQ) でプロモート**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` ヒストグラム 1.0 確認 確認 確認
   - 停電カウンター フラット チェック 確認降格手順 فالو کریں۔

## 降格/停電訓練

1. **合成 PQ 不足です**

   プレイグラウンド環境 ガード ディレクトリ クラシック エントリ トリミング トリム PQ リレーの無効化 オーケストレーター キャッシュのリロード:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **停電テレメトリで観測**

   - ダッシュボード: 「停電率」パネル 0 が急上昇
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` ٩و `anonymity_outcome="brownout"` اور `anonymity_reason="missing_majority_pq"` رپورٹ کرنا چاہئے۔

3. **ステージ B / ステージ A の降格**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ 供給 اب بھی ناکافی ہو تو `anon-guard-pq` پر 降格 کریں۔ドリルを実行する 電圧低下カウンターを解決する プロモーションを実行する プロモーションを実行する

4. **ディレクトリを監視します**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリとアーティファクト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus アラート:** یقینی بنائیں کہ `sorafs_orchestrator_policy_events_total` 電圧低下アラートが設定された SLO کے نیچے رہے (<5% کسی بھی 10 分ウィンドウ) और देखें
- **インシデント ログ:** テレメトリ スニペット、オペレーターのメモ、`docs/examples/soranet_pq_ratchet_fire_drill.log` 追加、追加
- **署名付きキャプチャ:** `cargo xtask soranet-rollout-capture` スコアボード ドリル ログ `artifacts/soranet_pq_rollout/<timestamp>/` コピー スコアボード BLAKE3 ダイジェストの計算署名済み `rollout_capture.json` 認証

例:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

生成されたメタデータ、署名、ガバナンス パケット、添付ファイル

## ロールバック

訓練、PQ 不足、ステージ A のネットワーク、TL の収集、メトリクスの収集、ガード ディレクトリの差分、インシデント トラッカーの添付ありがとうキャプチャ キャプチャ ガード ディレクトリのエクスポート 正常なサービスの復元

:::tip 回帰カバレッジ
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` ドリルとサポートの統合検証
:::