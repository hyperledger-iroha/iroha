---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
title: Учебная тревога PQ ラチェット SoraNet
Sidebar_label: Runbook PQ ラチェット
説明: オンコール リハーサルと、PQ 匿名性ポリシー、テレメトリの監視。
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/soranet/pq_ratchet_runbook.md`.ドキュメントを参照してください。
:::

## Назначение

ランブックは、ファイアドリル、ポスト量子 (PQ) 匿名性ポリシー、SoraNet をサポートします。オペレーターは昇進 (ステージ A -> ステージ B -> ステージ C)、制御された降格、ステージ B/A および PQ を供給します。テレメトリ フック (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) と、インシデント リハーサル ログのアーティファクトをドリルします。

## 前提条件

- `sorafs_orchestrator` バイナリ機能重み付け (リファレンス ドリル `docs/source/soranet/reports/pq_ratchet_validation.md` をコミット)。
- Доступ к Prometheus/Grafana スタック、который обслуживает `dashboards/grafana/soranet_pq_ratchet.json`。
- Номинальный ガード ディレクトリのスナップショット。ドリルの内容:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ソース ディレクトリの JSON、Norito バイナリ `soranet-directory build` のローテーション ヘルパー。

- メタデータと前段階のアーティファクトを発行者が CLI で確認するには:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- オンコール ネットワークと可観測性のウィンドウを変更します。

## プロモーションの手順

1. **段階監査**

   Зафиксируйте стартовый ステージ:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   プロモーション ожидайте `anon-guard-pq`。

2. **プロモーション × ステージ B (多数決 PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 минут для обновления が現れます。
   - Grafana (ダッシュボード `SoraNet PQ Ratchet Drill`) が表示され、「ポリシー イベント」が `outcome=met` と `stage=anon-majority-pq` で表示されます。
   - スクリーンショット、JSON およびインシデント ログを表示します。

3. **プロモーション × ステージ C (厳格な PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Проверьте、что ヒストグラム `sorafs_orchestrator_pq_ratio_*` стремится к 1.0。
   - Убедитесь、停電カウンター остается плоским;降格です。

## 降格/停電訓練

1. **Индуцируйте синтетический дефицит PQ**

   PQ リレー、プレイグラウンド、ガード ディレクトリ、エントリ、オーケストレーター キャッシュの機能:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **テレメトリの電圧低下**

   - ダッシュボード: 「停電率」が 0 です。
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` должен сообщить `anonymity_outcome="brownout"` с `anonymity_reason="missing_majority_pq"`。

3. **ステージ B / ステージ A による降格**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   `anon-guard-pq` で PQ を供給してください。ドリル、停電カウンター、プロモーション、プロモーションなど。

4. **ガードディレクトリ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリとアーティファクト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus アラート:** убедитесь、что ブラウンアウト アラート `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (<5% влюбом 10-минутном окне)。
- **インシデント ログ:** テレメトリ スニペットと `docs/examples/soranet_pq_ratchet_fire_drill.log` の情報。
- **署名付きキャプチャ:** `cargo xtask soranet-rollout-capture`、ドリル ログ、スコアボード、`artifacts/soranet_pq_rollout/<timestamp>/`、BLAKE3 ダイジェスト、スコアボードなど`rollout_capture.json`。

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

メタデータと署名のガバナンスを管理します。

## ロールバック

ドリルでは、PQ、ステージ A、ネットワーキング TL、メトリクス、ガード ディレクトリの差分、インシデント トラッカーを確認できます。ガード ディレクトリのエクスポートを実行し、ディレクトリをエクスポートします。

:::tip 回帰カバレッジ
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` 合成検証、ドリルのテスト。
:::