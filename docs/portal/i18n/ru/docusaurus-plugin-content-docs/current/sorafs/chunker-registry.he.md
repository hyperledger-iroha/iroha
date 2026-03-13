---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e157ade512fcc349a53fb02d7332c94a16778a6bdc959842463b0e9ef8418797
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_registry.md`. Держите обе копии синхронизированными, пока старый набор Sphinx не будет выведен из эксплуатации.
:::

## Реестр профилей chunker SoraFS (SF-2a)

Стек SoraFS согласует поведение chunking через небольшой namespaced-реестр.
Каждый профиль задает детерминированные параметры CDC, метаданные semver и ожидаемый digest/multicodec, используемый в manifests и CAR-архивах.

Авторы профилей должны обратиться к
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для получения требуемых метаданных, чеклиста валидации и шаблона предложения перед
отправкой новых записей. После одобрения изменения со стороны governance следуйте
[чеклисту rollout реестра](./chunker-registry-rollout-checklist.md) и
[playbook manifest в staging](./staging-manifest-playbook), чтобы продвинуть
fixtures в staging и production.

### Профили

| Namespace | Имя | SemVer | ID профиля | Мин (байты) | Цель (байты) | Макс (байты) | Маска разрыва | Multihash | Алиасы | Примечания |
|-----------|-----|--------|------------|-------------|--------------|--------------|---------------|-----------|--------|------------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Канонический профиль, используемый в fixtures SF-1 |

Реестр живет в коде как `sorafs_manifest::chunker_registry` (регулируется [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Каждая запись
выражается как `ChunkerProfileDescriptor` со следующими полями:

* `namespace` – логическая группировка связанных профилей (например, `sorafs`).
* `name` – читаемый человеком ярлык профиля (`sf1`, `sf1-fast`, …).
* `semver` – строка семантической версии для набора параметров.
* `profile` – фактический `ChunkProfile` (min/target/max/mask).
* `multihash_code` – multihash, используемый при вычислении digest чанка (`0x1f`
  для SoraFS по умолчанию).

Manifest сериализует профили через `ChunkingProfileV1`. Структура записывает метаданные реестра
(namespace, name, semver) вместе с сырыми CDC-параметрами и списком алиасов выше. Потребители
должны сначала попытаться выполнить lookup в реестре по `profile_id` и использовать inline-параметры,
когда встречаются неизвестные ID; список алиасов гарантирует, что HTTP-клиенты могут продолжать
канонический handle (`namespace.name@semver`) был первой записью в `profile_aliases`, за которой

Чтобы посмотреть реестр из tooling, запустите helper CLI:

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
```

