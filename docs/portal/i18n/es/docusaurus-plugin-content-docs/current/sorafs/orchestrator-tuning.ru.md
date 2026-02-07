---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: Implementación y organización de la organización
sidebar_label: Настройка оркестратора
descripción: Практичные значения по умолчанию, советы по настройке и аудит-чекпоинты для вывода multi-source оркестратора в GA.
---

:::nota Канонический источник
Introduzca `docs/source/sorafs/developer/orchestrator_tuning.md`. Deje copias sincronizadas, ya que no contiene documentos exclusivos.
:::

# Руководство по rollout y настройке оркестратора

Esta operación de operación en [configuraciones personalizadas](orchestrator-config.md) y
[implementación de múltiples fuentes de runbook](multi-source-rollout.md). Оно объясняет,
как настраивать оркестрать для каждой фазы rollout, как интерпретировать
артефакты marcador y какие сигналы телеметрии должны быть на месте перед
расширением трафика. Principales recomendaciones disponibles en CLI, SDK y
автоматизации, чтобы каждый узел следовал одной и той же детерминированной
búsqueda política.

## 1. Базовые наборы параметров

Начните с общего шаблона конфигурации и коректируйте небольшой набор ручек по мере
lanzamiento progresivo. Таблица ниже фиксирует рекомендованные значения для наиболее
распространённых фаз; parámetros, не указанные в таблице, берутся из
`OrchestratorConfig::default()` y `FetchOptions::default()`.| Faza | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Примечания |
|------|-----------------|-------------------------------|------------------------------------|-----------------------|------------------------------|------------|
| **Laboratorio/CI** | `3` | `2` | `2` | `2500` | `300` | Жёсткий лимит задержки и короткая Grace‑окно быстро выявляют шумную телеметрию. Si vuelve a intentarlo, puede que no se muestren correctamente los manifiestos. |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | Otros parámetros del producto están disponibles para pares experimentales. |
| **Canarias** | `6` | `3` | `3` | `5000` | `900` | Соответствует значениям по умолчанию; Al instalar `telemetry_region`, estos tableros pueden segmentar el tráfico canario. |
| **Disponibilidad general** | `None` (usted puede ser elegible) | `4` | `4` | `5000` | `900` | Para realizar un reintento/operación, haga un seguimiento de las tareas transitorias antes de realizar esta auditoría para determinar si es necesario. |- `scoreboard.weight_scale` inserta el dispositivo `10_000`, o cualquier otro sistema aguas abajo no requiere ninguna conexión eléctrica. Увеличение масштаба не меняет порядок провайдеров; оно лишь создаёт более плотное распределение кредитов.
- Si utiliza el paquete JSON y utiliza `--scoreboard-out`, conecte el archivo de audio a la computadora. parámetros.

## 2. Marcador de Гигиена

El marcador muestra tres manifiestos, monitores y televisores.
Antes de la producción:1. **Проверьте свежесть телеметрии.** Убедитесь, что snapshot’ы, указанные в
   `--telemetry-json`, были сняты в пределах заданного Grace‑окна. Записи старше
   `telemetry_grace_secs` завершатся ошибкой `TelemetryStale { last_updated }`.
   Mire esto como bloqueador y elimine los televisores de exportación antes de realizar el pedido.
2. **Elegibilidad de проверьте причины.** Сохраняйте артефакты через
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Каждая запись
   содержит блок `eligibility` с точной причиной отказа. Не обходите несоответствия
   возможностей или истекшие объявления; Mejore la carga útil.
3. **Проверьте дельты веса.** Seleccione el polo `normalised_weight` с предыдущим релизом.
   Сдвиги >10% должны соответствовать намеренным изменениям объявлений или телеметрии
   и должны быть отражены в журнале rollout.
4. **Архивируйте артефакты.** Настройте `scoreboard.persist_path`, чтобы каждый запуск
   публиковал финальный marcador instantáneo. Приложите артефакт к релизному досье
   вместе с манифестом и телеметрическим paquete.
