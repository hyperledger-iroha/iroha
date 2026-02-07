---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوت ٹریکر

یہ ٹریکر docs پورٹل کی ہر پریویو ویو ریکارڈ کرتا ہے تاکہ DOCS-SORA کے مالکان اور revisores de gobernanza دیکھ سکیں کہ کون سی cohorte فعال ہے، کس نے دعوتیں منظور کیں، اور کون سے artefactos ابھی توجہ چاہتے ہیں۔ جب بھی دعوتیں بھیجی جائیں، واپس لی جائیں یا مؤخر ہوں تو اسے اپ ڈیٹ کریں تاکہ pista de auditoría ریپوزٹری کے اندر رہے۔

## ویو اسٹیٹس| ویو | cohorte | ٹریکر ایشو | aprobador(es) | اسٹیٹس | ہدف ونڈو | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores de Docs + SDK, suma de comprobación y validación de archivos | `DOCS-SORA-Preview-W0` (GitHub/rastreador de operaciones) | Líder de Docs/DevRel + Portal TL | مکمل | Segundo trimestre de 2025 ہفتے 1-2 | دعوتیں 2025-03-25 کو بھیجی گئیں، ٹیلی میٹری سبز رہی، resumen de salida 2025-04-08 کو شائع ہوا۔ |
| **W1 - Socios** | SoraFS Integradores Torii NDA | `DOCS-SORA-Preview-W1` | Líder de Docs/DevRel + enlace de gobernanza | مکمل | Q2 2025 ہفتہ 3 | دعوتیں 2025-04-12 -> 2025-04-26، تمام آٹھ پارٹنرز کی تصدیق؛ evidencia [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) میں اور salida de resumen [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) میں۔ |
| **W2 - Comunidad** | Lista de espera de la comunidad de منتخب ( 2025-06-29، پوری مدت میں ٹیلی میٹری سبز؛ evidencia + hallazgos [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) میں۔ |
| **W3 - Cohortes Beta** | beta de finanzas/observabilidad + socio de SDK + defensor del ecosistema | `DOCS-SORA-Preview-W3` | Líder de Docs/DevRel + enlace de gobernanza | مکمل | Primer trimestre de 2026 ہفتہ 8 | دعوتیں 2026-02-18 -> 2026-02-28؛ resumen + datos del portal `preview-20260218` ویو سے تیار (دیکھیں [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> Nombre: ہر ٹریکر ایشو کو متعلقہ solicitud de vista previa de tickets سے لنک کریں اور انہیں `docs-portal-preview` پروجیکٹ میں archive تاکہ aprobaciones قابلِ دریافت رہیں۔

## فعال کام (W0)

- artefactos de verificación previa ریفریش (GitHub Actions `docs-portal-preview` رن 2025-03-24, descriptor `scripts/preview_verify.sh` کے ذریعے `preview-2025-03-24` ٹیگ سے verificar)۔
- ٹیلی میٹری líneas base محفوظ (`docs.preview.integrity`, `TryItProxyErrors` instantánea del panel W0 problema میں محفوظ)۔
- alcance متن [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے ساتھ lock, etiqueta de vista previa `preview-2025-03-24`۔
- پہلے پانچ mantenedores کے لئے solicitudes de admisión لاگ (tickets `DOCS-SORA-Preview-REQ-01`... `-05`).
- پہلی پانچ دعوتیں 2025-03-25 10:00-10:20 UTC کو بھیجیں، سات دن مسلسل سبز ٹیلی میٹری کے بعد؛ agradecimientos `DOCS-SORA-Preview-W0` میں محفوظ۔
- ٹیلی میٹری مانیٹرنگ + horario de oficina del anfitrión (2025-03-31 تک روزانہ registros; registro de puntos de control نیچے)۔
- comentarios/problemas de punto medio اکٹھے کر کے `docs-preview/w0` ٹیگ (دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- ویو resumen شائع + confirmaciones de salida de invitación (paquete de salida تاریخ 2025-04-08; دیکھیں [resumen W0] (./preview-feedback/w0/summary.md)) ۔
- Onda beta W3 ٹریک; مستقبل کی ویوز revisión de la gobernanza کے بعد شیڈول۔

## Socios de W1 y otros- قانونی اور aprobaciones de gobernanza۔ Anexo de socios 2025-04-05 کو سائن؛ Homologaciones `DOCS-SORA-Preview-W1` میں اپ لوڈ۔
- Telemetría + Pruébalo en escena۔ Cambiar ticket `OPS-TRYIT-147` 2025-04-06 کو اجرا؛ `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` y Grafana instantáneas de color
- Artefacto + preparación de suma de comprobación ۔ Verificación del paquete `preview-2025-04-12`؛ descriptor/suma de comprobación/registros de sonda `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ۔
- Invitar a la lista + despacho۔ آٹھ solicitudes de socios (`DOCS-SORA-Preview-REQ-P01...P08`) منظور؛ دعوتیں 2025-04-12 15:00-15:21 UTC کو بھیجیں، ہر revisor کا ack ریکارڈ۔
- Instrumentación de retroalimentación۔ روزانہ horario de oficina + puntos de control de telemetría ریکارڈ؛ digerir کے لئے [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) دیکھیں۔
- Lista final/registro de salida ۔ [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) اب invitar/ack marcas de tiempo, evidencia de telemetría, exportaciones de cuestionarios, اور punteros de artefactos 2025-04-26 تک ریکارڈ کرتا ہے تاکہ ola de gobernanza کو reproducir کر سکے۔

## Registro de invitaciones: mantenedores principales de W0| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Activo | verificación de suma de comprobación کی تصدیق؛ revisión de navegación/barra lateral پر توجہ۔ |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Activo | Recetas SDK + inicios rápidos Norito ٹیسٹ کر رہا ہے۔ |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Activo | Pruébelo consola + flujos ISO ویلیڈیٹ۔ |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Activo | SoraFS runbooks + documentos de orquestación آڈٹ۔ |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Activo | apéndices de telemetría/incidentes کا جائزہ؛ Cobertura de Alertmanager کے ذمہ دار۔ |

تمام دعوتیں ایک ہی `docs-portal-preview` artefacto (ejecutar 2025-03-24, etiqueta `preview-2025-03-24`) اور `DOCS-SORA-Preview-W0` میں محفوظ transcripción de verificación کو ریفرنس کرتی ہیں۔ Problema con el rastreador دونوں میں لاگ کریں۔

## Registro de puntos de control - W0| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-03-26 | Revisión de línea base de telemetría + horario de oficina | `docs.preview.integrity` + `TryItProxyErrors` Negro Rojo horario de oficina نے verificación de suma de comprobación مکمل ہونے کی تصدیق کی۔ |
| 2025-03-27 | Se publicó el resumen de comentarios del punto medio | Resumen [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) میں محفوظ؛ دو چھوٹے problemas de navegación `docs-preview/w0` کے تحت، کوئی incidente نہیں۔ |
| 2025-03-31 | Verificación puntual de telemetría de la última semana | آخری horario de oficina؛ revisores نے باقی کام تصدیق کیا، کوئی alerta نہیں۔ |
| 2025-04-08 | Resumen de salida + cierres de invitaciones | revisiones مکمل، عارضی رسائی منسوخ، hallazgos [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) میں آرکائیو؛ Rastreador W1 سے پہلے اپ ڈیٹ۔ |

## Registro de invitaciones: socios de W1| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completado | Comentarios de operaciones del orquestador 2025-04-20 کو دیا؛ salir de confirmación 15:05 UTC۔ |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completado | comentarios de lanzamiento `docs-preview/w1` میں لاگ؛ salir de confirmación 15:10 UTC۔ |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completado | ediciones de disputas/listas negras فائل؛ salir de confirmación 15:12 UTC۔ |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completado | Pruébelo tutorial de autenticación قبول؛ salir de confirmación 15:14 UTC۔ |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completado | Comentarios de RPC/OAuth salir de confirmación 15:16 UTC۔ |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completado | vista previa de la combinación de comentarios de integridad؛ salir de confirmación 15:18 UTC۔ |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completado | revisión de telemetría/redacción مکمل؛ salir de confirmación 15:22 UTC۔ || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completado | Comentarios del runbook DNS de puerta de enlace salir de confirmación 15:24 UTC۔ |

## Registro de puntos de control - W1

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-04-12 | Envío de invitaciones + verificación de artefactos | `preview-2025-04-12` descriptor/archivo کے ساتھ آٹھ partners کو ای میل؛ rastreador de agradecimientos میں محفوظ۔ |
| 2025-04-13 | Revisión básica de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` سبز؛ horario de oficina نے verificación de suma de comprobación مکمل ہونے کی تصدیق کی۔ |
| 2025-04-18 | Horario de oficina de media ola | `docs.preview.integrity` Rojo دو doc نٹس `docs-preview/w1` کے تحت لاگ (redacción de navegación + captura de pantalla Pruébelo) ۔ |
| 2025-04-22 | Verificación puntual final de telemetría | proxy + paneles de control کوئی نئی problemas نہیں، salir سے پہلے rastreador میں نوٹ۔ |
| 2025-04-26 | Resumen de salida + cierres de invitaciones | تمام socios نے revisión completa کی تصدیق کی، invita a revocar, evidencia [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) میں آرکائیو۔ |

## Resumen de la cohorte beta del W3

- 2026-02-18 کو invita a بھیجی گئیں، verificación de suma de verificación + reconocimientos اسی دن لاگ۔
- comentarios `docs-preview/20260218` میں جمع، problema de gobernanza `DOCS-SORA-Preview-20260218`؛ resumen + resumen `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` سے تیار۔
- 2026-02-28 کو verificación final de telemetría کے بعد revocación de acceso؛ rastreador + tablas de portal اپ ڈیٹ کر کے W3 مکمل دکھایا۔

## Registro de invitaciones: comunidad W2| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completado | Confirmación a las 16:06 UTC; Inicio rápido del SDK پر فوکس؛ salir 2025-06-29 کو کنفرم۔ |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completado | revisión de gobernanza/SNS salir 2025-06-29 کو کنفرم۔ |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completado | Norito comentarios sobre el tutorial salir reconfirmar 2025-06-29۔ |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completado | Revisión del runbook SoraFS salir reconfirmar 2025-06-29۔ |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completado | Notas de accesibilidad/UX شیئر؛ salir reconfirmar 2025-06-29۔ |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completado | Comentarios sobre localización لاگ؛ salir reconfirmar 2025-06-29۔ |
| comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completado | Verificaciones de documentos del SDK móvil salir reconfirmar 2025-06-29۔ || comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completado | Revisión del apéndice de observabilidad مکمل؛ salir reconfirmar 2025-06-29۔ |

## Registro de puntos de control - W2

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-06-15 | Envío de invitaciones + verificación de artefactos | `preview-2025-06-15` descriptor/archivo 8 revisores de la comunidad کے ساتھ شیئر؛ rastreador de agradecimientos میں محفوظ۔ |
| 2025-06-16 | Revisión básica de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` Cuadros de instrumentos Negro Pruébelo registros de proxy میں tokens comunitarios فعال۔ |
| 2025-06-18 | Horario de oficina y clasificación de problemas | دو sugerencias (redacción de información sobre herramientas `docs-preview/w2 #1`, barra lateral de localización `#2`) - دونوں Docs کو routed۔ |
| 2025-06-21 | Comprobación de telemetría + correcciones de documentos | Docs نے `docs-preview/w2 #1/#2` حل کیا؛ paneles de control سبز، کوئی incidente نہیں۔ |
| 2025-06-24 | Horario de oficina de la última semana | revisores نے باقی envíos de comentarios کنفرم کیے؛ کوئی alerta نہیں۔ |
| 2025-06-29 | Resumen de salida + cierres de invitaciones | acks ریکارڈ، revocación de acceso a vista previa, instantáneas de telemetría + artefactos آرکائیو (دیکھیں [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horario de oficina y clasificación de problemas | دو sugerencias de documentación `docs-preview/w1` کے تحت لاگ؛ کوئی incidentes یا alertas نہیں۔ |

## Ganchos de informes- ہر بدھ، اوپر والی tabla اور problema de invitación activa کو مختصر nota de estado سے اپ ڈیٹ کریں (invitaciones enviadas, revisores activos, incidentes).
- جب کوئی ویو بند ہو، ruta de resumen de comentarios شامل کریں (مثال: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) اور اسے `status.md` سے لنک کریں۔
- اگر [vista previa del flujo de invitación](./preview-invite-flow.md) کے criterios de pausa ٹرگر ہوں تو invitaciones دوبارہ شروع کرنے سے پہلے pasos de remediación یہاں شامل کریں۔