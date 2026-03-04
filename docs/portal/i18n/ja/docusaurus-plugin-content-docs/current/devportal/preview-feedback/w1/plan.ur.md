---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: W1 شراکت داروں کے لئے پری فلائٹ پلان
サイドバーラベル: W1 پلان
説明: پارٹنر プレビュー کوہوٹ کے لئے ٹاسکس، مالکان، اور ثبوت چیک لسٹ۔
---

| और देखें評価 |
| --- | --- |
|ああ | W1 - Torii インテグレータ |
| فدف ونڈو | 2025 年第 2 四半期 3 |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

## 大事

1. プレビューを確認する 確認する 確認する
2. プロキシを試してみる テレメトリ スナップショットを試す
3. チェックサムを検証し、アーティファクトをプレビューし、プローブを検査します。
4. チームのメンバー チームメンバーの名簿 リクエスト テンプレート チームのメンバー

## ٹاسک بریک ڈاؤن

| ID |ありがとう |意味 | और देखें और देखेंにゅう |
| --- | --- | --- | --- | --- | --- |
| W1-P1 |プレビュー規約補遺 قانونی منظوری حاصل کرنا |ドキュメント/DevRel リード -> 法務 | 2025-04-05 | ✅ और देखें قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 کو منظور ہوا؛ PDF ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | Try it プロキシ ステージング ونڈو (2025-04-10) محفوظ کرنا اور proxy health کی تصدیق |ドキュメント/DevRel + オペレーション | 2025-04-06 | ✅ और देखें `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 ٩و چلایا گیا؛ CLI トランスクリプト اور `.env.tryit-proxy.bak` محفوظ کر دیے گئے۔ |
| W1-P3 |プレビュー アーティファクト (`preview-2025-04-12`) `scripts/preview_verify.sh` + `npm run probe:portal` 記述子/チェックサムポータルTL | 2025-04-08 | ✅ और देखेंアーティファクト اور 検証ログ `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ ہیں؛プローブ出力の測定結果|
| W1-P4 |入力フォーム (`DOCS-SORA-Preview-REQ-P01...P08`) 連絡先 NDA を入力してください。ガバナンス連絡窓口 | 2025-04-07 | ✅ और देखें تمام آٹھ درخواستیں منظور ہوئیں (آخری دو 2025-04-11 کو منظور ہوئیں)؛承認 ٹریکر میں لنک ہیں۔ |
| W1-P5 | دعوتی متن تیار کرنا (`docs/examples/docs_preview_invite_template.md` پر مبنی)، ہر پارٹنر کے لئے `<preview_tag>` اور `<request_ticket>` سیٹうみん |ドキュメント/DevRel リード | 2025-04-08 | ✅ और देखें دعوت کا مسودہ 2025-04-12 15:00 UTC کو アーティファクト لنکس کے ساتھ بھیجا گیا۔ |

## プリフライト

> ٹِپ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` چلائیں تاکہ مراحل 1-5 خودکار طور پر چل جائیں (ビルド、チェックサム検証、ポータル プローブ、リンク チェッカー、プロキシ更新を試行)。 اس اسکرپٹ میں JSON لاگ بنتا ہے جسے ٹریکر ایشو کے ساتھ منسلک کیا جا سکتا ہے۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` ساتھ) または `build/checksums.sha256` اور `build/release.json` دوبارہ بنیں۔
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` `build/link-report.json` 記述子 アーカイブ アーカイブ
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ターゲット `--tryit-target` سے دیں) `.env.tryit-proxy` コミット ロールバック ロールバック `.bak` ロールバック
6. W1 ログ パス (記述子チェックサム、プローブ出力、試用プロキシ、Grafana スナップショット)۔

## और देखें

- [x] دستخط شدہ قانونی منظوری (PDF یا ٹکٹ لنک) `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہے۔
- [x] `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` Grafana のスクリーンショット
- [x] `preview-2025-04-12` 記述子チェックサム ログ `artifacts/docs_preview/W1/` کے تحت محفوظ ہیں۔
- [x] دعوت 名簿テーブル میں `invite_sent_at` タイムスタンプ مکمل ہیں (ٹریکر W1 ログ دیکھیں)۔
- [x] フィードバック アーティファクト [`preview-feedback/w1/log.md`](./log.md) میں نظر آتے ہیں، ہر پارٹنر کے لئے ایک row (2025-04-26 کو)名簿/テレメトリア/問題 ڈیٹا کے ساتھ اپ ڈیٹ)۔

جوں جوں کام آگے بڑھے یہ پلان اپ ڈیٹ کریں؛ロードマップの監査可能性の監査の可能性の評価

## فیڈبیک ورک فلو

1. レビュアー ٩ے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) テンプレート کاپی کریں،
   میٹا ڈیٹا بھریں اور مکمل کاپی
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` میں رکھیں۔
2. 招待状、テレメトリ チェックポイント、未解決の問題
   [`preview-feedback/w1/log.md`](./log.md) ライブログ میں خلاصہ کریں تاکہ ガバナンス審査員 پوری لہر کو
   リポジトリ、リプレイ、リポジトリ
3. 知識チェック、調査エクスポート、ログ記録、アーティファクト パス、添付ファイル
   トラッカーの問題に関するリンク