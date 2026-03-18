---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Адаптация [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Политика допуска и удостоверения личности SoraFS (Раскуньо SF-2b)

Это уведомление о захвате действий для **SF-2b**: определите
применение или поток допуска, реквизиты для идентификации и полезные нагрузки
проверка на наличие оружия SoraFS. Эла усиливается или альтовый процесс
Уровень описания в RFC de Arquitetura de SoraFS и разделение оставшейся работы
tarefas de engenharia rastreaveis.

## Политические цели

- Гарантия, что работающие операторы будут проверены при наличии общедоступных регистров `ProviderAdvertV1`, которые будут выполнены.
- Сделайте объявление об удостоверении личности документа, подтверждающего управление, конечные точки подтверждены и внесите минимальный вклад.
- Выполните детерминированные проверки для Torii, шлюзы и `sorafs-node` будут применены в качестве проверочных сообщений.
- Поддержка обновлений и отказов в чрезвычайных ситуациях, связанных с детерминизмом или эргономией ферраментов.

## Реквизиты для идентификации и ставки

| Реквизито | Описание | Энтрегавел |
|-----------|-----------|------------|
| Проведение объявления | Мы разработали регистратора по запросу Ed25519, который представляет собой каждую рекламу. О связке допущенных вооруженных сил и публичных действий, связанных с убийством правительства. | Вы можете найти имя `ProviderAdmissionProposalV1` с `advert_key` (32 байта) и ссылку без регистрации (`sorafs_manifest::provider_admission`). |
| Понтейру-де-Стейк | Требуется допуск к `StakePointer` без нулевого ответа для пула активных ставок. | Дополнительная проверка `sorafs_manifest::provider_advert::StakePointer::validate()` и экспорт ошибок в CLI/тестах. |
| Юридические теги | Osprovores declaram jurisdicao + contato Legal. | Установите или предложите образец в виде `jurisdiction_code` (ISO 3166-1 альфа-2) и `contact_uri` (дополнительно). |
| Проверка конечной точки | О конечной точке сообщается, что она была заменена сертификатором mTLS или QUIC. | Определите полезную нагрузку Norito `EndpointAttestationV1` и заблокируйте конечную точку до входного пакета. |

## Fluxo de admissao

1. **Криасао да пропоста**
   - CLI: дополнительный `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     производство `ProviderAdmissionProposalV1` + комплект проверки.
   - Проверка: требуется гарантия, ставка > 0, обработка канонического фрагмента в `profile_id`.
2. **Конец правительства**
   - Совет `blake3("sorafs-provider-admission-v1" || canonical_bytes)` с использованием существующих инструментов для конвертов
     (по модулю `sorafs_manifest::governance`).
   - О конверте и упорстве в `governance/providers/<provider_id>/admission.json`.
3. **Прием без регистрации**
   - Внедрить проверку совместимости (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI повторно использовать.
   - Настройте входной билет Torii, чтобы просмотреть рекламный дайджест или срок действия другого конверта.
4. **Обновление и возврат**
   - Дополнительный `ProviderAdmissionRenewalV1` с автоматическими опциями конечной точки/количества.
   - Экспортируйте CLI `--revoke`, который зарегистрирует или мотивирует отзыв и отправку на мероприятие по управлению.

## Условия реализации| Площадь | Тарефа | Владелец(и) | Статус |
|------|--------|----------|--------|
| Эскема | Определите `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) и `crates/sorafs_manifest/src/provider_admission.rs`. Реализовано в `sorafs_manifest::provider_admission` с помощью помощников по проверке подлинности.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | Хранение/Управление | Заключение |
| Интерфейс командной строки | Estender `sorafs_manifest_stub` с подкомандами: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | Заключение |

Поток CLI, полученный после получения пакетов промежуточных сертификатов (`--endpoint-attestation-intermediate`), выдает
канонические байты предложения/конверта и действительные ассинатуры для получения согласия на срок `sign`/`verify`. Подем операдоров
прямое использование рекламных материалов или повторное использование удаленных рекламных объявлений, а также архивов уничтоженных объявлений.
fornecidos или комбинация `--council-signature-public-key` с `--council-signature-file` для облегчения автоматической работы.

### Справочник по CLI

