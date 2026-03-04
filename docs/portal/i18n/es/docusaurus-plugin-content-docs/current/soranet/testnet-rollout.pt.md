---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Lanzamiento de testnet de SoraNet (SNNet-10)
sidebar_label: Lanzamiento de testnet (SNNet-10)
descripción: Plano de activación por fases, kit de onboarding y puertas de telemetría para promociones de testnet do SoraNet.
---

:::nota Fuente canónica
Esta página describe el plano de implementación de SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Mantenha ambas como copias sincronizadas.
:::

SNNet-10 coordina la activación en las etapas de superposición de anonimato de SoraNet en toda la red. Utilice este plano para traducir las viñetas de la hoja de ruta en entregables concretos, runbooks y puertas de telemetría para que cada operador entienda las expectativas antes de que SoraNet virar o transportar padrao.

## Fases de lancamento| Fase | Cronograma (alvo) | Escopo | Artefatos obrigatorios |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet fechado** | Cuarto trimestre de 2026 | 20-50 relevos en >=3 ASNs operados por contribuyentes core. | Kit de incorporación de Testnet, conjunto de fijación de guardia de humo, línea base de latencia + métricas de PoW, registro de simulacro de apagón. |
| **T1 - Beta pública** | Primer trimestre de 2027 | >=100 relés, rotación de guardia habilitada, vinculación de salida aplicada, SDK betas padrao em SoraNet com `anon-guard-pq`. | Kit de incorporación actualizado, lista de verificación de operadores, SOP de publicación de directorio, paquete de paneles de telemetría, relatorios de ensayo de incidentes. |
| **T2 - Padrón Mainnet** | Q2 2027 (condicionado a completar SNNet-6/7/9) | Red de producción padrao em SoraNet; transporta obfs/MASQUE e cumplimiento de PQ ratchet habilitados. | Minutas de aprobación de gobernanza, procedimiento de reversión directa únicamente, alarmas de degradación, relatorio assinado de métricas de éxito. |

Nao ha **caminho de salto** - cada fase deve entregar a telemetria e os artefactos de gobierno de la etapa anterior antes de la promoción.

## Kit de incorporación a testnet

Cada operador de retransmisión recibe un paquete determinístico con los siguientes archivos:| Artefacto | Descripción |
|----------|-------------|
| `01-readme.md` | Resumen, puntos de contacto y cronograma. |
| `02-checklist.md` | Lista de verificación previa al vuelo (hardware, accesibilidad de la red, política de verificación de seguridad). |
| `03-config-example.toml` | Configuración mínima de retransmisión + orquestador SoraNet incluido en los bloques de cumplimiento de SNNet-9, incluido un bloque `guard_directory` que fija el hash de la última instantánea de protección. |
| `04-telemetry.md` | Instrucciones para ligar los paneles de métricas de privacidad de SoraNet y los umbrales de alerta. |
| `05-incident-playbook.md` | Procedimiento de respuesta a una caída de tensión/baja de categoría con matriz de escalamiento. |
| `06-verification-report.md` | Plantilla que los operadores completan y devuelven cuando pasan las pruebas de humo. |

Una copia renderizada vive en `docs/examples/soranet_testnet_operator_kit/`. Cada promoción actualiza el kit; numeros de versao acompanham a fase (por ejemplo, `testnet-kit-vT0.1`).

Para operadores de beta publica (T1), o brief conciso em `docs/source/soranet/snnet10_beta_onboarding.md` resume prerequisitos, entregaveis de telemetria e o fluxo de envio, apontando para o kit deterministico y para os helpers de validacao.

`cargo xtask soranet-testnet-feed` genera el feed JSON que agrega a janela de promoción, roster de relevos, relatorio de métricas, evidencias de ejercicios y hashes de anexos referenciados en la plantilla de stage-gate. Assine os Drill Logs and Anexos con `cargo xtask soranet-testnet-drill-bundle` primero para que el registro de alimentación `drill_log.signed = true`.

## Métricas de éxitoUna promoción entre fases fica cerrada en la siguiente telemetría, coletada por no mínimo dos semanas:

- `soranet_privacy_circuit_events_total`: 95% de los circuitos completos sin eventos de apagón o degradación; os 5% restantes ficam limitados pela oferta de PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% dentro de janela alvo de 3 s; reportada vía `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (percentil 95) por regiao: <200 ms quando circuitos estiverem totalmente construidos, capturada vía `soranet_privacy_rtt_millis{percentile="p95"}`.

Plantillas de paneles de control y alertas vivas en `dashboard_templates/` e `alert_templates/`; Espelhe-os no su repositorio de telemetría y adicione-os a checks de lint do CI. Use `cargo xtask soranet-testnet-metrics` para generar o relatorio voltado a gobierno antes de solicitar a promoción.

Los envíos de stage-gate deben seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que aponta al formulario Markdown pronto para copiar en `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificación de verificación

Los operadores deben informar o seguir antes de entrar en cada fase:- [x] Anuncio de retransmisión assinado com o sobre de admisión atual.
- [x] Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) aprobada.
- [x] `guard_directory` aponta para el artefato `GuardDirectorySnapshotV2` más reciente e `expected_directory_hash_hex` coinciden con el resumen del comité (o inicio de registro de retransmisión o hash validado).
- [x] Métricas de PQ ratchet (`sorafs_orchestrator_pq_ratio`) permanecem acima dos umbrales alvo para una fase solicitada.
- [x] Configuración de cumplimiento GAR correspondiente a la etiqueta más reciente (ver catálogo SNNet-9).
- [x] Simulacao de alarme de downgrade (desativar recolectores, esperar alerta em 5 min).
- [x] Drill PoW/DoS ejecutado con etapas de mitigación documentadas.

Um template pre-preenchido esta incluido no kit de onboarding. Los operadores submetem o relatorio completo al servicio de asistencia técnica de gobierno antes de recibir credenciales de producción.

## Gobernanza e informes

- **Control de cambios:** las promociones exigen la aprobación del Consejo de Gobierno registrada en las actas del consejo y anexada a la página de estado.
- **Resumen de estado:** publicación actualizada semanalmente resumindo contagios de relés, ratio PQ, incidentes de apagón y elementos de acción pendientes (armazenado em `docs/source/status/soranet_testnet_digest.md` quando a cadencia comecar).
- **Rollbacks:** mantenga un plano de rollback assinado que regrese a la red para una fase anterior en 30 minutos, incluida la invalidación de DNS/guard cache y plantillas de comunicación con clientes.

## Activos de apoyo- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa el kit de incorporación de `xtask/templates/soranet_testnet/` para el directorio alvo (predeterminado `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` dispone de métricas de éxito SNNet-10 y emite un informe de aprobación/rechazo estructurado para revisiones de gobernanza. Una instantánea de ejemplo vive en `docs/examples/soranet_testnet_metrics_sample.json`.
- Plantillas de Grafana y Alertmanager viven en `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; Copie-os no repositorio de telemetría o conecte-os aos checks de lint do CI.
- La plantilla de comunicación de degradación para mensajes de SDK/portal reside en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Los resúmenes de estado semanales deben usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canónica.

Las solicitudes de extracción deben actualizar esta página junto con cualquier cambio de artefatos o telemetría para que el plan de implementación se mantenga canónico.