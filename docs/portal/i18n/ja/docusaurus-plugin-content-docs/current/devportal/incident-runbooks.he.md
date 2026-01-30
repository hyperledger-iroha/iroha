---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54fec7e975aa353802bc141c626f870d376d163c1be970385b329506c08b3ae4
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# インシデント・ランブックとロールバック演習

## 目的

ロードマップ項目 **DOCS-9** は、実行可能なプレイブックとリハーサル計画を求めています。
これによりポータル運用者は、リリース失敗から推測なしで復旧できます。このノートは、
高い信号度の 3 つのインシデント (デプロイ失敗、レプリケーション劣化、アナリティクス停止) を扱い、
エイリアスのロールバックと合成検証がエンドツーエンドで機能することを示す四半期ドリルを文書化します。

### 関連資料

- [`devportal/deploy-guide`](./deploy-guide) — パッケージング、署名、エイリアス昇格のワークフロー。
- [`devportal/observability`](./observability) — 以下で参照する release tags、analytics、probes。
- `docs/source/sorafs_node_client_protocol.md`
  と [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — レジストリのテレメトリーとエスカレーション閾値。
- `docs/portal/scripts/sorafs-pin-release.sh` と `npm run probe:*` のヘルパーは
  チェックリスト全体で参照します。

### 共通のテレメトリーとツール

| シグナル / ツール | 目的 |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | レプリケーション停止と SLA 違反を検出。 |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | バックログの深さと完了レイテンシを定量化し、トリアージに使用。 |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | 悪い deploy の後に起こりがちな gateway 側の失敗を可視化。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | リリースを gate し、rollbacks を検証する合成 probe。 |
| `npm run check:links` | 壊れたリンクの gate。各ミティゲーション後に使用。 |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` でラップ) | エイリアスの昇格/巻き戻しの仕組み。 |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | refusals/alias/TLS/replication テレメトリーを集約。PagerDuty アラートはこれらのパネルを証拠として参照。 |

## ランブック - デプロイ失敗または不正アーティファクト

### トリガー条件

- preview/production の probes が失敗 (`npm run probe:portal -- --expect-release=...`).
- Grafana が `torii_sorafs_gateway_refusals_total` または
  `torii_sorafs_manifest_submit_total{status="error"}` を rollout 後にアラート。
- QA が alias 昇格直後に壊れたルートや Try it proxy の失敗を検知。

### 即時封じ込め

1. **デプロイ凍結:** CI パイプラインに `DEPLOY_FREEZE=1` (GitHub workflow input) を設定するか、
   Jenkins ジョブを停止して追加のアーティファクトを止める。
2. **アーティファクト収集:** 失敗ビルドの `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}` と probe 出力をダウンロードし、
   ロールバックで正確な digest を参照できるようにする。
3. **関係者通知:** storage SRE、Docs/DevRel のリード、ガバナンス当番に通知
   (特に `docs.sora` への影響時)。

### ロールバック手順

1. 最後に正常だった manifest (LKG) を特定。production workflow は
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` に保存。
2. shipping helper で alias をその manifest に再バインド:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. ロールバックの summary を incident チケットに記録し、LKG と失敗 manifest の digest を添付。

### 検証

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` と `sorafs_cli proof verify ...`
   (deploy guide 参照) で再昇格した manifest がアーカイブ済み CAR と一致するか確認。
4. `npm run probe:tryit-proxy` で Try-It staging proxy の復旧を確認。

### 事後対応

1. 根本原因を理解した後にのみデプロイパイプラインを再開。
2. [`devportal/deploy-guide`](./deploy-guide) の "Lessons learned" に追加の教訓を追記。
3. 失敗したテストスイート (probe, link checker, など) の defect を登録。

## ランブック - レプリケーション劣化

### トリガー条件

- アラート: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` が 10 分継続。
- `torii_sorafs_replication_backlog_total > 10` が 10 分継続 (参照: `pin-registry-ops.md`).
- ガバナンスが release 後の alias 可用性の遅延を報告。

### トリアージ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) ダッシュボードを確認し、
   backlog が storage class か provider fleet のどちらに局所化しているか判断。
2. Torii ログの `sorafs_registry::submit_manifest` warning を確認し、
   submissions 自体が失敗しているかを特定。
3. `sorafs_cli manifest status --manifest ...` で replica 健全性をサンプル取得
   (provider ごとの replication 結果を表示)。

### ミティゲーション

1. `scripts/sorafs-pin-release.sh` で replica 数を増やした manifest (`--pin-min-replicas 7`) を再発行し、
   scheduler がより多い providers に負荷分散できるようにする。新しい digest を incident log に記録。
