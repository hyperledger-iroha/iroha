---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: Confidential assets اور ZK transfers
description: shielded circulation، registries اور operator controls کے لئے Phase C کا blueprint۔
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Confidential assets اور ZK transfer design

## Motivation
- opt-in shielded asset flows فراہم کرنا تاکہ domains شفاف circulation بدلے بغیر transactional privacy برقرار رکھ سکیں۔
- auditors اور operators کو circuits اور cryptographic parameters کے لئے lifecycle controls (activation، rotation، revocation) دینا۔

## Threat Model
- Validators honest-but-curious ہیں: consensus faithfully چلاتے ہیں لیکن ledger/state کو inspect کرنے کی کوشش کرتے ہیں۔
- Network observers block data اور gossiped transactions دیکھتے ہیں؛ private gossip channels کا کوئی assumption نہیں۔
- Out of scope: off-ledger traffic analysis، quantum adversaries (PQ roadmap میں علیحدہ ٹریک)، ledger availability attacks۔

## Design Overview
- Assets موجودہ transparent balances کے علاوہ *shielded pool* declare کر سکتے ہیں؛ shielded circulation cryptographic commitments سے represent ہوتی ہے۔
- Notes `(asset_id, amount, recipient_view_key, blinding, rho)` کو encapsulate کرتی ہیں، ساتھ:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`، note ordering سے independent۔
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transactions Norito-encoded `ConfidentialTransfer` payloads لاتی ہیں جن میں:
  - Public inputs: Merkle anchor، nullifiers، new commitments، asset id، circuit version۔
  - Recipients اور optional auditors کے لئے encrypted payloads۔
  - Zero-knowledge proof جو value conservation، ownership اور authorization attest کرتی ہے۔
- Verifying keys اور parameter sets on-ledger registries کے ذریعے activation windows کے ساتھ کنٹرول ہوتے ہیں؛ nodes unknown یا revoked entries کو refer کرنے والی proofs validate کرنے سے انکار کرتے ہیں۔
- Consensus headers active confidential feature digest پر commit کرتے ہیں تاکہ blocks صرف اسی وقت accept ہوں جب registry اور parameter state match کرے۔
- Proof construction Halo2 (Plonkish) stack استعمال کرتی ہے بغیر trusted setup؛ Groth16 یا دیگر SNARK variants v1 میں دانستہ طور پر unsupported ہیں۔

### Deterministic Fixtures

Confidential memo envelopes اب `fixtures/confidential/encrypted_payload_v1.json` میں canonical fixture کے ساتھ آتے ہیں۔ یہ dataset ایک مثبت v1 envelope اور منفی malformed samples پکڑتا ہے تاکہ SDKs parsing parity ثابت کر سکیں۔ Rust data-model tests (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) اور Swift suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) fixture کو براہ راست لوڈ کرتے ہیں، جس سے Norito encoding، error surfaces اور regression coverage codec کے ارتقا کے ساتھ aligned رہتے ہیں۔

Swift SDKs اب bespoke JSON glue کے بغیر shield instructions emit کر سکتے ہیں: 32-byte note commitment، encrypted payload اور debit metadata کے ساتھ `ShieldRequest` بنائیں، پھر `/v1/pipeline/transactions` پر sign اور relay کرنے کے لئے `IrohaSDK.submit(shield:keypair:)` (یا `submitAndWait`) کال کریں۔ Helper commitment lengths validate کرتا ہے، `ConfidentialEncryptedPayload` کو Norito encoder میں thread کرتا ہے، اور نیچے بیان کردہ `zk::Shield` layout کو mirror کرتا ہے تاکہ wallets Rust کے ساتھ lock-step رہیں۔

## Consensus Commitments & Capability Gating
- Block headers `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` expose کرتے ہیں؛ digest consensus hash میں حصہ لیتا ہے اور block acceptance کے لئے local registry view سے match ہونا چاہئے۔
- Governance مستقبل کے `activation_height` کے ساتھ `next_conf_features` پروگرام کر کے upgrades stage کر سکتی ہے؛ اس height تک block producers پچھلا digest emit کرتے رہتے ہیں۔
- Validator nodes کو `confidential.enabled = true` اور `assume_valid = false` کے ساتھ operate کرنا MUST ہے۔ Startup checks validator set join کرنے سے انکار کرتے ہیں اگر کوئی شرط fail ہو یا local `conf_features` diverge ہو۔
- P2P handshake metadata اب `{ enabled, assume_valid, conf_features }` شامل کرتا ہے۔ unsupported features advertise کرنے والے peers `HandshakeConfidentialMismatch` کے ساتھ reject ہوتے ہیں اور consensus rotation میں داخل نہیں ہوتے۔
- Non-validator observers `assume_valid = true` سیٹ کر سکتے ہیں؛ وہ confidential deltas کو blindly apply کرتے ہیں مگر consensus safety پر اثر نہیں ڈالتے۔

## Asset Policies
- ہر asset definition میں creator یا governance کی طرف سے set کیا گیا `AssetConfidentialPolicy` ہوتا ہے:
  - `TransparentOnly`: default mode؛ صرف transparent instructions (`MintAsset`, `TransferAsset` وغیرہ) allowed ہیں اور shielded operations reject ہوتے ہیں۔
  - `ShieldedOnly`: تمام issuance اور transfers کو confidential instructions استعمال کرنا ہوں گے؛ `RevealConfidential` ممنوع ہے تاکہ balances عوامی نہ ہوں۔
  - `Convertible`: holders نیچے بیان کردہ on/off-ramp instructions کے ذریعے transparent اور shielded representations کے درمیان value move کر سکتے ہیں۔
- Policies constrained FSM follow کرتی ہیں تاکہ funds stranded نہ ہوں:
  - `TransparentOnly → Convertible` (shielded pool فوراً enable)
  - `TransparentOnly → ShieldedOnly` (pending transition اور conversion window درکار)
  - `Convertible → ShieldedOnly` (minimum delay لازم)
  - `ShieldedOnly → Convertible` (migration plan درکار تاکہ shielded notes spendable رہیں)
  - `ShieldedOnly → TransparentOnly` disallowed ہے جب تک shielded pool empty نہ ہو یا governance outstanding notes کو unshield کرنے والی migration encode نہ کرے۔
- Governance instructions `ScheduleConfidentialPolicyTransition` ISI کے ذریعے `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` set کرتی ہیں اور `CancelConfidentialPolicyTransition` سے scheduled changes abort کر سکتی ہیں۔ Mempool validation یقینی بناتی ہے کہ کوئی transaction transition height کو straddle نہ کرے اور mid-block policy check change ہونے پر inclusion deterministic طور پر fail ہو۔
- Pending transitions نئے block کے شروع میں خودکار apply ہوتے ہیں: جب block height conversion window میں داخل ہو (ShieldedOnly upgrades کے لئے) یا `effective_height` پہنچے تو runtime `AssetConfidentialPolicy` update کرتا ہے، `zk.policy` metadata refresh کرتا ہے اور pending entry clear کرتا ہے۔ اگر `ShieldedOnly` transition mature ہونے پر transparent supply باقی ہو تو runtime change abort کر کے warning log کرتا ہے اور previous mode برقرار رہتا ہے۔
- Config knobs `policy_transition_delay_blocks` اور `policy_transition_window_blocks` minimum notice اور grace periods enforce کرتے ہیں تاکہ wallets switch کے آس پاس notes convert کر سکیں۔
- `pending_transition.transition_id` audit handle کے طور پر بھی کام کرتا ہے؛ governance کو transitions finalize یا cancel کرتے وقت اسے quote کرنا چاہئے تاکہ operators on/off-ramp reports correlate کر سکیں۔
- `policy_transition_window_blocks` default 720 ہے (60s block time پر تقریباً 12 گھنٹے)۔ Nodes governance requests جو shorter notice چاہیں انہیں clamp کرتے ہیں۔
- Genesis manifests اور CLI flows current اور pending policies expose کرتے ہیں۔ Admission logic execution time پر policy پڑھ کر confirm کرتی ہے کہ ہر confidential instruction authorized ہے۔
- Migration checklist — نیچے “Migration sequencing” میں Milestone M0 کے مطابق staged upgrade plan دیکھیں۔

#### Torii کے ذریعے transitions کی monitoring

Wallets اور auditors `GET /v1/confidential/assets/{definition_id}/transitions` کو poll کر کے active `AssetConfidentialPolicy` دیکھتے ہیں۔ JSON payload ہمیشہ canonical asset id، latest observed block height، `current_mode`، اس height پر effective mode (conversion windows عارضی طور پر `Convertible` report کرتے ہیں)، اور expected `vk_set_hash`/Poseidon/Pedersen identifiers شامل کرتا ہے۔ جب governance transition pending ہو تو response میں یہ بھی ہوتا ہے:

- `transition_id` - `ScheduleConfidentialPolicyTransition` سے واپس ملنے والا audit handle۔
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` اور derived `window_open_height` (وہ block جہاں wallets کو ShieldedOnly cut-over کیلئے conversion شروع کرنا ہو)۔

