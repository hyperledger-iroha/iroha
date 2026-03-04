---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プレビュー-フィードバック-w1-log
タイトル: ニュース W1
サイドバーラベル: W1
説明: قائمة مجمعة، نقاط قياس، وملاحظات المراجعين لموجة معاينة الشركاء الاولى。
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة **معاينة الشركاء W1**
المرافقة لمهام القبول في [`preview-feedback/w1/plan.md`](./plan.md) ومدخل متتبع الموجة في
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。 حدثه عند ارسال دعوة،
القسجيل لقطة قياس، او تصنيف بند ملاحظات حتى يتمكن مراجعو الحوكمة من اعادة تشغيل
重要な問題は、次のとおりです。

## قائمة الدفعة

| और देखेंニュース | ニュースNDA | 秘密保持契約を締結する世界時間 (UTC) |世界/世界 (UTC) |ああ |重要 |
| --- | --- | --- | --- | --- | --- | --- |
|パートナー-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مكتمل 2025-04-26 | sorafs-op-01;オーケストレーター。 |
|パートナー-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مكتمل 2025-04-26 | sorafs-op-02; Norito/テレメトリ。 |
|パートナー-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مكتمل 2025-04-26 | sorafs-op-03;フェイルオーバーが発生しました。 |
|パートナー-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مكتمل 2025-04-26 |鳥居-int-01; مراجعة دليل Torii `/v1/pipeline` و 料理本 試してみてください。 |
|パートナー-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مكتمل 2025-04-26 |鳥居-int-02;試してみてください (docs-preview/w1 #2)。 |
|パートナー-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مكتمل 2025-04-26 | SDK-パートナー-01;重要なクックブックは JS/Swift + 健全性 ISO です。 |
|パートナー-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مكتمل 2025-04-26 | SDK-パートナー-02;接続/テレメトリ。 |
|パートナー-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مكتمل 2025-04-26 |ゲートウェイ-ops-01;ゲートウェイ + セキュリティ プロキシ 試してみてください。 |

ログインしてください。** ログインしてください。
UTC 時間 W1。

## いいえ

|世界時間 (UTC) |プローブ / プローブ |ああ |認証済み |認証済み |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |ドキュメント/DevRel + オペレーション | ✅ كلها خضراء | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` |作戦 | ✅ いいえ | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | और देखेंドキュメント/DevRel + オペレーション | ✅ قطة قبل الدعوة، بلا تراجعات | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |試してみる |ドキュメント/DevRel リード | ✅ اجتاز فحص منتصف الموجة (0 تنبيهات; زمن Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |ニュース + プローブ ニュース |ドキュメント/DevRel + ガバナンス連携 | ✅ قطة خروج، صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |عينات ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) مجمعة كصادرات NDJSON + PNG تحت
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` ログイン
`docs-preview-integrity-<date>.json` です。

## और देखें

あなたのことを忘れないでください。 GitHub/ディスカッションする
ニュース ニュース ニュース ニュース ニュース
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|ああ |ああ |ああ |ああ |重要 |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` |低い |ドキュメント-コア-02 | ✅ 2025-04-18 | 2025 年 4 月 18 日試してみる + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` بالوسم الجديد)。 |
| `docs-preview/w1 #2` |低い |ドキュメントコア-03 | ✅ 2025-04-19 | 2025 年 4 月 19 日試してみる + 試してみる + 試してみる + 試してみる`artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| — |情報 |ドキュメント/DevRel リード | 🟢 और देखें كانت التعليقات المتبقية اسئلة/اجابات فقط؛ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` を参照してください。 |

## عرض المزيد المزيد المزيد

1. 評価 (評価 >=90%) 評価CSV 形式のファイル。
2. ログイン アカウント登録 ログイン アカウントを作成する
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`。
3. جدولة مكالمات المعالجة لمن يقل عن الحد وادونها في هذا الملف.

率 % % % % % (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`)。 ã‚¹ã‚¤ã‚¹ã‚¿
重要な情報最高のパフォーマンスを見せてください。
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## いいえ

- プレビュー記述子/チェックサム: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- プローブ + リンクチェック: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- プロキシ 試してみてください: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- 番号: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- メッセージ: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- ログイン: ログイン: ログイン: ログイン
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV 番号: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

حافظ على الجرد متزامنا مع تذكرة المتتبع. عند نسخ الاثار الى تذكرة الحوكمة
最高のパフォーマンスを見せてください。