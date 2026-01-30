---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3130e9a2b1f05a6b165d32c11381565944a195f2f9cdf4ea4031f3ca6e06ed6b
source_last_modified: "2025-11-14T04:43:21.972377+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sorafs/developer/orchestrator.md`. Manten ambas copias sincronizadas hasta que se retiren los docs heredados.
:::

# Guia del orquestador de fetch multi-origen

El orquestador de fetch multi-origen de SoraFS impulsa descargas deterministas y
paralelas desde el conjunto de proveedores publicado en adverts respaldados por
la gobernanza. Esta guia explica como configurar el orquestador, que senales de
fallo esperar durante los rollouts y que flujos de telemetria exponen indicadores
de salud.

## 1. Resumen de configuracion

El orquestador combina tres fuentes de configuracion:

| Fuente | Proposito | Notas |
|--------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza los pesos de proveedores, valida la frescura de la telemetria y persiste el scoreboard JSON usado para auditorias. | Respaldado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplica limites en tiempo de ejecucion (presupuestos de reintentos, limites de concurrencia, toggles de verificacion). | Se mapea a `FetchOptions` en `crates/sorafs_car::multi_fetch`. |
| Parametros de CLI / SDK | Limitan el numero de peers, adjuntan regiones de telemetria y exponen politicas de deny/boost. | `sorafs_cli fetch` expone estos flags directamente; los SDK los pasan via `OrchestratorConfig`. |

Los helpers JSON en `crates/sorafs_orchestrator::bindings` serializan la
configuracion completa en Norito JSON, lo que la hace portable entre bindings de
SDK y automatizacion.

### 1.1 Configuracion JSON de ejemplo

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Persiste el archivo mediante el apilado habitual de `iroha_config` (`defaults/`,
user, actual) para que los despliegues deterministas hereden los mismos limites
entre nodos. Para un perfil de fallback direct-only alineado con el rollout
SNNet-5a, consulta `docs/examples/sorafs_direct_mode_policy.json` y la guia
complementaria en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones de cumplimiento

