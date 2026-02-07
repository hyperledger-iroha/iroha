---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
title: ソラネットのPQラチェットシミュレータ
Sidebar_label: PQ ラチェットのランブック
説明: 保護者、政治家、テレメトリアの決定権の検証に関する PQ エスカロナダの推進者です。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/soranet/pq_ratchet_runbook.md`。満天アンバスコピアスシンクロニザダス。
:::

## プロポジト

SoraNet でのポスト量子 (PQ) エスカロナダの政治的安全性を保証するランブックです。ロス オペラドールは、プロモーション (ステージ A -> ステージ B -> ステージ C) での劣化制御とステージ B/A の供給 PQ を制御します。テレメトリのフック検証 (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) は、リハーサルのログとインシデントのアーチファクトを収集します。

## 前提条件

- Ultimo binario `sorafs_orchestrator` 機能重み付け (`docs/source/soranet/reports/pq_ratchet_validation.md` での参照デピューのシミュレーションへのコミット)。
- Prometheus/Grafana のスタックが `dashboards/grafana/soranet_pq_ratchet.json` に対応します。
- スナップショット名目上のデルガードディレクトリ。シミュレーションを行う前に、検証と検証を行ってください:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ソース ディレクトリで JSON を単独で公開し、Norito バイナリと `soranet-directory build` を再エンコードして、ヘルパーを回転させて取り出します。

- 発行者コントロールの CLI の前段階のキャプチャ メタデータ:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- オンコールのネットワーキングと可観測性を考慮した活動を行います。

## プロモーションのパソス

1. **段階監査**

   登録段階の初期:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` プロモーション前。

2. **プロモシオナ ステージ B (多数決 PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >= 5 minutos para que los refresquen の症状が現れる。
   - En Grafana (ダッシュボード `SoraNet PQ Ratchet Drill`) パネルの「ポリシー イベント」を確認して、`outcome=met` パラ `stage=anon-majority-pq` を確認します。
   - JSON パネルのスクリーンショットとインシデントの付属のログをキャプチャします。

3. **ステージ C のプロモーション (厳格な PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - ヒストグラム `sorafs_orchestrator_pq_ratio_*` が 1.0 であることを確認します。
   - ブラウンアウト永久プラノのコンタドールを確認します。いいえ、劣化が進んでいます。

## 劣化/電圧低下のシミュレーション

1. **PQ を誘導する**

   Deshabilita は、プレイグラウンドの記録とガード ディレクトリでの PQ リレー、クラシック ソラメンテ、オーケストレーターのレカルガ エル キャッシュを提供します。

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **停電時のテレメトリの観察**

   - ダッシュボード: パネルの「停電率」は 0 です。
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` デベ レポーター `anonymity_outcome="brownout"` コン `anonymity_reason="missing_majority_pq"`。

3. **劣化 a ステージ B / ステージ A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   シリコン供給 PQ が不足しているため、`anon-guard-pq` が劣化します。同様のターミナルで、ブラウンアウトとラス プロモシオネスの再構築を実現します。

4. **レストラン エル ガード ディレクトリ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリアとアーティファクト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **アラート Prometheus:** `sorafs_orchestrator_policy_events_total` のブラウンアウトに関するアラートを安全に管理し、SLO 設定を管理します (10 分間で 5% 未満)。
- **インシデント ログ:** 遠隔測定のスニペットとオペレータの補助は `docs/examples/soranet_pq_ratchet_fire_drill.log` にありません。
- **Captura farmada:** 米国 `cargo xtask soranet-rollout-capture` パラコピー、ドリル ログ、スコアボード、`artifacts/soranet_pq_rollout/<timestamp>/`、計算ダイジェスト BLAKE3 y producir un `rollout_capture.json` ファームダ。

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

統治に関する政府の補助機関。

## ロールバック

リアルな PQ をシミュレートし、ステージ A で永続的に管理し、通知ネットワーキング TL と追加のメトリクスを収集し、ディレクトリとインシデント トラッカーの差異を監視します。米国の輸出デルガードディレクトリは、レストランの通常のサービスを前方からキャプチャします。

:::回帰に関するヒント
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` 検証結果の検証は、シミュレーション結果に基づいて行われます。
:::