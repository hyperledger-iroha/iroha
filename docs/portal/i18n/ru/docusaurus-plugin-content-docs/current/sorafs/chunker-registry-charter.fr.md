---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр-чартер
заголовок: Charte du registere chunker SoraFS
Sidebar_label: Блокировщик реестра Charte
описание: Хартия управления для поиска и одобрения профилей.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/chunker_registry_charter.md`. Gardez les deux копирует синхронизированные копии только для полного восстановления набора наследия Сфинкса.
:::

# Charte de gouvernance du registere chunker SoraFS

> **Утверждено:** 29 октября 2025 г. на заседании Комиссии по инфраструктуре парламента Сора (голос
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Все поправки требуют
> Форма голосования по управлению; les équipes d'implémentation doivent предатель ce document comme
> Нормативное право на одобрение Хартии замены.

В этой таблице определен процесс и роли для преобразования фрагмента реестра SoraFS.
Полностью [Руководство по созданию блоков профилей] (./chunker-profile-authoring.md) и новый комментарий
профили были предложены, пересмотрены, ратифицированы и окончательно признаны недействительными.

## Порте

Аппликация на чашке `sorafs_manifest::chunker_registry` и др.
все инструменты для регистрации (манифестный CLI, рекламный CLI поставщика,
SDK). Она налагает инварианты псевдонимов и проверяемых ручных средств.
`chunker_registry::ensure_charter_compliance()` :

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone.
- Le handle canonique `namespace.name@semver` **сделайте** устройство на премьере
- Цепи псевдонимов являются тримеями, уникальными и не сталкиваются с ручками.
  канонические блюда.

## Роли

- **Автор(ы)** – подготовка предложения, подготовка светильников и сбор файлов.
  преувес детерминизма.
- **Рабочая группа по инструментам (TWG)** – действует предложение в помощь контрольным спискам
  publiées et s'assure que les invariants du registere sont уважением.
- **Управляющий совет (GC)** – изучить взаимопонимание между TWG, подписать конверт предложения
  и одобряем календари публикации/обесценивания.
- **Команда хранения** – поддержка реализации регистрации и публикации.
  les mises à jour de document.

## Поток цикла жизни

1. **Сумисс предложения**
   - Автор выполняет контрольный список проверки авторского руководства и творчества.
     JSON `ChunkerProfileProposalV1` су
     `docs/source/sorafs/proposals/`.
   - Включите CLI для вылазок:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Soumettre une PR-контентант, предложение, взаимопонимание детерминизма и т. д.
     неправильно зарегистрироваться.

2. **Обзор инструментов (TWG)**
   - Повторите контрольный список проверки (фикстуры, фазз, манифест конвейера/PoR).
   - Exécuter `cargo test -p sorafs_car --chunker-registry` и проверка, которая
     `ensure_charter_compliance()` проходит мимо нового входа.
   - Проверка совместимости CLI (`--list-profiles`, `--promote-profile`, потоковая передача
     `--json-out=-`) отобразите псевдонимы и неправильные идентификаторы.
   - Обеспечить судебное взаимопонимание по результатам проверки и невыполнения закона.3. **Утверждение совета (GC)**
   - Исследователь взаимопонимания TWG и метадоннеев предложения.
   - Подписчик дайджеста предложения (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     и добавьте подписи в конверте для совета по обслуживанию светильников.
   - Отправитель результатов голосования в протоколе управления.

4. **Публикация**
   - Fusionner la PR в течение дня:
     - `sorafs_manifest::chunker_registry_data`.
     - Документация (`chunker_registry.md`, авторские/соответствующие руководства).
     - Крепления и связи детерминизма.
   - Уведомление операторов и оборудования SDK о новом профиле и предыдущем развертывании.

5. **Обесценивание/восстановление**
   - Предложения, которые заменяют существующий профиль, включают в себя отверстие для публикации.
     двойной (периоды благодати) и план обновления.
   - По истечении срока действия льготного периода замените профиль на устаревший.
     в регистре и в журнале миграции.

6. **Срочные изменения**
   - Подавления или исправления, необходимые для голосования по совету большинства.
   - Le TWG документирует этапы смягчения рисков и измерения в журнале происшествий.

## Инструменты Attentes

- `sorafs_manifest_chunk_store` и `sorafs_manifest_stub` открыты:
  - `--list-profiles` для проверки регистрации.
  - `--promote-profile=<handle>` для создания блока канонических метадонников
    Лоры по продвижению профиля.
  - `--json-out=-` для отображения взаимосвязей со стандартным выводом, позволяющих просматривать журналы
    репродукции.
- `ensure_charter_compliance()` является вызовом для устранения дефектов в связанных с двоичными задачами проблемах.
  (`manifest_chunk_store`, `provider_advert_stub`). Les tests CI doivent échouer si
  de nouvelles entrées насильственная хартия.

## Регистрация

- Сохраните все связи детерминизма в `docs/source/sorafs/reports/`.
- «Минуты совещаний по принятию решений»
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` и `status.md` после внесения дополнительных изменений в регистрацию.

## Ссылки

- Руководство по созданию: [Руководство по созданию блоков профилей] (./chunker-profile-authoring.md)
- Контрольный список соответствия: `docs/source/sorafs/chunker_conformance.md`
- Ссылка на регистрацию: [Регистрация профилей] (./chunker-registry.md)