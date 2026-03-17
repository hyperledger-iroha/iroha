---
lang: ur
direction: rtl
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee7142a82f21e646d4d71844adbf779d180e5647
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/runtime_upgrades.md -->

# Runtime upgrades (IVM + Host) — بغیر downtime، بغیر hardfork

یہ دستاویز runtime upgrades کو نیٹ ورک روکے بغیر اور hardfork کے بغیر rollout کرنے کیلئے ایک
deterministic اور governance-controlled میکانزم بیان کرتی ہے۔ Nodes پہلے سے binaries رول آؤٹ
کرتے ہیں؛ activation آن چین ایک محدود height window کے اندر coordinated ہوتی ہے۔ پرانے
contracts بغیر تبدیلی کے چلتے رہتے ہیں؛ host ABI surface v1 پر فکس رہتی ہے۔

نوٹ (پہلا ریلیز): ABI v1 فکس ہے اور ABI version بڑھانے کا کوئی پلان نہیں۔ runtime upgrade manifests کو `abi_version = 1` رکھنا ہوگا، اور `added_syscalls`/`added_pointer_types` خالی ہوں گی۔

Goals
- مقررہ height window میں deterministic activation اور idempotent application۔
- ABI v1 کی استحکام برقرار رکھنا؛ runtime upgrades host ABI surface نہیں بدلتے۔
- Admission اور execution guardrails تاکہ pre-activation payloads نیا behavior enable نہ کر سکیں۔
- Operator-friendly rollout جس میں capability visibility اور واضح failure modes ہوں۔

Non-Goals
- نئی ABI versions متعارف کرنا یا syscall/pointer-type surfaces بڑھانا (اس ریلیز میں scope سے باہر)۔
- موجودہ syscall numbers یا pointer-type IDs کو بدلنا (منع ہے)۔
- updated binaries deploy کئے بغیر nodes کو live patch کرنا۔

Definitions
- ABI Version: `ProgramMetadata.abi_version` میں اعلان کردہ چھوٹا integer جو `SyscallPolicy` اور pointer-type allowlist منتخب کرتا ہے۔ پہلے ریلیز میں `1` پر فکس۔
- ABI Hash: مخصوص version کیلئے ABI surface کا deterministic digest: syscall list (numbers+shapes)، pointer-type IDs/allowlist، اور policy flags؛ اسے `ivm::syscalls::compute_abi_hash` compute کرتا ہے۔
- Syscall Policy: host mapping جو طے کرتی ہے کہ دی گئی ABI version اور host policy کیلئے syscall number allowed ہے یا نہیں۔
- Activation Window: half-open block-height interval `[start, end)` جس میں activation ٹھیک ایک بار `start` پر معتبر ہوتی ہے۔

State Objects (Data Model)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: manifest کے canonical Norito bytes کا Blake2b-256۔
- `RuntimeUpgradeManifest` fields:
  - `name: String` — human-readable label۔
  - `description: String` — operators کیلئے مختصر وضاحت۔
  - `abi_version: u16` — target ABI version جو activate ہو گی (پہلے ریلیز میں 1 ہونا چاہیے)۔
  - `abi_hash: [u8; 32]` — target policy کیلئے canonical ABI hash۔
  - `added_syscalls: Vec<u16>` — محفوظ delta list؛ پہلے release میں خالی رہنی چاہیے۔
  - `added_pointer_types: Vec<u16>` — محفوظ delta list؛ پہلے release میں خالی رہنی چاہیے۔
  - `start_height: u64` — پہلی block height جہاں activation allowed ہو۔
  - `end_height: u64` — activation window کی exclusive upper bound۔
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — upgrade artifacts کیلئے SBOM digests۔
  - `slsa_attestation: Vec<u8>` — raw SLSA attestation bytes (JSON میں base64)۔
  - `provenance: Vec<ManifestProvenance>` — canonical payload پر signatures۔
- `RuntimeUpgradeRecord` fields:
  - `manifest: RuntimeUpgradeManifest` — canonical proposal payload۔
  - `status: RuntimeUpgradeStatus` — proposal lifecycle state۔
  - `proposer: AccountId` — authority جس نے proposal submit کیا۔
  - `created_height: u64` — block height جہاں proposal ledger میں داخل ہوا۔
- `RuntimeUpgradeSbomDigest` fields:
  - `algorithm: String` — digest algorithm identifier۔
  - `digest: Vec<u8>` — raw digest bytes (JSON میں base64)۔
<!-- END RUNTIME UPGRADE TYPES -->
  - Invariants: `end_height > start_height`; `abi_version` کو `1` ہونا چاہیے؛ `abi_hash` کو `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)` کے برابر ہونا چاہیے؛ `added_*` خالی ہونا چاہیے؛ موجودہ numbers/IDs کو کبھی remove یا renumber نہ کریں۔

