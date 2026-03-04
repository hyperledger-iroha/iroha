---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-resumen
título: W2 فيڈبیک اور اسٹیٹس خلاصہ
sidebar_label: W2 خلاصہ
descripción: ola de vista previa de la comunidad (W2) کے لئے resumen en vivo۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W2 - revisores de la comunidad |
| دعوتی ونڈو | 2025-06-15 -> 2025-06-29 |
| آرٹیفیکٹ ٹیگ | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |
| شرکا | comunicación-vol-01... comunicación-vol-08 |

## نمایاں نکات

1. **Herramientas de gobernanza** - admisión de la comunidad پالیسی 2025-05-20 کو متفقہ طور پر منظور ہوئی؛ motivación/zona horaria فیلڈز کے ساتھ اپ ڈیٹ plantilla de solicitud `docs/examples/docs_preview_request_template.md` میں موجود ہے۔
2. **Evidencia de verificación previa** - Pruébelo cambio de proxy `OPS-TRYIT-188` 2025-06-09 کو چلایا گیا، Grafana paneles de control کیپچر کیے گئے، اور `preview-2025-06-15` کے descriptor/suma de comprobación/salidas de sonda `artifacts/docs_preview/W2/` میں archivo کیے گئے۔
3. **Ola de invitación** - آٹھ revisores de la comunidad کو 2025-06-15 کو مدعو کیا گیا، tabla de invitaciones del rastreador de reconocimientos میں لاگ ہوئے؛ سب نے navegación سے پہلے verificación de suma de comprobación مکمل کیا۔
4. **Comentarios** - `docs-preview/w2 #1` (redacción de información sobre herramientas) اور `#2` (orden de barra lateral de localización) 2025-06-18 کو فائل ہوئے اور 2025-06-21 تک حل ہو گئے (Docs-core-04/05)؛ لہر کے دوران کوئی incidentes نہیں ہوئے۔

## ایکشن آئٹمز| identificación | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W2-A1 | `docs-preview/w2 #1` (redacción de información sobre herramientas) حل کرنا۔ | Documentos-core-04 | ✅ مکمل (2025-06-21). |
| W2-A2 | `docs-preview/w2 #2` (barra lateral de localización) حل کرنا۔ | Documentos-core-05 | ✅ مکمل (2025-06-21). |
| W2-A3 | salir del archivo de pruebas کرنا + hoja de ruta/estado اپ ڈیٹ کرنا۔ | Líder de Docs/DevRel | ✅ مکمل (29/06/2025). |

## اختتامی خلاصہ (2025-06-29)

- تمام آٹھ revisores de la comunidad نے تکمیل کی تصدیق کی اور acceso a vista previa واپس لے لیا گیا؛ registro de invitaciones del rastreador de agradecimientos میں ریکارڈ ہوئے۔
- Instantáneas de telemetría (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) verde رہے؛ registros اور Pruébelo transcripciones proxy `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہیں۔
- paquete de evidencia (descriptor, registro de suma de verificación, resultado de la sonda, informe de enlace, capturas de pantalla Grafana, reconocimientos de invitación) `artifacts/docs_preview/W2/preview-2025-06-15/` Archivo میں ہوا۔
- rastreador کا W2 salida del registro del punto de control تک اپ ڈیٹ کیا گیا تاکہ hoja de ruta Planificación W3 شروع ہونے سے پہلے listo para auditoría ریکارڈ رکھے۔