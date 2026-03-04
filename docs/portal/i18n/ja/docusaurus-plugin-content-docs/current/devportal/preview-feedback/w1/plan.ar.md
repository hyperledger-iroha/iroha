---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: خطة التجهيز المسبق لشركاء W1
サイドバーラベル: W1
説明: مهام، مالكون، وقائمة ادلة لمجموعة معاينة الشركاء。
---

|認証済み |翻訳 |
| --- | --- |
|ああ | W1 - 国際 Torii |
| और देखें 2025 年 3 月 |
| وسم الاثر (مخطط) | `preview-2025-04-12` |
|回答 | 回答を表示する`DOCS-SORA-Preview-W1` |

## ああ

1. على موافقات قانونية وحوكمة لشروط معاينة الشركاء.
2. 試してみましょう。
3. チェックサムとプローブ。
4. ログインしてください。

## いいえ

|ああ |ああ |ああ | और देखेंああ |重要 |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على موافقة قانونية على ملحق شروط المعاينة |ドキュメント/DevRel リード -> 法務 | 2025-04-05 | ✅ और देखें 2025-04-05 日 `DOCS-SORA-Preview-W1-Legal` 日 2025 年 4 月 5 日PDF をご覧ください。 |
| W1-P2 | حجز نافذة staging لوكيل 試してみてください (2025-04-10) والتحقق من صحة الوكيل |ドキュメント/DevRel + オペレーション | 2025-04-06 | ✅ और देखें 2025-04-06 日 `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 日CLI と `.env.tryit-proxy.bak` を使用してください。 |
| W1-P3 |説明 (`preview-2025-04-12`) と `scripts/preview_verify.sh` + `npm run probe:portal` 記述子/チェックサム |ポータルTL | 2025-04-08 | ✅ और देखें `artifacts/docs_preview/W1/preview-2025-04-12/`؛ を実行してください。調査は、調査の結果です。 |
| W1-P4 |重要な摂取量 (`DOCS-SORA-Preview-REQ-P01...P08`) と秘密保持契約 |ガバナンス連絡窓口 | 2025-04-07 | ✅ और देखें تمت الموافقة على الطلبات الثمانية (اخر طلبين في 2025-04-11)؛ありがとうございます。 |
| W1-P5 | صياغة دعوة (مبنية على `docs/examples/docs_preview_invite_template.md`) ، وضبط `<preview_tag>` و `<request_ticket>` لكل شريك |ドキュメント/DevRel リード | 2025-04-08 | ✅ और देखें 2025-04-12 15:00 UTC 時間。 |

## قائمة التحقق قبل الاطلاق

> テスト: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` テスト 1-5 テスト (ビルド、チェックサム、プローブ、リンク チェッカー、試行)それ）。 JSON を使用して、JSON を使用してください。

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`) は、`build/checksums.sha256` و `build/release.json` です。
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` 記述子 `build/link-report.json` 記述子。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (`--tryit-target`); `.env.tryit-proxy` واحتفظ بـ `.bak` للرجوع。
6. حدّث تذكرة W1 بمسارات السجلات (チェックサム للdescriptor، مخرجات Probe، تغيير وكيل Try it، ولقطات Grafana)。

## قائمة ادلة الاثبات

- [x] موافقة قانونية موقعة (PDF او رابط التذكرة) مرفقة بـ `DOCS-SORA-Preview-W1`。
- [x] Grafana、`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`。
- [x] 記述子、チェックサム、`preview-2025-04-12`、`artifacts/docs_preview/W1/`。
- [x] 名簿 للدعوات مع حقول `invite_sent_at` معبأة (راجع سجل W1 في المتبع)。
- [x] اثار التغذية الراجعة منعكسة في [`preview-feedback/w1/log.md`](./log.md) مع صف لكل شريك (تم تحديثه 2025-04-26 名簿/テレメトリア/問題)。

حدّث هذه الخطة كلما تقدمت المهام؛ شير اليها المتتبع للحفاظ على قابلية تدقيق خارطة الطريق。

## سير عمل التغذية الراجعة

1. 重要な意味
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)
   الأ البيانات الوصفية، واحفظ النسخة المكتملة تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. 問題を解決する 問題を解決する
   [`preview-feedback/w1/log.md`](./log.md) حتى يتمكن مراجعو الحوكمة من اعادة تشغيل الموجة بالكامل
   ありがとうございます。
3. 最高のパフォーマンスを実現するために、最高のパフォーマンスを実現
   ログインしてください。