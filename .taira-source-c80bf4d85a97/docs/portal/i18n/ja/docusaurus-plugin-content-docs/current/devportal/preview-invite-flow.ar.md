---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سار دعوات المعاينة

## いいえ

**DOCS-SORA** يحدد تأهيل المراجعين وبرنامج الدعوات للمعاينة العامةベータ版をご利用ください。 صف هذه الصفحة كيفية فتح كل موجة دعوات، وما هي الاثار التي يجب شحنها قبل ارسالありがとうございます。回答:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) を参照してください。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサム。
- [`devportal/observability`](./observability.md) を参照してください。

## いいえ

|ああ |ああ、 عايير الدخول |ログイン して翻訳を追加する重要 |
| --- | --- | --- | --- | --- |
| **W0 - メンテナー الاساسيون** |メンテナはドキュメント/SDK を確認してください。 | GitHub `docs-portal-preview` チェックサム `npm run serve` アラートマネージャー 7 時間。 | P0 のバックログの数が 1 つあります。 |最高のパフォーマンスを見せてください。 、、、、、、、、、、、、、、、、、、、、、、、、。 |
| **W1 - パートナー** |秘密保持契約を締結してください。 |ステージングを試してみてください。 | جمع موافقة الشركاء (issue او نموذج موقع)، القياس عن بعد يظهر <=10 مراجعين متزامنين، لا تراجعات 14月14日。 | فرض قالب الدعوة + تذاكر الطلب. |
| **W2 - ニュース** |重要な問題は、重要な問題です。 | خروج W1، تدريبات الحوادث مجربة، FAQ العامة محدثة. | >=2 を実行すると、ロールバックが実行されます。 | (<=25) وجدولة اسبوعية. |

وثق اي موجة نشطة داخل `status.md` وفي متعقب طلبات المعاينة حتى ترى الحوكمة الوضع بنظرةうーん。

## 飛行前に

回答: **قبل** 回答:

1. **アソシエイトCI متاحة**
   - `docs-portal-preview` + 記述子 `.github/workflows/docs-portal-preview.yml`。
   - ピン لـ SoraFS موثق في `docs/portal/docs/devportal/deploy-guide.md` (記述子 التحويل موجود)。
2. **チェックサム**
   - `docs/portal/scripts/serve-verified-preview.mjs` は `npm run serve` です。
   - `scripts/preview_verify.sh` macOS + Linux をインストールします。
3. ** और देखें
   - `dashboards/grafana/docs_portal.json` يظهر حركة 試してみてください بصحة، وتنبيه `docs.preview.integrity` باللون الاخضر。
   - خر ملحق في `docs/portal/docs/devportal/observability.md` محدث بروابط Grafana。
4. **特別な日**
   - لمتعقب الدعوات جاهزة (واحدة لكل موجة を発行) を発行します。
   - قالب سجل المراجعين منسوخ (انظر [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - 問題は、SRE の問題です。

飛行前のフライトを確認してください。

## いいえ

1. **アオシュタット・アオアシス**
   - ログインしてください。
   - 重要な意味を持っています。
2.******
   - 問題が発生しました。
   - التحقق من المتطلبات (CLA/عقد، استخدام مقبول، موجز امني)。
3. **アフィリエイト**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال)。
   - 記述子 + ハッシュ ステージングを試してみてください。
   - マトリックス/スラック (Matrix/Slack) の問題。
4. **評価と評価**
   - حديث متعقب الدعوات بالقيم `invite_sent_at` و`expected_exit_at` والحالة (`pending`, `active`, `complete`、`revoked`)。
   - ログインしてください。
5. **重要な意味**
   - مراقبة `docs.preview.session_active` وتنبيهات `TryItProxyErrors`。
   - 最高のパフォーマンスを見せてください。
6.******
   - 世界第 1 位、`expected_exit_at` 。
   - تحديث الموجة بملخص قصير (نتائج، حوادث، خطوات تالية) قبل الانتقال للمجموعة التالية。

## いいえ

|認証済み | और देखेंログインしてください。
| --- | --- | --- |
|問題を解決する | GitHub `docs-portal-preview` | حديث بعد كل دعوة. |
|評価 | 評価 | 評価 | 評価`docs/portal/docs/devportal/reviewer-onboarding.md` | ログインしてください。ああ。 |
|ログイン して翻訳を追加する`docs/source/sdk/android/readiness/dashboards/<date>/` (اعادة استخدام حزمة القياس عن بعد) | لكل موجة + بعد الحوادث。 |
|重要な情報 | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (انشاء مجلد لكل موجة) | 5 番目に重要な意味があります。 |
| और देखें `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | DOCS-SORA を確認してください。 |

`cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
重要な情報を入力してください。 JSON の問題の発行 問題の解決 問題の解決 問題の解決 問題の解決テストを行ってください。

ارفق قائمة الادلة بـ `status.md` عند انتهاء كل موجة حتى يتم تحديث مدخل خارطة الطريقやあ。

## معايير التراجع والتوقف

اوقف مسار الدعوات (واخطر الحوكمة) عند حدوث اي مما يلي:- ロールバック (`npm run manage:tryit-proxy`) を試してください。
- 回答: >3 評価 7 評価。
- ニュース: ニュース、ニュース、ニュース、ニュース。
- エラー: チェックサムが不一致です `scripts/preview_verify.sh`。

ستأنف فقط بعد توثيق المعالجة في متعقب الدعوات وتأكيد استقرار لوحة القياس عن بعد 48 番です。