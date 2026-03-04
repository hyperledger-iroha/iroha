---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: Déploiement et réglage de l'orchestrateur
sidebar_label: Réglage del orquestador
descripción: Valores por prácticas predeterminadas, consejos de ajuste y puntos de auditoría para amener el orquestador multifuente en GA.
---

:::nota Fuente canónica
Refleje `docs/source/sorafs/developer/orchestrator_tuning.md`. Guarde las dos copias alineadas jusqu'à ce que la documentación heredada está retirada.
:::

# Guía de implementación y regulación del orquestador

Esta guía se aplica a la [référence de configuración](orchestrator-config.md) y le
[runbook de implementación de múltiples fuentes](multi-source-rollout.md). El explícito
comentario ajuster el orquestador para cada fase de implementación, comentario intérprete
Los artefactos del marcador y las señales de televisión deben estar en su lugar.
avant d'élargir le trafic. Aplique estas recomendaciones de manera coherente en
la CLI, el SDK y la automatización según la misma política de cada uno de ellos
buscar determinista.

## 1. Juegos de parámetros de base

Parte de un modelo de configuración compartido y ajuste de un pequeño conjunto de reglas
au fur et à mesure du déploiement. Le tableau ci-dessous captura los valores
recomendados para las fases más corrientes; les valeurs non listées retombent
sur les valores por defecto de `OrchestratorConfig::default()` e `FetchOptions::default()`.| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|-------|-----------------|-------------------------------|------------------------------------|--------------------------------|------------------------------|-------|
| **Laboratorio/CI** | `3` | `2` | `2` | `2500` | `300` | Un plafón de latencia serré y una ventana de gracia cortes se activan rápidamente en evidencia de una televisión bruyante. Gardez des reintries bas pour révéler les manifestes invalides plus tôt. |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | Reflète les valeurs de production tout en laissant de la margen pour des peers exploratoires. |
| **Canario** | `6` | `3` | `3` | `5000` | `900` | Corresponden a los valores predeterminados; Defina `telemetry_region` para permitir que los paneles de control giren en el tráfico canario. |
| **Disponibilidad general** | `None` (utiliza todos los elegibles) | `4` | `4` | `5000` | `900` | Aumente las veces de reintento y controle los fallos transitorios para conservar el determinismo a través de la auditoría. |- `scoreboard.weight_scale` resta el valor predeterminado `10_000` solo si un sistema disponible necesita otra resolución completa. Augmenter l'échelle ne change pas l'ordre des fournisseurs; Este producto es simple y tiene una distribución de créditos más densa.
- Al pasar una fase a la otra, persista el paquete JSON y utilice `--scoreboard-out` para que la pista de auditoría registre el juego de parámetros exactos.

## 2. Higiene del marcador

El marcador combina las exigencias del manifiesto, los anuncios de los proveedores y la televisión.
Avant d’avancer:1. **Valider la fraîcheur de la télémétrie.** Asegúrese de que las instantáneas referenciadas por
   `--telemetry-json` está capturado en la ventana de gracia configurada. Los entrantes
   plus anciennes que `telemetry_grace_secs` échouent con `TelemetryStale { last_updated }`.
   Traítelo como una parada durante y envíe la exportación de télémétrie antes de continuar.
2. **Inspecter les raisons d'éligibilité.** Persistez les artefactos vía
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Chaque entrante
   transporte un bloque `eligibility` con la causa exacta del chequeo. Ne contorno
   pas les ecarts de capacidad o les annonces caducados; Corrija la cantidad de carga útil.
3. **Revoir les écarts de poids.** Comparez le champ `normalised_weight` avec la
   liberación anterior. Las variaciones >10% deben corresponder a los cambios
   Volontaires d'annonces ou de télémétrie et être consignées dans le journal de déploiement.
4. **Archivar artefactos.** Configure `scoreboard.persist_path` para cada ejecución
   Emette le snapshot final du marcador. Adjuntar el artefacto al expediente de liberación
   con el manifiesto y el paquete de télémétrie.