SNNet-9 integra el cumplimiento dirigido por gobernanza en el orquestador. Un
nuevo objeto `compliance` en la configuracion Norito JSON captura los carve-outs
que fuerzan el pipeline de fetch a modo direct-only:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` declara los codigos ISO-3166 alpha-2 donde opera esta
  instancia del orquestador. Los codigos se normalizan a mayusculas durante el
  parseo.
- `jurisdiction_opt_outs` refleja el registro de gobernanza. Cuando alguna
  jurisdiccion de operador aparece en la lista, el orquestador aplica
  `transport_policy=direct-only` y emite la razon de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista digests de manifest (CIDs cegados, codificados en
  hex mayusculas). Las cargas coincidentes tambien fuerzan la planificacion
  direct-only y exponen el fallback `compliance_blinded_cid_opt_out` en la
  telemetria.
- `audit_contacts` registra las URI que la gobernanza espera que los operadores
  publiquen en sus playbooks GAR.
- `attestations` captura los paquetes de cumplimiento firmados que respaldan la
  politica. Cada entrada define una `jurisdiction` opcional (codigo ISO-3166
  alpha-2), un `document_uri`, el `digest_hex` canonico de 64 caracteres, el
  timestamp de emision `issued_at_ms` y un `expires_at_ms` opcional. Estos
  artefactos alimentan la checklist de auditoria del orquestador para que el
  tooling de gobernanza pueda vincular las anulaciones con la documentacion
  firmada.

Proporciona el bloque de compliance mediante el apilado habitual de
configuracion para que los operadores reciban anulaciones deterministas. El
orquestador aplica compliance _despues_ de los hints de write-mode: incluso si
un SDK solicita `upload-pq-only`, las exclusiones por jurisdiccion o manifiesto
siguen forzando transporte direct-only y fallan rapido cuando no existen
proveedores aptos.

Los catalogos canonicos de opt-out viven en
`governance/compliance/soranet_opt_outs.json`; el Consejo de Gobernanza publica
actualizaciones mediante releases etiquetados. Un ejemplo completo de
configuracion (incluyendo attestations) esta disponible en
`docs/examples/sorafs_compliance_policy.json`, y el proceso operativo se
captura en el
[playbook de cumplimiento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI y SDK

| Flag / Campo | Efecto |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita cuantos proveedores sobreviven al filtro del scoreboard. Ponlo en `None` para usar todos los proveedores elegibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita los reintentos por chunk. Superar el limite genera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Inyecta snapshots de latencia/fallo en el constructor del scoreboard. La telemetria obsoleta mas alla de `telemetry_grace_secs` marca proveedores como ineligibles. |
| `--scoreboard-out` | Persiste el scoreboard calculado (proveedores elegibles + ineligibles) para inspeccion posterior. |
| `--scoreboard-now` | Sustituye el timestamp del scoreboard (segundos Unix) para que las capturas de fixtures sigan siendo deterministas. |
| `--deny-provider` / hook de politica de score | Excluye deterministamente proveedores de la planificacion sin borrar adverts. Util para listas negras de respuesta rapida. |
| `--boost-provider=name:delta` | Ajusta los creditos de round-robin ponderado para un proveedor manteniendo intactos los pesos de gobernanza. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiqueta metricas emitidas y logs estructurados para que los dashboards puedan filtrar por geografia u ola de rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por defecto es `soranet-first` ahora que el orquestador multi-origen es la base. Usa `direct-only` al preparar un downgrade o seguir una directiva de cumplimiento, y reserva `soranet-strict` para pilotos PQ-only; las anulaciones de compliance siguen siendo el techo duro. |

SoraNet-first es ahora el valor por defecto, y los rollbacks deben citar el
bloqueador SNNet correspondiente. Tras graduarse SNNet-4/5/5a/5b/6a/7/8/12/13,
la gobernanza endurecera la postura requerida (hacia `soranet-strict`); hasta
entonces, solo las anulaciones motivadas por incidentes deben priorizar
`direct-only`, y deben registrarse en el log de rollout.

Todos los flags anteriores aceptan sintaxis estilo `--` tanto en
`sorafs_cli fetch` como en el binario `sorafs_fetch` orientado a
desarrolladores. Los SDK exponen las mismas opciones mediante builders tipados.

### 1.4 Gestion de cache de guards

La CLI ahora integra el selector de guards de SoraNet para que los operadores
puedan fijar relays de entrada de forma determinista antes del rollout completo
SNNet-5 de transporte. Tres nuevos flags controlan el flujo:

| Flag | Proposito |
|------|-----------|
| `--guard-directory <PATH>` | Apunta a un archivo JSON que describe el consenso de relays mas reciente (se muestra un subconjunto abajo). Pasar el directorio refresca la cache de guards antes de ejecutar el fetch. |
| `--guard-cache <PATH>` | Persiste el `GuardSet` codificado en Norito. Las ejecuciones posteriores reutilizan la cache incluso cuando no se suministra un directorio nuevo. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides opcionales para el numero de guards de entrada a fijar (por defecto 3) y la ventana de retencion (por defecto 30 dias). |
| `--guard-cache-key <HEX>` | Clave opcional de 32 bytes usada para etiquetar caches de guard con un MAC Blake3 para que el archivo pueda verificarse antes de reutilizarse. |

Las cargas del directorio de guards usan un esquema compacto:

El flag `--guard-directory` ahora espera un payload `GuardDirectorySnapshotV2`
codificado en Norito. El snapshot binario contiene:

- `version` — version del esquema (actualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadatos de consenso que deben coincidir con cada certificado embebido.
- `validation_phase` — compuerta de politica de certificados (`1` = permitir
  una sola firma Ed25519, `2` = preferir firmas dobles, `3` = requerir firmas
  dobles).
- `issuers` — emisores de gobernanza con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. Los fingerprints se calculan como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — una lista de bundles SRCv2 (salida de
  `RelayCertificateBundleV2::to_cbor()`). Cada bundle incluye el descriptor del
  relay, flags de capacidad, politica ML-KEM y firmas dobles Ed25519/ML-DSA-65.

La CLI verifica cada bundle contra las claves de emisor declaradas antes de
fusionar el directorio con la cache de guards. Los esquemas JSON heredados ya
no se aceptan; se requieren snapshots SRCv2.

Invoca la CLI con `--guard-directory` para fusionar el consenso mas reciente con
la cache existente. El selector conserva los guards fijados que aun estan dentro
la ventana de retencion y son elegibles en el directorio; los relays nuevos
reemplazan entradas expiradas. Tras un fetch exitoso, la cache actualizada se
escribe de nuevo en la ruta indicada por `--guard-cache`, manteniendo sesiones
subsecuentes deterministas. Los SDK pueden reproducir el mismo comportamiento al
llamar `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
y pasar el `GuardSet` resultante a `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite que el selector priorice guards con capacidad PQ
cuando se despliega SNNet-5. Los toggles de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) ahora degradan automaticamente relays
clasicos: cuando hay un guard PQ disponible el selector elimina los pines
clasicos sobrantes para que las sesiones posteriores favorezcan handshakes
hibridos. Los resumenes de CLI/SDK exponen la mezcla resultante via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` y los campos complementarios de candidatos/deficit/
variacion de supply, haciendo explicitos los brownouts y fallbacks clasicos.

