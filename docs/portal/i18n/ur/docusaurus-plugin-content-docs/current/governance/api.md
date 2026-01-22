---
id: api
lang: ur
direction: rtl
source: docs/portal/docs/governance/api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

حالت: گورننس نفاذی کاموں کے ساتھ چلنے والا ڈرافٹ/اسکیچ۔ عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔ Determinism اور RBAC پالیسی معیاری پابندیاں ہیں؛ جب `authority` اور `private_key` فراہم ہوں تو Torii ٹرانزیکشن سائن/سبمٹ کر سکتا ہے، ورنہ کلائنٹس بنا کر `/transaction` پر سبمٹ کرتے ہیں۔

جائزہ
- تمام endpoints JSON واپس کرتے ہیں۔ ٹرانزیکشن بنانے والے فلو کے لئے جوابات میں `tx_instructions` شامل ہوتے ہیں - ایک یا زیادہ instruction skeletons کی array:
  - `wire_id`: instruction ٹائپ کا registry identifier
  - `payload_hex`: Norito payload bytes (hex)
- اگر `authority` اور `private_key` (یا ballot DTOs میں `private_key`) فراہم ہوں تو Torii ٹرانزیکشن سائن اور سبمٹ کرتا ہے اور پھر بھی `tx_instructions` واپس کرتا ہے۔
- ورنہ کلائنٹس اپنی authority اور chain_id کے ساتھ SignedTransaction بناتے ہیں، پھر سائن کر کے `/transaction` پر POST کرتے ہیں۔
- SDK coverage:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` واپس کرتا ہے (status/kind fields کو normalize کرتا ہے)، `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے، `ToriiClient.get_governance_tally_typed` `GovernanceTally` واپس کرتا ہے، `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` واپس کرتا ہے، `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` واپس کرتا ہے، اور `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage` واپس کرتا ہے، جس سے پورے governance surface پر typed access ملتا ہے اور README میں usage examples دیے گئے ہیں۔
- Python lightweight client (`iroha_torii_client`): `ToriiClient.finalize_referendum` اور `ToriiClient.enact_proposal` typed `GovernanceInstructionDraft` bundles واپس کرتے ہیں (Torii کی `tx_instructions` skeleton کو wrap کرتے ہوئے)، تاکہ scripts Finalize/Enact flows بناتے وقت manual JSON parsing سے بچ سکیں۔
- JavaScript (`@iroha/iroha-js`): `ToriiClient` proposals, referenda, tallies, locks, unlock stats کے لئے typed helpers دیتا ہے، اور اب `listGovernanceInstances(namespace, options)` کے ساتھ council endpoints (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) بھی دیتا ہے تاکہ Node.js clients `/v1/gov/instances/{ns}` کو paginate کر سکیں اور VRF-backed workflows کو موجودہ contract instance listing کے ساتھ چلا سکیں۔

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Request (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "ih58…@wonderland?",
      "private_key": "...?"
    }
  - Response (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validation: nodes فراہم کردہ `abi_version` کے لئے `abi_hash` کو canonicalize کرتے ہیں اور mismatch پر reject کرتے ہیں۔ `abi_version = "v1"` کے لئے متوقع value `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ہے۔

Contracts API (deploy)
- POST `/v1/contracts/deploy`
  - Request: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - Behavior: IVM پروگرام باڈی سے `code_hash` اور header `abi_version` سے `abi_hash` نکالتا ہے، پھر `RegisterSmartContractCode` (manifest) اور `RegisterSmartContractBytes` (مکمل `.to` bytes) `authority` کی طرف سے سبمٹ کرتا ہے۔
  - Response: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Related:
    - GET `/v1/contracts/code/{code_hash}` -> ذخیرہ شدہ manifest واپس کرتا ہے
    - GET `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` واپس کرتا ہے
- POST `/v1/contracts/instance`
  - Request: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Behavior: فراہم کردہ bytecode deploy کرتا ہے اور `ActivateContractInstance` کے ذریعے `(namespace, contract_id)` mapping فوراً فعال کرتا ہے۔
  - Response: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Alias Service
- POST `/v1/aliases/voprf/evaluate`
  - Request: { "blinded_element_hex": "..." }
  - Response: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` evaluator implementation کو ظاہر کرتا ہے۔ موجودہ value: `blake2b512-mock`۔
  - Notes: deterministic mock evaluator جو Blake2b512 کو domain separation `iroha.alias.voprf.mock.v1` کے ساتھ apply کرتا ہے۔ یہ test tooling کے لئے ہے جب تک production VOPRF pipeline Iroha میں wire نہ ہو جائے۔
  - Errors: malformed hex input پر HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` envelope اور decoder error message واپس کرتا ہے۔
- POST `/v1/aliases/resolve`
  - Request: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Response: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - Notes: ISO bridge runtime staging درکار ہے (`[iso_bridge.account_aliases]` in `iroha_config`)۔ Torii whitespace ہٹا کر اور uppercase بنا کر lookup کرتا ہے۔ alias نہ ہو تو 404 اور ISO bridge runtime بند ہو تو 503 دیتا ہے۔
- POST `/v1/aliases/resolve_index`
  - Request: { "index": 0 }
  - Response: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - Notes: alias indices configuration order کے مطابق deterministic طریقے سے assign ہوتے ہیں (0-based)۔ کلائنٹس offline cache کر کے alias attestation events کے audit trails بنا سکتے ہیں۔

Code Size Cap
- Custom parameter: `max_contract_code_bytes` (JSON u64)
  - On-chain contract code storage کے لئے زیادہ سے زیادہ سائز (bytes میں) کنٹرول کرتا ہے۔
  - Default: 16 MiB۔ جب `.to` image حد سے بڑی ہو تو nodes `RegisterSmartContractBytes` کو invariant violation error کے ساتھ reject کرتے ہیں۔
  - Operators `SetParameter(Custom)` کے ذریعے `id = "max_contract_code_bytes"` اور numeric payload دے کر ایڈجسٹ کر سکتے ہیں۔

- POST `/v1/gov/ballots/zk`
  - Request: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes:
    - جب circuit کے public inputs میں `owner`, `amount`, `duration_blocks` شامل ہوں اور proof configured VK کے خلاف verify ہو جائے تو node `election_id` کے لئے governance lock بناتا یا بڑھاتا ہے۔ direction چھپی رہتی ہے (`unknown`); صرف amount/expiry اپڈیٹ ہوتے ہیں۔ re-votes monotonic ہیں: amount اور expiry صرف بڑھتے ہیں (node max(amount, prev.amount) اور max(expiry, prev.expiry) لگاتا ہے)۔
    - ZK re-votes جو amount یا expiry کم کرنے کی کوشش کریں server-side `BallotRejected` diagnostics کے ساتھ reject ہوتے ہیں۔
    - Contract execution کو `SubmitBallot` enqueue کرنے سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا لازم ہے؛ hosts one-shot latch enforce کرتے ہیں۔

- POST `/v1/gov/ballots/plain`
  - Request: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes: re-votes extend-only ہیں - نیا ballot موجودہ lock کا amount یا expiry کم نہیں کر سکتا۔ `owner` کو transaction authority کے برابر ہونا چاہئے۔ کم از کم مدت `conviction_step_blocks` ہے۔

- POST `/v1/gov/finalize`
  - Request: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…@wonderland?", "private_key": "...?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - On-chain effect (current scaffold): منظور شدہ deploy proposal کو enact کرنے سے `code_hash` keyed minimal `ContractManifest` شامل ہوتا ہے جس میں متوقع `abi_hash` ہوتا ہے اور proposal Enacted ہو جاتا ہے۔ اگر `code_hash` کے لئے مختلف `abi_hash` والا manifest پہلے سے ہو تو enactment reject ہوتا ہے۔
  - Notes:
    - ZK elections کے لئے contract paths کو `FinalizeElection` سے پہلے `ZK_VOTE_VERIFY_TALLY` کال کرنا لازمی ہے؛ hosts one-shot latch enforce کرتے ہیں۔ `FinalizeReferendum` ZK referenda کو اس وقت تک reject کرتا ہے جب تک election tally finalized نہ ہو جائے۔
    - Auto-close `h_end` پر صرف Plain referenda کے لئے Approved/Rejected emit کرتا ہے؛ ZK referenda Closed رہتے ہیں جب تک finalized tally submit نہ ہو اور `FinalizeReferendum` execute نہ ہو۔
    - Turnout checks صرف approve+reject استعمال کرتی ہیں؛ abstain turnout میں شمار نہیں ہوتا۔

- POST `/v1/gov/enact`
  - Request: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…@wonderland?", "private_key": "...?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes: جب `authority`/`private_key` فراہم ہوں تو Torii signed transaction سبمٹ کرتا ہے؛ ورنہ وہ skeleton واپس کرتا ہے جسے کلائنٹ سائن اور سبمٹ کرے۔ preimage اختیاری اور فی الحال معلوماتی ہے۔

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: proposal id hex (64 chars)
  - Response: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: referendum id string
  - Response: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - Response: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes: موجودہ council موجود ہو تو واپس کرتا ہے، ورنہ configured stake asset اور thresholds استعمال کر کے deterministic fallback derive کرتا ہے (VRF specs کو اس وقت تک reflect کرتا ہے جب تک live VRF proofs on-chain persist نہ ہوں)۔

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Request: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Behavior: ہر امیدوار کا VRF proof `chain_id`, `epoch` اور تازہ ترین block hash beacon سے مشتق canonical input کے خلاف verify کرتا ہے؛ output bytes کو desc ترتیب میں tiebreakers کے ساتھ sort کرتا ہے؛ top `committee_size` members واپس کرتا ہے۔ Persist نہیں کرتا۔
  - Response: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notes: Normal = pk in G1, proof in G2 (96 bytes). Small = pk in G2, proof in G1 (48 bytes). Inputs domain-separated ہیں اور `chain_id` شامل ہے۔

### Governance defaults (iroha_config `gov.*`)

Torii جب کوئی persisted roster نہ پائے تو fallback council `iroha_config` کے ذریعے parameterize کیا جاتا ہے:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Equivalent environment overrides:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` fallback members کی تعداد کو limit کرتا ہے جب council persist نہ ہو، `parliament_term_blocks` epoch length define کرتا ہے جو seed derivation کے لئے استعمال ہوتا ہے (`epoch = floor(height / term_blocks)`)، `parliament_min_stake` eligibility asset پر کم از کم stake (smallest units) enforce کرتا ہے، اور `parliament_eligibility_asset_id` منتخب کرتا ہے کہ candidates بناتے وقت کس asset balance کو scan کیا جائے۔

Governance VK verification کا کوئی bypass نہیں: ballot verification ہمیشہ `Active` verifying key اور inline bytes کا تقاضا کرتا ہے، اور environments کو test-only toggles پر انحصار نہیں کرنا چاہئے۔

RBAC
- On-chain execution کے لئے permissions درکار ہیں:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (future): `CanManageParliament`

Protected Namespaces
- Custom parameter `gov_protected_namespaces` (JSON array of strings) listed namespaces میں deploy کے لئے admission gating فعال کرتا ہے۔
- Clients کو protected namespaces پر deploy کے لئے transaction metadata keys شامل کرنی ہوں گی:
  - `gov_namespace`: target namespace (مثال: "apps")
  - `gov_contract_id`: namespace کے اندر logical contract id
- `gov_manifest_approvers`: optional JSON array of validator account IDs۔ جب lane manifest quorum > 1 declare کرے تو admission کے لئے transaction authority اور listed accounts دونوں درکار ہوتے ہیں تاکہ manifest quorum پورا ہو سکے۔
- Telemetry `governance_manifest_admission_total{result}` کے ذریعے holistic admission counters دکھاتی ہے تاکہ operators کامیاب admits کو `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, اور `runtime_hook_rejected` سے فرق کر سکیں۔
- Telemetry `governance_manifest_quorum_total{outcome}` (values `satisfied` / `rejected`) کے ذریعے enforcement path دکھاتی ہے تاکہ missing approvals کا audit ہو سکے۔
- Lanes اپنے manifests میں شائع شدہ namespace allowlist کو enforce کرتے ہیں۔ جو بھی ٹرانزیکشن `gov_namespace` سیٹ کرے اسے `gov_contract_id` دینا ہوگا، اور namespace کو manifest کے `protected_namespaces` سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` submissions بغیر metadata کے، جب protection enable ہو، reject ہوتے ہیں۔
- Admission اس بات کو enforce کرتا ہے کہ `(namespace, contract_id, code_hash, abi_hash)` کے لئے Enacted governance proposal موجود ہو؛ ورنہ validation NotPermitted error کے ساتھ fail ہوتا ہے۔

Runtime Upgrade Hooks
- Lane manifests `hooks.runtime_upgrade` declare کر سکتے ہیں تاکہ runtime upgrade instructions (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) کو gate کیا جا سکے۔
- Hook fields:
  - `allow` (bool, default `true`): جب `false` ہو تو تمام runtime-upgrade instructions reject ہو جاتے ہیں۔
  - `require_metadata` (bool, default `false`): `metadata_key` کے مطابق metadata entry درکار ہے۔
  - `metadata_key` (string): hook کا enforced metadata name۔ Default `gov_upgrade_id` جب metadata required ہو یا allowlist موجود ہو۔
  - `allowed_ids` (array of strings): metadata values کی optional allowlist (trim کے بعد)۔ اگر دیا گیا value فہرست میں نہ ہو تو reject۔
- Hook موجود ہو تو queue admission ٹرانزیکشن کے queue میں جانے سے پہلے metadata policy enforce کرتا ہے۔ Missing metadata, خالی values، یا allowlist سے باہر values deterministic NotPermitted error دیتی ہیں۔
- Telemetry `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` کے ذریعے outcomes track کرتی ہے۔
- Hook کی تسکین کرنے والی ٹرانزیکشنز کو metadata `gov_upgrade_id=<value>` (یا manifest-defined key) شامل کرنا ہوگا، ساتھ ہی manifest quorum کے مطابق validators کی approvals بھی درکار ہیں۔

Convenience Endpoint
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` کو براہ راست node پر apply کرتا ہے۔
  - Request: { "namespaces": ["apps", "system"] }
  - Response: { "ok": true, "applied": 1 }
  - Notes: admin/testing کے لئے ہے؛ اگر configure ہو تو API token درکار ہوگا۔ production کے لئے `SetParameter(Custom)` کے ساتھ signed transaction ترجیح دیں۔

CLI Helpers
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - namespace کے contract instances fetch کرتا ہے اور cross-check کرتا ہے کہ:
    - Torii ہر `code_hash` کے لئے bytecode ذخیرہ کرتا ہے، اور اس کا Blake2b-32 digest `code_hash` سے match کرتا ہے۔
    - `/v1/contracts/code/{code_hash}` میں موجود manifest matching `code_hash` اور `abi_hash` values رپورٹ کرتا ہے۔
    - `(namespace, contract_id, code_hash, abi_hash)` کے لئے enacted governance proposal موجود ہے جو اسی proposal-id hashing سے derive ہوتا ہے جو node استعمال کرتا ہے۔
  - `results[]` کے ساتھ JSON رپورٹ دیتا ہے (issues, manifest/code/proposal summaries) اور ایک لائن کا خلاصہ (اگر `--no-summary` نہ ہو)۔
  - Protected namespaces کے audit یا governance-controlled deploy workflows کی تصدیق کے لئے مفید۔
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - Protected namespaces میں deployment کے لئے JSON metadata skeleton دیتا ہے، جس میں optional `gov_manifest_approvers` شامل ہیں تاکہ manifest quorum rules پوری ہوں۔
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ہونے پر lock hints لازم ہیں، اور فراہم کیے گئے کسی بھی hints سیٹ میں `owner`, `amount` اور `duration_blocks` شامل ہونا ضروری ہے۔
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - ایک لائن خلاصہ اب deterministic `fingerprint=<hex>` دکھاتا ہے جو encoded `CastZkBallot` سے derive ہوتا ہے، ساتھ decoded hints (`owner`, `amount`, `duration_blocks`, `direction` جب فراہم ہوں)۔
  - CLI responses `tx_instructions[]` کو `payload_fingerprint_hex` اور decoded fields کے ساتھ annotate کرتے ہیں تاکہ downstream tooling skeleton کو بغیر Norito decoding کے verify کر سکے۔
  - Lock hints دینے سے node ZK ballots کے لئے `LockCreated`/`LockExtended` events emit کر سکتا ہے جب circuit وہی values expose کرے۔
- `iroha app gov vote --mode plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` aliases ZK flags کے ناموں کو mirror کرتے ہیں تاکہ scripting parity ہو۔
  - Summary output `vote --mode zk` کی طرح encoded instruction fingerprint اور readable ballot fields (`owner`, `amount`, `duration_blocks`, `direction`) شامل کرتا ہے، جس سے signature سے پہلے فوری تصدیق ہو جاتی ہے۔

Instances Listing
- GET `/v1/gov/instances/{ns}` - namespace کے لئے active contract instances کی فہرست۔
  - Query params:
    - `contains`: `contract_id` کی substring کے مطابق filter (case-sensitive)
    - `hash_prefix`: `code_hash_hex` کے hex prefix کے مطابق filter (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Response: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK helper: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) یا `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)۔

