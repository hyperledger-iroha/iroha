---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: конфигурация оркестратора
заголовок: Конфигурация ордера SoraFS
Sidebar_label: Конфигурация ордера
описание: Конфигурация оператора выборки из нескольких источников, интерпретация падений и очистка телеметрии.
---

:::обратите внимание на Фуэнте каноника
Эта страница отражает `docs/source/sorafs/developer/orchestrator.md`. Мы должны скопировать синхронизированные документы, чтобы удалить устаревшие документы.
:::

# Инструкция по выбору файлов из нескольких источников

Оркестр выборки нескольких источников SoraFS, импульс детерминированной выгрузки и
Paralelas desde el conjunto deprovedores publicado en respaldados por
ла гобернанса. Это объяснение, как настроить оркестера, что вам нужно
Я буду ждать пока развертывания и показы индикаторов телеметрии
де Салюд.

## 1. Возобновление настройки

Оркестр комбинирует три функции конфигурации:

| Фуэнте | Предложение | Заметки |
|--------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Нормализуйте песо-доверители, проверяйте яркость телеметрии и сохраняйте табло в формате JSON, используемое для аудиторий. | Ответ от `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Применение ограничено во времени выброса (предполагаемые повторные намерения, ограничения одновременного доступа, переключатели проверки). | См. карту `FetchOptions` и `crates/sorafs_car::multi_fetch`. |
| Параметры CLI/SDK | Ограничивайте количество пиров, дополнительные регионы телеметрии и демонстрируйте политику запрета/повышения. | `sorafs_cli fetch` напрямую отображает эти флаги; Лос SDK Лос Пасан через `OrchestratorConfig`. |

Помощники JSON в сериализованном формате `crates/sorafs_orchestrator::bindings`
Завершена конфигурация в Norito JSON, вот что у меня есть переносимые между привязками
SDK и автоматизация.

### 1.1 Конфигурация примера JSON

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

Сохраните архив среди обычного приложения `iroha_config` (`defaults/`,
пользователь, фактический) для того, чтобы детерминированные пределы были ограничены
между узлами. Для резервного копирования, доступного только напрямую с развертыванием
SNNet-5a, проконсультируйтесь с `docs/examples/sorafs_direct_mode_policy.json` и подскажите
Дополнения и `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Объявления о накоплениях

SNNet-9 интегрирует руководство губернатора и оркестера. ООН
новый объект `compliance` в конфигурации Norito JSON захватывает вырезки
что происходит с конвейером для получения режима только для прямого доступа:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` объявляет коды ISO-3166 альфа-2 в опере эста
  instancia del orquestador. Коды кодов нормализуются в течение длительного времени.
  парсео.
- `jurisdiction_opt_outs` отображается в реестре губернаторов. Куандо Альгуна
  юрисдикция оператора, доступная в списке, приложение el orquestador
  `transport_policy=direct-only` и создайте резервный вариант
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` список дайджестов манифеста (CIDs cegados, codeificados en
  шестигранные маяскулы). Совпадающие грузы также будут включены в планировку
  только для прямого подключения и резервного копирования `compliance_blinded_cid_opt_out` в
  телеметрия.
- `audit_contacts` зарегистрировать URI, который будет губернатором, прежде чем операторы
  Публиковать в своих игровых книгах GAR.
- `attestations` захватывает пакеты собранных фирм, которые передаются
  политика. При входе определяется опция `jurisdiction` (код ISO-3166).
  альфа-2), un `document_uri`, el `digest_hex` canonico de 64 символов, el
  временная метка излучения `issued_at_ms` и `expires_at_ms` необязательно. Эстос
  артефакты питания в контрольном списке зрительного зала оркестадора, чтобы он
  Инструменты правительства могут помочь вам с документацией
  фирмада.

Пропорция блока соблюдения требований при обычном использовании
Конфигурация для детерминированных настроек операторов. Эль
соответствие требованиям orquestador aplica _despues_ de los Hints de write-mode: incluso si
Запрос SDK `upload-pq-only`, исключения из юрисдикции или манифеста
siguen forzando Transporte - только прямой и Fallan Rapido Cuando не существует
проверяющие аптос.

