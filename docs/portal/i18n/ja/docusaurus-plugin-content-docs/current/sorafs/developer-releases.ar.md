---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: عملية الإصدار
概要: CLI/SDK の概要: CLI/SDK の概要:ああ。
---

# いいえ

SoraFS (`sorafs_cli`、`sorafs_fetch`、ヘルパー) の SDK
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) です。 حافظ خط إصدار واحد على
CLI のテストと lint/test のテストの実行
ヤステル。重要な問題は、次のとおりです。

## 0. أكيد اعتماد مراجعة الأمان

重要な情報:

- SF-6 の評価 ([レポート/sf6-security-review](./reports/sf6-security-review.md))
  SHA256 と互換性があります。
- أرفق رابط تذكرة المعالجة (مثل `governance/tickets/SF6-SR-2026.md`) وسجّل
  セキュリティ エンジニアリング ツール ワーキング グループ。
- حقّق من إغلاق قائمة المعالجة في المذكرة؛最高のパフォーマンスを見せてください。
- ハーネス ハーネス (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  ありがとうございます。
- 評価 - 評価 - 評価 - 評価 - 評価 - 評価 - 評価 - 評価 - 評価 `--identity-token-provider` و
  `--identity-token-audience=<aud>` صريحًا حتى يُلتقط نطاق Fulcio ضمن أدلة الإصدار。

重要な問題は、次のとおりです。

## 1. 評価/分析

يشغّل المساعد `ci/check_sorafs_cli_release.sh` التنسيق وClippy والاختبارات عبر
CLI と SDK のターゲット インターフェイス (`.target`) のターゲット インターフェイス
CI を参照してください。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

ينفذ سكربت التحققات التالية:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` `sorafs_car` (`cli`)
  و`sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` を表示します。

重要な問題を解決してください。 جب أن تكون بناءات
メインメッセージを表示チェリーピックを選択してください。
تحقق البوابة أيضًا من توفير أعلام التوقيع دون مفاتيح (`--identity-token-issuer`,
`--identity-token-audience`) 認証済みテストを行ってください。

## 2. いいえ。

CLI/SDK のバージョン SoraFS バージョン:

- `MAJOR`: バージョン 1.0。 1.0 セキュリティ `0.y`
  ** セキュリティ セキュリティ** セキュリティ デバイス、CLI セキュリティ、Norito。
- `MINOR`: ميزات متوافقة للخلف (أوامر/أعلام جديدة، حقول Norito جديدة خلف سياسة
  (英語)。
- `PATCH`: 認証済み 認証済み 認証済み 認証済み 認証済み 認証済み 認証済み
  ああ。

حافظ دائمًا على نسخ `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` متطابقة كي
SDK をダウンロードしてください。回答:

1. حدّث حقول `version =` في كل `Cargo.toml`.
2. `Cargo.lock` `cargo update -p <crate>@<new-version>` (評価)
   ساحة العمل نسخًا صريحة）。
3. شغّل بوابة الإصدار مرة أخرى لضمان عدم بقاء آرتيفاكتات قديمة。

## 3. 重要な問題

セキュリティ マークダウン セキュリティ CLI وSDK セキュリティ セキュリティ セキュリティ
ああ。 ستخدم القالب في `docs/examples/sorafs_release_notes.md` (انسخه إلى)
دليل آرتيفاكتات الإصدار واملأ الأقسام بتفاصيل ملموسة)。

回答:

- ** バージョン**: CLI および SDK 。
- **التوافق**: غييرات كاسرة، ترقيات السياسات، المتطلبات الدنيا للبوابة/العقدة。
- **コメント: TL;DR は、貨物、備品、および備品を含みます。
- **التحقق**: جزئات مخرجات الأوامر أو الأظرف والإصدار الدقيق لـ
  `ci/check_sorafs_cli_release.sh` 認証済み。

أرفق ملاحظات الإصدار المكتملة بالوسم (مثل نص إصدار GitHub) وخزّنها بجانب
ありがとうございます。

## 4. いいえ。

شغّل `scripts/release_sorafs_cli.sh` لتوليد حزمة التوقيع وملخص التحقق الذي
ُشحنمع كل إصدار. يقوم الغلاف ببناء CLI عند الحاجة، ويستدعي
`sorafs_cli manifest sign`، ثم يعيد تشغيل `manifest verify-signature` فورًا
重要なことは、次のことです。意味:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

例文:- تتبع مدخلات الإصدار (الحمولة، الخطط، الملخصات، تجزئة الرمز المتوقعة) داخل
  最高のパフォーマンスを見せてください。フォローする
  備品は `fixtures/sorafs_manifest/ci_sample/` です。
- CI 認証 `.github/workflows/sorafs-cli-release.yml` 認証فهي تشغّل بوابة
  ワークフローを確認します。
  حافظ على ترتيب الأوامر نفسه (بوابة الإصدار → التوقيع → التحقق) في أنظمة CI الأخرى
  テストを行ってください。
- حتفظ بالملفات `manifest.bundle.json` و`manifest.sig` و`manifest.sign.summary.json`
  و`manifest.verify.summary.json` معًا — فهي تشكّل الحزمة المشار إليها في إشعار الحوكمة.
- 試合の試合結果、チャンクの試合結果、チャンクの試合結果、チャンクの試合結果
  والملخصات إلى `fixtures/sorafs_manifest/ci_sample/` (وحدّث)
  `docs/examples/sorafs_ci_sample/manifest.template.json`) です。
  試合は、試合の試合結果を確認します。
- التقط سجل تشغيل تحقق قنوات `sorafs_cli proof stream` ذات الحدود وأرفقه بحزمة
  証明ストリーミングの証明。
- سجّل قيمة `--identity-token-audience` الدقيقة المستخدمة أثناء التوقيع في ملاحظات
  ああ最高のパフォーマンスを実現します。

ستخدم `scripts/sorafs_gateway_self_cert.sh` عندما يحمل الإصدار أيضًا إطلاقًا
すごい。 جّهه إلى حزمة المانيفست نفسها لإثبات أن الشهادة تطابق الآرتيفاكت المرشح:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5.

回答:

1. شغّل `sorafs_cli --version` و`sorafs_fetch --version` لتأكيد أن الثنائيات تعلن
   ありがとうございます。
2. جهّز إعداد الإصدار في ملف `sorafs_release.toml` مُلتزم في المستودع (مفضّل) أو
   重要な情報を確認してください。 جنّب الاعتماد على متغيرات بيئة عشوائية؛ああ
   CLI 認証 `--config` (認証) 認証を取得するための認証を取得します。
   すごいですね。
3. 重要な意味 (مفضّل) 意味:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. ارفع الآرتيفاكتات (حزم CAR، المانيفستات، ملخصات الأدلة، ملاحظات الإصدار، مخرجات
   الشهادات) إلى سجل المشروع وفق قائمة تحقق الحوكمة في [دليل النشر](./developer-deployment.md)。
   試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果 試合結果
   كي تتمكن أتمتة التدقيق من مقارنة الحزمة المنشورة مع التحكم بالمصدر.
5. ニュース ニュース ニュース ニュース ニュース ニュース /
   `manifest.sign/verify` を確認してください。 CI です。
   (أو أرشيف السجلات) التي شغّلت `ci/check_sorafs_cli_release.sh` و
   `scripts/release_sorafs_cli.sh`。 حدّث تذكرة الحوكمة ليتمكن المدققون من تتبع
   और देखेंジョブ `.github/workflows/sorafs-cli-release.yml`
   を確認してください。

## 6. 大事なこと

- تأكد من تحديث الوثائق التي تشير إلى الإصدار الجديد (البدء السريع، قوالب CI)
  ありがとうございます。
- أضف عناصر إلى خارطة الطريق إذا لزم عمل لاحق (مثل أعلام الترحيل أو إيقاف
  必要です)。
- ニュース、ニュース、ニュース、ニュース、ニュース、ニュース、ニュース、ニュース。

CLI と SDK を使用して、必要なソフトウェアを開発します。