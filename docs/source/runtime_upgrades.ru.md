---
lang: ru
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee7142a82f21e646d4d71844adbf779d180e5647
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# Runtime upgrades (IVM + Host) — без простоя и без hardfork

Этот документ задает детерминированный, управляемый governance механизм для развертывания
runtime upgrades без остановки сети и без hardfork. Узлы заранее раскатывают бинарники;
активация координируется on-chain в ограниченном окне высот. Старые контракты продолжают
работать без изменений; поверхность ABI хоста фиксирована на v1.

Примечание (первый релиз): ABI v1 фиксирован, повышения версии ABI не планируются. Runtime upgrade manifest должны задавать `abi_version = 1`, а `added_syscalls`/`added_pointer_types` должны быть пустыми.

Цели
- Детерминированная активация в запланированном окне высот с идемпотентным применением.
- Сохранить стабильность ABI v1; runtime upgrades не меняют поверхность ABI хоста.
- Ограничители admission и execution, чтобы payloads до активации не включали новое поведение.
- Удобный для операторов rollout с видимостью возможностей и ясными режимами отказа.

Не цели
- Ввод новых ABI версий или расширение поверхностей syscalls/pointer-типов (вне области в этом релизе).
- Изменение существующих номеров syscall или ID pointer-типов (запрещено).
- Живой патчинг узлов без раскатки обновленных бинарников.

Определения
- Версия ABI: небольшое целое, объявляемое в `ProgramMetadata.abi_version`, которое выбирает `SyscallPolicy` и allowlist pointer-типов. В первом релизе фиксируется на `1`.
- ABI Hash: детерминированный digest поверхности ABI для данной версии: список syscalls (номера+формы), ID/allowlist pointer-типов и флаги политики; вычисляется `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: host mapping, который решает, разрешен ли номер syscall для данной версии ABI и политики хоста.
- Activation Window: полуоткрытый интервал высоты блока `[start, end)`, в котором активация валидна ровно один раз в `start`.

Объекты состояния (Data Model)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 от канонических Norito bytes manifest.
- Поля `RuntimeUpgradeManifest`:
  - `name: String` — читаемая метка.
  - `description: String` — короткое описание для операторов.
  - `abi_version: u16` — целевая версия ABI для активации (должна быть 1 в первом релизе).
  - `abi_hash: [u8; 32]` — канонический ABI hash целевой политики.
  - `added_syscalls: Vec<u16>` — резервный список дельт; в первом релизе должен быть пустым.
  - `added_pointer_types: Vec<u16>` — резервный список дельт; в первом релизе должен быть пустым.
  - `start_height: u64` — первая высота блока, где разрешена активация.
  - `end_height: u64` — эксклюзивный верхний предел окна активации.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — SBOM digests для артефактов апгрейда.
  - `slsa_attestation: Vec<u8>` — сырые bytes SLSA attestation (base64 в JSON).
  - `provenance: Vec<ManifestProvenance>` — подписи над каноническим payload.
- Поля `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` — канонический payload предложения.
  - `status: RuntimeUpgradeStatus` — состояние жизненного цикла предложения.
  - `proposer: AccountId` — субъект, который отправил предложение.
  - `created_height: u64` — высота блока, где предложение вошло в ledger.
- Поля `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` — идентификатор алгоритма digest.
  - `digest: Vec<u8>` — сырые bytes digest (base64 в JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - Инварианты: `end_height > start_height`; `abi_version` должен быть `1`; `abi_hash` должен совпадать с `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`; `added_*` должен быть пустым; существующие номера/ID НЕЛЬЗЯ удалять или перенумеровывать.

Storage layout
- `world.runtime_upgrades`: MVCC map с ключом `RuntimeUpgradeId.0` (сырые 32 bytes hash) и значениями, закодированными как канонические Norito payloads `RuntimeUpgradeRecord`. Записи сохраняются между блоками; commits идемпотентны и безопасны при replay.

Инструкции (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Эффекты: вставить `RuntimeUpgradeRecord { status: Proposed }` с ключом `RuntimeUpgradeId`, если отсутствует.
  - Отклонить, если окно пересекается с любым Proposed/Activated record или если инварианты нарушены.
  - Идемпотентно: повторная отправка тех же канонических bytes manifest не меняет состояние.
  - Каноническое кодирование: bytes manifest должны соответствовать `RuntimeUpgradeManifest::canonical_bytes()`; неканонические кодировки отвергаются.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Предусловия: существует соответствующий Proposed record; `current_height` должен равняться `manifest.start_height`; `current_height < manifest.end_height`.
  - Эффекты: переключить record на `ActivatedAt(current_height)`; активный набор ABI остается `{1}` в первом релизе.
  - Идемпотентно: replays на той же высоте являются no-op; другие высоты детерминированно отклоняются.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Предусловия: статус Proposed и `current_height < manifest.start_height`.
  - Эффекты: переключить на `Canceled`.

События (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Правила admission
- Admission контрактов: в первом релизе допускается только `ProgramMetadata.abi_version = 1`; остальные значения отклоняются с `IvmAdmissionError::UnsupportedAbiVersion`.
  - Для ABI v1 пересчитать `abi_hash(1)` и требовать равенства с payload/manifest при наличии; иначе отклонить с `IvmAdmissionError::ManifestAbiHashMismatch`.
- Admission транзакций: инструкции `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` требуют соответствующих прав (root/sudo); должны удовлетворять ограничениям перекрытия окон.

Enforcement provenance
- Runtime-upgrade manifests могут содержать SBOM digests (`sbom_digests`), bytes SLSA attestation (`slsa_attestation`) и метаданные подписантов (`provenance` signatures). Подписи покрывают канонический `RuntimeUpgradeManifestSignaturePayload` (все поля manifest кроме списка подписей `provenance`).
- Конфигурация governance управляет enforcement через `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (разрешить отсутствие provenance, проверять если есть) или `required` (отклонять при отсутствии provenance).
  - `require_sbom`: когда `true`, требуется как минимум один SBOM digest.
  - `require_slsa`: когда `true`, требуется непустой SLSA attestation.
  - `trusted_signers`: список утвержденных публичных ключей подписантов.
  - `signature_threshold`: минимальное число доверенных подписей.
