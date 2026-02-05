---
lang: he
direction: rtl
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_privacy_commitments.md -->

# מסגרת מחויבויות פרטיות והוכחות (NX-10)

> **סטטוס:** 🈴 Completed (NX-10)  
> **בעלים:** Cryptography WG / Privacy WG / Nexus Core WG  
> **קוד קשור:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 מציגה משטח commit משותף ל-lanes פרטיות. כל dataspace מפרסם מתאר דטרמיניסטי שקושר את
Merkle roots או מעגלי zk-SNARK ל-hashes שניתנים לשחזור. הטבעת הגלובלית של Nexus יכולה לכן לאמת
העברות cross-lane והוכחות סודיות בלי parsers ייעודיים.

## יעדים והיקף

- לנרמל מזהי commitments כדי ש-manifests של governance ו-SDKs יסכימו על ה-slot המספרי שבו משתמשים
  בזמן admission.
- לספק helpers לאימות שניתנים לשימוש חוזר (`iroha_crypto::privacy`) כדי ש-runtimes, Torii ואודיטורים
  off-chain יאמתו proofs בצורה עקבית.
- לקשור zk-SNARK proofs ל-digest הקנוני של verifying key ולקידודי public inputs דטרמיניסטיים. ללא
  transcripts אד-הוק.
- לתעד workflow של registry/export כך ש-lane bundles וראיות governance יכללו את אותם hashes שה-runtime
  אוכף.

מחוץ להיקף של מסמך זה: מנגנוני DA fan-out, relay messaging ו-plumbing של settlement router. ראו
`nexus_cross_lane.md` לשכבות הללו.

## מודל מחויבות lane

ה-registry מאחסן רשימה מסודרת של `LanePrivacyCommitment` entries:

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

- **`LaneCommitmentId`** — מזהה 16-bit יציב שנרשם ב-lane manifests.
- **`CommitmentScheme`** — `Merkle` או `Snark`. גרסאות עתידיות (כגון bulletproofs) מרחיבות את ה-enum.
- **Copy semantics** — כל התיאורים מיישמים `Copy` כדי ש-configs יוכלו להשתמש בהם מחדש ללא heap churn.

ה-registry מגיע יחד עם Nexus lane bundle (`scripts/nexus/lane_registry_bundle.py`). כאשר governance
מאשר commitment חדש, ה-bundle מעדכן גם את manifest ה-JSON וגם את ה-overlay של Norito ש-admission צורך.

### סכימת manifest (`privacy_commitments`)

Lane manifests חושפים כעת מערך `privacy_commitments`. כל entry מקצה ID ו-scheme ומכיל פרמטרים
ספציפיים ל-scheme:

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

Registry bundler מעתיק את manifest, רושם את ה-tuples `(id, scheme)` ב-`summary.json`, ו-CI
(`lane_registry_verify.py`) מפרש מחדש את manifest כדי לוודא שה-summary תואם לתוכן על הדיסק.

## Merkle commitments

`MerkleCommitment` לוכד `HashOf<MerkleTree<[u8;32]>>` קנוני ועומק מקסימלי של audit path. מפעילים
מייצאים את ה-root ישירות מ-prover או snapshot של ה-ledger; אין re-hashing בתוך ה-registry.

זרימת אימות:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- Proofs שחורגים מ-`max_depth` גורמים ל-`PrivacyError::MerkleProofExceedsDepth`.
- Audit paths ממחזרים את `iroha_crypto::MerkleProof` כדי ש-shielded pools ו-Nexus private lanes ישתפו
  אותה serialization.
- Hosts שמכניסים pools חיצוניים ממירים shielded leaves דרך
  `MerkleTree::shielded_leaf_from_commitment` לפני בניית witness.

### רשימת בדיקה תפעולית

- לפרסם את ה-tuple `(id, root, depth)` ב-summary של lane manifest וב-evidence bundle.
- לצרף Merkle inclusion proof לכל העברה cross-lane ב-admission log; ה-helper משחזר את ה-root
  byte-for-byte, ולכן audits רק משווים את ה-hash המיוצא.
- לעקוב אחרי תקציבי עומק ב-telemetry (`nexus_privacy_commitments.merkle.depth_used`) כדי שהתרעות
  יופעלו לפני ש-rollouts חורגים מהמקסימום המוגדר.

## zk-SNARK commitments

`SnarkCircuit` entries קושרים ארבעה שדות:

| Field | Description |
|-------|-------------|
| `circuit_id` | מזהה נשלט governance עבור זוג circuit/version. |
| `verifying_key_digest` | Blake3 hash של verifying key קנוני (DER או Norito encoding). |
| `statement_hash` | Blake3 hash של public-input encoding קנוני (Norito או Borsh). |
| `proof_hash` | Blake3 hash של `verifying_key_digest || proof_bytes`. |

Helpers ל-runtime:

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

- `hash_public_inputs` ו-`hash_proof` נמצאים באותו מודול; SDKs צריכים לקרוא להם בעת יצירת manifests
  או proofs כדי להמנע מ-format drift.
