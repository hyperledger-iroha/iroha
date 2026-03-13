---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: vista previa-comentarios-w1-log
título: W1 فيڈبیک اور ٹیلیمیٹری لاگ
sidebar_label: W1 menú
descripción: پہلی پارٹنر ola de vista previa کے لئے مجموعی lista, ٹیلیمیٹری puntos de control, اور revisores نوٹس۔
---

یہ لاگ **W1 پارٹنر vista previa** کے لئے lista de invitaciones, ٹیلیمیٹری puntos de control, اور comentarios de revisores محفوظ کرتا ہے
جو [`preview-feedback/w1/plan.md`](./plan.md) کی tareas de aceptación اور wave tracker اندراج
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) کے ساتھ وابستہ ہیں۔ جب دعوت ارسال ہو،
ٹیلیمیٹری instantánea ریکارڈ ہو، یا clasificación de elementos de retroalimentación ہو تو اسے اپ ڈیٹ کریں تاکہ revisores de gobernanza
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت repetición کر سکیں۔

## Lista de cohortes| ID de socio | Solicitar billete | Acuerdo de confidencialidad | Invitación enviada (UTC) | Confirmación/primer inicio de sesión (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| socio-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Actualización 2025-04-26 | sorafs-op-01; Orchestrator documenta evidencia de paridad پر فوکس۔ |
| socio-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Actualización 2025-04-26 | sorafs-op-02; Norito/enlaces cruzados de telemetría کی توثیق۔ |
| socio-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Actualización 2025-04-26 | sorafs-op-03; simulacros de conmutación por error de múltiples fuentes چلائے۔ |
| socio-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Actualización 2025-04-26 | torii-int-01; Torii `/v2/pipeline` + Pruébalo reseña de libro de cocina۔ |
| socio-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Actualización 2025-04-26 | torii-int-02; Pruébelo con la actualización de captura de pantalla میں ساتھ دیا (docs-preview/w1 #2). |
| socio-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Actualización 2025-04-26 | socio-sdk-01; Comentarios sobre el libro de cocina de JS/Swift + comprobaciones de integridad del puente ISO۔ |
| socio-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Actualización 2025-04-26 | socio-sdk-02; cumplimiento 2025-04-11 کو clear، Notas de conexión/telemetría پر فوکس۔ || socio-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Actualización 2025-04-26 | puerta de enlace-ops-01; auditoría de la guía de operaciones de puerta de enlace + anónimo Pruébelo flujo de proxy۔ |

**Invitación enviada** اور **Ack** کے marcas de tiempo فوری طور پر درج کریں جب correo electrónico saliente جاری ہو۔
وقت کو W1 پلان میں دی گئی Horario UTC کے مطابق رکھیں۔

## Puntos de control de telemetría

| Marca de tiempo (UTC) | Cuadros de mandos / sondas | Propietario | Resultado | Artefacto |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operaciones | ✅ Todo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` transcripción | Operaciones | ✅ Escenificado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Cuadros de mandos + `probe:portal` | Documentos/DevRel + Operaciones | ✅ Instantánea previa a la invitación, sin regresiones | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Paneles de control + Pruébelo, diferencia de latencia de proxy | Líder de Docs/DevRel | ✅ Comprobación del punto medio superada (0 alertas; Pruébelo latencia p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Cuadros de instrumentos اوپر + sonda de salida | Docs/DevRel + enlace de gobernanza | ✅ Instantánea de salida, cero alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

روزانہ muestras de horario de oficina (2025-04-13 -> 2025-04-25) Exportaciones NDJSON + PNG کے طور پر
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` میں موجود ہیں، فائل نام
`docs-preview-integrity-<date>.json` اور متعلقہ capturas de pantalla کے ساتھ۔

## Comentarios y registro de problemasاس جدول کو hallazgos del revisor کے خلاصے کے لئے استعمال کریں۔ ہر entrada کو GitHub/discutir
billete اور اس forma estructurada سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کے ذریعے بھرا گیا۔

| Referencia | Gravedad | Propietario | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Bajo | Documentos-core-02 | ✅ Resuelto 2025-04-18 | Pruébelo con texto de navegación + ancla de barra lateral واضح کیا (`docs/source/sorafs/tryit.md` نئے etiqueta کے ساتھ اپ ڈیٹ). |
| `docs-preview/w1 #2` | Bajo | Documentos-core-03 | ✅ Resuelto 2025-04-19 | Pruébelo captura de pantalla + revisor de subtítulos کی درخواست پر اپ ڈیٹ؛ artefacto `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Información | Líder de Docs/DevRel | 🟢 Cerrado | باقی تبصرے صرف Preguntas y respuestas تھے؛ ہر پارٹنر کے formulario de comentarios میں محفوظ ہیں `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Verificación de conocimientos y seguimiento de encuestas

1. ہر revisor کے puntuaciones de las pruebas ریکارڈ کریں (objetivo >=90%); CSV exportado کو invitar artefactos کے ساتھ adjuntar کریں۔
2. formulario de comentarios سے حاصل respuestas a encuestas cualitativas جمع کریں اور انہیں
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` کے تحت espejo کریں۔
3. umbral سے نیچے والوں کے لئے programación de llamadas de remediación کریں اور انہیں اس فائل میں log کریں۔

تمام آٹھ revisores نے verificación de conocimientos میں >=94% اسکور کیا (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). کوئی llamadas de remediación درکار نہیں ہوئیں؛
ہر پارٹنر کے exportaciones de encuestas یہاں موجود ہیں:
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefactos- Vista previa del descriptor/paquete de suma de comprobación: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumen de sonda + enlace-comprobación: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Pruébelo, registro de cambios de proxy: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportaciones de telemetría: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Paquete de telemetría diaria en horario de oficina: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Comentarios + exportaciones de encuestas: carpetas específicas del revisor رکھیں
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` کے تحت
- Verificación de conocimientos CSV y resumen: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Problema con el rastreador de inventario کے ساتھ sincronización رکھیں۔ جب artefactos کو ticket de gobernanza میں کاپی کریں تو hashes adjuntos کریں
تاکہ auditores فائلیں بغیر acceso al shell کے verificar کر سکیں۔