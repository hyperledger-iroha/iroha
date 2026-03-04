---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: конфигурация оркестратора
заголовок: Конфигурация оркестратора SoraFS
Sidebar_label: Конфигурация оркестратора
описание: Конфигуратор оркестратора выборки из нескольких источников, интерпретатор вычислений и отладки телеметрии.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/developer/orchestrator.md`. Gardez les deux копирует синхронизированные копии только потому, что документация унаследована и уже удалена.
:::

# Руководство по оркестрации выборки из нескольких источников

Оркестр выборки из нескольких источников SoraFS пилотных телезарядок
Parallèles et deterministes depuis l’ensemble de fournisseurs publié dans des
реклама soutenus par la gouvernance. Руководство пользователя Ce, конфигуратор комментариев
l’orchestrateur, quels Signaux d’échec посещающий подвеску lesrollouts et quels
поток телеметрии, показывающий индикаторы здоровья.

## 1. Вид ансамбля конфигурации

L’orchestrateur fusionne trois source de Configuration:

| Источник | Объектив | Заметки |
|--------|----------|-------|
| `OrchestratorConfig.scoreboard` | Нормализуйте точки четверки, проверьте счетчик телеметрии и сохраните табло JSON, используемое для аудита. | Присоединяюсь к `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Applique des limites d’exécution (бюджеты повторной попытки, согласования, баскулы проверки). | Сопоставлено с `FetchOptions` в `crates/sorafs_car::multi_fetch`. |
| Параметры CLI/SDK | Ограничьте количество пар, атташе регионов телеметрии и разоблачите политику отрицания/поддержки. | `sorafs_cli fetch` выставляет направление флагов ces; SDK используется для распространения через `OrchestratorConfig`. |

Помощники JSON в серийном номере `crates/sorafs_orchestrator::bindings`
полная конфигурация в формате Norito JSON, перенос переносимых привязок SDK и т. д.
автоматизация.

### 1.1 Пример конфигурации JSON

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

Сохраняйте документацию с помощью постоянного пользователя `iroha_config` (`defaults/`, пользователь,
фактическое) afin que les déploiements deterministes héritent des mêmes limites sur
les nuuds. Выровняйте профиль ответа только для прямого подключения к развертыванию SNNet-5a,
Проконсультируйтесь с `docs/examples/sorafs_direct_mode_policy.json` и указаниями
ассоциированные компании в `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Отмена соответствия

SNNet-9 интегрирует пилотируемое управление для управления в оркестре.
Новый объект `compliance` в конфигурации Norito Файл захвата JSON
Выделения, которые заставляют конвейер выборки работать только в режиме Direct:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` объявляет коды ISO‑3166 альфа‑2 или работает именно так.
  экземпляр оркестратора. Нормальные и большие коды для вас
  разбор.
