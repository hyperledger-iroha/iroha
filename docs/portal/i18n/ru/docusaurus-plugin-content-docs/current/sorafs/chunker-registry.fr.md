---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр
заголовок: Registre des profils chunker SoraFS
Sidebar_label: Регистрационный блокировщик
описание: идентификаторы профиля, параметры и план согласования для регистрации блока SoraFS.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/chunker_registry.md`. Gardez les deux копирует синхронизированные копии только для полного восстановления набора наследия Сфинкса.
:::

## Регистр профилей чанка SoraFS (SF-2a)

Стек SoraFS регулирует возможности фрагментации через пространство имен малого реестра.
Каждый профиль назначает параметры, определенные CDC, метадоны каждый раз и дайджест/мультикодек, используемый в манифестах и ​​архивах CAR.

Консультант для авторов профилей
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для метадоннических требований, контрольного списка проверки и модели предстоящего предложения
de soumettre de nouvelles entrées. Любая модификация одобрена по закону
управление, suivez la
[контрольный список развертывания регистрации] (./chunker-registry-rollout-checklist.md) и др.
[книга манифеста в постановке] (./staging-manifest-playbook) для промо
Светильники, постановка и производство.

### Профили

| Пространство имен | Ном | СемВер | ID профиля | Мин (октеты) | Cible (октеты) | Макс (октеты) | Маска разрыва | Мультихэш | Псевдоним | Заметки |
|-----------|-----|--------|-------------|---------------|----------------|--------------|------------------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Канонический профиль, используемый в светильниках SF-1 |

Зарегистрируйтесь с кодом `sorafs_manifest::chunker_registry` (регистрационный номер [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Чак-закуска
примерно соответствует `ChunkerProfileDescriptor` с:

* `namespace` – логика перегруппировки профилей лжи (например, `sorafs`).
* `name` – доступная информация (`sf1`, `sf1-fast`, …).
* `semver` – семантическая цепочка версий для параметров игры.
* `profile` – ролик `ChunkProfile` (мин/цель/макс/маска).
* `multihash_code` – мультихэш используется для создания дайджестов фрагментов (`0x1f`
  по умолчанию SoraFS).

Манифест сериализует профили через `ChunkingProfileV1`. Структура регистрации метадоннеев
du registre (пространство имен, имя, имя) aux côtés des paramètres CDC bruts et de la liste d'alias ci-dessus.
Les consommateurs doivent d'abord tenter une recherche dans le registre par `profile_id` et revenir aux
встроенные параметры для идентификаторов неподключенного устройства; список псевдонимов гарантирует HTTP-клиентам
вам необходимо зарегистрироваться, чтобы канонический дескриптор (`namespace.name@semver`) был первым входом
`profile_aliases`, suivi des alias hérités.

Для того чтобы инспектор зарегистрировал необходимые утилиты, запустите помощник CLI:

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
```Все флаги CLI, зарегистрированные в JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) принимает `-` как процесс, который передает полезную нагрузку вместо стандартного вывода или вместо него.
создай документ. Cela facilite le piping des données vers les outils tout en conservant le
поведение по умолчанию для установления основного взаимопонимания.

### Матрица развертывания и план развертывания


Le tableau ci-dessous захватывает действующий статус поддержки для `sorafs.sf1@1.0.0` в этих файлах
Принципы композиторов. «Мост» разработан для Voie CARv1 + SHA-256, который
Требуется явный клиент для переговоров (`Accept-Chunker` + `Accept-Digest`).

| Композитор | Статут | Заметки |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Поддержка | Подтвердите канонический дескриптор + псевдоним, поток сообщений через `--json-out=-` и применение карты регистрации через `ensure_charter_compliance()`. |
| `sorafs_fetch` (оркестратор разработчика) | ✅ Поддержка | Зажгите `chunk_fetch_specs`, соберите полезные нагрузки `range` и соберите вылет CARv2. |
| SDK фикстур (Rust/Go/TS) | ✅ Поддержка | Обновить через `export_vectors` ; Ручка канонического устройства находится на первом месте в списке псевдонимов и является знаком конвертов совета. |
| Согласование профилей шлюза Torii | ✅ Поддержка | Внедрите всю грамматику `Accept-Chunker`, включая заголовки `Content-Chunker` и не раскрывайте мост CARv1, который соответствует явным требованиям перехода на более раннюю версию. |