Example response:

```json
{
  "asset_id": "rose#wonderland",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` response کا مطلب ہے کہ matching asset definition موجود نہیں۔ جب کوئی transition scheduled نہ ہو تو `pending_transition` field `null` ہوتا ہے۔

### Policy state machine

| Current mode       | Next mode        | Prerequisites                                                                 | Effective-height handling                                                                                         | Notes                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance نے verifier/parameter registry entries activate کئے ہوں۔ `ScheduleConfidentialPolicyTransition` submit کریں جس میں `effective_height ≥ current_height + policy_transition_delay_blocks` ہو۔ | Transition بالکل `effective_height` پر execute ہوتا ہے؛ shielded pool فوراً دستیاب ہوتا ہے۔                   | شفاف flows برقرار رکھتے ہوئے confidentiality enable کرنے کا default path۔               |
| TransparentOnly    | ShieldedOnly     | اوپر والی شرائط کے ساتھ `policy_transition_window_blocks ≥ 1` بھی۔                                                         | Runtime `effective_height - policy_transition_window_blocks` پر `Convertible` میں داخل ہوتا ہے؛ `effective_height` پر `ShieldedOnly` میں بدلتا ہے۔ | شفاف instructions disable ہونے سے پہلے deterministic conversion window دیتا ہے۔   |
| Convertible        | ShieldedOnly     | `effective_height ≥ current_height + policy_transition_delay_blocks` کے ساتھ scheduled transition۔ Governance SHOULD audit metadata کے ذریعے (`transparent_supply == 0`) certify کرے؛ runtime cut-over پر enforce کرتا ہے۔ | اوپر جیسی window semantics۔ اگر `effective_height` پر transparent supply non-zero ہو تو transition `PolicyTransitionPrerequisiteFailed` کے ساتھ abort ہو جاتا ہے۔ | asset کو مکمل confidential circulation میں lock کرتا ہے۔                                     |
| ShieldedOnly       | Convertible      | Scheduled transition؛ کوئی active emergency withdrawal نہیں (`withdraw_height` unset)۔                                    | `effective_height` پر state flip ہوتا ہے؛ reveal ramps دوبارہ کھلتی ہیں جبکہ shielded notes valid رہتے ہیں۔                           | maintenance windows یا auditor reviews کے لئے۔                                          |
| ShieldedOnly       | TransparentOnly  | Governance کو `shielded_supply == 0` ثابت کرنا ہوگا یا signed `EmergencyUnshield` plan stage کرنا ہوگا (auditor signatures درکار)۔ | Runtime `effective_height` سے پہلے `Convertible` window کھولتا ہے؛ اس height پر confidential instructions hard-fail ہو جاتی ہیں اور asset transparent-only mode میں واپس جاتا ہے۔ | last-resort exit۔ اگر window کے دوران کوئی confidential note spend ہو تو transition auto-cancel ہو جاتا ہے۔ |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` pending change clear کرتا ہے۔                                                        | `pending_transition` فوراً remove ہوتا ہے۔                                                                          | status quo برقرار؛ completeness کیلئے شامل۔                                             |

اوپر list نہ ہونے والے transitions governance submission پر reject ہوتے ہیں۔ Runtime scheduled transition apply کرنے سے پہلے prerequisites check کرتا ہے؛ failure asset کو previous mode میں واپس دھکیلتی ہے اور telemetry/block events کے ذریعے `PolicyTransitionPrerequisiteFailed` emit کرتی ہے۔

### Migration sequencing

2. **Stage the transition:** `policy_transition_delay_blocks` کو respect کرنے والے `effective_height` کے ساتھ `ScheduleConfidentialPolicyTransition` submit کریں۔ `ShieldedOnly` کی طرف جاتے وقت conversion window specify کریں (`window ≥ policy_transition_window_blocks`)۔
3. **Publish operator guidance:** واپس ملنے والا `transition_id` record کریں اور on/off-ramp runbook circulate کریں۔ Wallets اور auditors `/v1/confidential/assets/{id}/transitions` subscribe کر کے window open height جانتے ہیں۔
4. **Window enforcement:** window کھلتے ہی runtime policy کو `Convertible` پر switch کرتا ہے، `PolicyTransitionWindowOpened { transition_id }` emit کرتا ہے، اور conflicting governance requests reject کرنا شروع کرتا ہے۔
5. **Finalize or abort:** `effective_height` پر runtime transition prerequisites verify کرتا ہے (transparent supply صفر، emergency withdrawal نہیں وغیرہ)۔ Success policy کو requested mode پر flip کرتا ہے؛ failure `PolicyTransitionPrerequisiteFailed` emit کر کے pending transition clear کرتا ہے اور policy unchanged رہتی ہے۔
6. **Schema upgrades:** کامیاب transition کے بعد governance asset schema version bump کرتی ہے (مثلاً `asset_definition.v2`) اور CLI tooling manifest serialize کرتے وقت `confidential_policy` require کرتا ہے۔ Genesis upgrade docs operators کو validators restart سے پہلے policy settings اور registry fingerprints add کرنے کی ہدایت دیتے ہیں۔

Confidentiality enabled کے ساتھ شروع ہونے والی نئی networks desired policy کو genesis میں encode کرتی ہیں۔ پھر بھی launch کے بعد mode change کرتے وقت اوپر والی checklist follow کی جاتی ہے تاکہ conversion windows deterministic رہیں اور wallets کے پاس adjust کرنے کا وقت ہو۔

### Norito manifest versioning & activation

- Genesis manifests MUST `confidential_registry_root` custom key کیلئے `SetParameter` include کریں۔ Payload Norito JSON ہے جو `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` سے match کرتا ہے: جب کوئی verifier entry فعال نہ ہو تو field omit کریں (`null`)، ورنہ 32-byte hex string (`0x…`) دیں جو manifest میں موجود verifier instructions پر `compute_vk_set_hash` کے hash کے برابر ہو۔ Parameter missing یا hash mismatch ہونے پر nodes start سے انکار کرتے ہیں۔
- On-wire `ConfidentialFeatureDigest::conf_rules_version` manifest layout version embed کرتا ہے۔ v1 networks کیلئے اسے `Some(1)` رہنا MUST ہے اور یہ `iroha_config::parameters::defaults::confidential::RULES_VERSION` کے برابر ہے۔ Ruleset evolve ہو تو constant bump کریں، manifests regenerate کریں، اور binaries کو lock-step میں roll out کریں؛ versions mix کرنے سے validators `ConfidentialFeatureDigestMismatch` کے ساتھ blocks reject کرتے ہیں۔
- Activation manifests کو registry updates، parameter lifecycle changes، اور policy transitions bundle کرنا SHOULD ہے تاکہ digest consistent رہے:
  1. Planned registry mutations (`Publish*`, `Set*Lifecycle`) کو offline state view میں apply کریں اور `compute_confidential_feature_digest` سے post-activation digest compute کریں۔
  2. Computed hash کے ساتھ `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` emit کریں تاکہ lagging peers intermediate registry instructions miss کرنے کے باوجود correct digest recover کر سکیں۔
  3. `ScheduleConfidentialPolicyTransition` instructions append کریں۔ ہر instruction کو governance-issued `transition_id` quote کرنا لازم ہے؛ اسے بھولنے والے manifests runtime reject کرتا ہے۔
  4. Manifest bytes، SHA-256 fingerprint اور activation plan میں استعمال ہونے والا digest محفوظ کریں۔ Operators تینوں artefacts verify کر کے ہی vote دیتے ہیں تاکہ partitions سے بچا جا سکے۔
- جب rollout کیلئے deferred cut-over درکار ہو، target height کو companion custom parameter میں record کریں (مثلاً `custom.confidential_upgrade_activation_height`)۔ یہ auditors کو Norito-encoded proof دیتا ہے کہ validators نے digest change سے پہلے notice window honor کیا۔

## Verifier & Parameter Lifecycle
### ZK Registry
- Ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` اسٹور کرتا ہے جہاں `proving_system` فی الحال `Halo2` پر fixed ہے۔
- `(circuit_id, version)` pairs globally unique ہیں؛ registry circuit metadata کے lookup کیلئے secondary index رکھتی ہے۔ Duplicate pair رجسٹر کرنے کی کوشش admission پر reject ہوتی ہے۔
- `circuit_id` non-empty ہونا چاہئے اور `public_inputs_schema_hash` فراہم کرنا لازم ہے (عام طور پر verifier کے canonical public-input encoding کا Blake2b-32 hash)۔ Admission ان فیلڈز کے بغیر records reject کرتا ہے۔
- Governance instructions میں شامل ہیں:
  - `PUBLISH`، metadata-only `Proposed` entry add کرنے کیلئے۔
  - `ACTIVATE { vk_id, activation_height }`، epoch boundary پر activation schedule کرنے کیلئے۔
  - `DEPRECATE { vk_id, deprecation_height }`، آخری height mark کرنے کیلئے جہاں proofs entry کو reference کر سکیں۔
  - `WITHDRAW { vk_id, withdraw_height }`، emergency shutdown کیلئے؛ متاثرہ assets withdraw height کے بعد confidential spending freeze کرتے ہیں جب تک نئی entries activate نہ ہوں۔
