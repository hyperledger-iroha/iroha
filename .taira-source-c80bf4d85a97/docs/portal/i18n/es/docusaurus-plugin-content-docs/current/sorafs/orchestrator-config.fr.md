---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: Configuración del orquestador SoraFS
sidebar_label: Configuración del orquestador
Descripción: Configure el orquestador de búsqueda de múltiples fuentes, interprete los echecs y deboguer la telemetría.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/orchestrator.md`. Guarde las dos copias sincronizadas hasta que la documentación heredada esté retirada.
:::

# Guía del orquestador de búsqueda de fuentes múltiples

El orquestador de búsqueda multifuente de SoraFS piloto de descargas
paralelos y determinantes después del conjunto de proveedores publicado en el
anuncios soutenus par la gouvernance. Configurador de comentarios explique de la guía CE
El orquestador, quels signaux d'échec asistirán durante los lanzamientos y quels
flujo de télémétrie expuesto de indicadores de salud.

## 1. Vista del conjunto de configuración

El orquestador fusiona tres fuentes de configuración:| Fuente | Objetivo | Notas |
|--------|----------|-------|
| `OrchestratorConfig.scoreboard` | Normalice los pesos de los proveedores, valide el envío de la televisión y persista el marcador JSON utilizado para las auditorías. | Adossé à `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplicación de límites de ejecución (presupuestos de reintento, pagos de concurrencia, bases de verificación). | Mappé à `FetchOptions` en `crates/sorafs_car::multi_fetch`. |
| Parámetros CLI / SDK | Limite el nombre de los pares, agregue las regiones de televisión y exponga las políticas de denegación/impulso. | `sorafs_cli fetch` expone estas banderas directamente; El SDK lo propaga a través de `OrchestratorConfig`. |

Los ayudantes JSON en `crates/sorafs_orchestrator::bindings` serializan la
configuración completa en Norito JSON, el SDK portátil entre enlaces y
automatización.

### 1.1 Ejemplo de configuración JSON

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

Persiste el archivo a través del uso habitual `iroha_config` (`defaults/`, usuario,
actual) a fin de que los despliegues determinados hereden los mismos límites en
les nœuds. Para un perfil de respuesta solo directo alineado en el despliegue SNNet-5a,
consulte `docs/examples/sorafs_direct_mode_policy.json` et les indicaciones
asociados en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones de conformidadSNNet-9 integra la conformidad pilotada por la gobernanza del orquestador.
Un nuevo objeto `compliance` en la configuración Norito archivos de captura JSON
Carve-outs que fuerzan la canalización de recuperación en modo directo solamente:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara los códigos ISO‑3166 alpha‑2 sobre esta operación
  instancia del orquestador. Los códigos son normalizados en majuscules lors du
  analizando.
- `jurisdiction_opt_outs` refleja el registro de gobierno. Lorsqu'une
  jurisdicción operador aparato en la lista, el orquestador impone
  `transport_policy=direct-only` y abrió la razón de respaldo
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista los resúmenes de manifiesto (CID enmascarados, codificados en
  mayúscula hexadecimal). Les payloads corresponsales forcent aussi une
  Planificación solo directa y exposición del respaldo.
  `compliance_blinded_cid_opt_out` dans la télémétrie.
- `audit_contacts` registrar el URI que el gobierno atiende a los operadores
  dans leurs playbooks GAR.
- `attestations` captura los paquetes de conformidad firmados que justifican la
  política. Cada entrada define un `jurisdiction` opcional (código ISO‑3166
  alpha‑2), un `document_uri`, el `digest_hex` canónico de 64 caracteres, el
  marca de tiempo de emisión `issued_at_ms` y un `expires_at_ms` opcional. ces
  artefactos alimentan la lista de verificación de auditoría del orquestador afin que les
  Las herramientas de gobierno pueden confiar en las anulaciones de los documentos firmados.Fournissez le bloc de conformité via l’empilement habituel de configuración pour
que los operadores detectan las anulaciones determinadas. El orquestador
Aplique la conformidad _après_ a las sugerencias del modo de escritura: también si exige un SDK
`upload-pq-only`, las exclusiones de jurisdicción o de actos basculantes manifiestos
vers le transport direct-only et échouent rapidement lorsqu'aucun fournisseur
conforme n'existe.

