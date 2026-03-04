---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orchestrator-config
כותרת: Configuracion del orquestador de SoraFS
sidebar_label: Configuracion del orquestador
תיאור: Configura el orquestador de fetch multi-origen, interpreta fallos y depura la salida de telemetria.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/sorafs/developer/orchestrator.md`. Manten ambas copias sincronizadas hasta que se retiren los docs heredados.
:::

# Guia del orquestador de להביא ריבוי מקורות

El orquestador de fetch multi-origen de SoraFS impulsa descargas deterministas y
paralelas desde el conjunto de proveedores publicado en adverts respaldados por
לה גוברננסה. Esta guia explica como configurar el orquestador, que senales de
fallo esperar durante los rollouts y que flujos de telemetria exponen indicadores
דה סאלוד.

## 1. קורות חיים להגדרה

El orquestador combina tres fuentes de configuracion:

| פואנטה | פרופוזיטו | Notas |
|--------|--------|-------|
| `OrchestratorConfig.scoreboard` | נורמליזה לוס פסו דה פרועדורס, valida la frescura de la telemetria y persiste el לוח התוצאות JSON usado para auditorias. | Respaldado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplica limites en tiempo de ejecucion (presupuestos de reintentos, limites de concurrencia, toggles de verificacion). | ראה מפה של `FetchOptions` ו-`crates/sorafs_car::multi_fetch`. |
| Parametros de CLI / SDK | מגבלת מספר עמיתים, אזורי עזר טלמטריה והסברה פוליטית של הכחשה/חיזוק. | `sorafs_cli fetch` expone estos flags directamente; los SDK los pasan דרך `OrchestratorConfig`. |

Los helpers JSON en `crates/sorafs_orchestrator::bindings` serializan la
configuracion completa en Norito JSON.
SDK y automatizacion.

### 1.1 תצורת JSON דוגמה

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
משתמש, בפועל) para que los despliegues deterministas hereden los mismos limites
entre nodos. להורדה ישירה בלבד של תוכנית החלפה ישירה בלבד
SNNet-5a, consulta `docs/examples/sorafs_direct_mode_policy.json` y la guia
complementaria en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones de cumplimiento

SNNet-9 integra el cumplimiento dirigido por gobernanza en el orquestador. Un
nuevo objeto `compliance` en la configuracion Norito JSON captura los carve-outs
que fuerzan el pipeline להביא מוד ישיר בלבד:

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
- `jurisdiction_opt_outs` refleja el registro de gobernanza. קואנדו אלגונה
  שיפוט המפעיל aparece en la lista, el orquestador aplica
  `transport_policy=direct-only` y emite la razon de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` רשימה תקצירי המניפסט (CIDs cegados, codificados en
  hex mayusculas). Las cargas coincidentes tambien fuerzan la planificacion
  ישיר בלבד y exponen el fallback `compliance_blinded_cid_opt_out` en la
  טלמטריה.
- `audit_contacts` registra las URI que la gobernanza espera que los operadores
  publiquen en sus playbooks GAR.
- `attestations` captura los paquetes de cumplimiento firmados que respaldan la
  פוליטיקה. Cada entrada define una `jurisdiction` אופציונלי (קודיגו ISO-3166
  alpha-2), un `document_uri`, el `digest_hex` canonico de 64 caracters, el
  חותמת זמן של emision `issued_at_ms` y un `expires_at_ms` אופציונלי. אסטוס
  artefactos alimentan la checklist de auditoria del orquestador para que el
  tooling de gobernanza pueda vincular las anulaciones con la documentacion
  פיראדה.

Proporciona el bloque de compliance mediante el apilado habitual de
Configuracion para que los operadores reciban anulaciones deterministas. אל
תאימות orquestador aplica _despues_ de los רמזים למצב כתיבה: incluso si
un SDK solicita `upload-pq-only`, הסתייגויות לפי שיפוט
siguen forzando transporte ישירות בלבד y fallan rapido cuando no existen
proveedores aptos.

