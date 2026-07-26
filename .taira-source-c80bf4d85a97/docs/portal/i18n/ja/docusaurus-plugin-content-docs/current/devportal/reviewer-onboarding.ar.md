---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# أهيل مراجعي المعاينة

## いいえ

DOCS-SORA は、セキュリティを強化します。 और देखें
(`npm run serve`) 試してみてください。
テストを行ってください。 يشرح هذا الدليل كيفية جمع الطلبات، والتحقق من الاهلية،
ログインしてください。ああ、
[プレビュー招待フロー](./preview-invite-flow.md) لتخطيط الدفعات، وتيرة الدعوات، وصادرات القياس
ああ、最高のパフォーマンスを見せてください。

- **النطاق:** المراجعين الذين يحتاجون وصولا الى معاينة المستندات (`docs-preview.sora`,
  GitHub Pages SoraFS) GA。
- **خارج النطاق:** مشغلو Torii او SoraFS (مغطون بكتيبات オンボーディング الخاصة بهم)
  और देखें
  [`devportal/deploy-guide`](./deploy-guide.md))。

## دوار والمتطلبات المسبقة

|ああ | और देखेंログイン | ログイン重要 |
| --- | --- | --- | --- |
|コアメンテナー |煙テストを行ってください。 | GitHub と Matrix CLA を比較してください。 | غالبا موجود بالفعل في فريق GitHub `docs-preview`; علك قدم طلبا حتى يكون الوصول قابلا للتدقيق. |
|パートナーレビューアー | SDK をダウンロードしてください。 |プレビューを確認してください。 | يجب الاقرار بمتطلبات القياس عن بعد + التعامل مع البيانات。 |
|地域ボランティア |重要な情報を確認してください。 | GitHub のテスト、CoC のテスト。 | حافظ على صغر الدفعات؛ログインしてください。 |

セキュリティ:

1. 重要な情報。
2. قراءة ملاحق الامن/الرصد
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
3. الموافقة على تشغيل `docs/portal/scripts/preview_verify.sh` قبل تقديم اي
   スナップショット。

## 摂取量

1. طلب من مقدم الطلب تعبئة نموذج
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (او نسخه/لصقه في問題)。バージョン: バージョン バージョン GitHub バージョン バージョン
   最高のパフォーマンスを見せてください。
2. سجل الطلب في متتبع `docs-preview` (GitHub او تذكرة حوكمة を発行) وعين معتمدا。
3. 次の手順:
   - CLA / اتفاقية مساهم في الملف (او مرجع عقد شريك)。
   - 最高のパフォーマンスを実現します。
   - تقييم مخاطر مكتمل (مثال: مراجعي الشركاء تمت الموافقة عليهم من Legal)。
4. 問題を解決する 問題を解決する
   (メッセージ: `DOCS-SORA-Preview-####`)。

## いいえ

1. **説明書** — 説明書記述子 + 説明書ワークフロー CI ピン SoraFS
   (`docs-portal-preview`)。名前:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサム** — チェックサム:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   `scripts/serve-verified-preview.mjs` をビルドしてください。

3. **منح وصول GitHub (اختياري)** — اذا احتاج المراجعون الى فروع غير منشورة، اضفهم الى فريق
   GitHub `docs-preview` は、 طوال فترة المراجعة وسجل تغيير العضوية في الطلب です。

4. **قنوات الدعم** — شارك جهة المناوبة (Matrix/Slack) واجراء الحوادث من
   [`incident-runbooks`](./incident-runbooks.md)。

5. **القياس عن بعد + الملاحظات** — ذكر المراجعين بان التحليلات المجهولة يتم جمعها
   ([`observability`](./observability.md))。問題を解決する 問題を解決する
   [`preview-feedback-log`](./preview-feedback-log) كي يبقى ملخص الموجة محدثا.

## قائمة المراجع

重要な情報:

1. 重要な情報 (`preview_verify.sh`)。
2. `npm run serve` (`serve:verified`) チェックサム。
3. 必要な情報を入力してください。
4. OAuth/試してみる デバイスコード (デバイスコード) (デバイスコード)
   そうです。
5. تسجيل الملاحظات في المتتبع المتفق عليه (issue او مستند مشترك او نموذج) ووضع وسم اصدارああ。

## مسؤوليات المينتينرز وانهاء المشاركة|ログイン | ログインऔर देखें
| --- | --- |
|キックオフ |摂取量 مرفقة بالطلب، مشاركة الاثار + التعليمات، اضافة ادخال `invite-sent` عبر [`preview-feedback-log`](./preview-feedback-log) は、 مزامنة في منتصف الفترة اذا استمرت المراجة لاكثر من اسبوع 。 |
|モニタリング | مراقبة قياس المعاينة (حركة Try it غير معتادة، فشل プローブ) واتباع runbook الحوادث اذا كان هناك شيء مريب。 `feedback-submitted`/`issue-opened` を確認してください。 |
|オフボーディング | GitHub の SoraFS と `access-revoked`, ارشفة الطلب (يتضمن ملخص الملاحظات + الاجراءات المعلقة)، وتحديث سجل المراجعين。 [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)。 |

最高のパフォーマンスを見せてください。 الحفاظ على الاثر داخل المستودع (問題 + قوالب)
يساعد DOCS-SORA على البقاء قابلا للتدقيق ويسمح للحوكمة بتاكيد ان وصول المعاينة اتبعそうです。

## قوالب الدعوة والتتبع

- ログイン してください。
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)。
  チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム チェックサム
  ありがとうございます。
- ログインして、`<preview_tag>` و`<request_ticket>` を確認してください。
  摂取量を摂取する必要がある場合は、摂取量を摂取する必要があります。
  最高のパフォーマンスを見せてください。
- 問題を解決する 問題を解決する 問題を解決する `invite_sent_at` を発行するさいせい
  [プレビュー招待フロー](./preview-invite-flow.md) を参照してください。