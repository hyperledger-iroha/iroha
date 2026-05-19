<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Универсальное руководство по учетной записи

В этом руководстве собраны требования к развертыванию UAID (универсального идентификатора учетной записи) из
дорожную карту Nexus и упаковывает их в пошаговое руководство, ориентированное на оператора и SDK.
Он охватывает получение UAID, проверку портфеля/манифеста, шаблоны регуляторов,
и доказательства, которые должны сопровождать каждый манифест каталога приложений iroha
Publish` run (roadmap reference: `roadmap.md:2209`).

## 1. Краткий справочник UAID- UAID — это литералы `uaid:<hex>`, где `<hex>` — дайджест Blake2b-256,
  LSB установлен на `1`. Канонический тип живет в
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Записи учетных записей (`Account` и `AccountDetails`) теперь содержат дополнительный `uaid`.
  поле, чтобы приложения могли узнать идентификатор без специального хеширования.
- Политики идентификаторов скрытых функций могут связывать произвольные нормализованные входные данные.
  (номера телефонов, адреса электронной почты, номера счетов, строки партнеров) на идентификаторы `opaque:`.
  в пространстве имен UAID. Части цепи: `IdentifierPolicy`,
  `IdentifierClaimRecord` и индекс `opaque_id -> uaid`.
- Space Directory поддерживает карту `World::uaid_dataspaces`, которая связывает каждый UAID.
  к учетным записям пространства данных, на которые ссылаются активные манифесты. Torii повторно использует это
  карта для API `/portfolio` и `/uaids/*`.
- `POST /v1/accounts/onboard` публикует манифест Space Directory по умолчанию для
  глобальное пространство данных, когда оно не существует, поэтому UAID немедленно привязывается.
  Органы регистрации должны иметь `CanPublishSpaceDirectoryManifest{dataspace=0}`.
- Все SDK предоставляют помощники для канонизации литералов UAID (например,
  `UaidLiteral` в Android SDK). Помощники принимают необработанные 64-шестнадцатеричные дайджесты.
  (LSB=1) или литералы `uaid:<hex>` и повторно используйте те же кодеки Norito, чтобы
  дайджест не может перемещаться между языками.

## 1.1 Политики скрытых идентификаторов

UAID теперь являются якорем для второго уровня идентификации:- Глобальный `IdentifierPolicyId` (`<kind>#<business_rule>`) определяет
  пространство имен, метаданные общедоступных обязательств, ключ проверки преобразователя и
  канонический режим нормализации ввода (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` или `AccountNumber`).
- Утверждение связывает один производный идентификатор `opaque:` ровно с одним UAID и одним
  канонический `AccountId` в соответствии с этой политикой, но цепочка принимает только
  заявка, когда она сопровождается подписанным `IdentifierResolutionReceipt`.
- Разрешение остается потоком `resolve -> transfer`. Torii устраняет непрозрачность
  дескриптор и возвращает канонический `AccountId`; трансферты по-прежнему нацелены на
  каноническую учетную запись, а не литералы `uaid:` или `opaque:` напрямую.
- Политики теперь могут публиковать параметры входного шифрования BFV через
  `PolicyCommitment.public_parameters`. Если присутствует, Torii рекламирует их на
  `GET /v1/identifier-policies`, и клиенты могут отправлять входные данные, упакованные в BFV.
  вместо открытого текста. Запрограммированные политики заключают параметры BFV в
  канонический пакет `BfvProgrammedPublicParameters`, который также публикует
  общедоступный `ram_fhe_profile`; устаревшие необработанные полезные нагрузки BFV обновляются до этого
  канонический пакет при перестроении обязательства.
- Маршруты идентификатора проходят через один и тот же токен доступа Torii и ограничение скорости.
  проверяет, как и другие конечные точки, связанные с приложением. Они не являются обходом нормального
  Политика API.

## 1.2 Терминология

Разделение имен намеренно:- `ram_lfe` — это внешняя абстракция скрытых функций. Он охватывает политику
  регистрация, обязательства, общедоступные метаданные, квитанции об исполнении и
  режим проверки.
- `BFV` — это гомоморфная схема шифрования Бракерски/Фан-Веркаутерена, используемая
  некоторые бэкэнды `ram_lfe` для оценки зашифрованного ввода.
- `ram_fhe_profile` — это метаданные, специфичные для BFV, а не второе имя для целого.
  функция. Он описывает запрограммированную исполняющую машину BFV, которая осуществляет кошельки и
  верификаторы должны ориентироваться на случаи, когда политика использует запрограммированный бэкэнд.

В конкретных терминах:

- `RamLfeProgramPolicy` и `RamLfeExecutionReceipt` относятся к типам LFE-слоев.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` и
  `BfvRamProgramProfile` — это типы слоев FHE.
- `HiddenRamFheProgram` и `HiddenRamFheInstruction` — внутренние имена для
  скрытая программа BFV, выполняемая запрограммированным сервером. Они остаются на
  Сторона FHE, поскольку они описывают зашифрованный механизм выполнения, а не
  внешняя политика или абстракция получения.

## 1.3 Идентификация учетной записи и псевдонимы

Внедрение универсальной учетной записи не меняет каноническую модель идентификации учетной записи:- `AccountId` остается каноническим субъектом учетной записи без домена.
— Значения `AccountAlias` представляют собой отдельные привязки SNS поверх этого субъекта. А
  псевдоним с указанием домена, например `merchant@banka.paynet`, и псевдоним корня пространства данных.
  такие как `merchant@paynet`, оба могут разрешаться в один и тот же канонический `AccountId`.
- Каноническая регистрация аккаунта всегда `Account::new(AccountId)`/
  `NewAccount::new(AccountId)`; нет доменно-квалифицированных или доменно-материализованных
  путь регистрации.
- Владение доменом, разрешения на псевдонимы и другие действия на уровне домена в реальном времени.
  в их собственном состоянии и API, а не в самой учетной записи.
- Поиск общедоступных учетных записей следует этому разделению: запросы псевдонимов остаются общедоступными, а
  канонический идентификатор учетной записи остается чистым `AccountId`.

Правило реализации операторов, SDK и тестов: начните с канонического
`AccountId`, затем добавьте аренду псевдонимов, разрешения на пространство данных/домен и все
состояние собственности домена отдельно. Не синтезируйте поддельную учетную запись, основанную на псевдониме.
или ожидать появления любого поля связанного домена в записях учетной записи только потому, что псевдоним или
маршрут несет сегмент домена.

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. Получение и проверка UAID

Существует три поддерживаемых способа получения UAID:

1. **Считайте его из состояния мира или моделей SDK.** Любой `Account`/`AccountDetails`
   полезная нагрузка, запрошенная через Torii, теперь имеет поле `uaid`, заполняемое при
   участник выбрал универсальные учетные записи.
2. **Запросите реестры UAID.** Torii предоставляет
   `GET /v1/space-directory/uaids/{uaid}`, который возвращает привязки пространства данных.
   и метаданные манифеста, которые сохраняются на хосте Space Directory (см.
   `docs/space-directory.md` §3 для образцов полезной нагрузки).
3. **Выведите его детерминированно.** При загрузке новых UAID в автономном режиме хешируйте
   каноническое начальное число участника с Blake2b-256 и префикс результата с
   `uaid:`. Фрагмент ниже отражает помощник, описанный в
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```Всегда храните литерал в нижнем регистре и нормализуйте пробелы перед хешированием.
Помощники CLI, такие как `iroha app space-directory manifest scaffold` и Android
Анализатор `UaidLiteral` применяет те же правила обрезки, поэтому проверки управления могут
значения перекрестной проверки без специальных сценариев.

## 3. Проверка активов и манифестов UAID

Детерминированный агрегатор портфелей в `iroha_core::nexus::portfolio`
отображает каждую пару актив/пространство данных, которая ссылается на UAID. Операторы и SDK
может потреблять данные через следующие поверхности:

| Поверхность | Использование |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Возвращает пространство данных → актив → сводные данные о балансе; описано в `docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | Перечисляет идентификаторы пространства данных + литералы учетной записи, привязанные к UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Предоставляет полную историю `AssetPermissionManifest` для аудита. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Ярлык CLI, который оборачивает конечную точку привязки и при необходимости записывает JSON на диск (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Извлекает пакет JSON манифеста для пакетов доказательств. |

Пример сеанса CLI (URL-адрес Torii, настроенный через `torii_api_url` в `iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

Храните снимки JSON вместе с хешем манифеста, используемым во время проверок; тот
Наблюдатель Space Directory перестраивает карту `uaid_dataspaces` всякий раз, когда она манифестируется.
активировать, истечь или отозвать, поэтому эти снимки — самый быстрый способ доказать
какие привязки были активны в данную эпоху.## 4. Публикация манифестов возможностей с доказательствами

Используйте приведенный ниже порядок командной строки всякий раз, когда развертывается новое разрешение. Каждый шаг должен
попадут в пакет доказательств, записанный для утверждения руководством.

1. **Закодируйте манифест в формате JSON**, чтобы рецензенты видели детерминированный хэш раньше.
   подача:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Опубликуйте допуск**, используя полезную нагрузку Norito (`--manifest`) или
   описание JSON (`--manifest-json`). Запишите квитанцию Torii/CLI плюс
   хеш инструкции `PublishSpaceDirectoryManifest`:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Соберите доказательства SpaceDirectoryEvent.** Подпишитесь на рассылку
   `SpaceDirectoryEvent::ManifestActivated` и включите полезную нагрузку события в
   пакет, чтобы аудиторы могли подтвердить момент поступления изменения.

4. **Создайте пакет аудита**, привязав манифест к его профилю пространства данных и
   телеметрические крючки:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Проверьте привязки через Torii** (`bindings fetch` и `manifests fetch`) и
   заархивируйте эти файлы JSON с помощью хэша + пакета, указанного выше.

Контрольный список доказательств:

- [ ] Хэш манифеста (`*.manifest.hash`), подписанный утверждающим изменения.
- [ ] Получение CLI/Torii для вызова публикации (стандартный вывод или артефакт `--json-out`).
- [ ] `SpaceDirectoryEvent` полезная нагрузка, подтверждающая активацию.
- [ ] Аудит каталога пакета с профилем пространства данных, перехватчиками и копией манифеста.
- [ ] Привязки + снимки манифеста, полученные из Torii после активации.Это отражает требования `docs/space-directory.md` §3.2 при предоставлении SDK.
владельцы единственной страницы, на которую они могут указать во время проверки выпуска.

## 5. Шаблоны манифеста регулятора/региона

Используйте встроенные в репозиторий фикстуры в качестве отправной точки при создании манифестов возможностей.
для регулирующих органов или региональных надзорных органов. Они демонстрируют, как разрешить/запретить область действия.
правила и объяснить политические замечания, которые ожидают рецензенты.

| Крепеж | Цель | Основные моменты |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | Лента аудита ESMA/ESRB. | Разрешения только для чтения для `compliance.audit::{stream_reports, request_snapshot}` с запретом на выигрыш при розничных переводах, чтобы сохранить пассивность UAID регулятора. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | Переулок надзора JFSA. | Добавляет ограниченное разрешение `cbdc.supervision.issue_stop_order` (окно PerDay + `max_amount`) и явный запрет на `force_liquidation` для обеспечения двойного контроля. |

При клонировании этих приборов обновите:

1. Идентификаторы `uaid` и `dataspace`, соответствующие участнику и полосе, которую вы включаете.
2. Окна `activation_epoch`/`expiry_epoch` на основе расписания управления.
3. Поля `notes` со ссылками на политику регулятора (статья MiCA, JFSA
   круговой и др.).
4. Окна допусков (`PerSlot`, `PerMinute`, `PerDay`) и опционально
   `max_amount` ограничивает, поэтому SDK применяет те же ограничения, что и хост.

## 6. Примечания по миграции для потребителей SDKСуществующие интеграции SDK, которые ссылаются на идентификаторы учетных записей для каждого домена, должны быть перенесены на
UAID-ориентированные поверхности, описанные выше. Используйте этот контрольный список во время обновлений:

  идентификаторы учетных записей. Для Rust/JS/Swift/Android это означает обновление до последней версии.
  ящики рабочей области или повторное создание привязок Norito.
- **Вызовы API:** Замените запросы портфолио на уровне домена на
  `GET /v1/accounts/{uaid}/portfolio` и конечные точки манифеста/привязок.
  `GET /v1/accounts/{uaid}/portfolio` принимает дополнительный запрос `asset_id`.
  параметр, когда кошелькам нужен только один экземпляр актива. Помощники клиентов, такие как
  как `ToriiClient.getUaidPortfolio` (JS) и Android
  `SpaceDirectoryClient` уже оборачивает эти маршруты; предпочитаю их сделанным на заказ
  HTTP-код.
- **Кэширование и телеметрия:** Кэшируйте записи по UAID + пространству данных вместо необработанных данных.
  идентификаторы учетных записей и отправлять телеметрию, показывающую литерал UAID, чтобы операции могли
  сопоставить журналы с данными Space Directory.
- **Обработка ошибок:** Новые конечные точки возвращают строгие ошибки анализа UAID.
  описано в `docs/source/torii/portfolio_api.md`; раскрыть эти коды
  дословно, чтобы группы поддержки могли решать проблемы без повторных действий.
- **Тестирование:** Подключите упомянутые выше устройства (плюс ваши собственные манифесты UAID).
  в наборы тестов SDK для подтверждения двусторонних проверок Norito и оценок манифестов.
  соответствовать реализации хоста.

## 7. Ссылки- `docs/space-directory.md` — инструкция для оператора с более подробной информацией о жизненном цикле.
- `docs/source/torii/portfolio_api.md` — схема REST для портфеля UAID и
  манифестировать конечные точки.
- `crates/iroha_cli/src/space_directory.rs` — реализация CLI, упомянутая в
  это руководство.
- `fixtures/space_directory/capability/*.manifest.json` — регулятор, розничная торговля и
  Шаблоны манифестов CBDC готовы к клонированию.
