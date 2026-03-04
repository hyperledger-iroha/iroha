---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے ماخوذ۔

# SoraFS Может быть использован в качестве резервного копирования (SF-2b в составе)

Для **SF-2b** необходимо установить соединение с SoraFS. Вы можете использовать полезные нагрузки для получения информации о полезной нагрузке. تعریف اور نفاذ۔ SoraFS Архитектура RFC может быть использована для решения задач, связанных с архитектурой RFC. Если вы хотите, чтобы вы выбрали лучший вариант для себя,

## پالیسی کے اہداف

- یہ یقینی بنانا کہ صرف تصدیق شدہ آپریٹرز ہی `ProviderAdvertV1` ریکارڈز شائع کر سکیں جنہیں نیٹ ورک قبول کرے۔
- Чтобы получить доступ к конечным точкам, выполните следующие действия. Как сделать ставку или сделать ставку?
- Torii, шлюзы, а также `sorafs-node`, где можно установить шлюзы или шлюзы. ویریفیکیشن ٹولنگ فراہم کرنا۔
- Если вам нужна эргономика, вы можете сделать это с помощью эргономики. کرنا۔

## شناخت اور تقاضے

| تقاضا | وضاحت | ڈیلیوریبل |
|-------|-------|-----------|
| علان کلید کا ماخذ | Используйте пару ключей Ed25519. входной комплект | `ProviderAdmissionProposalV1` اسکیمہ میں `advert_key` (32 байта) ریفرنس کریں۔ |
| Ставка پوائنٹر | Для пула ставок можно использовать пул ставок `StakePointer`. | `sorafs_manifest::provider_advert::StakePointer::validate()` Проверка ошибок в интерфейсе командной строки/тестах и ​​проверка ошибок. |
| Юрисдикция ٹیگز | فراہم کنندگان юрисдикция + قانونی رابطہ ظاہر کرتے ہیں۔ | Дополнительная информация `jurisdiction_code` (ISO 3166-1 альфа-2) Дополнительная информация `contact_uri`. |
| Конечная точка Конечная точка может быть подключена к mTLS или QUIC, а также к удаленному доступу к сети. | Полезная нагрузка Norito `EndpointAttestationV1` Можно использовать для пакета входных данных, который позволяет получить конечную точку. کریں۔ |

## قبولیت کا ورک فلو

1. **Вечеринка с фруктами**
   - Интерфейс командной строки: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     شامل کریں جو `ProviderAdmissionProposalV1` + пакетный пакет بنائے۔
   - ویلیڈیشن: ضروری کو یقینی بنائیں۔
2. **Какой-то подарок**
   - Дополнительные инструменты для конвертов (например, `sorafs_manifest::governance`).
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` в качестве примера
   - Конверт `governance/providers/<provider_id>/admission.json` в упаковке.
3. ** رجسٹری میں اندراج**
   - Используйте верификатор (`sorafs_manifest::provider_admission::validate_envelope`) для проверки Torii/gateways/CLI.
   - Torii کے входной билет کو اپڈیٹ کریں تاکہ وہ ایسے реклама کو مسترد کرے جن کا дайджест یا конверт с истекающим сроком действия سے مختلف ہو۔
4. **День рождения**
   - Конечная точка/доля может быть установлена `ProviderAdmissionRenewalV1`.
   - Интерфейс командной строки `--revoke` может быть использован в качестве дополнительного программного обеспечения. گورننس ایونٹ بھیجے۔

## عمل درآمد کے کام| علاقہ | کام | Владелец(и) | حالت |
|-------|-----|----------|------|
| اسکیمہ | `crates/sorafs_manifest/src/provider_admission.rs` или `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito). `sorafs_manifest::provider_admission` میں ویلیڈیشن helpers کے ساتھ نافذ کیا گیا ہے۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Хранение/Управление | ✅ مکمل |
| Интерфейс CLI | `sorafs_manifest_stub` может быть использован для подключения: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Инструментальная рабочая группа | ✅ مکمل |

CLI для создания пакетов (`--endpoint-attestation-intermediate`)
каноническое предложение/байты конверта جاری کرتا ہے، اور `sign`/`verify` کے کے کونسل подписи کو ویری فائی کرتا ہے۔ آپریٹرز рекламные тела براہ راست فراہم کر سکتے ہیں یا подписанные рекламные объявления Доступны файлы подписей `--council-signature-public-key` или `--council-signature-file`, необходимые для автоматизации. آسان ہو۔

### Интерфейс командной строки

ہر کمانڈ کو `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے چلائیں۔- `proposal`
  - Наличие: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    И18НИ00000063Х, И18НИ00000064Х, И18НИ00000065Х,
    `--jurisdiction-code=<ISO3166-1>`, а также `--endpoint=<kind:host>`۔
  - Конечная точка имеет значение `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, ایک سرٹیفکیٹ بذریعہ
    `--endpoint-attestation-leaf=<path>` (если есть возможность использовать `--endpoint-attestation-intermediate=<path>`)
    Также можно использовать идентификаторы ALPN (`--endpoint-attestation-alpn=<token>`). Конечные точки QUIC в режиме реального времени
    `--endpoint-attestation-report[-hex]=...` کے ذریعے فراہم کر سکتے ہیں۔
  - Запись: канонические байты предложения Norito (`--proposal-out`) и JSON-файл.
    (стандартный номер `--json-out`).
- `sign`
  - В тексте: приветствие (`--proposal`), подписанное объявление (`--advert`), а также текст объявления.
    (`--advert-body`), эпоха хранения, подпись и подпись. подписи کو встроенные
    (`--council-signature=<signer_hex:signature_hex>`)
    `--council-signature-public-key` کو `--council-signature-file=<path>` کے ساتھ ملایا جائے۔
  - Подтвержденный конверт (`--envelope-out`) Для JSON-файлов и привязок дайджеста, подсчета подписантов и путей ввода.
