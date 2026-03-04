---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-operator-onboarding
title: تاهيل مشغلي データスペース في Sora Nexus
説明: نسخة مطابقة لـ `docs/source/sora_nexus_operator_onboarding.md` تتبع قائمة تدقيق الاصدار المشغلي Nexus.
---

:::メモ
テストは `docs/source/sora_nexus_operator_onboarding.md` です。重要な問題は、次のとおりです。
:::

# تاهيل مشغلي Data-Space في Sora Nexus

يلتقط هذا الدليل التدفق الشامل الذي يجب ان يتبعه مشغلو データスペース في Sora Nexus بعد الاعلان عنああ。 هو يكمل دليل المسارين (`docs/source/release_dual_track_runbook.md`) ومذكرة اختيار القطع (`docs/source/release_artifact_selection.md`) عبر شرح كيفية مواءمةマニフェスト/マニフェスト/マニフェスト/レーン/レーンを表示します。

## جمهور والمتطلبات المسبقة
- データスペース Nexus واستلمت تعيين データスペース (فهرس LANE، ومعرف/エイリアス للـ data-space، ومتطلبات سياسة )。
- リリース エンジニアリング (tarballs マニフェスト مفاتيح عامة)。
- 検証ツール/オブザーバー (検証ツール/オブザーバー) (Ed25519 ツール、BLS + PoP 検証ツール) () をオンに切り替えます。
- ソラ Nexus は、ブートストラップを実行します。

## الخطوة 1 - تاكيد ملف الاصدار
1. エイリアスはチェーン ID です。
2. `scripts/select_release_profile.py --network <alias>` (`--chain-id <id>`) は、次のことを意味します。 بقراءة `release/network_profiles.toml` ويطبع ملف النشر。ソラ Nexus يجب ان تكون النتيجة `iroha3`。リリースエンジニアリング。
3. セキュリティ セキュリティ (مثلا `iroha3-v3.2.0`);マニフェストを作成します。

## الخطوة 2 - جلب القطع والتحقق منها
1. قم بتنزيل حزمة `iroha3` (`<profile>-<version>-<os>.tar.zst`) وملفاتها المرافقة (`.sha256`، اختياري `.sig/.pub`،) `<profile>-<version>-manifest.json`، و `<profile>-<version>-image.json` ذا كنت تنشر عبر الحاويات）。
2. 必要な情報:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   `openssl` は、KMS を使用してください。
3. `PROFILE.toml` tarball の JSON 形式:
   - `profile = "iroha3"`
   - `version` و`commit` و`built_at` を確認してください。
   - ナタリー / ナタリー / ナタリー。
4. ハッシュ/ハッシュ/イメージID `<profile>-<version>-<os>-image.tar` イメージID `<profile>-<version>-image.json`。

## الخطوة 3 - تجهيز الاعداد من القوالب
1. `config/` は、次のことを意味します。
2. `config/` 番号:
   - `public_key`/`private_key` 認証済み Ed25519 認証済み。 HSM を使用してください。 HSM を使用してください。
   - `trusted_peers` و`network.address` و`torii.address` とピアブートストラップ。
   - حدث `client.toml` بنقطة نهاية Torii الموجهة للمشغل (بما في ذلك اعداد TLS اذا كان ذلك ينطبق) وبالاعتمادات التي قمت بتجهيزها لادوات التشغيل。
3. チェーン ID チェーン ID チェーン ガバナンス レーン - レーン レーンありがとうございます。
4. خطط لتشغيل العقدة مع خيار ملف Sora: `irohad --sora --config <path>`.管理者は、SoraFS とマルチレーンを使用します。

## الخطوة 4 - مواءمة بيانات データスペース وسياسات التوجيه
1. `config/config.toml` 評価 `[nexus]` 評価データスペース 評価 Nexus 評議会:
   - レーン、レーン、`lane_count` レーン。
   - エイリアス `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` エイリアス `index`/`id` エイリアスそうです。翻訳: 翻訳: 翻訳: 翻訳: 翻訳: 翻訳: 翻訳エイリアス エイリアス エイリアス エイリアス データスペース エイリアス。
   - データスペース `fault_tolerance (f)`;レーンリレー `3f+1` です。
2. حدث `[[nexus.routing_policy.rules]]` لالتقاط السياسة المعطاة لك.レーン `1` レーン `2`。データスペースとレーンの別名。リリース エンジニアリングを担当します。
3. `[nexus.da]` و`[nexus.da.audit]` و`[nexus.da.recovery]`。 من المتوقع ان يحتفظ المشغلون بالقيم المعتمدة من المجلس; 、 、 、 、 、 、 、 、 、 、 、 、 、 、、、、、、、、、、、、、、、、、、、、、、、、、、、
4. 重要な問題を解決します。ランブック ステータス `config.toml` ステータス (مع تنقيح الاسرار) ステータス。## الخطوة 5 - التحقق قبل التشغيل
1. 説明:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   يطبع هذا الاعدادات النهائية ويفشل مبكرا اذا كانت ادخالات الكتالوج/التوجيه غير متسقة اوジェネシスの起源。
2. اذا كنت تنشر باستخدام الحاويات، شغل نفس الامر داخل الصورة بعد تحميلها عبر `docker load -i <profile>-<version>-<os>-image.tar` (`--sora`)。
3. レーン/データスペースのプレースホルダー。 ذا وجدت، ارجع الى الخطوة 4 - لا يجب ان تعتمد عمليات الانتاج على معرفات プレースホルダー المرفقة معああ。
4. 煙 المحلي (مثلا ارسل استعلام `FindNetworkStatus` عبر `iroha_cli`، وتاكد من ان نقاط نهاية `nexus_lane_state_total`، وتحقق من مفاتيح البث تم تدويرها او استيرادها حسب الحاجة）。

## 6 - 6 - 6 - 6 - 6 - 6
1. خزّن `manifest.json` الذي تم التحقق منه وقطع التوقيع في تذكرة الاصدار حتى يتمكن المدققون منありがとうございます。
2. Nexus の操作。意味:
   - هوية العقدة (ピア ID ، اسماء المضيفين، نقطة نهاية Torii)。
   - レーン/データスペースの管理。
   - ハッシュ للثنائيات/الصور التي تم التحقق منها。
3. نسق القبول النهائي للاقران (ゴシップの種 وتخصيص レーン) مع `@nexus-core`。 على الموافقة;ソラ Nexus レーン マニフェスト قبول محدث。
4. ランブックのランブックは、ランブックのランブックをオーバーライドします。ベースライン。

## قائمة تدقيق مرجعية
- [ ] تم التحقق من ملف الاصدار كـ `iroha3`。
- [ ] ハッシュ والتواقيع للحزمة/الصورة。
- [ ] は、ピア、Torii を表示します。
- [ ] レーン/データスペース التوجيه في Nexus تطابق تعيين المجلس。
- [ ] مدقق الاعداد (`irohad --sora --config ... --trace-config`) يمر بدون تحذيرات。
- [ ] تم ارشفة マニフェスト/التواقيع في تذكرة التاهيل وتم اخطار Ops.

Nexus وتوقعات التليمتري، راجع [Nexus 移行ノート](./nexus-transition-notes)。