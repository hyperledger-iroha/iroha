---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: Конфигурация оркестратора SoraFS
sidebar_label: Configuración del orquestador
descripción: Настройка мульти-источникового fetch-оркестратора, интерпретация сбоев and отладка телеметрии.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/developer/orchestrator.md`. Deje copias sincronizadas, pero la documentación actual no se incluirá en ningún documento.
:::

# Руководство по мульти-источниковому fetch-оркестратору

Operador de búsqueda múltiple SoraFS que mejora los parámetros de búsqueda
Paralelamente, se publican anuncios en el sitio de anuncios.
controla la gobernanza. En esta descripción del programa, como operador del orquestador,
Cómo se activan las señales antes del lanzamiento y cómo se colocan los televisores
индикаторы здоровья.

## 1. Обзор конфигурации

El operador observa tres configuraciones históricas:| Источник | Назначение | Примечания |
|----------|------------|------------|
| `OrchestratorConfig.scoreboard` | Normalice todos los monitores, compruebe los numerosos televisores y el marcador JSON compatible con las auditorías. | Основан на `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Principales funciones de tiempo de ejecución (bloqueos de bloqueo, límites de paralelización y verificación de seguridad). | Mapas de `FetchOptions` y `crates/sorafs_car::multi_fetch`. |
| Parámetros CLI / SDK | Ограничивают число пиров, добавляют регионы телеметрии и выводят политики negar/impulsar. | `sorafs_cli fetch` elimina estas banderas; El SDK está instalado en `OrchestratorConfig`. |

Archivos JSON en la serie `crates/sorafs_orchestrator::bindings`
La configuración en Norito JSON, es un SDK permanente y automático.

### 1.1 Configuración básica JSON

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

Сохраняйте файл через стандартное наслаивание `iroha_config` (`defaults/`, usuario,
actual), чтобы детерминированные деплои наследовали одинаковые лимиты на всех
нодах. Para el perfil de solo directo, la implementación de SNNet-5a, смотрите
`docs/examples/sorafs_direct_mode_policy.json` y recomendaciones de uso en
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones соответствия

SNNet-9 supervisa el cumplimiento impulsado por la gobernanza en el operador. Nuevo objeto
`compliance` en configuraciones Norito Carve-outs de archivos JSON, которые переводят
recuperación de canalización solo directa:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` объявляет коды ISO‑3166 alpha‑2, где работает эта
  инстанция оркестратора. Los códigos normales del registro completo se realizan mediante análisis.
- `jurisdiction_opt_outs` зеркалирует реестр gobernancia. Когда любая юрисдикция
  оператора присутствует в списке, оркестратор применяет
  `transport_policy=direct-only` y un fallo de respaldo
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` перечисляет digest манифеста (CID ciego, en верхнем
  hexadecimal). Las cargas útiles de carga permiten la planificación y publicación solo directa
  respaldo `compliance_blinded_cid_opt_out` en televisores.
- `audit_contacts` cierra la URI, la gobernanza de la computadora se puede monitorear en GAR
  libros de jugadas операторов.
- `attestations` фиксирует подписанные пакеты de cumplimiento, на которых держится
  política. Каждая запись задает опциональную `jurisdiction` (ISO‑3166 alpha‑2),
  `document_uri`, канонический `digest_hex` (64 символа), marca de tiempo выдачи
  `issued_at_ms` y opcionalmente `expires_at_ms`. Эти артефакты попадают в
  аудит-чеклист оркестратора, чтобы herramientas de gobernanza связывал anulaciones с
  подписанными документами.Передавайте блок через стандартное наслаивание конфигурации, чтобы
Los operadores utilizan anulaciones determinadas. Оркестратор применяет
cumplimiento _после_ sugerencias de modo de escritura: даже если SDK запрашивает `upload-pq-only`,
optar por no participar en el transporte directo únicamente
и быстро завершает ошибкой, если не осталось совместимых провайдеров.