- Genesis manifests خودکار طور پر `confidential_registry_root` custom parameter emit کرتے ہیں جس کا `vk_set_hash` active entries سے match کرتا ہے؛ validation node کے consensus join سے پہلے digest کو local registry state کے خلاف cross-check کرتی ہے۔
- Verifier register یا update کیلئے `gas_schedule_id` ضروری ہے؛ verification enforce کرتی ہے کہ registry entry `Active` ہو، `(circuit_id, version)` index میں ہو، اور Halo2 proofs میں `OpenVerifyEnvelope` ہو جس کا `circuit_id`, `vk_hash`, اور `public_inputs_schema_hash` registry record سے match کرے۔

### Proving Keys
- Proving keys off-ledger رہتے ہیں مگر content-addressed identifiers (`pk_cid`, `pk_hash`, `pk_len`) کے ذریعے refer کیے جاتے ہیں جو verifier metadata کے ساتھ publish ہوتے ہیں۔
- Wallet SDKs PK data fetch کر کے hashes verify کرتے ہیں اور local cache میں رکھتے ہیں۔

### Pedersen & Poseidon Parameters
- الگ registries (`PedersenParams`, `PoseidonParams`) verifier lifecycle controls mirror کرتی ہیں، ہر ایک میں `params_id`, generators/constants کے hashes، activation/deprecation/withdraw heights ہوتے ہیں۔

