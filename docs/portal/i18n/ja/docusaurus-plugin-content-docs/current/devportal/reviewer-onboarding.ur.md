---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#پریویو ریویور آن بورڈنگ

## やあ

DOCS-SORA 問題を解決する 問題を解決するチェックサムゲートされたビルド
(`npm run serve`) 試してみてください فلو اگلا سنگ میل کھولتے ہیں: پبلک پریویو کے وسیع پیمانے پر
٩ھلنے سے پہلے ویری فائیڈ ریویورز کی آن بورڈنگ۔ یہ گائیڈ بیان کرتا ہے کہ درخواستیں کیسے جمع کی جائیں،
और देखें آف بورڈ کیسے کیا جائے۔
生産性の向上 生産性の向上 輸出の生産性
[プレビュー招待フロー](./preview-invite-flow.md) دیکھیں؛ فیچے کے مراحل اس پر فوکس کرتے ہیں
کہ ریویور منتخب ہونے کے بعد کون سی کاروائیاں کرنی ہیں۔

- **回答:** GA ドキュメント プレビュー (`docs-preview.sora`、GitHub Pages ビルド、SoraFS バンドル) の評価درکار ہے۔
- **آؤٹ آف اسکوپ:** Torii یا SoraFS آپریٹرز (اپنے オンボーディング キット میں کور) ال展開 (دیکھیں [`devportal/deploy-guide`](./deploy-guide.md))۔

## پری ریکویزٹس

| | | और देखेंアーティファクト |にゅう |
| --- | --- | --- | --- |
|コアメンテナー |煙テスト| GitHub ハンドル、Matrix 連絡先、CLA 署名済み| عموما پہلے سے `docs-preview` GitHub ٹیم میں ہوتا ہے؛ پھر بھی درخواست درج کریں تاکہ رسائی 監査可能 رہے۔ |
|パートナーレビューアー | SDK スニペット ガバナンス コンテンツ فائی کرنا۔ |企業メール、法的 POC、署名されたプレビュー規約| ٹیلی میٹری + データ処理要件 کی منظوری لازم ہے۔ |
|地域ボランティア |ユーザビリティに関するフィードバック| GitHub ハンドル、優先連絡先、タイムゾーン、CoC など| ٩وہورٹس چھوٹے رکھیں؛貢献者契約 سائن کرنے والے ریویورز کو ترجیح دیں۔ |

回答:

1. アーティファクトのプレビューと使用許可の確認
2. セキュリティ/可観測性の付録
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
3. スナップショット サーブ سے پہلے `docs/portal/scripts/preview_verify.sh` چلانے پر رضامند ہوں۔

## 摂取量

1. और देखें
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   فارم بھرنے کو کہیں (یا issue میں コピー/ペースト کریں)۔ کم از کم یہ ریکارڈ کریں: شناخت، رابطے کا طریقہ،
   GitHub ハンドルのレビュー日付とセキュリティ ドキュメントの日付
2. `docs-preview` トラッカー (GitHub 発行ガバナンス チケット) 承認者割り当て
3. پری ریکویزٹس چیک کریں:
   - CLA / 貢献者契約 فائل پر موجود ہو (パートナー契約参照)۔
   - 使用許諾の承認 درخواست میں محفوظ ہو۔
   - リスク評価 (リスク評価: パートナーのレビュー担当者が法的承認を承認)۔
4. 承認者 サインオフ 問題の追跡 変更管理エントリ 承認
   (意味: `DOCS-SORA-Preview-####`)۔

## プロビジョニング

1. **アーティファクトの説明** — CI ワークフロー SoraFS ピンの作成とプレビュー記述子 + アーカイブの作成
   (`docs-portal-preview` アーティファクト)۔アカウント:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサムの強制機能** — チェックサムゲートの機能:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   یہ `scripts/serve-verified-preview.mjs` کو 再利用 کرتا ہے تاکہ کوئی 未検証のビルド غلطی سے لانچ نہ ہو۔

