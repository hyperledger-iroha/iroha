---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#پریویو دعوتی فلو

## 大事

**DOCS-SORA** を使用してください。 آخری رکاوٹیں قرار دیتا ہے جن کے بعد پورٹل بیٹا سے باہر جا سکتا ہے۔ یہ صفحہ بیان کرتا ہے کہ ہر دعوتی ویو کیسے کھولی جائے، کون سے アーティファクト دعوتیں監査可能性 監査可能性 監査可能性 監査可能性 監査可能性回答:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) ہر ریویور کی ہینڈلنگ کے لئے۔
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサム
- [`devportal/observability`](./observability.md) ٹیلی میٹری アラート フックをエクスポートします。

## ویو پلان

| |認証済み | और देखें और देखेंにゅう |
| --- | --- | --- | --- | --- |
| **W0 - コアメンテナー** |ドキュメント/SDK メンテナは、初日に検証を行う必要があります。 | GitHub ٹیم `docs-portal-preview` آباد ہو، `npm run serve` チェックサム ゲート سبز ہو، Alertmanager 7 دن خاموش رہے۔ | P0 ドキュメントのバックログ ٹیگ شدہ، کوئی ブロッキング インシデント نہ ہو۔ |検証するアーティファクトのプレビューを確認する|
| **W1 - パートナー** | SoraFS 管理者 Torii インテグレーター、NDA 管理者、ガバナンス審査担当者| W0 のテスト プロキシ ステージングの試行|サインオフ (サインオフの発行) チェック チェック 同時レビュー者数 10 人未満 14 人 セキュリティの後退 チェック|招待状テンプレート + チケットのリクエスト|
| **W2 - コミュニティ** | ٩میونٹی ویٹ لسٹ سے منتخب contributors۔ | W1 のインシデント訓練のリハーサルが行われ、公開 FAQ が公開されました| 2 つ以上のドキュメント リリース プレビュー パイプライン ロールバック ロールバック ہوں۔ |同時招待数 محدود (<=25) اور ہفتہ وار بیچ۔ |

`status.md` プレビュー リクエスト トラッカーの管理 ガバナンス 管理 管理 管理 管理

## プリフライトチェックリスト

और देखें

1. **CI アーティファクト**
   - 評価 `docs-portal-preview` + 記述子 `.github/workflows/docs-portal-preview.yml` ذریعے اپ لوڈ ہو۔
   - SoraFS ピン `docs/portal/docs/devportal/deploy-guide.md` میں نوٹ ہو (カットオーバー記述子 موجود ہو)。
2. **チェックサムの強制**
   - `docs/portal/scripts/serve-verified-preview.mjs` `npm run serve` ذریعے を呼び出します ہو۔
   - `scripts/preview_verify.sh` macOS + Linux پر ٹیسٹ ہوں۔
3. **テレメトリーベースライン**
   - `dashboards/grafana/docs_portal.json` 試してみてください ٹریفک دکھائے اور `docs.preview.integrity` الرٹ سبز ہو۔
   - `docs/portal/docs/devportal/observability.md` کا تازہ 付録 Grafana لنکس کے ساتھ اپ ڈیٹ ہو۔
4. **ガバナンス成果物**
   - トラッカー問題を招待します (ہر ویو کے لئے ایک 問題)。 
   - レビュー担当者レジストリ テンプレート کاپی ہو (دیکھیں [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - SRE 承認問題の解決

دعوت بھیجنے سے پہلے トラッカーを招待 میں プリフライト مکمل ہونے کا اندراج کریں۔

## فلو کے مراحل

1. **امیدوار منتخب کریں**
   - ویٹ لسٹ شیٹ یا پارٹنر کیو سے نکالیں۔
   - リクエスト テンプレート ہونا یقینی بنائیں۔
2. **重要な要素**
   - トラッカー問題の承認者を招待します
   - 前提条件 (CLA/契約、許容される用途、セキュリティ概要)。
3. ** और देखें
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) プレースホルダー (`<preview_tag>`、`<request_ticket>`、連絡先)
   - 記述子 + アーカイブ ハッシュ「Try it ステージング URL」サポート チャネル
   - فائنل ای میل (یا Matrix/Slack トランスクリプト) 発行 میں محفوظ کریں۔
4. **オンボーディング ٹریک کریں**
   - トラッカーの招待 `invite_sent_at`、`expected_exit_at`、ステータス (`pending`、`active`、`complete`、`revoked`) और देखें
   - 監査可能性 レビューアの採用リクエスト
5. **テレメトリ機能**
   - `docs.preview.session_active` アラート `TryItProxyErrors` アラート アラート
   - ベースライン سے ہٹے تو 事件 کھولیں اور نتیجہ 招待エントリー کے ساتھ نوٹ کریں۔
6. **فیڈبیک جمع کریں اور خارج ہوں**
   - فیڈبیک آنے پر یا `expected_exit_at` گزرنے پر دعوتیں بند کریں۔
   - コホート (調査結果、インシデント、次のアクション) の問題 (調査結果、インシデント、次のアクション)

## 証拠と報告

|アーティファクト | ٩ہاں محفوظ کریں |ケイデンス |
| --- | --- | --- |
|招待トラッカーの問題 | GitHub پروجیکٹ `docs-portal-preview` | ہر دعوت کے بعد اپ ڈیٹ کریں۔ |
|査読者名簿のエクスポート | `docs/portal/docs/devportal/reviewer-onboarding.md` リンクされたレジストリ | ففتہ وار۔ |
|テレメトリースナップショット | `docs/source/sdk/android/readiness/dashboards/<date>/` (テレメトリ バンドルの再利用) | ہر ویو + 事件 کے بعد۔ |
|フィードバックダイジェスト | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ہر ویو کیلئے فولڈر بنائیں) | ویو exit کے 5 دن کے اندر۔ |
|ガバナンス会議メモ | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | ہر DOCS-SORA ガバナンス同期 سے پہلے بھریں۔ |ہر بیچ کے بعد `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
ビデオ ダイジェスト ビデオJSON の問題を解決する ガバナンスレビュー担当者を確認するعداد کی تصدیق کر سکیں۔

`status.md` کے ساتھ منسلک کریں تاکہ روڈ میپ انٹری جلدی پ ڈیٹ ہو سکے۔

## ロールバック、一時停止

管理: 管理: 管理:

- プロキシ インシデントのロールバックを試してみてください (`npm run manage:tryit-proxy`)。
- アラート疲労: プレビュー専用エンドポイント 7 つ、アラート ページ 3 つ以上。
- コンプライアンスのギャップ: 署名済みの条件、リクエスト テンプレート、および要求テンプレート
- 整合性リスク: `scripts/preview_verify.sh` チェックサムの不一致 پکڑا گیا۔

トラッカーを招待して修復を依頼してくださいتصدیق کے بعد ہی دوبارہ شروع کریں۔