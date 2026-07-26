---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پبلک پریویو دعوتی پلے بک

## پروگرام کے مقاصد

یہ پلے بک وضاحت کرتی ہے کہ ریویور آن بورڈنگ ورک فلو فعال ہونے کے بعد پبلک پریویو کیسے اعلان اور چلایا جائے۔
یہ DOCS-SORA روڈمیپ کو دیانت دار رکھتی ہے کیونکہ ہر دعوت کے ساتھ قابل تصدیقアーティファクト
フィードバック 評価 評価 評価 評価 評価 評価 評価

- **コメント:** 管理者が厳選したもの、プレビュー、使用可能なものを確認する❁❁❁❁
- **デフォルト:** デフォルトのウェーブ サイズ <= 25 14 時間のアクセス ウィンドウ 24 時間のインシデント対応

## और देखें

٩وئی بھی دعوت بھیجنے سے پہلے یہ کام مکمل کریں:

1. プレビュー アーティファクト CI میں اپلوڈ ہوں (`docs-portal-preview`,
   チェックサム マニフェスト、記述子、SoraFS バンドル)۔
2. `npm run --prefix docs/portal serve` (チェックサムゲート) اسی タグ پر ٹیسٹ کیا گیا ہو۔
3. آن بورڈنگ ٹکٹس 承認 ہوں اور 招待ウェーブ سے لنک ہوں۔
4. 可観測性と事件の検証
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))۔
5. フィードバックの問題のテンプレート (重大度、再現手順、スクリーンショット、環境情報) (重大度、再現手順、スクリーンショット、環境情報)
6. ドキュメント/DevRel + ガバナンスに関する説明

## دعوتی پیکیج

دعوت میں شامل ہونا چاہیے:

1. **検証済みアーティファクト** — SoraFS マニフェスト/計画 GitHub アーティファクト
   チェックサム マニフェスト ディスクリプタ チェックサム マニフェスト ディスクリプタ検証 ٩مانڈ واضح طور پر لکھیں تاکہ
   ریویورز サイト لانچ کرنے سے پہلے اسے چلا سکیں۔
2. **提供手順** — チェックサム ゲート プレビュー:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **セキュリティ リマインダー** — トークンの有効期限 خود بخود 有効期限 ہوتے ہیں، لنکس شیئر نہیں کیے جائیں،
   事件、事件、事故、事件
4. **フィードバック チャネル** — 問題のテンプレート/フォーム 応答時間の予想 واضح کریں۔
5. **プログラムの日付** — 開始日/終了日、オフィスアワー、同期、更新ウィンドウ、

और देखें
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
میں دستیاب ہے اور یہ 要件 پوری کرتا ہے۔プレースホルダー (日付、URL、連絡先)
और देखें

## پریویو ホストが暴露します

オンボーディングの確認 チケットの変更 プレビュー ホストのプロモーション 登録の確認
エンドツーエンドの手順を構築/公開/検証する
[プレビュー ホスト露出ガイド](./preview-host-exposure.md) دیکھیں۔

1. **ビルドのビルド:** タグ スタンプのリリース、確定的なアーティファクトの作成

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   ピンスクリプト `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、
   اور `portal.dns-cutover.json` کو `artifacts/sorafs/` میں لکھتا ہے۔波を誘う
   کے ساتھ 添付 کریں تاکہ ہر ریویور وہی ビット検証 کر سکے۔

2. **プレビュー エイリアス発行:** کمانڈ کو `--skip-submit` کے بغیر دوبارہ چلائیں
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` ガバナンス発行のエイリアス証明 فراہم کریں)۔
   説明 `docs-preview.sora` マニフェスト バインド 説明 証拠バンドル 説明
   `portal.manifest.submit.summary.json` 日 `portal.pin.report.json` 日

3. **デプロイメント プローブ:** 招待 بھیجنے سے پہلے エイリアス解決 チェックサム タグ マッチ マッチ
   یقینی بنائیں۔

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) フォールバック機能、便利な機能、機能
   プレビュー エッジを確認してください。

## کمیونیکیشن ٹائم لائن

| और देखें認証済み |オーナー |
| --- | --- | --- |
| D-3 |ファイナライズ アーティファクトの更新 検証 ドライラン |ドキュメント/開発リリース |
| D-2 |ガバナンスの承認 + チケットの変更 |ドキュメント/DevRel + ガバナンス |
| D-1 |テンプレート کے ذریعے دعوتیں بھیجیں، トラッカー میں 受信者リスト اپ ڈیٹ کریں |ドキュメント/開発リリース |
| D |キックオフ コール / オフィス アワー、テレメトリ ダッシュボードドキュメント/DevRel + オンコール |
| D+7 |中間点のフィードバック ダイジェスト、ブロックする問題、トリアージ |ドキュメント/開発リリース |
| D+14 |ウェーブ بند کریں، عارضی رسائی 取り消し کریں، `status.md` میں خلاصہ شائع کریں |ドキュメント/開発リリース |

## アクセス追跡テレメトリ

1. 受信者、招待タイムスタンプ、失効日、プレビュー フィードバック ロガー、データ
   (دیکھیں [`preview-feedback-log`](./preview-feedback-log)) تاکہ ہر wave ایک ہی 証拠追跡 شیئر کرے:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   サポートされているイベント `invite-sent`、`acknowledged`、`feedback-submitted`、
   `issue-opened`、`access-revoked`۔ログ ڈیفالٹ طور پر
   `artifacts/docs_portal_preview/feedback_log.json` میں موجود ہے؛ウェーブを招待します ٹکٹ کے ساتھ
   同意書を添付するクローズアウト پہلے サマリー ヘルパー استعمال کریں تاکہ
   監査可能なロールアップの日付:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```概要 JSON ウェーブの招待状、受信者、フィードバック数、イベントの数
   タイムスタンプを列挙する ہے۔ヘルパー
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   پر مبنی ہے، اس لئے وہی ワークフロー لوکل یا CI میں چل سکتا ہے۔要約する
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ダイジェスト テンプレート
2. テレメトリー ダッシュボード ウェーブ マーク فونے والے `DOCS_RELEASE_TAG` マーク タグ マーク マーク
   スパイク、コホートの招待、相関関係、分析、分析、分析、分析、分析、分析、分析、分析
3. `npm run probe:portal -- --expect-release=<tag>` プレビュー環境をデプロイします
   メタデータをリリースして宣伝する
4. インシデント、ランブック テンプレート、キャプチャ、コホート、リンク

## フィードバックと終了

1. フィードバック 共有ドキュメント 問題ボード میں جمع کریں۔アイテム `docs-preview/<wave>` タグ ٩ریں تاکہ
   ロードマップの所有者 انہیں آسانی سے クエリ کر سکیں۔
2. プレビュー ロガー 要約出力 波形レポート 要約 コホート `status.md` 要約 要約
   (参加者、調査結果、計画された修正) DOCS-SORA マイルストーン بدلا ہو تو `roadmap.md` اپ ڈیٹ کریں۔
3. [`reviewer-onboarding`](./reviewer-onboarding.md) オフボード手順は次のとおりです: アクセスの取り消し
   リクエスト アーカイブ 参加者 شکریہ ادا کریں۔
4. ウェーブ、アーティファクトの更新、チェックサム ゲート、招待テンプレート、日付、日付

プレイブックを確認する プレビューを確認する 監査可能 ドキュメント/DevRel を確認する
繰り返し可能 طریقہ ملتا ہے جیسے جیسے پورٹل GA کے قریب آتا ہے۔