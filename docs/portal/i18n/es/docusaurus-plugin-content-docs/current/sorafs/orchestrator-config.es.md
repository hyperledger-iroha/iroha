---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: Configuración del orquestador de SoraFS
sidebar_label: Configuración del orquestador
descripción: Configura el orquestador de fetch multi-origen, interpreta fallos y depura la salida de telemetría.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/orchestrator.md`. Mantenga ambas copias sincronizadas hasta que se retiren los documentos heredados.
:::

# Guía del orquestador de fetch multi-origen

El orquestador de fetch multi-origen de SoraFS impulsa descargas deterministas y
paralelas desde el conjunto de proveedores publicados en anuncios respaldados por
la gobernanza. Esta guía explica cómo configurar el orquestador, que señales de
fallo esperar durante los rollouts y que flujos de telemetria exponen indicadores
de salud.

## 1. Resumen de configuración

El orquestador combina tres fuentes de configuración:| Fuente | propuesta | Notas |
|--------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza los pesos de proveedores, valida la frescura de la telemetría y persiste el marcador JSON usado para auditorias. | Respaldado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplica limites en tiempo de ejecución (presupuestos de reintentos, limites de concurrencia, toggles de verificación). | Se mapea a `FetchOptions` y `crates/sorafs_car::multi_fetch`. |
| Parámetros de CLI / SDK | Limitan el número de pares, adjuntan regiones de telemetría y exponentes políticos de deny/boost. | `sorafs_cli fetch` exponen estos flags directamente; los SDK los pasan vía `OrchestratorConfig`. |

Los helpers JSON en `crates/sorafs_orchestrator::bindings` serializan la
configuracion completa en Norito JSON, lo que la hace portatil entre enlaces de
SDK y automatización.

### 1.1 Configuración JSON de ejemplo

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
usuario, actual) para que los despliegues deterministas hereden los mismos límites
entre nodos. Para un perfil de fallback direct-only alineado con el rollout
SNNet-5a, consulta `docs/examples/sorafs_direct_mode_policy.json` y la guia
complementario en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones de cumplimientoSNNet-9 integra el cumplimiento dirigido por gobernanza en el orquestador. ONU
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
```- `operator_jurisdictions` declara los codigos ISO-3166 alpha-2 donde opera esta
  instancia del orquestador. Los codigos se normalizan a mayusculas durante el
  parseo.
- `jurisdiction_opt_outs` refleja el registro de gobernanza. Cuando alguna vez
  jurisdicción de operador aparece en la lista, el orquestador aplica
  `transport_policy=direct-only` y emite la razón de respaldo
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista digests de manifest (CID cegados, codificados en
  mayúsculas hexagonales). Las cargas coinciden tambien fuerzan la planificacion
  direct-only y exponen el fallback `compliance_blinded_cid_opt_out` en la
  telemetría.
- `audit_contacts` registra las URI que la gobernanza espera que los operadores
  publiquen en sus playbooks GAR.
- `attestations` captura los paquetes de cumplimiento firmados que respaldan la
  política. Cada entrada define una `jurisdiction` opcional (código ISO-3166
  alpha-2), un `document_uri`, el `digest_hex` canónico de 64 caracteres, el
  timestamp de emisión `issued_at_ms` y un `expires_at_ms` opcional. estos
  artefactos alimentan la checklist de auditoria del orquestador para que el
  herramientas de gobernanza pueda vincular las anulaciones con la documentación
  firmada.Proporciona el bloque de cumplimiento mediante el apilado habitual de
configuración para que los operadores reciban anulaciones deterministas. el
orquestador aplica cumplimiento _despues_ de los tips de write-mode: incluso si
un SDK solicita `upload-pq-only`, las exclusiones por jurisdicción o manifiesto
siguen forzando transporte direct-only y fallan rapido cuando no existen
proveedores aptos.

