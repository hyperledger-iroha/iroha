---
lang: ur
direction: rtl
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_privacy_commitments.md -->

# پرائیویسی کمٹمنٹ اور پروف فریم ورک (NX-10)

> **اسٹیٹس:** مکمل (NX-10)  
> **مالکان:** Cryptography WG / Privacy WG / Nexus Core WG  
> **متعلقہ کوڈ:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 پرائیویٹ lanes کے لئے ایک مشترک commitment surface متعارف کراتا ہے۔ ہر dataspace ایک
deterministic descriptor شائع کرتا ہے جو اپنی Merkle roots یا zk-SNARK circuits کو قابلِ تکرار
hashes سے باندھتا ہے۔ اس طرح گلوبل Nexus ring کراس-لین ٹرانسفرز اور confidentiality proofs کو
bespoke parsers کے بغیر validate کر سکتا ہے۔

## مقاصد اور دائرہ کار

- کمٹمنٹ identifiers کو canonical بنانا تاکہ governance manifests اور SDKs admission کے دوران
  استعمال ہونے والے numeric slot پر متفق رہیں۔
- قابلِ دوبارہ استعمال verification helpers (`iroha_crypto::privacy`) فراہم کرنا تاکہ runtimes،
  Torii اور off-chain auditors proofs کو یکساں طور پر evaluate کریں۔
- zk-SNARK proofs کو canonical verifying-key digest اور deterministic public-input encodings سے
  bind کرنا۔ کوئی ad-hoc transcripts نہیں۔
- registry/export workflow کو دستاویز کرنا تاکہ lane bundles اور governance evidence میں وہی
  hashes شامل ہوں جو runtime نافذ کرتا ہے۔

اس نوٹ کے دائرہ کار سے باہر: DA fan-out mechanics، relay messaging اور settlement router plumbing۔
ان layers کے لئے `nexus_cross_lane.md` دیکھیں۔

## لین کمٹمنٹ ماڈل

Registry `LanePrivacyCommitment` entries کی ایک مرتب فہرست محفوظ کرتا ہے:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** — lane manifests میں ریکارڈ شدہ 16-bit stable identifier۔
- **`CommitmentScheme`** — `Merkle` یا `Snark`۔ مستقبل کی variants (مثلاً bulletproofs) enum کو
  extend کرتی ہیں۔
- **Copy semantics** — تمام descriptors `Copy` implement کرتے ہیں تاکہ configs انہیں heap churn کے
  بغیر reuse کر سکیں۔

Registry Nexus lane bundle (`scripts/nexus/lane_registry_bundle.py`) کے ساتھ چلتا ہے۔ جب governance
ایک نیا commitment منظور کرتی ہے، bundle JSON manifest اور admission کے ذریعے consume ہونے والے
Norito overlay دونوں کو update کرتا ہے۔

### Manifest Schema (`privacy_commitments`)

