---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: конфигурация оркестратора
заголовок: Настройка ордера SoraFS
Sidebar_label: Настройка ордера
описание: Настройте организатора выборки из нескольких источников, интерпретируйте данные и очистите данные телеметрии.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/developer/orchestrator.md`. Мантенья представил, как синхронизированные копии ели альтернативную документацию, оставшуюся в прошлом.
:::

# Инструкция по получению мультиоригема

Оркестратор загрузки нескольких исходных файлов для SoraFS обеспечивает определенные загрузки и
Paralelos a partir do conjunto deprovores publicado em respaldados respaldados
Пела Гунанка. Объясните, как настроить оратора на Синае
они будут продолжать работать во время развертываний и в ходе экспериментов по телеметрии
Индикадоры де Сауд.

## 1. Общие настройки конфигурации

Оператор, комбинирующий три шрифта конфигурации:

| Фонте | Предложение | Заметки |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Нормализация песо де-проверок, проверка свежести телеметрии и сохранение табло в формате JSON, используемое для аудиторий. | Апоядо `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Применение ограничений времени выполнения (бюджеты повторных попыток, ограничения согласования, переключатели проверки). | Mapeia для `FetchOptions` и `crates/sorafs_car::multi_fetch`. |
| Параметры CLI/SDK | Ограничение количества пиров, региональные ограничения телеметрии и политические методы запрета/повышения. | `sorafs_cli fetch` прямо имеет флаги; OS SDK распространяется через `OrchestratorConfig`. |

Помощники ОС JSON в сериализации `crates/sorafs_orchestrator::bindings`
Полная настройка в Norito JSON, Tornando — перенос между привязками SDK
е автомата.

### 1.1 Пример настройки JSON

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

Сохранение или архивирование обычно происходит с `iroha_config` (`defaults/`,
пользователь, фактический) для того, чтобы развертывание определяло границы между
нодос. Чтобы использовать резервный вариант только для прямого подключения к развертыванию SNNet-5a,
Проконсультируйтесь с `docs/examples/sorafs_direct_mode_policy.json` и ориентируйтесь
корреспондент `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Отмена согласования

Интегрированная ориентация на управление SNNet-9 без организатора. Хм
Новый объект `compliance` в конфигурации Norito JSON захватывает вырезки, которые
Forcam или конвейер выборки только в режиме Direct:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` объявляет коды ISO-3166 альфа-2, когда они есть.
  instancia do orquestrador Opera. Нормальные коды для большинства людей
  во время разбора.
- `jurisdiction_opt_outs` отправлен в государственный реестр. Когда ты хочешь
  Юрисдикционное право оператора отображается в списке, или приложение оркестратора
  `transport_policy=direct-only` и выдача резервного варианта
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` список дайджестов манифеста (CID, кодированные и
  hex maiusculo). Полезные нагрузки корреспонденты также для повестки дня камеры только для прямого доступа
  Мы используем резервный вариант `compliance_blinded_cid_opt_out` для телеметрии.
- `audit_contacts` регистрируется как URI, которые позволяют управлять операторами
  publiquem nos playbooks GAR.
- `attestations` захватывает пакеты соответствия, которые поддерживаются
  политика. Cada entrada определяет uma `jurisdiction` опционально (код ISO-3166
  альфа-2), um `document_uri`, o `digest_hex` канонический код из 64 символов, o
  временная метка выброса `issued_at_ms` и `expires_at_ms` необязательно. Эссес
  артефатос алиментам или контрольный список зрительного зала для оркестратора, чтобы
  Ферраментас де Гунанка Поссам Винкуляр отменяет документацию Ассинада.

Для блокировки или согласования с помощью обычной настройки для
que os operadores recebam отменяет детерминированные значения. О orquestrador aplica a
согласовать _depois_ подсказки режима записи: сообщение о том, что запрашивается SDK
`upload-pq-only`, отказ от юрисдикции или манифестации при оформлении или транспортировке
только для прямого доступа и быстрому достижению того, что существует в соответствии с существующими условиями.

Канонические каталоги отказа от участия при жизни
`governance/compliance/soranet_opt_outs.json`; o Общественный совет управления
обновляется посредством релизов tagueadas. Пример полной настройки
(включая подтверждения) esta disponivel em
`docs/examples/sorafs_compliance_policy.json`, и этот процесс работает
захват нет
[книга соответствия GAR] (../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Настройки CLI и SDK| Флаг / Кампо | Эфейто |
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничить количество введенных средств, чтобы отключить фильтр на табло. Defina `None` для использования всех элегантных элементов. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita повторяет попытку. Превышение или ограничение числа `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Снимки задержки/загрузки не отображаются в конструкторе табло. Телеметрия устарела по причине `telemetry_grace_secs`, марка проверена как неэлегивная. |
| `--scoreboard-out` | Продолжайте рассчитывать табло (элегивные + неэлегивные) для проверки позиции. |
| `--scoreboard-now` | Сохраняйте временные метки на табло (вторые Unix) для захвата определенных приборов. |
| `--deny-provider` / политический крючок | Исключаются средства детерминированной формы, которые можно удалить. Используется для быстрого внесения в черный список. |
| `--boost-provider=name:delta` | Корректировка кредитов в циклическом порядке обдумывает, как обеспечить сохранение неповрежденных песо в управлении. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Поворотные метрики и журналы созданы для того, чтобы панели мониторинга могли фильтровать географию или время развертывания. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | На падрао агоре и `soranet-first` я могу найти многопрофильного игрока и базу. Используйте `direct-only` для подготовки к переходу на более раннюю версию или для дальнейшего изменения соответствия, а также резервный `soranet-strict` для пилотных программ только для PQ; отменяет конформность, непрерывную отправку или жесткое тело. |

