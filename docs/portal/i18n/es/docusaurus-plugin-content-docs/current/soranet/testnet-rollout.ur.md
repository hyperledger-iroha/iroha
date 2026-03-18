---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Lanzamiento de SoraNet testnet (SNNet-10)
sidebar_label: Lanzamiento de Testnet (SNNet-10)
descripción: Plan de activación y plan de activación, kit de incorporación, promociones de testnet de SoraNet y puertas de telemetría.
---

:::nota Fuente canónica
یہ صفحہ `docs/source/soranet/testnet_rollout_plan.md` میں Plan de implementación de SNNet-10 کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentos retirar نہ ہو، دونوں کاپیاں sincronización رکھیں۔
:::

SNNet-10 نیٹ ورک بھر میں SoraNet anonimato superposición کی مرحلہ وار activación کو coordenada کرتا ہے۔ اس plan کو استعمال کریں تاکہ roadmap bullet کو entregables concretos، runbooks، اور puertas de telemetría میں بدلا جا سکے تاکہ ہر operador توقعات سمجھ لے اس سے پہلے کہ Transporte predeterminado de SoraNet بنے۔

## Fases de lanzamiento| Fase | Línea de tiempo (objetivo) | Alcance | Artefactos requeridos |
|-------|-------------------|-------|--------------------|
| **T0 - Red de prueba cerrada** | Cuarto trimestre de 2026 | >=3 ASN پر 20-50 relés جو contribuyentes principales چلاتے ہیں۔ | Kit de incorporación de Testnet, conjunto de humo de fijación de protección, latencia de referencia + métricas de PoW, registro de perforación de apagones. |
| **T1 - Beta pública** | Primer trimestre de 2027 | >=100 relés, rotación de guardia habilitada, vinculación de salida aplicada, SDK betas predeterminado طور پر SoraNet کے ساتھ `anon-guard-pq`. | Kit de incorporación actualizado, lista de verificación de operador, directorio de publicación de SOP, paquete de panel de telemetría, informes de ensayo de incidentes. |
| **T2 - Valor predeterminado de la red principal** | Segundo trimestre de 2027 (finalización de SNNet-6/7/9 cerrada) | Red de producción SoraNet پر default؛ Transportes obfs/MASQUE y aplicación de trinquete PQ habilitada ۔ | Actas de aprobación de gobernanza, procedimiento de reversión solo directo, alarmas de degradación, informe de métricas de éxito firmado. |

**کوئی saltar ruta نہیں** - ہر fase کو پچھلے مرحلے کی telemetría اور artefactos de gobernanza barco کرنا لازمی ہے قبل از promoción۔

## Kit de incorporación de Testnet

ہر operador de retransmisión کو درج ذیل فائلوں کے ساتھ ایک paquete determinista ملتا ہے:| Artefacto | Descripción |
|----------|-------------|
| `01-readme.md` | Descripción general, puntos de contacto, cronograma۔ |
| `02-checklist.md` | Lista de verificación previa al vuelo (hardware, accesibilidad de la red, verificación de la política de protección). |
| `03-config-example.toml` | Bloques de cumplimiento SNNet-9 کے ساتھ align کیا ہوا retransmisión SoraNet mínima + configuración del orquestador, جس میں `guard_directory` block شامل ہے جو تازہ ترین guard snapshot hash کو pin کرتا ہے۔ |
| `04-telemetry.md` | Paneles de métricas de privacidad de SoraNet y umbrales de alerta cable کرنے کی ہدایات۔ |
| `05-incident-playbook.md` | Procedimiento de respuesta a caídas/bajas de categoría y matriz de escalamiento۔ |
| `06-verification-report.md` | Plantilla جسے operadores pruebas de humo پاس ہونے کے بعد مکمل کر کے واپس دیتے ہیں۔ |

Copia renderizada `docs/examples/soranet_testnet_operator_kit/` میں موجود ہے۔ ہر promoción میں actualización del kit ہوتا ہے؛ fase de números de versión کو sigue کرتے ہیں (مثال کے طور پر `testnet-kit-vT0.1`).

Operadores beta pública (T1) کے لئے `docs/source/soranet/snnet10_beta_onboarding.md` میں breves requisitos previos de incorporación concisos, entregables de telemetría اور flujo de trabajo de envío خلاصہ کرتا ہے اور kit determinista اور validador ayudantes کی طرف اشارہ کرتا ہے۔

`cargo xtask soranet-testnet-feed` Fuente JSON بناتا ہے جو ventana de promoción, lista de retransmisiones, informe de métricas, evidencia de perforación اور hashes adjuntos کو agregado کرتا ہے جنہیں referencia de plantilla de etapa-gate کرتا ہے۔ پہلے `cargo xtask soranet-testnet-drill-bundle` سے registros de perforación اور archivos adjuntos firmar کریں تاکہ feed `drill_log.signed = true` registro کر سکے۔