Los catálogos canónicos de opt-out viven en
`governance/compliance/soranet_opt_outs.json`; el Consejo de Gobernanza pública
actualizaciones mediante lanzamientos etiquetados. Un ejemplo completo de
configuración (incluyendo atestados) está disponible en
`docs/examples/sorafs_compliance_policy.json`, y el proceso operativo se
captura en el
[playbook de cumplimiento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI y SDK| Bandera / Campo | Efecto |
|----------------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita cuantos proveedores sobreviven al filtro del marcador. Ponlo en `None` para usar todos los proveedores elegibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limite los reintentos por trozo. Superar el límite de géneros `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Inyecta instantáneas de latencia/fallo en el constructor del marcador. La telemetria obsoleta mas alla de `telemetry_grace_secs` marca proveedores como no elegibles. |
| `--scoreboard-out` | Persiste el marcador calculado (proveedores elegibles + no elegibles) para inspección posterior. |
| `--scoreboard-now` | Sustituye la marca de tiempo del marcador (segundos Unix) para que las capturas de partidos sigan siendo deterministas. |
| `--deny-provider` / gancho de política de puntuación | Excluye deterministamente proveedores de la planificacion sin borrar anuncios. Util para listas negras de respuesta rápida. |
| `--boost-provider=name:delta` | Ajusta los créditos de round-robin ponderados para un proveedor manteniendo intactos los pesos de gobernanza. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiquetas métricas emitidas y registros estructurados para que los paneles puedan filtrar por geografía u ola de rollout. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por defecto es `soranet-first` ahora que el orquestador multi-origen es la base. Usa `direct-only` al preparar un downgrade o seguir una directiva de cumplimiento, y reserva `soranet-strict` para pilotos PQ-only; las anulaciones de cumplimiento siguen siendo el techo duro. |

SoraNet-first es ahora el valor por defecto, y los rollbacks deben citar el
bloqueador SNNet correspondiente. Tras graduarse SNNet-4/5/5a/5b/6a/7/8/12/13,
la gobernanza endurecera la postura requerida (hacia `soranet-strict`); hasta
entonces, solo las anulaciones motivadas por incidentes deben priorizar
`direct-only`, y deben registrarse en el registro de implementación.

Todos los flags anteriores aceptan sintaxis estilo `--` tanto en
`sorafs_cli fetch` como en el binario `sorafs_fetch` orientado a
desarrolladores. Los SDK exponen las mismas opciones mediante constructores tipados.

### 1.4 Gestión de caché de guardias

La CLI ahora integra el selector de guardias de SoraNet para que los operadores
puedan fijar retransmisiones de entrada de forma determinista antes del despliegue completo
SNNet-5 de transporte. Tres nuevas banderas controlan el flujo:| Bandera | propuesta |
|------|-----------|
| `--guard-directory <PATH>` | Apunta a un archivo JSON que describe el consenso de relés más reciente (se muestra un subconjunto abajo). Pasar el directorio refresca la caché de guardias antes de ejecutar el fetch. |
| `--guard-cache <PATH>` | Persiste el `GuardSet` codificado en Norito. Las ejecuciones posteriores reutilizan la caché incluso cuando no se suministra un directorio nuevo. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides opcionales para el número de guardias de entrada a fijar (por defecto 3) y la ventana de retención (por defecto 30 días). |
| `--guard-cache-key <HEX>` | Clave opcional de 32 bytes usados ​​para etiquetar cachés de guardia con un MAC Blake3 para que el archivo pueda verificarse antes de reutilizarse. |

Las cargas del directorio de guardias usan un esquema compacto:

El flag `--guard-directory` ahora espera una carga útil `GuardDirectorySnapshotV2`
codificado en Norito. La instantánea binaria contiene:- `version` — versión del esquema (actualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadatos de consenso que deben coincidir con cada certificado embebido.
- `validation_phase` — compuerta de política de certificados (`1` = permitir
  una sola firma Ed25519, `2` = preferir firmas dobles, `3` = requerir firmas
  dobles).
- `issuers` — emisores de gobernanza con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. Las huellas dactilares se calculan como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — una lista de paquetes SRCv2 (salida de
  `RelayCertificateBundleV2::to_cbor()`). Cada paquete incluye el descriptor del
  relevo, banderas de capacidad, politica ML-KEM y firmas dobles Ed25519/ML-DSA-65.

La CLI verifica cada paquete contra las claves de emisor declaradas antes de
fusionar el directorio con la caché de guardias. Los esquemas JSON heredados ya
no se aceptan; Se requieren instantáneas SRCv2.Invoca la CLI con `--guard-directory` para fusionar el consenso más reciente con
el caché existente. El selector conserva los guardias fijados que aun estan dentro
la ventana de retención y son elegibles en el directorio; los relevos nuevos
reemplazan entradas caducadas. Tras un fetch exitoso, la caché actualizada se
escriba de nuevo en la ruta indicada por `--guard-cache`, manteniendo sesiones
subsecuentes deterministas. Los SDK pueden reproducir el mismo comportamiento al
llamar `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
y pasar el `GuardSet` resultante a `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite que el selector priorice guardes con capacidad PQ
cuando se despliega SNNet-5. Los interruptores de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) ahora degradan automáticamente los relés
clasicos: cuando hay un guardia PQ disponible el selector elimina los pinos
clásicos sobrantes para que las sesiones posteriores favorezcan handshakes
hibridos. Los resúmenes de CLI/SDK exponen la mezcla resultante vía
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` y los campos complementarios de candidatos/deficit/
variación de suministro, haciendo explícitos los apagones y retrocesos clásicos.Los directorios de guardias ahora pueden incorporar un paquete SRCv2 completo vía
`certificate_base64`. El orquestador decodifica cada paquete, revalida las firmas
Ed25519/ML-DSA y conserva el certificado analizado junto a la caché de guardias.
Cuando hay un certificado presente se convierte en la fuente canónica para
claves PQ, preferencias de handshake y ponderación; los certificados vencidos
se descartan y el selector vuelve a campos heredados del descriptor. Los
certificados se propagan a la gestión del ciclo de vida de circuitos y se
exponente vía `telemetry::sorafs.guard` y `telemetry::sorafs.circuit`, que registran
la ventana de validez, las suites de handshake y si se observaron firmas dobles
para cada guardia.

Usa los ayudantes de la CLI para mantener las instantáneas sincronizadas con los
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
mientras que `verify` repite el pipeline de validación para artefactos obtenidos
por otros equipos, emitiendo un resumen JSON que refleja la salida del selector
de guards en CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitosCuando se proporciona tanto el directorio de relés como la caché de guardias, el
orquestador activa el gestor de ciclo de vida de circuitos para preconstruir y
Renueve los circuitos de SoraNet antes de cada recuperación. La configuracion vive en
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) vía dos campos
nuevos:

