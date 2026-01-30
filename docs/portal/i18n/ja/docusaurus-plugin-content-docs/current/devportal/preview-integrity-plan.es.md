---
lang: es
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-integrity-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f167411de8854b6d0d0e525fd068a657e3ae75f3d60480dc31658491eea2d614
source_last_modified: "2025-11-07T10:33:21.912776+00:00"
translation_last_reviewed: 2026-01-30
---

# チェックサム検証付きプレビュー計画

この計画は、公開前にポータルの各プレビュー成果物を検証可能にするために必要な残作業をまとめたものです。目的は、レビュー担当者がCIでビルドされた正確なスナップショットをダウンロードすること、チェックサムマニフェストが不変であること、そしてNoritoメタデータ付きでSoraFSからプレビューを発見できることを保証することです。

## 目的

- **決定的ビルド:** `npm run build` が再現可能な出力を生成し、常に `build/checksums.sha256` を出力することを保証する。
- **検証済みプレビュー:** 各プレビュー成果物にチェックサムマニフェストを同梱し、検証に失敗した場合は公開を拒否する。
- **Noritoで公開するメタデータ:** プレビュー記述子（コミットメタデータ、チェックサムのダイジェスト、SoraFS CID）をNorito JSONとして保存し、ガバナンスツールがリリースを監査できるようにする。
- **運用者向けツール:** 利用者がローカルで実行できるワンステップ検証スクリプト（`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`）を提供する。スクリプトはチェックサム+記述子の検証フローをエンドツーエンドで包含するようになった。標準のプレビューコマンド（`npm run serve`）は `docusaurus serve` の前にこのヘルパーを自動で呼び出し、ローカルスナップショットがチェックサムで保護されるようにする（`npm run serve:verified` は明示的なエイリアスとして維持）。

## フェーズ1 — CIでの強制

1. `.github/workflows/docs-portal-preview.yml` を更新して次を実行する:
   - Docusaurus ビルド後に `node docs/portal/scripts/write-checksums.mjs` を実行（ローカルでは既に実行済み）。
   - `cd build && sha256sum -c checksums.sha256` を実行し、不一致ならジョブを失敗させる。
   - build ディレクトリを `artifacts/preview-site.tar.gz` としてパッケージし、チェックサムマニフェストをコピーし、`scripts/generate-preview-descriptor.mjs` を呼び出し、JSON 設定（`docs/examples/sorafs_preview_publish.json` を参照）で `scripts/sorafs-package-preview.sh` を実行して、メタデータと決定的な SoraFS バンドルを生成する。
   - 静的サイト、メタデータ成果物（`docs-portal-preview`, `docs-portal-preview-metadata`）、SoraFS バンドル（`docs-portal-preview-sorafs`）をアップロードし、マニフェスト、CAR サマリ、計画を再ビルドなしで確認できるようにする。
2. pull request にチェックサム検証結果を要約した CI バッジコメントを追加する（`docs-portal-preview.yml` の GitHub Script コメントステップで実装）。
3. `docs/portal/README.md`（CI セクション）にワークフローを記載し、公開チェックリストに検証手順へのリンクを追加する。

## 検証スクリプト

`docs/portal/scripts/preview_verify.sh` は、ダウンロードしたプレビュー成果物を手動の `sha256sum` 呼び出しなしで検証します。ローカルスナップショットを共有する際は `npm run serve`（または明示的なエイリアス `npm run serve:verified`）を使い、スクリプトを実行して `docusaurus serve` をワンステップで起動します。検証ロジック:

1. `build/checksums.sha256` に対して適切な SHA ツール（`sha256sum` または `shasum -a 256`）を実行する。
2. 必要に応じて、プレビュー記述子の `checksums_manifest` のダイジェスト/ファイル名、そして指定があればプレビューアーカイブのダイジェスト/ファイル名を比較する。
3. 不一致が検出された場合は非ゼロ終了し、レビュー担当者が改ざんされたプレビューをブロックできるようにする。

使用例（CI成果物を展開した後）:

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CIおよびリリース担当のエンジニアは、プレビューバンドルをダウンロードするたび、またはリリースチケットに成果物を添付するたびにこのスクリプトを実行すること。

## フェーズ2 — SoraFS 公開

1. プレビューワークフローに次のジョブを追加する:
   - `sorafs_cli car pack` と `manifest submit` を使ってビルド済みサイトを SoraFS の staging ゲートウェイにアップロードする。
   - 返されたマニフェストのダイジェストと SoraFS CID を取得する。
   - `{ commit, branch, checksum_manifest, cid }` を Norito JSON としてシリアライズする（`docs/portal/preview/preview_descriptor.json`）。
2. 記述子をビルド成果物と一緒に保存し、pull request のコメントに CID を表示する。
3. `sorafs_cli` を dry-run モードで実行する統合テストを追加し、将来の変更でもメタデータスキーマの互換性を維持する。

## フェーズ3 — ガバナンスと監査

1. 記述子構造を表す Norito スキーマ（`PreviewDescriptorV1`）を `docs/portal/schemas/` に公開する。
2. DOCS-SORA 公開チェックリストを更新し、次を要求する:
   - `sorafs_cli manifest verify` をアップロード済み CID に対して実行する。
   - チェックサムマニフェストのダイジェストと CID をリリース PR の説明に記録する。
3. リリース投票中に記述子とチェックサムマニフェストを照合するガバナンス自動化を接続する。

## 成果物と担当

| マイルストーン | 担当 | 目標 | 注記 |
|----------------|------|------|------|
| CIのチェックサム強制が完了 | Docs インフラ | 週1 | 失敗ゲートと成果物アップロードを追加。 |
| SoraFS プレビュー公開 | Docs インフラ / Storage チーム | 週2 | staging 資格情報へのアクセスと Norito スキーマ更新が必要。 |
| ガバナンス統合 | Docs/DevRel リード / ガバナンスWG | 週3 | スキーマを公開し、チェックリストとロードマップ項目を更新。 |

## 未解決の質問

- どの SoraFS 環境がプレビュー成果物を保持するべきか（staging か専用プレビューレーン）?
- 公開前にプレビュー記述子へ二重署名（Ed25519 + ML-DSA）は必要か?
- `sorafs_cli` 実行時に CI ワークフローが orchestrator の設定（`orchestrator_tuning.json`）を固定し、マニフェストの再現性を保つべきか?

`docs/portal/docs/reference/publishing-checklist.md` に決定事項を記録し、不明点が解消されたらこの計画を更新してください。