Канонические каталоги отказа от участия в программе
`governance/compliance/soranet_opt_outs.json`; Общественный совет правительства
актуализированные медианные выпуски этикетов. Полный пример
конфигурация (включая аттестации) доступна в
`docs/examples/sorafs_compliance_policy.json`, и рабочий процесс выполняется
захват на Эль
[playbook de cumplimento GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Настройки CLI и SDK| Флаг / Кампо | Эффект |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничение количества проверок сокращается в фильтре табло. Подайте сообщение в `None`, чтобы использовать все доступные элементы. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita los reintentos por chunk. Супер лимитированный род `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Добавьте снимки задержки/падения в конструктор табло. Телеметрия устарела, поскольку марка `telemetry_grace_secs` признана недопустимой. |
| `--scoreboard-out` | Сохраните табло для расчета (показатели годных и негодных) для последующей проверки. |
| `--scoreboard-now` | Установите временную метку на табло (вторые Unix), чтобы захватывать данные о приборах, которые были определены. |
| `--deny-provider` / политический крючок | Исключайте определенных поставщиков услуг по планированию без блокировки рекламы. Используйте черный список быстрого реагирования. |
| `--boost-provider=name:delta` | Отрегулируйте кредиты по круговой системе, чтобы обеспечить сохранение неповрежденных песо де-гобернанса. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Этикетные метрики и журналы структурированы для панелей мониторинга, которые можно использовать для географического распределения и развертывания. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | По дефекту `soranet-first` сейчас то, что многопрофильный Оркестр - это база. Используйте `direct-only`, чтобы подготовиться к переходу на более раннюю версию или перейти к директиве о накоплении и резерву `soranet-strict` для пилотных PQ-only; правила соблюдения требований должны быть соблюдены в течение длительного времени. |

SoraNet-first - это доблесть за отступничество, и откаты должны быть прочитаны
блокировщик корреспондента SNNet. Tras graduarse SNNet-4/5/5a/5b/6a/7/8/12/13,
la gobernanza endurecera la postura requerida (hacia `soranet-strict`); хаста
затем, только мотивы, связанные с инцидентами, должны быть приоритетными
`direct-only`, необходимо зарегистрироваться в журнале развертывания.

Todos los flags anteriores aceptan sintaxis estilo `--` tanto ru
`sorafs_cli fetch` как бинарный файл `sorafs_fetch` ориентирован на
desarrolladores. SDK демонстрирует неверные варианты, которые могут использовать типичные строители.

### 1.4 Действия по охране тайника

CLI теперь интегрирует переключатель защиты SoraNet для операторов
Пуэдан-Фихар: реле ввода формы, определяющее перед завершением развертывания
SNNet-5 транспорта. Трес новые флаги контролируют Эль-Флюхо:| Флаг | Предложение |
|------|-----------|
| `--guard-directory <PATH>` | Используйте архив JSON, который описывает согласованное состояние реле, которое вы получили (se muestra un subconjunto abajo). Откройте директорию и отобразите кэш-память перед извлечением выборки. |
| `--guard-cache <PATH>` | Сохраните `GuardSet`, закодированный в Norito. Задние выбросы повторно используются в кэше, когда он не находится в новом каталоге. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Отменяет дополнительные параметры для номера защиты входа в замок (при дефекте 3) и удержания клапана (при дефекте 30 дней). |
| `--guard-cache-key <HEX>` | Вы можете дополнительно использовать 32 байта для этикетирования кэшей защиты с MAC Blake3, чтобы архив можно было проверить перед повторным использованием. |

Лас-каргас-дель-директорио де охрана использует компактную конструкцию:

Флаг `--guard-directory` сейчас будет полезная нагрузка `GuardDirectorySnapshotV2`
закодировано в Norito. Бинарный файл моментального снимка содержит:

- `version` — версия таблицы (актуальная `2`).
- И18НИ00000080Х, И18НИ00000081Х, И18НИ00000082Х, И18НИ00000083Х —
  метаданные согласия, которые должны совпасть с каждым сертифицированным внедренным.
- `validation_phase` — Compuerta de Politica de Certificados (`1` = разрешено
  одна фирма Ed25519, `2` = предпочитаемые фирмы, `3` = требуемые фирмы
  дабл).
- `issuers` — emisores de gobernanza con `fingerprint`, `ed25519_public` y
  `mldsa65_public`. Отпечатки пальцев считаются как
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — список пакетов SRCv2 (появился
  `RelayCertificateBundleV2::to_cbor()`). Пакет Cada включает дескриптор
  реле, флаги емкости, политические ML-KEM и фирменные двойные Ed25519/ML-DSA-65.

La CLI verifica cada Bundle против клавиш, объявленных ранее
Слияние директории с тайником охраны. Лос-эскемас JSON здесь, да
нет, я не согласен; требуются снимки SRCv2.

Вызовите CLI с `--guard-directory`, чтобы объединить полученное согласие с
существующий кэш. Селектор сохраняет стражу в том месте, где она находится сейчас
la ventana de retencion y son elegibles в директории; Лос Релейс Нуэвос
reemplazan entradas expiradas. Если вы хотите получить выход, кэш актуализируется
опишите новый маршрут, указанный для `--guard-cache`, обслуживайте сеансы
последующие детерминисты. SDK может воспроизводить совместимые вещи.
ламара `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
и в результате `GuardSet` получился `SorafsGatewayFetchOptions`.`ml_kem_public_hex` разрешает селектору защиты приоритета с емкостью PQ
Когда вы выберете SNNet-5. Переключение этапа (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) сейчас ухудшаются автоматические реле
Класикос: когда охрана PQ может выбрать селектор, устраните лос-сосны
clasicos sobrantes para que las sesiones postiores благоприятствуют рукопожатиям
гибриды. Возобновленные результаты CLI/SDK демонстрируются в виде результатов через
И18НИ00000104Х/И18НИ00000105Х, И18НИ00000106Х,
И18НИ00000107Х, И18НИ00000108Х, И18НИ00000109Х,
`anonymity_classical_ratio` y los Campos Дополнительные кандидаты/дефицит/
вариации поставок, явные отключения электроэнергии и классические резервные варианты.

Директории охраны теперь могут подключиться к полному пакету SRCv2 через
`certificate_base64`. Пакет El orquestador decodifica cada, revalida las Firmas
Ed25519/ML-DSA и сохранение сертификатов анализа совместно с кэшем охранников.
Когда у вас есть сертификат, который будет преобразован в каноническую канонику для
клавы PQ, предпочтения рукопожатия и размышления; срок действия сертификатов истек
Если вы выберете карту и селектор превратится в кампус, находящийся в дескрипторе. Лос
Сертификаты для пропаганды цикла жизни и циклов жизни
экспонируется через `telemetry::sorafs.guard` и `telemetry::sorafs.circuit`, который зарегистрирован
la ventana de validez, las suites de рукопожатие и наблюдение за фирмами dobles
охранник пара када.

Используйте помощники CLI для хранения синхронизированных снимков с файлами
Публикаторы:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` загрузите и проверьте снимок SRCv2 перед записью на дискотеке,
Несколько минут, когда `verify` повторите конвейер проверки для получения артефактов
Для других устройств высылается возобновленный JSON, который отображает выбор селектора
защита в CLI/SDK.

### 1.5 Цикл жизни кругов

Когда вы пропорционируете директорию реле как тайник охраны,
orquestador active el gestor de ciclo de vida decircos para preconstruir y
обновить схемы SoraNet перед ежедневным извлечением. Конфигурация в режиме реального времени
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) через кампос
новые:

- `relay_directory`: сделать снимок каталога SNNet-3 для переходов.
  средний/выход можно выбрать детерминированную форму.
- `circuit_manager`: дополнительная конфигурация (при необходимости используется из-за дефекта).
  контролировать TTL цепи.

Norito JSON сейчас принят блок `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK возвращает данные директории через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) и CLI автоматически подключается постоянно
que se suministre `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).El gestor renueva Circuitos cuando cambian los Metadatos del Guard (конечная точка,
клава PQ или временная метка) или когда вы войдете в TTL. Эль помощник `refresh_circuits`
выборка invocado antes de cada (`crates/sorafs_orchestrator/src/lib.rs:1346`)
создавать журналы `CircuitEvent`, чтобы операторы могли принимать решения
цикл жизни. Эл тест на выдержку
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) демюэстра латенсия установлена
traves de tres rotaciones de Guards; проконсультируйтесь с Reporte ru
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Прокси QUIC локальный

Оркестр может дополнительно запустить локальный прокси-сервер QUIC для этого
Расширения навигации и адаптеры SDK не требуют проверки
сертификаты или сертификаты охранников кэша. Прокси-сервер указывает направление
петля, окончание соединения QUIC и раскрытие манифеста Norito, который описывает
сертификат и дополнительная защита кэша для клиентов. Лос-события де
Transporte Emitidos для прокси-сервера, который можно использовать через
`sorafs_orchestrator_transport_events_total`.

Воспользуйтесь прокси-сервером для перехода к новому блоку `local_proxy` в формате JSON
оркестратор:

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
```- `bind_addr` контроль доступа к прокси-серверу (США, Пуэрто `0` для запроса
  Пуэрто Эфимеро).