Les catalogues canonique d'opt-out résident dans
`governance/compliance/soranet_opt_outs.json`; el Consejo de Gobierno Público
les mises à jour via des releases taggées. Un ejemplo de configuración completa
(incluant les attestations) est disponible dans
`docs/examples/sorafs_compliance_policy.json`, y el proceso operativo está
capturado en le
[libro de jugadas de conformidad GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Parámetros CLI y SDK| Bandera / Campeón | Efecto |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limite el número de proveedores que pasan el filtro del marcador. Mettez `None` para utilizar todos los proveedores elegibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Cierre los reintentos por fragmento. Pase el límite de apertura `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Inyectar instantáneas de latencia/échec en el constructor del marcador. Un télémétrie perimée au-delà de `telemetry_grace_secs` hará que los proveedores no sean elegibles. |
| `--scoreboard-out` | Continúe con el cálculo del marcador (proveedores elegibles + no elegibles) para la inspección posterior a la ejecución. |
| `--scoreboard-now` | Recarga la marca de tiempo del marcador (segundos Unix) para guardar las capturas de dispositivos determinados. |
| `--deny-provider` / gancho de política de puntuación | Excluye a los proveedores de métodos determinados sin suprimir los anuncios. Útil para una lista negra como respuesta rápida. |
| `--boost-provider=name:delta` | El ajuste de los créditos de todos contra todos se ponderará por un proveedor sin contacto con los pesos de gobierno. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Etiqueta las métricas y registros estructurados para que los paneles de control puedan pivotar por geografía o vague de implementación. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por defecto, `soranet-first` mantiene que el orquestador de múltiples fuentes es la base. Utilice `direct-only` para una degradación o una directiva de conformidad y reserve `soranet-strict` aux pilotes PQ-only; les overrides de conformité restent le plafond dur. |

SoraNet-first está alterando el modo de vida predeterminado y las reversiones deben citarse
Corresponsal de Bloquant SNNet. Después de la graduación de SNNet-4/5/5a/5b/6a/7/8/12/13,
la gouvernance durcira la postura requerida (vers `soranet-strict`) ; d'ici là, les
seuls anula los motivos por incidente doivent privilegier `direct-only`, et ils
Sont à consignar dans le log de rollout.

Todas las banderas aceptan la sintaxis `--` en `sorafs_cli fetch` y
Desarrolladores orientados al binario `sorafs_fetch`. El SDK muestra las mismas opciones
a través de los tipos de constructores.

### 1.4 Gestión del caché de guardias

El cable CLI activa el selector de guardias SoraNet para permitir el acceso
operadores de épingler los relés de entrada de manera determinada antes de
Implementación completa del transporte SNNet-5. Tres nuevas banderas controlan este flujo:| Bandera | Objetivo |
|------|----------|
| `--guard-directory <PATH>` | Pointe ver un archivo JSON que indica el consenso de relés más recientes (sous-ensemble ci-dessous). Pase el directorio rafraîchit le cache des guards antes de la ejecución de fetch. |
| `--guard-cache <PATH>` | Mantenga la codificación `GuardSet` en Norito. Las ejecuciones posteriores reutilizarán el caché incluso sin el nuevo directorio. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Anula las opciones para el nombre de los guardias de entrada al épingler (por defecto 3) y la ventana de retención (por defecto 30 días). |
| `--guard-cache-key <HEX>` | Utilice una opción de 32 octetos para etiquetar los cachés de seguridad con un MAC Blake3 para verificar el archivo antes de su reutilización. |

Las cargas útiles del directorio de guardias utilizan un esquema compacto:

La bandera `--guard-directory` asiste a una carga útil `GuardDirectorySnapshotV2`
codificado en Norito. El contenido binario de la instantánea:- `version` — versión del esquema (actualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  Metadonnées de consenso devant correspondre à cada certificado integrado.
- `validation_phase` — puerta de política de certificados (`1` = autoriser une
  sola firma Ed25519, `2` = preferir las firmas dobles, `3` = exigir
  des firmas dobles).
- `issuers` — emisores de gobierno con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. Las huellas dactilares son calculadas como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — una lista de paquetes SRCv2 (salida
  `RelayCertificateBundleV2::to_cbor()`). El paquete Chaque incluye el descriptor del
  relevo, banderas de capacidad, política ML-KEM y firmas dobles
  Ed25519/ML-DSA-65.

La CLI verifica cada paquete con las claves de emisores declaradas antes de
Fusionar el directorio con el caché de guardias. Les esquisses JSON heritées ne
sont plus aceptados ; Las instantáneas SRCv2 son necesarias.Appelez la CLI con `--guard-directory` para fusionar el consenso más
récent avec le cache existente. El seleccionador conserva los guardias épinglés nuevamente
válidos en la ventana de retención y elegibles en el directorio; les
nuevos relés reemplazan los platos principales caducados. Après un fetch réussi, le cache
mis à jour est réécrit dans le chemin fourni via `--guard-cache`, gardant les
sesiones suivantes determinantes. El SDK reproduce el mismo comportamiento en
recurrente `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
et injectant le `GuardSet` resultante en `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite seleccionar la prioridad de las protecciones capaces de PQ
Colgante del despliegue SNNet-5. Los interruptores de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) degradan automáticamente los archivos
Relés clásicos: cuando un protector PQ está disponible, el seleccionador suprime los
pines clásicos en trop afin que las sesiones siguientes favorezcan las
híbridos de apretones de manos. Los currículums CLI/SDK exponen la mezcla resultante a través de
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` et les champs associés de candidats/déficit/delta de
suministro, alquiler explícito de apagones y retrocesos clásicos.Los directorios de guardias pueden sufrir desorción al embarcar un paquete SRCv2 completo a través de
`certificate_base64`. L'orchestreur décode cada paquete, revalide les
firmas Ed25519/ML-DSA y conservar el certificado analizado en el caché
de guardias. Lorsqu'un certificado está presente, il devient la source canonique pour
las claves PQ, las preferencias de apretón de manos y la ponderación; los certificados
Expirés sont écartés et le seleccionador revient aux champs herités du descripteur.
Les certificados se propagan en la gestión del ciclo de vida de los circuitos y
sont expuestos a través de `telemetry::sorafs.guard` y `telemetry::sorafs.circuit`, aquí
envíe la ventana de validación, las suites de apretón de manos y la observación o
No hay dobles firmas para cada guardia.

Utilice los ayudantes CLI para guardar las instantáneas alineadas con los editores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` descargue y verifique la instantánea SRCv2 antes de escribir en el disco,
tandis que `verify` rejoue le pipeline de validation pour des artefactos issus
de otros equipos, agregue un currículum JSON que refleje la selección del seleccionador
de guardias CLI/SDK.

### 1.5 Gestionador del ciclo de vida de los circuitosLorsque le directorio de retransmisiones y el caché de guardias sont fournis, el orquestador
Active el gestor del ciclo de vida de los circuitos para la preparación y la construcción.
Renovar los circuitos SoraNet antes de buscar. La configuración se encuentra
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) vía dos
nuevos campeones:

- `relay_directory`: transporta la instantánea del directorio SNNet-3 para que les
  saltea medio/salida de manera seleccionada de manera determinada.
- `circuit_manager`: configuración opcional (activada por defecto) que controla el
  TTL de circuitos.

Norito JSON acepta un bloque desormado `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

El SDK transmite los datos del directorio a través de
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), y la CLI del cable automático que
`--guard-directory` está disponible (`crates/iroha_cli/src/commands/sorafs.rs:365`).