SoraNet - первая агора или падрао де envio, и откаты, которые позволяют цитировать или блокировать
Соответствует SNNet. Депуа, который SNNet-4/5/5a/5b/6a/7/8/12/13 для выпускников,
govanca endurecera a postura requerida (по слухам, `soranet-strict`); съел ла,
apenas отменяет мотивы для инцидентов, которые должны быть приоритетными `direct-only`, и другие
должны быть зарегистрированы без журнала развертывания.

Все флаги ОС acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
Бинарный файл `sorafs_fetch` был отключен. Os SDK используются как простые операции
для типичных строителей.

### 1.4 Охранник тайников

Интеграция CLI или выбор средств защиты из SoraNet для имеющихся операторов
фиксация реле детерминированной формы до полного развертывания
транспортировать SNNet-5. Трес новос отмечает контроль или флюксо:

| Флаг | Предложение |
|------|-----------|
| `--guard-directory <PATH>` | Ответ на запрос JSON, который требует недавнего согласия на использование реле (подмножество abaixo). Перейдите к настройке каталога или кэшу защиты перед выполнением или выборкой. |
| `--guard-cache <PATH>` | Сохраняется или `GuardSet`, кодированный под Norito. В дальнейшем выполняется повторное использование или кэширование нового каталога. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Отменяет параметры номера защиты от входа в фиксатор (30 секунд) и удерживаемого канала (30 дней). |
| `--guard-cache-key <HEX>` | Дополнительно используйте 32 байта для маркировки кэшей защиты на MAC Blake3, чтобы архив был проверен перед повторным использованием. |

В качестве сотрудников справочника охранников usam um esquema Compacto:O flag `--guard-directory` назад, вперед и полезная нагрузка `GuardDirectorySnapshotV2`
кодифицирован под Norito. О снимке бинарного файла:

- `version` — версия этой таблицы (настоящая версия `2`).
- И18НИ00000080Х, И18НИ00000081Х, И18НИ00000082Х, И18НИ00000083Х —
  метаданные согласия, которые будут соответствовать каждому сертифицированному внедрению.
- `validation_phase` — ворота политики сертификатов (`1` = разрешениe ума
  assinatura Ed25519, `2` = предпочтительные assinaturas duplas, `3` = exigir assinaturas
  дупла).
- `issuers` — эмиссоры управления с `fingerprint`, `ed25519_public` e
  `mldsa65_public`. Отпечатки пальцев Os sao Calculados como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — список пакетов SRCv2 (указан
  `RelayCertificateBundleV2::to_cbor()`). Cada Bundle Carrega или дескриптор сделать
  реле, флаги возможностей, политика ML-KEM и дублированные значки Ed25519/ML-DSA-65.

