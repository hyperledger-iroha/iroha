---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: pq-ratchet-runbook
タイトル: PQ ラチェット في SoraNet
サイドバーラベル: PQ ラチェット
説明: セキュリティ セキュリティ、テレメトリ セキュリティ、およびテレメトリ セキュリティ。
---

:::メモ
テストは `docs/source/soranet/pq_ratchet_runbook.md` です。 حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة。
:::

## いいえ

ポスト量子 (PQ) とソラネット。 (ステージ A -> ステージ B -> ステージ C) ステージ B/A と PQ を比較します。テレメトリ フック (`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`) のアーティファクトが含まれています。

## いいえ

- バイナリ `sorafs_orchestrator` 機能重み付け (コミット `docs/source/soranet/reports/pq_ratchet_validation.md`)。
- Prometheus/Grafana または `dashboards/grafana/soranet_pq_ratchet.json`。
- スナップショット ガード ディレクトリ。回答:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

ソース ディレクトリ JSON と Norito バイナリ `soranet-directory build` ヘルパー。

- メタデータとアーティファクトの発行者と CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- オンコール対応中です。

## 認証済み

1. **評価**

   回答:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   `anon-guard-pq` を確認してください。

2. **ステージ B (多数決 PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظر >=5 دقائق حتى تتجدد が明示されます。
   - Grafana (`SoraNet PQ Ratchet Drill`) 「ポリシー イベント」 `outcome=met` `stage=anon-majority-pq`。
   - JSON 形式のファイル。

3. **ステージ C (厳密な PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - `sorafs_orchestrator_pq_ratio_*` 1.0 をインストールします。
   - ブラウンアウト يبقى ثابتا؛ありがとうございます。

## アーティスト / ブラウンアウト

1. **احداث نقص PQ اصطناعي**

   リレー PQ プレイグラウンド ガード ディレクトリ エントリー エントリ セキュリティ キャッシュ オーケストレータ:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **テレメトリによる電圧低下**

   - ダッシュボード:「停電率」0。
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch`、`anonymity_outcome="brownout"`、`anonymity_reason="missing_majority_pq"`。

3. **ステージB / ステージA**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   世界は、PQ は、`anon-guard-pq` です。ブラウンアウト、ブラウンアウト。

4. **ガードディレクトリ**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## テレメトリーとアーティファクト

- **ダッシュボード:** `dashboards/grafana/soranet_pq_ratchet.json`
- **تنبيهات Prometheus:** تاكد من تنبيه ブラウンアウト لـ `sorafs_orchestrator_policy_events_total` يبقى دون SLO المعتمد (<5% ضمن اي نافذة 10 月）。
- **インシデント ログ:** テレメトリ メッセージ `docs/examples/soranet_pq_ratchet_fire_drill.log`。
- ** 評価:** 評価 `cargo xtask soranet-rollout-capture` ドリル ログ 評価 `artifacts/soranet_pq_rollout/<timestamp>/` 評価 BLAKE3 ダイジェスト`rollout_capture.json` 。

意味:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

ガバナンスを強化します。

## ロールバック

管理者 PQ 管理者 ステージ A 管理者 ネットワーキング TL 管理者 管理者ディレクトリ 管理者ディレクトリありがとうございます。ガード ディレクトリを確認してください。

:::ヒント
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` يوفر التحقق الاصطناعي الذي يدعم هذا التمرين。
:::