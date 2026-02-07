---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Tracker de Invitaciones de Preview

Este tracker registra cada ola de preview del portal de docs para que los owners de DOCS-SORA y los revisores de gobernanza vean que cohorte esta active, quien aprobo las invitaciones y que artefactos suen pendientes. Actualizalo cada vez que se envien, revoquen o diifieran invitaciones para que el rastro de auditoria quede dentro del repositorio.

## Estado de olas

| אולה | קוהורט | גיליון דה סגווימיינטו | אפרודור(ים) | Estado | Ventana objetivo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - מתחזקי ליבה** | Maintainers de Docs + SDK validando el flujo de checksum | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Lead Docs/DevRel + Portal TL | Completado | Q2 2025 סמנים 1-2 | Invitaciones enviadas 2025-03-25, telemetria se mantuvo verde, resumen de salida publicado 2025-04-08. |
| **W1 - שותפים** | Operadores SoraFS, אינטגרדורים Torii bajo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + enlace de gobernanza | Completado | Q2 2025 Semana 3 | Invitaciones 2025-04-12 -> 2025-04-26 con los ocho partners confirmados; evidencia capturada en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y el resumen de salida en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - קומונידד** | Lista de espera comunitaria curada (<=25 a la vez) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + מנהל קהילה | Completado | Q3 2025 Semana 1 (tentativo) | Invitaciones 2025-06-15 -> 2025-06-29 con telemetria verde todo el periodo; evidencia + hallazgos capturados en [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes בטא** | Beta finanzas/observabilidad + שותף SDK + Defensor del ecosistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + enlace de gobernanza | Completado | Q1 2026 סמנה 8 | הזמנות 2026-02-18 -> 2026-02-28; קורות חיים + נתונים של פורטל generados דרך ola `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> הערה: Enlaza Cada issue del Tracker con los tickets de solicitud de preview y archivalas bajo el proyecto `docs-portal-preview` para que las aprobaciones sigan siendo descubribles.

## Tareas actives (W0)- Artefactos de preflight actualizados (הוצאת GitHub Actions `docs-portal-preview` 2025-03-24, אימות מתאר באמצעות `scripts/preview_verify.sh` usando tag `preview-2025-03-24`).
- קווי בסיס של טלמטריה (`docs.preview.integrity`, תמונת מצב של לוחות מחוונים `TryItProxyErrors` שומרים על נושא W0).
- Texto de outreach bloqueado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) תצוגה מקדימה של תג `preview-2025-03-24`.
- Solicitudes de ingreso registradas para los primeros cinco maintenanceers (כרטיסים `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Primeras cinco invitaciones enviadas 2025-03-25 10:00-10:20 UTC despues de siete dias consecutivos de telemetria verde; acuses guardados en `DOCS-SORA-Preview-W0`.
- Monitoreo de telemetria + שעות המשרד של המארח (יומני צ'ק-אין מהירות 2025-03-31; log de checkpoints abajo).
- משוב de mitad de ola / issues recopilados y etiquetados `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen de ola publicado + confirmaciones de salida de invitaciones (חבילה de salida fechado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Ola Beta W3 seguida; Futuras Olas Programadas Segun Revision de Gobernanza.

## קורות חיים של שותפים W1

- Aprobaciones legales y de gobernanza. תוספת של שותפים 2025-04-05; aprobaciones subidas a `DOCS-SORA-Preview-W1`.
- Telemetria + נסה את זה בימוי. Ticket de cambio `OPS-TRYIT-147` ejecutado 2025-04-06 עם צילומי מצב Grafana de `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` arch.
- Preparacion de artefacto + checksum. חבילה `preview-2025-04-12` מאומת; יומנים מתאר/checksum/probe guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster de invitaciones + envio. Ocho solicitudes de partners (`DOCS-SORA-Preview-REQ-P01...P08`) aprobadas; invitaciones enviadas 2025-04-12 15:00-15:21 UTC cones acuses registrados por revisor.
- Instrumentacion de feedback. יומני שעות המשרד + נקודות ביקורת של טלמטריה registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para el digest.
- סגל סגל / log de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) ahora registra timestamps de invitacion/ack, evidencia de telemetria, exports de quiz y punteros de artefactos al 2025-04-26 para que gobernanza la pueda.

## Log de invitaciones - מתחזקי הליבה של W0| ID de revisor | רול | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | מתחזק פורטל | `DOCS-SORA-Preview-REQ-01` | 25-03-2025 10:05 | 2025-04-08 10:00 | Activo | Confirmo Verificacion de Checksum; enfocado en revision de nav/sidebar. |
| sdk-rust-01 | עופרת SDK חלודה | `DOCS-SORA-Preview-REQ-02` | 25-03-2025 10:08 | 2025-04-08 10:00 | Activo | Probando recetas de SDK + התחלה מהירה של Norito. |
| sdk-js-01 | מתחזק JS SDK | `DOCS-SORA-Preview-REQ-03` | 25-03-2025 10:12 | 2025-04-08 10:00 | Activo | Validando consola נסה את זה + flujos ISO. |
| sorafs-ops-01 | SoraFS קשר מפעיל | `DOCS-SORA-Preview-REQ-04` | 25-03-2025 10:15 | 2025-04-08 10:00 | Activo | Auditando runbooks de SoraFS + docs de orquestacion. |
| observability-01 | צפיות TL | `DOCS-SORA-Preview-REQ-05` | 25-03-2025 10:18 | 2025-04-08 10:00 | Activo | Revisando apendices de telemetria/incidentes; אחראי דה cobertura de Alertmanager. |

Todas las invitaciones referencian el mismo artefacto `docs-portal-preview` (הוצאת 2025-03-24, תג `preview-2025-03-24`) y el registro de verificacion capturado en `DOCS-SORA-Preview-W0`. Cualquier alta/pausa debe registrarse tanto en la tabla anterior como en la issue del tracker antes de proceder a la suuiente ola.

## יומן מחסומים - W0

| Fecha (UTC) | אקטיבידד | Notas |
| --- | --- | --- |
| 26-03-2025 | Revision de telemetria baseline + שעות עבודה | `docs.preview.integrity` + `TryItProxyErrors` se mantuvieron verdes; שעות המשרד confirmaron que todos los revisores completaron la verificacion de checksum. |
| 27-03-2025 | Digest de feedback intermedio publicado | Resumen capturado en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dos issues menores de nav registradas como `docs-preview/w0`, sin incidentes reportados. |
| 31-03-2025 | Chequeo de telemetria de la ultima semana | שעות המשרד של Ultimas לפני יציאה; revisores confirmaron tareas restantes en curso, sin alertas. |
| 2025-04-08 | Resumen de salida + cierres de invitaciones | ביקורות קומפלטאדאס אישור, acceso temporal revocado, hallazgos archivados en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker actualizado antes de preparar W1. |

## Log de invitaciones - שותפים W1| ID de revisor | רול | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| soraps-op-01 | מפעיל SoraFS (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 26/04/2025 15:00 | Completado | משוב Entrego de ops del orquestador 2025-04-20; ack de salida 15:05 UTC. |
| soraps-op-02 | מפעיל SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12-04-2025 15:03 | 26/04/2025 15:00 | Completado | רישום תגובות להפצה ב-`docs-preview/w1`; ack 15:10 UTC. |
| soraps-op-03 | מפעיל SoraFS (ארה"ב) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 26/04/2025 15:00 | Completado | Ediciones de dispute/רשימה שחורה; ack 15:12 UTC. |
| torii-int-01 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P04` | 12-04-2025 15:09 | 26/04/2025 15:00 | Completado | Walkthrough de Try it auth aceptado; ack 15:14 UTC. |
| torii-int-02 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P05` | 12-04-2025 15:12 | 26/04/2025 15:00 | Completado | הערות לרישום RPC/OAuth; ack 15:16 UTC. |
| sdk-partner-01 | שותף SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12-04-2025 15:15 | 26/04/2025 15:00 | Completado | משוב de integridad de preview fusionado; ack 15:18 UTC. |
| sdk-partner-02 | שותף SDK (אנדרואיד) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 26/04/2025 15:00 | Completado | Revision de telemetria/redaction hecha; ack 15:22 UTC. |
| gateway-ops-01 | מפעיל שער | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 26/04/2025 15:00 | Completado | הערות ל-Runbook של רישום שער DNS; ack 15:24 UTC. |

## יומן מחסומים - W1

| Fecha (UTC) | אקטיבידד | Notas |
| --- | --- | --- |
| 2025-04-12 | Envio de invitaciones + verificacion de artefactos | Los ocho partners recibieron דוא"ל con descriptor/ארכיון `preview-2025-04-12`; אקוס registrados en el tracker. |
| 2025-04-13 | Revision de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` en verde; שעות המשרד confirmaron verificacion de checksum completada. |
| 2025-04-18 | שעות המשרד de mitad de ola | `docs.preview.integrity` se mantuvo verde; dos nits de docs registrados como `docs-preview/w1` (ניסוח ניווט + צילום מסך של נסה את זה). |
| 22-04-2025 | Chequeo final de telemetria | פרוקסי + לוחות מחוונים מומלצים; חטא בעיות נואבות, anotado en el tracker antes de salida. |
| 26-04-2025 | Resumen de salida + cierres de invitaciones | Todos los partners confirmaron revision, invitaciones revocadas, evidencia archivada en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap de Cohorte Beta W3

- Invitaciones enviadas 2026-02-18 con verificacion de checksum + acuses registrados el mismo dia.
- משוב recopilado bajo `docs-preview/20260218` con issue de gobernanza `DOCS-SORA-Preview-20260218`; digest + resumen generados דרך `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revocado 2026-02-28 despues del chequeo final de telemetria; גשש + טבלאות של הפורטל בפועל עבור מרקר W3 como completado.

## Log de invitaciones - קהילת W2| ID de revisor | רול | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | מבקר קהילה (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15-06-2025 16:00 | 29/06/2025 16:00 | Completado | אק 16:06 UTC; enfocado en quickstarts de SDK; salida confirmada 2025-06-29. |
| comm-vol-02 | מבקר קהילה (ממשל) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Completado | Revision de gobernanza/SNS hecha; salida confirmada 2025-06-29. |
| comm-vol-03 | מבקר קהילה (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Completado | משוב של הדרכה Norito רישום; ack 2025-06-29. |
| comm-vol-04 | מבקר קהילה (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Completado | Revision de runbooks SoraFS hecha; ack 2025-06-29. |
| comm-vol-05 | מבקר קהילה (נגישות) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Completado | Notas de accesibilidad/UX compartidas; ack 2025-06-29. |
| comm-vol-06 | מבקר קהילה (לוקליזציה) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Completado | משוב על registrado localizacion; ack 2025-06-29. |
| comm-vol-07 | מבקר קהילה (נייד) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Completado | בדיקות של מסמכים של SDK ניידים; ack 2025-06-29. |
| comm-vol-08 | מבקר קהילה (צפיות) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Completado | Revision de apendice de observabilidad hecha; ack 2025-06-29. |

## יומן מחסומים - W2

| Fecha (UTC) | אקטיבידד | Notas |
| --- | --- | --- |
| 2025-06-15 | Envio de invitaciones + verificacion de artefactos | תיאור/ארכיון `preview-2025-06-15` compartido con 8 revisores; מאשים guardados en tracker. |
| 2025-06-16 | Revision de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` en verde; logs del proxy נסה את זה אסימונים של muestran comunitarios activos. |
| 2025-06-18 | שעות משרד y triage de issues | Dos sugerencias (נוסח `docs-preview/w2 #1` de tooltip, `#2` sidebar de localizacion) - אסמכתא של Docs. |
| 21-06-2025 | Chequeo de telemetria + תיקוני מסמכים | Docs resolvio `docs-preview/w2 #1/#2`; לוחות מחוונים verdes, אירועי חטא. |
| 24-06-2025 | שעות המשרד de la ultima semana | Revisores confirmaron envios pendientes; אין אזהרות שונות. |
| 29-06-2025 | Resumen de salida + cierres de invitaciones | Acks registrados, acceso de preview revocado, צילומי מצב + ארכיון חפצים (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | שעות משרד y triage de issues | Dos sugerencias de documentacion registradas bajo `docs-preview/w1`; חטא מקרי התראה. |

## Hooks de reporte- Cada miercoles, actualiza la tabla del tracker y la issue de invitaciones active con una not corta de estado (invitaciones enviadas, revisores activos, incidentes).
- Cuando una ola cierre, agrega la ruta del resumen de feedback (por ejemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y enlazala desde `status.md`.
- Si se activen criterios de pausa de [זרימת הזמנת תצוגה מקדימה](./preview-invite-flow.md), agrega los pasos de remediacion aqui antes de reanudar invitaciones.