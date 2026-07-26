---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w2-plan
タイトル: 摂取計画 W2
サイドバーラベル: プラン W2
説明: 摂取、承認、および事前準備のチェックリスト プレビュー コミュナウテール。
---

|要素 |詳細 |
| --- | --- |
|あいまい | W2 - 査読者コミュニティ |
|フェネトレ・シブル | 2025 年第 3 四半期 セメイン 1 (テンタティフ) |
|タグダルティファクト (planifie) | `preview-2025-06-15` |
|問題追跡ツール | `DOCS-SORA-Preview-W2` |

## 目的

1. 受け入れコミュニケーションと審査のワークフローの基準を定義します。
2. 名簿の承認ガバナンスおよび承認可能な使用法に関する追加事項を取得します。
3. アーティファクト プレビューは、チェックサムとテレメトリーのバンドルを検証し、新しいフェネトルを生成します。
4. プロキシを準備して、招待状の前にダッシュボードなどを試してみてください。

## デコパージュ デ タッシュ

| ID |タチェ |責任者 |エシャンス |法令 |メモ |
| --- | --- | --- | --- | --- | --- |
| W2-P1 |コミュニティの受け入れ基準 (適格性、最大スロット、要件 CoC) とガバナンスを拡散するためのガイドラインを作成する |ドキュメント/DevRel リード | 2025-05-15 |ターミナル | La politique d'intake a ete mergee dans `DOCS-SORA-Preview-W2` et endossee lors de la Congress du conseil 2025-05-20. |
| W2-P2 |質問に関するコミュニケーション (モチベーション、ディスポニビライト、ローカリゼーションの必要性) |ドキュメントコア-01 | 2025-05-18 |ターミナル | `docs/examples/docs_preview_request_template.md` には、コミュニティのセクションのメンテナンス、公式の参照が含まれます。 |
| W2-P3 |計画の承認を得るガバナンスを取得する (同窓会で投票 + 登録議事録) |ガバナンス連絡窓口 | 2025-05-22 |ターミナル | 2025 年 5 月 20 日、全員一致で採択するよう投票する。分 + 点呼は `DOCS-SORA-Preview-W2` によって決まります。 |
| W2-P4 | Planifier le staging du proxy お試しください + ラ フェネートル W2 によるテレメトリのキャプチャ (`preview-2025-06-15`) |ドキュメント/DevRel + オペレーション | 2025-06-05 |ターミナル |チケット変更 `OPS-TRYIT-188` 承認および実行 2025-06-09 02:00-04:00 UTC;スクリーンショット Grafana アーカイブの平均チケット。 |
| W2-P5 |新しいタグのアーティファクト プレビュー (`preview-2025-06-15`) およびアーカイバ記述子/チェックサム/プローブ ログの構築/検証者 |ポータルTL | 2025-06-07 |ターミナル | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025-06-10 を実行します。出力は在庫数 `artifacts/docs_preview/W2/preview-2025-06-15/` です。 |
| W2-P6 |コミュニティへの招待者名簿の作成者 (<= 25 人のレビュー担当者、多数の階層) の連絡先がガバナンスに基づいて承認されています。コミュニティマネージャー | 2025-06-10 |ターミナル | 8 人のレビュー担当者のプレミア共同体が承認します。トラッカーの要求 ID `DOCS-SORA-Preview-REQ-C01...C08` ログ。 |

## 事前チェックリスト

- [x] 承認ガバナンスの登録 (再会のメモ + 投票の先取特権) `DOCS-SORA-Preview-W2` を添付します。
- [x] `docs/examples/` のような、定期的な委員会に対する要求のテンプレート。
- [x] 記述子 `preview-2025-06-15`、ログ チェックサム、プローブ出力、リンク レポートおよびトランスクリプト プロキシ `artifacts/docs_preview/W2/` をストックしてみます。
- [x] スクリーンショット Grafana (`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`) は、ラ フェネトルのプリフライト W2 をキャプチャします。
- [x] Tableau 名簿の招待状、アベック ID レビュー担当者、チケット要求およびタイムスタンプ、承認の確認 (セクション W2 デュ トラッカー)。

Garder ce は 1 時間の計画を立てます。トラッカーは、ロードマップ DOCS-SORA を参照して、W2 の招待状を事前に確認してください。