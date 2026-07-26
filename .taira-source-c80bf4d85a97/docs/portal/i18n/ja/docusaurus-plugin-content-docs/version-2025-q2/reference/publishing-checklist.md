---
lang: ja
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# チェックリストの公開

開発者ポータルを更新するときは常に、このチェックリストを使用してください。それは、
CI のビルド、GitHub Pages のデプロイメント、および手動のスモーク テストがすべてのセクションをカバーします
リリースまたはロードマップのマイルストーンが到着する前。

## 1. ローカル検証

- `npm run sync-openapi -- --version=current --latest` (1 つ以上を追加)
  凍結されたスナップショットに対して Torii OpenAPI が変更されると、`--mirror=<label>` フラグが立てられます)。
- `npm run build` – `Build on Iroha with confidence` ヒーロー コピーを確認します。
  `build/index.html` に表示されます。
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – を確認します。
  チェックサム マニフェスト (ダウンロードした CI をテストするときに `--descriptor`/`--archive` を追加)
  工芸品）。
- `npm run serve` – チェックサムゲート付きプレビューヘルパーを起動し、
  `docusaurus serve` を呼び出す前にマニフェストを作成するため、レビュー担当者がマニフェストを参照することはありません。
  署名されていないスナップショット (`serve:verified` エイリアスは明示的な呼び出し用に残ります)。
- `npm run start` 経由でタッチしたマークダウンとライブ リロードをスポットチェックします
  サーバー。

## 2. プルリクエストのチェック

- `docs-portal-build` ジョブが `.github/workflows/check-docs.yml` で成功したことを確認します。
- `ci/check_docs_portal.sh` が実行されたことを確認します (CI ログにヒーローの煙チェックが表示されます)。
- プレビュー ワークフローでマニフェスト (`build/checksums.sha256`) がアップロードされていることを確認します。
  プレビュー検証スクリプトは成功しました (CI ログには、
  `scripts/preview_verify.sh` 出力)。
- GitHub Pages 環境から公開されたプレビュー URL を PR に追加します
  説明。

## 3. セクションの承認

|セクション |オーナー |チェックリスト |
|----------|----------|----------|
|ホームページ |開発者 |ヒーロー コピーがレンダリングされ、クイックスタート カードが有効なルートにリンクされ、CTA ボタンが解決されます。 |
| Norito | Norito WG |概要ガイドと入門ガイドでは、最新の CLI フラグと Norito スキーマ ドキュメントを参照しています。 |
| SoraFS |ストレージチーム |クイックスタートは完了まで実行され、マニフェスト レポート フィールドが文書化され、フェッチ シミュレーション命令が検証されます。 |
| SDK ガイド | SDK のリード | Rust/Python/JS ガイドは現在のサンプルをコンパイルし、ライブ リポジトリにリンクします。 |
|参考資料 |ドキュメント/開発リリース |インデックスには最新の仕様がリストされており、Norito コーデック リファレンスは `norito.md` と一致します。 |
|アーティファクトのプレビュー |ドキュメント/開発リリース | PR に添付された `docs-portal-preview` アーティファクト、スモーク チェック パス、レビュー担当者と共有されたリンク。 |
|セキュリティとサンドボックスを試す |ドキュメント/DevRel · セキュリティ | OAuth デバイス コード ログインが構成され (`DOCS_OAUTH_*`)、`security-hardening.md` チェックリストが実行され、CSP/Trusted Types ヘッダーが `npm run build` または `npm run probe:portal` で検証されました。 |

各行を PR レビューの一部としてマークするか、フォローアップ タスクをメモして、ステータスを確認します。
追跡は正確なままです。

## 4. リリースノート

- `https://docs.iroha.tech/` (または環境 URL) を含めます
  デプロイメント ジョブから) リリース ノートとステータス更新に記載されています。
- 新しいセクションまたは変更されたセクションを明示的に呼び出して、下流チームがどこにあるかを把握できるようにします。
  独自のスモークテストを再実行します。