Выполните cada comando через `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Требуемые флаги: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>`, и это снова `--endpoint=<kind:host>`.
  - Проверка конечной точки с помощью `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, сертифицировано через
    `--endpoint-attestation-leaf=<path>` (обычно `--endpoint-attestation-intermediate=<path>`
    необязательный элемент для каждого элемента) и идентификаторы ALPN для переговоров
    (`--endpoint-attestation-alpn=<token>`). Конечные точки QUIC Podem для транспортных связей
    `--endpoint-attestation-report[-hex]=...`.
  - Саида: канонические байты предложения Norito (`--proposal-out`) и резюме в формате JSON.
    (стандартный вывод Padrao или `--json-out`).
- `sign`
  - Вводы: предложение (`--proposal`), рекламное объявление (`--advert`), дополнительная реклама.
    (`--advert-body`), эпоха удержания и исчезновение меня из рук в руки. Как ассинатурас подем
    Ser fornecidas inline (`--council-signature=<signer_hex:signature_hex>`) или через архивы или комбинаторы
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Создание действительного конверта (`--envelope-out`) и связанного JSON с индикацией дайджеста,
    Заражение убийц и жертв проникновения.
- `verify`
  - Действительный конверт существует (`--envelope`), с дополнительной проверкой предложения, рекламы или
    Корреспондент корреспондента рекламы. Относительные значения JSON для дайджеста, статус проверки
    de assinaturas e quais artefatos opcionais corderam.
- `renewal`
  - Vincula um конверт одобрен предварительно утвержденным дайджестом. Запросить
    `--previous-envelope=<path>` и наследник `--envelope=<path>` (полезные нагрузки ambos Norito).
    O CLI проверяет, что псевдонимы доступа, возможности и постоянные неизменяемые рекламные блоки,
    enquanto разрешить атуализацию ставок, конечных точек и метаданных. Публикуйте канонические байты
    `ProviderAdmissionRenewalV1` (`--renewal-out`) вместо резюме в формате JSON.
- `revoke`
  - Подготовьте комплект экстренной помощи `ProviderAdmissionRevocationV1` для создания конверта cujo.
    сер отступник. Запрос `--envelope=<path>`, `--reason=<text>`, менос ума `--council-signature`,
    e `--revoked-at`/`--notes` опционально. Активация CLI и проверка или дайджест отзыва, выделение или полезная нагрузка
    Norito через `--revocation-out` и импортируйте резюме в формате JSON в виде дайджеста и числа ассинатур.
| Проверка | Выполните проверку совместимости с использованием Torii, шлюзов и `sorafs-node`. Унитарные тесты прувера + интеграция CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | Сетевые TL/хранилище | Заключение |
| Интеграция Torii | Пройдите проверку приема рекламы с номером Torii, просматривайте рекламу на политических форумах и телеметрии. | Сеть TL | Заключение | Torii перед отправкой конвертов управления (`torii.sorafs.admission_envelopes_dir`), проверка соответствия дайджеста/уничтожение в течение длительного времени при проглатывании и телеметрии admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] || Реновакао | Дополнительный пример обновления/отмены + помощники CLI, публикация циклического просмотра наших документов (с помощью Runbook и команд CLI в `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | Хранение/Управление | Заключение |
| Телеметрия | Определения информационных панелей/предупреждений `provider_admission` (досрочное обновление, срок действия конверта). | Наблюдаемость | В прогрессе | О contador `torii_sorafs_admission_total{result,reason}` существует; информационные панели/отложенные оповещения.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de renovacao e revogacao

#### Программа обновления (актуализация кола/топологии)
1. Создание или правопреемник предложения/рекламы от `provider-admission proposal` и `provider-admission sign`,
   увеличение `--retention-epoch` и настройка доли/конечных точек в соответствии с необходимостью.
2. Выполнить
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда проверки мощности/включения изменений через `AdmissionRecord::apply_renewal`,
   излучать `ProviderAdmissionRenewalV1` и вводить дайджесты для журнала управления.
3. Замените передний конверт на `torii.sorafs.admission_envelopes_dir`, подтвердите Norito/JSON de renovacao.
   нет репозитория управления и добавления хэша обновления + эпохи хранения `docs/source/sorafs/migration_ledger.md`.
4. Уведомление операторов о том, что новый конверт активен и контролируется
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` для подтверждения приема.
5. Перегенерируйте и подтвердите канонические настройки через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) действителен, как указано в Norito, навсегда.

#### Отзыв о чрезвычайной ситуации
1. Идентификация конверта, скомпрометированного и отправленного обратно:
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
   Узел CLI или `ProviderAdmissionRevocationV1`, проверка соединения или соединения по ассинатурам через
   `verify_revocation_signatures` относится к дайджесту отзыва.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. Удаление конверта `torii.sorafs.admission_envelopes_dir`, распространение отозванного Norito/JSON для кэшей.
   допуск и регистрация или хэш-мотив для государственного управления.
3. Обратите внимание на `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` для подтверждения операционной системы.
   кэширует отгруженную или отозванную рекламу; уберите артефакты из ретроспективы инцидентов.

## Яички и телеметрия- Дополнительные золотые приспособления для предложений и конвертов для въезда в них.
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) для восстановления заявок и проверки конвертов.
- Оснастка gerados включает `metadata.json` com дайджесты canonicos; яички ниже по течению
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Доказательство интегрированных тестов:
  - Torii размещает рекламу в конвертах с истекшим или истекшим сроком годности.
  - O CLI faz туда-обратно -> конверт -> проверка.
  - Обновление управления вращением подтверждает конечную точку с изменением идентификатора проверки.
- Реквизиты телеметрии:
  - Эмитированные сообщения `provider_admission_envelope_{accepted,rejected}` и Torii. `torii_sorafs_admission_total{result,reason}` назад результаты были приняты/отклонены.
  - Дополнительные оповещения об истечении срока действия на панелях наблюдения (обновление каждые 7 дней).

## Проксимос пассос

1. В качестве изменений в задаче Norito завершается формат и используются помощники по проверке формы, включенные в `sorafs_manifest::provider_admission`. Нао-ха имеет флаги.
2. Интерфейс командной строки Fluxos (`proposal`, `sign`, `verify`, `renewal`, `revoke`) документируется и выполняется с помощью интегрированных тестов; управлять синхронизированными сценариями управления или Runbook.
3. Torii прием/обнаружение конвертов и показ телеметрических сообщений для проверки/возврата.
4. Сосредоточьтесь на наблюдении: просмотрите информационные панели/оповещения о допуске для обновлений, произошедших в течение определенного периода времени (`torii_sorafs_admission_total`, индикаторы истечения срока действия).