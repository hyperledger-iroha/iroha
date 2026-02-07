---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Адаптировано из [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Политика подтверждения и идентификации провайдеров SoraFS (черновик SF-2b)

Эта записка фиксирует практические результаты для **SF-2b**: определение и
необходимость применять процесс с учетом требований к идентичности и
аттестационные payload'ы для провайдеров хранения SoraFS. Она регулирует
процесс высокого уровня, описанный в RFC-архитектуре SoraFS, и разбивается
Оставшаяся работа на отслеживаемые инженерные задачи.

## Цели политики

- Гарантировать, что только проверенные операторы могут публиковать записи `ProviderAdvertV1`, которые принимает сеть.
- Привязать каждый ключ объявления к документу об идентичности, утвержденному управлению, аттестованным эндпоинтам и минимальному вкладу.
- Предоставить определенные инструменты проверок, чтобы Torii, шлюзы и `sorafs-node` применялись одинаковые проверки.
- Поддержка обновлений и аварийное аннулирование без нарушений детерминизма или удобных инструментов.

## Требования к идентичности и ставке

| Требование | Описание | Результат |
|------------|----------|-----------|
| Происхождение включения объявления | Провайдеры должны зарегистрировать пару ключей Ed25519, которыми подписывается каждое объявление. Бандл допускает хранение открытого ключа вместе с подписью управления. | Расширить схему `ProviderAdmissionProposalV1` полем `advert_key` (32 байта) и сослаться на нее из реестра (`sorafs_manifest::provider_admission`). |
| Указатель ставки | Для допуска требуется ненулевой `StakePointer`, указывающий на активный пул ставок. | Добавьте валидацию в `sorafs_manifest::provider_advert::StakePointer::validate()` и выведите ошибки в CLI/тестах. |
| Теги юрисдикции | Провайдеры объявляют юрисдикцию + юридический контакт. | Расширить схему предложений полем `jurisdiction_code` (ISO 3166-1 альфа-2) и опциональным `contact_uri`. |
| Аттестация эндпоинта | Каждый заявленный эндпоинт должен быть подкреплен отчетом сертификата mTLS или QUIC. | Определить полезную нагрузку Norito `EndpointAttestationV1` и хранить ее на каждой конечной точке в бандле, допускающем. |

## Процесс допуска

1. **Создание предложения**
   - CLI: добавить `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`,
     формирующий `ProviderAdmissionProposalV1` + бандл аттестации.
   - Проверка: подтверждено наличие обязательных полей, доля > 0, дескриптор канонического чанкера в `profile_id`.
2. **Одобрение управления**
   - Совет подписывает `blake3("sorafs-provider-admission-v1" || canonical_bytes)` с учетом обстоятельств.
     инструменты конверт (модуль `sorafs_manifest::governance`).
   - Конверт сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Внесение в реестр**
   - Реализовать общий валидатор (`sorafs_manifest::provider_admission::validate_envelope`), который
     переют используют Torii/шлюзы/CLI.
   - Обновить путь, допуская Torii, чтобы отклонить сроки рекламы, в которых дайджест или действие отличаются от конверта.
4. **Обновление и отзыв**
   - Добавить `ProviderAdmissionRenewalV1` с опциональными обновлениями эндпоинтов/стейка.
   - Открыть путь CLI `--revoke`, который фиксирует причину отзыва и отправляет управление событием.

## Задачи реализации| область | Задача | Владелец(и) | Статус |
|---------|--------|----------|--------|
| Схема | Определить `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) в `crates/sorafs_manifest/src/provider_admission.rs`. Реализовано в `sorafs_manifest::provider_admission` с помощниками проверки.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хранение/Управление | ✅ Завершено |
| Инструменты CLI | Расширить `sorafs_manifest_stub` подкомандами: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | ✅ Завершено |

Поток CLI теперь принимает сертификаты промежуточных бандлов (`--endpoint-attestation-intermediate`),
выдает стандартные байты предложений/конвертов и наконец-то совет во время `sign`/`verify`. Операторы могут
Объявление о передаче тела напрямую или переиспользовать подписанные объявления, а файлы с подпиской можно
сочетание, сочетание `--council-signature-public-key` с `--council-signature-file` для удобства автоматизации.

### Справочник CLI

Запускайте каждую команду через `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Обязательные флаги: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>` и как минимум один `--endpoint=<kind:host>`.
  - Для аттестации каждого эндпоинта требуется `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, сертификат через
    `--endpoint-attestation-leaf=<path>` (плюс опциональный `--endpoint-attestation-intermediate=<path>`
    для каждого элемента цепочки) и любой согласованный идентификатор ALPN.
    (`--endpoint-attestation-alpn=<token>`). Для транспортных эндпоинтов QUIC можно передавать отчеты через
    `--endpoint-attestation-report[-hex]=...`.
  - Выход: стандартные байты предложений Norito (`--proposal-out`) и JSON-сводка.
    (стандартный вывод по умолчанию или `--json-out`).
- `sign`
  - Входные данные: предложение (`--proposal`), подписанное объявление (`--advert`), дополнительное объявление о кузове.
    (`--advert-body`), эпоха хранения и как минимум одна подпись. Подписи можно передать
    встроенный (`--council-signature=<signer_hex:signature_hex>`) или через файлы, совмещая
    `--council-signature-public-key` с `--council-signature-file=<path>`.
  - Формирует валидированный конверт (`--envelope-out`) и JSON-отчет с привязками дайджеста,
    числами подписантов и входными путями.
- `verify`
  - Проверяет существующий конверт (`--envelope`) и опционально сверяет содержание предложения,
    реклама или реклама тела. JSON-отчет подсвечивает дайджест значений, проверка статуса подписей
    и какие дополнительные аксессуары совпали.
- `renewal`
  - Связывает новый заявленный конверт с ранее утвержденным сборником. Требуются
    `--previous-envelope=<path>` и последующий `--envelope=<path>` (оба полезных данных Norito).
    CLI впоследствии показал, что псевдонимы профилей, возможности и рекламный ключ остаются неизменными, при этом допускается
    обновления стейка, эндпоинтов и метаданных. Выводит канонические байты `ProviderAdmissionRenewalV1`
    (`--renewal-out`) плюс JSON-сводку.
- `revoke`
  - Выпускает аварийный бандл `ProviderAdmissionRevocationV1` для провайдера, чей конверт нужно отозвать.
    Требует `--envelope=<path>`, `--reason=<text>`, как минимум одну `--council-signature` и опциональные
    `--revoked-at`/`--notes`. CLI подписывает и, наконец, дайджест обзора, записывает полезную нагрузку Norito через
    `--revocation-out` и печатает JSON-отчет с дайджестом и числом подписей.
| Проверка | Реализовать общий валидатор, результат Torii, шлюзами и `sorafs-node`. Предоставить модуль + интеграционные тесты CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сетевые TL/хранилище | ✅ Завершено |
| Интеграция Torii | Подключить валидатор для приема рекламы в Torii, отклонять рекламу вне политики и публиковать телеметрию. | Сеть TL | ✅ Завершено | Torii теперь загружает конверты управления (`torii.sorafs.admission_envelopes_dir`), сначала совпадение дайджеста/подписи при приеме и публикацию телеметрии допускаем.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || Обновление | Добавьте обновление/отзыв схемы + помощники CLI, опубликуйте жизненный цикл в документации (см. runbook ниже и команды CLI в документации). `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Хранение/Управление | ✅ Завершено |
| Телеметрия | Определить панели мониторинга/оповещения `provider_admission` (пропущенное обновление, конверт срока действия). | Наблюдаемость | 🟠 В процессе | Счетчик `torii_sorafs_admission_total{result,reason}` существует; информационные панели/оповещения в работе.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook и обновления

#### Плановое обновление (обновления кола/топологии)
1. Возьмите пару следующих предложений/объявлений через `provider-admission proposal` и `provider-admission sign`,
   Увеличьте `--retention-epoch` и обновите ставку/эндпоинты по мере необходимости.
2. Выполните
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда в последнее время сохраняет возможности/профиль месторождений через
   `AdmissionRecord::apply_renewal`, выпускает `ProviderAdmissionRenewalV1` и печатает дайджесты для
   журнал управления.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Замените предыдущий конверт на `torii.sorafs.admission_envelopes_dir`, закоммитьте Norito/JSON обновления.
   в репозитории управления и разделов обновления хэша + эпоха хранения в `docs/source/sorafs/migration_ledger.md`.
4. Сообщите операторам, что новый конверт активирован, и отслеживайте.
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` для получения подтверждения.
5. Пересоздайте и зафиксируйте канонические крепления через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) на прошлой неделе Norito выходов.

