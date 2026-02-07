---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр-чартер
заголовок: Carta do registro de chunker da SoraFS
Sidebar_label: Карта регистрации фрагментов
описание: Хартия управления для подчинения и подтверждения работоспособности блока.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/chunker_registry_charter.md`. Мантенья представился как копиас синхронизадас.
:::

# Хартия управления регистром фрагментов данных SoraFS

> **Утверждение:** 29 октября 2025 г. Комиссия по инфраструктуре парламента Пело Сора (veja
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qualquer memenda requer um
> формальное голосование за правительство; Оборудования для реализации должны использовать этот документ как
> Нормативно было одобрено, что эта карта заменила это.

Эта карта определяет процесс и файлы папок для эволюции реестра фрагментов SoraFS.
Дополните [Guia de autoria de perfis de chunker] (./chunker-profile-authoring.md) как новое описание
были предложены, пересмотрены, ратифицированы и в конечном итоге прекращены.

## Эскопо

Карта будет применена каждый раз в `sorafs_manifest::chunker_registry` e
Качественный инструментарий, который используется для регистрации (манифестный CLI, CLI для рекламы провайдера,
SDK). Она вводит неизменяемые псевдонимы и обрабатывает проверенные данные.
`chunker_registry::ensure_charter_compliance()`:

- Идентификаторы результатов в виде внутренних положительных результатов, которые увеличивают монотонную форму.
- O handle canonico `namespace.name@semver` **deve** aparecer como a primeira
  вход в `profile_aliases`. Альтернативные псевдонимы могут быть следующими.
- В качестве строк псевдонимов устройств unicas e nao colidem com обрабатываются canonicos.
  де outras entradas.

## Папейс

- **Автор(а)** - подготовить предложение, перегенерировать светильники и выбрать
  доказательства детерминизма.
- **Рабочая группа по инструментам (TWG)** – проверка предложения использования контрольных списков
  Публикации и подтверждения того, что неизменные варианты регистрации всегда присутствуют.
- **Управляющий совет (GC)** - пересмотр или отношение к TWG, заверенный или конверт с предложением
  Электронное одобрение публичных/прекращающихся мероприятий.
- **Команда хранения** — создание системы регистрации и публикации.
  аутуализация документов.

## Fluxo do ciclo de vida

1. **Подача предложения**
   - Автор выполняет контрольный список проверки подлинности авторских прав и криков.
     хм JSON `ChunkerProfileProposalV1` рыдание
     `docs/source/sorafs/proposals/`.
   - Включена информация о CLI:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie um PR contendo приспособления, предложения, отношения детерминизма и
     атуализационно сделать регистрацию.

2. **Ревизия инструмента (TWG)**
   - Повторите контрольный список проверки (фикстуры, фазз, конвейер манифеста/PoR).
   - Выполните `cargo test -p sorafs_car --chunker-registry` и гарантируйте, что
     `ensure_charter_compliance()` прошло с новой стороны.
   - Проверка совместимости с CLI (`--list-profiles`, `--promote-profile`, потоковая передача
     `--json-out=-`) отображаются псевдонимы и обрабатываются автоматически.
   - Produza um relatorio curto resumindo achados и status de aprovacao/reprovacao.3. **Утверждение совета (GC)**
   - Пересмотр отношений TWG и предлагаемых метаданных.
   - Assine o Digest da proposta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     И приложение в качестве ассинатуры или конверта для совета по сбору средств для оборудования.
   - Зарегистрируйте результаты голосования по государственному управлению.

4. **Публикация**
   - Слияние фасадов для PR, предварительно:
     - `sorafs_manifest::chunker_registry_data`.
     - Документация (`chunker_registry.md`, руководство автора/согласования).
     - Светильники и соотношения детерминизма.
   - Уведомления операторов и оснащение SDK о новых профилях и планах развертывания.

5. **Уничтожение / Encerramento**
   - Предложения о том, что можно заменить существующую опасность, включив в нее публичную публикацию.
     дупла (периоды каренсии) и план обновления.
     нет регистрации и актуализации бухгалтерской книги миграции.

6. **Непредвиденные обстоятельства**
   - Удаления или исправления могут быть проголосованы за одобрение в большинстве случаев.
   - TWG документирует этапы снижения риска и актуализирует журнал инцидентов.

## Ожидания от инструментов

- `sorafs_manifest_chunk_store` и `sorafs_manifest_stub` объяснение:
  - `--list-profiles` для проверки регистрации.
  - `--promote-profile=<handle>` для включения или блокировки используемых канонических метаданных
    ао промоутер гм перфил.
  - `--json-out=-` для передачи данных на стандартный вывод, привычные журналы проверки
    репродуктивный.
- `ensure_charter_compliance()` и вызов для инициализации соответствующих двоичных файлов
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes de CI devem falhar se
  novas entradas violarem a carta.

## Регистр

- Армазенские все связи детерминизма в `docs/source/sorafs/reports/`.
- Как сообщается, референсиам принимает решения о чанкере
  `docs/source/sorafs/migration_ledger.md`.
- Атуализировать `roadmap.md` и `status.md` после каждого отдельного случая без регистрации.

## Ссылки

- Руководство по авторизации: [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Контрольный список соответствия: `docs/source/sorafs/chunker_conformance.md`
- Ссылка на регистрацию: [Регистрация перфиса блоков] (./chunker-registry.md)