Все флаги CLI, которые пишут JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`), принимают `-` в качестве пути, что стримит payload в stdout вместо
создания файла. Это облегчает пайпинг данных в tooling при сохранении стандартного
поведения печати основного отчета.

### Матрица совместимости и план rollout


Таблица ниже отражает текущий статус поддержки `sorafs.sf1@1.0.0` в ключевых компонентах.
"Bridge" относится к совместимому каналу CARv1 + SHA-256, который требует явной
клиентской переговорной фазы (`Accept-Chunker` + `Accept-Digest`).

| Компонент | Статус | Примечания |
|-----------|--------|------------|
| `sorafs_manifest_chunk_store` | ✅ Поддерживается | Валидирует канонический handle + алиасы, стримит отчеты через `--json-out=-` и применяет charter реестра через `ensure_charter_compliance()`. |
| `sorafs_fetch` (developer orchestrator) | ✅ Поддерживается | Читает `chunk_fetch_specs`, понимает payloads способности `range` и собирает выход CARv2. |
| SDK fixtures (Rust/Go/TS) | ✅ Поддерживается | Перегенерированы через `export_vectors`; канонический handle идет первым в каждом списке алиасов и подписан council envelopes. |
| Negotiation профиля в Torii gateway | ✅ Поддерживается | Реализует полную грамматику `Accept-Chunker`, включает заголовки `Content-Chunker` и открывает CARv1 bridge только по явным запросам downgrade. |

Развертывание телеметрии:

- **Телеметрия fetch чанков** — CLI Iroha `sorafs toolkit pack` эмитит digests чанков, метаданные CAR и корни PoR для ingestion в dashboards.
- **Provider adverts** — payloads adverts включают метаданные capabilities и aliases; проверяйте покрытие через `/v2/sorafs/providers` (например, наличие capability `range`).
- **Мониторинг gateway** — операторы должны сообщать пары `Content-Chunker`/`Content-Digest`, чтобы обнаруживать неожиданные downgrades; ожидается, что использование bridge снизится до нуля до депрекации.

Политика депрекации: как только утвержден профиль-преемник, запланируйте окно двойной публикации
CARv1 bridge из production gateways.

Чтобы проверить конкретного свидетеля PoR, укажите индексы chunk/segment/leaf и при необходимости
сохраните proof на диск:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Вы можете выбрать профиль по числовому id (`--profile-id=1`) или по handle реестра
(`--profile=sorafs.sf1@1.0.0`); формат handle удобен для скриптов, которые
передают namespace/name/semver напрямую из governance metadata.

Используйте `--promote-profile=<handle>` для вывода JSON-блока метаданных (включая все
зарегистрированные алиасы), который можно вставить в `chunker_registry_data.rs` при
продвижении нового профиля по умолчанию:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (и необязательный файл proof) включает корневой digest, байты выбранного leaf
(в hex) и sibling digests сегмента/чанка, чтобы верификаторы могли пересчитать hash слоев
64 KiB/4 KiB относительно значения `por_root_hex`.

Чтобы валидировать существующий proof относительно payload, передайте путь через
`--por-proof-verify` (CLI добавляет `"por_proof_verified": true`, когда свидетель
совпадает с вычисленным корнем):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Для пакетной выборки используйте `--por-sample=<count>` и при желании задайте seed/путь вывода.
CLI гарантирует детерминированный порядок (`splitmix64` seeded) и прозрачно усечет запрос,
когда он превышает число доступных листьев:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub отражает те же данные, что удобно при скриптинге выбора `--chunker-profile-id`
в пайплайнах. Оба chunk store CLI также принимают канонический формат handle
(`--profile=sorafs.sf1@1.0.0`), поэтому build-скрипты могут избежать жесткого
хардкода числовых ID:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Поле `handle` (`namespace.name@semver`) совпадает с тем, что CLIs принимают через
`--profile=…`, поэтому его можно безопасно копировать в автоматизацию.

### Согласование chunker

Gateways и клиенты объявляют поддерживаемые профили через provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (неявно через реестр)
    capabilities: [...]
}
```

Планирование multi-source чанков объявляется через capability `range`. CLI принимает ее с
`--capability=range[:streams]`, где опциональный числовой суффикс кодирует предпочтительную
параллельность range-fetch у провайдера (например, `--capability=range:64` объявляет бюджет 64 streams).
Когда суффикс отсутствует, потребители возвращаются к общему hint `max_streams`, опубликованному в другом
месте advert.

При запросе CAR-данных клиенты должны отправлять заголовок `Accept-Chunker`, перечисляя кортежи
`(namespace, name, semver)` в порядке предпочтения:

```

Gateways выбирают взаимно поддерживаемый профиль (по умолчанию `sorafs.sf1@1.0.0`) и отражают
решение в ответном заголовке `Content-Chunker`. Manifests встраивают выбранный профиль, чтобы
узлы downstream могли валидировать раскладку чанков без опоры на HTTP-переговоры.

### Совместимость CAR

сохраняется путь экспорта CARv1+SHA-2:

* **Основной путь** – CARv2, BLAKE3 payload digest (`0x1f` multihash),
  `MultihashIndexSorted`, профиль chunk записан как выше.
  МОГУТ выдавать этот вариант, когда клиент опускает `Accept-Chunker` или запрашивает
  `Accept-Digest: sha2-256`.

заголовки для совместимости, но не должны заменять канонический digest.

### Соответствие

* Профиль `sorafs.sf1@1.0.0` соответствует публичным fixtures в
  `fixtures/sorafs_chunker` и corpora, зарегистрированным в
  `fuzz/sorafs_chunker`. End-to-end паритет проверяется в Rust, Go и Node
  через предоставленные тесты.
* `chunker_registry::lookup_by_profile` подтверждает, что параметры дескриптора
  совпадают с `ChunkProfile::DEFAULT`, чтобы защититься от случайной дивергенции.
* Manifests, производимые `iroha app sorafs toolkit pack` и `sorafs_manifest_stub`, включают метаданные реестра.