- `jurisdiction_opt_outs` отражает реестр управления. Лорску'ун
  аппарат юрисдикции в списке, оркестр навязывает
  `transport_policy=direct-only` и есть смысл отката
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` список дайджестов манифеста (маскированные CID, кодированные в
  шестнадцатеричный маюскул). Корреспонденты Les payloads forcent aussi une
  планирование только напрямую и резервный вариант
  `compliance_blinded_cid_opt_out` в телеметрии.
- `audit_contacts` регистрирует URI, используемый для управления операторами.
  в книгах-пособиях GAR.
- `attestations` захватывает пакеты соответствия, которые являются оправданными
  политика. Введите название опции `jurisdiction` (код ISO‑3166).
  альфа-2), `document_uri`, канонический `digest_hex` с 64 символами,
  временная метка выпуска `issued_at_ms` и опция `expires_at_ms`. Цес
  артефакты, питающие контрольный список аудита оркестра, который есть
  Функции управления могут переопределять подписанные документы.

Fournissez le bloc de conformité с помощью привычного набора конфигурации для
que les opérateurs reçoivent des overrides deterministes. Оркестратор
аппликация в соответствии с подсказками режима записи: мем, если требуется SDK
`upload-pq-only`, отказ от юрисдикции или явный отказ от юрисдикции
Vers le Transport - только прямой транспорт и échouent Rapidement lorsqu'aucun Fournisseur
соответствовать несуществованию.

Канонические каталоги для резидентов
`governance/compliance/soranet_opt_outs.json` ; Публичный совет по управлению
les Mises à jour через теги релизов. Пример завершения конфигурации
(включая аттестации) доступно в
`docs/examples/sorafs_compliance_policy.json`, и рабочий процесс работает
захват в ле
[playbook de conformité GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Параметры CLI и SDK| Флаг / Чемпион | Эффет |
|--------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничьте количество пользователей, которые проходят через фильтр табло. Mettez `None` для использования всеми имеющими на это право специалистами. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Повторите попытку по частям. Отмените ограничение сокращения `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Добавьте снимки задержки/проверки в конструктор табло. Телеметрия сначала работает на `telemetry_grace_secs`, и она не дает права тренироваться. |
| `--scoreboard-out` | Продолжайте подсчитывать табло (четыре допущенных к участию + неподходящие) для проверки после запуска. |
| `--scoreboard-now` | Дополнительная метка времени на табло (секунды Unix), чтобы сохранять записи детерминированных приборов. |
| `--deny-provider` / политический крючок | Исключите профессиональные навыки детерминизма без рекламы. Утилита для быстрого внесения в черный список. |
| `--boost-provider=name:delta` | Отрегулируйте кредиты по круговой системе, подумав о том, чтобы повар без прикосновения к средствам управления. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Этикет метрик и структур журналов служит для того, чтобы панели мониторинга могли служить поворотным моментом в географическом или неопределенном развертывании. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | По умолчанию `soranet-first` поддерживает то, что оркестратор с несколькими источниками является базовым. Используйте `direct-only` для перехода на более раннюю версию или директивы соответствия и резервируйте `soranet-strict` для дополнительных пилотов только для PQ; les overrides de conformité restent le plafond dur. |

SoraNet-first est désormais le défaut livré, et lesrollbacks doivent citer le
Блокантный корреспондент SNNet. После окончания SNNet-4/5/5a/5b/6a/7/8/12/13,
la gouvernance durcira la posture requise (версия `soranet-strict`); d’ici la, les
Seuls отменяет мотивы инцидента, привилегированные `direct-only` и т. д.
как отправитель в журнале развертывания.

Все флаги, которые принимают синтаксис `--` в `sorafs_cli fetch` и т. д.
binaire orienté devloppeurs `sorafs_fetch`. SDK предоставляет параметры мемов
через типы строителей.

### 1.4 Действия по охране кэша

Кабель CLI отключается для выбора защиты SoraNet для обеспечения дополнительного доступа
операторы управления реле перед входом в детерминированный режим
внедрение полного транспортного комплекса SNNet-5. Трое новых флагов контролируют поток:| Флаг | Объектиф |
|------|----------|
| `--guard-directory <PATH>` | Pointe vers un fichier JSON, определяющий консенсус реле le plus Récent (sous-ensemble ci-dessous). Проходя каталог, загружайте кэш охранников перед выполнением выборки. |
| `--guard-cache <PATH>` | Сохраните `GuardSet` в кодировке Norito. Следующие казни повторно используют каталог кэша мемов без нуво. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Отменяет параметры для выбора номера защиты входа (по умолчанию 3) и окна удержания (по умолчанию 30 дней). |
| `--guard-cache-key <HEX>` | Опция из 32 октетов используется для тегирования кэшей защиты с MAC Blake3 для проверки подлинности карты перед повторным использованием. |

Полезные данные каталога охранников, использующие компактную схему:

Флаг `--guard-directory` вызывает деформацию полезной нагрузки `GuardDirectorySnapshotV2`
закодирован в Norito. Содержимое двоичного снимка:

- `version` — версия схемы (актуальный `2`).
- И18НИ00000080Х, И18НИ00000081Х, И18НИ00000082Х, И18НИ00000083Х —
  метадоны консенсуса соответствуют полному сертификату.
- `validation_phase` — ворота политических сертификатов (`1` = autoriser une
  отдельная подпись Ed25519, `2` = предпочитает двойные подписи, `3` = exiger
  подписи дез дабл).
- `issuers` — специалисты по управлению с `fingerprint`, `ed25519_public` и др.
  `mldsa65_public`. Отпечатки пальцев, которые считаются такими же
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — список пакетов SRCv2 (вылет
  `RelayCertificateBundleV2::to_cbor()`). Комплект чак, включающий описание
  эстафета, флаги емкости, политика ML-KEM и двойные подписи
  Ed25519/ML-DSA-65.