- `relay_directory`: lleva el snapshot del directorio SNNet-3 para que los hops
  middle/exit puedan seleccionarse de forma determinista.
- `circuit_manager`: configuración opcional (habilitada por defecto) que
  controla el TTL del circuito.

Norito JSON ahora acepta un bloque `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK reenfian los datos del directorio vía
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), y la CLI lo conecta automáticamente siempre
que se suministre `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

El gestor renueva circuitos cuando cambian los metadatos del guard (endpoint,
clave PQ o marca de tiempo fijada) o cuando vence el TTL. El ayudante `refresh_circuits`
invocado antes de cada fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs de `CircuitEvent` para que los operadores rastreen decisiones del
ciclo de vida. prueba de remojo
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demuestra latencia estable a
traves de tres rotaciones de guardias; consulta el reporte en
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC localEl orquestador puede iniciar opcionalmente un proxy QUIC local para que las
extensiones del navegador y los adaptadores SDK no tengan que gestionar
certificados o claves de caché de guardias. El proxy se enlaza a una dirección
loopback, termina conexiones QUIC y devuelve un manifiesto Norito que describe el
certificado y la clave de caché de guardia opcional al cliente. Los eventos de
transporte emitidos por el proxy se contabilizan vía
`sorafs_orchestrator_transport_events_total`.

