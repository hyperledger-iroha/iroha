---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр
заголовок: Реестр профиля чанка SoraFS
Sidebar_label: Реестр Chunker
описание: реестр SoraFS chunker, идентификаторы профилей, параметры и план переговоров.
---

:::примечание
:::

## SoraFS Реестр профиля чанка (SF-2a)

SoraFS поведение фрагментации стека Как использовать реестр с пространством имен и как вести переговоры
детерминированные параметры CDC профиля, метаданные Semver, ожидаемый дайджест/назначение нескольких кодеков, манифесты, CAR-архивы и т. д.

Авторы профиля کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Создайте метаданные, контрольный список проверки и шаблон предложения, а затем выберите нужные записи и отправьте их.
جب управление تبدیلی утвердить کر دے تو
[контрольный список развертывания реестра](./chunker-registry-rollout-checklist.md)
[постановочный манифест манифеста] (./staging-manifest-playbook) کے مطابق приспособления کو постановочный اور Production میں продвигать کریں۔

### Профили

| Пространство имен | Имя | СемВер | Идентификатор профиля | Мин (байты) | Цель (байты) | Макс. (байты) | Разбить маску | Мультихэш | Псевдонимы | Заметки |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Светильники SF-1 میں استعمال ہونے والا канонический профиль |

Регистрационный код `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) Запись `ChunkerProfileDescriptor` может быть использована в следующих случаях:

* `namespace` – профили для логической группировки (например, `sorafs`).
* `name` – انسان کے لیے читаемая метка профиля (`sf1`, `sf1-fast`, …)۔
* `semver` – набор параметров کے لیے семантическая версия строки۔.
* `profile` – اصل `ChunkProfile` (мин/цель/макс/маска)۔
* `multihash_code` – дайджесты фрагментов для мультихеширования (`0x1f`
  SoraFS по умолчанию کے لیے)۔

Манифест `ChunkingProfileV1` позволяет создавать профили и сериализовать их. یہ структура метаданных реестра
(пространство имен, имя, семвер) Необработанные параметры CDC или список псевдонимов, или список записей, или список псевдонимов, или список псевдонимов, или список записей.
Потребители могут использовать `profile_id`, выполнять поиск в реестре, использовать неизвестные идентификаторы, встроенные параметры и резервный вариант. چاہیے؛

Реестр и инструменты, а также проверка и вспомогательный CLI-интерфейс:

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

CLI и флаги JSON и JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) path کے طور پر `-` в зависимости от типа полезной нагрузки stdout потока ہوتا ہے بجائے فائل بنانے کے۔
Инструменты и канал данных Как настроить основной отчет и как настроить поведение по умолчанию

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```Взаимно поддерживаемый профиль шлюзов Наличие профиля (по умолчанию `sorafs.sf1@1.0.0`) Заголовок ответа `Content-Chunker` Отражает значение Манифесты Профиль профиля встраивание کرتے ہیں تاکہ нижестоящих узлов HTTP-согласование پر انحصار کیے بغیر, макет фрагмента validate کر سکیں۔



* **Основной путь** – CARv2, дайджест полезной нагрузки BLAKE3 (мультихэш `0x1f`),
  `MultihashIndexSorted`, профиль фрагмента اوپر کے مطابق, запись ہوتا ہے۔


### Соответствие

* `sorafs.sf1@1.0.0` профиль общедоступных светильников (`fixtures/sorafs_chunker`) или `fuzz/sorafs_chunker` کے تحت зарегистрируйте корпорацию سے match کرتا ہے۔ Сквозная четность Rust, Go и Node, тесты и упражнения, а также упражнения.
* `chunker_registry::lookup_by_profile` утверждает параметры дескриптора `ChunkProfile::DEFAULT` سے совпадение или случайное расхождение سے بچا جا سکے۔
* `iroha app sorafs toolkit pack` или `sorafs_manifest_stub` سے بنے манифестирует метаданные реестра в обычном порядке.