Storage Layout
- `world.runtime_upgrades`: MVCC map جس کی key `RuntimeUpgradeId.0` (raw 32-byte hash) ہے اور values canonical Norito `RuntimeUpgradeRecord` payloads میں encoded ہیں۔ entries blocks کے درمیان persist رہتی ہیں؛ commits idempotent اور replay-safe ہیں۔

Instructions (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Effects: اگر موجود نہ ہو تو `RuntimeUpgradeRecord { status: Proposed }` کو `RuntimeUpgradeId` key کے تحت insert کریں۔
  - Reject اگر window کسی اور Proposed/Activated record سے overlap کرے یا invariants fail ہوں۔
  - Idempotent: وہی canonical manifest bytes دوبارہ submit کرنے سے کچھ نہیں ہوتا۔
  - Canonical encoding: manifest bytes کو `RuntimeUpgradeManifest::canonical_bytes()` سے match ہونا چاہیے؛ non-canonical encodings reject ہوں گے۔
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: matching Proposed record موجود ہو؛ `current_height` کو `manifest.start_height` کے برابر ہونا چاہیے؛ `current_height < manifest.end_height`۔
  - Effects: record کو `ActivatedAt(current_height)` پر flip کریں؛ active ABI set پہلی ریلیز میں `{1}` رہتا ہے۔
  - Idempotent: اسی height پر replays no-op ہیں؛ دیگر heights deterministically reject ہوتی ہیں۔
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: status Proposed ہو اور `current_height < manifest.start_height`۔
  - Effects: `Canceled` پر flip کریں۔

Events (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Admission Rules
- Contract Admission: پہلے ریلیز میں صرف `ProgramMetadata.abi_version = 1` قبول ہے؛ باقی اقدار `IvmAdmissionError::UnsupportedAbiVersion` کے ساتھ reject ہوتی ہیں۔
  - ABI v1 کیلئے `abi_hash(1)` دوبارہ compute کریں اور payload/manifest سے مطابقت مانگیں؛ ورنہ `IvmAdmissionError::ManifestAbiHashMismatch` کے ساتھ reject کریں۔
- Transaction Admission: `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` کیلئے مناسب permissions (root/sudo) درکار ہیں؛ window overlap constraints پوری ہونی چاہئیں۔

Provenance Enforcement
- Runtime-upgrade manifests SBOM digests (`sbom_digests`)، SLSA attestation bytes (`slsa_attestation`)، اور signer metadata (`provenance` signatures) لے سکتے ہیں۔ signatures canonical `RuntimeUpgradeManifestSignaturePayload` پر ہوتے ہیں (manifest کے تمام fields سوا `provenance` signature list)۔
- Governance config `governance.runtime_upgrade_provenance` کے تحت enforcement کنٹرول کرتی ہے:
  - `mode`: `optional` (missing provenance قبول کرے، موجود ہو تو verify) یا `required` (provenance غائب ہو تو reject)۔
  - `require_sbom`: جب `true` ہو تو کم از کم ایک SBOM digest ضروری ہے۔
  - `require_slsa`: جب `true` ہو تو non-empty SLSA attestation ضروری ہے۔
  - `trusted_signers`: منظور شدہ signer public keys کی فہرست۔
  - `signature_threshold`: کم از کم قابل اعتماد signatures کی تعداد۔
- Provenance rejections instruction failures میں stable error codes کے ساتھ ظاہر ہوتے ہیں (prefix `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetry: `runtime_upgrade_provenance_rejections_total{reason}` provenance rejection reasons گنتا ہے۔

Execution Rules
- VM Host Policy: program execution کے دوران `ProgramMetadata.abi_version` سے `SyscallPolicy` derive کریں۔ اس version کیلئے unknown syscalls `VMError::UnknownSyscall` پر map ہوتے ہیں۔
- Pointer-ABI: allowlist `ProgramMetadata.abi_version` سے derive ہوتی ہے؛ اس version کیلئے allowlist سے باہر types decode/validation میں reject ہوتے ہیں۔
- Host Switching: ہر block active ABI set دوبارہ compute کرتا ہے؛ پہلی ریلیز میں یہ `{1}` رہتا ہے، مگر activation record ہوتی ہے اور idempotent ہے ( `runtime_upgrade_admission::activation_allows_v1_in_same_block` سے validate)۔
  - Syscall policy binding: `CoreHost` transaction کی declared ABI version پڑھتا ہے اور `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` کو per-block `SyscallPolicy` کے خلاف enforce کرتا ہے۔ host transaction-scoped VM instance reuse کرتا ہے، لہذا mid-block activations محفوظ ہیں — بعد والی transactions updated policy observe کرتی ہیں جبکہ پہلے والی اپنی original version پر رہتی ہیں۔

Determinism & Safety Invariants
- Activation صرف `start_height` پر ہوتی ہے اور idempotent ہے؛ `start_height` سے نیچے reorgs deterministic طور پر دوبارہ apply ہوتے ہیں جب block دوبارہ آ جائے۔
- Active ABI set پہلی ریلیز میں `{1}` پر فکس ہے۔
- کوئی dynamic negotiation consensus یا execution order پر اثر نہیں ڈالتی؛ capability gossip صرف informational ہے۔

Operator Rollout (No Downtime)
1) ایسا node binary deploy کریں جو نیا runtime artifact شامل کرے اور ABI v1 برقرار رکھے۔
2) telemetry سے fleet readiness دیکھیں۔
3) `ProposeRuntimeUpgrade` ایک مناسب طور پر آگے window کے ساتھ submit کریں (مثلاً `H+N`)۔
4) `start_height` پر `ActivateRuntimeUpgrade` شامل block کا حصہ بن کر execute ہوتی ہے اور activation ریکارڈ ہوتی ہے؛ ABI v1 پر رہتی ہے۔

Torii & CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ abi_version: u16 }` (implemented)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implemented)
  - `GET /v1/runtime/upgrades` -> records کی فہرست (implemented)۔
  - `POST /v1/runtime/upgrades/propose` -> `ProposeRuntimeUpgrade` کو wrap کرتا ہے (instruction skeleton واپس کرتا ہے; implemented)۔
  - `POST /v1/runtime/upgrades/activate/:id` -> `ActivateRuntimeUpgrade` کو wrap کرتا ہے (instruction skeleton واپس کرتا ہے; implemented)۔
  - `POST /v1/runtime/upgrades/cancel/:id` -> `CancelRuntimeUpgrade` کو wrap کرتا ہے (instruction skeleton واپس کرتا ہے; implemented)۔
- CLI
  - `iroha runtime abi active` (implemented)
  - `iroha runtime abi hash` (implemented)
  - `iroha runtime upgrade list` (implemented)
  - `iroha runtime upgrade propose --file <manifest.json>` (implemented)
  - `iroha runtime upgrade activate --id <id>` (implemented)
  - `iroha runtime upgrade cancel --id <id>` (implemented)

Core Query API
- Norito singular query (signed):
  - `FindAbiVersion` Norito-encoded struct `{ abi_version: u16 }` واپس کرتا ہے۔
  - مثال: `docs/source/samples/find_active_abi_versions.md` (type/fields اور JSON example)۔

Implementation Notes (صرف v1)
- iroha_data_model
  - `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, instruction enums, events، اور JSON/Norito codecs کو roundtrip tests کے ساتھ شامل کریں۔
- iroha_core
  - WSV: `runtime_upgrades` registry شامل کریں جس میں overlap checks اور getters ہوں۔
  - Executors: ISI handlers implement کریں؛ events emit کریں؛ admission rules enforce کریں۔
  - Admission: program manifests کو `abi_version` activity اور `abi_hash` equality کے مطابق gate کریں۔
  - Syscall policy mapping: active ABI set کو VM host constructor تک پہنچائیں؛ determinism کیلئے execution start پر block height استعمال کریں۔
  - Tests: activation window idempotency، overlap rejections، pre/post admission behavior۔
- ivm
  - ABI surface v1 پر فکس ہے؛ syscall lists اور ABI hashes golden tests سے pin ہیں۔
- iroha_cli / iroha_torii
  - اوپر listed endpoints اور commands شامل کریں؛ manifests کیلئے Norito JSON helpers؛ بنیادی integration tests۔
- Kotodama compiler
  - `abi_version = 1` emit کریں اور canonical v1 `abi_hash` کو `.to` manifests میں embed کریں۔

Telemetry
- `runtime.abi_version` gauge اور `runtime.upgrade_events_total{kind}` counter شامل کریں۔

Security Considerations
- صرف root/sudo propose/activate/cancel کر سکتے ہیں؛ manifests مناسب طور پر signed ہوں۔
- Activation windows front-running کو روکتی ہیں اور deterministic application یقینی بناتی ہیں۔
- `abi_hash` interface surface کو pin کرتا ہے تاکہ binaries کے درمیان silent drift نہ ہو۔

Acceptance Criteria (Conformance)
- Nodes `abi_version != 1` کے ساتھ code کو ہر وقت deterministically reject کرتے ہیں۔
- Runtime upgrades ABI policy نہیں بدلتے؛ موجودہ programs v1 کے ساتھ بغیر تبدیلی کے چلتے رہتے ہیں۔
- ABI hashes اور syscall lists کیلئے golden tests x86-64/ARM64 پر پاس ہوں۔
- Activation idempotent ہے اور reorgs میں محفوظ ہے۔

</div>
