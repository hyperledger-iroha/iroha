---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::メモ
تعكس هذه الصفحة `docs/source/sns/address_display_guidelines.md` وتعمل الان
كمرجع بوابة موحد。 PR をご覧ください。
:::

SDK を使用して、SDK を使用してください。
さい。 Android 用のアプリ
`examples/android/retail-wallet` UX 番号:

- **هدفا نسخ منفصلان.** وفر زرين واضحين للنسخ: IH58 (المفضل) والصيغة
  ソラ (`sora...`، الخيار الثاني)。 IH58 国際会議
  QR。 يجب ان تتضمن الصيغة المضغوطة تحذيرا مضمنا لانها تعمل فقط داخل
  ソラ。 Android の素材をダウンロード
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`، ويطابق
  iOS SwiftUI UX バージョン `AddressPreviewCard` バージョン
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **خط ثابت ونص قابل للتحديد.** اعرض السلسلتين بخط 等幅 مع
  `textIsSelectable="true"` IME を使用します。
  パスワード: يمكن لـ IME اعادة كتابة kana او حقن نقاط كود بعرض
  うーん。
- **اشارات النطاق الافتراضي الضمني.** عندما يشير المحدد الى النطاق الضمني
  `default`، اعرض توضيحا يذكر المشغلين بان لا حاجة لاي لاحقة。やあ
  ダイジェスト版。
- **حمولات QR IH58.** يجب ان ترمز رموز QR سلسلة IH58. QRコードを入力してください
  خطا واضحا بدلا من صورة فارغة。
- ** और देखें
  IME を使用してください。

Unicode/IME هذه الضوابط يمنع فساد Unicode/IME خارطة الطريق ADDR-6
意味: محافظ/مستكشفات。

## すごい

ニュース ニュース ニュース ニュース ニュース ニュース ニュース
回答:

- Android: `/img/sns/address_copy_android.svg`

  ![مرجع نسخ مزدوج Android](/img/sns/address_copy_android.svg)

- iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع نسخ مزدوج iOS](/img/sns/address_copy_ios.svg)

## SDK を使用する

SDK セキュリティ セキュリティ IH58 セキュリティ セキュリティ UI セキュリティ
重要:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクター: `inspectAccountId(...)` يعيد سلسلة تحذير مضغوطة ويضيفها
  الى `warnings` عندما يقدم المستدعون リテラル `sora...`، حتى يتمكن مستكشفو
  المحافظ/لوحات التحكم من عرض تحذير Sora 限定 اثناء تدفقات اللصق/التحقق بدلا
  عرض ه فقط عند توليد الصيغة المضغوطة ذاتيا.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

UI を使用してください。 और देखें
JavaScript による `selector` `domainSummary` (`tag`, `digest_hex`,
`registry_id`、`label`) حتى تتمكن واجهات UI من تحديد ما اذا كان المحدد Local-12
最高のパフォーマンスを見せてください。

## عرض حي لادوات المستكشف



評価:- طبق `data-copy-mode="ih58|compressed|qr"` على ازرار النسخ حتى تتمكن الواجهات
  عرض المزيد مز مقياس Torii
  `torii_address_format_total`。 और देखें
  `iroha:address-copy` مع `{mode,timestamp}` - اربط ذلك بخط تحليلاتك/تليمترتك
  (مثل ارسالها الى Segment او جامع NORITO) حتى تتمكن لوحات المتابعة من ربط
  最高のパフォーマンスを見せてください。 عكس ايضا عدادات نطاق
  Torii (`torii_address_domain_total{domain_kind}`) は、次のとおりです。
  مراجعات تقاعد Local-12 من تصدير دليل 30 يوم `domain_kind="local12"` مباشرة من
  `address_ingest` と Grafana。
- بط كل عنصر تحكم بتلميحات `aria-label`/`aria-describedby` مميزة تشرح ما اذا
  كانت السلسلة امنة للمشاركة (IH58) او خاصة بـ Sora (مضغوطة)。認証済み
  最高のパフォーマンスを見せてください。
- وفر منطقة اعلان حية (مثل `<output aria-live="polite">...</output>`) تعلن
  ボイスオーバー/トークバック ボイスオーバー/トークバック ボイスオーバー/トークバック ボイスオーバー/トークバック
  Swift/Android。

هذه الاتاحة تحقق ADDR-6b عبر اثبات قدرة المشغلين على مراقبة كل من ادخال Torii
ローカル。

## ローカル -> グローバル

[ローカル -> グローバル](local-to-global-toolkit.md) دارة تدقيق وتحويل
地元の人々。 JSON を使用して、JSON を使用します。
IH58/国際標準化機構 国際規格 IH58/国際規格
Grafana アラートマネージャーがカットオーバーされました。

## مرجع سريع للتخطيط الثنائي (ADDR-1a)

SDK の定義 (マニフェスト マニフェスト)
ワイヤー `docs/account_structure.md`。ああ
ヘッダー `header · selector · controller`:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5)ログイン して翻訳を追加する
  `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` يميز بين المتحكم الفردي (`0`) والمتعدد التواقيع (`1`)。
- `norm_version = 1` يشفر قواعد محدد Norm v1。 ستعيد المعايير المستقبلية استخدام
  2 番目に重要です。
- `ext_flag` 認証 `0` 認証最高のパフォーマンスを見せてください。

ヘッダー:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI と SDK の管理:

- `0x00` = نطاق افتراضي ضمني (بدون حمولة)。
- `0x01` = ダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = مدخل سجل عالمي (`registry_id:u32` ビッグエンディアン)。

16 進数 16 進数 16 進数 16 進数 ドキュメント/テスト:

|ログイン | ログインヘックス |
|---------------|---------------|
| और देखें `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|ダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|ニュース (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

راجع `docs/source/references/address_norm_v1.md` لجدول المحدد/الحالة الكامل و
`docs/account_structure.md` ログインしてください。

## فرض الصيغ القانونية

ローカル アクセス IH58 接続 IH58 接続
CLI と ADDR-5:1. `iroha tools address inspect` يصدر الان ملخص JSON منظم مع IH58 والحمولة المضغوطة
   16 進数。評価 `domain` 評価 `kind`/`warning`
   عرض المزيد `input_domain`. عندما يكون `kind` هو `local12`
   CLI の標準エラー出力 ويعكس ملخص JSON 認証 التوجيه حتى تتمكن خطوط CI و
   SDK は次のとおりです。 مرر `legacy  suffix` متى اردت اعادة تشغيل الترميز المحول
   كـ `<ih58>@<domain>`。
2. SDK の開発/開発 JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  يحافظ المساعد على بادئة IH58 المكتشفة من literal ما لم تقدم `networkPrefix`
  最高のパフォーマンスを見せてください。

3. テスト `ih58.value` テスト `compressed`
   من الملخص (او اطلب ترميزا اخر عبر `--format`)。 هذه السلاسل امنة بالفعل
   意味。
4. マニフェストは、マニフェストをマニフェストします。
   ローカルのカットオーバー。
5. 意味を理解する
   `iroha tools address audit --input addresses.txt --network-prefix 753`。セキュリティ
   リテラル مفصولة باسطر جديدة (التعليقات التي تبدا بـ `#` يتم تجاهلها، و
   `--input -` او عدم وجود علم يستخدم STDIN) ، ويصدر تقرير JSON بملخصات
   ローカル。ああ
   `--allow-errors` عند تدقيق ダンプ القديمة التي تحتوي صفوفا مهملة، واضبط
   ローカル CI です。
6. عندما تحتاج لاعادة كتابة سطر بسطر، استخدم
  ローカル情報
  CSV `input,status,format,...` セキュリティ セキュリティ
  خفاقات التحليل في مرور واحد。 يتخطى المساعد الصفوف غير المحلية افتراضيا،
  ويحول كل ادخال متبق الى الترميز المطلوب (IH58/مضغوط/hex/JSON)، ويحافظ على
  `legacy  suffix` を参照してください。 `--allow-errors`
  リテラル تالفة 。
7. CI/lint تشغيل `ci/check_address_normalize.sh` الذي يستخرج محددات
   ローカル `fixtures/account/address_vectors.json`، ويحولها عبر
   `iroha tools address normalize`، ويعيد تشغيل
   `iroha tools address audit` ダイジェスト
   地元。

`torii_address_local8_total{endpoint}` 認証済み
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}`、Grafana
`dashboards/grafana/address_ingest.json` 評価: 評価:
ローカル ローカル 12 ローカル 12 30 يوما
Torii ローカル 8 ローカル 8 メインネット セキュリティ ローカル 12 ローカル 12
あなたのことを忘れないでください。 CLI を使用する
- ツールチップ SDK のツールチップ
重要な問題は、次のとおりです。 Torii 認証済み
回帰。認証済み `torii_address_domain_total{domain_kind}` 認証済み
Grafana (`dashboards/grafana/address_ingest.json`) ADDR-7 の意味
اثبات ان `domain_kind="local12"` بقيت صفرا خلال نافذة 30 يوما المطلوبة قبل ان
メインネットを管理します。アラートマネージャー
(`dashboards/alerts/address_ingest_rules.yml`) 評価:- `AddressLocal8Resurgence` يستدعي عندما يبلغ سياق عن زيادة Local-8 جديدة。ああ
  ロールアウトの確認 セキュリティ SDK の確認 セキュリティ ロールアウトの確認
  (`true`)。
- `AddressLocal12Collision` يعمل عندما يقوم اسمان Local-12 ハッシュ الى نفس
  ダイジェスト。マニフェストの更新 ローカル -> グローバルのダイジェスト
  Nexus ロールアウトのロールアウト
  下流側。
- `AddressInvalidRatioSlo` يحذر عندما يتجاوز معدل عدم الصلاحية على مستوى
  الاسطول (ローカル 8/ストリクト モード) 0.1% です。ああ
  `torii_address_invalid_total` グラフィックス SDK および SDK
  重要な情報を確認してください。

### مقتطف مذكرة الاصدار (محفظة ومُستكشف)

バージョンとバージョンのカットオーバー:

> **العناوين:** تمت اضافة مساعد `iroha tools address normalize`
> وربطه في CI (`ci/check_address_normalize.sh`) حتى تتمكن مسارات المحفظة/المستكشف
> ローカル アクセス IH58/ローカル 8/ローカル 12
> メインネット。 حدث اي عمليات تصدير مخصصة لتشغيل الامر وارفق القائمة المعيارية
> を参照してください。