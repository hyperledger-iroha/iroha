---
lang: ru
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8990e19977e7f3fb370b9c8f66064542135fa171
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# Runtime upgrades (IVM + Host) — без простоя и без hardfork

Этот документ задает детерминированный, управляемый governance механизм для добавления новых
возможностей IVM/host (например, новых syscalls и типов pointer-ABI) без остановки сети и
без hardfork. Узлы заранее раскатывают бинарники; активация координируется on-chain в
ограниченном окне высот. Старые контракты продолжают работать без изменений; новые возможности
гейтятся по версии ABI и политике.

Примечание (первый релиз): поддерживается только ABI v1. Runtime upgrade manifest для других ABI версий отклоняются до будущего релиза с новой ABI.

Цели
- Детерминированная активация в запланированном окне высот с идемпотентным применением.
- Сосуществование нескольких ABI версий; не ломать существующие бинарники.
- Ограничители admission и execution, чтобы payloads до активации не включали новое поведение.
- Удобный для операторов rollout с видимостью возможностей и ясными режимами отказа.

Не цели
- Изменение существующих номеров syscall или ID pointer-типов (запрещено).
- Живой патчинг узлов без раскатки обновленных бинарников.

Определения
- Версия ABI: небольшое целое, объявляемое в `ProgramMetadata.abi_version`, которое выбирает `SyscallPolicy` и allowlist pointer-типов.
- ABI Hash: детерминированный digest поверхности ABI для данной версии: список syscalls (номера+формы), ID/allowlist pointer-типов и флаги политики; вычисляется `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: host mapping, который решает, разрешен ли номер syscall для данной версии ABI и политики хоста.
- Activation Window: полуоткрытый интервал высоты блока `[start, end)`, в котором активация валидна ровно один раз в `start`.

Объекты состояния (Data Model)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 от канонических Norito bytes manifest.
- Поля `RuntimeUpgradeManifest`:
  - `name: String` — читаемая метка.
  - `description: String` — короткое описание для операторов.
  - `abi_version: u16` — целевая версия ABI для активации.
  - `abi_hash: [u8; 32]` — канонический ABI hash целевой политики.
  - `added_syscalls: Vec<u16>` — номера syscalls, которые становятся допустимыми в этой версии.
  - `added_pointer_types: Vec<u16>` — идентификаторы pointer-типов, добавленных апгрейдом.
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
  - Инварианты: `end_height > start_height`; `abi_version` строго больше любой активной версии; `abi_hash` должен совпадать с `ivm::syscalls::compute_abi_hash(policy_for(abi_version))`; `added_*` должен перечислять ровно аддитивную разницу между новой ABI политикой и предыдущей активной; существующие номера/ID НЕЛЬЗЯ удалять или перенумеровывать.

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
  - Эффекты: переключить record на `ActivatedAt(current_height)`; добавить `abi_version` в активный набор ABI.
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
- Переключение host: каждый блок пересчитывает активный ABI набор; как только активация коммитится, последующие транзакции в этом же блоке наблюдают новую политику (проверено `runtime_upgrade_admission::activation_allows_new_abi_in_same_block`).
  - Привязка политики syscalls: `CoreHost` читает заявленную ABI версию транзакции и применяет `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` против `SyscallPolicy` на блок. Host переиспользует VM инстанс с областью транзакции, поэтому mid-block активации безопасны — поздние транзакции наблюдают обновленную политику, а ранние продолжают с исходной версией.

Инварианты детерминизма и безопасности
- Активация происходит только на `start_height` и идемпотентна; reorg ниже `start_height` детерминированно применяет ее при возвращении блока.
- Существующие версии ABI остаются активными бесконечно; новые версии лишь расширяют активный набор.
- Никакая динамическая договоренность не влияет на консенсус или порядок исполнения; gossip возможностей носит информационный характер.

Operator rollout (без простоя)
1) Развернуть бинарник узла, поддерживающий новую ABI версию (`v+1`), но не активировать ее.
2) Наблюдать возможности флота через telemetry (процент узлов, объявляющих поддержку `v+1`).
3) Отправить `ProposeRuntimeUpgrade` с окном достаточно заранее (например, `H+N`).
4) На `start_height` `ActivateRuntimeUpgrade` исполняется автоматически в составе включенного блока и переключает активный набор host; узлы без апдейта продолжат работать со старыми контрактами, но отклонят admission/execution программ `v+1`.
5) После активации пересобрать/развернуть контракты, нацеленные на `v+1`.

Torii и CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (implemented)
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
  - `FindActiveAbiVersions` возвращает Norito-структуру `{ active_versions: [u16], default_compile_target: u16 }`.
  - См. пример: `docs/source/samples/find_active_abi_versions.md` (тип/поля и JSON пример).

Требуемые изменения кода (по crate)
- iroha_data_model
  - Добавить `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums инструкций, события и JSON/Norito codecs с roundtrip тестами.
- iroha_core
  - WSV: добавить реестр `runtime_upgrades` с проверками пересечений и getters.
  - Executors: реализовать ISI handlers; эмитить события; применять правила admission.
  - Admission: гейтить program manifests по активности `abi_version` и равенству `abi_hash`.
  - Syscall policy mapping: передать активный ABI набор в конструктор VM host; обеспечить детерминизм через высоту блока на старте исполнения.
  - Tests: идемпотентность окна активации, отклонения перекрытий, поведение admission до/после.
- ivm
  - Определить `ABI_V2` (пример) с политикой: расширить `abi_syscall_list()`; mapping `is_syscall_allowed(policy, number)`; расширение политики pointer-типов.
  - Пересчитать и закрепить golden tests: `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`.
- iroha_cli / iroha_torii
  - Добавить перечисленные выше endpoints и команды; Norito JSON helpers для manifests; базовые интеграционные тесты.
- Kotodama compiler
  - Разрешить таргетинг `abi_version = v+1`; встроить корректный `abi_hash` выбранной версии в `.to` manifests.

Telemetry
- Добавить gauge `runtime.active_abi_versions` и counter `runtime.upgrade_events_total{kind}`.

Security considerations
- Только root/sudo могут propose/activate/cancel; manifests должны быть корректно подписаны.
- Окна активации предотвращают front-running и обеспечивают детерминированное применение.
- `abi_hash` фиксирует поверхность интерфейса, предотвращая скрытый drift между бинарниками.

Acceptance criteria (Conformance)
- До активации узлы детерминированно отвергают код с `abi_version = v+1`.
- После активации на `start_height` узлы принимают и исполняют `v+1`; старые программы продолжают работать без изменений.
- Golden tests для ABI hashes и списков syscalls проходят на x86-64/ARM64.
- Активация идемпотентна и безопасна при reorg.
