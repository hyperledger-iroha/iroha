---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : خطة التجهيز المسبق لشركاء W1
sidebar_label : par W1
description: مهام، مالكون، وقائمة ادلة لمجموعة معاينة الشركاء.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - شركاء ومتكاملو Torii |
| نافذة الهدف | الربع الثاني 2025 الاسبوع 3 |
| وسم الاثر (مخطط) | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |

## الاهداف

1. الحصول على موافقات قانونية وحوكمة لشروط معاينة الشركاء.
2. Essayez-le et essayez-le.
3. Utilisez la somme de contrôle et les sondes.
4. انهاء قائمة الشركاء وقوالب الطلبات قبل ارسال الدعوات.

## تفصيل المهام| المعرف | المهمة | المالك | الاستحقاق | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على موافقة قانونية على ملحق شروط المعاينة | Responsable Docs/DevRel -> Juridique | 2025-04-05 | ✅ مكتمل | تمت الموافقة على التذكرة القانونية `DOCS-SORA-Preview-W1-Legal` le 2025-04-05؛ ملف PDF مرفق بالمتتبع. |
| W1-P2 | حجز نافذة staging لوكيل Essayez-le (2025-04-10) والتحقق من صحة الوكيل | Docs/DevRel + Ops | 2025-04-06 | ✅ مكتمل | Publié par `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` le 06/04/2025 Utilisez CLI et `.env.tryit-proxy.bak`. |
| W1-P3 | بناء اثر المعاينة (`preview-2025-04-12`), تشغيل `scripts/preview_verify.sh` + `npm run probe:portal`, وارشفة descripteur/sommes de contrôle | Portail TL | 2025-04-08 | ✅ مكتمل | تم حفظ الاثر وسجلات التحقق تحت `artifacts/docs_preview/W1/preview-2025-04-12/`؛ مخرجات sonde مرفقة بالمتتبع. |
| W1-P4 | مراجعة نماذج admission للشركاء (`DOCS-SORA-Preview-REQ-P01...P08`) et les NDA | Liaison gouvernance | 2025-04-07 | ✅ مكتمل | تمت الموافقة على الطلبات الثمانية (اخر طلبين في 2025-04-11)؛ الروابط موجودة في المتتبع. |
| W1-P5 | Prise en charge (pour `docs/examples/docs_preview_invite_template.md`) et `<preview_tag>` et `<request_ticket>` pour votre | Responsable Docs/DevRel | 2025-04-08 | ✅ مكتمل | ارسلت مسودة الدعوة في 2025-04-12 15:00 UTC مع روابط الاثر. |

## قائمة التحقق قبل الاطلاق

> Utiliser : Utiliser `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour les tâches 1 à 5 (build, somme de contrôle, sonde, vérificateur de liens, et essayez-le). JSON est également compatible avec les applications.1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et `build/link-report.json` pour le descripteur.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (او مرر الهدف المناسب عبر `--tryit-target`) ; ثبّت التحديث في `.env.tryit-proxy` et `.bak` لللرجوع.
6. Utilisez W1 pour utiliser la somme de contrôle (descripteur de somme de contrôle, sonde de test, essayez-le, ou Grafana).

## قائمة ادلة الاثبات

- [x] موافقة قانونية موقعة (PDF او رابط التذكرة) مرفقة بـ `DOCS-SORA-Preview-W1`.
- [x] pour Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] descripteur et somme de contrôle pour `preview-2025-04-12` comme `artifacts/docs_preview/W1/`.
- [x] جدول roster للدعوات مع حقول `invite_sent_at` معبأة (راجع سجل W1 في المتتبع).
- [x] اثار التغذية الراجعة منعكسة في [`preview-feedback/w1/log.md`](./log.md) مع صف لكل شريك (تم تحديثه 2025-04-26 ببيانات roster/telemetria/issues).

حدّث هذه الخطة كلما تقدمت المهام؛ يشير اليها المتتبع للحفاظ على قابلية تدقيق خارطة الطريق.

## سير عمل التغذية الراجعة1. لكل مراجع، انسخ القالب في
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   املأ البيانات الوصفية، واحفظ النسخة المكتملة تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. لخص الدعوات ونقاط القياس والمسائل المفتوحة داخل السجل الحي في
   [`preview-feedback/w1/log.md`](./log.md) حتى يتمكن مراجعو الحوكمة من اعادة تشغيل الموجة بالكامل
   دون مغادرة المستودع.
3. عند وصول صادرات المعرفة او الاستبيانات، ارفقها في مسار الاثر المذكور في السجل
   واربط تذكرة المتتبع.