Los directorios de guards ahora pueden incorporar un bundle SRCv2 completo via
`certificate_base64`. El orquestador decodifica cada bundle, revalida las firmas
Ed25519/ML-DSA y conserva el certificado analizado junto a la cache de guards.
Cuando hay un certificado presente se convierte en la fuente canonica para
claves PQ, preferencias de handshake y ponderacion; los certificados expirados
se descartan y el selector vuelve a campos heredados del descriptor. Los
certificados se propagan a la gestion del ciclo de vida de circuitos y se
exponen via `telemetry::sorafs.guard` y `telemetry::sorafs.circuit`, que registran
la ventana de validez, las suites de handshake y si se observaron firmas dobles
para cada guard.

Usa los helpers de la CLI para mantener los snapshots sincronizados con los
publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` descarga y verifica el snapshot SRCv2 antes de escribirlo en disco,
mientras que `verify` repite el pipeline de validacion para artefactos obtenidos
por otros equipos, emitiendo un resumen JSON que refleja la salida del selector
de guards en CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Cuando se proporcionan tanto el directorio de relays como la cache de guards, el
orquestador activa el gestor de ciclo de vida de circuitos para preconstruir y
renovar circuitos de SoraNet antes de cada fetch. La configuracion vive en
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) via dos campos
nuevos:

- `relay_directory`: lleva el snapshot del directorio SNNet-3 para que los hops
  middle/exit puedan seleccionarse de forma determinista.
- `circuit_manager`: configuracion opcional (habilitada por defecto) que
  controla el TTL del circuito.

Norito JSON ahora acepta un bloque `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK reenfian los datos del directorio via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), y la CLI lo conecta automaticamente siempre
que se suministre `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

El gestor renueva circuitos cuando cambian los metadatos del guard (endpoint,
clave PQ o timestamp fijado) o cuando vence el TTL. El helper `refresh_circuits`
invocado antes de cada fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs de `CircuitEvent` para que los operadores rastreen decisiones del
ciclo de vida. El soak test
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demuestra latencia estable a
traves de tres rotaciones de guards; consulta el reporte en
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

El orquestador puede iniciar opcionalmente un proxy QUIC local para que las
extensiones del navegador y los adaptadores SDK no tengan que gestionar
certificados o claves de cache de guards. El proxy se enlaza a una direccion
loopback, termina conexiones QUIC y devuelve un manifest Norito que describe el
certificado y la clave de cache de guard opcional al cliente. Los eventos de
transporte emitidos por el proxy se contabilizan via
`sorafs_orchestrator_transport_events_total`.

Habilita el proxy a traves del nuevo bloque `local_proxy` en el JSON del
orquestador:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` controla donde escucha el proxy (usa puerto `0` para solicitar
  un puerto efimero).