- `verify`
  - Конверт (`--envelope`) для поиска соответствующего предложения, объявления и тела объявления. کرتا ہے۔ Значения дайджеста JSON, статус проверки подписи, а также дополнительные артефакты и соответствие ہوئے۔
- `renewal`
  - Воспользуйтесь конвертом, чтобы получить дайджест, который можно использовать اس کے لیے
    `--previous-envelope=<path>` или `--envelope=<path>` (полезные нагрузки Norito)
    CLI позволяет использовать псевдонимы профилей, возможности и ключи для рекламы, а также возможность использовать ставку. конечные точки, метаданные и другие метаданные канонический `ProviderAdmissionRenewalV1` байт (`--renewal-out`) в формате JSON или ہوتا ہے۔
- `revoke`
  - Лучший поставщик услуг `ProviderAdmissionRevocationV1`, пакет, который можно использовать в конверте или конверте.
    `--envelope=<path>`, `--reason=<text>`, `--council-signature`, `--council-signature` درکار ہے، اور
    `--revoked-at`/`--notes` اختیاری ہیں۔ Дайджест отзыва CLI для подписи/проверки полезной нагрузки Norito
    `--revocation-out` کے ذریعے لکھنٹ کرتا کے ساتھ JSON رپورٹ پرنٹ کرتا ہے۔
| ویریفیکیشن | Torii, шлюзы, `sorafs-node` کے لیے مشترکہ verifier کریں۔ модуль + интеграционные тесты CLI فراہم کریں۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Сетевые TL/хранилище | ✅ مکمل |
| Torii Информация | верификатор Torii для приема рекламы Использование телеметрии کریں۔ | Сеть TL | ✅ مکمل | Torii в конвертах управления (`torii.sorafs.admission_envelopes_dir`). телеметрия ظاہر کرتا ہے。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 || تجدید | Поддержка/помощь + CLI-помощники Руководство по жизненному циклу и документация по использованию Runbook (runbook) `provider-admission renewal`/`revoke` Интерфейс командной строки دیکھیں)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Хранение/Управление | ✅ مکمل |
| ٹیلیمیٹری | `provider_admission` информационные панели и оповещения о тревоге (окончание срока действия конверта)۔ | Наблюдаемость | 🟠 جاری | Код `torii_sorafs_admission_total{result,reason}` موجود ہے؛ информационные панели/оповещения زیر التوا ہیں۔【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید اور منسوخی کا رن بُک

#### شیڈول شدہ تجدید (кол/топология اپڈیٹس)
1. `provider-admission proposal` اور `provider-admission sign` کے ذریعے جانشین предложение/реклама جوڑا بنائیں،
   `--retention-epoch` Выберите ставку/конечные точки для определения доли/конечных точек.
2. Свобода
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   یہ کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے возможности/профиль کرتی ہے،
   `ProviderAdmissionRenewalV1` جاری کرتی ہے، اور گورننس لاگ کے لیے дайджесты پرنٹ کرتی ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` конверт для обновления Norito/JSON для создания коммита کریں،
   Хэш обновления + эпоха хранения: `docs/source/sorafs/migration_ledger.md`.
4. Возьмите конверт с конвертом и проглотите его, если хотите.
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` مانیٹر کریں۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے канонические светильники
   CI (`ci/check_sorafs_fixtures.sh`) Norito Обеспечивает стабильность и стабильность работы.

#### ہنگامی منسوخی
1. Конверт, который можно использовать в следующих случаях:
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
   CLI `ProviderAdmissionRevocationV1` используется для создания подписей `verify_revocation_signatures`. ہے،
   Дайджест отзыва можно найти в 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` конверт для отзыва Norito/JSON для кэширования допусков
   Как использовать хеш-код, чтобы получить доступ к хеш-функции
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` можно использовать для кэширования отозванной рекламы или удаления
   артефакты отзыва и ретроспектива инцидентов

## ٹیسٹنگ اور ٹیلیمیٹری- предложения о приеме, конверты и золотые светильники, `fixtures/sorafs_manifest/provider_admission/`, которые можно использовать.
- CI (`ci/check_sorafs_fixtures.sh`) Для создания предложений и конвертов для конвертов.
- تیار شدہ светильники и канонические дайджесты کے ساتھ `metadata.json` شامل ہوتا ہے؛ последующие тесты یہ утверждать کرتے ہیں کہ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Интеграционные тесты. Дополнительные сведения:
  - Torii ایسے реклама مسترد کرتا ہے جن کے конверты для поступления
  - Предложение CLI → конверт → проверка или двусторонний обмен данными.
  - Можно указать идентификатор провайдера, указать конечную точку, а также вращать ее.
- Вот что можно сказать:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز испускать کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` принимается/отклоняется.
  - Панели мониторинга наблюдения и предупреждения об истечении срока действия.

## اگلے اقدامات

1. ✅ Norito С помощью помощников проверки можно использовать `sorafs_manifest::provider_admission`. گئے ہیں۔ Флаги функций
2. ✅ Интерфейс командной строки (`proposal`, `sign`, `verify`, `renewal`, `revoke`) для интеграционных тестов. سے گزارے گئے ہیں؛ Как использовать Runbook для чтения или записи
3. ✅ Torii принимает конверты приема/обнаружения и принимает счетчики телеметрии.
4. Наблюдаемость: Панели мониторинга/оповещения о входе وارننگ ہو (`torii_sorafs_admission_total`, индикаторы срока годности)۔