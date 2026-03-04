---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр-чартер
заголовок: Карта регистрации фрагментов SoraFS
Sidebar_label: Карта регистрации фрагментов
описание: Хартия губернатора для презентаций и апробаций файлов-чанкеров.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/chunker_registry_charter.md`. Нам пришлось скопировать синхронизированные копии, чтобы удалить комплект документации Sphinx.
:::

# Карта управления реестром фрагментов SoraFS

> **Утверждение:** 29 октября 2025 г., комиссия по инфраструктуре парламента Эль-Сора (версия
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Cualquier enmienda requiere un
> формальное голосование губернатора; оборудование для реализации должно быть использовано в этом документе как
> Нормативно необходимо, чтобы была подтверждена хартия, которая удовлетворяет требованиям.

Эта хартия определяет процесс и роли для развития реестра блоков SoraFS.
Дополнение к [Руководству по авторизации файлов фрагментов] (./chunker-profile-authoring.md) с описанием как новое
Перфили будут предложены, пересмотрены, ратифицированы и в конечном итоге обесценятся.

## Альканс

Карта приложения для каждого входа в `sorafs_manifest::chunker_registry` y
более широкий инструментарий для использования реестра (CLI манифеста, CLI объявления поставщика,
SDK). Удаление неизменяемых псевдонимов и обработка проверенных данных
`chunker_registry::ensure_charter_compliance()`:

- Los IDs de perfil son enteros positivos que aumentan de forma monotona.
- Ручка canónico `namespace.name@semver` **debe** aparecer como primera
  вход в `profile_aliases`. Siguen los alias heredados.
- Las cadenas de alias se recortan, son únicas y no colisionan con handles canónicos
  де другие входы.

## Роли

- **Автор(а)** – подготовка имущества, регенерация приспособлений и копирование
  доказательства детерминизма.
- **Рабочая группа по инструментам (TWG)** – проверка правильности использования контрольных списков
  Публикации и подтверждения того, что неизменные варианты регистрации являются обязательными.
- **Управляющий совет (GC)** – пересмотр отчета TWG, фирма по обеспечению собственности
  и будут отменены площади публикации/устаривания.
- **Команда хранения** – управление реализацией реестра и публикации.
  актуализация документации.

## Flujo del ciclo de vida

1. **Презентация имущества**
   - Автор выдает контрольный список проверки руководства и создания
     un JSON `ChunkerProfileProposalV1` ru
     `docs/source/sorafs/proposals/`.
   - Включите раздел CLI:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Отправка PR-материалов о предстоящих событиях, публикациях, репортажах о детерминизме и
     актуализация реестра.

2. **Ревизия инструмента (TWG)**
   - Воспроизведите контрольный список проверки (фикстуры, фазз, конвейер манифеста/PoR).
   - Выбросьте `cargo test -p sorafs_car --chunker-registry` и убедитесь, что
     `ensure_charter_compliance()` шаг с новым входом.
   - Проверка совместимости CLI (`--list-profiles`, `--promote-profile`, потоковая передача
     `--json-out=-`) отражает псевдоним и обрабатывает актуализированные файлы.
   - Подготовьте краткий отчет о результатах проверки и подтверждении фактов.3. **Проверка совета (GC)**
   - Проверка отчета TWG и метаданных собственности.
   - Фирма-эл-дайджест де-ла-пропуэста (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     и снова фирмы, которые собирают советы по ремонту оборудования.
   - Зарегистрируйте результат голосования в государственных актах.

4. **Публикация**
   - Fusiona el PR, актуализировано:
     - `sorafs_manifest::chunker_registry_data`.
     - Документация (`chunker_registry.md`, авторские/согласованные инструкции).
     - Светильники и отчеты о детерминизме.
   - Уведомление операторов и оборудования SDK о новом профиле и запланированном развертывании.

5. **Устаревание / Ретиро**
   - Las propuestas que sustituyen un perfil existente deben cluir una ventana de publicación
     двойной (периоды благодати) и план реализации.
     в реестре и актуализировать книгу миграции.

6. **Камбии в чрезвычайных ситуациях**
   - Для устранения исправлений или исправлений требуется голосование за совет с одобрением мэрии.
   - TWG должна документировать действия по смягчению последствий рисков и актуализировать реестр инцидентов.

## Ожидания от инструментов

- `sorafs_manifest_chunk_store` и `sorafs_manifest_stub` показаны:
  - `--list-profiles` для проверки реестра.
  - `--promote-profile=<handle>` для создания блока метаданных используемого канона.
    Аль-промоутер ип-перфил.
  - `--json-out=-` для передачи сообщает о стандартном выводе, привычном журнале изменений
    воспроизводимые.
- `ensure_charter_compliance()` вызывается в соответствующих бинарных файлах.
  (`manifest_chunk_store`, `provider_advert_stub`). Las Pruebas CI deben Fallar Si
  Nuevas Entradas Violan La Carta.

## Регистр

- Следите за всеми отчетами о детерминизме в `docs/source/sorafs/reports/`.
- Las actas del consejo que справочные решения о жизнеспособности фрагмента
  `docs/source/sorafs/migration_ledger.md`.
- Актуализация `roadmap.md` и `status.md` после каждого камбио-мэра реестра.

## Ссылки

- Руководство по авторизации: [Guía de autoría de perfiles de chunker](./chunker-profile-authoring.md)
- Контрольный список соответствия: `docs/source/sorafs/chunker_conformance.md`
- Ссылка на реестр: [Регистр файлов фрагментов] (./chunker-registry.md)