- Отказы provenance возвращают стабильные коды ошибок в отказах инструкций (префикс `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetry: `runtime_upgrade_provenance_rejections_total{reason}` считает причины отказов provenance.

Правила исполнения
- Политика VM Host: во время исполнения программы вывести `SyscallPolicy` из `ProgramMetadata.abi_version`. Неизвестные syscalls для этой версии мапятся в `VMError::UnknownSyscall`.
- Pointer-ABI: allowlist выводится из `ProgramMetadata.abi_version`; типы вне allowlist для этой версии отклоняются при decode/validation.
- Переключение host: каждый блок пересчитывает активный ABI набор; в первом релизе он остается `{1}`, но активация записывается и идемпотентна (проверено `runtime_upgrade_admission::activation_allows_v1_in_same_block`).
  - Привязка политики syscalls: `CoreHost` читает заявленную ABI версию транзакции и применяет `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` против `SyscallPolicy` на блок. Host переиспользует VM инстанс с областью транзакции, поэтому mid-block активации безопасны — поздние транзакции наблюдают обновленную политику, а ранние продолжают с исходной версией.

Инварианты детерминизма и безопасности
- Активация происходит только на `start_height` и идемпотентна; reorg ниже `start_height` детерминированно применяет ее при возвращении блока.
- Активный набор ABI фиксирован на `{1}` в первом релизе.
- Никакая динамическая договоренность не влияет на консенсус или порядок исполнения; gossip возможностей носит информационный характер.

Operator rollout (без простоя)
1) Развернуть бинарник узла с новым runtime артефактом при сохранении ABI v1.
2) Наблюдать готовность флота через telemetry.
3) Отправить `ProposeRuntimeUpgrade` с окном достаточно заранее (например, `H+N`).
4) На `start_height` `ActivateRuntimeUpgrade` исполняется в составе включенного блока и фиксирует активацию; ABI остается v1.

Torii и CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ abi_version: u16 }` (implemented)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implemented)
  - `GET /v1/runtime/upgrades` -> список records (implemented).
  - `POST /v1/runtime/upgrades/propose` -> оборачивает `ProposeRuntimeUpgrade` (возвращает instruction skeleton; implemented).
  - `POST /v1/runtime/upgrades/activate/:id` -> оборачивает `ActivateRuntimeUpgrade` (возвращает instruction skeleton; implemented).
  - `POST /v1/runtime/upgrades/cancel/:id` -> оборачивает `CancelRuntimeUpgrade` (возвращает instruction skeleton; implemented).
- CLI
  - `iroha runtime abi active` (implemented)
  - `iroha runtime abi hash` (implemented)
  - `iroha runtime upgrade list` (implemented)
  - `iroha runtime upgrade propose --file <manifest.json>` (implemented)
  - `iroha runtime upgrade activate --id <id>` (implemented)
  - `iroha runtime upgrade cancel --id <id>` (implemented)

Core Query API
- Norito одиночный запрос (подписанный):
  - `FindAbiVersion` возвращает Norito-структуру `{ abi_version: u16 }`.
  - См. пример: `docs/source/samples/find_active_abi_versions.md` (тип/поля и JSON пример).

Примечания по реализации (только v1)
- iroha_data_model
  - Добавить `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums инструкций, события и JSON/Norito codecs с roundtrip тестами.
- iroha_core
  - WSV: добавить реестр `runtime_upgrades` с проверками пересечений и getters.
  - Executors: реализовать ISI handlers; эмитить события; применять правила admission.
  - Admission: гейтить program manifests по активности `abi_version` и равенству `abi_hash`.
  - Syscall policy mapping: передать активный ABI набор в конструктор VM host; обеспечить детерминизм через высоту блока на старте исполнения.
  - Tests: идемпотентность окна активации, отклонения перекрытий, поведение admission до/после.
- ivm
  - Поверхность ABI фиксирована на v1; списки syscalls и ABI hashes закреплены golden tests.
- iroha_cli / iroha_torii
  - Добавить перечисленные выше endpoints и команды; Norito JSON helpers для manifests; базовые интеграционные тесты.
- Kotodama compiler
  - Эмитить `abi_version = 1` и встраивать canonical v1 `abi_hash` в `.to` manifests.

Telemetry
- Добавить gauge `runtime.abi_version` и counter `runtime.upgrade_events_total{kind}`.

Security considerations
- Только root/sudo могут propose/activate/cancel; manifests должны быть корректно подписаны.
- Окна активации предотвращают front-running и обеспечивают детерминированное применение.
- `abi_hash` фиксирует поверхность интерфейса, предотвращая скрытый drift между бинарниками.

Acceptance criteria (Conformance)
- Узлы детерминированно отвергают код с `abi_version != 1` всегда.
- Runtime upgrades не меняют ABI политику; существующие программы продолжают работать без изменений с v1.
- Golden tests для ABI hashes и списков syscalls проходят на x86-64/ARM64.
- Активация идемпотентна и безопасна при reorg.
