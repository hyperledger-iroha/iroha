---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-22T08:21:32.146994+00:00"
translation_last_reviewed: 2026-01-30
---

> Адаптировано из [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Политика допуска и идентификации провайдеров SoraFS (черновик SF-2b)

Эта записка фиксирует практические результаты для **SF-2b**: определить и
принудительно применять процесс допуска, требования к идентичности и
аттестационные payload'ы для провайдеров хранения SoraFS. Она расширяет
процесс высокого уровня, описанный в RFC архитектуры SoraFS, и разбивает
оставшуюся работу на отслеживаемые инженерные задачи.

## Цели политики

- Гарантировать, что только проверенные операторы могут публиковать записи `ProviderAdvertV1`, которые сеть принимает.
- Привязать каждый ключ объявления к документу идентичности, утвержденному governance, аттестованным эндпоинтам и минимальному вкладу stake.
- Предоставить детерминированные инструменты проверки, чтобы Torii, шлюзы и `sorafs-node` применяли одинаковые проверки.
- Поддерживать обновление и аварийное аннулирование без нарушения детерминизма или удобства инструментов.

## Требования к идентичности и stake

| Требование | Описание | Результат |
|------------|----------|-----------|
| Происхождение ключа объявления | Провайдеры должны зарегистрировать пару ключей Ed25519, которой подписывается каждое advert. Бандл допуска хранит публичный ключ вместе с подписью governance. | Расширить схему `ProviderAdmissionProposalV1` полем `advert_key` (32 bytes) и сослаться на нее из реестра (`sorafs_manifest::provider_admission`). |
| Указатель stake | Для допуска требуется ненулевой `StakePointer`, указывающий на активный staking pool. | Добавить валидацию в `sorafs_manifest::provider_advert::StakePointer::validate()` и выводить ошибки в CLI/тестах. |
| Теги юрисдикции | Провайдеры объявляют юрисдикцию + юридический контакт. | Расширить схему предложения полем `jurisdiction_code` (ISO 3166-1 alpha-2) и опциональным `contact_uri`. |
| Аттестация эндпоинта | Каждый объявленный эндпоинт должен быть подкреплен отчетом сертификата mTLS или QUIC. | Определить Norito payload `EndpointAttestationV1` и хранить его по каждому эндпоинту в бандле допуска. |

## Процесс допуска

1. **Создание предложения**
   - CLI: добавить `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`,
     формирующий `ProviderAdmissionProposalV1` + бандл аттестации.
   - Валидация: убедиться в наличии обязательных полей, stake > 0, канонического chunker handle в `profile_id`.
2. **Одобрение governance**
   - Совет подписывает `blake3("sorafs-provider-admission-v1" || canonical_bytes)` используя существующие
     инструменты envelope (модуль `sorafs_manifest::governance`).
   - Envelope сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Внесение в реестр**
   - Реализовать общий валидатор (`sorafs_manifest::provider_admission::validate_envelope`), который
     переиспользуют Torii/шлюзы/CLI.
   - Обновить путь допуска Torii, чтобы отклонять adverts, у которых digest или срок действия отличается от envelope.
4. **Обновление и отзыв**
   - Добавить `ProviderAdmissionRenewalV1` с опциональными обновлениями эндпоинтов/stake.
   - Открыть путь CLI `--revoke`, который фиксирует причину отзыва и отправляет событие governance.

## Задачи реализации

| Область | Задача | Owner(s) | Статус |
|---------|--------|----------|--------|
| Схема | Определить `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) в `crates/sorafs_manifest/src/provider_admission.rs`. Реализовано в `sorafs_manifest::provider_admission` с помощниками валидации.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ Завершено |
| Инструменты CLI | Расширить `sorafs_manifest_stub` подкомандами: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ Завершено |

CLI поток теперь принимает промежуточные бандлы сертификатов (`--endpoint-attestation-intermediate`),
выдает канонические байты предложения/envelope и проверяет подписи совета во время `sign`/`verify`. Операторы могут
передавать тела advert напрямую или переиспользовать подписанные adverts, а файлы подписей можно
передавать, сочетая `--council-signature-public-key` с `--council-signature-file` для удобства автоматизации.

### Справочник CLI

Запускайте каждую команду через `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - Обязательные флаги: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, и как минимум один `--endpoint=<kind:host>`.
  - Для аттестации каждого эндпоинта требуются `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, сертификат через
    `--endpoint-attestation-leaf=<path>` (плюс опциональный `--endpoint-attestation-intermediate=<path>`
    для каждого элемента цепочки) и любые согласованные ALPN ID
    (`--endpoint-attestation-alpn=<token>`). Для QUIC-эндпоинтов можно передать транспортные отчеты через
    `--endpoint-attestation-report[-hex]=...`.
  - Выход: канонические байты предложения Norito (`--proposal-out`) и JSON-сводка
    (stdout по умолчанию или `--json-out`).
- `sign`
  - Входные данные: предложение (`--proposal`), подписанный advert (`--advert`), опциональное тело advert
    (`--advert-body`), retention epoch и как минимум одна подпись совета. Подписи можно передавать
    inline (`--council-signature=<signer_hex:signature_hex>`) или через файлы, сочетая
    `--council-signature-public-key` с `--council-signature-file=<path>`.
  - Формирует валидированный envelope (`--envelope-out`) и JSON-отчет с привязками digest,
    числом подписантов и входными путями.
- `verify`
  - Проверяет существующий envelope (`--envelope`) и опционально сверяет соответствующее предложение,
    advert или тело advert. JSON-отчет подсвечивает значения digest, статус проверки подписей
    и какие опциональные артефакты совпали.
- `renewal`
  - Связывает новый утвержденный envelope с ранее утвержденным digest. Требуются
    `--previous-envelope=<path>` и последующий `--envelope=<path>` (оба Norito payload).
    CLI проверяет, что profile aliases, capabilities и advert key остаются неизменными, при этом допускает
    обновления stake, эндпоинтов и metadata. Выводит канонические байты `ProviderAdmissionRenewalV1`
    (`--renewal-out`) плюс JSON-сводку.
- `revoke`
  - Выпускает аварийный бандл `ProviderAdmissionRevocationV1` для провайдера, чей envelope нужно отозвать.
    Требует `--envelope=<path>`, `--reason=<text>`, как минимум одну `--council-signature` и опциональные
    `--revoked-at`/`--notes`. CLI подписывает и проверяет digest отзыва, записывает Norito payload через
    `--revocation-out` и печатает JSON-отчет с digest и числом подписей.
| Проверка | Реализовать общий валидатор, используемый Torii, шлюзами и `sorafs-node`. Предоставить unit + CLI интеграционные тесты.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ Завершено |
| Интеграция Torii | Подключить валидатор к приему adverts в Torii, отклонять adverts вне политики и публиковать телеметрию. | Networking TL | ✅ Завершено | Torii теперь загружает governance envelopes (`torii.sorafs.admission_envelopes_dir`), проверяет совпадение digest/подписи при приеме и публикует телеметрию допуска.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Обновление | Добавить схемы обновления/отзыва + CLI помощники, опубликовать жизненный цикл в документации (см. runbook ниже и команды CLI в `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ Завершено |
| Телеметрия | Определить dashboards/alerts `provider_admission` (пропущенное обновление, срок действия envelope). | Observability | 🟠 В процессе | Счетчик `torii_sorafs_admission_total{result,reason}` существует; dashboards/alerts в работе.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook обновления и отзыва

#### Плановое обновление (обновления stake/топологии)
1. Соберите пару последующего предложения/advert через `provider-admission proposal` и `provider-admission sign`,
   увеличив `--retention-epoch` и обновив stake/эндпоинты по необходимости.
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
   Команда проверяет неизменность полей capability/profile через
   `AdmissionRecord::apply_renewal`, выпускает `ProviderAdmissionRenewalV1` и печатает digests для
   журнала governance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Замените предыдущий envelope в `torii.sorafs.admission_envelopes_dir`, закоммитьте Norito/JSON обновления
   в репозиторий governance и добавьте hash обновления + retention epoch в `docs/source/sorafs/migration_ledger.md`.
4. Сообщите операторам, что новый envelope активен, и отслеживайте
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` для подтверждения приема.
5. Пересоздайте и зафиксируйте канонические fixtures через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) проверяет стабильность Norito выходов.

#### Аварийный отзыв
1. Определите компрометированный envelope и выпустите отзыв:
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
   CLI подписывает `ProviderAdmissionRevocationV1`, проверяет набор подписей через
   `verify_revocation_signatures` и сообщает digest отзыва.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Удалите envelope из `torii.sorafs.admission_envelopes_dir`, распространите Norito/JSON отзыва в admission-кеши
   и зафиксируйте hash причины в протоколе governance.
3. Отслеживайте `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, чтобы подтвердить,
   что кеши отбросили отозванный advert; храните артефакты отзыва в ретроспективах инцидента.

## Тестирование и телеметрия

- Добавить golden fixtures для предложений и envelope допуска в
  `fixtures/sorafs_manifest/provider_admission/`.
- Расширить CI (`ci/check_sorafs_fixtures.sh`) для пересоздания предложений и проверки envelope.
- Сгенерированные fixtures включают `metadata.json` с каноническими digests; downstream-тесты утверждают
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Предоставить интеграционные тесты:
  - Torii отклоняет adverts с отсутствующими или просроченными admission envelopes.
  - CLI проходит round-trip предложения → envelope → verification.
  - Обновление governance вращает endpoint аттестацию без изменения provider ID.
- Требования по телеметрии:
  - Emit `provider_admission_envelope_{accepted,rejected}` counters в Torii. ✅ `torii_sorafs_admission_total{result,reason}` теперь показывает accepted/rejected.
  - Добавить предупреждения об истечении срока в наблюдаемость (обновление требуется в течение 7 дней).

## Следующие шаги

1. ✅ Завершены изменения схем Norito и добавлены помощники валидации в `sorafs_manifest::provider_admission`. Feature flags не нужны.
2. ✅ CLI потоки (`proposal`, `sign`, `verify`, `renewal`, `revoke`) задокументированы и проверены интеграционными тестами; поддерживайте скрипты governance в синхронизации с runbook.
3. ✅ Torii admission/discovery принимает envelopes и публикует телеметрические счетчики принятия/отклонения.
4. Фокус на наблюдаемости: завершить dashboards/alerts по admission, чтобы обновления, требуемые в течение семи дней, поднимали предупреждения (`torii_sorafs_admission_total`, expiry gauges).