5. **Consigner la preuve du mix fournisseurs.** La metadata de `scoreboard.json` _et_ le
   `summary.json` expositor de doivent correspondiente `provider_count`,
   `gateway_provider_count` y la etiqueta derivada `provider_mix` según los lectoresPuede comprobar si la ejecución es `direct-only`, `gateway-only` o `mixed`. les
   captura el reportero doivent de puerta de enlace `provider_count=0` más `provider_mix="gateway-only"`,
   tandis que les ejecuciones mixtas requieren des cuentas non nuls pour les dos fuentes.
   `cargo xtask sorafs-adoption-check` imponer ces campeones (et échoue si les comptes/labels
   divergente), alors exécutez-le toujours avec `ci/check_sorafs_orchestrator_adoption.sh`
   O su secuencia de comandos de captura para producir el paquete de pruebas `adoption_report.json`.
   Cuando las puertas de enlace Torii están implícitas, conserve `gateway_manifest_id`/`gateway_manifest_cid`
   en los metadatos del marcador para que la puerta de adopción pueda corregir el sobre
   du manifeste avec le mix fournisseurs capturé.

Para las definiciones de campos detallados, vea
`crates/sorafs_car/src/scoreboard.rs` y la estructura del currículum CLI expuesta por
`sorafs_cli fetch --json-out`.

## Referencia de banderas CLI y SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) y el envoltorio
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) parte del mismo
superficie de configuración del orquestador. Utilice las banderas siguientes lors de la
Capture los pasos previos al despliegue o recupere los accesorios canónicos:

Référence partagée des flags multi-source (guarde la ayuda CLI y los documentos sincronizados en
editar únicamente este archivo):- `--max-peers=<count>` limite el número de proveedores elegibles que pasan el filtro del marcador. Deje que el video se transmita después de todos los proveedores elegibles, use `1` únicamente para ejercitar voluntariamente el mono-fuente alternativo. Refleje la clave `maxPeers` en el SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` transmite el límite de reintentos por fragmento aplicado por `FetchOptions`. Utilice la tabla de implementación de la guía de ajuste para los valores recomendados; Las ejecuciones CLI que recopilan las preferencias deben corresponder a los valores predeterminados del SDK para garantizar la paridad.
- `--telemetry-region=<label>` etiqueta las series Prometheus `sorafs_orchestrator_*` (y los relés OTLP) con una etiqueta de región/entorno según los paneles que distinguen laboratorio, puesta en escena, canario y GA.
- `--telemetry-json=<path>` inyecta la instantánea referenciada por el marcador. Mantenga el JSON en el fondo del marcador para que los auditores puedan reanudar la ejecución (y para que `cargo xtask sorafs-adoption-check --require-telemetry` proporcione el flujo OTLP para alimentar la captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) activa los ganchos del puente del observador. Cuando esto está definido, el orquestador difunde los fragmentos a través del proxy Norito/Kaigi local para que los clientes naveguen, guarden cachés y habitaciones Kaigi reçoivent les mêmes reçus que Rust.- `--scoreboard-out=<path>` (eventuellement avec `--scoreboard-now=<unix_secs>`) persiste la instantánea de elegibilidad para los auditores. Asociez siempre el JSON persiste aux artefactos de télémétrie y de manifeste référencés dans le ticket de release.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplica ajustes determinados después de los anuncios de anuncios. Utilice estas banderas únicamente para las repeticiones; les downgrades de production doivent passer par des artifacts de gouvernance pour que cada nœud applique le même bundle de politique.
- `--provider-metrics-out` / `--chunk-receipts-out` conserva las métricas de salud del proveedor y los recursos de trozos referenciados por la lista de verificación de implementación; adjuntar los dos artefactos al depósito de la prueba de adopción.

Ejemplo (utilizando el accesorio publicado):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

El SDK permite la misma configuración a través de `SorafsGatewayFetchOptions` en el
cliente Rust (`crates/iroha/src/client.rs`), enlaces JS
(`javascript/iroha_js/src/sorafs.js`) y el SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Gardez ces ayudantes alineados
con los valores predeterminados de la CLI para que los operadores puedan copiarlos
politiques dans l’automatisation sans sofás de traducción ad hoc.

## 3. Ajuste de la política de recuperación

`FetchOptions` controla los reintentos, la coincidencia y la verificación. Lors du
regla :- **Reintentos:** aumenta `per_chunk_retry_limit` au-delà de `4` accroît le temps
  de recuperación más riesgo de enmascarar los fallos de los proveedores. Preférez
  Garder `4` como plafón y ordenador en la rotación de los proveedores para
  expositor les mauvais intérpretes.
- **Seuil d'échec :** `provider_failure_threshold` determina cuando un proveedor
  está desactivado para el resto de la sesión. Alinear este valor en la política
  de reintentos : un seuil inferior au presupuesto de reintentos fuerza el orquestador à
  Expulse un par antes de que todos los reintentos no sean presionados.
- **Concurrencia:** laissez `global_parallel_limit` non défini (`None`) à moins
  Qué entorno específico no puede saturar las playas anunciadas. Lorsque
  Definitivamente, asegúrese de que el valor soit ≤ à la somme des Budgets de Streams des
  proveedores para evitar el hambre.
- **Conmutadores de verificación:** `verify_lengths` e `verify_digests` hacen rester
  activés en producción. Ils garantissent le determinisme lorsque des flottes
  mixtes de fournisseurs sont actives; ne les désactivez que dans des environnements
  de fuzzing isolés.

## 4. Transporte y puesta en escena anónimo

Utilice los campos `rollout_phase`, `anonymity_policy` e `transport_policy` para
representante de la postura de confidencialidad:- Preférez `rollout_phase="snnet-5"` y laissez la politique d'anonymat por defecto
  suivre les jalons SNNet-5. Reemplazar a través del exclusivo `anonymity_policy_override`
  lorsque la gouvernance émet una directiva firmada.
- Gardez `transport_policy="soranet-first"` como base tan que SNNet-4/5/5a/5b/6a/7/8/12/13 sont 🈺
  (voir `roadmap.md`). Utilice `transport_policy="direct-only"` únicamente para des
  downgrades documentés ou des exercices de conformité, et listening the revue de
  cobertura PQ avant de promouvoir `transport_policy="soranet-strict"` — ce niveau
  échouera rapidement si seuls des relais classiques subsistent.
- `write_mode="pq-only"` ne doit être appliqué que lorsque chaque chemin d’écriture
  (SDK, orquestador, herramientas de gobierno) puede satisfacer las exigencias de PQ. durante
  les rollouts, gardez `write_mode="allow-downgrade"` afin que les réponses d’urgence
  Pulse en las rutas directas mientras la señal de televisión es
  degradación.
- La selección de guardias y la preparación de circuitos se activan en el
  repertorio SoraNet. Fournissez le snapshot signé de `relay_directory` et
  persistez le cache `guard_set` afin que le churn de guards reste dans la fenêtre
  convocatoria de retención. El empreinte du cache registrado por `sorafs_cli fetch`
  fait partie de l'evidence de rollout.

## 5. Ganchos de degradación y conformidad

Dos subsistemas del orquestador asistentes para hacer respeto a la política sans
manual de intervención:- **Remédiation des downgrades** (`downgrade_remediation`): vigilancia de eventos
  `handshake_downgrade_total` y, después de pasar de `threshold` configurado en
  `window_secs`, fuerza el proxy local en `target_mode` (por defecto solo metadatos).
  Conservar los valores por defecto (`threshold=3`, `window=300`, `cooldown=900`) solo
  si les postmortems monrent un autre schéma. Documentez toute override dans le
  Diario de implementación y garantía de que los paneles de control siguientes
  `sorafs_proxy_downgrade_state`.
- **Politique de conformité** (`compliance`): les carve-outs de juridiction et de
  manifeste passent par les listes d’opt-out gérées par la gouvernance. N'intégrez
  jamais d'overrides ad hoc dans le bundle de configuración; demandez plutôt une
  mise à jour signée de `governance/compliance/soranet_opt_outs.json` y redéployez
  el JSON genérico.

Para los dos sistemas, persista el paquete de configuración resultante e incluyalo
aux preuves de release afin que les auditeurs puissent retracer les bascules.

## 6. Telemetría y paneles de control

Antes de ampliar el lanzamiento, confirme que las señales siguientes están activas en
El medio ambiente cible :- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  Debe estar en cero después del fin del canario.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — doivent se estabilizador sous 10% colgante le
  canari et rester sous 5% après GA.
- `sorafs_orchestrator_policy_events_total`: válida la etapa de asistencia al lanzamiento
  está activo (etiqueta `stage`) y registra las caídas de tensión a través de `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — Suivent l'offre de relais PQ face aux
  attentes de la politique.