Habilita el proxy a través del nuevo bloque `local_proxy` en el JSON del
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
```- `bind_addr` controla donde escucha el proxy (usa puerto `0` para solicitar
  un puerto efímero).
- `telemetry_label` se propaga a las métricas para que los paneles puedan
  distinguir apoderados de sesiones de fetch.
- `guard_cache_key_hex` (opcional) permite que el proxy exponga la misma caché
  de guards con clave que usan CLI/SDK, manteniendo alineadas las extensiones
  del navegador.
- `emit_browser_manifest` alterna si el handshake devuelve un manifest que las
  extensiones pueden almacenar y validar.
- `proxy_mode` selecciona si el proxy puentea trafico localmente (`bridge`) o
  solo emite metadatos para que los SDK abran circuitos SoraNet por su cuenta
  (`metadata-only`). El proxy por defecto es `bridge`; establece `metadata-only`
  cuando una workstation deba exponer el manifest sin reenviar streams.
- `prewarm_circuits`, `max_streams_per_circuit` y `circuit_ttl_hint_secs`
  exponen tips adicionales al navegador para que pueda presuponer streams
  paralelos y entender que tan agresivamente el proxy reutiliza circuitos.
- `car_bridge` (opcional) apunta a una caché local de archivos CAR. el campo
  `extension` controla el sufijo agregado cuando el objetivo de stream omite
  `*.car`; establece `allow_zst = true` para servir cargas útiles `*.car.zst`
  precomprimidos directamente.
- `kaigi_bridge` (opcional) exponen rutas Kaigi en spool al proxy. el campo`room_policy` anuncia si el bridge opera en modo `public` o `authenticated`
  para que los clientes del navegador preseleccionen las etiquetas GAR
  correctas.
- `sorafs_cli fetch` exponente anula `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`, permitiendo alternar el modo de
  ejecucion o apuntar a spools alternativas sin modificar la politica JSON.
- `downgrade_remediation` (opcional) configura el gancho de degradación automática.
  Cuando esta habilitado el orquestador observa la telemetria de relevos para
  detectar rafagas de downgrade y, despues del `threshold` configurado dentro de
  `window_secs`, fuerza el proxy local al `target_mode` (por defecto
  `metadata-only`). Una vez que cesan los downgrades, el proxy vuelve a
  `resume_mode` despues de `cooldown_secs`. Usa el arreglo `modes` para limitar
  el trigger a roles de relevo específicos (por defecto relevo de entrada).

Cuando el proxy se ejecuta en modo bridge sirve dos servicios de aplicación:- **`norito`** – el objetivo de stream del cliente se resuelve relativo a
  `norito_bridge.spool_dir`. Los objetivos se sanitizan (sin atravesar ni rutas
  absolutas) y, cuando el archivo no tiene extensión, se aplica el sufijo
  configurado antes de transmitir la carga útil literalmente al navegador.
- **`car`** – los objetivos de stream se resuelven dentro de
  `car_bridge.cache_dir`, heredan la extensión por defecto configurada y
  payloads comprimidos a menos que `allow_zst` este activado. Los
  bridges exitosos responden con `STREAM_ACK_OK` antes de transferir los bytes
  del archivo para que los clientes puedan canalizar la verificación.

En ambos casos el proxy entrega la HMAC de cache-tag (cuando existe una clave
cache de guard durante el handshake) y registra códigos de razón de telemetría
`norito_*` / `car_*` para que los tableros diferencien salidas, archivos
faltantes y fallos de sanitizacion de un vistazo.

`Orchestrator::local_proxy().await` exponen el handle en ejecucion para que los
llamadores puedan leer el PEM del certificado, obtener el manifiesto del
navegador o solicitar un apagado ordenado cuando la aplicación finaliza.

Cuando se habilita, el proxy ahora sirve registros **manifest v2**. además del
certificado existente y la clave de caché de guardia, v2 agrega:- `alpn` (`"sorafs-proxy/1"`) y un arreglo `capabilities` para que los clientes
  confirmen el protocolo de streaming que deben usar.
- Un `session_id` por handshake y un bloque de sal `cache_tagging` para derivar
  afinidades de guardia por sesión y etiquetas HMAC.
- Consejos de circuito y selección de guardia (`circuit`, `guard_selection`,
  `route_hints`) para que las integraciones del navegador expongan una UI más
  rica antes de abrir arroyos.
- `telemetry_v2` con perillas de muestreo y privacidad para instrumentación local.
- Cada `STREAM_ACK_OK` incluye `cache_tag_hex`. Los clientes reflejan el valor en
  el header `x-sorafs-cache-tag` al emitir solicitudes HTTP o TCP para que las
  Las selecciones de guardia en caché permanecerán cifradas en reposo.

Estos campos conservan el formato anterior; los clientes antiguos pueden ignorar
las nuevas claves y continuar usando el subconjunto v1.

## 2. Semántica de fallos

El orquestador aplica comprobaciones estrictas de capacidad y presupuestos antes.
que se transfiera un solo byte. Los fallos caen en tres categorias:1. **Fallos de elegibilidad (pre-flight).** Proveedores sin capacidad de rango,
   anuncios caducados o telemetria obsoleta quedan registrados en el artefacto
   del marcador y se omiten en la planificacion. Los resúmenes de CLI llenan
   el arreglo `ineligible_providers` con razones para que los operadores puedan
   inspeccionar el drift de gobernanza sin raspar logs.
2. **Agotamiento en tiempo de ejecución.** Cada proveedor registra fallos
   consecutivos. Una vez que se alcanza el `provider_failure_threshold`
   configurado, el proveedor se marca como `disabled` por el resto de la sesión.
   Si todos los proveedores pasan a `disabled`, el orquestador devuelve
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Los limites estrictos se presentan como errores
   estructurados:
   - `MultiSourceError::NoCompatibleProviders` — el manifiesto requiere un tramo
     de trozos o alineación que los proveedores restantes no pueden cumplir.
   - `MultiSourceError::ExhaustedRetries` — se consume el presupuesto de
     reintentos por trozo.
   - `MultiSourceError::ObserverFailed` — los observadores aguas abajo (ganchos de
     streaming) rechazaron un fragmento verificado.

Cada error incluye el índice del fragmento problemático y, cuando está disponible,
la razón final de fallo del proveedor. Trata estos fallos como bloqueadores de
release: los reintentos con la misma entrada reproduciran el fallo hasta que
cambien el anuncio, la telemetria o la salud del proveedor subyacente.### 2.1 Persistencia del marcador

Cuando se configura `persist_path`, el orquestador escribe el marcador final
tras cada ejecucion. El documento JSON contiene:

- `eligibility` (`eligible` o `ineligible::<reason>`).
- `weight` (peso normalizado asignado para esta ejecución).
- metadatos del `provider` (identificador, endpoints, presupuesto de
  concurrencia).

Archiva los snapshots de scoreboard junto a los artefactos de release para que
las decisiones de blacklist y rollout siguen siendo auditables.

## 3. Telemetria y depuracion

### 3.1 Métricas Prometheus

El orquestador emite las siguientes métricas vía `iroha_telemetry`:| Métrica | Etiquetas | Descripción |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de fetches orquestados en vuelo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma que registra la latencia de búsqueda de extremo a extremo. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de fallos terminales (reintentos agotados, sin proveedores, fallo del observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de intentos de reintento por proveedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de fallos a nivel de sesión que llevan a deshabilitar proveedores. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Conteo de decisiones de política de anonimato (cumplida vs brownout) agrupadas por etapa de rollout y razón de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de la cuota de relés PQ dentro del conjunto SoraNet seleccionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de ratios de oferta de relevos PQ en la instantánea del marcador. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma del déficit de política (brecha entre el objetivo y la cuota PQ real). || `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma de la cuota de relevos clásicos usados ​​en cada sesión. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de recuentos de relevos clásicos seleccionados por sesión. |

