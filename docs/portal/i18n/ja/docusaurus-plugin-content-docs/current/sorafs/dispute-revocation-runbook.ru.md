---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fabaac6014e0e67dca1a417354399b08d9e35fe54ce8465ec0eef5038f9f58e
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: dispute-revocation-runbook
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs/dispute_revocation_runbook.md` を反映しています。レガシーの Sphinx ドキュメントが退役するまで、両方のコピーを同期してください。
:::

## 目的

このランブックは、SoraFS の容量紛争の提出、失効の調整、データ退避が決定的に完了することの確保について、ガバナンス運用者をガイドします。

## 1. インシデントの評価

- **トリガー条件:** SLA 違反 (稼働率/PoR 失敗)、複製不足、または請求の不一致の検知。
- **テレメトリの確認:** プロバイダーの `/v1/sorafs/capacity/state` と `/v1/sorafs/capacity/telemetry` のスナップショットを取得します。
- **ステークホルダー通知:** Storage Team (プロバイダー運用)、Governance Council (意思決定機関)、Observability (ダッシュボード更新)。

## 2. 証拠バンドルの準備

1. 生のアーティファクト (telemetry JSON、CLI ログ、監査メモ) を収集します。
2. 決定的なアーカイブ (例: tarball) に正規化し、以下を記録します:
   - BLAKE3-256 digest (`evidence_digest`)
   - メディアタイプ (`application/zip`, `application/jsonl` など)
   - ホスティング URI (object storage、SoraFS pin、または Torii からアクセス可能な endpoint)
3. ガバナンスの証拠収集バケットに write-once アクセスで保管します。

## 3. 紛争の提出

1. `sorafs_manifest_stub capacity dispute` 用の JSON spec を作成します:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI を実行します:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` を確認します (種別、証拠 digest、タイムスタンプ)。
4. ガバナンス取引キュー経由で Torii `/v1/sorafs/capacity/dispute` にリクエスト JSON を送信します。応答の `dispute_id_hex` を記録してください。これは後続の失効アクションと監査レポートのアンカーとなります。

## 4. 退避と失効

1. **猶予期間:** 失効が迫っていることをプロバイダーに通知し、ポリシーで許される場合は pin されたデータの退避を許可します。
2. **`ProviderAdmissionRevocationV1` の生成:**
   - 承認済みの理由で `sorafs_manifest_stub provider-admission revoke` を使用します。
   - 署名と失効 digest を検証します。
3. **失効の公開:**
   - 失効リクエストを Torii に送信します。
   - プロバイダー adverts がブロックされていることを確認します (`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` が増加することを想定)。
4. **ダッシュボード更新:** プロバイダーを失効として表示し、紛争 ID を参照し、証拠バンドルへのリンクを付けます。

## 5. ポストモーテムとフォローアップ

- タイムライン、根本原因、是正措置をガバナンスのインシデントトラッカーに記録します。
- 補償 (ステークのスラッシュ、手数料の clawback、顧客返金) を決定します。
- 学びを文書化し、必要に応じて SLA 閾値や監視アラートを更新します。

## 6. 参考資料

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (紛争セクション)
- `docs/source/sorafs/provider_admission_policy.md` (失効ワークフロー)
- 監視ダッシュボード: `SoraFS / Capacity Providers`

## チェックリスト

- [ ] 証拠バンドルを収集し、ハッシュ化した。
- [ ] 紛争ペイロードをローカルで検証した。
- [ ] Torii の紛争トランザクションが受理された。
- [ ] 失効を実行した (承認済みの場合)。
- [ ] ダッシュボード/ランブックを更新した。
- [ ] ガバナンス評議会にポストモーテムを提出した。