Unlock Sweep (Operator/Audit)
- GET `/v1/gov/unlocks/stats`
  - Response: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes: `last_sweep_height` سب سے حالیہ block height دکھاتا ہے جہاں expired locks sweep اور persist کئے گئے۔ `expired_locks_now` ان lock records کو scan کر کے نکلتا ہے جن میں `expiry_height <= height_current` ہو۔
- POST `/v1/gov/ballots/zk-v1`
  - Request (v1-style DTO):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "ih58…@wonderland?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - `BallotProof` JSON براہ راست قبول کر کے `CastZkBallot` skeleton واپس کرتا ہے۔
  - Request:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // ZK1 یا H2* container کا base64
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // optional AccountId جب circuit owner commit کرے
        "nullifier": null                 // optional 32-byte hex string (nullifier hint)
      }
    }
  - Response:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notes:
    - Server optional `root_hint`/`owner`/`nullifier` کو ballot سے `CastZkBallot` کے `public_inputs_json` میں map کرتا ہے۔
    - Envelope bytes کو instruction payload کے لئے base64 میں دوبارہ encode کیا جاتا ہے۔
    - جب Torii ballot submit کرتا ہے تو `reason` بدل کر `submitted transaction` ہو جاتا ہے۔
    - یہ endpoint صرف تب دستیاب ہے جب `zk-ballot` feature enabled ہو۔

