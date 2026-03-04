---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр
заголовок: Реестр файлов фрагментов SoraFS
Sidebar_label: Реестр фрагментов
описание: идентификаторы доступа, параметры и план согласования для регистра блоков SoraFS.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/chunker_registry.md`. Нам пришлось скопировать синхронизированные копии, чтобы удалить комплект документации Sphinx.
:::

## Регистр файлов фрагментов SoraFS (SF-2a)

В стеке SoraFS используется средство разделения на фрагменты, связанное с небольшим регистром с пространством номеров.
В каждом файле заданы параметры CDC, определенные метаданные и дайджест/мультикодек, которые используются в манифестах и ​​архивах CAR.

Консультант по авторским файлам
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для требуемых метаданных, контрольный список проверки и план продажи перед отправкой новых заявок.
Una vez que la gobernanza aprueba un cambio, sigue el
[контрольный список развертывания реестра] (./chunker-registry-rollout-checklist.md) и эл.
[playbook de Manifest en Staging](./staging-manifest-playbook) для промоутера
Лос-фурнитура, постановка и постановка.

### Перфайлы

| Пространство имен | Номбре | СемВер | Идентификатор пользователя | Мин (байты) | Цель (байты) | Макс. (байты) | Короткая тушь | Мультихэш | Псевдоним | Заметки |
|-----------|--------|--------|-------------|--------------|----------------|-------------|-----------------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Используемые канонические материалы для светильников SF-1 |

Регистр живет в коде как `sorafs_manifest::chunker_registry` (пользователь от [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Када-энтрада
Выражается как `ChunkerProfileDescriptor` с:

* `namespace` – логическая группировка связанных файлов (стр. например, `sorafs`).
* `name` – этикет, разборчивый для людей (`sf1`, `sf1-fast`, …).
* `semver` – семантическая версия версии для соединения параметров.
* `profile` – el `ChunkProfile` реальный (мин/цель/макс/маска).
* `multihash_code` – мультихеш, используемый для создания дайджестов фрагмента (`0x1f`
  для значения по умолчанию SoraFS).

Сериализация манифеста в файлах mediante `ChunkingProfileV1`. Регистрационная структура
метаданные реестра (пространство имен, имя, имя) вместе с параметрами CDC
в грубом порядке и в списке псевдонимов Мострада Арриба. Потребители не хотят отказываться от услуг
включение в реестр для `profile_id` и повторение встроенных параметров в любое время
апаресканские идентификаторы desconocidos; список псевдонимов, которые могут гарантировать HTTP-клиенты
Seguir enviando обрабатывает наследственные файлы в `Accept-Chunker` без предварительной подготовки. Лас-реглас-де-ла
карта регистрации exigen que el handle canónico (`namespace.name@semver`) sea la
Пример ввода в `profile_aliases`, следующий псевдоним наследника.

Для проверки реестра с помощью инструментов вызовите помощник CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```Все флаги CLI, которые записывают JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) принимает `-` как обычный, и он передает полезную нагрузку на стандартный вывод в
создать архив. Это позволяет легко получать данные и использовать инструменты, когда они находятся в рабочем состоянии.
Обратите внимание на дефект, связанный с подтверждением основного отчета.

### Матрица развертывания и план деспльега


La tabla siguiente captura el estado de soporte para `sorafs.sf1@1.0.0` ru
Принципиальные компоненты. «Мост» см. ссылку на CARv1 + SHA-256.
что требуется явная договоренность с клиентом (`Accept-Chunker` + `Accept-Digest`).

