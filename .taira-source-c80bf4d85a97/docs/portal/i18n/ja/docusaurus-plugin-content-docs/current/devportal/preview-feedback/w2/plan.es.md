---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w2-plan
タイトル: 摂取コミュニティ W2 の計画
サイドバーラベル: プラン W2
説明: プレビュー共同体における証拠の摂取、使用法およびチェックリスト。
---

|アイテム |詳細 |
| --- | --- |
|オラ | W2 - 査読者コミュニティ |
|ベンタナオブジェティボ | 2025 年第 3 四半期 セマナ 1 (tentativa) |
|タグ・デ・アーティファクト (プラネアード) | `preview-2025-06-15` |
|問題デルトラッカー | `DOCS-SORA-Preview-W2` |

## オブジェクト

1. 摂取コミュニティと検査の基準を定義します。
2. 名簿の承認および追加条項を遵守します。
3. チェックサムとテレメトリのバンドルのプレビュー検証の成果物を新しいイベントに反映します。
4. プロキシを準備します。招待される前にダッシュボードで試してみてください。

## デグロース デ タレアス

| ID |タレア |責任者 |フェチャ限定 |エスタード |メモ |
| --- | --- | --- | --- | --- | --- |
| W2-P1 |摂取コミュニティの編集基準 (対象、最大スロット数、CoC の要件) と知事の回覧 |ドキュメント/DevRel リード | 2025-05-15 |コンプリート | `DOCS-SORA-Preview-W2` は、2025 年 5 月 20 日の再会での政治の融合を目指しています。 |
| W2-P2 |特定のコミュニティでの事前の要請テンプレートの実際 (動機、ディスポニビリダー、地域性の必要性) |ドキュメントコア-01 | 2025-05-18 |コンプリート | `docs/examples/docs_preview_request_template.md` アホラには、セクション コミュニティ、公式の参照が含まれます。 |
| W2-P3 | Asegurar aprobacion de gobernanza para el plan de intake (再会への投票 + 登録行為) |ガバナンス連絡窓口 | 2025-05-22 |コンプリート | 2025-05-20 に投票してください。 `DOCS-SORA-Preview-W2` で点呼を行います。 |
| W2-P4 |プログラマ ステージング デル プロキシ Try it + ベンタナ W2 でのテレメトリア キャプチャ (`preview-2025-06-15`) |ドキュメント/DevRel + オペレーション | 2025-06-05 |コンプリート | Ticket de cambio `OPS-TRYIT-188` 2025-06-09 02:00-04:00 UTC; Grafana アーカイブ チケットのスクリーンショット。 |
| W2-P5 |プレビュー (`preview-2025-06-15`) のアーティファクトの新しいタグの構築/検証、アーカイブ記述子/チェックサム/プローブ ログ |ポータルTL | 2025-06-07 |コンプリート | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` se ejecuto 2025-06-10; Guardados bajo `artifacts/docs_preview/W2/preview-2025-06-15/` を出力します。 |
| W2-P6 | Armar の招待者コミュニティ (<=25 人の査読者、多数のエスカロナド) の連絡先情報を確認してください。コミュニティマネージャー | 2025-06-10 |コンプリート | 8 人の査読者共同体による入門書。トラッカーの登録 ID `DOCS-SORA-Preview-REQ-C01...C08`。 |

## 証拠のチェックリスト

- [x] `DOCS-SORA-Preview-W2` に付属する Gobernanza 登録 (再会の記録 + 投票へのリンク)。
- [x] 現実的なコミットメントのテンプレート `docs/examples/`。
- [x] 記述子 `preview-2025-06-15`、チェックサムのログ、プローブ出力、リンク レポート、トランスクリプト、プロキシを保護してみてください。`artifacts/docs_preview/W2/`。
- [x] Grafana (`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`) のスクリーンショットは、プリフライト W2 のキャプチャーです。
- [x] レビュアーの招待状の名簿、環境設定のチケットの要請とタイムスタンプの表 (W2 デル トラッカー版)。

管理エステ計画の実現。トラッカーは、ロードマップ DOCS-SORA を参照して、W2 のサルガン ラス インビタシオネスを正確に参照します。