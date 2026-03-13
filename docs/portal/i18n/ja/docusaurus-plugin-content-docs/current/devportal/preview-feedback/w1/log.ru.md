---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: Лог отзывов и телеметрии W1
サイドバーラベル: Лог W1
説明: Сводный 名簿、телеметрические チェックポイント、заметки 査読者、プレビュー - волны партнеров。
---

Этот лог хранит 名簿、телеметрические チェックポイント、отзывы для
**W1 のプレビュー**、最新のバージョンを確認
[`preview-feedback/w1/plan.md`](./plan.md) と表示されます。
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。 Обновляйте его、когда отправлено приглазение、
スナップショットとトリアージ、ガバナンスレビュー担当者による評価
あなたのことを忘れないでください。

## Рoster когорты

|パートナー ID | Тикет запроса | NDA を取得 | Приглавление отправлено (UTC) | Ack/первый логин (UTC) | Статус | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Завербено 2025-04-26 | sorafs-op-01;オーケストレーター ドキュメントのパリティを確認してください。 |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Завербено 2025-04-26 | sorafs-op-02;クロスリンク Norito/テレメトリ。 |
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Завербено 2025-04-26 | sorafs-op-03;マルチソースフェイルオーバー訓練です。 |
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Завербено 2025-04-26 |鳥居-int-01; ревью 料理本 Torii `/v2/pipeline` + 試してみましょう。 |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Завербено 2025-04-26 |鳥居-int-02;試してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Завербено 2025-04-26 | SDK-パートナー-01;フィードバック、クックブック JS/Swift + サニティ チェック、ISO ブリッジ。 |
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Завербено 2025-04-26 | SDK-パートナー-02;コンプライアンス закрыт 2025-04-11、фокус на заметках 接続/テレメトリ。 |
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Завербено 2025-04-26 |ゲートウェイ-ops-01; аудит ops гайда ゲートウェイ + анонимизированный поток プロキシを試してください。 |

Заполните **Приглазение отправлено** и **Ack** сразу после отправки письма.
Привяжите время к UTC расписанию、заданному в плане W1.

## チェックポイントをチェックする| Время (UTC) |ダッシュボード/プローブ | Владелец | Результат | Артефакт |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション | ✅ Все зеленое | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Транскрипт `npm run manage:tryit-proxy -- --stage preview-w1` |作戦 | ✅ Подготовлено | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | + `probe:portal` | ログインしてください。ドキュメント/DevRel + オペレーション | ✅ 招待前のスナップショット。 `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |プロキシを試してみてください。ドキュメント/DevRel リード | ✅ 中間点チェック (0 алертов; латентность お試しください p95=410 ミリ秒) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |プローブ + 終了プローブ |ドキュメント/DevRel + ガバナンス連携 | ✅ スナップショットを終了します。 `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Ежедневные выборки オフィスアワー (2025-04-13 -> 2025-04-25) упакованы как NDJSON + PNG экспорты под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` с именами файлов
`docs-preview-integrity-<date>.json` と соответствующими скринсотами。

## 問題に関する問題

レビューアを評価してください。 Ссылайтесь на каждый элемент на GitHub/discuss
тикет и на структурированную форму, заполненную через
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|参考資料 |重大度 |オーナー |ステータス |メモ |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 | ✅ 2025 年 4 月 18 日に解決済み | Уточнены формулировка навигации 試してみてください + якорь サイドバー (`docs/source/sorafs/tryit.md` обновлен новым ラベル)。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 | ✅ 2025 年 4 月 19 日に解決済み |試してみてください。アーティファクト `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| - |情報 |ドキュメント/DevRel リード | 🟢 定休日 | Остальные комментарии были только Q&A; зафиксированы в форме каждого партнера под `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## 知識チェックとアンケート

1. クイズ (クイズ >=90%) для каждого 評論家。 CSV 形式で表示されます。
2. アンケート調査、フィードバック、フィードバックの送信
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`。
3. 修復を完了するには、次の手順を実行します。

レビューアの評価 >=94% в 知識チェック (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。修復 звонки не потребовались;
輸出調査結果 каждого партнера находятся под
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## Инвентаризация артефактов

- バンドル プレビュー記述子/チェックサム: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- サマリープローブ + リンクチェック: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシを試してください: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- テレメトリのエクスポート: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 毎日のオフィスアワーテレメトリーバンドル: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- フィードバック + アンケートのエクスポート: レビュー担当者ごとにレポートが作成されます。
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- 知識チェック CSV および概要: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

трекера を発行してください。ガバナンスを強化する
シェルを破壊する必要があります。