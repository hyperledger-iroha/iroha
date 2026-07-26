---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: Guía de implementación pos-cuántica SNNet-16G
sidebar_label: Plano de implementación PQ
Descripción: Guía operativa para promover el protocolo de enlace hibrido X25519+ML-KEM de SoraNet de canary para relés, clientes y SDK predeterminados.
---

:::nota Fuente canónica
Esta página espelha `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas como copias sincronizadas.
:::

SNNet-16G concluye el lanzamiento poscuántico del transporte de SoraNet. Los botones `rollout_phase` permiten que los operadores coordenen una promoción determinística del requisito actual de guardia Stage A para una cobertura mayoritaria Stage B y una postura PQ estrita Stage C sin editar JSON/TOML bruto para cada superficie.

Este libro de jugadas cobre:

- Definicoes de fase e os novos Knobs de configuracao (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) cableados sin base de código (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo de flags SDK y CLI para que cada cliente acompañe el lanzamiento.
- Expectativas de programación de canary retransmisión/cliente y os paneles de gobierno que gateiam a promocao (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback e referencias al fire-drill runbook ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases| `rollout_phase` | Estación de anonimato efectivo | Efeito padrao | Uso típico |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | Exigir al menos un guardia PQ por circuito mientras a frota aquece. | Baseline e semanas iniciais de canary. |
| `ramp` | `anon-majority-pq` (Etapa B) | Viesar a selección para relés PQ con >= dos tercos de cobertura; relevos classicos ficam como fallback. | Canarias por regiao de relevos; alterna la vista previa en SDK. |
| `default` | `anon-strict-pq` (Etapa C) | Los circuitos del coche apenas PQ y soportan alarmas de bajada. | Promoción final de telemetría y aprobación de gobernanza. |

Si una superficie también define `anonymity_policy` explícitamente, ela sobrescreve a fase para aquele componente. Omitir o stage explicito agora faz difer ao valor de `rollout_phase` para que los operadores possam virar a fase una vez por ambiente e deixar os client herdarem.

## Referencia de configuración

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

El cargador del orquestador resuelve la etapa de respaldo en tiempo de ejecución (`crates/sorafs_orchestrator/src/lib.rs:2229`) y el exponente a través de `sorafs_orchestrator_policy_events_total` e `sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para fragmentos prontos para aplicar.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` ahora registra una fase parseada (`crates/iroha/src/client.rs:2315`) para que helper commands (por ejemplo `iroha_cli app sorafs fetch`) reportem a fase actual junto con una política de anonimato padrao.

##Automacao

Estos ayudantes `cargo xtask` automatizan la gestión de la programación y la captura de artefatos.

1. **Gerar o programar regional**

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

   Las duraciones incluyen los sufijos `s`, `m`, `h` o `d`. El comando emite `artifacts/soranet_pq_rollout_plan.json` y un resumen Markdown (`artifacts/soranet_pq_rollout_plan.md`) que puede ser enviado con una solicitud de cambio.

2. **Capturar artefactos del taladro con assinaturas**

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

   El comando copia los archivos guardados para `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula resúmenes de BLAKE3 para cada artefato y guarda `rollout_capture.json` conteniendo metadatos y una assinatura Ed25519 sobre la carga útil. Utilice una pequeña clave privada que assina como minutos de simulacro de incendio para que un gobierno valide una captura rápidamente.

## Matriz de banderas SDK y CLI| Superficie | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` o confiar en la fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos los conmutadores de SDK mapeiam para el mismo analizador de escenario usado por el orquestador (`crates/sorafs_orchestrator/src/lib.rs:365`), incluidas implementaciones de archivos multilingüe en lock-step con una fase configurada.

## Lista de verificación de programación canaria

1. **Verificación previa (T menos 2 semanas)**- Confirmar que o brownout rate de Stage A esta =70% por regiao (`sorafs_orchestrator_pq_candidate_ratio`).
   - Agendar o slot de Governance Review que aprova a Janela de Canarias.
   - Actualizar `sorafs.gateway.rollout_phase = "ramp"` en puesta en escena (editar o orquestador JSON y reimplementar) y hacer un simulacro de tubería de promoción.

2. **Relevo canario (día T)**

   - Promover uma regiao por vez configurando `rollout_phase = "ramp"` no orquestador y nos manifiesta de relevo participantes.
   - Monitorear "Eventos de política por resultado" y "Tasa de apagones" en el panel PQ Ratchet (ágora con panel de implementación) por dos veces o TTL para guardar caché.
   - Fazer snapshots de `sorafs_cli guard-directory fetch` antes y después para armazenamento de auditoria.

