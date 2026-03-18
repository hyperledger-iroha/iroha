---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Адаптируйте [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Политика приема и идентификации поставщиков SoraFS (Brouillon SF-2b)

В этой заметке собраны действующие книги для **SF-2b**: определение и др.
применение рабочего процесса допуска, требований идентификации и полезных данных
аттестация поставщиков складских запасов SoraFS. Elle étend leprocessus
высокий уровень декрета в RFC d'architecture SoraFS и развязка труда
Остался в напряжении и отслеживании инженерных навыков.

## Политические цели

- Гарантия, что только операторы проверят возможность публикации регистраций.
  `ProviderAdvertV1` принимается по формуле.
- Подайте объявление об утверждении документа, удостоверяющего личность, в рамках управления,
  конечные точки подтверждены и минимальный вклад.
- Чтобы выполнить проверку, определите, что Torii, шлюзы
  et `sorafs-node` примените элементы управления мемами.
- Сторонник обновления и срочного отзыва без кассера
  детерминизм и эргономика инструментов.

## Требования к идентичности и интересам

| Необходимость | Описание | Ливрабле |
|----------|-------------|----------|
| Провенанс де ла кле д'аннонс | Поставщики должны зарегистрировать пару ключей Ed25519, которые подписывают каждую рекламу. Пакет входных билетов открыт для общественности с подписью правительства. | Соберите схему `ProviderAdmissionProposalV1` с `advert_key` (32 байта) и укажите ссылку на регистрацию (`sorafs_manifest::provider_admission`). |
| Пуантёр де кол | Требование о входе в `StakePointer` не имеет нулевого значения по сравнению с активным пулом ставок. | Дополните проверку в `sorafs_manifest::provider_advert::StakePointer::validate()` и исправьте ошибки в CLI/тестах. |
| Теги юрисдикции | Поставщики заявили о юрисдикции + юридический контакт. | Создайте схему предложения с опциями `jurisdiction_code` (ISO 3166-1 альфа-2) и `contact_uri`. |
| Аттестация конечной точки | Объявлено, что конечная точка связана с соглашением о сертификате mTLS или QUIC. | Определите полезную нагрузку Norito `EndpointAttestationV1` и хранилище по конечной точке в пакете доступа. |

## Порядок поступления1. **Создание предложения**
   - CLI: дополнительный `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     производитель `ProviderAdmissionProposalV1` + пакет аттестации.
   - Проверка: s'assurer des champs requis, ставка > 0, дескриптор canonique chunker в `profile_id`.
2. **Одобрение управления**
   - Совет по подписи `blake3("sorafs-provider-admission-v1" || canonical_bytes)` через l'outillage
     существующий конверт (модуль `sorafs_manifest::governance`).
   - Конверт сохраняется в `governance/providers/<provider_id>/admission.json`.
3. **Внесение в реестр**
   - Разработчик частичной проверки (`sorafs_manifest::provider_admission::validate_envelope`)
     повторно использовать по Torii/gateways/CLI.
   - Mettre à jour le chemin d'admission Torii для отмены рекламы, не перевариваемой или не истеченной
     разные конверты.
4. **Обновление и отзыв**
   - Добавлен `ProviderAdmissionRenewalV1` с дополнительными опциями для конечной точки/количества.
   - Разоблачите CLI `--revoke`, который зарегистрирует причину отзыва и даст возможность управлять.

## Особенности реализации

| Домен | Таче | Владелец(и) | Статут |
|--------|------|----------|--------|
| Схема | Определения `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) су `crates/sorafs_manifest/src/provider_admission.rs`. Реализован в `sorafs_manifest::provider_admission` с помощниками проверки.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хранение/Управление | ✅ Термине |
| Интерфейс командной строки Outillage | Étendre `sorafs_manifest_stub` с подчиненными командами: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | ✅ |

Интерфейс командной строки Flux принимает пакеты промежуточных сертификатов (`--endpoint-attestation-intermediate`), принимает канонические байты, предложения/конверты и действительные подписи совета `sign`/`verify`. Операторы могут использовать рекламные щиты или повторно использовать рекламные знаки, а также фишеры подписи могут быть объединены в `--council-signature-public-key` с `--council-signature-file` для облегчения автоматизации.

### Справка по интерфейсу командной строки

