---
lang: ja
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

# Android パートナー SLA ヒアリングノート - テンプレート

AND8 SLA discovery セッションごとにこのテンプレートを使用する。記入済みのコピーは
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` に保存し、
サポート artefacts (質問票回答、確認連絡、添付) を同じディレクトリに添付する。

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. アジェンダとコンテキスト

- セッションの目的 (パイロット範囲、リリースウィンドウ、テレメトリ期待値)。
- 事前共有した参照 docs (support playbook、リリースカレンダー、
  テレメトリダッシュボード)。

## 2. ワークロード概要

| トピック | ノート |
|---------|--------|
| 対象ワークロード / チェーン | |
| 期待トランザクション量 | |
| 重要ビジネス時間 / blackout 期間 | |
| 規制レジーム (GDPR, MAS, FISC など) | |
| 必要言語 / ローカリゼーション | |

## 3. SLA 議論

| SLA クラス | パートナー期待値 | baseline との差分? | 必要アクション |
|-----------|------------------|--------------------|----------------|
| クリティカル修正 (48 h) | | Yes/No | |
| 高優先度 (5 business days) | | Yes/No | |
| メンテナンス (30 days) | | Yes/No | |
| Cutover 通知 (60 days) | | Yes/No | |
| インシデント連絡頻度 | | Yes/No | |

パートナーが要求した追加 SLA 条項を記録する (例: 専用電話ブリッジ、
追加テレメトリエクスポート)。

## 4. テレメトリとアクセス要件

- Grafana / Prometheus のアクセス要件:
- ログ/トレースのエクスポート要件:
- オフライン証跡またはドシエの期待値:

## 5. コンプライアンスと法務ノート

- 管轄通知要件 (法令 + タイミング)。
- インシデント更新用の法務連絡先。
- データレジデンシ制約 / 保存要件。

## 6. 決定事項とアクションアイテム

| 項目 | Owner | 期限 | ノート |
|------|-------|------|-------|
| | | | |

## 7. 了承

- パートナーは baseline SLA を了承したか? (Y/N)
- フォローアップの承認方法 (email / ticket / signature):
- 確認メールまたは議事録を、このディレクトリに添付してからクローズする。