Канонические каталоги opt-out находятся в
`governance/compliance/soranet_opt_outs.json`; Совет по gobernanza публикует
обновления через lanzamientos etiquetados. Полный пример конфигурации (включая
certificaciones) доступен в `docs/examples/sorafs_compliance_policy.json`, а
операционный процесс описан в
[libro de jugadas соответствия GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Aplicaciones CLI y SDK| Bandera / Polonia | Efecto |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничивает, сколько провайдеров пройдут фильтр marcador. Instale `None`, para utilizar todos los proveedores elegibles. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Ограничивает число ретраев на chunk. El límite anterior es `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Вкалывает instantáneas латентности/сбоев в построитель marcador. El televisor instalado con el dispositivo `telemetry_grace_secs` no es elegible. |
| `--scoreboard-out` | Сохраняет вычисленный marcador (elegibles + no elegibles провайдеры) для пост-анализа. |
| `--scoreboard-now` | Переопределяет marcador de marca de tiempo (segundos Unix), чтобы captura de dispositivos оставались детерминированными. |
| `--deny-provider` / gancho политики puntuación | Los anuncios seleccionados se incluyen en la planificación basada en anuncios publicitarios. Полезно для быстрого listas negras. |
| `--boost-provider=name:delta` | Корректирует round robin ponderado кредиты провайдера, не меняя веса gobernancia. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Las métricas y los logotipos estructurales de Markiruet pueden agrupar geografías o implementarse por completo. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | En el caso de `soranet-first`, este es un operador de múltiples sistemas: básico. Utilice `direct-only` para degradar o cumplir con directivas, y `soranet-strict` se instala para pilotos de solo PQ; el cumplimiento anula остаются жестким потолком. |

SoraNet-first теперь дефолт, а rollbacks должны ссылаться на соответствующий
Bloqueador SNNet. После выпуска SNNet-4/5/5a/5b/6a/7/8/12/13 gobernanza ужесточит
требуемую позу (в сторону `soranet-strict`); до этого только anular по
Algunos usuarios priorizan `direct-only` y no lo hacen en el registro.
lanzamiento.

Todas las banderas que aparecen en la sintaxis `--` y `sorafs_cli fetch`, luego y en
разработческом бинаре `sorafs_fetch`. SDK actualiza las opciones que se escriben
constructores.

### 1.4 Actualización de caché de guardia

CLI теперь подключает selector guardias SoraNet, чтобы операторы могли
детерминированно закреплять relés de entrada до полноценного implementación SNNet-5.
Рабочий процесс контролируют три новых флага:| Bandera | Назначение |
|------|-----------|
| `--guard-directory <PATH>` | Utilice el formato JSON para el consenso de retransmisión posterior (no disponible). El directorio anterior está activado para guardar la caché antes de buscar. |
| `--guard-cache <PATH>` | Сохраняет Norito codificado como `GuardSet`. Las siguientes funciones utilizan caché, ya que ningún directorio nuevo no está disponible. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Anulaciones opcionales para los guardias de entrada (para el número 3) y para el usuario (para el número 30). |
| `--guard-cache-key <HEX>` | El botón opcional de 32 teclas para guardar caché en Blake3 MAC, puede proporcionar archivos después de una implementación avanzada. |

Directorio de protección de cargas útiles используют компактную схему:

El indicador `--guard-directory` contiene la carga útil codificada con Norito
`GuardDirectorySnapshotV2`. Бинарный instantánea содержит:- `version` — версия схемы (сейчас `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  Consenso metadano, muchos de ellos se unen a cada uno de los participantes.
- `validation_phase` — puertas de enlace político (`1` = разрешить одну Ed25519
  подпись, `2` = предпочесть двойные подписи, `3` = требовать двойные подписи).
- `issuers` — Gobernanza de emisores con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. Huella digital вычисляется как
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — список paquetes SRCv2 (выход `RelayCertificateBundleV2::to_cbor()`).
  Каждый paquete содержит retransmisión de descriptores, indicadores de capacidad, política ML-KEM y
  двойные подписи Ed25519/ML-DSA-65.

CLI проверяет каждый paquete против объявленных ключей emisor перед объединением
instantáneas.

Utilice CLI con `--guard-directory` para obtener el consenso actual
существующим caché. Selector сохраняет закрепленные guardias, которые еще в окне
удержания и допустимы в directorio; новые relés заменяют просроченные записи.
Después de la aplicación de recuperación de caché no deseada según `--guard-cache`,
обеспечивая детерминированность следующих сессий. SDK воспроизводят поведение,
вызывая `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
и передавая полученный `GuardSet` в `SorafsGatewayFetchOptions`.`ml_kem_public_hex` позволяет selector приоритизировать PQ-guards во время
lanzamiento de SNNet-5. La etapa alterna (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) теперь автоматически понижают классические relés: когда
доступен PQ guard, selector сбрасывает лишние clavijas clásicas, чтобы последующие
сессии предпочитали гибридные apretones de manos. Resúmenes de CLI/SDK disponibles
micrófonos por `anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` y связанные поля кандидатов/дефицита/дельт suministro,
делая apagones y retrocesos clásicos явными.

Directorios de guardia теперь могут содержать полный paquete SRCv2 через
`certificate_base64`. Оркестратор декодирует каждый paquete, повторно проверяет
подписи Ed25519/ML-DSA y сохраняет разобранный сертификат вместе с guard cache.
Según la certificación de las claves PQ estándar,
настроек apretones de manos y весов; просроченные сертификаты отбрасываются, и selector
Circuito de ciclo completo y dispositivos disponibles según `telemetry::sorafs.guard` y
`telemetry::sorafs.circuit`, фиксируя окно валидности, handshake suites y
наличие двойных подписей для каждого guard.

Utilice ayudantes de CLI para sincronizar instantáneas con
publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
````fetch` almacena y valida una instantánea SRCv2 antes de grabar en el disco, en `verify`
повторяет валидации для артефактов из других команд, выдавая JSON
resumen, который зеркалит selector de protección de salida CLI/SDK.

### 1.5 Circuito de ciclismo de montaña

Когда доступны y directorio de retransmisión, y caché de protección, окестратор активирует circuito
administrador de ciclo de vida para la actualización y actualización de circuitos SoraNet
перед каждым buscar. Configuración actualizada en `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) через два новых поля:

- `relay_directory`: captura de pantalla del directorio SNNet-3, saltos intermedios/de salida
  выбирались детерминированно.
- `circuit_manager`: configuración opcional (включена по умолчанию),
  контролирующая TTL цепей.

Norito Bloque de formato JSON `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK передают данные directorio через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), un enlace CLI y un código automático
anterior `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Менеджер обновляет circuitos, когда меняются метаданные guard (punto final, clave PQ
o marca de tiempo fijada) o marca de tiempo TTL. Ayudante `refresh_circuits`, вызываемый
Antes de buscar (`crates/sorafs_orchestrator/src/lib.rs:1346`), emite logotipos
`CircuitEvent`, el operador puede desactivar el ciclo de funcionamiento. remojar
prueba `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) Demostración estable
латентность на трех rotaciones guardias; смотрите отчет в
`docs/source/soranet/reports/circuit_stability.md:1`.### 1.6 Programas rápidos locales