El gestor renueva los circuitos cuando los metadonnées de guard changent
(punto final, clave PQ o marca de tiempo de envío) o cuando caduque el TTL. El ayudante
`refresh_circuits` llamada antes de buscar (`crates/sorafs_orchestrator/src/lib.rs:1346`)
Emet des logs `CircuitEvent` para que los operadores puedan rastrear las decisiones
Liées au Cycle de vie. prueba de remojo
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) Demostrar una latencia estable
sur trois rotaciones de guardias; ver la relación asociada en
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC localEl orquestador puede lanzar opcionalmente un proxy QUIC local según el
El navegador de extensiones y los adaptadores SDK no permiten gestionar los certificados.
ni les clés du cache de guards. El proxy se encuentra en una dirección de bucle invertido y finaliza
las conexiones QUIC y enviar un manifiesto Norito decrivant le certificado et la
clé de cache de guard optionnelle au client. Los eventos de transporte emitidos por
El proxy se cuenta a través de `sorafs_orchestrator_transport_events_total`.

Active el proxy a través del nuevo bloque `local_proxy` en el JSON del orquestador:

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
```- `bind_addr` controla la dirección de escucha del proxy (utiliza el puerto `0` para
  demander un port éphémère).
- `telemetry_label` se propaga aux métriques para que los tableros se distingan
  los proxy de las sesiones de recuperación.
- `guard_cache_key_hex` (opcional) permite al proxy de exposición el mismo caché de
  Guarda en memoria el CLI/SDK, manteniendo las extensiones alineadas del navegador.
- `emit_browser_manifest` bascule le retour d’un manifest que les extensions
  peuvent stocker et valider.
- `proxy_mode` seleccione si el proxy relaja la ubicación de tráfico (`bridge`) o
  No hay metadones para que el SDK se abra junto con los circuitos.
  SoraNet (`metadata-only`). El proxy está por defecto en `bridge`; elección
  `metadata-only` cuando un poste debe exponer el manifiesto sin transmitir el flujo.
- `prewarm_circuits`, `max_streams_per_circuit` y `circuit_ttl_hint_secs`
  exponer sugerencias complementarias en el navegador para ajustar el flujo
  paralelos y comprendiendo el nivel de reutilización de circuitos.
- `car_bridge` (opcional) apunta a un caché local de archivos CAR. El campeón
  `extension` controla el sufijo añadido cuando el cible omet `*.car`; definir
  `allow_zst = true` para servir directamente las cargas útiles `*.car.zst` precomprimidas.
- `kaigi_bridge` (opcional) expone las rutas que Kaigi almacena en el proxy. El campeón`room_policy` anuncia si el puente funciona en modo `public` o
  `authenticated` para que el navegador de los clientes preseleccione las etiquetas
  Apropiados por GAR.
- `sorafs_cli fetch` expone archivos anulados `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`, permiten cambiar el modo de ejecución
  O puntero a carretes alternativos sin modificar la política JSON.
- `downgrade_remediation` (opcional) configura el gancho de degradación automática.
  Cuando esté activo, el orquestador vigilará la televisión de retransmisiones para
  detector de rafales de downgrade y, después de pasar de `threshold` en la
  Ventana `window_secs`, forzar la versión local del proxy `target_mode` (por defecto
  `metadata-only`). Une fois les downgrades stoppés, le proxy revient au
  `resume_mode` después de `cooldown_secs`. Utilice la tabla `modes` para limitar
  le déclencheur à des rôles de Relay Spécifiques (por defecto les Relays d’entrée).

Cuando el torneo proxy está en modo puente, aparecen dos aplicaciones de servicios:- **`norito`** – la cible de flux du client est résolue par rapport à
  `norito_bridge.spool_dir`. Les cibles sont sanitizées (pas de traversal, pas de
  chemins absolus), y cuando un archivo no tenga extensión, el sufijo configurado
  Está aplicado antes de la transmisión de la carga útil al ver el navegador.
- **`car`** – los cibles de flujo se resuelven en `car_bridge.cache_dir`, heredado
  de la extensión configurada por defecto y rechazando las cargas útiles comprimidas
  si `allow_zst` está activado. Les bridges réussis répondent avec `STREAM_ACK_OK`
  antes de transferir los octetos del archivo hasta que los clientes puedan
  tubería la verificación.

En los dos casos, el proxy activa el HMAC de la etiqueta de caché (cuando una clave de caché
était présente lors du handshake) y registre los códigos de razón de la télémétrie
`norito_*` / `car_*` para que los paneles de control distingan un golpe de efecto
Succès, les fichiers manquants et les échecs de higienización.

`Orchestrator::local_proxy().await` expone el mango en curso para que les
Los apelantes pueden leer el PEM del certificado y recuperar el manifiesto del navegador.
O exige un arresto gratuito por el cierre de la aplicación.

Cuando el proxy está activado, se desactivan los registros **manifest v2**.
Además del certificado existente y de la clave de caché de guardia, la v2 agrega:- `alpn` (`"sorafs-proxy/1"`) y un cuadro `capabilities` para los clientes
  Confirme el protocolo de flujo de uso.
- Un `session_id` por apretón de manos y un bloque de salario `cache_tagging` para derivar
  des affinités de guard par session et des tags HMAC.
- Des consejos de circuito y selección de guardia (`circuit`, `guard_selection`,
  `route_hints`) para que las integraciones del navegador expongan una interfaz de usuario más rica
  avant l'ouverture des flux.
- `telemetry_v2` con botones de échantillonnage y confidencialidad para
  l'localización de instrumentación.
- Chaque `STREAM_ACK_OK` incluido `cache_tag_hex`. Los clientes reflejan el valor
  en la línea `x-sorafs-cache-tag` para solicitudes HTTP o TCP según las
  Selecciones de guardia en caché restent chiffrées au repos.

Estos campeones están incluidos en el manifiesto v2; los clientes no consumen
explicitement les clés qu’ils supportent et ignorar le reste.

## 2. Sémantique des échecs

El orquestador aplica las verificaciones estrictas de capacidades y presupuestos.
avant de transférer le moindre octet. Les échecs tombent dans tres categorías:1. **Échecs d’éligibilité (pré-vol).** Les fournisseurs sans capacité de plage,
   Los anuncios auxiliares caducados o en la télémétrie périmée sont consignados dans l’artefact
   du scoreboard et omis de la planification. Los currículums CLI remplissent le
   tableau `ineligible_providers` con las razones según los operadores
   inspectent les derivas de gouvernance sans scraper les logs.
2. **Épuisement à l’exécution.** Chaque fournisseur suit les échecs consécutifs.
   Une fois `provider_failure_threshold` atteint, le fournisseur est marqué
   `disabled` para el resto de la sesión. Si todos los proveedores deviennent
   `disabled`, el orquestador enviado
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forma d’erreurs
   estructurados :
   - `MultiSourceError::NoCompatibleProviders` — le manifest requiert un span de
     trozos o una alineación que les fournisseurs restants ne peuvent honorer.
   - `MultiSourceError::ExhaustedRetries` — el presupuesto de reintentos por fragmento a été
     consomé.
   - `MultiSourceError::ObserverFailed` — les observateurs downstream (ganchos de
     streaming) ont rejeté un fragmento verificado.

Cada error al embarcar el índice del trozo fautif y, cuando esté disponible, la razón
finale d'échec du fournisseur. Traitez ces erreurs como bloqueadores de liberación
— les reintries avec la même entrée reproduiront l’échec tant que l’advert, la
La telemetría o la salud del proveedor sous-jacent no cambian.### 2.1 Persistencia del marcador

Cuando `persist_path` está configurado, el orquestador escribe el marcador final
Después de cada carrera. El contenido del documento JSON:

- `eligibility` (`eligible` o `ineligible::<reason>`).
- `weight` (podes normalizados asignados para esta ejecución).
- métadonnées du `provider` (identificador, puntos finales, presupuesto de concurrencia).

Archivez les snapshots de scoreboard avec les artefactos de release afin que les
elección de listas negras y de implementación de restantes auditables.

## 3. Telemétrie et débogage

### 3.1 Métricas Prometheus

El orquestador conoció las métricas siguientes a través de `iroha_telemetry`:| Métrico | Etiquetas | Descripción |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge des fetches orquestrés en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | El histograma registra la latencia de búsqueda de combate en combate. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (retries épuisés, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Compteur des tentatives de retry par fournisseur. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de fournisseur au niveau session menant à la desactivation. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Cuenta de decisiones políticas anónimas (tenue vs brownout) agrupadas por etapa de implementación y razón de respaldo. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de la parte de los relés PQ en el conjunto SoraNet seleccionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de proporciones de oferta de relés PQ en la instantánea del marcador. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma del déficit de política (écart entre l'objectif et la part PQ réelle). || `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma de la parte de relés clásicos utilizados en cada sesión. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de los ordenadores de relés clásicos seleccionados por sesión. |