Integra las métricas en los tableros de staging antes de activar las perillas de
producción. El diseño recomendado refleja el plan de observabilidad SF-6:

1. **Fetches activos** — alerta si el calibre sube sin terminaciones equivalentes.
2. **Ratio de reintentos** — avisa cuando los contadores `retry` superan
   líneas de base históricas.
3. **Fallos de proveedor** — dispara alertas al buscapersonas cuando cualquier proveedor
   supera `session_failure > 0` dentro de 15 minutos.

### 3.2 Objetivos de registros estructurados

El orquestador público eventos estructurados en objetivos deterministas:

- `telemetry::sorafs.fetch.lifecycle` — marcas de ciclo de vida `start` y
  `complete` con conteo de fragmentos, reintentos y duración total.
- `telemetry::sorafs.fetch.retry` — eventos de reintento (`provider`, `reason`,
  `attempts`) para alimentar el manual de triaje.
- `telemetry::sorafs.fetch.provider_failure` — proveedores deshabilitados por
  errores repetidos.
- `telemetry::sorafs.fetch.error` — fallos terminales resumidos con `reason` y
  metadatos opcionales del proveedor.Envia estos flujos al pipeline de logs Norito existente para que la respuesta a
incidentes tenga una única fuente de verdad. Los eventos del ciclo de vida.
exponen la mezcla PQ/clasica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sus contadores asociados,
lo que facilita cablear tableros sin raspar métricas. Durante los lanzamientos de GA,
fija el nivel de logs en `info` para eventos de ciclo de vida/reintento y usa
`warn` para terminales de fallos.