Пакет проверки CLI для борьбы с заранее объявленными метками
Каталог fusionner с кэшем защиты. Наследие JSON не имеет значения
сын плюс принятые; Снимки SRCv2 необходимы.

Нажмите CLI с `--guard-directory` для обеспечения консенсуса плюс
последний раз с существующим кэшем. Выбор режима сохранения охранников на бис
действительны в пределах удержания и имеют право в каталоге; лес
Новые реле заменяют устаревшие продукты. После того, как принесете ресси, положите кэш
Mis à jour est reécrit dans le chemin fourni через `--guard-cache`, gardant les
сессии suivantes deterministes. SDK воспроизводит поведение мема в
истец `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
и инъекционный препарат `GuardSet` стал результатом `SorafsGatewayFetchOptions`.`ml_kem_public_hex` позволяет выбрать приоритет защиты, поддерживающей PQ
подвеска выкатная SNNet-5. Переключатели шага (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) автоматически ухудшает работу
Классические реле: lorsqu’un Guard PQ est disponible, le sélecteur supprime les
классические булавки в тропе, которые будут понравиться избранным сеансам
рукопожатия гибриды. Резюме CLI/SDK предоставляют возможность смешивания результатов через
И18НИ00000104Х/И18НИ00000105Х, И18НИ00000106Х,
И18НИ00000107Х, И18НИ00000108Х, И18НИ00000109Х,
`anonymity_classical_ratio` и ассоциации кандидатов/дефицит/дельта де
поставка, отказ от отключения электроэнергии и классические резервные варианты.

Каталоги охранников, которые могут быть разочарованы, помещают полный пакет SRCv2 через
`certificate_base64`. Пакет декодирования L’orchestrateur, повторная проверка файлов
подписи Ed25519/ML-DSA и сохранение сертификатов анализа в кэше
де охранники. Имеющийся сертификат является отклонением от канонического источника для
les clés PQ, предпочтения рукопожатия и размышления; сертификаты
истекает срок действия карт и выбор возобновляемых полей по наследству описания.
Сертификаты являются пропагандой в рамках жизненного цикла и др.
все это раскрывается через `telemetry::sorafs.guard` и `telemetry::sorafs.circuit`, которые
отправка окончаний действительности, наборов рукопожатий и наблюдения или
не двойные подписи для охраны чека.

Используйте помощники CLI для выравнивания снимков с редакторами:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` зарядите и проверьте снимок SRCv2 перед записью на диск,
tandis que `verify` возобновите конвейер проверки для выпуска артефактов
другие устройства, отображающие резюме в формате JSON, которое отражает выборку выбора
защита CLI/SDK.

### 1.5 Обучение циклической жизни цепей

Лорск — директория реле и тайник охранников в фурнисе, l’orchestrateur
активный цикл жизни цепей для предварительного строительства и др.
Обновление цепей SoraNet перед выборкой. Конфигурация найдена
су `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) через Deux
Новые чемпионы:

- `relay_directory`: перенести снимок в каталог SNNet-3 для этих файлов.
  находится в середине/выходе при выборе определенного способа действий.
- `circuit_manager`: опция конфигурации (активна по умолчанию), управляющая файлом
  TTL цепей.

Norito JSON принимается в блоке `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK передает файлы каталога через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), а также CLI автоматический кабель для этого
`--guard-directory` есть четыре (`crates/iroha_cli/src/commands/sorafs.rs:365`).Le gestionnaire renouvelle les Circuits lorsque les métadonnées de Guard Changent
(конечная точка, ключ PQ или временная метка отправки) или срок действия TTL истекает. Ле помощник
`refresh_circuits` вызов перед выборкой чака (`crates/sorafs_orchestrator/src/lib.rs:1346`)
запись журналов `CircuitEvent` для того, чтобы операторы могли отслеживать решения
Liées au Cycle de vie. Тест на замачивание
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) демонстрация стабильной задержки
ротация охранников sur trois; voir le rapport associé dans
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Прокси QUIC локальный

