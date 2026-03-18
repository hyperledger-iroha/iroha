---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::note メモ
یہ صفحہ `docs/source/sns/address_display_guidelines.md` کی عکاسی کرتا ہے اور اب
پورٹل کی کینونیکل کاپی ہے۔広報活動 広報活動 広報活動 広報活動 広報活動 広報活動 広報活動
:::

SDK のセキュリティ SDK のセキュリティ ペイロードのセキュリティ
Android の開発
`examples/android/retail-wallet` میں مطلوبہ UX پیٹرن دکھایا گیا ہے:

- **دو کاپی اہداف۔** دو واضح کاپی بٹنز دیں: I105 (ترجیحی) اور صرف Sora والا
  ٩مپریسڈ فارم (`sora...`، 2 番目に良い)。 I105 ہمیشہ بیرونی شیئرنگ کے لئے محفوظ ہے اور QR
  ペイロード بناتا ہے۔ کمپریسڈ فارم میں インライン وارننگ لازمی ہے کیونکہ یہ صرف
  ソラ意識 ایپس میں کام کرتا ہے۔ Android のマテリアルの開発
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` میں
  iOS SwiftUI と `examples/ios/NoritoDemo/Sources/ContentView.swift`
  ٩ے اندر `AddressPreviewCard` کے ذریعے یہی UX دہراتا ہے۔
- **等幅文字列 等幅文字列
  `textIsSelectable="true"` کے ساتھ رینڈر کریں تاکہ صارفین IME کے بغیر اقدار
  और देखें قابلِ ترمیم فیلڈز سے بچیں: IME kana کو بدل سکتا ہے یا ゼロ幅
  پوائنٹس داخل کر سکتا ہے۔
- ** غیر ضمنی ڈیفالٹ ڈومین کے اشارے۔** جب selector ضمنی `default` ڈومین کی طرف
  ہو تو ایک キャプション دکھائیں جو آپریٹرز کو یاد دلائے کہ 接尾辞 درکار نہیں۔
  正規の ڈومین لیبل ہائی لائٹ کرنا چاہیے جب セレクター
  ダイジェストエンコード
- **I105 QR ペイロード** QR ٩وڈز کو I105 文字列エンコード چاہیے۔ QR
  世代 ناکام ہو تو خالی تصویر کے بجائے واضح エラー دکھائیں۔
- ** ٩لپ بورڈ پیغام۔** کمپریسڈ فارم کاپی کرنے کے بعد トースト یا スナックバー دکھائیں
  جو صارفین کو یاد دلائے کہ یہ صرف Sora ہے اور IME سے خراب ہو سکتا ہے۔

Unicode/IME の破損、UX の破損、および IME の破損。
ADDR-6 ロードマップの承認が完了しました

## और देखें

ログインしてください。 ログインしてください。 ログインしてください。 ログインしてください。 ログインしてください。
और देखें

- Android 番号: `/img/sns/address_copy_android.svg`

  ![Android ڈوئل کاپی ریفرنس](/img/sns/address_copy_android.svg)

- iOS バージョン: `/img/sns/address_copy_ios.svg`

  ![iOS ڈوئل کاپی ریفرنس](/img/sns/address_copy_ios.svg)

## SDK ヘルパー

SDK サポート ヘルパー サポート I105 サポート サポート サポート
文字列 دیتا ہے تاکہ UI لیئرز مستقل رہیں:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクタ: `inspectAccountId(...)` کمپریسڈ وارننگ 文字列 لوٹاتا ہے
  اور اسے `warnings` میں شامل کرتا ہے جب کالرز `sora...` リテラル دیں، تاکہ والٹ/
  ペースト/検証 فلو میں Sora のみ وارننگ دکھا سکیں، نہ کہ
  صرف تب جب وہ کمپریسڈ فارم خود بنائیں۔
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

ヘルパーの機能 UI のエンコード 機能のエンコードJavaScript
ヘルパー `domainSummary` `selector` ペイロード (`tag`、`digest_hex`、`registry_id`、
`label`) بھی دیتا ہے تاکہ UI یہ ظاہر کر سکیں کہ selector Local-12 ہے یا رجسٹری
生のペイロードを解析する

## 計測器の使用

<エクスプローラーアドレスカード />

テレメトリ アクセシビリティ ミラー ミラー:

- کاپی بٹنز پر `data-copy-mode="i105|I105|qr"` لگائیں تاکہ فرنٹ اینڈز Torii
  میٹرک `torii_address_format_total` کے ساتھ 使用カウンタ نکال سکیں۔ああ
  ڈیمو کمپوننٹ `{mode,timestamp}` کے ساتھ `iroha:address-copy` ایونٹ بھیجتا ہے؛
  アナリティクス/テレメトリー パイプライン (セグメント 6 NORITO 支援コレクター)
  ダッシュボードのダッシュボードのアドレス形式のアドレス形式
  相関関係を示すTorii ドメインカウンター
  (`torii_address_domain_total{domain_kind}`) فیڈ میں بھیجیں تاکہ Local-12
  ریٹائرمنٹ ریویوز `address_ingest` Grafana بورڈ سے براہ راست 30 دن کا ثبوت
  `domain_kind="local12"` 認証済み
- ہر کنٹرول کے لئے الگ `aria-label`/`aria-describedby` ہنٹس دیں جو بتائیں کہ
  リテラル شیئر کرنے کے لئے محفوظ ہے (I105) یا صرف Sora (کمپریسڈ)۔ और देखें
  キャプション 説明 میں شامل کریں تاکہ 支援技術 وہی سیاق دکھائے
  جو بصری طور پر نظر آتا ہے۔
- ライブ リージョン (مثلاً `<output aria-live="polite">...</output>`) رکھیں جو کاپی
  Swift/Android 対応 VoiceOver/TalkBack
  قیے کے مطابق۔

یہ 計装 ADDR-6b پوری کرتی ہے کیونکہ یہ دکھاتی ہے کہ آپریٹرز Local
セレクター Torii インジェスト クライアント側コピー モード
دونوں کا مشاہدہ کر سکتے ہیں۔

## ローカル -> グローバル移行ツールキットローカル セレクターによる変換の監査ヘルパー JSON 監査
I105/کمپریسڈ لسٹ دونوں بناتا ہے جنہیں آپریٹرز 準備チケット کے ساتھ منسلک
ランブック Grafana ダッシュボード アラート マネージャー アラート マネージャー
ストリクトモードのカットオーバー ゲート バージョン

## レイアウト فوری حوالہ (ADDR-1a)

SDK の高度なアドレス ツール (インスペクター、検証ヒント、マニフェスト ビルダー)
開発者 `docs/account_structure.md` 正規ワイヤー
فارمیٹ کی طرف بھیجیں۔レイアウト ہمیشہ `header · selector · controller` ہوتا ہے، جہاں
ヘッダー ビット یہ ہیں:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5)ゼロ以外の数字
  `AccountAddressError::InvalidHeaderVersion` اٹھانی چاہئیں۔
- `addr_class` シングル (`0`) マルチシグ (`1`) コントローラー
- `norm_version = 1` Norm v1 セレクター エンコード重要な規範
  2 ビット فیلڈ کو دوبارہ استعمال کریں گے۔
- `ext_flag` ہمیشہ `0` ہے؛ビット数 ペイロード拡張機能 ہیں۔

セレクターのヘッダーの値:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI SDK とセレクターの説明:

- `0x00` = ضمنی ڈیفالٹ ڈومین (کوئی ペイロード نہیں)۔
- `0x01` = ダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = رجسٹری انٹری (`registry_id:u32` ビッグエンディアン)。

16 進数の مثالیں جنہیں والٹ ٹولنگ ドキュメント/テスト میں لنک یا embed کر سکتی ہے:

|セレクター |正規の 16 進数 |
|---------------|---------------|
| और देखें `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|ダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| और देखें `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

セレクター/ステート `docs/source/references/address_norm_v1.md` اور
バイト図 `docs/account_structure.md` バイト図

## 正規形

ADDR-5 の概要 CLI ワークフローの概要:

1. `iroha tools address inspect` I105 の標準 16 進ペイロード数
   構造化された JSON の概要概要 `kind`/`warning` والے `domain`
   آبجیکٹ بھی ہوتے ہیں اور `input_domain` کے ذریعے دیے گئے ڈومین کو بھی エコー
   ありがとうございます`kind` `local12` ہو تو CLI stderr پر وارننگ دیتا ہے اور JSON
   概要 فنمائی دہراتا ہے تاکہ CI パイプライン اور SDK surface کر سکیں۔
   エンコードを変換する `<i105>@<domain>` を再生する
   فاہیں تو `legacy  suffix` دیں۔
2. SDK の説明/概要 JavaScript ヘルパーの説明:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.I105);
   ```
  ヘルパー リテラル سے 検出 کیا گیا I105 プレフィックス محفوظ رکھتا ہے جب تک آپ
  `networkPrefix` واضح طور پر فراہم نہ کریں؛デフォルト以外のネットワークを使用する
  概要 خاموشی سے デフォルトのプレフィックス کے ساتھ دوبارہ レンダリング نہیں ہوتے۔

