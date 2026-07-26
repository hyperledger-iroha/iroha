---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: W2 کمیونٹی ingesta پلان
sidebar_label: W2 پلان
descripción: کمیونٹی cohorte de vista previa کے لئے admisión, aprobaciones, اور lista de verificación de evidencia۔
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W2 - Revisores de کمیونٹی |
| ہدف ونڈو | Tercer trimestre de 2025 ہفتہ 1 (عارضی) |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-06-15` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W2` |

## مقاصد

1. Criterios de admisión y flujo de trabajo de verificación
2. تجویز کردہ lista اور apéndice de uso aceptable کے لئے aprobación de la gobernanza حاصل کرنا۔
3. suma de comprobación, verificar, vista previa del artefacto, paquete de telemetría, etc.
4. دعوت بھیجنے سے پہلے Pruébelo proxy اور paneles کو etapa کرنا۔

## ٹاسک بریک ڈاؤن| identificación | ٹاسک | مالک | مقررہ تاریخ | اسٹیٹس | نوٹس |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | Criterios de admisión (elegibilidad, espacios máximos, requisitos de CoC) تیار کرنا اور gobernanza کو گردش کرنا | Líder de Docs/DevRel | 2025-05-15 | ✅ مکمل | admisión پالیسی `DOCS-SORA-Preview-W2` میں fusionar ہوئی اور 2025-05-20 کے consejo میٹنگ میں respaldar ہوئی۔ |
| W2-P2 | plantilla de solicitud کو کمیونٹی سوالات کے ساتھ اپ ڈیٹ کرنا (motivación, disponibilidad, necesidades de localización) | Documentos-core-01 | 2025-05-18 | ✅ مکمل | `docs/examples/docs_preview_request_template.md` میں اب Comunidad سیکشن شامل ہے، جو ingesta فارم میں حوالہ ہے۔ |
| W2-P3 | admisión پلان کے لئے aprobación de la gobernanza حاصل کرنا (votación de la reunión + acta registrada) | Enlace de gobernanza | 2025-05-22 | ✅ مکمل | ووٹ 2025-05-20 کو متفقہ طور پر پاس ہوا؛ minutos + pase de lista `DOCS-SORA-Preview-W2` میں لنک ہیں۔ |
| W2-P4 | W2 ونڈو کے لئے Pruébelo preparación proxy + captura de telemetría شیڈول کرنا (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | ✅ مکمل | cambiar ticket `OPS-TRYIT-188` منظور اور 2025-06-09 02:00-04:00 UTC میں ejecutar ہوا؛ Grafana capturas de pantalla ٹکٹ کے ساتھ archivo ہیں۔ |
| W2-P5 | Nueva etiqueta de artefacto de vista previa (`preview-2025-06-15`) compilar/verificar کرنا اور descriptor/suma de verificación/archivo de registros de sonda کرنا | Portal TL | 2025-06-07 | ✅ مکمل | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 2025-06-10 کو چلایا گیا؛ salidas `artifacts/docs_preview/W2/preview-2025-06-15/` میں محفوظ ہیں۔ || W2-P6 | کمیونٹی lista de invitados تیار کرنا (<=25 revisores, lotes preparados) información de contacto aprobada por la gobernanza کے ساتھ | Responsable de la comunidad | 2025-06-10 | ✅ مکمل | پہلے cohorte کے 8 revisores de la comunidad منظور ہوئے؛ ID de solicitud `DOCS-SORA-Preview-REQ-C01...C08` rastreador میں لاگ ہیں۔ |

## Lista de verificación de evidencia

- [x] registro de aprobación de gobernanza (notas de la reunión + enlace de votación) `DOCS-SORA-Preview-W2` کے ساتھ منسلک ہے۔
- [x] Plantilla de solicitud actualizada `docs/examples/` کے تحت commit ہے۔
- [x] Descriptor `preview-2025-06-15`, registro de suma de comprobación, salida de sonda, informe de enlace, Pruébalo transcripción proxy `artifacts/docs_preview/W2/` میں محفوظ ہیں۔
- [x] Capturas de pantalla de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) Ventana de verificación previa de W2 کے لئے محفوظ ہیں۔
- [x] Tabla de lista de invitaciones میں ID de revisor, solicitar tickets, اور envío de marcas de tiempo de aprobación سے پہلے بھرے گئے (rastreador کے W2 سیکشن میں دیکھیں)۔

یہ پلان اپ ڈیٹ رکھیں؛ tracker اسے ریفرنس کرتا ہے تاکہ DOCS-SORA roadmap واضح طور پر دیکھ سکے کہ Invitaciones W2 سے پہلے کیا باقی ہے۔