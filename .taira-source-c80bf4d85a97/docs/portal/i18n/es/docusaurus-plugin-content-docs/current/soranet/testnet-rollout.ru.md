---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Роллаут testnet SoraNet (SNNet-10)
sidebar_label: Prueba de red (SNNet-10)
descripción: Activaciones de planes portátiles, kit de incorporación y puertas de telemetría para la red de prueba SoraNet.
---

:::nota Канонический источник
Esta página está diseñada para implementar el plan SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Asegúrese de copiar las copias sincronizadas, ya que los documentos disponibles no contienen información exclusiva.
:::

SNNet-10 координирует поэтапную активацию superposición de anonimato SoraNet по всей сети. Implemente este plan, qué viñetas de la hoja de ruta se encuentran en los entregables concretos, runbooks y puertas de telemetría, qué operadores de cada operador están asociados con esto, como SoraNet. станет транспортом по умолчанию.

## Фазы запуска| Faza | Tamil (цель) | Объем | Objetos obsoletos |
|-------|-------------------|-------|--------------------|
| **T0 - Red de prueba cerrada** | Cuarto trimestre de 2026 | 20-50 relés en >=3 ASN, contribuyentes principales. | Kit de incorporación de Testnet, conjunto de humo de fijación de protección, latencia de referencia + métricas de PoW, registro de perforación de apagones. |
| **T1 - Beta pública** | Primer trimestre de 2027 | >=100 relés, rotación de protección activada, vinculación de salida instalada, SDK betas по умолчанию используют SoraNet с `anon-guard-pq`. | Kit de incorporación completo, lista de verificación de operador, SOP de publicación de directorio, paquete de panel de telemetría, informes de ensayo de incidentes. |
| **T2 - Valor predeterminado de la red principal** | Q2 2027 (с гейтом на завершение SNNet-6/7/9) | Producción сеть по умолчанию SoraNet; включены obfs/MASQUE transportes y aplicación de trinquete PQ. | Gobernanza de aprobación de protocolos, procedimiento de reversión solo directo, alarmas de degradación, informe de métricas de éxito completas. |

**Пути пропуска нет** - каждая фаза обязана доставить telemetría y artefactos de gobernanza antes de los estadios.

## Kit de incorporación de Testnet

El operador de retransmisión de CAждый debe utilizar paquetes determinados con las siguientes archivos:| Artefacto | Descripción |
|----------|-------------|
| `01-readme.md` | Обзор, контакты и таймлайн. |
| `02-checklist.md` | Lista de verificación previa al vuelo (hardware, accesibilidad de la red, verificación de la política de guardia). |
| `03-config-example.toml` | Configuración mínima de relé + orquestador SoraNet, compatible con bloques de cumplimiento SNNet-9, bloque integrado `guard_directory`, pin-it hash basado en instantánea de protección. |
| `04-telemetry.md` | Instrucciones para los paneles de métricas de privacidad y umbrales de alerta de SoraNet. |
| `05-incident-playbook.md` | Процедура реакции на brownout/downgrade с матрицей эскалации. |
| `06-verification-report.md` | Shablon, который оператор заполняет и возвращает после прохождения pruebas de humo. |

La copia adicional se envía en `docs/examples/soranet_testnet_operator_kit/`. Каждое повышение обновляет kit; номера версий следуют фазе (por ejemplo, `testnet-kit-vT0.1`).

El resumen de incorporación de los operadores de la beta pública (T1) en `docs/source/soranet/snnet10_beta_onboarding.md` resume los requisitos previos, los entregables de telemetría y los flujos de trabajo, el kit de determinación y los asistentes de validación.

`cargo xtask soranet-testnet-feed` genera feed JSON, ventana de promoción agregada, lista de retransmisiones, informe de métricas, evidencia de perforación y hashes adjuntos, plantilla de puerta de escenario asociada. Puede colocar registros de perforación y accesorios en el `cargo xtask soranet-testnet-drill-bundle` y en el dispositivo de alimentación `drill_log.signed = true`.

## Uso métricoUna vez que se utilizan datos de telemetría, se deben realizar dos operaciones:

- `soranet_privacy_circuit_events_total`: 95% de los circuitos se salvan de una caída de tensión o de una degradación; оставшиеся 5% ограничены suministro PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% durante 3 s; informes de `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (percentil 95) por región: <200 ms después de los circuitos posteriores, ficticios según `soranet_privacy_rtt_millis{percentile="p95"}`.

Panel de control y plantillas de alerta disponibles en `dashboard_templates/` y `alert_templates/`; отзеркальте их вашем repositorio de telemetría y добавьте в CI lint checks. Utilice `cargo xtask soranet-testnet-metrics` para generar una orientación de gobernanza que se realiza antes de que se produzca un error.

Los envíos de etapa de entrada deben enviarse a `docs/source/soranet/snnet10_stage_gate_template.md`, que se encuentra en el formulario de Markdown en `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificación de verificación

Los operadores que pueden realizar la operación según la situación en la que se encuentren:- ✅ Anuncio de retransmisión подписан текущим sobre de admisión.
- ✅ Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) проходит.
- ✅ `guard_directory` utiliza el artefacto `GuardDirectorySnapshotV2` y `expected_directory_hash_hex` combinado con el resumen del comité (antes de iniciar el registro de retransmisión con hash).
- ✅ Las métricas de trinquete PQ (`sorafs_orchestrator_pq_ratio`) están diseñadas para adaptarse a estaciones de trabajo.
- ✅ La configuración de cumplimiento de GAR se realiza mediante etiqueta posterior (см. каталог SNNet-9).
- ✅ Alarma de degradación de simulación (recolectores de alerta, alerta de 5 minutos).
- ✅ Mitigación de perforación PoW/DoS mejorada con documentos.

Предзаполненный шаблон включен в onboarding kit. Los operadores realizan tareas de mantenimiento en el servicio de asistencia técnica de gobernanza antes de las credenciales de producción.

## Gobernanza y отчетность

- **Control de cambios:** promociones mediante la aprobación del Consejo de Gobierno, la actualización de las actas del consejo y la página de estado de la página de estado.
- **Resumen de estado:** publique todas las novedades sobre relés de colisión, relación PQ, apagones individuales y elementos de acción adicionales (escrito en `docs/source/status/soranet_testnet_digest.md` después inicio de sesión).
- **Reversiones:** puede implementar un plan de reversión, configurarlo antes de 30 minutos, invalidar DNS/guard cache y plantillas de comunicación con el cliente.

## Activos de apoyo- `cargo xtask soranet-testnet-kit [--out <dir>]` kit de incorporación de materiales de `xtask/templates/soranet_testnet/` en el catálogo de teléfonos (con el mismo `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` ofrece métricas de éxito de SNNet-10 y evalúa la estructura de aprobación/rechazo de las revisiones de gobernanza. La instantánea principal se encuentra en `docs/examples/soranet_testnet_metrics_sample.json`.
- Plantillas Grafana y Alertmanager integradas en `dashboard_templates/soranet_testnet_overview.json` y `alert_templates/soranet_testnet_rules.yml`; Explore el repositorio de telemetría o realice comprobaciones de pelusa de CI.
- La comunicación de degradación del SDK/mensajería del portal se realizó en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Nuevos resúmenes de estado utilizan el formulario `docs/source/status/soranet_testnet_weekly_digest.md` para el formulario canónico.

Las solicitudes de extracción permiten modificar esta página con múltiples artefactos y telemetría y un plan de implementación estándar.