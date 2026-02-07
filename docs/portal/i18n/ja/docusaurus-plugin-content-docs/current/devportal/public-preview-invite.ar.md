---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل دعوات المعاينة العامة

## ありがとうございます

يوضح هذا الدليل كيفية اعلان وتشغيل المعاينة العامة بعد تفعيل سير عمل تاهيل المراجعين。 حافظ على
DOCS-SORA をテストし、テストを実行します。 और देखें
ログインしてください。

- **الجمهور:** قائمة منقحة من اعضاء المجتمع والشركاء والـ メンテナー الذين وقعوا سياسة الاستخدامああ、それは。
- **الحدود:** 24 時間以内、25 時間以内、14 時間以内。

## قائمة فحص بوابة الاطلاق

回答:

1. ロシア連邦 CI (`docs-portal-preview`,
   マニフェスト チェックサム、記述子、バンドル SoraFS)。
2. `npm run --prefix docs/portal serve` (مقيد بالchecksum) タグ。
3. تذاكر تاهيل المراجعين معتمدة ومربوطة بموجة الدعوة.
4. 重要な情報
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
5. フィードバックに関する問題の問題 (評価、評価、評価、評価、評価、評価、評価、評価、評価) ）。
6. ドキュメント/DevRel + ガバナンスを強化します。

## حزمة الدعوة

セキュリティ:

1. ** 安全性** — マニフェスト/計画 SoraFS アーティファクト GitHub
   マニフェスト チェックサムと記述子。 ذكر امر التحقق صراحة حتى يتمكن المراجعون
   重要な点は次のとおりです。
2. ** チェックサム — チェックサム:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **評価**********************************************************
   ويجب الابلاغعن الحوادث فورا。
4. **フィードバック** — 問題を解決します。
5. **評価** — 評価/評価/同期
   ログインしてください。

और देखें
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
غطي هذه المتطلبات。 حدّث العناصر النائبة (التواريخ، URLs، وجهات الاتصال)
そうです。

## ظهار مضيف المعاينة

最高のパフォーマンスを見せてください。やあ
[دليل اظهار مضيف المعاينة](./preview-host-exposure.md) لخطوات ビルド/公開/検証 من البداية للنهاية
ありがとうございます。

1. ** リリース タグ وانتج اثارا حتمية。

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

   ピン番号 `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、
   `portal.dns-cutover.json` と `artifacts/sorafs/`。 هذه الملفات بموجة الدعوة
   重要な問題は、重要な問題を解決することです。

2. ** 別名 المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (別名 الصادر عن الحوكمة)。
   マニフェスト `docs-preview.sora` を確認してください
   `portal.manifest.submit.summary.json` و`portal.pin.report.json` は、

3. **فحص النشر:** تاكد من ان alias يحل وانه checksum يطابق tag قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) フォールバック フォールバック
   プレビュー エッジを確認してください。

## いいえ

|ああ |ああ |オーナー |
| --- | --- | --- |
| D-3 |テストを実行し、予行演習を実行します。ドキュメント/開発リリース |
| D-2 | موافقة الحوكمة + تذكرة تغيير |ドキュメント/DevRel + ガバナンス |
| D-1 |追跡者追跡者追跡 | 追跡者追跡者ドキュメント/開発リリース |
| D |キックオフ/オフィスアワー| キックオフ / オフィスアワードキュメント/DevRel + オンコール |
| D+7 |ダイジェスト、フィードバック、トリアージ、トリアージ、フィードバック、トリアージドキュメント/開発リリース |
| D+14 | `status.md` | ログイン して翻訳を追加するドキュメント/開発リリース |

## عبع الوصول والقياس عن بعد

1. プレビュー フィードバック ロガー
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   回答:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   `invite-sent`、`acknowledged`、`feedback-submitted`、
   `issue-opened`、و`access-revoked`。 عيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; بتذكرة موجة الدعوة
   عنماذج الموافقة。概要 ロールアップ ロールアップ 要約
   重要:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   概要 JSON يعد الدعوات لكل موجة، المستلمين المفتوحين، تعداد フィードバック، وطابع
   ありがとうございます。 और देखें
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)、
   CI のワークフローを確認します。ダイジェスト版
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   要約を要約します。
2. ضع وسم `DOCS_RELEASE_TAG` الموجة على لوحات القياس حتى يمكن ربط
   重要なコホート。
3. `npm run probe:portal -- --expect-release=<tag>` の意味
   メタデータを確認してください。
4. ランブックを実行します。

## フィードバック1. 問題に関するフィードバック。認証済み `docs-preview/<wave>` 認証済み
   ロードマップが重要です。
2. 要約プレビュー ロガーの概要を確認する
   `status.md` (ヤステル) وحدّث `roadmap.md`
   DOCS-SORA を確認してください。
3. オフボーディングを行う
   [`reviewer-onboarding`](./reviewer-onboarding.md): ログインしてください。
4. ゲートとチェックサムをチェックする
   قالب الدعوة بتواريخ جديدة.

تطبيق هذا الدليل بشكل متسق يحافظ على قابلية تدقيق برنامج المعاينة ويمنح Docs/DevRel
GA。