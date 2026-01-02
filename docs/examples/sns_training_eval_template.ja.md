---
lang: ja
direction: ltr
source: docs/examples/sns_training_eval_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0397eec8373494a74060a7244a7f923714723e5feacc4a54374ee4461f0ddde1
source_last_modified: "2025-11-15T09:17:43.374603+00:00"
translation_last_reviewed: 2026-01-01
---

# SNS トレーニング評価テンプレート

各セッション終了直後にこのアンケートを配布する。回答はフォームまたは Markdown で回収し、
`artifacts/sns/training/<suffix>/<cycle>/feedback/` に保存する。

## セッションメタデータ
- サフィックス:
- サイクル:
- 言語:
- 日付:
- ファシリテーター:

## 評価スケール
1 - 低い / 2 - やや低い / 3 - 良い / 4 - とても良い / 5 - 最高

| 質問 | 1 | 2 | 3 | 4 | 5 |
|------|---|---|---|---|---|
| KPI walkthrough の明確さ | [ ] | [ ] | [ ] | [ ] | [ ] |
| ラボの有用性 | [ ] | [ ] | [ ] | [ ] | [ ] |
| ペース + 時間配分 | [ ] | [ ] | [ ] | [ ] | [ ] |
| ローカライズ品質 (slides + facilitation) | [ ] | [ ] | [ ] | [ ] | [ ] |
| サフィックスローンチに向けた全体的な自信 | [ ] | [ ] | [ ] | [ ] | [ ] |

## 自由記述
1. もっと深掘りが必要なトピックは?
2. ワークブックに不足していたツール/ドキュメントは?
3. ローカライズは期待に沿っていたか? もし違うなら理由は?
4. プログラムが追跡すべき追加コメント/ブロッカー。

## フォローアップ
- `[]` governance tracker にフィードバックを記録 (ticket: __________)
- `[]` annex export を取得 (path: ____________________)
- `[]` アクション項目を割り当て (owner + 期限)