- `telemetry_label` se propaga a las metricas para que los dashboards puedan
  distinguir proxies de sesiones de fetch.
- `guard_cache_key_hex` (opcional) permite que el proxy exponga la misma cache
  de guards con clave que usan CLI/SDK, manteniendo alineadas las extensiones
  del navegador.
- `emit_browser_manifest` alterna si el handshake devuelve un manifest que las
  extensiones pueden almacenar y validar.
- `proxy_mode` selecciona si el proxy puentea trafico localmente (`bridge`) o
  solo emite metadatos para que los SDK abran circuitos SoraNet por su cuenta
  (`metadata-only`). El proxy por defecto es `bridge`; establece `metadata-only`
  cuando una workstation deba exponer el manifest sin reenviar streams.
- `prewarm_circuits`, `max_streams_per_circuit` y `circuit_ttl_hint_secs`
  exponen hints adicionales al navegador para que pueda presupuestar streams
  paralelos y entender que tan agresivamente el proxy reutiliza circuitos.
- `car_bridge` (opcional) apunta a una cache local de archivos CAR. El campo
  `extension` controla el sufijo agregado cuando el objetivo de stream omite
  `*.car`; establece `allow_zst = true` para servir payloads `*.car.zst`
  precomprimidos directamente.
- `kaigi_bridge` (opcional) expone rutas Kaigi en spool al proxy. El campo
  `room_policy` anuncia si el bridge opera en modo `public` o `authenticated`
  para que los clientes del navegador preseleccionen las etiquetas GAR
  correctas.
- `sorafs_cli fetch` expone overrides `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`, permitiendo alternar el modo de
  ejecucion o apuntar a spools alternativos sin modificar la politica JSON.
- `downgrade_remediation` (opcional) configura el hook de downgrade automatico.
  Cuando esta habilitado el orquestador observa la telemetria de relays para
  detectar rafagas de downgrade y, despues del `threshold` configurado dentro de
  `window_secs`, fuerza el proxy local al `target_mode` (por defecto
  `metadata-only`). Una vez que cesan los downgrades, el proxy vuelve a
  `resume_mode` despues de `cooldown_secs`. Usa el arreglo `modes` para limitar
  el trigger a roles de relay especificos (por defecto relays de entrada).

Cuando el proxy se ejecuta en modo bridge sirve dos servicios de aplicacion:

- **`norito`** – el objetivo de stream del cliente se resuelve relativo a
  `norito_bridge.spool_dir`. Los objetivos se sanitizan (sin traversal ni rutas
  absolutas) y, cuando el archivo no tiene extension, se aplica el sufijo
  configurado antes de transmitir el payload literalmente al navegador.
- **`car`** – los objetivos de stream se resuelven dentro de
  `car_bridge.cache_dir`, heredan la extension por defecto configurada y
  rechazan payloads comprimidos a menos que `allow_zst` este activado. Los
  bridges exitosos responden con `STREAM_ACK_OK` antes de transferir los bytes
  del archivo para que los clientes puedan canalizar la verificacion.

En ambos casos el proxy entrega la HMAC de cache-tag (cuando existia una clave
cache de guard durante el handshake) y registra codigos de razon de telemetria
`norito_*` / `car_*` para que los dashboards diferencien exitos, archivos
faltantes y fallos de sanitizacion de un vistazo.

`Orchestrator::local_proxy().await` expone el handle en ejecucion para que los
llamadores puedan leer el PEM del certificado, obtener el manifest del
navegador o solicitar un apagado ordenado cuando la aplicacion finaliza.

Cuando se habilita, el proxy ahora sirve registros **manifest v2**. Ademas del
certificado existente y la clave de cache de guard, v2 agrega:

- `alpn` (`"sorafs-proxy/1"`) y un arreglo `capabilities` para que los clientes
  confirmen el protocolo de stream que deben usar.
