---
lang: ja
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

# Docs preview フィードバックフォーム (W1 パートナー wave)

W1 レビュー担当者からフィードバックを収集する際にこのテンプレートを使用する。
パートナーごとに複製し、メタデータを記入し、完成版を
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` に保存する。

## レビュー担当者メタデータ

- **パートナーID:** `partner-w1-XX`
- **申請チケット:** `DOCS-SORA-Preview-REQ-PXX`
- **招待送信 (UTC):** `YYYY-MM-DD hh:mm`
- **checksum 承認 (UTC):** `YYYY-MM-DD hh:mm`
- **主なフォーカス領域:** (例: _SoraFS オーケストレータ docs_, _Torii ISO フロー_)

## テレメトリとアーティファクトの確認

| チェック項目 | 結果 | エビデンス |
| --- | --- | --- |
| checksum 検証 | ✅ / ⚠️ | ログのパス (例: `build/checksums.sha256`) |
| Try it proxy スモークテスト | ✅ / ⚠️ | `npm run manage:tryit-proxy …` の抜粋 |
| Grafana ダッシュボード確認 | ✅ / ⚠️ | スクリーンショットのパス |
| ポータル probe レポート確認 | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

レビュー担当者が追加の SLO を確認する場合は行を追加する。

## フィードバックログ

| 領域 | Severity (info/minor/major/blocker) | 内容 | 提案修正または質問 | トラッカー issue |
| --- | --- | --- | --- | --- |
| | | | | |

最後の列に GitHub issue または内部チケットを記載し、preview tracker が修正項目を
このフォームに紐付けられるようにする。

## アンケート要約

1. **checksum ガイダンスと招待プロセスへの信頼度は?** (1–5)
2. **どの docs が最も/最も役に立たなかった?** (短答)
3. **Try it proxy またはテレメトリダッシュボードへのアクセスでブロッカーはあった?**
4. **追加のローカリゼーションやアクセシビリティ内容は必要?**
5. **GA 前に他のコメントはある?**

短い回答を記録し、外部フォームを使った場合は生のアンケートエクスポートを添付する。

## 知識チェック

- スコア: `__/10`
- 不正解の質問 (あれば): `[#1, #4, …]`
- フォローアップ (スコア < 9/10): remediation call を予定したか? y/n

## サインオフ

- レビュー担当者名とタイムスタンプ:
- Docs/DevRel レビュー担当者名とタイムスタンプ:

署名済みのコピーを関連 artefacts と一緒に保管し、監査担当者が波を
追加のコンテキストなしで再現できるようにする。
