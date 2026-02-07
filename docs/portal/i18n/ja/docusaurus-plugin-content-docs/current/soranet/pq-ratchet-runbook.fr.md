---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
title: シミュレーション PQ ラチェット SoraNet
Sidebar_label: Runbook PQ ラチェット
説明: オンコールでリハーサルを行い、プロモウヴォワールやレトログラーダー、匿名の PQ および遠隔測定の検証を行います。
---

:::note ソースカノニク
Cette ページは `docs/source/soranet/pq_ratchet_runbook.md` を反映します。 Gardez les deux は、ドキュメントの再考と一致するものをコピーします。
:::

## 目的

SoraNet のポスト量子 (PQ) の匿名政治を構築する一連のファイアドリル ガイドです。操作者は昇進 (ステージ A -> ステージ B -> ステージ C) を繰り返し、逆行制御対象者とステージ B/A および PQ ベースを制御します。テレメトリーの有効なフック (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) と、リハーサルの記録を収集したアーティファクトを収集します。

## 前提条件

- Dernier binaire `sorafs_orchestrator` の avec 能力重み付け (`docs/source/soranet/reports/pq_ratchet_validation.md` のドリルを参照して事後的にコミット)。
- au スタック Prometheus/Grafana にアクセスし、`dashboards/grafana/soranet_pq_ratchet.json` を起動します。
- スナップショット名目上のガード ディレクトリ。事前にドリルをコピーして検証します:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ファイル ソース ディレクトリは JSON で公開されており、バイナリで再エンコードされます。Norito avec `soranet-directory build` は、ローテーション ヘルパーの実行前に実行されます。

- 発行者による CLI のメタデータと事前段階のアーティファクトのローテーションをキャプチャします。

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- 変更を承認するフェネトルは、オンコール ネットワーキングと可観測性を備えます。

## エテープ・ド・プロモーション

1. **段階監査**

   出発段階の登録:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Attendez `anon-guard-pq` アバント プロモーション。

2. **プロモーションとステージ B (多数決 PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - 出席者は、必要なマニフェストを 5 分以上注ぎます。
   - Grafana (ダッシュボード `SoraNet PQ Ratchet Drill`) は、「ポリシー イベント」添付ファイル `outcome=met` と `stage=anon-majority-pq` を確認します。
   - JSON のスクリーンショットをキャプチャし、事件のログを添付します。

3. **プロモーションとステージ C (厳格な PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - ヒストグラム `sorafs_orchestrator_pq_ratio_*` 傾向バージョン 1.0 を確認します。
   - 電圧低下の残りの部分を確認します。シノン・スイベス・レ・エテープ・デ・逆行。

## 逆行ドリル/ブラウンアウト

1. **Induire une penurie PQ synthetique**

   Desactivez lesリレーPQ dans l'environnement playground en taillant le Guardディレクトリaux seules entrees classics、puis rechargez leキャッシュオーケストレーター:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **監視員によるテレメトリのブラウンアウト**

   - ダッシュボード: le panneau "Brownout Rate" monte au-dessus de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` doit レポーター `anonymity_outcome="brownout"` avec `anonymity_reason="missing_majority_pq"`。

3. **レトログラーダー vs ステージ B / ステージ A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ の残りが不十分です。逆行バージョン `anon-guard-pq`。安定性と再アップリケのプロモーションを行うために、定期的な監視を行います。

4. **レストラン ル ガード ディレクトリ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリとアーティファクト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **アラート Prometheus:** `sorafs_orchestrator_policy_events_total` の電圧低下を警告し、SLO 設定を停止します (10 分間のフェネトレ時間で 5% 未満)。
- **インシデント ログ:** `docs/examples/soranet_pq_ratchet_fire_drill.log` を操作するテレメトリーのスニペットとメモが記録されています。
- **署名者のキャプチャ:** `cargo xtask soranet-rollout-capture` は、コピー機のドリル ログとスコアボードの `artifacts/soranet_pq_rollout/<timestamp>/` を利用し、計算機は BLAKE3 のダイジェストを作成し、`rollout_capture.json` 署名を作成します。

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

Joignez のメタデータ生成と文書ガバナンスの署名。

## ロールバック

真実のペニューリー PQ を確認し、ステージ A で休息し、ネットワーク TL と添付ファイルのメトリクスを収集して、ガード ディレクトリの監視ディレクトリ オー トラッカーを追跡します。ディレクトリ キャプチャのエクスポートとレストランの通常のサービスを利用します。

:::ヒント クーベルチュール回帰
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fournit la validation synthetique qui sooutient ce ドリル。
:::