El operador puede utilizar de forma opcional los programas QUIC locales y sus dispositivos de control.
La actualización y los adaptadores SDK no mejoran las contraseñas ni protegen las claves de caché. Proximidad
слушает loopback-адрес, завершает QUIC соединения and возвращает Norito manifest,
описывающий сертификат y опциональный guard cache key. Transporte события,
эмитируемые прокси, учитываются в `sorafs_orchestrator_transport_events_total`.

Haga clic en el nuevo bloque `local_proxy` en el ordenador JSON:

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
```- `bind_addr` se encuentra en la dirección de procesamiento (utiliza el puerto `0` para el efecto
  puerto).
- `telemetry_label` распространяется в метриках, чтобы дашборды различали прокси
  y buscar-сессии.
- `guard_cache_key_hex` (opcionalmente) permite realizar el proceso de activación de la clave
  guard cache, que utiliza CLI/SDK, que almacena contraseñas
  синхронизированными.
- `emit_browser_manifest` включает выдачу manifest, который расширения могут
  сохранять и проверять.
- `proxy_mode` incluye, incluye el proceso de mayor tráfico local (`bridge`) o
  Para crear metadanos, el SDK incluye circuitos SoraNet.
  (`metadata-only`). По умолчанию `bridge`; используйте `metadata-only`, если
  рабочая станция должна выдавать manifiesto без ретрансляции потоков.
- `prewarm_circuits`, `max_streams_per_circuit` y `circuit_ttl_hint_secs`
  передают браузеру дополнительные sugerencias, чтобы он мог бюджетировать параллельные
  потоки и понимать агрессивность circuitos de reutilización.
- `car_bridge` (opcionalmente) указывает на локальный cache CAR-архивов. polo
  `extension` задает суфикс, добавляемый когда target не содержит `*.car`; задайте
  `allow_zst = true` para la mayoría de los clientes `*.car.zst`.
- `kaigi_bridge` (opcional) экспонирует Kaigi routes из spool в прокси. polo
  `room_policy` muestra el código `public` o `authenticated`, que están bloqueados
  Los clientes deben utilizar etiquetas GAR correctas.- `sorafs_cli fetch` el controlador anula `--local-proxy-mode=bridge|metadata-only`
  Y `--local-proxy-norito-spool=PATH`, puede abrir el tiempo de ejecución o descargarlo.
  Bobinas alternativas basadas en políticas JSON.
- `downgrade_remediation` (opcionalmente) gancho de degradación automático.
  Когда включено, оркестратор следит за relés de telemetría para la degradación de всплесков y,
  Después de `threshold` junto con `window_secs`, el proceso de permanencia previo
  en `target_mode` (по умолчанию `metadata-only`). Когда degradaciones прекращаются,
  прокси возвращается к `resume_mode` после `cooldown_secs`. Используйте массив
  `modes`, чтобы ограничить триггер конкретными ролями relés (по умолчанию relés de entrada).

Когда прокси работает в bridge-reжиме, он обслуживает два приложения:

- **`norito`** — destino de transmisión клиента разрешается относительно
  `norito_bridge.spool_dir`. Objetivos санитизируются (без transversal, без
  абсолютных путей), y если файл без расширения, применяется настроенный суфикс
  до отправки payload в браузер.
- **`car`** — objetivos de transmisión разрешаются внутри `car_bridge.cache_dir`, наследуют
  Una mejor optimización y eliminación de cargas útiles, además de `allow_zst`, no está disponible.
  Успешный bridge отвечает `STREAM_ACK_OK` перед передачей байтов архива, чтобы
  verificación de tuberías de клиенты могли.Algunas funciones previas a la etiqueta de caché HMAC (la clave de caché de protección está disponible en
время handshake) y записывает `norito_*` / `car_*` códigos de motivo, чтобы дашборды
различали успехи, отсутствие файлов и ошибки санитизации.

`Orchestrator::local_proxy().await` manija de puerta para puerta PEM
сертификата, получения browser manifest o корректного завершения при выходе
приложения.

Когда прокси включен, он теперь отдает записи **manifiesto v2**. pomimo
существующих сертификата и guard cache key, v2 добавляет:

- `alpn` (`"sorafs-proxy/1"`) y masa `capabilities`, sus clientes clientes
  протокол потока.
- `session_id` para apretón de manos y `cache_tagging` bloque de sal para derivación
  afinidad de guardia de sesión y etiquetas HMAC.
- Consejos para la selección de circuito y protección (`circuit`, `guard_selection`,
  `route_hints`) para una gran interfaz de usuario para la captura de fotos.
- `telemetry_v2` con perillas de selección y privacidad para instrumentos locales.
- Каждый `STREAM_ACK_OK` включает `cache_tag_hex`. Los clientes hacen esta oferta
  En el caso de `x-sorafs-cache-tag` en HTTP/TCP, esta protección en caché
  selecciones оставались зашифрованными на диске.

Estos son algunos de los mejores sistemas: los clientes más famosos pueden ignorar nuevos clientes y
продолжать использовать subconjunto v1.

## 2. Семантика отказов

Оркестратор применяет строгие проверки возможностей и бюджетов до передачи
первого байта. Otras opciones en tres categorías:1. **Отказы по elegible (antes del vuelo).** Провайдеры без capacidad de alcance,
   просроченные anuncios или устаревшая телеметрия фиксируются в marcador
   Los artefactos no se pueden colocar en la planificación. Resúmenes de CLI заполняют массив
   `ineligible_providers` причинами, чтобы операторы podrían detectar la deriva
   gobernanza без парсинга логов.
2. **Agotamiento del tiempo de ejecución.** Каждый провайдер отслеживает последовательные ошибки.
   Когда достигается `provider_failure_threshold`, провайдер помечается как
   `disabled` до конца сессии. Если все провайдеры стали `disabled`, оркестратор
   возвращает `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Детерминированные прерывания.** Algunas limitaciones de la estructura
   ошибки:
   - `MultiSourceError::NoCompatibleProviders` — манифест требует span/alineación,
     который оставшиеся провайдеры не могут соблюсти.
   - `MultiSourceError::ExhaustedRetries` — исчерпан presupuesto ретраев на trozo.
   - `MultiSourceError::ObserverFailed` — observadores posteriores (ganchos de transmisión)
     отклонили проверенный trozo.