- Cibles de log `telemetry::sorafs.fetch.*`: debe ser enviado al agregador
  de logs partagé avec des recherches sauvegardées pour `status=failed`.

Cargue el tablero Grafana canonique desde después
`dashboards/grafana/sorafs_fetch_observability.json` (exportado en el puerto bajo
**SoraFS → Obtener observabilidad**) según los seleccionados región/manifest, el mapa de calor
reintentos por proveedor, histogramas de latencia de fragmentos y ordenadores
de blocage corresponsal à ce que SRE examina lors des burn-ins. Raccordez les règles
Alertmanager en `dashboards/alerts/sorafs_fetch_rules.yml` y validez de la sintaxis
Prometheus con `scripts/telemetry/test_sorafs_fetch_alerts.sh` (el asistente ejecuta
Ubicación `promtool test rules` o vía Docker). Las transferencias de alertas exigen le
También hay un bloque de ruta que el script imprime para que los operadores puedan unirse.
La evidencia en el ticket de implementación.

### Flujo de trabajo de grabación de télemétrieEl elemento de hoja de ruta **SF-6e** exige una grabación de télémétrie de 30 días antes de
basculer l'orchestrateur multi-source vers ses valeurs GA. Utilice los scripts del
Referencia para capturar un paquete de artefactos reproducibles cada día.
ventana :

1. Ejecute `ci/check_sorafs_orchestrator_adoption.sh` con las variables de burn-in
   definiciones. Ejemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Le ayudante se regocija `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   Escrito `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` y `adoption_report.json` bajo
   `artifacts/sorafs_orchestrator/<timestamp>/`, e imponer un nombre mínimo de
   proveedores elegibles a través de `cargo xtask sorafs-adoption-check`.
2. Cuando las variables de burn-in están presentes, el script también aparece
   `burn_in_note.json`, capturando la etiqueta, el índice del día, el ID del manifiesto,
   la source de télémétrie et les digests des artefactos. Únase a JSON en el diario
   de rollout afin qu’il soit clair quelle capture a satisfait chaque jour de la
   ventana de 30 días.
3. Importe el cuadro Grafana del día (`dashboards/grafana/sorafs_fetch_observability.json`)
   en el espacio de trabajo puesta en escena/producción, taguez-le avec le label de burn-in
   y verifique que cada panel affiche des échantillons pour le manifeste/région testés.
4. Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o `promtool test rules …`)
   chaque fois que `dashboards/alerts/sorafs_fetch_rules.yml` change, afin de documenter
   que le routage des alertes corresponden aux métriques exportées colgante le burn-in.
5. Archivar la instantánea del tablero, la salida de prueba de alertas y la cola de registros
   des recherches `telemetry::sorafs.fetch.*` con los artefactos del orquestador para
   que la gouvernance puisse rejouer l’evidence sans extraire de métriques des systèmes live.

## 7. Lista de verificación de implementación1. Régénérez les scoreboards en CI con la configuración candidata y capturez les
   artefactos bajo control de versión.
2. Ejecute la búsqueda de elementos fijos en cada entorno (laboratorio, puesta en escena,
   canari, producción) y adjuntar los artefactos `--scoreboard-out` et `--json-out`
   en el registro de implementación.
3. Pase en revista los paneles de télémétrie con el ingeniero de astreinte, en
   vérifiant que toutes les métriques ci-dessus ont des échantillons live.
4. Registrez le chemin de configuración final (souvent via `iroha_config`) et le
   comprometerse con el registro de gobierno utilizado para los anuncios y la conformidad.
5. Actualice el rastreador de lanzamiento e informe los equipos SDK de los nuevos
   Por defecto, los clientes de integración están alineados.

Siga esta guía para mantener los despliegues del orquestador determinante y
auditables, todos los botones de retroacción claras para ajustar
los presupuestos de reintento, la capacidad de los proveedores y la postura de confidencialidad.