Lane manifests اب `privacy_commitments` array expose کرتے ہیں۔ ہر entry ایک ID اور scheme تفویض
کرتی ہے اور scheme-specific parameters شامل کرتی ہے:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x...",
        "statement_hash": "0x...",
        "proof_hash": "0x..."
      }
    }
  ]
}
```

Registry bundler manifest کو copy کرتا ہے، commitment `(id, scheme)` tuples کو `summary.json` میں
ریکارڈ کرتا ہے، اور CI (`lane_registry_verify.py`) manifest کو دوبارہ parse کرتی ہے تاکہ summary
disk contents سے match کرے۔

## Merkle Commitments

`MerkleCommitment` ایک canonical `HashOf<MerkleTree<[u8;32]>>` اور زیادہ سے زیادہ audit-path depth
capture کرتا ہے۔ Operators root کو اپنے prover یا ledger snapshot سے براہ راست export کرتے ہیں؛
registry کے اندر re-hashing نہیں ہوتا۔

تصدیقی فلو:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- جو proofs `max_depth` سے بڑھ جائیں وہ `PrivacyError::MerkleProofExceedsDepth` emit کرتے ہیں۔
- Audit paths `iroha_crypto::MerkleProof` utilities کو reuse کرتے ہیں تاکہ shielded pools اور Nexus
  private lanes ایک ہی serialization شیئر کریں۔
- جو hosts external pools ingest کرتے ہیں وہ witness بنانے سے پہلے shielded leaves کو
  `MerkleTree::shielded_leaf_from_commitment` کے ذریعے convert کرتے ہیں۔

### عملی چیک لسٹ

- lane manifest summary اور evidence bundle میں `(id, root, depth)` tuple publish کریں۔
- ہر cross-lane transfer کے لئے Merkle inclusion proof کو admission log کے ساتھ attach کریں؛ helper
  root کو byte-for-byte reproduce کرتا ہے، اس لئے audits صرف exported hash کو compare کرتے ہیں۔
- telemetry میں depth budgets (`nexus_privacy_commitments.merkle.depth_used`) track کریں تاکہ alerts
  configured maxima سے پہلے fire ہوں۔

## zk-SNARK Commitments

`SnarkCircuit` entries چار fields باندھتی ہیں:

| فیلڈ | تفصیل |
|-------|-------------|
| `circuit_id` | سرکٹ/ورژن جوڑے کے لئے governance-controlled شناخت۔ |
| `verifying_key_digest` | canonical verifying key (DER یا Norito encoding) کا Blake3 hash۔ |
| `statement_hash` | canonical public-input encoding (Norito یا Borsh) کا Blake3 hash۔ |
| `proof_hash` | `verifying_key_digest || proof_bytes` کا Blake3 hash۔ |

Runtime helpers:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` اور `hash_proof` ایک ہی module میں ہیں؛ SDKs کو manifests یا proofs بناتے
  وقت انہیں call کرنا چاہئے تاکہ format drift سے بچا جا سکے۔
- recorded statement hash اور پیش کیے گئے public inputs کے درمیان کوئی mismatch
  `PrivacyError::SnarkStatementMismatch` دیتا ہے۔
- Proof bytes پہلے سے اپنی canonical compressed form (Groth16, Plonk, وغیرہ) میں ہونے چاہئیں۔
  helper صرف hash binding verify کرتا ہے؛ مکمل curve verification prover/verifier service میں رہتی
  ہے۔

### Evidence Workflow

1. verifying key digest اور proof hash کو lane bundle manifest میں export کریں۔
2. raw proof artefact کو governance evidence package کے ساتھ attach کریں تاکہ auditors
   `hash_proof` دوبارہ compute کر سکیں۔
3. deterministic replays کے لئے statement hash کے ساتھ canonical public-input encoding (hex یا
   Norito) publish کریں۔

## Proof Ingestion Pipeline

1. **Lane manifests** یہ بتاتے ہیں کہ ہر contract یا programmable-money bucket کو کون سا
   `LaneCommitmentId` govern کرتا ہے۔
2. **Admission** `ProofAttachment.lane_privacy` entries consume کرتا ہے، routed lane کے لئے attached
   witnesses کو `LanePrivacyRegistry::verify` کے ذریعے verify کرتا ہے، اور validated commitment ids
   کو lane compliance (`privacy_commitments_any_of`) میں feed کرتا ہے تاکہ programmable-money
   allow/deny rules queue کرنے سے پہلے proof کی موجودگی enforce کریں۔
3. **Telemetry** `nexus_privacy_commitments.{merkle,snark}` counters کو `LaneId`, `commitment_id` اور
   outcome (`ok`, `depth_mismatch`, وغیرہ) کے ساتھ increment کرتی ہے۔
4. **Governance evidence** وہی helper استعمال کر کے acceptance reports regenerate کرتی ہے
   (`ci/check_nexus_lane_registry_bundle.sh` bundle verification میں تب hook ہوتا ہے جب SNARK
   metadata آتی ہے)۔

