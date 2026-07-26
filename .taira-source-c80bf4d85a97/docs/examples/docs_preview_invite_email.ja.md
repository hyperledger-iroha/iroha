---
lang: ja
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

# Docs ポータル プレビュー招待 (サンプルメール)

外部向けメッセージを作成する際はこのサンプルを使う。この文面は W2 コミュニティレビュー
(`preview-2025-06-15`) に送った実際のコピーを反映しているため、今後の wave が古いチケットを
掘り返すことなくトーン、検証ガイダンス、証跡の流れを揃えられる。新しい招待を送る前に、
アーティファクトのリンク、ハッシュ、リクエスト ID、日付を更新すること。

```text
件名: [DOCS-SORA] docs ポータル プレビュー preview-2025-06-15 招待 Horizon Wallet 向け

こんにちは Sam,

W2 コミュニティプレビューに Horizon Wallet を再度ご提供いただきありがとうございます。W2
は承認済みのため、以下の手順を完了次第レビューを開始できます。アーティファクトとアクセス
トークンは必ず非公開にしてください。すべての招待は DOCS-SORA-Preview-W2 で追跡され、
監査チームが承認記録を確認します。

1. 検証済みアーティファクトをダウンロード (SoraFS/CI に配布したものと同一):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. 展開前に bundle を検証:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. チェックサム enforcement を有効にしてプレビューを起動:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. テスト前に hardened runbooks を確認:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. フィードバックは DOCS-SORA-Preview-REQ-C04 で提出し、各指摘に
   `docs-preview/w2` を付ける。構造化 intake を希望する場合は
   docs/examples/docs_preview_feedback_form.md を使う。

サポートは Matrix (`#docs-preview:matrix.org`) にあります。office hours は
2025-06-18 15:00 UTC に実施します。セキュリティ/インシデントのエスカレーションは
ops@sora.org または +1-555-0109 の docs on-call 宛てに直ちに連絡してください。
office hours を待たないでください。

Horizon Wallet のプレビューアクセス期間は 2025-06-15 -> 2025-06-29 です。完了次第
お知らせいただければ一時アクセスキーを失効させ、トラッカーへクローズを記録します。

ポータルの GA に向けたご協力に感謝します。

- DOCS-SORA チーム
```