- כל mismatch בין ה-statement hash הרשום לבין ה-public inputs המוצגים גורם ל-
  `PrivacyError::SnarkStatementMismatch`.
- Proof bytes חייבים להיות כבר בפורמט הדחוס הקנוני (Groth16, Plonk וכו'). ה-helper מאמת רק את
  hash binding; אימות עקומות מלא נשאר בשירות prover/verifier.

### Workflow ראיות

1. לייצא verifying key digest ו-proof hash ל-manifest של lane bundle.
2. לצרף את raw proof artefact לחבילת ראיות governance כדי שאודיטורים יוכלו לחשב מחדש `hash_proof`.
3. לפרסם public-input encoding קנוני (hex או Norito) לצד statement hash עבור replays דטרמיניסטיים.

## Proof Ingestion Pipeline

1. **Lane manifests** מצהירים איזה `LaneCommitmentId` שולט בכל חוזה או bucket של programmable-money.
2. **Admission** צורכת `ProofAttachment.lane_privacy` entries, מאמתת את ה-witnesses המצורפים
   ל-lane המנותבת דרך `LanePrivacyRegistry::verify`, ומעבירה את commitment ids המאומתים ל-lane
   compliance (`privacy_commitments_any_of`) כדי שחוקי allow/deny של programmable-money ידרשו
   קיום proofs לפני enqueue.
3. **Telemetry** מגדילה מונים `nexus_privacy_commitments.{merkle,snark}` עם `LaneId`, `commitment_id`
   ותוצאת outcome (`ok`, `depth_mismatch`, וכו').
4. **Governance evidence** משתמשת באותו helper כדי לייצר acceptance reports מחדש
   (`ci/check_nexus_lane_registry_bundle.sh` מתחבר לאימות bundle כאשר מטאדאטה SNARK מגיעה).

### נראות אופרטורית

Endpoint של Torii `/v1/sumeragi/status` חושף כעת את המערך `lane_governance[].privacy_commitments`
כדי שמפעילים ו-SDKs יוכלו להשוות את ה-registry החי מול manifests שפורסמו בלי לקרוא מחדש את bundle.
ה-snapshot נבנה בתוך `crates/iroha_core/src/sumeragi/status.rs`, מיוצא על ידי Torii REST/JSON
handlers (`crates/iroha_torii/src/routing.rs`), ומפוענח על ידי כל client
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`),
ומשקף את סכימת manifest עבור Merkle roots ו-SNARK digests.

Commitment-only או split-replica lanes נכשלים ב-admission אם manifest שלהם משמיט את סעיף
`privacy_commitments`, כדי להבטיח שזרימות programmable-money לא יתחילו עד שעוגני proofs דטרמיניסטיים
נשלחים עם ה-bundle.

## Runtime Registry & Admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) עושה snapshot למבני
  `LaneManifestStatus` מה-manifest loader ומאחסן מפות commitments לכל lane לפי `LaneCommitmentId`.
  ה-transaction queue ו-`State` מתקינים את ה-registry הזה לצד כל manifest reload
  (`Queue::install_lane_manifests`, `State::install_lane_manifests`), כך ש-admission ו-consensus
  validation תמיד ניגשים ל-commitments הקנוניים.
- `LaneComplianceContext` נושא כעת הפניה אופציונלית `Arc<LanePrivacyRegistry>`
  (`crates/iroha_core/src/compliance/mod.rs`). כאשר `LaneComplianceEngine` בוחן טרנזקציה,
  זרימות programmable-money יכולות לבדוק את אותם commitments per-lane שמפורסמים ב-Torii לפני
  enqueue של ה-payload.
- Admission ו-core validation מחזיקים `Arc<LanePrivacyRegistry>` לצד handle של governance manifest
  (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`), ומבטיחים שמודולי
  programmable-money ו-future interlane hosts קוראים תצוגה עקבית של privacy descriptors גם כאשר
  manifests מתחלפים.

## Runtime Enforcement

Lane privacy proofs כעת נוסעים יחד עם transaction attachments דרך `ProofAttachment.lane_privacy`.
Torii admission וה-validation ב-`iroha_core` מאמתים כל attachment מול registry של ה-lane המנותבת
באמצעות `LanePrivacyRegistry::verify`, מתעדים את proven commitment ids ומעבירים אותם ל-compliance
engine. כל כלל `privacy_commitments_any_of` מעריך ל-`false` אלא אם attachment תואם מאומת בהצלחה,
כך ש-programmable-money lanes לא יוכלו להיכנס ל-queue או commit בלי ה-witness הנדרש. כיסוי יחידות
קיים ב-`interlane::tests` וב-lane compliance tests כדי לייצב את path של attachments ואת ה-policy
guardrails.

לנסיונות מידיים, ראו את בדיקות היחידה ב-
[`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) שמדגימות מקרי הצלחה וכשל עבור Merkle ו-
zk-SNARK commitments.

</div>