- Un `session_id` por handshake y un bloque de sal `cache_tagging` para derivar
  afinidades de guard por sesion y tags HMAC.
- Hints de circuito y seleccion de guard (`circuit`, `guard_selection`,
  `route_hints`) para que las integraciones del navegador expongan una UI mas
  rica antes de abrir streams.
- `telemetry_v2` con knobs de muestreo y privacidad para instrumentacion local.
- Cada `STREAM_ACK_OK` incluye `cache_tag_hex`. Los clientes reflejan el valor en
  el header `x-sorafs-cache-tag` al emitir solicitudes HTTP o TCP para que las
  selecciones de guard en cache permanezcan cifradas en reposo.

Estos campos preservan el formato previo; los clientes antiguos pueden ignorar
las nuevas claves y continuar usando el subconjunto v1.

## 2. Semantica de fallos

El orquestador aplica comprobaciones estrictas de capacidad y presupuestos antes
que se transfiera un solo byte. Los fallos caen en tres categorias:

1. **Fallos de elegibilidad (pre-flight).** Proveedores sin capacidad de rango,
   adverts expirados o telemetria obsoleta quedan registrados en el artefacto
   del scoreboard y se omiten en la planificacion. Los resumenes de CLI llenan
   el arreglo `ineligible_providers` con razones para que los operadores puedan
   inspeccionar el drift de gobernanza sin raspar logs.
2. **Agotamiento en tiempo de ejecucion.** Cada proveedor registra fallos
   consecutivos. Una vez que se alcanza el `provider_failure_threshold`
   configurado, el proveedor se marca como `disabled` por el resto de la sesion.
   Si todos los proveedores pasan a `disabled`, el orquestador devuelve
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Los limites estrictos se presentan como errores
   estructurados:
   - `MultiSourceError::NoCompatibleProviders` — el manifiesto requiere un tramo
     de chunks o alineacion que los proveedores restantes no pueden cumplir.
   - `MultiSourceError::ExhaustedRetries` — se consumio el presupuesto de
     reintentos por chunk.
   - `MultiSourceError::ObserverFailed` — los observadores downstream (hooks de
     streaming) rechazaron un chunk verificado.

Cada error incluye el indice del chunk problematico y, cuando esta disponible,
la razon final de fallo del proveedor. Trata estos fallos como bloqueadores de
release: los reintentos con la misma entrada reproduciran el fallo hasta que
cambien el advert, la telemetria o la salud del proveedor subyacente.

### 2.1 Persistencia del scoreboard

Cuando se configura `persist_path`, el orquestador escribe el scoreboard final
tras cada ejecucion. El documento JSON contiene:

- `eligibility` (`eligible` o `ineligible::<reason>`).
- `weight` (peso normalizado asignado para esta ejecucion).
- metadatos del `provider` (identificador, endpoints, presupuesto de
  concurrencia).

Archiva los snapshots de scoreboard junto a los artefactos de release para que
las decisiones de blacklist y rollout sigan siendo auditables.

## 3. Telemetria y depuracion

### 3.1 Metricas Prometheus

El orquestador emite las siguientes metricas via `iroha_telemetry`:

