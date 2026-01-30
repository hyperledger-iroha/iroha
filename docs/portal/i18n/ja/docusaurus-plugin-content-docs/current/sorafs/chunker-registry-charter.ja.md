---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb3d1cec933b2d64f0a0e4e8be9651d868b743c22d8e66048eff91da536c56ab
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry-charter
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
`docs/source/sorafs/chunker_registry_charter.md` を反映しています。レガシーの Sphinx ドキュメントセットが退役するまで、両方のコピーを同期してください。
:::

# SoraFS チャンカー・レジストリ ガバナンス・チャーター

> **批准:** 2025-10-29 に Sora Parliament Infrastructure Panel が批准（
> `docs/source/sorafs/council_minutes_2025-10-29.md` を参照）。改定には正式なガバナンス投票が必要であり、
> 後継チャーターが承認されるまで本書を規範として扱うこと。

このチャーターは、SoraFS チャンカー・レジストリを進化させるためのプロセスと役割を定義します。
[Chunker Profile Authoring Guide](./chunker-profile-authoring.md) を補完し、新しいプロファイルが提案・レビュー・批准され、
最終的に廃止されるまでの流れを記述します。

## 適用範囲

このチャーターは `sorafs_manifest::chunker_registry` のすべてのエントリと、
レジストリを利用するあらゆる tooling（manifest CLI、provider-advert CLI、SDKs）に適用されます。
`chunker_registry::ensure_charter_compliance()` が検証する alias と handle の不変条件を強制します:

- プロファイル ID は正の整数で、単調に増加します。
- 正規ハンドル `namespace.name@semver` は `profile_aliases` の先頭エントリに
- alias 文字列は trim され、ユニークで、他エントリの正規ハンドルと衝突しません。

## 役割

- **Author(s)** – 提案を準備し、fixtures を再生成し、決定性の証拠を収集します。
- **Tooling Working Group (TWG)** – 公開済みチェックリストで提案を検証し、レジストリの不変条件が守られていることを確認します。
- **Governance Council (GC)** – TWG レポートをレビューし、提案エンベロープに署名し、公開/廃止のタイムラインを承認します。
- **Storage Team** – レジストリ実装を維持し、ドキュメント更新を公開します。

## ライフサイクルのワークフロー

1. **提案提出**
   - 著者はオーサリングガイドの検証チェックリストを実行し、
     `docs/source/sorafs/proposals/` に `ChunkerProfileProposalV1` JSON を作成します。
   - 次の CLI 出力を含めます:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - fixtures、提案、決定性レポート、レジストリ更新を含む PR を提出します。

2. **ツーリングレビュー (TWG)**
   - 検証チェックリスト（fixtures、fuzz、manifest/PoR パイプライン）を再実行します。
   - `cargo test -p sorafs_car --chunker-registry` を実行し、
     `ensure_charter_compliance()` が新しいエントリで通ることを確認します。
   - CLI の挙動（`--list-profiles`, `--promote-profile`, streaming `--json-out=-`）が、
     更新された alias と handle を反映していることを確認します。
   - 調査結果と pass/fail ステータスを要約した短いレポートを作成します。

3. **評議会承認 (GC)**
   - TWG レポートと提案メタデータをレビューします。
   - 提案 digest (`blake3("sorafs-chunker-profile-v1" || bytes)`) に署名し、
     fixtures と並べて保持される評議会エンベロープへ署名を追加します。
   - 投票結果をガバナンスの議事録に記録します。

4. **公開**
   - PR をマージし、以下を更新します:
     - `sorafs_manifest::chunker_registry_data`.
     - ドキュメント (`chunker_registry.md`, authoring/conformance ガイド)。
     - fixtures と決定性レポート。
   - 新しいプロファイルと計画されたロールアウトをオペレーターと SDK チームへ通知します。

5. **廃止 / サンセット**
   - 既存プロファイルを置き換える提案は、二重公開ウィンドウ（猶予期間）と upgrade 計画を含める必要があります。
     移行レジャーを更新します。

6. **緊急変更**
   - 削除や hotfix には、評議会の過半数承認投票が必要です。
   - TWG はリスク緩和の手順を文書化し、インシデントログを更新します。

## ツーリングに求めること

- `sorafs_manifest_chunk_store` と `sorafs_manifest_stub` は以下を提供します:
  - レジストリの検査に使う `--list-profiles`。
  - プロファイル昇格時に使用する正規メタデータブロックを生成する `--promote-profile=<handle>`。
  - レポートを stdout にストリームし、再現可能なレビュー・ログを作る `--json-out=-`。
- `ensure_charter_compliance()` は関連バイナリの起動時に呼び出されます
  (`manifest_chunk_store`, `provider_advert_stub`)。新しいエントリがチャーターに違反した場合、CI テストは失敗する必要があります。

## 記録管理

- すべての決定性レポートは `docs/source/sorafs/reports/` に保管します。
- chunker の決定に言及する評議会の議事録は
  `docs/source/sorafs/migration_ledger.md` に保存します。
- レジストリの主要な変更後に `roadmap.md` と `status.md` を更新します。

## 参考資料

- オーサリングガイド: [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)
- 準拠チェックリスト: `docs/source/sorafs/chunker_conformance.md`
- レジストリ参照: [Chunker Profile Registry](./chunker-registry.md)
