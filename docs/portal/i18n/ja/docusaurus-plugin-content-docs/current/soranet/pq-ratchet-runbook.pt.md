---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
タイトル: Simulacro PQ ラチェット ド ソラネット
Sidebar_label: PQ ラチェットのランブック
説明: プロモーターのオンコール対応は、テレメトリの確定性を確認するための政治的監視を担当します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/soranet/pq_ratchet_runbook.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

## プロポジト

ランブックは、SoraNet のポスト量子 (PQ) の政治的シミュレーションを実行するためのシーケンスを作成します。オペラドールのプロモーション (ステージ A -> ステージ B -> ステージ C) は、ステージ B/A と PQ のパフォーマンスを制御します。テレメトリのフックのシミュレーション (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) は、インシデントのリハーサル ログに基づいて収集された芸術品です。

## 前提条件

- Ultimo binario `sorafs_orchestrator` com 機能重み付け (コミット igual ou posterior ao リファレンス do ドリル mostrado em `docs/source/soranet/reports/pq_ratchet_validation.md`)。
- Acesso ao スタック Prometheus/Grafana que サーブ `dashboards/grafana/soranet_pq_ratchet.json`。
- スナップショット名目上のディレクトリをガードします。バスクと有効なコピーピアは、シミュレーションを実行します:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ソース ディレクトリを JSON として公開し、Norito バイナリ COM `soranet-directory build` を再エンコードして、ロータリー オス ヘルパーを参照してください。

- 発行者側の CLI による前段階の技術情報によるメタデータのキャプチャ:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- ジャネラ・デ・ムダンカは、ネットワーキングと可観測性のオンコール時間を承認します。

## 宣伝活動

1. **段階監査**

   登録ステージのイニシャル:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espere `anon-guard-pq` はプロモーション用です。

2. **ステージ B のプロモバ (多数決 PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 分でセレム アトゥアリザドスが現れます。
   - いいえ、Grafana (ダッシュボード `SoraNet PQ Ratchet Drill`) は、「ポリシー イベント」のほとんどの `outcome=met` パラ `stage=anon-majority-pq` を確認します。
   - JSON のスクリーンショットをキャプチャし、インシデントのログを作成します。

3. **ステージ C のプロモバ (厳格な PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - バージョン 1.0 のヒストグラム `sorafs_orchestrator_pq_ratio_*` を検証します。
   - 停電の永久計画を確認します。カソ・コントラリオ、シガ・オス・パソス・デ・デスプロモカオ。

## Despromocao / 停電ドリル

1. **インドゥサ ウマ エスカセ シンテティカ デ PQ**

   Desative リレー PQ は、アンビエンテ プレイグラウンド レドゥジンド、ガード ディレクトリ、古典的なアペナス、デポワ リカルレグ、オーケストレーターをキャッシュするものではありません。

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **停電時のテレメトリを観察します**

   - ダッシュボード: 「停電率」0 を表示します。
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` 開発レポーター `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`。

3. **デスプロモバ ステージ B / ステージ A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   不十分な場合は PQ を参照してください。`anon-guard-pq` のデスプロモバを参照してください。これは、プロモーション用のシステムとして、ブラウンアウトの制御を確立するための仮想端末です。

4. **レストラン・ガードディレクトリ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリアとアルティファト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **アラート Prometheus:** `sorafs_orchestrator_policy_events_total` でブラウンアウトに関するアラートが発生しました。SLO 設定が行われています (10 分間で 5% 未満)。
- **インシデント ログ:** テレメトリーの記録は管理者 `docs/examples/soranet_pq_ratchet_fire_drill.log` に記録されていません。
- **キャプチャ アッシナダ:** `cargo xtask soranet-rollout-capture` パラ コピア、ドリル ログ、スコアボードパラ `artifacts/soranet_pq_rollout/<timestamp>/`、計算ダイジェスト BLAKE3、`rollout_capture.json` アッシナドを使用します。

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

別の OS メタデータはガバナンスの機能を提供します。

## ロールバック

PQ の実際のエスカセをシミュレートし、ステージ A を永続的に管理し、ネットワーク TL の別のメトリクスを収集し、インシデント トラッカーとディレクトリを監視します。レストランのガード ディレクトリのキャプチャをエクスポートしたり、通常のサービスを使用したりしてください。

:::回帰に関するヒント
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` は、シミュレーションを継続的に検証するためのものです。
:::