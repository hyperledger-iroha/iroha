---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: W1 فيڈبیک اور اختتامی خلاصہ
sidebar_label: W1 خلاصہ
descripción: پارٹنر/Torii onda de vista previa de integradores کے لئے نتائج، اقدامات اور اختتامی ثبوت۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W1 - Integradores پارٹنرز اور Torii |
| دعوتی ونڈو | 2025-04-12 -> 2025-04-26 |
| آرٹیفیکٹ ٹیگ | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |
| شرکا | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## نمایاں نکات

1. **Suma de comprobación ورک فلو** - تمام revisores نے `scripts/preview_verify.sh` کے ذریعے descriptor/archivo کی تصدیق کی؛ registros کو دعوتی agradecimientos کے ساتھ محفوظ کیا گیا۔
2. **ٹیلیمیٹری** - Tableros de instrumentos `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` پوری لہر کے دوران verde رہے؛ کوئی incidentes یا páginas de alerta نہیں ہوئیں۔
3. **Documentos فيڈبیک (`docs-preview/w1`)** - دو معمولی نٹس ریکارڈ ہوئیں:
   - `docs-preview/w1 #1`: Pruébelo سیکشن میں redacción de navegación واضح کرنا (حل ہو گیا)۔
   - `docs-preview/w1 #2`: Pruébelo اسکرین شاٹ اپ ڈیٹ کرنا (حل ہو گیا)۔
4. **Paridad de Runbook** - Operadores SoraFS con enlaces cruzados `orchestrator-ops` y `multi-source-rollout` con enlaces cruzados W0 خدشات حل کیے۔

## ایکشن آئٹمز| identificación | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W1-A1 | `docs-preview/w1 #1` کے مطابق Pruébelo redacción de navegación اپ ڈیٹ کرنا۔ | Documentos-core-02 | ✅ مکمل (2025-04-18). |
| W1-A2 | `docs-preview/w1 #2` کے مطابق Pruébelo اسکرین شاٹ اپ ڈیٹ کرنا۔ | Documentos-core-03 | ✅ مکمل (2025-04-19). |
| W1-A3 | پارٹنر hallazgos اور evidencia de telemetría کو hoja de ruta/estado میں سمری کرنا۔ | Líder de Docs/DevRel | ✅ مکمل (rastreador + status.md دیکھیں). |

## اختتامی خلاصہ (2025-04-26)

- تمام آٹھ revisores نے آخری horario de oficina میں تکمیل کی تصدیق کی، لوکل artefactos صاف کیے، اور ان کی رسائی واپس لی گئی۔
- ٹیلیمیٹری اختتام تک verde رہی؛ Instantáneas `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہیں۔
- دعوتی registro میں salida acuses de recibo شامل کیے گئے؛ rastreador نے W1 کو 🈴 پر سیٹ کیا اور puntos de control شامل کیے۔
- paquete de evidencia (descriptor, registro de suma de verificación, resultado de la sonda, transcripción de proxy Pruébelo, capturas de pantalla de telemetría, resumen de comentarios) `artifacts/docs_preview/W1/` Archivo میں ہوا۔

## اگلے اقدامات

- Plan de admisión de la comunidad W2 تیار کریں (aprobación de gobernanza + ajustes de plantilla de solicitud).
- W2 wave کے لئے etiqueta de artefacto de vista previa ریفریش کریں اور تاریخیں فائنل ہونے پر preflight اسکرپٹ دوبارہ چلائیں۔
- W1 کے قابل اطلاق hallazgos کو hoja de ruta/estado میں منتقل کریں تاکہ onda comunitaria کے پاس تازہ orientación ہو۔