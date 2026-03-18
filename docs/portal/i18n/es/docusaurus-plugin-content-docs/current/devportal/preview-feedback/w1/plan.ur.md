---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-plan
título: W1 شراکت داروں کے لئے پری فلائٹ پلان
sidebar_label: W1 پلان
descripción: vista previa de پارٹنر کوہوٹ کے لئے ٹاسکس، مالکان، اور ثبوت چیک لسٹ۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W1 - Integradores پارٹنرز اور Torii |
| ہدف ونڈو | Q2 2025 ہفتہ 3 |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

## مقاصد

1. Vista previa de پارٹنر شرائط کے لئے قانونی اور گورننس منظوری حاصل کرنا۔
2. دعوتی بنڈل میں استعمال ہونے والے Pruébelo proxy اور instantáneas de telemetría تیار کرنا۔
3. suma de comprobación سے verificar شدہ vista previa del artefacto اور sonda نتائج تازہ کرنا۔
4. دعوتیں بھیجنے سے پہلے پارٹنر roster اور plantillas de solicitud حتمی کرنا۔

## ٹاسک بریک ڈاؤن| identificación | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | vista previa del anexo de términos کے لئے قانونی منظوری حاصل کرنا | Líder de Docs/DevRel -> Legal | 2025-04-05 | ✅ مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 کو منظور ہوا؛ PDF ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | Pruébelo proxy کا puesta en escena ونڈو (2025-04-10) محفوظ کرنا اور proxy health کی تصدیق | Documentos/DevRel + Operaciones | 2025-04-06 | ✅ مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 کو چلایا گیا؛ Transcripción CLI اور `.env.tryit-proxy.bak` محفوظ کر دیے گئے۔ |
| W1-P3 | artefacto de vista previa (`preview-2025-04-12`) بنانا، `scripts/preview_verify.sh` + `npm run probe:portal` چلانا، descriptor/sumas de comprobación محفوظ کرنا | Portal TL | 2025-04-08 | ✅ مکمل | artefacto اور registros de verificación `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ ہیں؛ salida de sonda ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P4 | پارٹنر formularios de admisión (`DOCS-SORA-Preview-REQ-P01...P08`) کا جائزہ، contactos اور NDAs کی تصدیق | Enlace de gobernanza | 2025-04-07 | ✅ مکمل | تمام آٹھ درخواستیں منظور ہوئیں (آخری دو 2025-04-11 کو منظور ہوئیں)؛ aprobaciones ٹریکر میں لنک ہیں۔ |
| W1-P5 | دعوتی متن تیار کرنا (`docs/examples/docs_preview_invite_template.md` پر مبنی), ہر پارٹنر کے لئے `<preview_tag>` اور `<request_ticket>` سیٹ کرنا | Líder de Docs/DevRel | 2025-04-08 | ✅ مکمل | دعوت کا مسودہ 2025-04-12 15:00 UTC کو artefacto لنکس کے ساتھ بھیجا گیا۔ |

## Preflight چیک لسٹ> ٹِپ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` چلائیں تاکہ مراحل 1-5 خودکار طور پر چل جائیں (compilación, verificación de suma de comprobación, sonda de portal, verificador de enlaces, Pruébelo, actualización de proxy). اس اسکرپٹ میں JSON لاگ بنتا ہے جسے ٹریکر ایشو کے ساتھ منسلک کیا جا سکتا ہے۔

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) تاکہ `build/checksums.sha256` اور `build/release.json` دوبارہ بنیں۔
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` اور `build/link-report.json` کو descriptor کے ساتھ archivo کریں۔
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (یا مناسب objetivo `--tryit-target` سے دیں)؛ اپ ڈیٹ شدہ `.env.tryit-proxy` کو commit کریں اور rollback کے لئے `.bak` رکھیں۔
6. W1 ٹریکر ایشو کو rutas de registro کے ساتھ اپ ڈیٹ کریں (suma de comprobación del descriptor, salida de la sonda, Pruébelo proxy تبدیلی، اور Instantáneas Grafana) ۔

## ثبوت چیک لسٹ

- [x] دستخط شدہ قانونی منظوری (PDF یا ٹکٹ لنک) `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہے۔
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` کے Grafana capturas de pantalla ۔
- [x] Descriptor `preview-2025-04-12` y registro de suma de comprobación `artifacts/docs_preview/W1/` کے تحت محفوظ ہیں۔
- [x] tabla de lista de دعوت میں `invite_sent_at` marcas de tiempo مکمل ہیں (ٹریکر W1 log دیکھیں)۔
- [x] artefactos de retroalimentación [`preview-feedback/w1/log.md`](./log.md) میں نظر آتے ہیں، ہر پارٹنر کے لئے ایک row (2025-04-26 کو lista/telemetría/problemas ڈیٹا کے ساتھ اپ ڈیٹ)۔

جوں جوں کام آگے بڑھے یہ پلان اپ ڈیٹ کریں؛ ٹریکر اسے hoja de ruta کی auditabilidad برقرار رکھنے کے لئے حوالہ دیتا ہے۔

## فیڈبیک ورک فلو1. ہر crítico کے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) Plantilla de plantilla کاپی کریں،
   میٹا ڈیٹا بھریں اور مکمل کاپی
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` میں رکھیں۔
2. invitaciones, puntos de control de telemetría, problemas abiertos
   [`preview-feedback/w1/log.md`](./log.md) کے registro en vivo میں خلاصہ کریں تاکہ revisores de gobernanza پوری لہر کو
   repositorio کے اندر ہی repetición کر سکیں۔
3. جب verificación de conocimientos یا exportaciones de encuestas آئیں تو انہیں registro میں درج ruta de artefacto پر adjuntar کریں
   اور problema del rastreador سے enlace کریں۔