3. **GitHub رسائی دیں (اختیاری)** — اگر ریویورز کو 未公開ブランチ چاہییں تو انہیں review مدت کے لئے
   `docs-preview` GitHub ٹیم میں شامل کریں اور メンバーシップ تبدیلی درخواست میں درج کریں۔

4. **サポート チャネル شیئر کریں** — オンコール連絡先 (Matrix/Slack) およびインシデント手順
   [`incident-runbooks`](./incident-runbooks.md) سے شیئر کریں۔

5. **テレメトリ + フィードバック** — 数字と数字、匿名化された分析、数字、数字
   (دیکھیں [`observability`](./observability.md))۔ دعوت میں دیے گئے フィードバック フォーム یا 問題テンプレート فراہم کریں
   イベント کو [`preview-feedback-log`](./preview-feedback-log) ヘルパー سے لاگ کریں تاکہ wave summary اپ ٹو ڈیٹ رہے۔

## سٹریویور چیک لسٹ

پریویو تک رسائی سے پہلے ریویورز کو یہ مکمل کرنا ہوگا:

1. ダウンロードしたアーティファクトを確認します (`preview_verify.sh`)
2. `npm run serve` (`serve:verified`) チェックサム ガード チェックサム ガード
3. セキュリティと可観測性の確保
4. OAuth/Try it コンソール、デバイス コード ログイン、デバイス コード ログイン、デバイス コード ログイン、デバイス コード ログイン、デバイス コード ログイン、プロダクション トークン、プロダクション トークン
5. 合意されたトラッカーの調査結果 (問題の共有ドキュメント)、プレビュー リリース タグ、タグ、およびタグ

## メンテナー داریاں اور オフボーディング| |ありがとう |
| --- | --- |
|キックオフ |摂取チェックリストの摂取チェックリストの摂取チェックリストの摂取量のチェックリストの摂取量のチェックリストの摂取量のチェックリストの摂取量のチェックリストの摂取量の確認[`preview-feedback-log`](./preview-feedback-log) ذریعے `invite-sent` انٹری شامل کریں، اور اگر review ایک ہفتے سے中間点同期 ٩ریں۔ |
|モニタリング |プレビュー テレメトリ (テスト テスト、プローブの失敗) テスト、インシデント ランブック、テスト調査結果 `feedback-submitted`/`issue-opened` イベント 波形メトリクス درست رہیں۔ |
|オフボーディング | GitHub یا SoraFS رسائی واپس لیں، `access-revoked` درج کریں، درخواست アーカイブ کریں (フィードバックの概要 + 未解決のアクション) کریں)، اور reviewer registry اپ ڈیٹ کریں۔ سیویور مقامی builds صاف کرنے کو کہیں اور [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) سے بنا ダイジェスト منسلک کریں۔ |

波 کے درمیان 回転 کرتے وقت بھی یہی عمل استعمال کریں۔リポジトリの紙の軌跡
(問題 + テンプレート) DOCS-SORA 監査可能 ガバナンス 管理 管理 管理
プレビュー アクセス 文書化されたコントロール

## 招待テンプレートと追跡

- アウトリーチ活動
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  فائل سے کریں۔チェックサム プレビュー チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム
  使用可能 پالیسی تسلیم کریں۔
- テンプレート `<preview_tag>`、`<request_ticket>` 連絡先チャネル プレースホルダー بدلیں۔
  最終メッセージ ایک کاپی 受験チケット میں محفوظ کریں تاکہ ریویورز، 承認者 اور 監査人
  بھیجے گئے الفاظ کا حوالہ دے سکیں۔
- 追跡スプレッドシートの発行日 `invite_sent_at` タイムスタンプ 終了日の確認
  [プレビュー招待フロー](./preview-invite-flow.md) پورٹ خودکار طور پر cohort اٹھا سکے۔ [プレビュー招待フロー](./preview-invite-flow.md)