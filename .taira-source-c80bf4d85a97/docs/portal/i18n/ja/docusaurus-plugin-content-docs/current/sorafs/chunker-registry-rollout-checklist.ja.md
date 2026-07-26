---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b305acb6a08f736ea8e47321d23a6a44d7af18b0d75aff34a08032264ad4e38c
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry-rollout-checklist
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
`docs/source/sorafs/chunker_registry_rollout_checklist.md` を反映しています。レガシーの Sphinx ドキュメントセットが退役するまで、両方のコピーを同期してください。
:::

# SoraFS レジストリ ロールアウトチェックリスト

このチェックリストは、ガバナンス憲章が承認された後に、新しい chunker
プロファイルまたは provider admission bundle をレビューから本番へ昇格するための
手順をまとめています。

> **適用範囲:** `sorafs_manifest::chunker_registry`、provider admission envelopes、
> または正規 fixture bundles（`fixtures/sorafs_chunker/*`）を変更するすべての
> リリースに適用されます。

## 1. 事前検証

1. fixtures を再生成して決定性を確認します:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md`（または対象プロファイルの
   レポート）の決定性ハッシュが再生成された成果物と一致することを確認します。
3. `sorafs_manifest::chunker_registry` が `ensure_charter_compliance()` で
   コンパイルできることを次のコマンドで確認します:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 提案ドシエを更新します:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` の評議会議事録エントリ
   - 決定性レポート

## 2. ガバナンス承認

1. Tooling Working Group レポートと提案ダイジェストを Sora Parliament
   Infrastructure Panel に提示します。
2. 承認内容を `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` に記録します。
3. Parliament 署名済みエンベロープを fixtures と一緒に公開します:
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. ガバナンス取得ヘルパーでエンベロープにアクセスできることを確認します:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージングロールアウト

詳細な手順は [ステージング manifest プレイブック](./staging-manifest-playbook) を
参照してください。

1. `torii.sorafs` discovery を有効にし、admission enforcement をオン
   (`enforce_admission = true`) にした Torii をデプロイします。
2. `torii.sorafs.discovery.admission.envelopes_dir` が指す staging レジストリ
   ディレクトリへ、承認済み provider admission envelopes を配置します。
3. discovery API 経由で provider adverts が伝播していることを確認します:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. ガバナンスヘッダー付きで manifest/plan エンドポイントを実行します:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. テレメトリダッシュボード（`torii_sorafs_*`）とアラートルールが、新しい
   プロファイルをエラーなく報告していることを確認します。

## 4. 本番ロールアウト

1. ステージングの手順を本番 Torii ノードで繰り返します。
2. アクティベーションウィンドウ（日時、猶予期間、ロールバック計画）を
   オペレーターと SDK チャンネルに告知します。
3. 次を含むリリース PR をマージします:
   - 更新された fixtures とエンベロープ
   - ドキュメント変更（憲章参照、決定性レポート）
   - roadmap/status の更新
4. リリースにタグを付け、署名済みアーティファクトをプロビナンスのために
   アーカイブします。

## 5. ロールアウト後監査

1. ロールアウト 24 時間後に最終メトリクス（discovery 件数、fetch 成功率、
   エラーヒストグラム）を収集します。
2. `status.md` を短いサマリーと決定性レポートへのリンクで更新します。
3. 追跡タスク（例: 追加のプロファイル作成ガイダンス）を `roadmap.md` に記録します。
