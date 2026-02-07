---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry
כותרת: Реестр профилей chunker SoraFS
sidebar_label: Реестр chunker
תיאור: ID профилей, параметры и план переговоров для реестра chunker SoraFS.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_registry.md`. Держите обе копии синхронизированными, пока старый набор Sphinx не будет выведен из эксплуатации.
:::

## Реестр профилей chunker SoraFS (SF-2a)

סט SoraFS согласует поведение chunking через небольшой namespaced-реестр.
Каждый профиль задает детерминированные параметры CDC, метаданные semver ו-ожидаемый digest/multicodec, используемый CAR в-מניפסטים.

Авторы профилей должны обратиться к
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для получения требуемых метаданных, чеклиста валидации и шаблона предложения перед
отправкой новых записей. После одобрения изменения со стороны ממשל следуйте
[чеклисту rollout реестра](./chunker-registry-rollout-checklist.md) и
[מניפסט של ספר הפעלה בהיערכות](./staging-manifest-playbook), чтобы продвинуть
מתקנים в בימוי и ייצור.

### פרופילים

| מרחב שמות | Имя | SemVer | ID профиля | Мин (байты) | Цель (байты) | Макс (байты) | Маска разрыва | Multihash | Алиасы | Примечания |
|-----------|-----|--------|-------------------------------------------------------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Канонический профиль, используемый в גופי SF-1 |

Реестр живет в коде как `sorafs_manifest::chunker_registry` (регулируется [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Каждая запись
выражается как `ChunkerProfileDescriptor` со следующими полями:

* `namespace` – логическая группировка связанных профилей (לדוגמה, `sorafs`).
* `name` – читаемый человеком ярлык профиля (`sf1`, `sf1-fast`, …).
* `semver` – строка семантической версии для набора параметров.
* `profile` – фактический `ChunkProfile` (מינימום/יעד/מקסימום/מסכה).
* `multihash_code` – multihash, используемый при вычислении digest чанка (`0x1f`
  ל-SoraFS по умолчанию).

Manifest сериализует профили через `ChunkingProfileV1`. Структура записывает метаданные реестра
(מרחב שם, שם, semver) вместе с сырыми CDC-параметрами и списком алиасов выше. Потребители
должны сначала попытаться выполнить lookup в реестре по `profile_id` ו- использовать inline-параметры,
когда встречаются неизвестные מזהה; список алиасов гарантирует, что HTTP-клиенты могут продолжать
ידית канонический (`namespace.name@semver`) был первой записью в `profile_aliases`, за которой

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
`--por-sample-out`), принимают `-` в качестве пути, что стримит מטען в stdout вместо
создания файла. Это облегчает пайпинг данных в tooling при сохранении стандартного
поведения печати основного отчета.### השקת Матрица совместимости и план


לא ניתן להשתמש בתקשורת בתקשורת `sorafs.sf1@1.0.0` בקליפ.
"גשר" относится к совместимому каналу CARv1 + SHA-256, который требует явной
клиентской переговорной фазы (`Accept-Chunker` + `Accept-Digest`).

| Компонент | Статус | Примечания |
|-----------|--------|--------|
| `sorafs_manifest_chunk_store` | ✅ Поддерживается | ידית Валидирует канонический + алиасы, стримит отчеты через `--json-out=-` и применяет charter реестра через SoraFS. |
| `sorafs_fetch` (מתזמר מפתח) | ✅ Поддерживается | Читает `chunk_fetch_specs`, מטענים פנויים способности `range` ו собирает выход CARv2. |
| אביזרי SDK (Rust/Go/TS) | ✅ Поддерживается | Перегенерированы через `export_vectors`; канонический מטפל במעטפות המועצה. |
| משא ומתן профиля в Torii שער | ✅ Поддерживается | Реализует полную граматику `Accept-Chunker`, включает заголовки `Content-Chunker` ו открывает CARv1 bridge тольпян לשדרג לאחור. |

Развертывание телеметрии:

- **Телеметрия fetch чанков** — CLI Iroha `sorafs toolkit pack` эмитит מעכל чанков, метаданные CAR ו- корни PoR לבליעה בלוח המחוונים.
- **פרסומות ספק** — פרסומות מטענים включают метаданные יכולות וכינויים; проверяйте покрытие через `/v1/sorafs/providers` (לדוגמה, יכולת наличие `range`).
- **שער Мониторинг** — операторы должны сообщать пары `Content-Chunker`/`Content-Digest`, чтобы обнаружаневать обнаружанивать downgrade; ожидается, что использование bridge снизится до нуля до депрекации.

Политика депрекации: как только утвержден профиль-преемник, запланируйте окно двойной публикации
גשר CARv1 из שערי ייצור.

Чтобы проверить конкретного свидетеля PoR, укажите индексы chunk/segment/leaf и при необходимости
сохраните הוכחה на диск:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Вы можете выбрать профиль по числовому id (`--profile-id=1`) או по handle реестра
(`--profile=sorafs.sf1@1.0.0`); ידית פורמט удобен для скриптов, которые
הצג מרחב שמות/שם/semver עבור מטא נתונים של ממשל.

הצג את `--promote-profile=<handle>` עבור вывода JSON-Bлока метаданных (включая все
зарегистрированные алиасы), который можно вставить в `chunker_registry_data.rs` при
продвижении нового профиля по умолчанию:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (ו необязательный файл הוכחה) включает корневой digest, байты выбранного עלה
(в hex) и Sibling digests сегмента/чанка, чтобы верификаторы могли пересчитать hash слоев
64 KiB/4 KiB относительно значения `por_root_hex`.

Чтобы валидировать существующий הוכחה относительно מטען, передайте путь через
`--por-proof-verify` (CLI добавляет `"por_proof_verified": true`, когда свидетель
совпадает с вычисленным корнем):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Для пакетной выборки используйте `--por-sample=<count>` и при желании задайте seed/путь вывода.
CLI гарантирует детерминированный порядок (`splitmix64` seeded) и прозрачно усечет запрос,
когда он превышает число доступных листьев:```
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
    "פרופיל_מזהה": 1,
    "namespace": "סורפים",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "גודל_מינימלי": 65536,
    "מטרת_גודל": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "קוד multihash": 31
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
    יכולות: [...]
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

Gateways выбирают взаимно поддерживаемый профиль (по умолчанию `sorafs.sf1@1.0.0`) ו отражают
решение в ответном заголовке `Content-Chunker`. Manifests встраивают выбранный профиль, чтобы
узлы במורד הזרם могли валидировать раскладку чанков ללא אופציות על HTTP-переговоры.

### Совместимость CAR

сохраняется путь экспорта CARv1+SHA-2:

* **Основной путь** – CARv2, BLAKE3 תקציר מטען (`0x1f` multihash),
  `MultihashIndexSorted`, профиль chunk записан как выше.
  МОГУТ выдавать этот вариант, когда клиент опускает `Accept-Chunker` או זרקור
  `Accept-Digest: sha2-256`.

заголовки для совместимости, но не должны заменять канонический digest.

### Соответствие

* Профиль `sorafs.sf1@1.0.0` соответствует публичным גופי в
  `fixtures/sorafs_chunker` и corpora, зарегистрированным в
  `fuzz/sorafs_chunker`. מקצה לקצה паритет проверяется в Rust, Go и Node
  через предоставленные тесты.
* `chunker_registry::lookup_by_profile` подтверждает, что параметры дескриптора
  совпадают с `ChunkProfile::DEFAULT`, чтобы защититься от случайной дивергенции.
* מניפסטים, производимые `iroha app sorafs toolkit pack` ו-`sorafs_manifest_stub`, включают метаданные реестра.