## Deterministic Ordering & Nullifiers
- ہر asset `CommitmentTree` رکھتا ہے جس میں `next_leaf_index` ہوتا ہے؛ blocks commitments کو deterministic order میں append کرتے ہیں: block order میں transactions iterate کریں؛ ہر transaction کے اندر serialized `output_idx` ascending میں shielded outputs iterate کریں۔
- `note_position` tree offsets سے derive ہوتا ہے مگر nullifier کا حصہ **نہیں**؛ یہ صرف proof witness میں membership paths کے لئے استعمال ہوتا ہے۔
- Reorgs کے تحت nullifier stability PRF design سے guarantee ہوتی ہے؛ PRF input `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` bind کرتا ہے، اور anchors تاریخی Merkle roots کو reference کرتے ہیں جو `max_anchor_age_blocks` سے محدود ہیں۔

## Ledger Flow
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - `Convertible` یا `ShieldedOnly` asset policy درکار؛ admission asset authority check کرتا ہے، current `params_id` retrieve کرتا ہے، `rho` sample کرتا ہے، commitment emit کرتا ہے، Merkle tree update کرتا ہے۔
   - `ConfidentialEvent::Shielded` emit کرتا ہے جس میں new commitment، Merkle root delta اور audit trail کیلئے transaction call hash ہوتا ہے۔
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - VM syscall registry entry سے proof verify کرتا ہے؛ host یقینی بناتا ہے کہ nullifiers unused ہوں، commitments deterministic طور پر append ہوں، اور anchor recent ہو۔
   - Ledger `NullifierSet` entries record کرتا ہے، recipients/auditors کیلئے encrypted payloads store کرتا ہے، اور `ConfidentialEvent::Transferred` emit کرتا ہے جو nullifiers، ordered outputs، proof hash، اور Merkle roots summarize کرتا ہے۔
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - صرف `Convertible` assets کیلئے؛ proof validate کرتا ہے کہ note value revealed amount کے برابر ہے، ledger transparent balance credit کرتا ہے، اور nullifier spent mark کر کے shielded note burn کرتا ہے۔
   - `ConfidentialEvent::Unshielded` emit کرتا ہے جس میں public amount، consumed nullifiers، proof identifiers اور transaction call hash شامل ہیں۔

