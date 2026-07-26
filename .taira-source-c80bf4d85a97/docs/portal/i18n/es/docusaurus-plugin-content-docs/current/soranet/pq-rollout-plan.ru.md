---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: Плейбук постквантового lanzamiento SNNet-16G
sidebar_label: PQ de implementación del plan
descripción: Dispositivo operativo del producto híbrido X25519+ML-KEM handshake SoraNet de Canary de forma predeterminada en relés, clientes y SDK.
---

:::nota Канонический источник
:::

SNNet-16G lanzó el lanzamiento post-cuántico para el transporte de SoraNet. Perillas `rollout_phase` permiten que el operador coordine la promoción de la tecnología Requisito de guardia de la Etapa A para la cobertura mayoritaria de la Etapa B y la postura PQ estricta de la Etapa C sin redactar el contenido JSON/TOML para archivos de datos.

Este libro de jugadas está escrito:

- Configuración de fases y nuevas perillas de configuración (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) en código base (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Marcas de SDK y CLI de mapeo, implementación de cliente de чтобы каждый.
- Ожидания programación canaria para relé/cliente y paneles de gobierno, promoción de puerta de enlace (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de retroceso y accesorios para el runbook de simulacro de incendio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carta Faz| `rollout_phase` | Эффективная etapa de anonimato | Эффект по умолчанию | Aplicación típica |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | Требовать как minимум один PQ guard на circuito, пока флот прогревается. | Baseline y ранние недели canario. |
| `ramp` | `anon-majority-pq` (Etapa B) | Смещать выбор в сторону Relés PQ для >= двух третей cobertura; классические relés остаются reserva. | Canarias de relevo regional; La vista previa del SDK alterna. |
| `default` | `anon-strict-pq` (Etapa C) | Pruebe circuitos exclusivos de PQ y utilice alarmas de degradación. | Promoción final después de la gestión de telemetría y aprobación. |

Si el dispositivo está encendido `anonymity_policy`, en la fase periférica de este componente. La fase de inicio de sesión se retrasa cuando se instala `rollout_phase`, los operadores pueden modificar la fase según la información y los datos de los clientes. унаследовать его.

## Configuraciones de referencia

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Loader Orchestrator crea una etapa de respaldo en varias versiones (`crates/sorafs_orchestrator/src/lib.rs:2229`) y elimina las de `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. См. `docs/examples/sorafs_rollout_stage_b.toml` y `docs/examples/sorafs_rollout_stage_c.toml` para fragmentos de fragmentos.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` теперь сохраняет разобранную fase (`crates/iroha/src/client.rs:2315`), чтобы helper команды (например `iroha_cli app sorafs fetch`) могли сообщать текущую fase вместе с política de anonimato predeterminada.

## Automatización

El asistente `cargo xtask` automatiza la generación de captura y captura de artefactos.

1. **Programación regional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Duraciones принимают суффиксы `s`, `m`, `h` o `d`. El comando `artifacts/soranet_pq_rollout_plan.json` y el resumen de Markdown (`artifacts/soranet_pq_rollout_plan.md`) pueden aplicar una solicitud de cambio.

2. **Capturar artefactos de perforación con подписями**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   El comando copia los archivos en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, los resúmenes de BLAKE3 del artefacto y la información `rollout_capture.json` con metadatos y Ed25519 подписью поверх carga útil. Utilice la clave privada, las actas del simulacro de incendio y la gobernanza de su dispositivo de captura válida.

## Matriz de etiquetas SDK y CLI| Superficie | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` или опираться на fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Este SDK alterna la configuración entre el analizador de escenario, el analizador y el orquestador (`crates/sorafs_orchestrator/src/lib.rs:365`), lo que permite implementar implementaciones multilingües en la fase de bloqueo.

## Lista de verificación de programación de Canarias

1. **Verificación previa (T menos 2 semanas)**- Tenga en cuenta que la tasa de caída de tensión de la etapa A =70% en la región (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте ranura de revisión de gobernanza, который утверждает окно canary.
   - Actualice `sorafs.gateway.rollout_phase = "ramp"` en puesta en escena (construya el orquestador JSON y vuelva a implementar) y active el proceso de promoción de prueba.

2. **Relevo canario (día T)**

   - Para cada región, coloque `rollout_phase = "ramp"` en el orquestador y manifiestos de relés.
   - Seleccione "Eventos de política por resultado" y "Tasa de apagones" en el panel de control de PQ Ratchet (con el panel desplegable) en la tecnología de caché de protección TTL.
   - Recorta instantáneas `sorafs_cli guard-directory fetch` para y después de auditar el almacenamiento.

3. **Cliente/SDK canario (T más 1 semana)**

   - Cambie `rollout_phase = "ramp"` a las configuraciones del cliente o anule `stage-b` de cohortes de SDK recientes.
   - Consulte las diferencias de telemetría (`sorafs_orchestrator_policy_events_total`, agrupadas en `client_id` e `region`) y utilice el registro de incidentes de implementación.

4. **Promoción predeterminada (T más 3 semanas)**

   - Después de la gestión de aprobación, se modifican las configuraciones del orquestador y del cliente en `rollout_phase = "default"` y se actualiza la lista de verificación de preparación disponible en los artefactos de lanzamiento.

## Lista de verificación de gobernanza y evidencia| Cambio de fase | Puerta de promoción | Paquete de pruebas | Paneles y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Tasa de caída de tensión de etapa A = 0,7 desde la promoción de la región, verificación del ticket Argon2 p95  Predeterminado *(aplicación de la Etapa C)* | Grabación de telemetría SN16 de 30 días, `sn16_handshake_downgrade_total` más en la línea de base, `sorafs_orchestrator_brownouts_total` más en el cliente canario y ensayo de alternancia de proxy. | Transcripción `sorafs_cli proxy set-mode --mode gateway|direct`, junto con `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, registro `sorafs_cli guard-directory verify`, paquete completo `cargo xtask soranet-rollout-capture --label default`. | Para la placa de trinquete PQ más paneles de degradación SN16, disponibles en `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. || Degradación de emergencia/preparación para reversión | Activación de contadores de degradación completos, verificación del directorio de guardia y eventos de degradación habituales en el búfer `/policy/proxy-toggle`. | Lista de verificación de `docs/source/ops/soranet_transport_rollback.md`, registros `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y plantillas de notificación. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y otros paquetes de alerta (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Храните каждый artefacto en `artifacts/soranet_pq_rollout/<timestamp>_<label>/` combinado con el generador `rollout_capture.json`, чтобы paquetes de gobernanza junto con el marcador, rastros y resúmenes de Promtool.
- Приложите SHA256 resume загруженных доказательств (minutas PDF, paquete de captura, instantáneas de guardia) к actas de promoción, чтобы aprobaciones del Parlamento можно было воспроизвести без доступа к grupo de preparación.
- Siga el plan de telemetría en el ticket de promoción, podrá cambiar el sistema `docs/source/soranet/snnet16_telemetry_plan.md` para bajar vocabularios y umbrales de alerta.

## Actualizaciones del panel y la telemetría

`dashboards/grafana/soranet_pq_ratchet.json` incluye el panel de anotaciones "Plan de implementación", la configuración de este libro de jugadas y la fase de desarrollo de temas, las revisiones de gobernanza de cada uno de ellos подтвердить активную etapa. Deje que el panel de control esté sincronizado con los mandos integrados.

Para las alertas, las reglas de diseño incluyen la etiqueta `stage`, las fases canarias y predeterminadas activan otros umbrales de políticas (`dashboards/alerts/soranet_handshake_rules.yml`).## Ganchos de retroceso

### Predeterminado -> Rampa (Etapa C -> Etapa B)

1. Degradar al orquestador con `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (y eliminar la fase en las configuraciones del SDK), con la Etapa B para toda la flota.
2. Registre a los clientes en el perfil de transporte básico como `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, transcripción completa del flujo de trabajo de auditabilidad `/policy/proxy-toggle`.
3. Introduzca `cargo xtask soranet-rollout-capture --label rollback-default` para ver las diferencias del directorio de guardia, la salida de Promtool y las capturas de pantalla del panel de control en `artifacts/soranet_pq_rollout/`.

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Importe la instantánea del directorio de protección, la descarga previa a la promoción, la descarga de `sorafs_cli guard-directory import --guard-directory guards.json` y la descarga posterior de `sorafs_cli guard-directory verify`, el paquete de degradación incluido. hash.
2. Instale `rollout_phase = "canary"` (o anule `anonymity_policy stage-a`) en las configuraciones de Orchestrator y Client, coloque el taladro de trinquete PQ en [Runbook de trinquete PQ] (./pq-ratchet-runbook.md), haga clic en доказать tubería de degradación.
3. Utilice capturas de pantalla de telemetría PQ Ratchet y SN16 adicionales y resultados de alerta en el registro de incidentes antes de la gobernanza.

### Recordatorios de barandillas

- Utilice `docs/source/ops/soranet_transport_rollback.md` para reducir la degradación y corregir la mitigación adicional como `TODO:` en el rastreador de implementación para robots pospuestos.
- Utilice `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` para almacenar `promtool test rules` y después de revertir la deriva de alerta para capturar toda la información. paquete.