- `telemetry_label` распространяет метрики для того, чтобы можно было использовать информационные панели.
  различать прокси-серверы сеансов выборки.
- `guard_cache_key_hex` (необязательно) разрешает прокси-серверу открывать неправильный кеш
  защита с использованием CLI/SDK, поддержание расширений
  дель Навегадор.
- `emit_browser_manifest` альтернативно, если рукопожатие раскрывает манифест, который
  расширения могут быть доступны и действительны.
- `proxy_mode`, выбранный прокси-сервером для локального трафика (`bridge`) o
  соло-эмитированные метаданные, чтобы SDK отключил схемы SoraNet для вашего текущего сайта
  (И18НИ00000140Х). Прокси-сервер неисправен `bridge`; establece `metadata-only`
  Когда рабочая станция выставляет манифест без возобновления потоков.
- `prewarm_circuits`, `max_streams_per_circuit` и `circuit_ttl_hint_secs`
  Дополнительные подсказки для навигации, которые помогут вам начать трансляцию
  параллельно и предполагалось, что прокси-сервер будет повторно использован.
- `car_bridge` (дополнительно) можно добавить в локальный кэш архива CAR. Эль Кампо
  `extension` контролирует совокупность суфиев, когда объект потока отсутствует
  И18НИ00000148Х; установить `allow_zst = true` для полезных нагрузок сервера `*.car.zst`
  precomprimidos Directamente.