Развертывание телеметрии:

- **Телеметрия выборки фрагментов** — CLI Iroha `sorafs toolkit pack` обеспечивает дайджесты фрагментов, метадонные CAR и расовые PoR для загрузки на информационные панели.
- **Объявления поставщика** — полезные данные рекламы, включающие метадонные емкости и псевдонимы; активировать кувертюру через `/v2/sorafs/providers` (например, наличие емкости `range`).
- **Шлюз наблюдения** — операторы связывают соединения `Content-Chunker`/`Content-Digest` для обнаружения невнимательного перехода на более раннюю версию; l'usage du Bridge - это цензура, которая равняется нулю перед обесцениванием.

Политика девальвации: une fois qu'un profil Successeur est Ratifié, planifiez une uneetre de двойной публикации
(документ в предложении) до марки `sorafs.sf1@1.0.0`, как устаревший до регистрации и выбытия
мост CARv1 для шлюзов в производстве.

Инспектор заливки для конкретного времени PoR, четыре индекса фрагмента/сегмента/фрагмента и т. д., в качестве опции,
persistez la preuve sur disque:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Вы можете выбрать профиль по номеру идентификатора (`--profile-id=1`) или по дескриптору регистрации
(`--profile=sorafs.sf1@1.0.0`) ; La forme handle est pratique pour les scripts qui
трансметентное пространство имен/имя/semver направление depuis les métadonnées de gouvernance.

Используйте `--promote-profile=<handle>` для создания метадонированного блока JSON (и содержит все псевдонимы).
зарегистрируйтесь), который может быть найден в `chunker_registry_data.rs` для продвижения нового профиля
по умолчанию:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```Основной раппорт (et le fichier de preuve optionnel), включая расовый дайджест, les октеты de feuille échantillonnés
(закодировано в шестнадцатеричном формате) и дайджесты фрагментов/фрагментов, которые могут быть повторно проверены
диваны размером 64 КБ/4 КБ имеют значение `por_root_hex`.

Чтобы проверить существующее состояние против полезной нагрузки, пройдите через
`--por-proof-verify` (CLI ajoute `"por_proof_verified": true`, открывающий тему
соответствует расовому расчету):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Для частого сбора урожая используйте `--por-sample=<count>` и приготовьтесь к посадке/вылазке.
CLI гарантирует определённый порядок (задание с `splitmix64`) и автоматический запуск двигателя
la requête dépasse les feuilles disponibles:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

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

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: Profile_id (неявно через реестр)
    возможности: [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

Шлюзы, выбранные с поддержкой взаимного профиля (по умолчанию `sorafs.sf1@1.0.0`)
и отражает решение через заголовок ответа `Content-Chunker`. Лес манифесты
Интегрированный профиль по выбору, который позволяет нижестоящим узлам проверять расположение фрагментов
без необходимости использования HTTP-переговоров.

### Поддержка АВТОМОБИЛЯ

мы сохраняем путь экспорта CARv1+SHA-2:

* **Принципал Chemin** – CARv2, дайджест полезной нагрузки BLAKE3 (мультихеш `0x1f`),
  `MultihashIndexSorted`, зарегистрируйте профиль фрагмента как один.
  PEUVENT раскрывает этот вариант для клиента Omet `Accept-Chunker` или по требованию
  `Accept-Digest: sha2-256`.

Дополнительные сведения для перехода больше не нужны для замены канонического дайджеста.

### Соответствует

* Профиль `sorafs.sf1@1.0.0` соответствует дополнительным публичным светильникам в
  `fixtures/sorafs_chunker` и другие зарегистрированные в нашей компании корпорации
  `fuzz/sorafs_chunker`. La parité de bout en bout est exercée en Rust, Go et Node
  через les test fournis.
* `chunker_registry::lookup_by_profile` подтверждает параметры описания
  корреспондент `ChunkProfile::DEFAULT` для предотвращения всех случайных расхождений.
* Декларации о продуктах с номерами `iroha app sorafs toolkit pack` и `sorafs_manifest_stub` включают в себя метадонники регистрации.