Los catalogos canonicos de opt-out viven en
`governance/compliance/soranet_opt_outs.json`; el Consejo de Gobernanza publica
האקטואליזציונות המדיאנטים משחררים כללי התנהגות. Un emplo completo de
configuracion (כולל עדויות) esta disponible en
`docs/examples/sorafs_compliance_policy.json`, y el processo operativo se
captura en el
[Playbook de cumplimiento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI y SDK| דגל / קמפו | אפקטו |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita cuantos proveedores sobreviven al filtro del לוח התוצאות. Ponlo en `None` עבור שימוש בהישגים מתאימים. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita los reintentos por chunk. Superar el limite genera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Inyecta תצלומי מצב של עכבות/פלות בלוח התוצאות. La telemetria obsoleta mas alla de `telemetry_grace_secs` מארקה מוכיחה כמו פסולים. |
| `--scoreboard-out` | המשך חישוב לוח התוצאות (מוכיח את הזכאים + הבלתי זכאים) לבדיקה האחורית. |
| `--scoreboard-now` | חותמת זמן של לוח התוצאות (Segundos Unix) עבור מתקנים וסימנים. |
| `--deny-provider` / הוק דה פוליטיקה דה ניקוד | Excluye deterministamente proveedores de la planificacion sin borrar adverts. השתמש ב-para lists negras de respuesta rapida. |
| `--boost-provider=name:delta` | Ajusta los creditos de round-robin ponderado para un proveedor manteniendo intactos los pesos de gobernanza. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | כללי כללי emitidas y logs estructurados para que los dashboards puedan filtrar por geographa u ola de rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Por defecto es `soranet-first` ahora que el orquestador multi-origen es la base. ארה"ב `direct-only` על הכנה לירידה בדירוג או מפרט את ההנחיות, ו-Reserva `soranet-strict` עבור פיילוט PQ בלבד; las anulaciones de compliance siguen siendo el techo duro. |

SoraNet-first es ahora el valor por defecto, y los rollbacks deben citar el
bloqueador SNNet correspondiente. Tras graduarse SNNet-4/5/5a/5b/6a/7/8/12/13,
la gobernanza endurecera la postura requerida (hacia `soranet-strict`); hasta
entonces, solo las anulaciones motivadas por incidentes deben priorizar
`direct-only`, y deben registrarse in el log de rollout.

Todos los flags anteriores aceptan sintaxis estilo `--` tanto en
`sorafs_cli fetch` como en el binario `sorafs_fetch` orientado a
desarrolladores. Los SDK exponen las mismas opciones mediante builders tipados.

### 1.4 Gestion de cache de guards

La CLI ahora integra el selector de guards de SoraNet עבור מפעילים
ממסרים של puedan fijar de entrada de forma determinista antes del rollout completo
SNNet-5 de transporte. Tres nuevos flags controlan el flujo:| דגל | פרופוזיטו |
|------|--------|
| `--guard-directory <PATH>` | Apunta un archivo JSON que לתאר את קונסנסו דה ממסרים mas reciente (se muestra un subconjunto abajo). Pasar el directorio refresca la cache de guards antes de ejecutar el fetch. |
| `--guard-cache <PATH>` | Persiste el `GuardSet` codificado en Norito. Las ejecuciones posteriores reutilizan la cache incluso cuando no se suministra un directorio nuevo. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | עוקף את האופציות על מספר השומרים דה אנטרדה א פיאר (פור defecto 3) y la ventana de retencion (por defecto 30 dias). |
| `--guard-cache-key <HEX>` | Clave אופציונלי של 32 בתים ארה"ב עבור מטמונים כלליים של גארד עם MAC Blake3 עבור אימות ארכיון קודים מחדש. |

Las cargas del directorio de guards usan un esquema compacto:

El flag `--guard-directory` ahora espera unload pay `GuardDirectorySnapshotV2`
codificado en Norito. התוכן הבינארי של תמונת מצב:

- `version` - גרסה של esquema (actualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadatos de consenso que deben coincidir con cada certificado embebido.
- `validation_phase` — מחשב פוליטיקה של תעודות (`1` = אישורים
  una sola firma Ed25519, `2` = מעדיף פירמס כפולים, `3` = דורשים חברות
  כפולים).
- `issuers` — emisores de gobernanza con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. טביעות אצבע משולבות כמו
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` - רשימה של חבילות SRCv2 (סלידה דה
  `RelayCertificateBundleV2::to_cbor()`). צרור קאדה כולל תיאור חלק
  ממסר, flags de capacidad, politica ML-KEM y firmas dobles Ed25519/ML-DSA-65.

La CLI verifica cada צרור contra las claves de emisor declaradas antes de
fusionar el directorio con la cache de guards. סביר להניח ש-JSON נהנה
no se aceptan; יש צורך בתמונות SRCv2.

Invoca la CLI con `--guard-directory` para fusionar el consenso mas reciente con
la cache existente. El Selector Conserva Los Guards Fijados que Aun Estan Dentro
la ventana de retencion y son elegibles en el directorio; los relays nuevos
reemplazan entradas expiradas. Tras un fetch exitoso, la cache actualizada se
escribe de nuevo en la ruta indicada por `--guard-cache`, manteniendo sesiones
subsecuentes deterministas. Los SDK pueden reproducerer el mismo comportamiento al
llamar `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
y pasar el `GuardSet` resultante a `SorafsGatewayFetchOptions`.`ml_kem_public_hex` מתיר que el selector priorice guards con capacidad PQ
cuando se despliega SNNet-5. Los toggles de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) ahora degradan automaticamente relays
קלאסיקות: cuando hay un guard PQ disponible el selector elimina los pines
clasicos sobrantes para que las sesiones posteriores favorezcan לחיצות ידיים
היברידיות. Los resumenes de CLI/SDK exponen la mezcla resultante via
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` y los campos complementarios de candidatos/deficit/
variacion de supply, haciendo explicitos los brownouts y fallbacks clasicos.

Los directorios de guards ahora pueden incorporar un bundle SRCv2 completo via
`certificate_base64`. El orquestador decodifica cada bundle, revalida las firmas
Ed25519/ML-DSA y conserva el certificado analizado junto a la cache de guards.
Cuando hay un certificado presente se convierte en la fuente canonica para
claves PQ, preferences de לחיצת יד y ponderacion; los certificados expirados
se descartan y el selector vuelve a campos heredados del descriptor. לוס
certificados se propagan a la gestion del ciclo de vida de circuitos y se
exponen דרך `telemetry::sorafs.guard` y `telemetry::sorafs.circuit`, אתה רושם
la ventana de validez, las suites de לחיצת יד y si se observaron firmas dobles
para cada guard.

Usa los helpers de la CLI para mantener los fotos sincronizados con los
מפרסמים:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` הורד y verifica el תמונת מצב SRCv2 antes de escribirlo in disco,
mientras que `verify` repite el pipeline de validacion para artefactos obtenidos
por otros equipos, emitiendo un resumen JSON que refleja la salida del selector
de guards en CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Cuando se proporcionan tanto el directorio de relays como la cache de guards, el
orquestador active el gestor de ciclo de vida de circuitos para preconstruir y
שיפוצים של מעגלים של SoraNet לפני אחזור. La configuracion vive en
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) דרך דוס קמפוס
נואבוס:

- `relay_directory`: lleva el snapshot del directorio SNNet-3 para que los hops
  אמצע/יציאה puedan seleccionarse de forma determinista.
- `circuit_manager`: תצורה אופציונלית
  controla el TTL del circuito.

Norito JSON הוספה ל-`circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK reenfian los datas del directorio via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), y la CLI lo conecta automaticamente siempre
que se suministre `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).El gestor renueva circuitos cuando cambian los metadatos del guard (נקודת קצה,
clave PQ o חותמת זמן fijado) o cuando vence el TTL. El helper `refresh_circuits`
אחזור אינבוקדו אנטה דה קאדה (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs de `CircuitEvent` para que los operadores rastreen decisiones del
ciclo de vida. בדיקת אל השרייה
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demuestra latencia estable a
traves de tres rotaciones de guards; consulta el reporte en
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC מקומי

El orquestador puede iniciar optionalmente un proxy QUIC local para que las
הרחבות של נבגדור ו-OS Adaptadores SDK לא ניתנת לביצוע
תעודות או מטמון שומרים. El proxy se enlaza a una direccion
loopback, termina conexiones QUIC y devuelve un manifest Norito que describe el
תעודת שמירה אופציונלית לכל לקוחות. Los eventos de
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
```- `bind_addr` controla donde escucha el proxy (ארה"ב puerto `0` לעורך דין
  un puerto efimero).
- `telemetry_label` se propaga a las metricas para que los dashboards puedan
  distinguir proxies de sesiones de fetch.
- `guard_cache_key_hex` (אופציונלי) אישור que el proxy exponga la misma cache
  de guards con clave que usan CLI/SDK, manteniendo alineadas las extensiones
  del navegador.
- `emit_browser_manifest` אלטרנה סי el לחיצת יד devuelve un manifest que las
  extensiones pueden almacenar y validar.
- `proxy_mode` selecciona si el proxy puentea trafico localmente (`bridge`) o
  solo emite metadatos para que los SDK abran circuitos SoraNet por su cuenta
  (`metadata-only`). El proxy por defecto es `bridge`; establece `metadata-only`
  cuando una workstation deba exponer el manifest sin reenviar streams.
- `prewarm_circuits`, `max_streams_per_circuit` y `circuit_ttl_hint_secs`
  אקספונן רמז אדיקונלס אל נאבגדור פאר que pueda presupuestar streams
  paralelos y entender que tan agresivamente el proxy reutiliza circuitos.
- `car_bridge` (אופציונלי) apunta a una cache local de archivos CAR. אל קאמפו
  `extension` controla el sufijo agregado cuando el objetivo de stream omite
  `*.car`; establece `allow_zst = true` עבור עומסי שירותים `*.car.zst`
  precomprimidos directamente.
- `kaigi_bridge` (אופציונלי) חשיפה של Kaigi ו-spool al proxy. אל קאמפו
  `room_policy` anuncia si el bridge opera en modo `public` o `authenticated`
  para que los clientes del navegador preseleccionen las etiquetas GAR
  correctas.
- `sorafs_cli fetch` מעקף את `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`, permitiendo alternar el modo de
  פליטת אופונטר וסלילים אלטרנטיביים כדי לשנות את הפוליטיקה של JSON.
- `downgrade_remediation` (אופציונלי) תצורה או הוק לשדרוג אוטומטי.
  Cuando esta habilitado el orquestador observa la telemetria de relays para
  detectar rafagas de downgrade y, despues del `threshold` configurado dentro de
  `window_secs`, fuerza el proxy local al `target_mode` (ללא מוצא
  `metadata-only`). אם אתה מפסיק את הדירוגים לאחור, אל proxy vuelve a
  `resume_mode` despues de `cooldown_secs`. Usa el arreglo `modes` להגביל
  el trigger a roles de relay especificos (por defecto relays de entrada).

קואנדו אל ה-proxy se ejecuta en modo bridge sirve dos servicios de aplicacion:

- **`norito`** - el objetivo de stream del cliente se resuelve relativo a
  `norito_bridge.spool_dir`. Los objetivos se sanitizan (sin traversal ni rutas
  absolutas) y, cuando el archivo no tiene extension, se aplica el sufijo
  configurado antes de transmitir el מטען ליטרלמנטה אל navegador.
- **`car`** - los objetivos de stream se resuelven dentro de
  `car_bridge.cache_dir`, הרחבה ל-defecto configurada y
  rechazan payloads comprimidos and menos que `allow_zst` este activedo. לוס
  bridges exitosos responden con `STREAM_ACK_OK` antes de transferir los bytes
  del archivo para que los clientes puedan canalizar la verificacion.En ambos casos el proxy entrega la HMAC de cache-tag (cuando existia una clave
cache de guard durante el handshake) y registra codigos de razon de telemetria
`norito_*` / `car_*` עבור לוחות מחוונים שונים יציאות, ארכיונים
faltantes y fallos de sanitizacion de un vistazo.

`Orchestrator::local_proxy().await` expone el handle en diseecucion para que los
llamadores puedan leer el PEM del certificado, obtener el manifest del
navegador o solicitar un apagado ordenado cuando la aplicacion finaliza.

אפשר למצוא את זה, ה-proxy ahora serve registros **manifest v2**. אדמס דל
אישור קיים ו-la clave de cache de guard, גרסה 2:

- `alpn` (`"sorafs-proxy/1"`) y un arreglo `capabilities` para que los clientes
  לאשר את הפרוטוקול דה סטרים que deben usar.
- Un `session_id` por לחיצת יד y un bloque de sal `cache_tagging` para נגזרת
  afinidades de guard por sesion y תגים HMAC.
- רמזים למעגל ולבחירה בשמירה (`circuit`, `guard_selection`,
  `route_hints`) para que las integraciones del navegador expongan una UI mas
  זרמים ריקה אנטה דה אבריר.
- `telemetry_v2` עם כפתורי מוסטר ופרטיות למכשירים מקומיים.
- Cada `STREAM_ACK_OK` כולל `cache_tag_hex`. Los clientes reflejan el valor en
  el header `x-sorafs-cache-tag` al emitir מבקש HTTP o TCP para que las
  selecciones de guard en cache permanezcan cifradas en reposo.

Estos campos preservan el formato previo; los clientes antiguos pueden ignorar
las nuevas claves y continuar usando el subconjunto v1.

## 2. Semantica de fallos

El orquestador aplica comprobaciones estrictas de capacidad y presupuestos antes
que se transfiera un solo byte. Los fallos caen en tres categorias:

1. **Fallos de elegibilidad (לפני טיסה).** Proveedores sin capacidad de rango,
   תוקף פרסומות או טלמטריה מיושן
   del התוצאות y se omiten en la planificacion. קורות חיים של CLI llenan
   el arreglo `ineligible_providers` con razones para que los operadores puedan
   inspeccionar el drift de gobernanza sin raspar logs.
2. **Agotamiento en tiempo de ejecucion.** Cada proveedor registra fallos
   עוקבים. Una vez que se alcanza el `provider_failure_threshold`
   configurado, el proveedor se marca como `disabled` por el resto de la sesion.
   כל מה שצריך לעשות הוא `disabled`, el orquestador devuelve
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Los limites estrictos se presentan como errores
   estructurados:
   - `MultiSourceError::NoCompatibleProviders` - אל המניפסט דרושות טרמו
     de chunks o alineacion que los proveedores restantes no pueden cumplir.
   - `MultiSourceError::ExhaustedRetries` — se consumio el presupuesto de
     reintentos por chunk.
   - `MultiSourceError::ObserverFailed` - los observadores במורד הזרם (hooks de
     סטרימינג) rechazaron un chunk verificado.שגיאה כללית כוללת את המדד של הנתח הבעייתי,
la razon final de fallo del proveedor. Trata estos fallos como bloqueadores de
שחרור: los reintentos con la misma entrada reproduciran el fallo hasta que
cambien el advert, la telemetria o la salud del proveedor subyacente.

### 2.1 לוח תוצאות התמדה

הגדרת תצורה `persist_path`, אל orquestador escribe אל לוח התוצאות האחרון
פליטת tras cada. תוכן המסמך של JSON:

- `eligibility` (`eligible` או `ineligible::<reason>`).
- `weight` (peso normalizado asignado para esta ejecucion).
- metadatos del `provider` (זיהוי, נקודות קצה, preupuesto de
  concurrencia).

ארכיון תמונות בזק של לוח התוצאות לשחרור אומנות
לאס החלטות של הרשימה השחורה וההשקה Signan siendo Auditables.

## 3. Telemetria y depuracion

### 3.1 מדדים Prometheus

El orquestador emite las suientes metricas דרך `iroha_telemetry`:

| מטריקה | תוויות | תיאור |
|--------|--------|------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | מד דה מביא אורקסטאדוס בנוף. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma que registra la latencia de fetch de extremo a extremo. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de fallos terminales (reintentos agotados, sin proveedores, fallo del observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de intentos de reintento por proveedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de fallos a nivel de sesion que levan and disabilitar proveedores. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Conteo de decisiones de politica de anonimato (cumplida vs brownout) agrupadas por etapa de rollout y razon de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de la cuota de relays PQ dentro del set SoraNet seleccionado. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | היסטוריית היחסים של ממסרים PQ על תמונת מצב של לוח התוצאות. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma del Deficit de politica (ברכיה אנטרה אל אוביקטיביו y la cuota PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma de la cuota de relays clasicos usada en cada sesion. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de conteos de relays clasicos seleccionados por sesion. |

אינטגרה לאס מטריקאות עם לוחות מחוונים של הבמה לפני הפעלת ידיות
ייצור. התכנון המומלץ לתכנון SF-6:

1. **משחזר פעילים** - אזהרה סי אל מד sube sin completions equivalentes.
2. **Ratio de reintentos** — avisa cuando los contadores `retry` superan
   קווי בסיס היסטוריים.
3. **Fallos de proveedor** - dispara alertas al pager cuando cualquier proveedor
   supera `session_failure > 0` דנטרו 15 דקות.### 3.2 יעדי יומנים

El orquestador publica eventos estructurados במטרות דטרמיניסטיות:

- `telemetry::sorafs.fetch.lifecycle` — מארקס דה ciclo de vida `start` y
  `complete` עם נתחים, רכיבים מחדש ואורך סה"כ.
- `telemetry::sorafs.fetch.retry` — eventos de reintento (`provider`, `reason`,
  `attempts`) para alimentar el triage manual.
- `telemetry::sorafs.fetch.provider_failure` — מוכיח את רצונו
  errores repetidos.
- `telemetry::sorafs.fetch.error` — fallos terminales resumidos con `reason` y
  metadatos opcionales del proveedor.

Envia estos flujos al pipeline de logs Norito existente para que la respuesta a
incidentes tenga una unica fuente de verdad. Los eventos de ciclo de vida
exponen la mezcla PQ/clasica via `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sus contadores asociados,
הלו que facilita לוחות המחוונים של כבלים סינון ראספר מטריקאות. השקות Durante de GA,
fija el nivel de logs en `info` para eventos de ciclo de vida/reintento y usa
`warn` עבור טרמינלים של fallos.

### 3.3 קורות חיים JSON

Tanto `sorafs_cli fetch` como el SDK de Rust devuelven un resume estructurado
que contiene:

- `provider_reports` conteos de exito/fracaso y si un proveedor fue
  deshabilitado.
- `chunk_receipts` נתח הוכחה.
- arreglos `retry_stats` e `ineligible_providers`.

Archiva el archivo de resumen al depurar proveedores problematicos: los קבלות
mapean directamente a los metadatos de logs anteriores.

## 4. אופרציה של רשימת רשימות

1. **הכנה לתצורה ב-CI.** Ejecuta `sorafs_fetch` con la
   תצורה אובייקטיבית, pasa `--scoreboard-out` עבור לכידת La Vista de
   elegibilidad y compara con el release anterior. מוכיח Cualquier אינו כשיר
   inesperado detiene la promocion.
2. **Validar telemetria.** Asegura que el despliegue exporta metricas
   `sorafs.fetch.*` יומני estructurados antes de habilitar מביאים ריבוי מקורות
   עבור שימושים. La ausencia de metricas suele indicar que el facade del
   orquestador no fue invocado.
3. **עקיפות דוקומנטריות.** Al aplicar ajustes de emergencia `--deny-provider`
   o `--boost-provider`, מאשר את JSON (o la invocacion CLI) ב-tu changelog.
   Los rollbacks deben revertir el override y capturar un nuevo snapshot del
   לוח התוצאות.
4. **חזור על בדיקות עשן.** Tras modificar presupuestos de reintentos o limites
   de proveedores, צפה ב-hacer להביא את המתקן canonico
   (`fixtures/sorafs_manifest/ci_sample/`) y verifica que los receipts de
   chunks sigan siendo deterministas.

Seguir los pasos anteriores mantiene el comportamiento del orquestador
ניתן לשחזור בהפצות פור פאזות y proporciona la telemetria necesaria para
תשובה לאירועים.

### 4.1 Anulaciones de politicaLos operadores pueden fijar la etapa activa de transporte/anonimato sin editar
la configuracion base estableciendo `policy_override.transport_policy` y
`policy_override.anonymity_policy` en su JSON de `orchestrator` (o suministrando
`--transport-policy-override=` / `--anonymity-policy-override=` a
`sorafs_cli fetch`). Cuando cualquiera de los מבטל את esta presente el
orquestador omite el fallback brownout רגיל: si el nivel PQ solicitado no se
puede satisfacer, el fetch falla con `no providers` en lugar de degradar en
שתיקה. Volver al comportamiento por defecto es tan simple como limpiar los
campos de override.