### 3.3 Resumen JSON

Tanto `sorafs_cli fetch` como el SDK de Rust devuelven un resumen estructurado
que contiene:

- `provider_reports` con conteos de exito/fracaso y si un proveedor fue
  deshabilitado.
- `chunk_receipts` detallando que proveedor resuelve cada trozo.
- arreglos `retry_stats` e `ineligible_providers`.

Archiva el archivo de resumen al depurar proveedores problemáticos: los recibos
mapean directamente a los metadatos de logs anteriores.

## 4. Lista de verificación operativa1. **Preparar la configuracion en CI.** Ejecuta `sorafs_fetch` con la
   configuración objetivo, pasa `--scoreboard-out` para capturar la vista de
   elegibilidad y compara con el lanzamiento anterior. Cualquier proveedor no elegible
   inesperado detiene la promoción.
2. **Validar telemetria.** Asegura que el despliegue exporta métricas
   `sorafs.fetch.*` y logs estructurados antes de habilitar fetches multi-origen
   para usuarios. La ausencia de métricas suele indicar que la fachada del
   orquestador no fue invocado.
3. **Documentar overrides.** Al aplicar ajustes de emergencia `--deny-provider`
   o `--boost-provider`, confirme el JSON (o la invocación CLI) en su registro de cambios.
   Los rollbacks deben revertir el override y capturar una nueva instantánea del
   marcador.
4. **Repetir pruebas de humo.** Tras modificar presupuestos de reintentos o límites
   de proveedores, vuelve a hacer fetch del accesorio canónico
   (`fixtures/sorafs_manifest/ci_sample/`) y verifica que los recibos de
   los trozos siguen siendo deterministas.

Seguir los pasos anteriores mantiene el comportamiento del orquestador
reproducible en implementaciones por fases y proporciona la telemetría necesaria para
la respuesta a incidentes.

### 4.1 Anulaciones de políticaLos operadores pueden fijar la etapa activa de transporte/anonimato sin editar
la configuracion base estableciendo `policy_override.transport_policy` y
`policy_override.anonymity_policy` en su JSON de `orchestrator` (o suministrando
`--transport-policy-override=` / `--anonymity-policy-override=` un
`sorafs_cli fetch`). Cuando cualquiera de los overrides esta presente el
El orquestador omite el fallo de caída habitual: si el nivel PQ solicitado no se
puede satisfacer, el fetch falla con `no providers` en lugar de degradar en
silencio. Volver al comportamiento por defecto es tan simple como limpiar los
campos de anulación.