#### Аварийный отзыв
1. Определите скомпрометированный конверт и выпустите отзыв:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI подписывает `ProviderAdmissionRevocationV1`, сначала набор подписей через
   `verify_revocation_signatures` и сообщает дайджест отзыва.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Удалите конверт с `torii.sorafs.admission_envelopes_dir`, распространите Norito/JSON отзыв в допуске-кеши.
   и зафиксируйте хеш-код в управлении протоколом.
3. Отслеживайте `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, чтобы обеспечить надежность.
   что кеши отбросили отозванный рекламный ролик; Сохраните артефакты-отзывы в ретроспективах инцидентов.

## Тестирование и телеметрия- Добавьте золотые светильники для подарков и конверт, содержащий
  `fixtures/sorafs_manifest/provider_admission/`.
- Расширить CI (`ci/check_sorafs_fixtures.sh`) для пересоздания предложений и проверки конверта.
- Сгенерированные светильники включают `metadata.json` со стандартными дайджестами; нисходящие тесты
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Предоставить интеграционные тесты:
  - Torii отклоняет объявления с отсутствующими или просроченными конвертами приема.
  - CLI проходит двустороннее предложение → конверт → проверка.
  - Обновление управления ротацией аттестации конечной точки без изменения идентификатора провайдера.
- Требования по телеметрии:
  - Выдает счетчики `provider_admission_envelope_{accepted,rejected}` в Torii. ✅ `torii_sorafs_admission_total{result,reason}` теперь показывает принято/отклонено.
  - Добавить замечание о сроке наблюдения (обновление требуется на 7 дней).

## Следующие шаги

1. ✅ Завершены изменения схемы Norito и добавлены помощники валидации в `sorafs_manifest::provider_admission`. Флаги функций не нужны.
2. ✅ CLI-потоки (`proposal`, `sign`, `verify`, `renewal`, `revoke`) задокументированы и проверены интеграционными тестами; подождите скрипты управления в синхронизации с runbook.
3. ✅ Torii прием/обнаружение принимает конверты и публикует телеметрические счетчики приема/отклонения.
4. Фокус на наблюдениях: формы информационных панелей/оповещения о допуске, обновления, требуемые в течение семи дней, поднимали уведомление (`torii_sorafs_admission_total`, индикаторы истечения срока действия).