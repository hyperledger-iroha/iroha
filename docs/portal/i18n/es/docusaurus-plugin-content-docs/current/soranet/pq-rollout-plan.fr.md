---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: Guía de implementación post-quantique SNNet-16G
sidebar_label: Plan de implementación PQ
Descripción: Guía operativa para promover el handshake híbrido X25519+ML-KEM de SoraNet de Canary de forma predeterminada en relés, clientes y SDK.
---

:::nota Fuente canónica
:::

SNNet-16G finaliza el despliegue posterior al transporte de SoraNet. Las perillas `rollout_phase` permiten a los operadores de coordinación una promoción determinante del requisito de protección de la Etapa A frente a la cobertura mayoritaria de la Etapa B y la postura PQ estricta de la Etapa C sin modificador de JSON/TOML brut para cada superficie.

Esta cobertura del libro de jugadas:

- Definiciones de fase y nuevos controles de configuración (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) y cables en el código base (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo de banderas SDK y CLI para que cada cliente pueda seguir el lanzamiento.
- Attentes de scheduling canary retransmisión/cliente más les tableros de gobierno qui gate la promoción (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de reversión y referencias del runbook de simulacro de incendio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Carta de fases| `rollout_phase` | Etapa de anonimato eficaz | Efecto por defecto | Uso típico |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | Exiger au moins un guard PQ par circuito colgante que la flotte se rechauffe. | Baseline et estrenos semaines de canary. |
| `ramp` | `anon-majority-pq` (Etapa B) | Favorezca la selección frente a los relés PQ para >= dos niveles de cobertura; Los relés clásicos permanecen en reserva. | Canarias relevos por región; alterna la vista previa del SDK. |
| `default` | `anon-strict-pq` (Etapa C) | Imposte los circuitos únicos PQ y active las alarmas de degradación. | Se completa la promoción final une fois la telemetrie et le sign-off Governance. |

Si una superficie definida además de un `anonymity_policy` explícito, anulará la fase para este componente. Omettre l'etape explicite diferre maintenant a la valeur `rollout_phase` afin que les operatorurs puissent basculer une fois par environnement et laisser les client l'heriter.

## Referencia de configuración

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

El cargador del orquestador genera la copia de seguridad de la cinta en tiempo de ejecución (`crates/sorafs_orchestrator/src/lib.rs:2229`) y la exposición a través de `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. Vea `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para aplicar los fragmentos.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` registre maintenant la fase parsee (`crates/iroha/src/client.rs:2315`) después de que los comandos auxiliares (por ejemplo `iroha_cli app sorafs fetch`) puedan reportar la fase actual aux cotes de la politique d'anonymat por defecto.

## Automatización

Dos ayudantes `cargo xtask` automatizan la generación del cronograma y la captura de artefactos.

1. **Generar el horario regional**

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

   Las duras aceptan los sufijos `s`, `m`, `h` o `d`. El comando emet `artifacts/soranet_pq_rollout_plan.json` y un currículum Markdown (`artifacts/soranet_pq_rollout_plan.md`) se unieron a la demanda de cambio.

2. **Capturer los artefactos del taladro con firmas**

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

   El comando copia los archivos almacenados en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula los resúmenes de BLAKE3 para cada artefacto y escribe `rollout_capture.json` con metadatos más una firma Ed25519 en la carga útil. Utilice la clave privada meme que tiene que firmar las actas del simulacro de incendio para que la gobernanza pueda validar rápidamente la captura.

## Matrice de flags SDK y CLI| Superficie | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` o reposador en la fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos los SDK de alternancia se asignan al analizador de escenarios de memes utilizados por el orquestador (`crates/sorafs_orchestrator/src/lib.rs:365`), por lo que las implementaciones en varios idiomas permanecen en bloque con la fase configurada.

## Lista de verificación de programación canaria

1. **Verificación previa (T menos 2 semanas)**- Confirmador de que la tasa de apagón Stage A es =70% por región (`sorafs_orchestrator_pq_candidate_ratio`).
   - Planificador del slot de gobernanza que revisa la fenetre canaria.
   - Mettre a jour `sorafs.gateway.rollout_phase = "ramp"` en puesta en escena (editor del orquestador JSON y redistribuidor) y ejecución en seco del canal de promoción.

2. **Relevo canario (día T)**

   - Promouvoir une region a la fois en definissant `rollout_phase = "ramp"` sur l'orchestrator et les manifests de relevos participantes.
   - Vigilancia de "Eventos de política por resultado" y "Tasa de apagones" en el panel PQ Ratchet (que incluye el mantenimiento del despliegue del panel) durante dos veces el TTL de la caché de guardia.
   - Capturador de instantáneas `sorafs_cli guard-directory fetch` antes y después para almacenamiento de auditoría.

3. **Cliente/SDK canario (T más 1 semana)**

   - Bascule `rollout_phase = "ramp"` en las configuraciones del cliente o pase las anulaciones `stage-b` para las cohortes designadas por el SDK.
   - Capture las diferencias de telemetría (grupo `sorafs_orchestrator_policy_events_total` por `client_id` e `region`) y las conecte al registro de incidentes de implementación.

4. **Promoción predeterminada (T más 3 semanas)**

   - Una vez validado el gobierno, el orquestador basculante y las configuraciones del cliente versiones `rollout_phase = "default"`, gire la lista de verificación de preparación del firmante en los artefactos de lanzamiento.

## Lista de verificación de gobernanza y evidencia| Cambio de fase | Puerta de promoción | Paquete de pruebas | Paneles y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Tasa de caída de tensión Etapa A = 0,7 por región promue, verificación de ticket Argon2 p95  Predeterminado *(aplicación de la Etapa C)* | Telemetría de grabación SN16 de 30 días atteint, `sn16_handshake_downgrade_total` plat au baseline, `sorafs_orchestrator_brownouts_total` un cliente canario de cero duración y ensayo del registro de conmutación de proxy. | Transcripción `sorafs_cli proxy set-mode --mode gateway|direct`, salida `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, registro `sorafs_cli guard-directory verify` y firma del paquete `cargo xtask soranet-rollout-capture --label default`. | Meme tableau PQ Ratchet plus les panneaux SN16 downgrade documentes dans `docs/source/sorafs_orchestrator_rollout.md` et `dashboards/grafana/soranet_privacy_metrics.json`. || Degradación de emergencia/preparación para reversión | Rechace cuando los ordenadores degradan el monte, el directorio de guardia de verificación hace eco o el buffer `/policy/proxy-toggle` registra los eventos de degradación. | Lista de verificación `docs/source/ops/soranet_transport_rollback.md`, registros `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y plantillas de notificación. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y los dos paquetes de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Stockez caque artefacto sous `artifacts/soranet_pq_rollout/<timestamp>_<label>/` avec le `rollout_capture.json` genere afin que les paquetes de gobernanza contienen el marcador, les traces promtool et les digests.
- Adjunte los resúmenes SHA256 des preuves chargees (minutas en PDF, paquete de captura, instantáneas de guardia) aux minutas de promoción para que las aprobaciones del Parlamento puedan ser renovadas sin acceso al grupo de puesta en escena.
- Consulte el plan de telemetría en el ticket de promoción para comprobar que `docs/source/soranet/snnet16_telemetry_plan.md` contiene la fuente canónica de vocabularios de degradación y las siguientes alertas.

## Paneles de control y telemetría de Mise a day

`dashboards/grafana/soranet_pq_ratchet.json` incluye mantenimiento de un panel de anotación "Plan de implementación" que envía este libro de jugadas y expone la fase actual después de que las revistas de gobernanza confirmen que la etapa está activa. Guarde la descripción del panel sincronizado con las futuras evoluciones de los mandos de configuración.Para la alerta, asegúrese de que las reglas existentes utilizan la etiqueta `stage` después de que las fases canarias y por defecto disminuyen los umbrales de política separada (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversión

### Predeterminado -> Rampa (Etapa C -> Etapa B)

1. Retrogradez l'orchestrator avec `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (et faites miroiter la meme stage sur les configs SDK) para que Stage B reprenne sur toute la flotte.
2. Forcez les client vers le profil de transport sur `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, y capturant la transcripción afin que el flujo de trabajo de remediación `/policy/proxy-toggle` sigue siendo auditable.
3. Ejecute `cargo xtask soranet-rollout-capture --label rollback-default` para archivar los directorios de guardia de diferencias, la herramienta de selección y los paneles de captura de pantalla en `artifacts/soranet_pq_rollout/`.

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Importe la captura del directorio de protección de instantáneas antes de la promoción con `sorafs_cli guard-directory import --guard-directory guards.json` y relancez `sorafs_cli guard-directory verify` para que el paquete de degradación incluya los hashes.
2. Defina `rollout_phase = "canary"` (o anule con `anonymity_policy stage-a`) en el orquestador y en las configuraciones del cliente, luego reinicie el taladro de trinquete PQ del [PQ ratchet runbook](./pq-ratchet-runbook.md) para probar la degradación de la tubería.
3. Adjunte las capturas de pantalla actualizadas de PQ Ratchet y la telemetría SN16 además de los resultados de alertas en el registro de incidentes antes de la gestión de notificaciones.

### Rappels barandilla- Referencia `docs/source/ops/soranet_transport_rollback.md` a cada degradación y registre toda mitigación temporal como el artículo `TODO:` en el rastreador de implementación para seguir.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` bajo cobertura `promtool test rules` antes y después de una reversión para que todas las alertas se documenten con el paquete de captura.