Выполните команду команды через `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Требуемые флаги: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>` и еще больше `--endpoint=<kind:host>`.
  - Аттестация конечной точки по адресу `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, сертифицирован через
    `--endpoint-attestation-leaf=<path>` (плюс `--endpoint-attestation-intermediate=<path>`
    опция для элемента цепочки) и весь ID ALPN Négocié
    (`--endpoint-attestation-alpn=<token>`). Конечные точки QUIC могут обеспечивать связь между транспортировкой через
    `--endpoint-attestation-report[-hex]=...`.
  - Вылазка: канонические байты предложения Norito (`--proposal-out`) и резюме в формате JSON.
    (стандартно по умолчанию или `--json-out`).
- `sign`
  - Входные билеты: предложение (`--proposal`), рекламный знак (`--advert`), вариант размещения рекламы.
    (`--advert-body`), эпоха хранения и еще несколько минут до подписания совета. Дополнительные подписи
    встроенные Fournies (`--council-signature=<signer_hex:signature_hex>`) или через фичи и комбинации
    `--council-signature-public-key` с `--council-signature-file=<path>`.
  - Создайте действительный конверт (`--envelope-out`) и взаимопонимание JSON, безразличное к связям с дайджестом,
    le nombre de Signataires и les chemins d'entrée.
- `verify`
  - Действующий конверт существует (`--envelope`), с возможностью проверки предложения,
    корреспондент de l'advert или du corps d'advert. Взаимосвязь JSON встретилась перед ценностями дайджеста,
    l'état de verification de подпись и другие артефакты корреспондентов.
- `renewal`
  - Получите одобрение нового конверта или предварительное одобрение. Запрос
    `--previous-envelope=<path>` и успешный `--envelope=<path>` (две полезные нагрузки Norito).
    CLI проверяет, что псевдонимы профиля, емкости и оставшиеся кнопки объявления изменяются,
    рекламируйте авторизованные ошибки в течение дня, конечные точки и метаданные. Ознакомьтесь с каноническими байтами
    `ProviderAdmissionRenewalV1` (`--renewal-out`) — это резюме в формате JSON.
- `revoke`
  - Срочный пакет `ProviderAdmissionRevocationV1` для поставщика, который не делает конверта
    На пенсию. Запрос `--envelope=<path>`, `--reason=<text>`, еще раз
    `--council-signature` и дополнительные опции `--revoked-at`/`--notes`. Подписан и действителен CLI
    дайджест отзыва, расшифровка полезной нагрузки Norito через `--revocation-out` и установка связи в формате JSON.
    с дайджестом и номерами подписей.
| Проверка | Реализована часть верификатора, использующая Torii, шлюзы и `sorafs-node`. Функционирование единых тестов + интеграция CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сетевые TL/хранилище | ✅ Термине || Интеграция Torii | Введите верификатор при приеме рекламы Torii, отклоните рекламу за пределами политики и проверьте телеметрию. | Сеть TL | ✅ Термине | Torii взимать плату за конверты управления (`torii.sorafs.admission_envelopes_dir`), проверять дайджест корреспонденции/подпись проглатываемого продукта и раскрывать телеметрию d'admission.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Реновация | Добавление схемы обновления/отзыва + помощники CLI, публикация руководства по циклу жизни в документах (voir runbook ci-dessous et Commandes CLI) `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Хранение/Управление | ✅ Термине |
| Телеметрия | Définir информационные панели/оповещения `provider_admission` (обновление истекло, срок действия конверта истек). | Наблюдаемость | 🟠 Курс | Компьютер `torii_sorafs_admission_total{result,reason}` существует; информационные панели/оповещения и оповещения.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Книга обновлений и аннулирования

#### Программа обновления (Mises à Jour de Stake/topologie)
1. Создайте пару успешных предложений/объявлений с `provider-admission proposal` и `provider-admission sign`, а также дополните их `--retention-epoch` и добавьте к ним ставку/конечные точки, если они есть.
2. Экзекутес
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Команда действительных полей емкости/профиля меняется через
   `AdmissionRecord::apply_renewal`, émet `ProviderAdmissionRenewalV1` и залейте дайджесты
   журнал управления.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Замените предыдущий конверт в `torii.sorafs.admission_envelopes_dir`, зафиксируйте Norito/JSON обновления в хранилище управления и добавьте хеш обновления + эпоху хранения в `docs/source/sorafs/migration_ledger.md`.
4. Уведомите операторов о том, что новый конверт активен и находится под наблюдением `torii_sorafs_admission_total{result="accepted",reason="stored"}` для подтверждения приема.
5. Обновите и зафиксируйте канонические светильники через `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) действителен, если вылеты Norito остаются в конюшнях.

#### Срочный отзыв
1. Идентификация скомпрометированного конверта и его отзыва:
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
   Подпись CLI `ProviderAdmissionRevocationV1`, проверка ансамбля подписей через
   `verify_revocation_signatures`, и сообщите дайджест отзыва.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Поднимите конверт `torii.sorafs.admission_envelopes_dir`, раздайте Norito/JSON отзыва из кэша доступа и зарегистрируйте хеш-код в течение нескольких минут управления.
3. Surveillez `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` для подтверждения того, что кэши были оставлены без объявления; сохранить артефакты отзыва в случае ретроспективы инцидента.

## Тесты и телеметрия- Золотая фурнитура для предложений и конвертов для приема гостей.
  `fixtures/sorafs_manifest/provider_admission/`.
- Используйте CI (`ci/check_sorafs_fixtures.sh`) для обработки предложений и проверки конвертов.
- Общие светильники включают `metadata.json` с каноническими дайджестами; Лес тесты ниже по течению валидны
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Функционал тестов интеграции:
  - Torii отклоняйте рекламные объявления с конвертами для приема, которые истекли или истекли.
  - Le CLI fait un aller-retour Offer → конверт → проверка.
  - Обновление управления стало поворотным пунктом аттестации конечной точки без смены идентификатора провайдера.
- Требования к телеметрии:
  - Проверьте компьютеры `provider_admission_envelope_{accepted,rejected}` в Torii. ✅ `torii_sorafs_admission_total{result,reason}` отображает принятые/отклоненные результаты.
  - Добавление предупреждений об истечении срока действия на панелях наблюдения (обновление в течение 7 дней).

## Prochaines étapes

1. ✅ Завершение изменений схемы Norito и интеграция помощников проверки в нее.
   `sorafs_manifest::provider_admission`. Требование к флагу функции Aucun.
2. ✅ CLI рабочих процессов (`proposal`, `sign`, `verify`, `renewal`, `revoke`), документация и упражнения с помощью тестов интеграции; синхронизируйте сценарии управления с Runbook.
3. ✅ L’admission/discovery Torii включит конверты и обнажит контролёров телеметрии для приёма/отклонения.
4. Фокус наблюдения: окончание информационных панелей/предупреждений о доступе для обновлений в сентябре, когда предупреждения исчезли (`torii_sorafs_admission_total`, индикаторы срока действия).