---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w2-plan
タイトル: プラノ デ インテーク コミュニティ W2
サイドバーラベル: Plano W2
説明: 摂取、プレビュー共同体による証拠のチェックリストの承認。
---

|アイテム |デタルヘス |
| --- | --- |
|恩田 | W2 - 査読者コミュニティ |
|ジャネラ・アルボ | 2025 年第 3 四半期 セマナ 1 (tentativa) |
| Tag de artefato (プラネハド) | `preview-2025-06-15` |
| do トラッカーの問題 | `DOCS-SORA-Preview-W2` |

## オブジェクト

1. 受け入れコミュニティとワークフローの審査基準を定義します。
2. 事前に政府の承認を得て、名簿を事前に承認し、追加文書を作成します。
3. 新しいジャネラのテレメトリのバンドルとチェックサムのプレビュー検証の最新情報。
4. プロキシを準備します。環境を整える前に、OS ダッシュボードを試してください。

## デスドブラメント デ タレファス

| ID |タレファ |返信 |プラゾ |ステータス |メモ |
| --- | --- | --- | --- | --- | --- |
| W2-P1 |摂取コミュニティの基準 (資格、最大スロット数、CoC 要件) と政府の循環 |ドキュメント/DevRel リード | 2025-05-15 |結論 | 2025 年 5 月 20 日の政治は、`DOCS-SORA-Preview-W2` と一致するものです。 |
| W2-P2 |コミュニティの要請テンプレート (動機、ディスポニビリダード、地域の必要性) |ドキュメントコア-01 | 2025-05-18 |結論 | `docs/examples/docs_preview_request_template.md` アゴラにはセカオ コミュニティが含まれており、公式の参照はありません。 |
| W2-P3 |ガランティル アプロバカオ デ ガバナンカ パラ オ プラノ デ インテーク (投票と登録 + 登録) |ガバナンス連絡窓口 | 2025-05-22 |結論 | 2025-05-20 に投票を承認してください。点呼リンクは `DOCS-SORA-Preview-W2` です。 |
| W2-P4 |プロキシのプログラマ ステージング Try it + ジャネラ W2 でのテレメトリのキャプチャ (`preview-2025-06-15`) |ドキュメント/DevRel + オペレーション | 2025-06-05 |結論 |チケット変更 `OPS-TRYIT-188` 2025-06-09 02:00-04:00 UTC に承認され、実行されます。スクリーンショット Grafana arquivados com o チケット。 |
| W2-P5 |プレビュー (`preview-2025-06-15`) アーキバー記述子/チェックサム/プローブ ログの作成/検証新しいタグ |ポータルTL | 2025-06-07 |結論 | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` ロドゥ em 2025-06-10;アルマゼナドス em `artifacts/docs_preview/W2/preview-2025-06-15/` を出力します。 |
| W2-P6 |委員会のメンバー名簿 (<=25 人の査読者、多数のエスカロナド) が政府の承認を得る |コミュニティマネージャー | 2025-06-10 |結論 | Primeiro の 8 人のレビュー担当者は、共同体として承認します。必要な ID `DOCS-SORA-Preview-REQ-C01...C08` にはトラッカーが登録されていません。 |

## 証拠のチェックリスト

- [x] Registro de aprovacao de Government (notas de reuniao + link de voto) anexado a `DOCS-SORA-Preview-W2`。
- [x] テンプレートは、泣き叫ぶ `docs/examples/` をコミットしました。
- [x] 記述子 `preview-2025-06-15`、チェックサム ログ、プローブ出力、リンク レポート、トランスクリプト、プロキシ `artifacts/docs_preview/W2/` を試してみてください。
- [x] スクリーンショット Grafana (`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`) は、Janela プリフライト W2 のキャプチャです。
- [x] レビュー担当者の COM ID、招待チケット、および事前準備のタイムスタンプを表示します (W2 ではトラッカーがありません)。

マンテンハ エステ プラノ アトゥアリザド。トラッカー、パラグラフ参照、ロードマップ DOCS-SORA は、W2 を招待する前に確認する必要があります。