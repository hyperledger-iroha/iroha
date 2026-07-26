---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-11-05T23:45:36.259903+00:00"
translation_last_reviewed: 2026-01-30
---

# 公開チェックリスト

開発者ポータルを更新するたびにこのチェックリストを使用してください。CI ビルド、GitHub Pages のデプロイ、手動の smoke テストが、リリースやロードマップのマイルストーンに到達する前にすべてのセクションをカバーすることを保証します。

## 1. ローカル検証

- `npm run sync-openapi -- --version=current --latest`（Torii OpenAPI が変更された場合は `--mirror=<label>` を 1 つ以上追加して凍結スナップショットを作成）。
- `npm run build` – `Build on Iroha with confidence` の hero コピーが `build/index.html` に引き続き表示されることを確認します。
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – チェックサムマニフェストを検証します（CI からダウンロードした artefact をテストする場合は `--descriptor`/`--archive` を追加）。
- `npm run serve` – チェックサムでゲートされた preview helper を起動し、`docusaurus serve` を呼ぶ前にマニフェストを検証するので、reviewer が署名無しスナップショットを閲覧することはありません（`serve:verified` エイリアスは明示的な呼び出し用に残ります）。
- `npm run start` とライブリロードサーバーを使って変更した markdown をスポットチェックします。

## 2. プルリクエストのチェック

- `.github/workflows/check-docs.yml` の `docs-portal-build` ジョブが成功したことを確認します。
- `ci/check_docs_portal.sh` が実行されたことを確認します（CI ログに hero smoke check が表示されます）。
- preview ワークフローがマニフェスト（`build/checksums.sha256`）をアップロードし、preview 検証スクリプトが成功したことを確認します（CI ログに `scripts/preview_verify.sh` の出力が表示されます）。
- GitHub Pages 環境の公開 preview URL を PR の説明に追加します。

## 3. セクションのサインオフ

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Hero コピーが表示され、quickstart カードが有効なルートにリンクし、CTA ボタンが解決される。 |
| Norito | Norito WG | overview と getting-started ガイドが最新の CLI フラグと Norito スキーマ docs を参照している。 |
| SoraFS | Storage Team | quickstart が最後まで実行でき、manifest レポートのフィールドが文書化され、fetch シミュレーション手順が検証されている。 |
| SDK guides | SDK leads | Rust/Python/JS のガイドが現在の例をコンパイルし、ライブのリポジトリへリンクしている。 |
| Reference | Docs/DevRel | インデックスに最新の specs が掲載され、Norito codec reference が `norito.md` と一致している。 |
| Preview artifact | Docs/DevRel | `docs-portal-preview` アーティファクトが PR に添付され、smoke checks が通り、リンクが reviewers に共有されている。 |
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device-code login が設定され（`DOCS_OAUTH_*`）、`security-hardening.md` のチェックリストが実行され、CSP/Trusted Types ヘッダーが `npm run build` または `npm run probe:portal` で検証されている。 |

各行を PR レビューの一部としてチェックするか、フォローアップタスクを記録してステータス追跡を正確に保ってください。

## 4. リリースノート

- `https://docs.iroha.tech/`（またはデプロイジョブの環境 URL）をリリースノートとステータス更新に含めてください。
- 新規または変更されたセクションを明示し、下流チームがどこで自分たちの smoke テストを再実行すべきか分かるようにします。