3. 正規ペイロード `i105.value` `I105` 再利用
   کریں (یا `--format` کے ذریعے دوسری エンコード مانگیں)۔ یہ 文字列 پہلے سے
   بیرونی شیئرنگ کے لئے محفوظ ہیں۔
4. マニフェスト、レジストリ、顧客向けドキュメント、正規のドキュメント
   ローカル セレクター ローカル セレクター
   فوں گے۔
5. いいえ いいえ いいえ
   `iroha tools address audit --input addresses.txt --network-prefix 753` और देखेंああ
   改行区切りリテラル پڑھتی ہے ( `#` سے شروع ہونے والے コメント نظرانداز
   ہوتے ہیں، اور `--input -` یا کوئی فلیگ نہ ہو تو STDIN استعمال ہوتا ہے)، ہر
   正規/I105 (評価)/圧縮 (`sora`) (`sora`、2 番目に優れた) 要約 JSON 形式の要約
   行数 `--allow-errors` ローカル セレクター
   CI میں بلاک کرنے کے لئے تیار ہوں تو `strict CI post-check` سے آٹومیشن گیٹ کریں۔
6. 改行から改行への書き換え
  ローカルセレクター修復スプレッドシート
  `input,status,format,...` CSV の標準エンコーディング
  警告 解析失敗 پاس میں نمایاں کرے۔ヘルパー ڈیفالٹ طور پر
  ローカル以外の行、エントリ、エンコード (I105/圧縮 (`sora`) 2 番目/16 進数/JSON)
  میں بدلتا ہے، اور `legacy  suffix` پر اصل ڈومین محفوظ رکھتا ہے۔ `--allow-errors`
  ٩ے ساتھ جوڑیں تاکہ خراب リテラル والے ダンプ پر بھی スキャン جاری رہے۔