5. **Фиксируйте доказательства микса провайдеров.** Метаданные `scoreboard.json` _и_
   соответствующий `summary.json` должны содержать `provider_count`,
   `gateway_provider_count` y la etiqueta вычисленный `provider_mix`, чтобы ревьюеры могли
   Utilice un filtro de `direct-only`, `gateway-only` o `mixed`. Puerta de enlace‑снимки
   должны показывать `provider_count=0` и `provider_mix="gateway-only"`, а смешанныепрогоны — ненулевые значения для обеих источников. `cargo xtask sorafs-adoption-check`
   валидирует эти поля (и падает при несоответствии счётчиков/лейблов), поэтому запускайте
   его вместе с `ci/check_sorafs_orchestrator_adoption.sh` или своим скриптом захвата,
   чтобы получить paquete `adoption_report.json`. Когда задействованы Torii puertas de enlace,
   сохраняйте `gateway_manifest_id`/`gateway_manifest_cid` в метаданных marcador, чтобы
   La puerta de adopción puede enviar un sobre manifestado con dispositivos médicos seguros.

Подробные определения полей см. в
`crates/sorafs_car/src/scoreboard.rs` y resumen de la estructura CLI, actualizado
`sorafs_cli fetch --json-out`.

## Справочник флагов CLI y SDK

`sorafs_cli fetch` (см. `crates/sorafs_car/src/bin/sorafs_cli.rs`) y обёртка
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) используют
одну и ту же поверхность конфигурации оркестратора. Используйте следующие флаги
при сборе evidencia или воспроизведении канонических accesorios:

Общая справка по флагам multi-source (держите CLI help and документацию синхронной,
правя только этот файл):- `--max-peers=<count>` ограничивает число elegible-провайдеров, проходящих фильтр marcador. Asegúrese de que conecte con todos los proveedores elegibles y establezca `1` para obtener respaldo de fuente única. Inserte `maxPeers` en SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` solicita un reintento limitado en un fragmento, primero `FetchOptions`. Utilice la implementación de la tabla según las opciones recomendadas; CLI‑progony, собирающие evidencia, должны соответствовать дефолтам SDK для сохранения паритета.
- `--telemetry-region=<label>` помечает серии Prometheus `sorafs_orchestrator_*` (y OTLP-реле) лейблом региона/окружения, чтобы дашборды разделяли laboratorio, puesta en escena, canario y GA‑трафик.
- `--telemetry-json=<path>` инжектит instantánea, указанный в marcador. Configure el formato JSON en el marcador, los auditores pueden enviar mensajes de texto (y el archivo `cargo xtask sorafs-adoption-check --require-telemetry`, como captura de pantalla OTLP).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) включают gancho’и bridge‑наблюдателя. Los operadores de escritorio bloquean trozos de servidores locales Norito/Kaigi proxy, clientes potenciales, cachés de guardia y salas de Kaigi. же recibos, что и Rust.- `--scoreboard-out=<path>` (opcional con `--scoreboard-now=<unix_secs>`) confirma la elegibilidad de instantáneas para los auditores. Utilice el formato JSON en dispositivos de televisión y Manifestantes que se encuentren en el ticket de compra.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` proporciona correctores determinados de metadatos. Utilice estas banderas para repetirlas; Una rebaja de categoría para los artículos de gobernanza, que utilizan principalmente el paquete de políticas.
- `--provider-metrics-out` / `--chunk-receipts-out` сохраняют метрики здоровья по провайдерам и recibos по fragments из rollout-checklist; приложите оба артефакта при подаче evidencia по adopción.

Primero (con el accesorio público instalado):

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

El SDK se implementa en la configuración de `SorafsGatewayFetchOptions` en el cliente Rust
(`crates/iroha/src/client.rs`), enlaces JS
(`javascript/iroha_js/src/sorafs.js`) y SDK de Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Держите эти хелперы
Sincronizado con la configuración CLI, los operadores pueden copiar políticas en la automatización
без специальных слоёв трансляции.

## 3. Настройка политики buscar