CastZkBallot Verification Path
- `CastZkBallot` فراہم کردہ base64 proof decode کرتا ہے اور خالی یا خراب payloads کو reject کرتا ہے (`BallotRejected` with `invalid or empty proof`).
- Host referendum (`vk_ballot`) یا governance defaults سے ballot verifying key resolve کرتا ہے اور تقاضا کرتا ہے کہ record موجود ہو، `Active` ہو، اور inline bytes رکھتا ہو۔
- Stored verifying-key bytes کو `hash_vk` کے ساتھ دوبارہ hash کیا جاتا ہے؛ commitment mismatch ہو تو verification سے پہلے execution روک دیا جاتا ہے تاکہ tampered registry entries سے بچا جا سکے (`BallotRejected` with `verifying key commitment mismatch`).
- Proof bytes `zk::verify_backend` کے ذریعے registered backend کو dispatch ہوتے ہیں؛ invalid transcripts `BallotRejected` with `invalid proof` کے ساتھ ظاہر ہوتے ہیں اور instruction deterministically fail ہوتی ہے۔
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- Successful proofs `BallotAccepted` emit کرتے ہیں؛ duplicate nullifiers، stale eligibility roots، یا lock regressions پھر بھی پہلے بیان کردہ rejection reasons دیتے ہیں۔