7. CI/lint 自動化 `ci/check_address_normalize.sh` 開発者
   `fixtures/account/address_vectors.json` ローカル セレクター
   `iroha tools address normalize` 認証済み
   `iroha tools address audit` دوبارہ چلاتی ہے تاکہ ثابت ہو کہ
   ローカルダイジェストをリリース`torii_address_local8_total{endpoint}` いいえ
`torii_address_collision_total{endpoint,kind="local12_digest"}`×
`torii_address_collision_domain_total{endpoint,domain}` ボード Grafana ボード
`dashboards/grafana/address_ingest.json` 施行信号 دیتے ہیں: جب
プロダクション ダッシュボード 30 個の合法的なローカル投稿
ローカル 12 の衝突 Torii ローカル 8 ゲートのメインネットのハード障害
ローカル 12 グローバル ドメインと一致するレジストリ エントリ ہوں۔
オペレータ向けの CLI 出力のフリーズ - 警告文字列
SDK ツールチップ 自動化の管理 ロードマップの終了基準 ロードマップの終了基準
やあ回帰診断、開発/テスト クラスター、`false` または `false`
`torii_address_domain_total{domain_kind}` Grafana (`dashboards/grafana/address_ingest.json`)
ADDR-7 証拠パック ثابت کر سکے کہ ADDR-7 証拠パック
セレクターを無効にするアラートマネージャーパック
(`dashboards/alerts/address_ingest_rules.yml`) ガードレールの名前:

- `AddressLocal8Resurgence` ページ番号 ہے 番号 コンテキスト番号 Local-8
  増分厳密モードのロールアウト、ダッシュボード、問題のある SDK
  デフォルト (`true`) 信号 صفر نہ ہو جائے، پھر デフォルト (`true`) بحال کریں۔
- `AddressLocal12Collision` ローカル 12 ラベル ダイジェスト
  پر ハッシュ ہوں۔マニフェスト プロモーション ローカル -> グローバル ツールキット ダイジェスト
  マッピング Nexus ガバナンス 座標 پہلے کہ
  レジストリ エントリ دوبارہ جاری ہو یا ダウンストリーム ロールアウト بحال ہوں۔
- `AddressInvalidRatioSlo` フリート全体の無効な比率 (ローカル 8/
  厳密モードの拒否 (0.1%) 10 % SLO 0.1% (SLO)
  `torii_address_invalid_total` 説明 コンテキスト/理由 説明
  SDK の所有者 座標 厳密モード 厳密モード

### ریلیز نوٹ اسنیپٹ (والٹ اور ایکسپلورر)

カットオーバー شامل کریں:

> **アドレス:** `iroha tools address normalize` ヘルパー
> ٩یا گیا اور اسے CI (`ci/check_address_normalize.sh`) میں وائر کیا گیا تاکہ
> میں تبدیل کر سکیں، قبل اس کے کہ Local-8/Local-12 メインネット پر بلاک ہوں۔ああ、
> カスタム エクスポート ٩و اپ ڈیٹ کریں تاکہ کمانڈ چلائی جائے اور 正規化されたリスト
> 証拠バンドルをリリースする