Каждая ошибка содержит индекс проблемного chunk y, когда доступно, финальную
причину отказа провайдера. Считайте эти ошибки bloqueadores de liberación — повторные
anuncios publicitarios en el tema de entrada de anuncios, anuncios, televisores o artículos
провайдера не изменятся.

### 2.1 Сохранение marcador

При настройке `persist_path` оркестратор записывает финальный marcador после
каждого прогона. Archivo de documento JSON:- `eligibility` (`eligible` o `ineligible::<reason>`).
- `weight` (normalmente, según la versión anterior).
- метаданные `provider` (identificador, puntos finales, бюджет параллелизма).

Архивируйте marcador de instantáneas вместе с lanzamiento артефактами, чтобы решения по
lista negra y lanzamiento оставались проверяемыми.

## 3. Telemetría y medición

### 3.1 Métricas Prometheus

El operador emite algunas métricas según `iroha_telemetry`:| Métrica | Etiquetas | Descripción |
|---------|--------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Indicador активных buscar-operación. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма полной латентности buscar. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Счетчик финальных отказов (исчерпаны ретраи, нет провайдеров, ошибка observer). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Счетчик попыток ретраев по провайдерам. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Счетчик провайдерских отказов на уровне сессии, приводящих к отключению. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Estas políticas anónimas (profundamente vs. apagón) se encuentran en el lanzamiento y el retroceso de los estadios. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | El sistema de relés PQ está conectado a SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма доли PQ retransmisiones en el marcador de instantáneas. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма дефицита политики (разница между целью и фактической долей PQ). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma de relés clásicos en cada sesión. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Гистограмма числа выбранных классических retransmisiones en sesión. |Integre las métricas en los tableros de preparación con las perillas de producción.
Рекомендуемая раскладка повторяет план наблюдаемости SF-6:

1. **Recuperaciones activas**: alerta, o calibre растет без соответствующих completaciones.
2. **Proporción de reintentos**: se utiliza en las líneas de base históricas anteriores a `retry`.
3. **Fallas del proveedor** — activar alertas de buscapersonas, когда любой провайдер превышает
   `session_failure > 0` antes de 15 minutos.

### 3.2 Destinos de registro estructurados

El operador de la estructura publicitaria se ocupa de los objetivos determinados:

- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` y `complete` con niños
  trozos, ретраев и общей длительностью.
- `telemetry::sorafs.fetch.retry` — события ретраев (`provider`, `reason`,
  `attempts`) para el triaje más importante.
- `telemetry::sorafs.fetch.provider_failure` — провайдеры, отключенные из-за
  повторяющихся ошибок.
- `telemetry::sorafs.fetch.error`: opciones finales con `reason` y opciones opcionales
  метаданными провайдера.

Направляйте эти потоки в существующий Norito log pipeline, чтобы у incidente
respuesta был единый источник истины. Ciclo de vida события показывают PQ/clásico
mezclar через `anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` y связанные счетчики, что упрощает настройку
дашбордов без парсинга метрик. Los nuevos lanzamientos de GA incluyen nuevos logotipos
`info` para el ciclo de vida/reintento soluciona y analiza `warn` para errores de terminal.

### 3.3 Resúmenes JSONResumen de la estructura del SDK de `sorafs_cli fetch` y Rust, codificador:

- `provider_reports` с числами успехов/сбоев и статусом отключения провайдера.
- `chunk_receipts`, показывающий какой провайдер обслужил каждый chunk.
- masas `retry_stats` y `ineligible_providers`.

Архивируйте resumen при отладке проблемных провайдеров — recibos напрямую
соотносятся с лог-метаданными выше.

## 4. Lista de operaciones

1. **Implemente la configuración en CI.** Introduzca `sorafs_fetch` con el televisor
   Configuración, consulte `--scoreboard-out` para ver las características de elegibilidad y
   сравните с предыдущим lanzamiento. Любой неожиданный proveedor no elegible
   bloquee la promoción.
2. **Проверьте телеметрию.** Убедитесь, что деплой экспортирует метрики
   `sorafs.fetch.*` y registros estructurales previos a la búsqueda de fuentes múltiples
   для пользователей. Отсутствие метрик обычно означает, что фасад оркестратора
   не был вызван.
3. **Anulaciones de configuración.** En caso de emergencia `--deny-provider` o
   `--boost-provider` actualiza JSON (o CLI) en el registro de cambios. Reversiones
   Aquí se pueden anular y crear nuevas instantáneas del marcador.
4. **Pruebas de humo.** После изменения presupuestos ретраев или caps
   провайдеров заново выполните buscar accesorio канонического
   (`fixtures/sorafs_manifest/ci_sample/`) и убедитесь, что recibos por trozos
   остаются детерминированными.Следование шагам выше делает поведение оркестратора воспроизводимым в puesta en escena
implementaciones y dispositivos previos a la respuesta a incidentes.

### 4.1 Anulaciones de políticas

Los operadores pueden cerrar la actividad de transporte/anonimato этап без изменения базовой
configuraciones, задав `policy_override.transport_policy` y
`policy_override.anonymity_policy` en JSON `orchestrator` (или передав
`--transport-policy-override=` / `--anonymity-policy-override=`
`sorafs_cli fetch`). Если anular присутствует, оркестратор пропускает обычный
reserva de caída: если требуемый PQ tier недостижим, recuperar завершается с
`no providers` Esta es una degradación. Возврат к поведению по умолчанию —
простое очищение anular полей.