`FetchOptions` reintento mejorado, paralización y verificación. Por ejemplo:- **Reintentos:** повышение `per_chunk_retry_limit` выше `4` увеличивает время восстановления,
  но может маскировать проблемы провайдеров. Predpochitайте держать `4` как потолок и
  полагаться на ротацию провайдеров для выявления слабых.
- **Порог отказов:** `provider_failure_threshold` определяет, когда провайдер отключается
  на остаток сессии. Configure esta opción con la política de reintento: por no haber sido bloqueada
  reintentar заставляет оркестратор выгнать peer ещё до исчерпания всех reintentos.
- **Параллелизм:** оставляйте `global_parallel_limit` unset (`None`), если только конкретная
  среда не может насытить объявленные диапазоны. Если задано, убедитесь, что значение
  ≤ сумме потоковых бюджетов провайдеров, чтобы избежать hambre.
- **Тогглы верификации:** `verify_lengths` y `verify_digests` должны оставаться включёнными
  в продакшене. Они гарантируют детерминизм при смешанном составе провайдеров; отключайте
  их только в изолированных fuzzing‑средах.

## 4. Transporte estatal y anónimo

Utilice los polos `rollout_phase`, `anonymity_policy` y `transport_policy`, entre otros.
описать приватностную позу:- Predpochitайте `rollout_phase="snnet-5"` y позволяйте дефолтной POLитике анонимности
  Utilice la etapa SNNet-5. Переопределяйте через `anonymity_policy_override` только
  когда gobernanza выпускает подписанную директиву.
- Introduzca `transport_policy="soranet-first"` en la base SNNet-4/5/5a/5b/6a/7/8/12/13 en 🈺
  (см. `roadmap.md`). Utilice la herramienta `transport_policy="direct-only"` para documentos
  downgrade/комплаенс‑учений и дождитесь ревью PQ‑покрытия перед повышением до
  `transport_policy="soranet-strict"` — Este programa está actualizado o no está disponible
  классические реле.
- `write_mode="pq-only"` primero inicie sesión en una máquina de escritura (SDK, operador,
  herramientas de gobernanza) способны удовлетворить PQ-требования. Во время lanzamiento держите
  `write_mode="allow-downgrade"`, estas reacciones variables pueden implementarse en programas
  маршруты, пока телеметрия отмечает downgrade.
- Las guardias y los circuitos de preparación se encuentran en el catálogo de SoraNet. Передавайте подписанный
  instantánea `relay_directory` y caché `guard_set`, чтобы churn guards оставался
  в согласованном retención‑окне. Отпечаток cache, зафиксированный `sorafs_cli fetch`, входит
  в lanzamiento de evidencia.

## 5. Хуки деградации и комплаенса

Este es el caso de la orquesta que debe respetar la política de la comunidad:- **Remediación degradada** (`downgrade_remediation`): отслеживает события
  `handshake_downgrade_total` y, después de `threshold` en `window_secs`,
  Utilice el proxy local en `target_mode` (solo metadatos). Сохраняйте
  дефолты (`threshold=3`, `window=300`, `cooldown=900`), если только постмортемы не
  показывают другой патерн. Documente la anulación de espacios en el lanzamiento diario y
  убедитесь, что дашборды отслеживают `sorafs_proxy_downgrade_state`.
- **Политика комплаенса** (`compliance`): carve-outs по юрисдикциям и манифестам
  проходят через списки opt-out, управляемые gobernanza. Никогда не встраивайте
  anulación ad-hoc en configuraciones de paquete; вместо этого запросите подписанное обновление
  `governance/compliance/soranet_opt_outs.json` y actualiza el formato JSON.

Для обеих систем сохраняйте итоговый paquete de configuraciones y включайте его в evidencia
релиза, чтобы аудиторы могли проследить причины дауншифтов.

## 6. Telemetría y tableros

Antes del lanzamiento del programa, siga las siguientes señales activas en esta página de televisión:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  должен быть нулевым после завершения canary.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — должны стабилизироваться ниже 10% во время canary
  и оставаться ниже 5% после GA.