| Metrica | Labels | Descripcion |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de fetches orquestados en vuelo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma que registra la latencia de fetch de extremo a extremo. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de fallos terminales (reintentos agotados, sin proveedores, fallo del observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de intentos de reintento por proveedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de fallos a nivel de sesion que llevan a deshabilitar proveedores. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Conteo de decisiones de politica de anonimato (cumplida vs brownout) agrupadas por etapa de rollout y razon de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de la cuota de relays PQ dentro del set SoraNet seleccionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de ratios de oferta de relays PQ en el snapshot del scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma del deficit de politica (brecha entre el objetivo y la cuota PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma de la cuota de relays clasicos usada en cada sesion. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de conteos de relays clasicos seleccionados por sesion. |

Integra las metricas en dashboards de staging antes de activar los knobs de
produccion. El layout recomendado refleja el plan de observabilidad SF-6:

1. **Fetches activos** — alerta si el gauge sube sin completions equivalentes.
2. **Ratio de reintentos** — avisa cuando los contadores `retry` superan
   baselines historicos.
3. **Fallos de proveedor** — dispara alertas al pager cuando cualquier proveedor
   supera `session_failure > 0` dentro de 15 minutos.

### 3.2 Targets de logs estructurados

El orquestador publica eventos estructurados en targets deterministas:

- `telemetry::sorafs.fetch.lifecycle` — marcas de ciclo de vida `start` y
  `complete` con conteo de chunks, reintentos y duracion total.
- `telemetry::sorafs.fetch.retry` — eventos de reintento (`provider`, `reason`,
  `attempts`) para alimentar el triage manual.
- `telemetry::sorafs.fetch.provider_failure` — proveedores deshabilitados por
  errores repetidos.
- `telemetry::sorafs.fetch.error` — fallos terminales resumidos con `reason` y
  metadatos opcionales del proveedor.

Envia estos flujos al pipeline de logs Norito existente para que la respuesta a
incidentes tenga una unica fuente de verdad. Los eventos de ciclo de vida
exponen la mezcla PQ/clasica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sus contadores asociados,
lo que facilita cablear dashboards sin raspar metricas. Durante rollouts de GA,
fija el nivel de logs en `info` para eventos de ciclo de vida/reintento y usa
`warn` para fallos terminales.

### 3.3 Resumenes JSON

Tanto `sorafs_cli fetch` como el SDK de Rust devuelven un resumen estructurado
que contiene:

- `provider_reports` con conteos de exito/fracaso y si un proveedor fue
  deshabilitado.
- `chunk_receipts` detallando que proveedor resolvio cada chunk.
- arreglos `retry_stats` e `ineligible_providers`.

Archiva el archivo de resumen al depurar proveedores problematicos: los receipts
mapean directamente a los metadatos de logs anteriores.

## 4. Checklist operativa

1. **Preparar la configuracion en CI.** Ejecuta `sorafs_fetch` con la
   configuracion objetivo, pasa `--scoreboard-out` para capturar la vista de
   elegibilidad y compara con el release anterior. Cualquier proveedor ineligible
   inesperado detiene la promocion.
2. **Validar telemetria.** Asegura que el despliegue exporta metricas
   `sorafs.fetch.*` y logs estructurados antes de habilitar fetches multi-origen
   para usuarios. La ausencia de metricas suele indicar que el facade del
   orquestador no fue invocado.
3. **Documentar overrides.** Al aplicar ajustes de emergencia `--deny-provider`
   o `--boost-provider`, confirma el JSON (o la invocacion CLI) en tu changelog.
   Los rollbacks deben revertir el override y capturar un nuevo snapshot del
   scoreboard.
4. **Repetir smoke tests.** Tras modificar presupuestos de reintentos o limites
   de proveedores, vuelve a hacer fetch del fixture canonico
   (`fixtures/sorafs_manifest/ci_sample/`) y verifica que los receipts de
   chunks sigan siendo deterministas.

Seguir los pasos anteriores mantiene el comportamiento del orquestador
reproducible en rollouts por fases y proporciona la telemetria necesaria para
la respuesta a incidentes.

### 4.1 Anulaciones de politica

Los operadores pueden fijar la etapa activa de transporte/anonimato sin editar
la configuracion base estableciendo `policy_override.transport_policy` y
`policy_override.anonymity_policy` en su JSON de `orchestrator` (o suministrando
`--transport-policy-override=` / `--anonymity-policy-override=` a
`sorafs_cli fetch`). Cuando cualquiera de los overrides esta presente el
orquestador omite el fallback brownout habitual: si el nivel PQ solicitado no se
puede satisfacer, el fetch falla con `no providers` en lugar de degradar en
silencio. Volver al comportamiento por defecto es tan simple como limpiar los
campos de override.