Пакет CLI verifica cada противоречит тому, что элементы эмиссора объявляются раньше
mesclar или каталог com или Cache de Guards. Альтернативные варианты JSON в большинстве случаев
ацеитос; моментальные снимки SRCv2 как обязательные.

Вызовите CLI через `--guard-directory`, чтобы получить последнее согласие или согласие с ним.
кэш существует. O селектор сохраняет фиксацию, которая всегда остается на месте.
Джанела де retencao e sao elegiveis нет каталога; Новос реле заменить entradas
истекает. После этого вы можете получить доступ к кэшу и сохранить его в памяти.
no caminho fornecido через `--guard-cache`, mantendo sessoes Lateres
критерии. SDK можно воспроизвести или выполнить вручную
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` е
пройти или `GuardSet` в результате `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` разрешает выбор приоритета защиты с возможностью PQ
Когда развертывание SNNet-5 уже началось. Переключение этапа (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) агора rebaixam реле классические
автоматически: когда защита PQ отключена или селектор удаляет штифты
classicos excedentes para que sessoesпоследующие фавориты рукопожатий гибридов.
Наши резюме CLI/SDK отображают результат ошибки через `anonymity_status`/
И18НИ00000105Х, И18НИ00000106Х, И18НИ00000107Х,
И18НИ00000108Х, И18НИ00000109Х, И18НИ00000110Х
Кампос, дополняющий кандидатов/дефицит/дельта предложения, Торнандо Кларос
классические отключения и резервные варианты.

Каталоги стражей назад можно загрузить с полным пакетом SRCv2 через
`certificate_base64`. O orquestrador decodifica cada Bundle, повторно действующий как
Assinaturas Ed25519/ML-DSA и удаленный сертификат анализа, полученный из кэша
охранники. Когда сертификат вручен, он оторвался от канонического шрифта для
отвечает на вопрос PQ, предпочитает рукопожатие и размышления; сертификаты с истекшим сроком действия в Сан-Франциско
Descartados и o селектор, возвращающий альтернативные варианты дескриптора. Сертификаты
propagam-se pela gestao do ciclo de vida de Circuitos e sao expostos через
`telemetry::sorafs.guard` и `telemetry::sorafs.circuit`, которые зарегистрируют Джанелу
подтверждение, наборы рукопожатий и двойные ассинатуры для наблюдения за
Када охранник.Используйте помощники ОС для CLI для создания снимков на сайте Sincronia com publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` добавьте и проверьте снимок SRCv2 перед самой дискотекой, на дискотеке
`verify` воспроизводит конвейер проверки для виндовых устройств внешнего оборудования,
Выдаю резюме JSON, которое отвечает за выбор средств защиты CLI/SDK.

### 1.5 Цикл жизни кругов

Когда каталог ретрансляции и кэш де охранников, или оркестратор
активный цикл жизненного цикла для предварительного строительства и ремонта
Circuitos SoraNet до каждой загрузки. Конфигурация для жизни в `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) через dois novos Campos:

- `relay_directory`: сохранение или снимок каталога SNNet-3 для переходов.
  средний/выходной режим выбран в детерминированной форме.
- `circuit_manager`: дополнительная конфигурация (пригодная для управления), которая управляет
  TTL делает схему.

Norito JSON назад к блоку `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