## Validator misbehaviour اور joint consensus

### Slashing اور Jailing workflow

Consensus جب کوئی validator پروٹوکول کی خلاف ورزی کرے تو Norito-encoded `Evidence` emit کرتا ہے۔ ہر payload in-memory `EvidenceStore` میں آتا ہے اور اگر پہلے نہ دیکھا گیا ہو تو WSV-backed `consensus_evidence` map میں materialize ہو جاتا ہے۔ `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocks) سے پرانے ریکارڈ reject ہو جاتے ہیں تاکہ archive bounded رہے، مگر rejection کو operators کے لئے log کیا جاتا ہے۔ Evidence within the horizon also respects `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) and the slashing delay `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`); governance can cancel penalties with `CancelConsensusEvidencePenalty` before slashing applies; the record is marked `penalty_cancelled` and `penalty_cancelled_at_height`.

Recognized offences `EvidenceKind` سے one-to-one map ہوتے ہیں؛ discriminants مستحکم ہیں اور data model کے ذریعے enforce ہوتے ہیں:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - validator نے اسی `(phase,height,view,epoch)` کے لئے متضاد hashes پر دستخط کئے۔
- **InvalidQc** - aggregator نے ایسا commit certificate gossip کیا جس کی شکل deterministic checks میں fail ہو (مثلا خالی signer bitmap)۔
- **InvalidProposal** - leader نے ایسا بلاک propose کیا جو structural validation میں fail ہو (مثلا locked-chain rule توڑے)۔
- **Censorship** — signed submission receipts show a transaction that was never proposed/committed.