3. **Cliente/SDK canario (T más 1 semana)**

   - Trocar para `rollout_phase = "ramp"` en configuraciones de cliente o pasar anulaciones `stage-b` para cohortes de SDK designadas.
   - Capturar diferencias de telemetría (`sorafs_orchestrator_policy_events_total` agrupadas por `client_id` e `region`) y adjuntar al registro de incidentes de implementación.

4. **Promoción predeterminada (T más 3 semanas)**

   - Después de aprobar el gobierno, modificar las configuraciones del orquestador y del cliente para `rollout_phase = "default"` y rotar la lista de verificación de preparación asociada para los artefactos de lanzamiento.

## Lista de verificación de gobernanza y evidencia| Cambio de fase | Puerta de promoción | Paquete de pruebas | Paneles y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Tasa de apagón de etapa A = 0.7 por promoción real, verificación de ticket Argon2 p95  Predeterminado *(aplicación de la Etapa C)* | Burn-in de telemetria SN16 de 30 días concluido, `sn16_handshake_downgrade_total` flat no baseline, `sorafs_orchestrator_brownouts_total` zero durante o canary de client, y o rehearsal do proxy toggle logado. | Transcripción `sorafs_cli proxy set-mode --mode gateway|direct`, salida de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, registro de `sorafs_cli guard-directory verify`, y paquete assinado `cargo xtask soranet-rollout-capture --label default`. | El tablero mesmo PQ Ratchet y los paneles de degradación SN16 documentados en `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. || Degradación de emergencia/preparación para reversión | Cuando se producen contadores de degradación, se verifica que el directorio de guardia no esté actualizado o el buffer `/policy/proxy-toggle` registra eventos de degradación sustentados. | Lista de verificación de `docs/source/ops/soranet_transport_rollback.md`, registros de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y plantillas de notificación. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y ambos paquetes de alerta (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Armazene cada artefato em `artifacts/soranet_pq_rollout/<timestamp>_<label>/` com o `rollout_capture.json` generado para que los paquetes de gobernanza contengan el marcador, los rastros y los resúmenes de Promtool.
- Anexe resume SHA256 das evidencias carregadas (actas en PDF, paquete de captura, instantáneas de guardia) como actas de promoción para que el Parlamento aprovacoes possam ser reproducidas sin acceso al grupo de puesta en escena.
- Referencia del plano de telemetría sin ticket de promoción para probar que `docs/source/soranet/snnet16_telemetry_plan.md` sigue enviando a fuente canónica para vocabularios de degradación y umbrales de alerta.

## Actualizaciones de tablero y telemetría

`dashboards/grafana/soranet_pq_ratchet.json` ahora incluye un panel de notas "Plan de implementación" que se vincula a este manual y muestra una fase actual para que las revisiones de gobernanza confirmen cual etapa está activa. Mantenga la descripción del panel sincronizado con mudancas futuras en las perillas de configuración.Para alertar, garanta que como registros existentes use la etiqueta `stage` para que como fases canary y default disparen umbrales de política separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de retroceso

### Predeterminado -> Rampa (Etapa C -> Etapa B)

1. Faca demotion do Orchestrator com `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (y espelhe a mesma fase nos SDK configs) para que Stage B regrese em toda a fricción.
2. Forzar a los clientes a un perfil de transporte seguro a través de `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando la transcripción para que el flujo de trabajo `/policy/proxy-toggle` continúe auditándose.
3. Utilice `cargo xtask soranet-rollout-capture --label rollback-default` para archivar diferencias de directorio de guardia, salida de Promtool y capturas de pantalla del tablero en `artifacts/soranet_pq_rollout/`.

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Importe la instantánea del directorio de guardia capturada antes de la promoción con `sorafs_cli guard-directory import --guard-directory guards.json` y monte `sorafs_cli guard-directory verify` nuevamente para que el paquete de degradación incluya hashes.
2. Ajuste `rollout_phase = "canary"` (o anule con `anonymity_policy stage-a`) en las configuraciones del orquestador y del cliente, luego montó novamente el taladro de trinquete PQ en [PQ ratchet runbook](./pq-ratchet-runbook.md) para probar la tubería de degradación.
3. Anexe capturas de pantalla actualizadas de PQ Ratchet y telemetría SN16 más los resultados de alertas y el registro de incidentes antes de la gobernanza de notificaciones.

### Recordatorios de barandillas- Referencia `docs/source/ops/soranet_transport_rollback.md` siempre que se produzca una degradación y registre cualquier atenuante temporal como el artículo `TODO:` sin seguimiento de implementación para acompañamiento.
- Mantenha `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` sob cobertura de `promtool test rules` antes y después de una reversión para que o alerta de deriva fique documentado junto al paquete de captura.