OS SDK encaminham dados делает каталог через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), а также CLI или автоматическое подключение всегда, когда
`--guard-directory` и fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova Circuitos всегда, когда метаданные охраняют мудам (конечная точка, Chave
PQ или временная метка (фиксированная), или когда истекает срок TTL. О помощник `refresh_circuits`
выборка invocado antes de cada (`crates/sorafs_orchestrator/src/lib.rs:1346`)
создавать журналы `CircuitEvent` для того, чтобы операторы могли принимать циклические решения
де вида. O тест на выдержку `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) демонстрация задержки estavel
atraves de tres rotacoes de Guards; вежа или отношение к ним
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Прокси QUIC локальный

Организатор может дополнительно запустить локальный прокси-сервер QUIC для расширения
Навигатор и адаптеры SDK должны иметь точные сертификаты или сертификаты
тайник охраны. Чтобы прокси-лига и петлевая обратная связь endereco, подключились к QUIC и
возврат манифеста Norito, получение сертификата и защита кэша
по желанию клиента. События транспортировки излучаемых прокси-серверов через
`sorafs_orchestrator_transport_events_total`.

Используйте прокси-сервер для нового блока `local_proxy` без JSON для запроса:

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
```- `bind_addr` управление доступом к прокси-серверу (используйте порт `0` для запроса порта
  эфемера).
- `telemetry_label` распространяется в качестве показателей для различающихся информационных панелей.
  прокси сеансов выборки.
- `guard_cache_key_hex` (необязательно) разрешает использование прокси-сервера или кэша сообщений
  Охранники, использующие CLI/SDK, расширяют возможности навигации.
- `emit_browser_manifest` альтернативно или рукопожатие переходит в манифест
  extensoes podem Armazenar e validar.
- `proxy_mode` выбор локального прокси-сервера (`bridge`) или приложений
  метаданные для SDK, которые используют SoraNet для собственной собственности
  (И18НИ00000140Х). О прокси-сервере `bridge`; используйте `metadata-only`, когда
  Рабочая станция должна экспортировать или манифестировать ретранслируемые потоки.
- `prewarm_circuits`, `max_streams_per_circuit` и `circuit_ttl_hint_secs`
  Дополнительные подсказки для навигации, чтобы можно было использовать параллельные потоки или потоки
  предложите или сколько-нибудь или прокси-сервер повторно использовать схемы.
- `car_bridge` (необязательно) для кэша локальных архивов CAR. О Кампо
  `extension` управление или суфикс анексадо, когда или какой-либо поток опускается `*.car`;
  определение `allow_zst = true` для полезных нагрузок сервера `*.car.zst` precomprimidos.
- `kaigi_bridge` (необязательно) поочередно запускайте буфер в прокси-сервере. О Кампо
  `room_policy` объявляет о мосте Opera в режиме `public` или `authenticated`
  для того, чтобы клиенты предварительно выбрали метки GAR.
- `sorafs_cli fetch` expõe переопределяет `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, разрешите альтернативный режим выполнения или
  можно использовать альтернативные варианты, модифицирующие политику JSON.
- `downgrade_remediation` (дополнительно) позволяет настроить автоматический переход на более раннюю версию.
  Когда вы умеете, оркестратор наблюдает за телеметрией реле для раджадас
  Понижение предыдущей версии, после настройки `threshold` вместо `window_secs`, принудительно
  локальный прокси для `target_mode` (padrao `metadata-only`). Когда понижается версия ОС
  Cessam, или прокси-сервер возвращается к `resume_mode` после `cooldown_secs`. Использовать массив
  `modes` для ограничения или ограничения функций специальных реле (реле Padrao
  де энтрада).

Когда или прокси-род в режиме моста служит для обслуживания приложений:

- **`norito`** – любой поток клиента и относительное разрешение
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (sem traversal, sem caminhos)
  absolutos), e quando o arquivo nao tem extensao, o sufixo configurado e aplicado
  перед тем, как полезная нагрузка будет передана навигатору.
- **`car`** – любой поток может быть решен вслед за `car_bridge.cache_dir`, Herdam
  Расширенная конфигурация и доступ к полезной нагрузке включены в меню, которое
  `allow_zst` — это навык. Мосты bem-sucedidos Answerem com `STREAM_ACK_OK`
  перед передачей байтов в архив, чтобы клиенты могли использовать конвейер Fazer
  да проверено.В каждом случае или в случае использования прокси-сервера или HMAC сделайте кэш-тег (когда у вас есть
кэш де-охраны во время рукопожатия или рукопожатия) и регистрация кодов телеметрии
`norito_*` / `car_*` для успешных различий между панелями мониторинга, старых архивов
и быстрая дезинфекция.

`Orchestrator::local_proxy().await` используйте или обрабатывайте выполнение для ваших задач
получить сертификат от PEM, подать заявление или подать навигатор или адвоката
encerramento gracioso, когда приложение завершено.

При необходимости прокси-серверы обслуживают регистры **manifest v2**. Алем делать
Существующий сертификат и запись в кэш-память, а также дополнение к версии 2:

- `alpn` (`"sorafs-proxy/1"`) и массив `capabilities` для клиентов
  Подтвердите протокол потока, который был принят.
- UM `session_id` для рукопожатия и блока продажи `cache_tagging` для получения
  добавлены меры защиты для сеансов и тегов HMAC.
- Подсказки по замыканию и выбору защиты (`circuit`, `guard_selection`,
  `route_hints`) для интеграции навигации по интерфейсу пользователя раньше
  ручьи де Абрир.
- `telemetry_v2` с ручками блокировки и конфиденциальности для местных инструментов.
- Cada `STREAM_ACK_OK`, включая `cache_tag_hex`. Clientes espelham o valor без заголовка
  `x-sorafs-cache-tag` или необходимые отправители HTTP или TCP для выбора защиты
  их кэш навсегда зашифрован в хранилище.

Esses Campos Fazem Parte do Schema atual; Клиенты могут использовать или использовать соединение
полные или некоммерческие потоки.

## 2. Семантика фальхаса

Приложение orquestrador проверено на соответствие требованиям и бюджетам до того, как вы это сделаете.
Unico байт Seja Transferido. В трех категориях есть следующие категории:

1. **Falhas de elegibilidade (предполетная подготовка).** Provedores sem capacidade de range,
   Рекламные объявления истекли или телеметрия устарела, зарегистрированы без артефактов
   табло и опущенные в повестке дня. Резюме загрузки CLI или массива
   `ineligible_providers` com razoes для того, чтобы операторы проверили дрейф
   бревна губернатора сем распара.
2. **Esgotamento em runtime.** Cadaprovor rastreiafalhasconsecutivas. Куандо
   `provider_failure_threshold` и тингидо, о провидоре и рынке как `disabled`
   остаток остатка. Если все условия перехода указаны для `disabled`,
   о оркестратор реторна
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Определённые прерывания.** Ограничивает жёсткую хирургию как ошибку образования:
   - `MultiSourceError::NoCompatibleProviders` — o манифест exige um span de
     куски или остатки того, что осталось до конца, не принесет нам чести.
   - `MultiSourceError::ExhaustedRetries` — бюджет повторных попыток для фрагмента
     потреблять.
   - `MultiSourceError::ObserverFailed` — наблюдатели ниже по течению (крючки де
     потоковое вещание) rejeitaram um chunk verificado.

Если вы ошибочно включили или указали проблемный фрагмент, когда вы его удалили, разао
финал де фала до проверор. Trate esses erros como bloqueadores de Release —
повторные попытки с повторным входом в воспроизведение и показ рекламы, телеметрия
или саудэ делать проверор ниже мудем.

### 2.1 Постоянное таблоКогда `persist_path` и конфигурация, организатор записывает финал на табло
апос када беги. В документе JSON рассматривается:

- `eligibility` (`eligible` или `ineligible::<reason>`).
- `weight` (нормализованное песо для этого запуска).
- метаданные `provider` (идентификатор, конечные точки, бюджет согласования).

Архивные снимки делаются на табло вместе с артефактами для публикации для принятия решений.
де банименто и постоянное внедрение аудита.

## 3. Телеметрия и очистка

###3.1 Метрика Prometheus

Оркестратор выдает следующие метрики через `iroha_telemetry`:

| Метрика | Этикетки | Описание |
|---------|--------|-----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Датчик извлекает оркестры в вас. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма регистрации задержки извлечения из моста в мост. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (повторить попытку, выполнить проверку, выполнить обсерватор). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Попробуйте повторить попытку. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Контадор-де-фалхас-де-проверитель на сеансе, который оставил вас дестабилизирующим. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Заражение анонимными политическими решениями (примирение против отключения электроэнергии) усиливается для стадии развертывания и мотива для отступления. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Гистограмма участия реле PQ не связана с выбранным SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма соотношений предложений реле PQ без снимка на табло. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма дефицита политики (разрыв между альво и реальным участием PQ). |
| `sorafs_orchestrator_classical_ratio` | И18НИ00000235Х, И18НИ00000236Х | Гистограмма участия в классических американских эстафетах каждый раз. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Гистограмма контактов классических реле, выбранных для сеанса. |

Интегрируйте метрики в информационные панели для настройки перед привычными ручками.
производство. Рекомендуемая схема расположения или план наблюдения SF-6:

1. **Извлекает данные** – оповещение о завершении работы корреспондентов.
2. **Razao de retries** — сообщение о том, что сообщение `retry` превышает базовые показатели
   исторический.
3. **Falhas deprovor** — dispara alertas no пейджер quando qualquerprovor
   cruza `session_failure > 0` за 15 минут.

### 3.2 Целевые показатели журнала исследований

Организатор публичных мероприятий разрабатывает определенные цели:- `telemetry::sorafs.fetch.lifecycle` — маркидоры `start` и `complete` com
  Заражение фрагментов, повторные попытки и общая длительность.
- `telemetry::sorafs.fetch.retry` — события повторной попытки (`provider`, `reason`,
  `attempts`) для руководства по сортировке пищевых продуктов.
- `telemetry::sorafs.fetch.provider_failure` — desabilitados desabilitados devido a.
  повторяющиеся ошибки.
- `telemetry::sorafs.fetch.error` — окончание возобновления работы с `reason` e
  метададос опционаис сделать проверор.

Включение — это потоки для конвейера журналов Norito, которые существуют для того, чтобы
Ответ на происшествие - это уникальный шрифт де Вердаде. События циклической жизни
экспонировать мистуру PQ/classica через `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` и наши партнеры,
Tornando Simples интегрирует информационные панели с метриками raspar. Durante выпускает
GA, исправление уровня журнала `info` для событий циклической жизни/повторной попытки и использования
`warn` для завершения работы.

### 3.3 Резюме в формате JSON

Когда `sorafs_cli fetch` в SDK Rust возвращается к существующему резюме:

- `provider_reports` com contagens de sucesso/falha e se oprovor foi
  дезабилитадо.
- `chunk_receipts` подробно описано, как проверить или найти каждый фрагмент.
- массивы `retry_stats` и `ineligible_providers`.

Arquive o arquivo de resumo ao depurarprovoresprodáticos — квитанции
Mapeiam непосредственно для метаданных журнала.

## 4. Контрольный список в рабочем состоянии

1. **Подготовка конфигурации без CI.** Выполните команду `sorafs_fetch` при настройке.
   также введите пароль `--scoreboard-out`, чтобы получить визуальную элегантность и
   сравните с com или высвободите переднюю часть. Qualquerprovor inelegivel inesperado
   вмешиваться в рекламу.
2. **Действительная телеметрия.** Гарантия, что можно развернуть экспортные метрики `sorafs.fetch.*`
   Электронные журналы, созданные до хабилитарного режима, извлекают разные источники для пользователей. А
   Ausencia de metricas Normalmente Indica que a fachada do orquestrador nao foi
   призыв.
3. **Документарные переопределения.** Можно использовать `--deny-provider` или `--boost-provider`.
   В экстренных случаях используйте JSON (или вызов CLI) без журнала изменений. Откаты разработки
   возврат или переопределение и захват новых снимков на табло.
4. **Повторно выполните дымовые тесты.** Депо изменения бюджетов повторных попыток или ограничений
   Проверки, обновление или выборка приспособлений Canonico
   (`fixtures/sorafs_manifest/ci_sample/`) и проверка квитанций о фрагментах
   постоянный детерминист.

Следите за тем, чтобы пассы были надежными или удобными для воспроизводства оркестратора.
развертывание на этапе и для обеспечения телеметрии, необходимой для ответа на инциденты.

### 4.1 Отмена политики

Operadores Podem Fixar o Estagio ativo de Transporte/Anonimato Sem Editar a
База конфигурации, определенная `policy_override.transport_policy` e
`policy_override.anonymity_policy` в своем JSON от `orchestrator` (или случайно
`--transport-policy-override=` / `--anonymity-policy-override=` ао
`sorafs_cli fetch`). Когда вы отменяете это, или оркестратор может
o Обычное аварийное снижение напряжения: se o nivel PQ solicitado nao puder ser satisfeito, o
выберите Falha com `no providers`, чтобы снизить уровень шума. о откат
для удобства и простоты, когда вы очищаете кампус де переопределения.