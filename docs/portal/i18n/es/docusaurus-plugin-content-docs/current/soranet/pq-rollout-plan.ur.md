---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: Manual de estrategias posterior al lanzamiento cuántico de SNNet-16G
sidebar_label: Plan de implementación de PQ
descripción: SoraNet کے híbrido X25519+ML-KEM handshake کو canary سے relés تک predeterminados, clientes اور SDK میں promover کرنے کے لئے عملی رہنمائی۔
---

:::nota Fuente canónica
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانا conjunto de documentación retirar نہ ہو، دونوں کاپیاں sincronización رکھیں۔
:::

Transporte SNNet-16G SoraNet کے لئے implementación post-cuántica مکمل کرتا ہے۔ `rollout_phase` operadores de perillas کو coordenada de promoción determinista کرنے دیتے ہیں جو موجودہ Requisito de guardia de etapa A سے Cobertura mayoritaria de etapa B اور Etapa C postura estricta de PQ تک جاتی ہے، بغیر ہر superficie کے raw JSON/TOML کو editar کئے۔

یہ libro de jugadas درج ذیل چیزیں portada کرتا ہے:

- Definiciones de fase y perillas de configuración (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) y base de código cableada (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- SDK اور CLI flags mapeo تاکہ ہر implementación del cliente کو track کر سکے۔
- Expectativas de programación canary de retransmisión/cliente, paneles de gobierno, promoción y puerta de enlace (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos de reversión y referencias del runbook de simulacro de incendio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases| `rollout_phase` | Etapa de anonimato efectivo | Efecto predeterminado | Uso típico |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | فلیٹ کے گرم ہونے تک ہر circuito کے لئے کم از کم ایک PQ guard لازم کریں۔ | Línea de base اور ابتدائی canario ہفتے۔ |
| `ramp` | `anon-majority-pq` (Etapa B) | انتخاب کو PQ relés کی طرف sesgo کریں تاکہ >= دو تہائی cobertura ہو؛ Relés clásicos de reserva رہیں۔ | Canarias de relevo región por región؛ La vista previa del SDK alterna ۔ |
| `default` | `anon-strict-pq` (Etapa C) | Los circuitos exclusivos de PQ aplican alarmas de degradación y degradación. | Telemetría اور aprobación de gobernanza مکمل ہونے کے بعد آخری promoción۔ |

اگر کوئی superficie explícita `anonymity_policy` بھی set کرے تو وہ اس componente کے لئے fase کو anular کرتا ہے۔ Etapa explícita نہ ہو تو اب `rollout_phase` valor پر diferir ہوتا ہے تاکہ operadores ہر entorno میں ایک بار fase flip کریں اور clientes اسے heredar کریں۔

## Referencia de configuración

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Tiempo de ejecución del cargador de Orchestrator Resolución de etapa de reserva کرتا ہے (`crates/sorafs_orchestrator/src/lib.rs:2229`) اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے superficie کرتا ہے۔ `docs/examples/sorafs_rollout_stage_b.toml` y `docs/examples/sorafs_rollout_stage_c.toml` Varios fragmentos listos para aplicar

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` اب registro de fase analizada کرتا ہے (`crates/iroha/src/client.rs:2315`) تاکہ comandos de ayuda (مثال کے طور پر `iroha_cli app sorafs fetch`) موجودہ fase کو política de anonimato predeterminada کے ساتھ informe کر سکیں۔

## Automatización

Los ayudantes `cargo xtask` programan la generación y automatizan la captura de artefactos کرتے ہیں۔

1. **El horario regional genera کریں**

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

   Duraciones `s`, `m`, `h`, یا `d` sufijo قبول کرتے ہیں۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` اور Resumen de rebajas (`artifacts/soranet_pq_rollout_plan.md`) emite کرتی ہے جسے solicitud de cambio کے ساتھ بھیجا جا سکتا ہے۔

2. **Artefactos de perforación کو firmas کے ساتھ captura کریں**

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

   کمانڈ فراہم کردہ فائلیں `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں کاپی کرتی ہے، ہر artefacto کے لئے BLAKE3 computa کرتی ہے، اور `rollout_capture.json` لکھتی ہے جس میں metadatos y carga útil پر Ed25519 firma شامل ہوتا ہے۔ اسی clave privada کو استعمال کریں جو firma de minutos de simulacro de incendio کرتی ہے تاکہ gobernanza جلدی validar کر سکے۔

## Matriz de indicadores de SDK y CLI| Superficie | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` یا fase پر انحصار | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

El SDK alterna el analizador de escenario, el mapa, el orquestador, la fase configurada (`crates/sorafs_orchestrator/src/lib.rs:365`), la fase configurada de implementaciones multilingües y el paso de bloqueo. رہتے ہیں۔

## Lista de verificación de programación de Canarias

1. **Verificación previa (T menos 2 semanas)**- Confirmar la tasa de caída de tensión de la etapa A پچھلے دو ہفتوں میں =70% ہے (`sorafs_orchestrator_pq_candidate_ratio`).
   - Calendario de espacios de revisión de gobernanza کریں جو ventana canaria aprobar کرتا ہے۔
   - Actualización de puesta en escena `sorafs.gateway.rollout_phase = "ramp"` (edición JSON del orquestador y redistribución) y proceso de promoción y ejecución en seco

2. **Relevo canario (día T)**

   - ایک وقت میں ایک región promover کریں، `rollout_phase = "ramp"` کو orquestador اور manifiestos de retransmisión participantes پر establecer کرتے ہوئے۔
   - Panel de control de PQ Ratchet, "Eventos de política por resultado", "Tasa de apagones", caché de protección TTL, configuración del monitor, (panel de control, panel de despliegue, configuración)
   - Auditar almacenamiento کے لئے `sorafs_cli guard-directory fetch` instantáneas پہلے اور بعد میں لیں۔

3. **Cliente/SDK canario (T más 1 semana)**

   - Configuraciones del cliente میں `rollout_phase = "ramp"` flip کریں یا منتخب SDK cohorts کے لئے `stage-b` anula دیں۔
   - Captura de diferencias de telemetría کریں (`sorafs_orchestrator_policy_events_total` کو `client_id` اور `region` کے حساب سے grupo کریں) اور انہیں registro de incidentes de implementación کے ساتھ adjuntar کریں۔

4. **Promoción predeterminada (T más 3 semanas)**

   - Aprobación de gobernanza, orquestador, configuraciones de cliente, `rollout_phase = "default"`, interruptor, lista de verificación de preparación firmada, liberación de artefactos, rotación

## Lista de verificación de gobernanza y evidencia| Cambio de fase | Puerta de promoción | Paquete de pruebas | Paneles y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Tasa de caída de tensión de etapa A پچھلے 14 دن میں = 0.7 فی región promocionada, verificación de ticket Argon2 p95  Predeterminado *(aplicación de la Etapa C)* | Grabación de telemetría SN16 de 30 días مکمل، `sn16_handshake_downgrade_total` baseline پر flat، client canary کے دوران `sorafs_orchestrator_brownouts_total` صفر، اور proxy toggle ensayo log۔ | Transcripción `sorafs_cli proxy set-mode --mode gateway|direct`, salida `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, registro `sorafs_cli guard-directory verify`, paquete `cargo xtask soranet-rollout-capture --label default` firmado. | وہی Tablero de trinquete PQ اور Paneles de degradación SN16 جو `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں documentado ہیں۔ || Degradación de emergencia/preparación para reversión | Activador تب ہوتا ہے جب pico de contadores de degradación کریں، error de verificación del directorio de guardia ہو، یا `/policy/proxy-toggle` buffer مسلسل eventos de degradación ریکارڈ کرے۔ | Lista de verificación `docs/source/ops/soranet_transport_rollback.md`, registros `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y plantillas de notificación | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, paquetes de alertas adicionales (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- ہر artefacto کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں store کریں، generado `rollout_capture.json` کے ساتھ، تاکہ paquetes de gobernanza میں marcador, rastros de promtool, اور resúmenes شامل ہوں۔
- Evidencia cargada (minutas PDF, paquete de captura, instantáneas de guardia) کے SHA256 resume las actas de promoción کے ساتھ adjuntar کریں تاکہ Grupo de preparación de aprobaciones del Parlamento تک رسائی کے بغیر repetición ہو سکیں۔
- Boleto de promoción میں plan de telemetría کا حوالہ دیں تاکہ ثابت ہو کہ `docs/source/soranet/snnet16_telemetry_plan.md` vocabularios de degradación اور umbrales de alerta کے لئے fuente canónica ہے۔

## Actualizaciones del panel y la telemetría

`dashboards/grafana/soranet_pq_ratchet.json` اب Panel de anotaciones "Plan de implementación" کے ساتھ nave ہوتا ہے جو اس playbook سے enlace کرتا ہے اور fase actual ظاہر کرتا ہے تاکہ revisiones de gobernanza etapa activa کی تصدیق کر سکیں۔ Descripción del panel کو perillas de configuración کی مستقبل تبدیلیوں کے ساتھ sync رکھیں۔

Alerta کے لئے یقینی بنائیں کہ موجودہ reglas `stage` etiqueta استعمال کریں تاکہ canario اور fases predeterminadas الگ umbrales de política desencadenante کریں (`dashboards/alerts/soranet_handshake_rules.yml`).## Ganchos de retroceso

### Predeterminado -> Rampa (Etapa C -> Etapa B)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` کے ذریعے orquestador کو degradar کریں (اور Configuraciones SDK میں وہی espejo de fase کریں) تاکہ Etapa B پورے flota میں دوبارہ لاگو ہو۔
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے clientes کو perfil de transporte seguro پر مجبور کریں، اور captura de transcripción کریں تاکہ `/policy/proxy-toggle` flujo de trabajo de remediación auditable رہے۔
3. `cargo xtask soranet-rollout-capture --label rollback-default` چلائیں تاکہ guard-directory diffs, salida de Promtool, اور capturas de pantalla del tablero کو `artifacts/soranet_pq_rollout/` میں archive کیا جا سکے۔

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Promoción سے پہلے captura کیا گیا instantánea del directorio de guardia `sorafs_cli guard-directory import --guard-directory guards.json` کے ذریعے importación کریں اور `sorafs_cli guard-directory verify` دوبارہ چلائیں تاکہ paquete de degradación میں hashes شامل ہوں۔
2. Orchestrator اور configuraciones del cliente میں `rollout_phase = "canary"` set کریں (یا `anonymity_policy stage-a` anulación) اور پھر [PQ ratchet runbook](./pq-ratchet-runbook.md) سے PQ ratchet taladradora repetir کریں La tubería de degradación de تاکہ prueba ہو۔
3. Capturas de pantalla de telemetría PQ Ratchet اور SN16 actualizadas کے ساتھ resultados de alerta کو registro de incidentes میں adjuntar کریں، پھر gobernanza کو notificar

### Recordatorios de barandillas

- جب بھی degradación ہو `docs/source/ops/soranet_transport_rollback.md` referir کریں اور mitigación temporal کو seguimiento de implementación میں `TODO:` کے طور پر registro کریں تاکہ seguimiento ہو سکے۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو rollback سے پہلے اور بعد میں `promtool test rules` cobertura میں رکھیں تاکہ paquete de captura de deriva de alerta کے ساتھ documento ہو۔