## Métricas de éxitoFases کے درمیان promoción درج ذیل telemetría پر cerrada ہے، جو کم از کم دو ہفتے جمع کی جاتی ہے:

- `soranet_privacy_circuit_events_total`: Caída del 95% de los circuitos یا degradación کے بغیر مکمل ہوں؛ باقی 5% de suministro PQ سے محدود ہوں۔
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: روزانہ sesiones de recuperación کا =99%; `soranet_privacy_throttles_total{scope="congestion"}` کے ذریعے informe ہوتا ہے۔
- Latencia (percentil 95) por región: circuitos مکمل ہونے کے بعد <200 ms، `soranet_privacy_rtt_millis{percentile="p95"}` کے ذریعے captura ہوتی ہے۔

Panel de control y plantillas de alerta `dashboard_templates/` y `alert_templates/` میں موجود ہیں؛ انہیں اپنے repositorio de telemetría میں mirror کریں اور CI comprobaciones de pelusa میں شامل کریں۔ Promoción کی درخواست سے پہلے informe de gobernanza بنانے کے لئے `cargo xtask soranet-testnet-metrics` استعمال کریں۔

Envíos de etapa کو `docs/source/soranet/snnet10_stage_gate_template.md` فالو کرنا ہوگا، جو `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` میں موجود Formulario Markdown listo para copiar کی طرف لنک کرتا ہے۔

## Lista de verificación de verificación

ہر fase میں داخل ہونے سے پہلے operadores کو درج ذیل پر aprobación کرنا ہوگا:- ✅ Anuncio de retransmisión موجودہ sobre de admisión کے ساتھ firmado ہو۔
- ✅ Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) پاس ہو۔
- ✅ `guard_directory` تازہ ترین `GuardDirectorySnapshotV2` artefacto کی طرف اشارہ کرے اور `expected_directory_hash_hex` resumen del comité سے coincidencia ہو (registro de hash validado de inicio de retransmisión کرتا ہے)۔
- ✅ Métricas de trinquete PQ (`sorafs_orchestrator_pq_ratio`) مطلوبہ etapa کے umbrales objetivo سے اوپر رہیں۔
- ✅ Configuración de cumplimiento de GAR تازہ ترین etiqueta سے coincidencia کرے (catálogo SNNet-9 دیکھیں)۔
- ✅ Simulación de alarma de degradación (los recopiladores desactivan la alerta de کریں، 5 منٹ میں esperando کریں)۔
- ✅ Pasos de mitigación documentados de perforación PoW/DoS کے ساتھ ejecutar ہو۔

ایک kit de incorporación de plantillas precargadas میں شامل ہے۔ Operadores مکمل رپورٹ servicio de asistencia técnica de gobernanza کو enviar کرتے ہیں، پھر credenciales de producción ملتے ہیں۔

## Gobernanza y presentación de informes

- **Control de cambios:** promociones کے لئے Aprobación del Consejo de Gobierno درکار ہے جو actas del consejo میں درج ہو اور página de estado کے ساتھ adjuntar ہو۔
- **Resumen de estado:** ہفتہ وار actualizaciones شائع کریں جن میں relé recuento, PQ ratio, incidentes de apagón اور elementos de acción pendientes شامل ہوں (cadencia شروع ہونے کے بعد `docs/source/status/soranet_testnet_digest.md` میں محفوظ)۔
- **Reversiones:** ایک plan de reversión firmado برقرار رکھیں جو 30 منٹ میں نیٹ ورک کو پچھلے fase پر واپس لے جائے، جس میں Invalidación de DNS/guard cache en plantillas de comunicación del cliente شامل ہوں۔

## Activos de apoyo- `cargo xtask soranet-testnet-kit [--out <dir>]` `xtask/templates/soranet_testnet/` Kit de incorporación کو directorio de destino میں materializar کرتا ہے (predeterminado `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` Las métricas de éxito de SNNet-10 evalúan کرتا ہے اور revisiones de gobernanza کے لئے informe estructurado de aprobación/rechazo emitir کرتا ہے۔ Instantánea de muestra `docs/examples/soranet_testnet_metrics_sample.json` میں موجود ہے۔
- Grafana اور Plantillas de Alertmanager `dashboard_templates/soranet_testnet_overview.json` اور `alert_templates/soranet_testnet_rules.yml` میں موجود ہیں؛ انہیں repositorio de telemetría میں copia کریں یا comprobaciones de pelusa CI میں cable کریں۔
- SDK/mensajería del portal کے لئے plantilla de comunicación de degradación `docs/source/soranet/templates/downgrade_communication_template.md` میں ہے۔
- Resúmenes de estado semanales en forma canónica کے طور پر `docs/source/status/soranet_testnet_weekly_digest.md` استعمال کرنا چاہئے۔

Solicitudes de extracción کو اس صفحے کو artefactos یا telemetría میں کسی بھی تبدیلی کے ساتھ actualización کرنا چاہئے تاکہ plan de implementación canónico رہے۔