## Data Model Additions
- `ConfidentialConfig` (new config section) with enablement flag, `assume_valid`, gas/limit knobs, anchor window, verifier backend.
- `ConfidentialNote`, `ConfidentialTransfer`, اور `ConfidentialMint` Norito schemas explicit version byte (`CONFIDENTIAL_ASSET_V1 = 0x01`) کے ساتھ۔
- `ConfidentialEncryptedPayload` AEAD memo bytes کو `{ version, ephemeral_pubkey, nonce, ciphertext }` میں wrap کرتا ہے، default `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` XChaCha20-Poly1305 layout کیلئے۔
- Canonical key-derivation vectors `docs/source/confidential_key_vectors.json` میں ہیں؛ CLI اور Torii endpoint ان fixtures کے خلاف regress کرتے ہیں۔
- `asset::AssetDefinition` کو `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ملتا ہے۔
- `ZkAssetState` transfer/unshield verifiers کیلئے `(backend, name, commitment)` binding persist کرتا ہے؛ execution ان proofs کو reject کرتا ہے جن کا referenced یا inline verifying key registered commitment سے match نہ کرے۔
- `CommitmentTree` (per asset with frontier checkpoints), `NullifierSet` keyed by `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` world state میں store ہوتے ہیں۔
- Mempool early duplicate detection اور anchor age checks کیلئے transient `NullifierIndex` اور `AnchorIndex` structures maintain کرتا ہے۔
- Norito schema updates میں public inputs کی canonical ordering شامل ہے؛ round-trip tests encoding determinism ensure کرتے ہیں۔
- Encrypted payload roundtrips unit tests (`crates/iroha_data_model/src/confidential.rs`) کے ذریعے lock ہیں۔ Follow-up wallet vectors auditors کیلئے canonical AEAD transcripts attach کریں گے۔ `norito.md` envelope کیلئے on-wire header document کرتا ہے۔

## IVM Integration & Syscall
- `VERIFY_CONFIDENTIAL_PROOF` syscall introduce کریں جو قبول کرتا ہے:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` اور resultant `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall registry سے verifier metadata load کرتا ہے، size/time limits enforce کرتا ہے، deterministic gas charge کرتا ہے، اور proof success پر ہی delta apply کرتا ہے۔
- Host read-only `ConfidentialLedger` trait expose کرتا ہے جو Merkle root snapshots اور nullifier status retrieve کرتا ہے؛ Kotodama library witness assembly helpers اور schema validation فراہم کرتی ہے۔
- Pointer-ABI docs proof buffer layout اور registry handles واضح کرنے کیلئے update ہوئے ہیں۔

## Node Capability Negotiation
- Handshake `feature_bits.confidential` کے ساتھ `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` advertise کرتا ہے۔ Validator participation کیلئے `confidential.enabled=true`, `assume_valid=false`, identical verifier backend identifiers اور matching digests ضروری ہیں؛ mismatches `HandshakeConfidentialMismatch` کے ساتھ handshake fail کرتے ہیں۔
- Config صرف observer nodes کیلئے `assume_valid` support کرتا ہے: disabled ہونے پر confidential instructions پر deterministic `UnsupportedInstruction` آتا ہے بغیر panic؛ enabled ہونے پر observers proofs verify کئے بغیر state deltas apply کرتے ہیں۔
- Mempool confidential transactions reject کرتا ہے اگر local capability disabled ہو۔ Gossip filters peers without matching capabilities کو shielded transactions بھیجنے سے گریز کرتے ہیں جبکہ unknown verifier IDs کو size limits کے اندر blind-forward کرتے ہیں۔

### Reveal Pruning & Nullifier Retention Policy

Confidential ledgers کو note freshness ثابت کرنے اور governance-driven audits replay کرنے کیلئے کافی history retain کرنی ہوتی ہے۔ `ConfidentialLedger` کی default policy یہ ہے:

- **Nullifier retention:** spent nullifiers کم از کم `730` دن (24 مہینے) spend height کے بعد رکھیں، یا اگر regulator زیادہ مدت مانگے تو وہ۔ Operators `confidential.retention.nullifier_days` کے ذریعے window extend کر سکتے ہیں۔ retention window کے اندر nullifiers Torii کے ذریعے queryable رہیں تاکہ auditors double-spend absence ثابت کر سکیں۔
- **Reveal pruning:** transparent reveals (`RevealConfidential`) متعلقہ note commitments کو block finalisation کے فوراً بعد prune کرتے ہیں، مگر consumed nullifier اوپر والی retention rule کے تابع رہتا ہے۔ Reveal-related events (`ConfidentialEvent::Unshielded`) public amount، recipient اور proof hash record کرتے ہیں تاکہ historical reveals reconstruct کرنے کیلئے pruned ciphertext کی ضرورت نہ ہو۔
- **Frontier checkpoints:** commitment frontiers rolling checkpoints رکھتے ہیں جو `max_anchor_age_blocks` اور retention window میں سے بڑے کو cover کرتے ہیں۔ Nodes پرانے checkpoints صرف تب compact کرتے ہیں جب interval کے تمام nullifiers expire ہو جائیں۔
- **Stale digest remediation:** اگر digest drift کی وجہ سے `HandshakeConfidentialMismatch` آئے تو operators کو (1) cluster میں nullifier retention windows align ہونے کی تصدیق کرنی چاہئے، (2) `iroha_cli app confidential verify-ledger` چلا کر retained nullifier set کے خلاف digest regenerate کرنا چاہئے، اور (3) refreshed manifest redeploy کرنا چاہئے۔ Prematurely pruned nullifiers کو network join سے پہلے cold storage سے restore کرنا ہوگا۔

Local overrides کو operations runbook میں document کریں؛ retention window بڑھانے والی governance policies کو node configuration اور archival storage plans کے ساتھ lockstep میں update کرنا ضروری ہے۔

### Eviction & Recovery Flow

1. Dial کے دوران `IrohaNetwork` advertised capabilities compare کرتا ہے۔ mismatch `HandshakeConfidentialMismatch` raise کرتا ہے؛ connection بند ہوتی ہے اور peer discovery queue میں رہتا ہے بغیر `Ready` بنے۔
2. Failure network service log میں surface ہوتی ہے (remote digest اور backend سمیت)، اور Sumeragi peer کو proposal یا voting کیلئے schedule نہیں کرتا۔
3. Operators verifier registries اور parameter sets (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) align کر کے یا agreed `activation_height` کے ساتھ `next_conf_features` stage کر کے remediation کرتے ہیں۔ Digest match ہوتے ہی اگلا handshake خودکار طور پر succeed کرتا ہے۔
4. اگر stale peer کوئی block broadcast کر دے (مثلاً archival replay کے ذریعے)، validators اسے `BlockRejectionReason::ConfidentialFeatureDigestMismatch` کے ساتھ deterministic طور پر reject کرتے ہیں، جس سے network میں ledger state consistent رہتا ہے۔

### Replay-safe handshake flow

1. ہر outbound attempt fresh Noise/X25519 key material allocate کرتا ہے۔ Signed handshake payload (`handshake_signature_payload`) local اور remote ephemeral public keys، Norito-encoded advertised socket address، اور `handshake_chain_id` کے ساتھ compile ہونے پر chain identifier concatenate کرتا ہے۔ Message node سے نکلنے سے پہلے AEAD-encrypted ہوتی ہے۔
2. Responder peer/local key order کو reverse کر کے payload recompute کرتا ہے اور `HandshakeHelloV1` میں embedded Ed25519 signature verify کرتا ہے۔ چونکہ دونوں ephemeral keys اور advertised address signature domain میں ہیں، captured message کو کسی دوسرے peer کے خلاف replay کرنا یا stale connection recover کرنا deterministic طور پر fail ہوتا ہے۔
3. Confidential capability flags اور `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` کے اندر travel کرتے ہیں۔ Receiver `{ enabled, assume_valid, verifier_backend, digest }` tuple کو local `ConfidentialHandshakeCaps` کے ساتھ compare کرتا ہے؛ mismatch ہونے پر `HandshakeConfidentialMismatch` کے ساتھ early exit ہوتا ہے۔
4. Operators کو digest ( `compute_confidential_feature_digest` کے ذریعے) recompute کرنا اور updated registries/policies کے ساتھ nodes restart کرنا MUST ہے۔ Old digests advertise کرنے والے peers handshake fail کرتے رہیں گے اور stale state کو validator set میں واپس آنے سے روکیں گے۔
5. Handshake successes/failures standard `iroha_p2p::peer` counters (`handshake_failure_count` وغیرہ) update کرتے ہیں اور structured log entries emit کرتے ہیں جن میں remote peer ID اور digest fingerprint tags ہوتے ہیں۔ ان indicators کو monitor کریں تاکہ rollout کے دوران replay attempts یا misconfigurations پکڑی جا سکیں۔

## Key Management & Payloads
- Per-account key derivation hierarchy:
  - `sk_spend` → `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Encrypted note payloads AEAD استعمال کرتے ہیں جو ECDH-derived shared keys سے بنے ہوتے ہیں؛ optional auditor view keys asset policy کے مطابق outputs کے ساتھ attach کئے جا سکتے ہیں۔
