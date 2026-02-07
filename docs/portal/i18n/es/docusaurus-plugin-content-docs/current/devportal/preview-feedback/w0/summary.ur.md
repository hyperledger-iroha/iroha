---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w0-resumen
título: W0 کے وسط کا فيڈبیک خلاصہ
sidebar_label: W0 فيڈبیک (وسط)
descripción: mantenedores principales کے ola de vista previa کے لئے وسطی چیک پوائنٹس، نتائج اور elementos de acción.
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W0 - mantenedores principales |
| خلاصہ تاریخ | 2025-03-27 |
| ریویو ونڈو | 2025-03-25 -> 2025-04-08 |
| شرکا | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidad-01 |
| آرٹیفیکٹ ٹیگ | `preview-2025-03-24` |

## نمایاں نکات

1. **Suma de comprobación ورک فلو** - تمام revisores نے تصدیق کی کہ `scripts/preview_verify.sh`
   مشترکہ descriptor/archivo جوڑی کے خلاف کامیاب رہا۔ کسی دستی anular کی
   ضرورت نہیں ہوئی۔
2. **نیویگیشن فيڈبیک** - barra lateral کی ترتیب میں دو معمولی مسائل رپورٹ ہوئے
   (`docs-preview/w0 #1-#2`). دونوں Docs/DevRel کو دیے گئے اور لہر کو بلاک
   نہیں کرتے۔
3. **SoraFS runbook برابری** - sorafs-ops-01 en `sorafs/orchestrator-ops` اور
   `sorafs/multi-source-rollout` Enlaces cruzados y enlaces cruzados Enlaces cruzados
   número de seguimiento بنایا گیا؛ W1 سے پہلے حل کرنا ہے۔
4. **ٹیلیمیٹری ریویو** - observabilidad-01 نے تصدیق کی کہ `docs.preview.integrity`,
   `TryItProxyErrors` اور Registros de proxy de prueba سب verde رہے؛ Alerta de کوئی فائر نہیں ہوا۔

## ایکشن آئٹمز| identificación | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W0-A1 | entradas de la barra lateral del portal de desarrollo کو دوبارہ ترتیب دینا تاکہ revisores والے docs نمایاں ہوں (`preview-invite-*` کو ایک ساتھ رکھیں). | Documentos-core-01 | مکمل - barra lateral de documentos de revisores کو مسلسل دکھاتا ہے (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` o `sorafs/multi-source-rollout` Enlace cruzado y enlace cruzado | Sorafs-ops-01 | مکمل - ہر runbook اب دوسرے کی طرف لنک کرتا ہے تاکہ rollout کے دوران دونوں گائیڈ نظر آئیں۔ |
| W0-A3 | rastreador de gobernanza کے ساتھ instantáneas de telemetría + paquete de consultas شیئر کرنا۔ | Observabilidad-01 | مکمل - paquete `DOCS-SORA-Preview-W0` کے ساتھ منسلک ہے۔ |

## اختتامی خلاصہ (2025-04-08)

- پانچوں revisores نے تکمیل کی تصدیق کی، لوکل compila صاف کیے، اور vista previa ونڈو سے
  باہر نکل گئے؛ revocaciones de acceso `DOCS-SORA-Preview-W0` میں ریکارڈ ہیں۔
- لہر کے دوران کوئی incidentes یا alertas نہیں ہوئے؛ paneles de telemetría پوری مدت
  verde رہے۔
- نیویگیشن + cross-link اقدامات (W0-A1/A2) نافذ ہو چکے ہیں اور اوپر کے docs میں
  دکھائی دیتے ہیں؛ Rastreador de evidencia de telemetría (W0-A3) کے ساتھ منسلک ہے۔
- paquete de pruebas محفوظ کر دیا گیا: capturas de pantalla de telemetría, دعوت کی reconocimientos,
  اور یہ خلاصہ problema con el rastreador سے لنک ہیں۔

## اگلے اقدامات

- W1 کھولنے سے پہلے W0 elementos de acción نافذ کریں۔
- قانونی منظوری اور proxy staging slot حاصل کریں، پھر [vista previa del flujo de invitación](../../preview-invite-flow.md) میں
  بیان کردہ verificación previa de onda de socio اقدامات پر عمل کریں۔_یہ خلاصہ [rastreador de invitación de vista previa](../../preview-invite-tracker.md) سے منسلک ہے تاکہ
Hoja de ruta DOCS-SORA قابلِ سراغ رہے۔_