- `kaigi_bridge` (необязательно) отображать руты Kaigi в буферизации через прокси. Эль Кампо
  `room_policy` объявляет, что мост оперы в режиме `public` или `authenticated`
  для клиентов, предварительно выбранных по этикету GAR
  корректно.
- `sorafs_cli fetch` экспонирование переопределяет `--local-proxy-mode=bridge|metadata-only`
  y `--local-proxy-norito-spool=PATH`, разрешите альтернативный режим
  выброс или запуск альтернативных катушек без изменения политического JSON.
- `downgrade_remediation` (дополнительно) настройте автоматический переход на более раннюю версию.
  Когда у вас есть возможность наблюдать за телеметрией реле для
  обнаруживает возможность понижения версии y, после настройки `threshold`
  `window_secs`, используйте локальный прокси-сервер `target_mode` (из-за дефекта
  `metadata-only`). Una vez que cesan los downgrades, el proxy vuelve a
  `resume_mode` независимо от `cooldown_secs`. Используйте параметр `modes` для ограничения
  он активирует определенные роли реле (из-за неисправных реле входа).

Когда прокси-сервер активируется в режиме моста с сервисами приложения:

- **`norito`** – цель потока клиента должна быть получена относительно
  `norito_bridge.spool_dir`. Los objetivos se sanitizan (грех обхода ни рутов)
  absolutas) y, когда архив не будет расширен, используйте приложение el sufijo
  настройте перед передачей полезной нагрузки буквально навигатору.
- **`car`** – цели потока будут восстановлены здесь
  `car_bridge.cache_dir`, это расширение из-за дефекта конфигурации
  Перезагруженные полезные нагрузки были заблокированы, когда `allow_zst` был активирован. Лос
  мосты выходят из ответа с `STREAM_ACK_OK` перед передачей байтов
  архив, чтобы клиенты могли выполнить проверку.В случаях, когда прокси-сервер передает кэш-тег HMAC (когда существует клава
кэш-де-охрана во время рукопожатия) и регистрация кодов телеметрии
`norito_*` / `car_*` для разных панелей мониторинга, выходов и архивов
фальтанты и санитарные условия для просмотра.

`Orchestrator::local_proxy().await` покажите ручку и извлеките ее для того, чтобы
Ламадоры могут просмотреть сертификат PEM и получить манифест
навигатор или запросить ответ на запрос, когда приложение будет завершено.