Оркестратор может использовать локальный прокси-сервер QUIC в зависимости от того, что вам нужно.
Расширения навигации и адаптеры SDK не требуют получения сертификатов
ни ле кле дю кэш де охранников. Прокси-сервер находится в шлейфе адреса, завершается
соединения QUIC и отправка манифеста Norito, выписавшего сертификат и др.
очистите кэш-де-защиту от клиента. Les événements de Transport émis par
прокси-сервер работает через `sorafs_orchestrator_transport_events_total`.

Активируйте прокси через новый блок `local_proxy` в JSON de l’orchestrateur:

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
```- `bind_addr` контроль адреса прокси-сервера (используйте порт `0` для
  требование un port éphémère).
- `telemetry_label` распространяет дополнительные показатели для различных панелей мониторинга.
  прокси-серверы для извлечения сеансов.
- `guard_cache_key_hex` (опция) позволяет использовать прокси-сервер для кэша памяти
  защищает CLI/SDK, следит за тем, чтобы расширения были согласованы.
- `emit_browser_manifest` позволяет восстановить манифест расширений.
  peuvent stocker et valider.
- `proxy_mode` выберите прокси-реле локального трафика (`bridge`) или
  я не знаю, какие метадоны нужны для того, чтобы SDK мог использовать eux-mêmes des Circuits
  СораНет (`metadata-only`). Прокси-сервер установлен по умолчанию в `bridge` ; выбор
  `metadata-only`, когда вы отправите сообщение об открытии манифеста без ретранслятора потоков.
- `prewarm_circuits`, `max_streams_per_circuit` и `circuit_ttl_hint_secs`
  Дополнительные подсказки для навигации по бюджету в потоке
  Parallèles et comprendre le niveau de reutilisation des Circuits.
- `car_bridge` (опция) указывает на кэш локальных архивов CAR. Чемпион
  `extension` контроль над суффиксом, установленным на кабеле `*.car` ; определенный
  `allow_zst = true` для направления обслуживания полезных нагрузок `*.car.zst` перед сжатием.
- `kaigi_bridge` (опция) предоставляет маршруты Kaigi, буферизованные на прокси-сервере. Чемпион
  `room_policy` объявляет, что мост работает в режиме `public` или
  `authenticated` указывает, что клиенты перемещаются по предварительно выбранным меткам
  ГАР присваивает.
- `sorafs_cli fetch` предоставляет переопределения файлов `--local-proxy-mode=bridge|metadata-only`
  et `--local-proxy-norito-spool=PATH`, позволяющий отключить режим выполнения
  или указатель или альтернативные катушки без модификатора политического JSON.
- `downgrade_remediation` (опция) настроить автоматический переход на более раннюю версию.
  Лорскиль активен, оркестр наблюдает за телеметрией реле для
  обнаружение понижения версии Rafales и после прекращения `threshold` в
  Фен `window_secs`, принудительно использовать локальный прокси-сервер версии `target_mode` (по умолчанию
  `metadata-only`). Une fois les downgrades stoppés, le proxy reient au
  `resume_mode` после `cooldown_secs`. Используйте таблицу `modes` ограничителя заливки
  le déclencheur à des roles de relecifiques (по умолчанию реле d’entrée).

Когда прокси-сервер работает в режиме моста, отображаются приложения двух сервисов:- **`norito`** – поток потока клиента является решением по взаимопониманию
  `norito_bridge.spool_dir`. Les cibles sont sanitizées (pas de traversal, pas de
  chemins absolus), et lorsqu’un fichier n’a pas d’extension, le suffixe configuré
  это приложение перед стримером полезной нагрузки, которая используется для навигации.
- **`car`** – флюсовые кабели являются резольвентными в `car_bridge.cache_dir`, наследуются
  Расширение по умолчанию настроено и удалено для сжатия полезных данных
  если `allow_zst` активен. Les Bridges Réussis Répondent с `STREAM_ACK_OK`
  перед передачей октетов архива до тех пор, пока клиенты не смогут
  конвейер проверки.

В двух случаях прокси-сервер работает с HMAC du кэш-тегом (когда находится кэш-тег).
присутствует во время рукопожатия) и зарегистрируйте коды телеметрии
`norito_*` / `car_*` для тех панелей мониторинга, которые отличаются от удачного удара
успехи, профессиональные мастера и курсы санитарной обработки.

`Orchestrator::local_proxy().await` выставьте дескриптор для тех файлов, которые вам нужны
апеллянты могут получить сертификат PEM, возвратить навигационный манифест
Вы требуете отсрочки подачи заявления.

Когда прокси-сервер активен, происходит деформация регистрации **манифест v2**.
Кроме существующего сертификата и ключа кэша защиты, добавлена версия v2:

- `alpn` (`"sorafs-proxy/1"`) и таблица `capabilities` для клиентов
  подтверждение протокола Flux для пользователя.
- Un `session_id` для рукопожатия и блока продажи `cache_tagging` для получения
  привязки защиты к сеансу и тегам HMAC.
- Подсказки по замыканию и выбору защиты (`circuit`, `guard_selection`,
  `route_hints`) afin que les интеграции, навигация, открытая и UI плюс богатая
  avant l’ouverture des flux.
- `telemetry_v2` с ручками управления и конфиденциальности для
  локаль инструмента.
- Чак `STREAM_ACK_OK`, включая `cache_tag_hex`. Клиенты отражают ценность
  при разговоре `x-sorafs-cache-tag` по запросам HTTP или TCP, которые нужны
  выбор защиты в кэше, оставшихся в хранилищах.

Ces champs s’ajoutent au Manifest v2; Клиенты Les Clients Doivent Consommer
разъяснение того, что люди поддерживают и игнорируют отдых.

## 2. Семантика ошибок

L’orchestrateur applique des verifications strictes de capacités et de Budgets
перед передачей моего октета. Les échecs tombent в трех категориях:1. **Échecs d’éligabilité (pre-vol).** Les fournisseurs sans capacité de plage,
   Срок действия рекламных объявлений истекает или в ближайшее время они отправляются в артефакт
   du табло и упущения из планирования. Резюме CLI remplissent le
   Таблица `ineligible_providers` с причинами, которые действуют операторы
   инспектор по управлению механизмами без скребка бревен.
2. **Исполнение для выполнения.** Сделайте последовательность действий.
   Une fois `provider_failure_threshold` внимание, le fournisseur est marqué
   `disabled` для остатка сеанса. Si tous les fournisseurs deviennent
   `disabled`, отсылающий оркестр
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Arrêts déterministes.** Les limites dures remontent sous forme d’erreurs
   структуры:
   - `MultiSourceError::NoCompatibleProviders` — манифест требует промежутка времени
     куски или выравнивание, в котором остающиеся служащие не имеют права чести.
   - `MultiSourceError::ExhaustedRetries` — бюджет повторных попыток по частям
     консоме.
   - `MultiSourceError::ObserverFailed` — наблюдатели ниже по течению (крючки де
     потоковая передача) не отклоняется и не проверяется фрагмент.

Chaque erreur embarque l’index du chunk fautif et, lorsque disponible, la raison
Finale d’échec du Fournisseur. Traitez ces erreurs comme des bloqueurs de Release
— повторные попытки с входом в воспроизводимый мем, чтобы
телеметрия или сантехник-су-жасент не менялись.

### 2.1 Постоянство табло

Когда `persist_path` настроен, оркестратор записывает финальное табло
пробежка после шака. Содержимое документа в формате JSON:

- `eligibility` (`eligible` или `ineligible::<reason>`).
- `weight` (нормальные точки, назначенные для этого запуска).
- métadonnées du `provider` (идентификатор, конечные точки, бюджет согласования).

Архивируйте снимки табло с артефактами, выпущенными до тех пор, пока они не появятся.
Выбор черного списка и развертывание оставшихся проверяемых объектов.

## 3. Телеметрия и дебогаж

### 3.1 Метрики Prometheus

L’orchestrateur émet les métriques suivantes через `iroha_telemetry`:| Метрика | Этикетки | Описание |
|---------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge des fetches orchestres en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма регистрирует задержку выборки в бою. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Compteur des échecs terminaux (повторные попытки, aucun fournisseur, échec observateur). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Счетчик пробных попыток повторной попытки. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Compteur des échecs de Fournisseur au niveau session menant à la desactivation. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Сравнение анонимных политических решений (теню против отключения электроэнергии) группируется по этапу развертывания и причине отступления. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Гистограмма части реле PQ в выбранном наборе SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма соотношений реле PQ в снимке табло. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма политического дефицита (écart entre l’objectif et la part PQ réelle). |
| `sorafs_orchestrator_classical_ratio` | И18НИ00000235Х, И18НИ00000236Х | Гистограмма части классических реле, используемых в текущем сеансе. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Гистограмма счетчиков классических реле, выбранных по сеансу. |

Интегрируйте эти показатели на информационных панелях настройки перед активными ручками
эн производство. Рекомендованное расположение для отражения плана наблюдения SF-6:

1. **Извлекает действия** – оповещения о множественных сообщениях о завершении.
2. **Коэффициент повторных попыток** — предотвращение потери данных `retry`
   исторические основы.
3. **Échecs Fournisseurs** – отключить пейджер, когда он будет готов.
   пройти `session_failure > 0` в течение 15 минут.

### 3.2 Структурные бревна

L’orchestrateur publie des événements structurés vers des cibles deterministes:

- `telemetry::sorafs.fetch.lifecycle` — марки `start` и `complete` с
  количество фрагментов, повторных попыток и общая длительность.
- `telemetry::sorafs.fetch.retry` — повторные попытки (`provider`, `reason`,
  `attempts`) для ручного анализа.
- `telemetry::sorafs.fetch.provider_failure` — четыре кнопки отключены для
  повторяющиеся ошибки.
- `telemetry::sorafs.fetch.error` — проверка окончательных резюме с `reason` и др.
  метадонные варианты фурниссера.Acheminez ces flux vers le конвейер журналов Norito существует до тех пор, пока не будет ответа
Все инциденты являются уникальным источником истины. Les événements de Cycle de vie
выставить микс PQ/classique через `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` и советы компаний-партнеров,
ce qui facilite le câblage des приборных панелей без скребка для метрик. Кулон
Развертывания GA, блокирование уровня журналов в `info` для циклических событий
запустите/повторите попытку и используйте `warn` для устранения терминальных ошибок.

### 3.3 Резюме в формате JSON

`sorafs_cli fetch` и SDK Rust отсылают к содержимому структуры резюме:

- `provider_reports` с успешными проверками и деактивацией.
- `chunk_receipts` détaillant quel fournisseur - удовлетворительный кусок чака.
- таблицы `retry_stats` и `ineligible_providers`.

Archivez le fichier de резюме pour déboguer les fournisseurs défaillants: les
квитанции отображаются в направлении метадонных журналов ci-dessus.

## 4. Контрольный список операций

1. **Подготовка конфигурации в CI.** Lancez `sorafs_fetch` с
   Кабель конфигурации, проход `--scoreboard-out` для захвата звука
   d'éligabilité, et faites un diff a vec le Release precédent. Весь фурниссер
   неправомочный блок по продвижению по неявке.
2. **Проверка телеметрии.** Убедитесь, что развертывание экспортируется.
   метрики `sorafs.fetch.*` и структуры журналов перед активацией выборки
   несколько источников для пользователей. Отсутствие современных индийских метрик
   Что фасад оркестратора не привлекал внимания.
3. **Задокументируйте переопределения.** Lors d’un `--deny-provider` или `--boost-provider`.
   В случае срочности отправьте JSON (или вызов CLI) в журнал изменений. Лес
   откаты doivent revoquer l’override et capture un nouveau snapshot de
   табло.
4. **Обновите дымовые тесты.** После изменения бюджетов повторных попыток или
   Caps de Fournisseurs, поставщик светильников Canonique
   (`fixtures/sorafs_manifest/ci_sample/`) и проверьте квитанции о фрагментах
   остающиеся детерминистами.

Suivre les étapes ci-dessus garde le comportement de l’orchestrateur
воспроизводимый в поэтапном развертывании и для необходимой телеметрии
реагирование на инциденты.

### 4.1 Отмена политики

Les Operateurs Peuvent épingler la Phase de Transport/anonymat active sans
модификатор базовой конфигурации в определенной конфигурации
`policy_override.transport_policy` и `policy_override.anonymity_policy` в
читать JSON `orchestrator` (или в четырех
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). Lorsqu’un отвергает настоящее, l’orchestrateur saute le
Обычный резервный режим отключения: если уровень PQ не требуется, он не может быть удовлетворен,
le fetch échoue avec `no providers` вместо ухудшения тишины. Ле
возвращение к поведению по умолчанию состоит из простого просмотра полей
переопределить.