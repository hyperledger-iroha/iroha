---
lang: ja
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

# Docs ポータル プレビューアクセス申請 (テンプレート)

公開プレビュー環境へのアクセスを付与する前にレビュー担当者の情報を収集する際にこの
テンプレートを使用する。Markdown を issue またはリクエストフォームにコピーし、
プレースホルダーを置き換える。

```markdown
## 申請概要
- 申請者: <氏名 / 組織>
- GitHub ハンドル: <username>
- 連絡手段: <email/Matrix/Signal>
- 地域とタイムゾーン: <UTC offset>
- 希望開始 / 終了日: <YYYY-MM-DD -> YYYY-MM-DD>
- レビュー種別: <Core maintainer | Partner | Community volunteer>

## コンプライアンス チェックリスト
- [ ] プレビュー acceptable-use policy に署名済み (link).
- [ ] `docs/portal/docs/devportal/security-hardening.md` を確認。
- [ ] `docs/portal/docs/devportal/incident-runbooks.md` を確認。
- [ ] テレメトリ収集と匿名化分析を承認 (yes/no).
- [ ] SoraFS の alias を要求 (yes/no). Alias 名: `<docs-preview-???>`

## アクセス要件
- プレビュー URL: <https://docs-preview.sora.link/...>
- 必要な API スコープ: <Torii read-only | Try it sandbox | none>
- 追加コンテキスト (SDK テスト、ドキュメントレビューの焦点など):
  <details here>

## 承認
- レビュー担当 (maintainer): <name + date>
- ガバナンスチケット / 変更申請: <link>
```

---

## コミュニティ向け質問 (W2+)
- プレビューアクセスの動機 (1文):
- 主なレビュー焦点 (SDK, governance, Norito, SoraFS, other):
- 週あたりの時間確保と対応可能ウィンドウ (UTC):
- ローカリゼーションまたはアクセシビリティの要件 (yes/no + details):
- コミュニティ行動規範 + プレビュー acceptable-use 追補を確認 (yes/no):