- `sorafs_orchestrator_policy_events_total` — подтверждает активность ожидаемой стадии
  rollout (лейбл `stage`) y фиксирует apagones через `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — отслеживают доступность PQ-реле относительно
  ожиданий политики.
- Лог‑таргеты `telemetry::sorafs.fetch.*` — должны поступать в общий лог-агрегатор с
  сохранёнными запросами для `status=failed`.

Загрузите канонический Grafana‑дашборд из
`dashboards/grafana/sorafs_fetch_observability.json` (transporte en el portal
**SoraFS → Obtener observabilidad**), чтобы селекторы регион/manifest, reintentos de mapa de calor
по провайдерам, гистограммы задержек trozos y счётчики стагнации соответствовали
Además, SRE proporciona un burn-in. Подключите правила Alertmanager en
`dashboards/alerts/sorafs_fetch_rules.yml` y compruebe la sintaxis Prometheus
`scripts/telemetry/test_sorafs_fetch_alerts.sh` (ayudante de descarga automática
`promtool test rules` localmente o en Docker). Для traspaso de alerta требуется тот же
bloque de enrutamiento, который печатает скрипт, чтобы оperatorы могли приложить evidencia
к lanzamiento‑тикету.

### Telemetros grabados en procesoHoja de ruta de Punk **SF-6e** требует 30-дневного burn-in телеметрии перед переключением
Orquestrator de múltiples fuentes en GA‑дефолты. Используйте скрипты repository, чтобы
ежедневно собирать воспроизводимый paquete артефактов на протяжении окна:

1. Presione `ci/check_sorafs_orchestrator_adoption.sh` con permanentemente instalados
   quemado. Ejemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Programa auxiliar `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   записывает `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` y `adoption_report.json` en
   `artifacts/sorafs_orchestrator/<timestamp>/`, y проверяет чимальное число
   elegibles‑провайдеров через `cargo xtask sorafs-adoption-check`.
2. Когда переменные burn-in присутствуют, script также выпускает `burn_in_note.json`,
   фиксируя label, индекс дня, id манифеста, источник телеметрии и дайджесты артефактов.
   Utilice este JSON para el lanzamiento diario, ya que es un archivo de almacenamiento rápido
   день 30-дневного окна.
3. Importar el nombre de usuario Grafana‑дашборд (`dashboards/grafana/sorafs_fetch_observability.json`)
   En la puesta en escena/producción del espacio de trabajo, пометьте его burn-in label и убедитесь, что каждый
   панель отображает выборки для тестируемых manifiesto/región.
4. Introduzca `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o `promtool test rules …`)
   всякий раз при изменении `dashboards/alerts/sorafs_fetch_rules.yml`, чтобы задокументировать
   Alerta de enrutamiento automática: métricas y deportivas en burn-in.
5. Архивируйте снимок дашборда, вывод теста alert‑ов и хвост логов по запросам
   `telemetry::sorafs.fetch.*` вместе с артефактами оркестратора, чтобы gobernabilidad могло
   воспроизвести evidencia без обращения к live-sistemas.

## 7. Lanzamiento de la lista de verificación1. Verifique los marcadores en CI con una configuración adecuada y coloque artefactos en VCS.
2. Запустите детерминированный accesorios de búsqueda en каждом окружении (laboratorio, puesta en escena, canario, producción)
   и приложите артефакты `--scoreboard-out` и `--json-out` к записи rollout.
3. Проверьте телеметрические дашборды совместно с on-call инженером, убедившись, что все
   вышеуказанные метрики имеют живые выборки.
4. Complete las configuraciones de configuración finales (por ejemplo `iroha_config`) y git‑commit
   gobernanza‑реестра, использованного для объявлений и комплаенса.
5. Despliegue de herramientas y utilice el SDK de comandos para nuevas plantillas y clientes
   интеграции оставались синхронизированными.

Следование этому руководству сохраняет развёртывания оркестратора детерминированными и
аудируемыми, одновременно предоставляя ясные контуры обратной связи для настройки
reintentar‑бюджетов, ёмкости провайдеров и приватностной позы.