### Operator Visibility

Torii کا `/v1/sumeragi/status` endpoint اب `lane_governance[].privacy_commitments` array expose کرتا
ہے تاکہ operators اور SDKs live registry کو published manifests کے ساتھ diff کر سکیں بغیر bundle
دوبارہ پڑھنے کے۔ snapshot `crates/iroha_core/src/sumeragi/status.rs` میں بنتا ہے، Torii کے REST/JSON
handlers (`crates/iroha_torii/src/routing.rs`) کے ذریعے export ہوتا ہے، اور ہر client
(`javascript/iroha_js/src/toriiClient.js`, `python/iroha_python/src/iroha_python/client.py`,
`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`) کے ذریعے decode ہوتا ہے، جو Merkle roots اور
SNARK digests دونوں کے لئے manifest schema کو reflect کرتا ہے۔

Commitment-only یا split-replica lanes اب admission میں fail ہوتی ہیں اگر ان کے manifest میں
`privacy_commitments` سیکشن غائب ہو، جس سے programmable-money flows اس وقت تک شروع نہیں ہو سکتیں
جب تک deterministic proof anchors bundle کے ساتھ ship نہ ہوں۔

## Runtime Registry & Admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) manifest loader کی
  `LaneManifestStatus` structures کا snapshot لیتا ہے اور `LaneCommitmentId` سے keyed per-lane
  commitment maps محفوظ کرتا ہے۔ Transaction queue اور `State` اس registry کو ہر manifest reload
  کے ساتھ install کرتے ہیں (`Queue::install_lane_manifests`, `State::install_lane_manifests`)،
  تاکہ admission path اور consensus validation کے پاس ہمیشہ canonical commitments تک رسائی ہو۔
- `LaneComplianceContext` اب ایک optional `Arc<LanePrivacyRegistry>` reference رکھتا ہے
  (`crates/iroha_core/src/compliance/mod.rs`)۔ جب `LaneComplianceEngine` ایک transaction evaluate
  کرتا ہے، programmable-money flows Torii کے ذریعے expose ہونے والے اسی per-lane commitments کو
  payload queue کرنے سے پہلے inspect کر سکتے ہیں۔
- Admission اور core validation governance manifest handle (`crates/iroha_core/src/queue.rs`,
  `crates/iroha_core/src/state.rs`) کے ساتھ ایک `Arc<LanePrivacyRegistry>` رکھتے ہیں، جو یقینی
  بناتا ہے کہ programmable-money modules اور مستقبل کے interlane hosts manifests rotate ہونے کے
  باوجود privacy descriptors کا consistent view پڑھیں۔

## Runtime Enforcement

Lane privacy proofs اب `ProofAttachment.lane_privacy` کے ذریعے transaction attachments کے ساتھ
جاتی ہیں۔ Torii admission اور `iroha_core` validation ہر attachment کو routed lane کے registry کے
خلاف `LanePrivacyRegistry::verify` سے verify کرتے ہیں، ثابت شدہ commitment ids ریکارڈ کرتے ہیں،
اور انہیں compliance engine میں feed کرتے ہیں۔ کوئی بھی rule جو `privacy_commitments_any_of`
specify کرے اب `false` evaluate ہوتی ہے جب تک کوئی matching attachment کامیابی سے verify نہ ہو
جائے، لہذا programmable-money lanes required witness کے بغیر queue یا commit نہیں ہو سکتیں۔ Unit
coverage `interlane::tests` اور lane compliance tests میں ہے تاکہ attachment path اور policy
guardrails stable رہیں。

فوری تجربات کے لئے [`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) میں unit tests دیکھیں
جو Merkle اور zk-SNARK commitments کے لئے کامیابی اور ناکامی دونوں کیسز دکھاتے ہیں۔

</div>