Когда вы уже знакомы, прокси-сервер теперь будет доступен для регистрации **manifest v2**. Адемас дель
существующий сертификат и ключ кэша охраны, v2 в совокупности:

- `alpn` (`"sorafs-proxy/1"`) и назначен `capabilities` для клиентов
  Подтвердите протокол потока, который нужно использовать.
- Un `session_id` для рукопожатия и блока продажи `cache_tagging` для получения
  afinidades de Guard для сеанса и тегов HMAC.
- Подсказки по схемам и выбору защиты (`circuit`, `guard_selection`,
  `route_hints`) для интеграции навигации в основной пользовательский интерфейс
  ручьи Рики Антес де Абрир.
- `telemetry_v2` с ручками управления и конфиденциальности для локальных инструментов.
- Cada `STREAM_ACK_OK` включает `cache_tag_hex`. Клиенты отражают доблесть в
  Заголовок `x-sorafs-cache-tag` с запросами HTTP или TCP для этого
  Выбранные параметры защиты кэша могут постоянно храниться в хранилище.

Estos Campos Preservan el Formato Previo; Los clientes antiguos могут игнорировать
las nuevas claves и продолжить использование el subconjunto v1.

## 2. Семантика падения

Приложение El Orquestador Comprobaciones ограничивает возможности и предполагает возможность раньше
что означает передачу одиночного байта. Los Fallos Caen в трех категориях:

1. **Fallos de elegibilidad (предполетная подготовка).** Proveedores sin capacidad de rango,
   Реклама устарела или телеметрия устарела, если она зарегистрирована в артефакте
   табло и пропущено на плане. Резюме CLI Иленана
   Аррегло `ineligible_providers` с разногласиями, которые можно использовать
   Inspectionar el Drift de Gobernanza без распарки бревен.
2. **Agotamiento en timeepo de ejecucion.** При регистрации падения.
   последовательные. Una vez que se alcanza el `provider_failure_threshold`
   в конфигурации, поставщик будет отмечен как `disabled` для восстановления сеанса.
   Если все проверки прошли через `disabled`, el orquestador devuelve
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Определенные прерывания.** Ограничения ограничены, если они представляют собой ошибки.
   структуры:
   - `MultiSourceError::NoCompatibleProviders` — манифест, требующий трамвая
     куски или соединения, которые остаются в наличии, не могут быть собраны.
   - `MultiSourceError::ExhaustedRetries` — если это предположение
     reintentos por chunk.
   - `MultiSourceError::ObserverFailed` — los observadores вниз по течению (крючки de
     потоковая передача) повторно проверьте фрагмент.Ошибка Cada, включающая проблемный индекс фрагмента, когда это возможно,
La razon Final de Fallo del Providedor. Trata estos Fallos como bloqueadores de
выпуск: los reintentos con la misma entrada reproduciran el Fallo Hasta que
нажмите на рекламу, телеметрию или приветственный адрес субьяценте.

### 2.1 Сохранение табло

Когда вы настроите `persist_path`, организатор напишет финальное табло
Трас Када выброс. Документ JSON содержит:

- `eligibility` (`eligible` или `ineligible::<reason>`).
- `weight` (нормализованный пес, назначенный для этого выброса).
- метаданные `provider` (идентификатор, конечные точки, предпосылки
  конкуренция).

Архивируйте снимки табло вместе с выпущенными артефактами для того, чтобы
решения по черному списку и развертыванию, которые подлежат проверке.

## 3. Телеметрия и очистка

###3.1 Метрика Prometheus

Оркестр передает метрические данные через `iroha_telemetry`:

| Метрика | Этикетки | Описание |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Датчик извлекает предметы на ходу. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма, которая регистрирует задержку выборки экстремального значения. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de Fallos Terminales (reintentosagotados, sinprovedores, Fallo del observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Контадор намерений вернуть посредника. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Контадор де Фаллос достиг уровня сессии, на котором он оказался дешабилитаром. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Концепции анонимных политических решений (кумулида против отключения электроэнергии) принимаются для этапа развертывания и обоснования отступления. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Гистограмма реле PQ в выбранном режиме SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма соотношений предложений реле PQ и снимок табло. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма дефицита политики (прерывание между целями и реальной ситуацией PQ). |
| `sorafs_orchestrator_classical_ratio` | И18НИ00000235Х, И18НИ00000236Х | Гистограмма циклов реле, используемых в каждом сеансе. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Гистограмма наборов реле, выбранных для каждой сессии. |

Интегрируйте метрики на панели управления перед активацией кнопок
производство. Рекомендованная схема отражает план наблюдения SF-6:

1. **Извлекает активы** — эквиваленты предупреждений о том, что датчик не выполнил действия.
2. **Ratio de reintentos** — сообщение, полученное от contadores `retry`, суперан
   исторические исходные данные.
3. **Fallos deprovedor** — отображать оповещения на пейджере, когда вы выполняете запрос
   супер `session_failure > 0` за 15 минут.### 3.2 Цели структурированных журналов

Организатор публичных мероприятий имеет определённые цели:

- `telemetry::sorafs.fetch.lifecycle` — марки циклической жизни `start` y
  `complete` содержит фрагменты, повторные намерения и общую продолжительность.
- `telemetry::sorafs.fetch.retry` — события повторного действия (`provider`, `reason`,
  `attempts`) для руководства по сортировке продуктов питания.
- `telemetry::sorafs.fetch.provider_failure` — проверенные дешабилитадос пор
  повторяющиеся ошибки.
- `telemetry::sorafs.fetch.error` — окончательные терминалы возобновляются с `reason` y
  дополнительные метаданные поставщика.

Отправка распространяется по конвейеру журналов Norito, существующему для ответа
инциденты - это уникальные события. Лос-события циклической жизни
экспонировать mezcla PQ/clasica через `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` и наши партнеры,
вот какие удобные кабельные приборные панели без raspar metrics. Durante развертывает GA,
fija el nivel de logs en `info` для событий циклической жизни/возврата в США
`warn` для кончиков пальцев.

### 3.3 Возобновление JSON

Танто `sorafs_cli fetch` как SDK de Rust, разработанный и возобновленный estructurado
что содержит:

- `provider_reports` с сообщением о выходе/разломе и доставке топлива
  дешабилитадо.
- `chunk_receipts` подробно описывает разрешение каждого фрагмента.
- arreglos `retry_stats` и `ineligible_providers`.

Архив резюме в поисках проблемных документов: квитанции
Mapean направляется к метаданным предыдущих журналов.

## 4. Контрольный список действий

1. **Подготовьте конфигурацию в CI.** Выполните `sorafs_fetch` с помощью
   Конфигурация объекта, шаг `--scoreboard-out` для захвата обзора
   элегантность и сравнение с передним выпуском. Куалькье Проведедор не имеет права участвовать
   неуверенно провести промо-акцию.
2. **Действительная телеметрия.** Убедитесь, что экспортированы метрики.
   `sorafs.fetch.*` y logs estructurados ante de habilitar извлекает данные из нескольких источников.
   для пользователей. La ausencia de metricas suele indicar que el фасад дель
   orquestador no fue invocado.
3. **Документарные изменения.** Применимые настройки экстренной помощи `--deny-provider`
   o `--boost-provider`, подтвердите JSON (или вызов CLI) в журнале изменений.
   Откаты должны вернуться к переопределению и сделать новый снимок
   табло.
4. **Повторите дымовые тесты.** С изменениями, допускаемыми повторными действиями или ограничениями.
   Проверяющие, вы можете получить доступ к приспособлению Canonico
   (`fixtures/sorafs_manifest/ci_sample/`) и проверьте квитанции
   куски сиган сиендо детерминистас.

Следите за передними пасами, чтобы они были в порядке
воспроизводимость и развертывание для этапов и пропорциональности телеметрии, необходимой для
ответ на инцидент.

### 4.1 Политические рекомендацииОперативники могут попасть на этап активной транспортировки/анонима без редактирования
База конфигурации установлена `policy_override.transport_policy` y
`policy_override.anonymity_policy` в формате JSON от `orchestrator` (или суммарный
`--transport-policy-override=` / `--anonymity-policy-override=` а
`sorafs_cli fetch`). Когда кто-то переопределяет это,
orquestador опустите обычное аварийное отключение напряжения: si el nivel PQ solicitado no se
Вы можете получить удовлетворение, если вы получите ошибку с `no providers` и ухудшите качество
молчание. Поведение из-за дефекта - это просто, как ясный лос
Кампус де переопределение.