| Компонент | Эстадо | Заметки |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Сопортадо | Подтвердите канонический дескриптор + псевдоним, передавайте отчеты через `--json-out=-` и применяйте карту регистрации с `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Ретирадо | Конструктор манифеста поддержки; США `iroha app sorafs toolkit pack` для упаковки CAR/манифеста и обслуживания `--plan=-` для определенной повторной проверки. |
| `sorafs_provider_advert_stub` | ⚠️ Ретирадо | Помощник по проверке подлинности в автономном режиме; Рекламные объявления поставщика должны производиться через конвейер публикации и проверки через `/v1/sorafs/providers`. |
| `sorafs_fetch` (оркестратор разработчика) | ✅ Сопортадо | Lee `chunk_fetch_specs`, содержит полезные нагрузки `range` и набор данных CARv2. |
| Исправления SDK (Rust/Go/TS) | ✅ Сопортадо | Регенерация через `export_vectors`; Ручка canonico появляется впервые в каждом списке псевдонимов и является фирмой для обсуждения. |
| Согласование файлов на шлюзе Torii | ✅ Сопортадо | Реализована полная грамматика `Accept-Chunker`, включая заголовки `Content-Chunker` и экспонирование моста CARv1 в одиночку и подробные инструкции по переходу на более раннюю версию. |

Описание телеметрии:

- **Телеметрия выборки фрагментов** — CLI Iroha `sorafs toolkit pack` создает дайджесты фрагментов, метаданные CAR и обрабатывает PoR для приема на информационных панелях.
- **Реклама поставщика** — полезные данные рекламы включают метаданные возможностей и псевдонимов; действительный выбор через `/v1/sorafs/providers` (стр. например, наличие емкости `range`).
- **Монитор шлюза** — операторы должны сообщать о паре `Content-Chunker`/`Content-Digest`, чтобы детектор неожиданно понизил версию; Вы ожидаете, что использование моста станет причиной устаревания.

Политика депрекасьона: то, что было ратифицировано преемником неудачи, программа двойного выпуска публикаций
(документация в собственности) до маркировки `sorafs.sf1@1.0.0` как устаревшая в реестре и удаленная
мост CARv1 для шлюзов в производстве.

Для проверки конкретного теста PoR, пропорциональные индексы фрагментов/сегментов/частей и опционально
упорствуй на дискотеке:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Можно выбрать профиль по номеру идентификатора (`--profile-id=1`) или по дескриптору регистрации.
(И18НИ00000075Х); форма с дескриптором удобна для сценариев, которые
pasen namespace/name/semver непосредственно для метаданных правительства.США `--promote-profile=<handle>` для вывода блока метаданных JSON (включая все псевдонимы
зарегистрированных пользователей), которые можно использовать в `chunker_registry_data.rs` для продвижения новой ошибки из-за дефекта:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (и дополнительный архив данных), включая дайджест, байты нужных байтов
(шестнадцатеричное кодирование) и дайджесты сегментов/кусков для проверки, которые можно перефразировать
las capas de 64 KiB/4 KiB против доблести `por_root_hex`.

Чтобы проверить существование полезной нагрузки, пройдите по пути
`--por-proof-verify` (CLI añade `"por_proof_verified": true`, когда тестиго
совпадают с рассчитанным количеством):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Для muestreo en lote, США `--por-sample=<count>` и дополнительных пропорций для семян/салида.
CLI гарантирует определённый порядок (вид с `splitmix64`) и обрезку прозрачной формы, когда это происходит.
la solicitud supere las hojas disponibles:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

```
$ Cargo Run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    «идентификатор_профиля»: 1,
    "пространство имен": "сорафа",
    "имя": "SF1",
    "семвер": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    «мультихэш_код»: 31
  }
]
```

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: Profile_id (неявно через реестр)
    возможности: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

Шлюзы выбраны взаимным образом (из-за дефекта `sorafs.sf1@1.0.0`)
и отразить решение через заголовок ответа `Content-Chunker`. Лос-манифесты
вставьте элегантный профиль, чтобы узлы ниже по течению могли проверить расположение фрагментов
грех зависит от обмена данными по HTTP.

### Сопорте АВТОМОБИЛЬ

Сохраняем путь экспорта CARv1+SHA-2:

* **Ruta primaria** – CARv2, дайджест полезной нагрузки BLAKE3 (мультихэш `0x1f`),
  `MultihashIndexSorted`, просмотр фрагмента, зарегистрированного по прибытии.
  PUEDEN выставляет этот вариант, если клиент опустит `Accept-Chunker` или запросит
  `Accept-Digest: sha2-256`.

дополнительные возможности для перехода, но вам не придется повторно заменять канонический дайджест.

### Конформидад

* Перфил `sorafs.sf1@1.0.0` назначается общедоступным приборам на
  `fixtures/sorafs_chunker` и зарегистрированные организации в
  `fuzz/sorafs_chunker`. Комплексный подход, основанный на Rust, Go и Node
  медианте лас прюбас провистас.
* `chunker_registry::lookup_by_profile` подтверждает, что параметры дескриптора
  совпало с `ChunkProfile::DEFAULT` для устранения случайных расхождений.
* Манифесты, созданные для `iroha app sorafs toolkit pack` и `sorafs_manifest_stub`, включают метаданные реестра.