---
lang: ja
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

# Docs ポータル プレビュー招待 (テンプレート)

レビュー担当者にプレビューアクセス手順を送る際にこのテンプレートを使用する。プレースホルダー
(`<...>`) を実際の値に置き換え、メッセージで参照する descriptor + archive の
アーティファクトを添付し、最終文面を対応する intake チケットに保存する。

```text
件名: [DOCS-SORA] docs ポータル プレビュー <preview_tag> 招待 <reviewer/org> 向け

こんにちは <name>,

GA 前の docs ポータルレビューに協力していただきありがとうございます。wave <wave_id>
に承認されています。プレビューを閲覧する前に以下の手順を実施してください:

1. CI もしくは SoraFS から検証済みアーティファクトをダウンロード:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archive: <archive_url> (`sha256:<archive_sha256>`)
2. チェックサムゲートを実行:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. チェックサム強制を有効にしてプレビューを起動:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. acceptable-use、security、observability の注意事項を確認:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. <request_ticket> でフィードバックを提出し、各指摘に `<preview_tag>` を付ける。

サポートは <contact_channel> で受けられます。インシデントやセキュリティの問題は
<incident_channel> ですぐ報告してください。Torii API トークンが必要な場合は
チケット経由で申請し、production の資格情報を再利用しないでください。

プレビューアクセスは <end_date> に失効します (書面で延長されない限り)。ガバナンスのため
checksums と招待メタデータを記録しています。終了したら知らせてください。こちらで
クリーンにオフボードします。

改めてご協力ありがとうございます。

- DOCS-SORA チーム
```