- CLI additions: `confidential create-keys`, `confidential send`, `confidential export-view-key`, memos decrypt کرنے کیلئے auditor tooling، اور `iroha app zk envelope` helper جو Norito memo envelopes offline produce/inspect کرتا ہے۔ Torii `POST /v1/confidential/derive-keyset` کے ذریعے وہی derivation flow فراہم کرتا ہے اور hex/base64 دونوں forms واپس دیتا ہے تاکہ wallets programmatically key hierarchies fetch کر سکیں۔

## Gas, Limits & DoS Controls
- Deterministic gas schedule:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas per public input.
  - `5` gas per proof byte، plus per-nullifier (`300`) اور per-commitment (`500`) charges۔
  - Operators یہ constants node config (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) کے ذریعے override کر سکتے ہیں؛ changes startup یا config hot-reload پر propagate ہو کر cluster میں deterministic طور پر apply ہوتے ہیں۔
- Hard limits (configurable defaults):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms` سے زیادہ proofs instruction کو deterministic طور پر abort کرتے ہیں (governance ballots `proof verification exceeded timeout` emit کرتے ہیں، `VerifyProof` error return کرتا ہے)۔
- Additional quotas liveness ensure کرتے ہیں: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, اور `max_public_inputs` block builders کو bound کرتے ہیں؛ `reorg_depth_bound` (≥ `max_anchor_age_blocks`) frontier checkpoint retention govern کرتا ہے۔
- Runtime اب per-transaction یا per-block limits exceed کرنے والی transactions reject کرتا ہے، deterministic `InvalidParameter` errors emit کرتا ہے اور ledger state unchanged رہتی ہے۔
- Mempool `vk_id`, proof length، اور anchor age کے ذریعے confidential transactions کو prefilter کرتا ہے، verifier invoke کرنے سے پہلے resource usage bounded رکھتا ہے۔
- Verification deterministic طور پر timeout یا bound violation پر halt ہوتی ہے؛ transactions explicit errors کے ساتھ fail ہوتی ہیں۔ SIMD backends optional ہیں مگر gas accounting alter نہیں کرتے۔

### Calibration Baselines & Acceptance Gates
- **Reference platforms.** Calibration runs کو نیچے دیئے گئے تین hardware profiles cover کرنا MUST ہے۔ اگر سب profiles capture نہ ہوں تو review reject کر دے۔

  | Profile | Architecture | CPU / Instance | Compiler flags | Purpose |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) یا Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | vector intrinsics کے بغیر floor values establish کرتا ہے؛ fallback cost tables tune کرنے کیلئے استعمال ہوتا ہے۔ |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | AVX2 path validate کرتا ہے؛ چیک کرتا ہے کہ SIMD speedups neutral gas tolerance کے اندر رہیں۔ |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | NEON backend کو deterministic اور x86 schedules کے ساتھ aligned رکھتا ہے۔ |

- **Benchmark harness.** تمام gas calibration reports MUST ہوں:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` deterministic fixture confirm کرنے کیلئے۔
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` جب بھی VM opcode costs بدلیں۔

- **Fixed randomness.** benches چلانے سے پہلے `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` export کریں تاکہ `iroha_test_samples::gen_account_in` deterministic `KeyPair::from_seed` path پر switch ہو۔ Harness ایک بار `IROHA_CONF_GAS_SEED_ACTIVE=…` print کرتا ہے؛ variable missing ہو تو review MUST fail ہو۔ نئی calibration utilities بھی auxiliary randomness introduce کرتے وقت اس env var کو honor کریں۔

- **Result capture.**
  - Criterion summaries (`target/criterion/**/raw.csv`) ہر profile کیلئے release artefact میں upload کریں۔
  - Derived metrics (`ns/op`, `gas/op`, `ns/gas`) کو [Confidential Gas Calibration ledger](./confidential-gas-calibration) میں git commit اور compiler version کے ساتھ store کریں۔
  - ہر profile کیلئے آخری دو baselines برقرار رکھیں؛ نئی report validate ہونے کے بعد پرانی snapshots delete کریں۔

- **Acceptance tolerances.**
  - `baseline-simd-neutral` اور `baseline-avx2` کے درمیان gas deltas ≤ ±1.5% رہیں۔
  - `baseline-simd-neutral` اور `baseline-neon` کے درمیان gas deltas ≤ ±2.0% رہیں۔
  - ان thresholds سے تجاوز کرنے والی calibration proposals کو schedule adjustments یا discrepancy/mitigation وضاحت کرنے والا RFC درکار ہوگا۔

- **Review checklist.** Submitters ذمہ دار ہیں:
  - `uname -a`, `/proc/cpuinfo` excerpts (model, stepping)، اور `rustc -Vv` calibration log میں شامل کریں۔
  - bench output میں `IROHA_CONF_GAS_SEED` کے echo ہونے کی تصدیق کریں (benches active seed print کرتے ہیں)۔
  - pacemaker اور confidential verifier feature flags production سے match ہوں (`--features confidential,telemetry` جب Telemetry کے ساتھ benches چلائیں)۔

## Config & Operations
- `iroha_config` میں `[confidential]` سیکشن شامل:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetry aggregate metrics emit کرتا ہے: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, اور `confidential_policy_transitions_total`، plaintext data expose کئے بغیر۔
- RPC surfaces:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Testing Strategy
- Determinism: blocks کے اندر randomized transaction shuffling identical Merkle roots اور nullifier sets yield کرتا ہے۔
- Reorg resilience: anchors کے ساتھ multi-block reorgs simulate کریں؛ nullifiers stable رہتے ہیں اور stale anchors reject ہوتے ہیں۔
- Gas invariants: SIMD acceleration کے ساتھ اور بغیر nodes پر identical gas usage verify کریں۔
- Boundary testing: size/gas ceilings پر proofs، max in/out counts، timeout enforcement۔
- Lifecycle: verifier اور parameter activation/deprecation کیلئے governance operations، rotation spend tests۔
- Policy FSM: allowed/disallowed transitions، pending transition delays، effective heights کے گرد mempool rejection۔
- Registry emergencies: emergency withdrawal `withdraw_height` پر affected assets freeze کرتا ہے اور اس کے بعد proofs reject کرتا ہے۔
- Capability gating: mismatched `conf_features` والے validators blocks reject کرتے ہیں؛ `assume_valid=true` والے observers consensus کو متاثر کئے بغیر sync رہتے ہیں۔
- State equivalence: validator/full/observer nodes canonical chain پر identical state roots produce کرتے ہیں۔
- Negative fuzzing: malformed proofs، oversized payloads، اور nullifier collisions deterministic طور پر reject ہوتے ہیں۔

## Outstanding Work
- Halo2 parameter sets (circuit size, lookup strategy) benchmark کریں اور نتائج calibration playbook میں ریکارڈ کریں تاکہ اگلی `confidential_assets_calibration.md` refresh کے ساتھ gas/timeout defaults update ہوں۔
- Auditor disclosure policies اور متعلقہ selective-viewing APIs finalize کریں، اور governance draft sign-off کے بعد approved workflow کو Torii میں wire کریں۔
- Witness encryption scheme کو multi-recipient outputs اور batched memos تک extend کریں، SDK implementers کیلئے envelope format document کریں۔
- Circuits، registries، اور parameter-rotation procedures کی external security review commission کریں اور findings کو internal audit reports کے ساتھ archive کریں۔
- Auditor spentness reconciliation APIs specify کریں اور view-key scope guidance publish کریں تاکہ wallet vendors ایک جیسے attestation semantics implement کر سکیں۔

## Implementation Phasing
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ Nullifier derivation اب Poseidon PRF design (`nk`, `rho`, `asset_id`, `chain_id`) follow کرتا ہے اور ledger updates میں deterministic commitment ordering enforce ہوتی ہے۔
   - ✅ Execution proof size caps اور per-transaction/per-block confidential quotas enforce کرتا ہے، over-budget transactions کو deterministic errors کے ساتھ reject کرتا ہے۔
   - ✅ P2P handshake `ConfidentialFeatureDigest` (backend digest + registry fingerprints) advertise کرتا ہے اور mismatches کو `HandshakeConfidentialMismatch` کے ذریعے deterministic fail کرتا ہے۔
   - ✅ Confidential execution paths میں panics remove کئے گئے اور unsupported nodes کیلئے role gating add کیا گیا۔
   - ⚪ Verifier timeout budgets اور frontier checkpoints کیلئے reorg depth bounds enforce کرنا۔
     - ✅ Verification timeout budgets enforce ہوئے؛ `verify_timeout_ms` سے تجاوز کرنے والی proofs اب deterministic fail ہوتی ہیں۔
     - ✅ Frontier checkpoints اب `reorg_depth_bound` respect کرتے ہیں، configured window سے پرانے checkpoints prune کرتے ہوئے deterministic snapshots برقرار رکھتے ہیں۔
   - `AssetConfidentialPolicy`, policy FSM، اور mint/transfer/reveal instructions کیلئے enforcement gates introduce کریں۔
   - Block headers میں `conf_features` commit کریں اور registry/parameter digests diverge ہونے پر validator participation refuse کریں۔
2. **Phase M1 — Registries & Parameters**
   - `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` registries governance ops، genesis anchoring اور cache management کے ساتھ land کریں۔
   - Syscall کو registry lookups، gas schedule IDs، schema hashing، اور size checks require کرنے کیلئے wire کریں۔
   - Encrypted payload format v1، wallet key derivation vectors، اور confidential key management کیلئے CLI support ship کریں۔
3. **Phase M2 — Gas & Performance**
   - Deterministic gas schedule، per-block counters، اور telemetry کے ساتھ benchmark harnesses implement کریں (verify latency، proof sizes، mempool rejections)۔
   - CommitmentTree checkpoints، LRU loading، اور nullifier indices کو multi-asset workloads کیلئے harden کریں۔
4. **Phase M3 — Rotation & Wallet Tooling**
   - Multi-parameter اور multi-version proof acceptance enable کریں؛ governance-driven activation/deprecation کو transition runbooks کے ساتھ support کریں۔
   - Wallet SDK/CLI migration flows، auditor scanning workflows، اور spentness reconciliation tooling deliver کریں۔
5. **Phase M4 — Audit & Ops**
   - Auditor key workflows، selective disclosure APIs، اور operational runbooks فراہم کریں۔
   - External cryptography/security review schedule کریں اور findings `status.md` میں publish کریں۔

ہر phase roadmap milestones اور متعلقہ tests update کرتی ہے تاکہ blockchain network کیلئے deterministic execution guarantees برقرار رہیں۔