Intégrez ces métriques dans les Dashboards de Staging antes de activar las perillas.
en producción. La disposición recomendada refleja el plan de observación SF-6:

1. **Obtiene actifs** — alerta si le calibre monte sin terminaciones correspondientes.
2. **Ratio de reintentos** — evita que los ordenadores `retry` pasen
   líneas de base históricas.
3. **Échecs fournisseurs** — declenche les alertes pager quand un fournisseur
   pase `session_failure > 0` en una ventana de 15 minutos.

### 3.2 Cibles de troncos estructurados

El orquestador público de eventos estructurados frente a cibles determinados:

- `telemetry::sorafs.fetch.lifecycle` — marcas `start` y `complete` con
  nombre de fragmentos, reintentos y duración total.
- `telemetry::sorafs.fetch.retry` — eventos de reintento (`provider`, `reason`,
  `attempts`) para el análisis manual.
- `telemetry::sorafs.fetch.provider_failure` — proveedores desactivados para
  errores repetidos.
- `telemetry::sorafs.fetch.error` — échecs terminaux résumés avec `reason` et
  métadonnées optionnelles du fournisseur.Acheminez ces flux vers le pipeline de logs Norito existe afin que la respuesta
Aux incidentes son una fuente única de verdad. Los eventos del ciclo de vida.
exponent le mix PQ/classique vía `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sus ordenadores asociados,
Esto facilita el cableado de los salpicaderos sin raspador de medidas. Colgante
Los despliegues GA, bloquean el nivel de registros en `info` para los eventos de ciclo.
de vie/retry et utilisez `warn` pour les erreurs terminales.

### 3.3 Currículums JSON

`sorafs_cli fetch` y el SDK Rust envían un currículum estructurado con el siguiente contenido:

- `provider_reports` con los ordenadores exitosos/échec y el estado de desactivación.
- `chunk_receipts` Détaillant quel fournisseur a satisfait cada trozo.
- los cuadros `retry_stats` y `ineligible_providers`.

Archivez le fichier de résumé pour deboguer les fournisseurs défaillants: les
recibos mappent directement aux métadonnées de logs ci-dessus.

## 4. Lista de verificación operativa1. **Prepare la configuración en CI.** Lancez `sorafs_fetch` con la
   configuración cible, pase `--scoreboard-out` para capturar la vista
   d'éligibilité, et faites un diff avec le release précédent. Todo facilitador
   inelegible inattendu bloquea la promoción.
2. **Valider la télémétrie.** Asegúrese de que el despliegue exporte les
   Métricas `sorafs.fetch.*` y los registros estructurados antes de activar las búsquedas
   múltiples fuentes para los usuarios. La ausencia de medidas indica souvent
   que la fachada del orquestador n'a pas été appelée.
3. **Registrar las anulaciones.** Lors d'un `--deny-provider` o `--boost-provider`
   En caso de urgencia, registre el JSON (o la CLI de invocación) en el registro de cambios. les
   las reversiones deben activar la anulación y capturar una nueva instantánea de
   marcador.
4. **Relancer les smoke tests.** Después de modificar los presupuestos de reintentar o des
   caps de fournisseurs, rebuscador de la fijación canónica
   (`fixtures/sorafs_manifest/ci_sample/`) y verifique que los recibos de trozos
   restent déterministes.

Suivre les étapes ci-dessus garde le comportement de l’orchestrateur
reproducible en los despliegues en fases y proporciona la télémétrie nécessaire à
la respuesta a los incidentes.

### 4.1 Anulaciones políticasLos operadores pueden interrumpir la fase de transporte/anónimo activo sin
modificar la configuración de base en definición
`policy_override.transport_policy` y `policy_override.anonymity_policy` en
leur JSON `orchestrator` (o un proveedor
`--transport-policy-override=` / `--anonymity-policy-override=` a
`sorafs_cli fetch`). Cuando hay una anulación presente, el orquestador saltea el
hábito de caída de tensión alternativa: si el nivel PQ demandé ne peut pas être satisfait,
le fetch échoue con `no providers` en lugar de degradar el silencio. le
El retorno al comportamiento por defecto consiste simplemente en ver los campos.
anular.