2. backlog が単一 provider に起因する場合、replication scheduler で一時無効化
   (`pin-registry-ops.md` に記載) し、他の providers に alias 更新を強制する manifest を提出。
3. alias の新鮮さが replication parity より重要な場合、staging 済みの温かい manifest (`docs-preview`) に
   alias を再バインドし、SRE が backlog を解消した後にフォローアップ manifest を発行。

### 復旧とクローズ

1. `torii_sorafs_replication_sla_total{outcome="missed"}` を監視し、カウントの収束を確認。
2. `sorafs_cli manifest status` の出力をエビデンスとして保存し、全 replica が準拠状態に戻ったことを示す。
3. replication backlog の post-mortem を作成/更新し、次のステップを記録
   (provider scaling, chunker tuning, など)。

## ランブック - アナリティクス/テレメトリー停止

### トリガー条件

- `npm run probe:portal` は成功するが `AnalyticsTracker` のイベントが
  15 分以上ダッシュボードに取り込まれない。
- privacy review がドロップイベントの急増を報告。
- `npm run probe:tryit-proxy` が `/probe/analytics` で失敗。

### 対応

1. build-time の入力を確認: `DOCS_ANALYTICS_ENDPOINT` と
   `DOCS_ANALYTICS_SAMPLE_RATE` が release artifact (`build/release.json`) に設定されているか。
2. `DOCS_ANALYTICS_ENDPOINT` を staging collector に向けて `npm run probe:portal` を再実行し、
   tracker が payload を送信しているか確認。
3. collectors がダウンしている場合、`DOCS_ANALYTICS_ENDPOINT=""` を設定して rebuild し、
   tracker を短絡させる。障害ウィンドウを incident timeline に記録。
4. `scripts/check-links.mjs` が `checksums.sha256` の fingerprint を継続して出力することを確認
   (analytics 停止で sitemap 検証を *止めない* )。
5. collector 復旧後、`npm run test:widgets` を実行して analytics helper の unit tests を通してから再公開。

### 事後対応

1. 新しい collector 制限や sampling 要件があれば [`devportal/observability`](./observability) を更新。
2. analytics データがポリシー外で欠落/マスクされた場合は governance 通知を行う。

## 四半期レジリエンスドリル

**各四半期の最初の火曜日** (Jan/Apr/Jul/Oct) に両方のドリルを実施するか、重大な
インフラ変更の直後に実施します。アーティファクトは
`artifacts/devportal/drills/<YYYYMMDD>/` に保存します。

| Drill | 手順 | エビデンス |
| ----- | ----- | -------- |
| エイリアスロールバック演習 | 1. 最新の production manifest を使って "Failed deployment" の rollback を再現。<br/>2. probe が通ったら production に再バインド。<br/>3. `portal.manifest.submit.summary.json` と probe ログを drill フォルダに保存。 | `rollback.submit.json`, probe の出力、演習の release tag。 |
| 合成検証監査 | 1. production と staging に対して `npm run probe:portal` と `npm run probe:tryit-proxy` を実行。<br/>2. `npm run check:links` を実行して `build/link-report.json` を保存。<br/>3. probe 成功を示す Grafana パネルのスクリーンショット/エクスポートを添付。 | probe ログと manifest fingerprint を参照する `link-report.json`。 |

ドリル未実施は Docs/DevRel マネージャと SRE governance review にエスカレーションしてください。
ロードマップは、エイリアスの rollback と portal probes が健全であることを示す
四半期ごとの決定的な証拠を要求しています。

## PagerDuty とオンコール連携

- PagerDuty サービス **Docs Portal Publishing** が `dashboards/grafana/docs_portal.json` から生成される
  アラートを所有します。`DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`,
  `DocsPortal/TLSExpiry` のルールは Docs/DevRel の primary を page し、Storage SRE を secondary に設定します。
- page を受けたら `DOCS_RELEASE_TAG` を含め、影響を受けた Grafana パネルのスクリーンショットを添付し、
  mitigation 開始前に probe/link-check 出力を incident notes にリンクします。
- mitigation (rollback または redeploy) の後、`npm run probe:portal`, `npm run check:links` を再実行し、
  メトリクスが閾値内に戻った Grafana スナップショットを取得します。解決前に全証拠を PagerDuty
  インシデントへ添付します。
- 2 つのアラートが同時に発火した場合 (例: TLS expiry と backlog)、最初に refusals をトリアージし
  (publishing を停止)、rollback 手順を実行してから Storage SRE と橋上で TLS/backlog を解消します。