Operators اور tooling payloads کو inspect اور re-broadcast کر سکتے ہیں:

- Torii: `GET /v1/sumeragi/evidence` اور `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, اور `... submit --evidence-hex <payload>`.

Governance کو evidence bytes کو canonical proof کے طور پر treat کرنا چاہئے:

1. **Payload جمع کریں** اس کے expire ہونے سے پہلے۔ raw Norito bytes کو height/view metadata کے ساتھ archive کریں۔
2. **Penalty stage کریں** payload کو referendum یا sudo instruction میں embed کر کے (مثلا `Unregister::peer`)۔ Execution payload کو دوبارہ validate کرتا ہے؛ malformed یا stale evidence deterministically reject ہوتی ہے۔
3. **Follow-up topology schedule کریں** تاکہ offending validator فوراً واپس نہ آ سکے۔ عام flows میں `SetParameter(Sumeragi::NextMode)` اور `SetParameter(Sumeragi::ModeActivationHeight)` updated roster کے ساتھ queue کئے جاتے ہیں۔
4. **نتائج audit کریں** `/v1/sumeragi/evidence` اور `/v1/sumeragi/status` کے ذریعے تاکہ evidence counter بڑھے اور governance نے removal نافذ کیا ہو۔

### Joint-consensus sequencing

Joint consensus اس بات کی ضمانت دیتا ہے کہ outgoing validator set boundary block finalize کرے اس سے پہلے کہ نیا set propose شروع کرے۔ Runtime جوڑی شدہ parameters کے ذریعے یہ rule enforce کرتا ہے:

- `SumeragiParameter::NextMode` اور `SumeragiParameter::ModeActivationHeight` کو **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` کو update لانے والے بلاک کی height سے strictly بڑا ہونا چاہیے، تاکہ کم از کم ایک بلاک lag ملے۔
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) configuration guard ہے جو zero-lag hand-offs کو روکتا ہے:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`) delays consensus slashing so governance can cancel penalties before they apply.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Runtime اور CLI staged parameters کو `/v1/sumeragi/params` اور `iroha --output-format text ops sumeragi params` کے ذریعے ظاہر کرتے ہیں، تاکہ operators activation heights اور validator rosters کی تصدیق کر سکیں۔
- Governance automation کو ہمیشہ:
  1. evidence پر مبنی removal (یا reinstatement) فیصلہ finalize کرنا چاہیے۔
  2. `mode_activation_height = h_current + activation_lag_blocks` کے ساتھ follow-up reconfiguration queue کرنا چاہیے۔
  3. `/v1/sumeragi/status` کی نگرانی کرنی چاہیے جب تک `effective_consensus_mode` متوقع height پر switch نہ ہو جائے۔

جو بھی script validators rotate کرے یا slashing apply کرے اسے **zero-lag activation** یا hand-off parameters کو omit نہیں کرنا چاہیے؛ ایسی transactions reject ہو جاتی ہیں اور نیٹ ورک پچھلے mode میں رہتا ہے۔

## Telemetry surfaces

- Prometheus metrics governance activity export کرتے ہیں:
  - `governance_proposals_status{status}` (gauge) proposals کی گنتی status کے حساب سے track کرتا ہے۔
  - `governance_protected_namespace_total{outcome}` (counter) اس وقت increment ہوتا ہے جب protected namespaces کی admission deploy کو allow یا reject کرے۔
  - `governance_manifest_activations_total{event}` (counter) manifest insertions (`event="manifest_inserted"`) اور namespace bindings (`event="instance_bound"`) record کرتا ہے۔
- `/status` میں `governance` object شامل ہے جو proposal counts کو reflect کرتا ہے، protected namespace totals report کرتا ہے، اور recent manifest activations (namespace, contract id, code/ABI hash, block height, activation timestamp) list کرتا ہے۔ Operators اس field کو poll کر کے تصدیق کر سکتے ہیں کہ enactments نے manifests اپڈیٹ کئے اور protected namespace gates نافذ ہیں۔
- Grafana template (`docs/source/grafana_governance_constraints.json`) اور `telemetry.md` میں telemetry runbook دکھاتا ہے کہ stuck proposals، missing manifest activations، یا runtime upgrades کے دوران unexpected protected-namespace rejections کے لئے alerts کیسے wire کئے جائیں۔