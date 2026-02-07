---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Адаптация [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Политика допуска и удостоверения личности SoraFS (Borador SF-2b)

Esta nota captura los entregables accionables для **SF-2b**: определите y
приложение к входному каналу, реквизиты для идентификации и полезные нагрузки
проверено для проверки подлинности SoraFS. Усиление альтового процесса
нивел описания в RFC de Arquitectura de SoraFS и разделить оставшуюся работу
в сфере инженерных разработок.

## Политические цели

- Гарантия, что только проверенные операции могут быть опубликованы в реестрах.
  `ProviderAdvertV1`, который принимает красный цвет.
- Просьба сообщить, что документ, удостоверяющий личность, одобрен для
  gobernanza, конечные точки подтверждены и вносят минимальный вклад.
- Проверьте инструменты проверки, определяющие Torii, шлюзы и
  `sorafs-node` применяет элементы управления неправильно.
- Сопровождение обновления и отмены чрезвычайной ситуации без детерминизма и
  эргономия инструментов.

## Реквизиты, удостоверяющие личность и долю

| Реквизито | Описание | Вступительный |
|-----------|-------------|------------|
| Процедура объявления | Поставщики должны быть зарегистрированы по ключам Ed25519, которые рекламируют каждую фирму. Пакет допусков остается в публичном объединении с государственной фирмой. | Расширение имени `ProviderAdmissionProposalV1` с `advert_key` (32 байта) и ссылка на реестр (`sorafs_manifest::provider_admission`). |
| Пунтеро-де-Стейк | Для входа в пул `StakePointer` не требуется участие в активном пуле ставок. | Выполните проверку `sorafs_manifest::provider_advert::StakePointer::validate()` и выявите ошибки в CLI/тестах. |
| Юридический этикет | Los provedores декларируют юрисдикцию + юридический контакт. | Удлинитель объекта с `jurisdiction_code` (ISO 3166-1 альфа-2) и `contact_uri` опционально. |
| Проверка конечной точки | О конечной точке должно быть отправлено сообщение о сертификате mTLS или QUIC. | Определите полезную нагрузку Norito `EndpointAttestationV1` и укажите конечную точку в пакете доступа. |

## Входной билет

1. **Создание собственности**
   - CLI: añadir `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     производство `ProviderAdmissionProposalV1` + комплект проверки.
   - Проверка: обеспечить требуемые поля, ставка > 0, обработать канонический блокировщик в `profile_id`.
2. **Эндозо де гобернанса**
   - Совет фирмы `blake3("sorafs-provider-admission-v1" || canonical_bytes)` с использованием инструментов
     существующий конверт (модуль `sorafs_manifest::governance`).
   - Конверт сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Введение в реестр**
   - Внедрить проверку соответствия (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI reutilicen.
   - Актуализируйте маршрут доступа по номеру Torii для повторной загрузки рекламы в дайджест или по истечении срока действия конверта.
4. **Реновация и отзыв**
   - Añadir `ProviderAdmissionRenewalV1` с дополнительными актуализациями конечной точки/количества.
   - Exponer una ruta CLI `--revoke`, который регистрирует мотив отзыва и завидует событию правительства.

## Условия реализации| Площадь | Тарея | Владелец(и) | Эстадо |
|------|-------|----------|--------|
| Эскема | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) или `crates/sorafs_manifest/src/provider_admission.rs`. Реализовано в `sorafs_manifest::provider_admission` с помощниками проверки.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хранение/Управление | ✅ Завершено |
| Инструментарий CLI | Расширитель `sorafs_manifest_stub` с подкомандами: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | ✅ |

Интерфейс CLI сейчас принимает пакеты промежуточных сертификатов (`--endpoint-attestation-intermediate`), выдает канонические байты собственности/конверта и проверяет фирму совета в течение всего срока действия `sign`/`verify`. Операторы могут напрямую использовать рекламные объявления или повторно использовать рекламные компании, а также архивы фирм могут быть объединены в комбинацию `--council-signature-public-key` с `--council-signature-file` для облегчения автоматизации.

### Справочник по CLI

Вызовите команду через `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Требуемые флаги: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>` и все остальные `--endpoint=<kind:host>`.
  - El atestado для конечной точки ожидал `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, сертификат через
    `--endpoint-attestation-leaf=<path>` (чаще `--endpoint-attestation-intermediate=<path>`
    необязательно для каждого элемента кадены) и идентификатора ALPN, согласованного
    (`--endpoint-attestation-alpn=<token>`). Конечные точки QUIC позволяют получать отчеты о транспортировке
    `--endpoint-attestation-report[-hex]=...`.
  - Сброс: канонические байты Norito (`--proposal-out`) и возобновленный JSON.
    (стандартный вывод из-за дефекта `--json-out`).
- `sign`
  - Вводы: собственность (`--proposal`), рекламная фирма (`--advert`), дополнительный рекламный ролик.
    (`--advert-body`), эпоха хранения и все мои советы. Лас-фирмас-пуден
    suministrarse inline (`--council-signature=<signer_hex:signature_hex>`) или из комбинированных архивов
    `--council-signature-public-key` с `--council-signature-file=<path>`.
  - Создание валидного конверта (`--envelope-out`) и отчета о привязках JSON с указанием дайджеста,
    Conteo de Firmantes и Rutas de Entrada.
- `verify`
  - Действующий конверт существует (`--envelope`), с дополнительной проверкой собственности,
    реклама или корреспондентская реклама. Отчет JSON потерял значение дайджеста, состояние
    проверка фирм и необязательные артефакты совпадают.
- `renewal`
  - Получите конверт, одобренный предварительно ратифицированным дайджестом. Требование
    `--previous-envelope=<path>` и преемник `--envelope=<path>` (полезные нагрузки ambos Norito).
    Проверка CLI, что псевдонимы доступа, возможности и кнопки рекламы остаются неизменными без изменений,
    Время позволяет актуализировать ставки, конечные точки и метаданные. Публикуйте канонические байты
    `ProviderAdmissionRenewalV1` (`--renewal-out`) является возобновленным JSON.
- `revoke`
  - Отправьте комплект экстренной помощи `ProviderAdmissionRevocationV1` для доставки необходимого конверта.
    вернуться назад. Требуется `--envelope=<path>`, `--reason=<text>`, все это
    `--council-signature` и дополнительно `--revoked-at`/`--notes`. Фирменный и проверенный CLI
    дайджест отзыва, запись полезной нагрузки Norito через `--revocation-out` и ввод отчета JSON
    с дайджестом и содержимым фирм.
| Проверка | Выполните проверку совместимости с использованием Torii, шлюзов и `sorafs-node`. Проверьте унитарную интеграцию + интеграцию CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сетевые TL/хранилище | ✅ Завершено || Интеграция Torii | Подключите верификатор к приему рекламы по номеру Torii, проверьте политическую рекламу и телеметрию. | Сеть TL | ✅ Завершено | Torii сейчас отправляет государственные конверты (`torii.sorafs.admission_envelopes_dir`), проверяет совпадения дайджеста/фирму во время проглатывания и показывает телеметрию прием.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Реновация | Следуйте инструкциям по обновлению/отзыву + помощники CLI, публикуйте инструкции по циклу жизни в документах (с помощью runbook и команд CLI в `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Хранение/Управление | ✅ Завершено |
| Телеметрия | Definir информационные панели/оповещения `provider_admission` (неудачное обновление, срок действия конверта). | Наблюдаемость | 🟠 В процессе | Контадор `torii_sorafs_admission_total{result,reason}` существует; информационные панели/отложенные оповещения.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook обновления и отзыва

#### Программа обновления (актуализация кол/топологии)
1. Создайте объект/объявление преемника с `provider-admission proposal` и `provider-admission sign`, увеличивайте `--retention-epoch` и актуализируйте долю/конечные точки в следующий раз.
2. Эекута
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда valida Campos de Capacidad/perfil sin cambios через
   `AdmissionRecord::apply_renewal`, излучайте `ProviderAdmissionRenewalV1` и вводите дайджесты для
   log de gobernanza.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Замените передний конверт в `torii.sorafs.admission_envelopes_dir`, подтвердите Norito/JSON обновления в репозитории правительства и объедините хэш обновления + эпоху хранения в `docs/source/sorafs/migration_ledger.md`.
4. Уведомите операторов о том, что новый конверт активен и контролируется `torii_sorafs_admission_total{result="accepted",reason="stored"}` для подтверждения приема.
5. Обновите и подтвердите канонические настройки через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) действителен, как и Norito, постоянные настройки.

#### Отзыв чрезвычайной ситуации
1. Идентификация скомпрометированного конверта и его отзыв:
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
   Фирма CLI `ProviderAdmissionRevocationV1`, проверка набора фирм через
   `verify_revocation_signatures`, отправьте отчет об отзыве.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Получите конверт `torii.sorafs.admission_envelopes_dir`, распространите Norito/JSON отзыва в кэши доступа и зарегистрируйте хэш мотивации в действиях губернатора.
3. Проверьте `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`, чтобы подтвердить, что кэши удалены из объявления; сохранение артефактов отзыва в ретроспективе инцидентов.

## Пруэбас и телеметрия- Золотые светильники Añadir для квартир и конвертов для входных билетов.
  `fixtures/sorafs_manifest/provider_admission/`.
- Расширитель CI (`ci/check_sorafs_fixtures.sh`) для регенерации свойств и проверки конвертов.
- Сгенерированные светильники включают `metadata.json` с каноническими дайджестами; Прюбас ниже по течению
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Проверочные процедуры интеграции:
  - Torii — рекламные объявления в конвертах с пропуском или сроком годности.
  - В CLI есть обратный путь → конверт → проверка.
  - Обновление губернаторского аттестата конечной точки без замены удостоверения личности провайдера.
- Реквизиты телеметрии:
  - Эмитированные сообщения `provider_admission_envelope_{accepted,rejected}` и Torii. ✅ `torii_sorafs_admission_total{result,reason}` сейчас показывает, что результаты приняты/отклонены.
  - Новые оповещения об истечении срока действия на панелях наблюдения (обновление в течение 7 дней).

## Проксимос Пасос

1. ✅ Завершение внесения изменений в программу Norito и включение помощников по проверке в
   `sorafs_manifest::provider_admission`. Флаги функций не требуются.
2. ✅ Интерфейс командной строки (`proposal`, `sign`, `verify`, `renewal`, `revoke`) документируется и отображается через процесс интеграции; сохранить синхронизированные сценарии правительства с Runbook.
3. ✅ Torii прием/обнаружение конвертов и показ контадоров телеметрии для приема/перехвата.
4. Внимание при наблюдении: завершите информационные панели/предупреждения о доступе для обновлений, которые должны быть обновлены на сайте в непредусмотренные дни (`torii_sorafs_admission_total`, индикаторы истечения срока действия).